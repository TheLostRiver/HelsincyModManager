import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "app/routing/routeRegistry.tsx": `
    export const appRoutes = [
      { id: "dashboard", path: "/" }, { id: "mods", path: "/mods" },
      { id: "settings", path: "/settings" }, { id: "profiles", path: "/profiles" },
      { id: "future", path: "/future" },
    ];
    export const enabledRouteIds = new Set(["dashboard", "mods", "settings", "profiles"]);
  `,
  "app/appearance/useColorScheme.ts": 'export const useColorScheme = () => ({ preference: "system", setPreference() {} });',
  "shared/i18n/index.ts": `
    export const useI18n = () => ({ locale: "zh_cn", preference: "zh_cn", systemLocale: "zh_cn", setPreference() {} });
    export const resolveCopy = (copy, locale) => copy[locale];
    export const coreLocales = ["zh_cn"];
    export const localeMeta = { zh_cn: { nativeName: "简体中文" } };
  `,
  "shared/feedback/index.ts": "export const useFeedback = () => ({ pushToast() {} });",
  "features/game-setup/useGamePrerequisites.ts": "export const useGamePrerequisites = () => ({});",
  ...Object.fromEntries([
    ["features/game-setup/GamePrerequisitePanel.tsx", "GamePrerequisitePanel"],
    ["features/settings/BackgroundProtectionPanel.tsx", "BackgroundProtectionPanel"],
    ["features/settings/DebugLogSettingsPanel.tsx", "DebugLogSettingsPanel"],
    ["features/settings/ModImportSettingsPanel.tsx", "ModImportSettingsPanel"],
    ["features/settings/ModStorageSettingsPanel.tsx", "ModStorageSettingsPanel"],
  ].map(([path, name]) => [path, `export const ${name} = () => null;`])),
});
const { AppRouteProvider } = await import("./AppRouteProvider.tsx");
const { useAppRoute } = await import("./useAppRoute.ts");
const { SettingsPage } = await import("../../features/settings/SettingsPage.tsx");
const { START_PAGE_STORAGE_KEY, LAST_ROUTE_STORAGE_KEY } = await import("./startPagePreference.ts");

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const options = { concurrency: false, timeout: 5000 };

async function mountApp(t, seed = {}, storageFlags = {}) {
  const values = new Map(Object.entries(seed));
  const flags = { ...storageFlags };
  const previousWindow = globalThis.window;
  globalThis.window = { localStorage: {
    getItem(key) { if (flags.readBlocked) throw new Error("read blocked"); return values.get(key) ?? null; },
    setItem(key, value) { if (flags.writeBlocked) throw new Error("write blocked"); values.set(key, value); },
  } };
  const state = { firstPath: null };
  function Capture() {
    state.route = useAppRoute();
    state.firstPath ??= state.route.currentPath;
    return state.route.currentRoute.id === "settings" ? React.createElement(SettingsPage) : null;
  }
  let root;
  const mount = async () => { await act(async () => {
    root = TestRenderer.create(React.createElement(React.StrictMode, null,
      React.createElement(AppRouteProvider, null, React.createElement(Capture))));
  }); };
  await mount();
  t.after(async () => { await act(async () => root.unmount()); globalThis.window = previousWindow; });
  const group = () => root.root.findAllByType("fieldset").find((node) =>
    node.findByType("legend").children.join("") === "启动后打开");
  return { values, flags, state,
    navigate: async (path) => { await act(async () => state.route.navigate(path)); },
    restart: async () => { await act(async () => root.unmount()); await mount(); },
    choose: async (value) => { await act(async () => group().findAllByType("button")[["dashboard", "mods", "last"].indexOf(value)].props.onClick()); },
    selected: () => group().findAllByType("button").map((node) => node.props["aria-pressed"]),
    alerts: () => root.root.findAllByProps({ role: "alert" }),
    resetPreviews: async () => { await act(async () => root.root.findAllByType("input")[0].props.onChange());
      await act(async () => root.root.findByProps({ className: "settings-reset-button" }).props.onClick()); },
  };
}

test("a saved Mod library choice is applied on the first route render", options, async (t) => {
  const app = await mountApp(t, { [START_PAGE_STORAGE_KEY]: "mods" });
  assert.equal(app.state.firstPath, "/mods");
  assert.equal(app.state.route.currentPath, "/mods");
});

test("selecting a start page saves it without navigating away, and survives page and app remounts", options, async (t) => {
  const app = await mountApp(t);
  await app.navigate("/settings");
  await app.choose("mods");
  assert.equal(app.values.get(START_PAGE_STORAGE_KEY), "mods");
  assert.equal(app.state.route.currentPath, "/settings");
  await app.navigate("/profiles");
  await app.navigate("/settings");
  assert.deepEqual(app.selected(), [false, true, false]);
  await app.resetPreviews();
  assert.deepEqual(app.selected(), [false, true, false]);
  await app.restart();
  assert.equal(app.state.route.currentPath, "/mods");
});

test("dashboard is explicit and is not replaced by the last visited page", options, async (t) => {
  const app = await mountApp(t, { [START_PAGE_STORAGE_KEY]: "mods" });
  await app.navigate("/settings");
  await app.choose("dashboard");
  await app.navigate("/profiles");
  await app.restart();
  assert.equal(app.state.route.currentPath, "/");
});

test("last page restores the last committed route, including settings", options, async (t) => {
  const app = await mountApp(t);
  await app.navigate("/settings");
  await app.choose("last");
  await app.navigate("/profiles");
  await app.restart();
  assert.equal(app.state.route.currentPath, "/profiles");
  await app.navigate("/settings");
  await app.restart();
  assert.equal(app.state.route.currentPath, "/settings");
  assert.deepEqual(app.selected(), [false, false, true]);
});

for (const lastId of ["replacements", "games", "tasks", "future", "https://invalid.example", ""]) {
  test(`unavailable last route ${lastId || "(empty)"} falls back to dashboard`, options, async (t) => {
    const app = await mountApp(t, { [START_PAGE_STORAGE_KEY]: "last", [LAST_ROUTE_STORAGE_KEY]: lastId });
    assert.equal(app.state.route.currentPath, "/");
  });
}

test("invalid preference or storage read failure does not prevent startup", options, async (t) => {
  const app = await mountApp(t, { [START_PAGE_STORAGE_KEY]: "broken", [LAST_ROUTE_STORAGE_KEY]: "profiles" });
  assert.equal(app.state.route.currentPath, "/");
  app.values.set(START_PAGE_STORAGE_KEY, "mods");
  app.flags.readBlocked = true;
  await app.restart();
  assert.equal(app.state.route.currentPath, "/");
});

test("failed preference writes keep the saved choice and show an error; route writes cannot block navigation", options, async (t) => {
  const app = await mountApp(t);
  await app.navigate("/settings");
  app.flags.writeBlocked = true;
  await app.choose("mods");
  assert.deepEqual(app.selected(), [true, false, false]);
  assert.equal(app.alerts().length, 1);
  await app.navigate("/profiles");
  assert.equal(app.state.route.currentPath, "/profiles");
  app.flags.writeBlocked = false;
  await app.navigate("/settings");
  await app.choose("mods");
  assert.equal(app.alerts().length, 0);
  await app.restart();
  assert.equal(app.state.route.currentPath, "/mods");
});

test("a rejected navigation cannot replace the remembered page", options, async (t) => {
  const app = await mountApp(t, { [START_PAGE_STORAGE_KEY]: "last", [LAST_ROUTE_STORAGE_KEY]: "mods" });
  await app.navigate("/profiles");
  await app.navigate("/future");
  assert.equal(app.state.route.currentPath, "/profiles");
  await app.restart();
  assert.equal(app.state.route.currentPath, "/profiles");
});
