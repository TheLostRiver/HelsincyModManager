import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act, useContext } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "app/routing/useAppRoute.ts": "export const useAppRoute = () => ({ currentRoute: globalThis.__tourTest.route });",
  "shared/onboarding/TourOverlay.tsx": `import { createElement } from "react";
    export const TourOverlay = (props) => createElement("tour-probe", props,
      props.renderStepContent?.(props.steps[props.stepIndex]));`,
});
const { TourProvider } = await import("./TourProvider.tsx");
const { TourContext } = await import("./TourContext.ts");
const { I18nProvider } = await import("../../shared/i18n/I18nProvider.tsx");
const { useI18n } = await import("../../shared/i18n/useI18n.ts");
const { onboardingTourCopy } = await import("./onboardingTourCopy.ts");
const options = { concurrency: false, timeout: 5000 };
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

async function mountTour(t, { preference, progress, route = "dashboard" } = {}) {
  const storage = new Map();
  if (preference) storage.set("helsincy.localePreference", JSON.stringify({ version: 1, preference }));
  if (progress) storage.set("helsincy.onboarding", JSON.stringify({ schemaVersion: 1, tours: {
    "hmm.first-run": { contentVersion: 5, outcome: progress },
  } }));
  const frames = new Map();
  let nextFrame = 0;
  const state = { route: { id: route }, storage };
  const oldWindow = globalThis.window;
  const oldDocument = globalThis.document;
  globalThis.__tourTest = state;
  globalThis.window = {
    localStorage: { getItem: (key) => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) },
    requestAnimationFrame: (callback) => { frames.set(++nextFrame, callback); return nextFrame; },
    cancelAnimationFrame: (id) => frames.delete(id),
    addEventListener() {}, removeEventListener() {},
  };
  globalThis.document = { documentElement: { lang: "" } };
  function Capture() {
    state.tour = useContext(TourContext);
    state.i18n = useI18n();
    return null;
  }
  const tree = () => React.createElement(React.StrictMode, null, React.createElement(I18nProvider, null,
    React.createElement(TourProvider, null, React.createElement(Capture))));
  let root;
  await act(async () => { root = TestRenderer.create(tree()); });
  t.after(async () => {
    await act(async () => root.unmount());
    globalThis.window = oldWindow;
    globalThis.document = oldDocument;
    delete globalThis.__tourTest;
  });
  async function flushFrames() {
    await act(async () => {
      const callbacks = [...frames.values()];
      frames.clear();
      callbacks.forEach((callback) => callback());
    });
  }
  await flushFrames();
  return {
    state, root, flushFrames,
    overlay: () => root.root.findAllByType("tour-probe")[0]?.props,
    select: async (value) => { await act(async () => root.root.findByProps({ type: "radio", value }).props.onChange()); },
    changeRoute: async (id) => {
      state.route = { id };
      await act(async () => root.update(tree()));
      await flushFrames();
    },
  };
}

test("first onboarding step shows every native language and defaults to Chinese", options, async (t) => {
  const h = await mountTour(t);
  assert.equal(h.overlay().steps[0].id, "language");
  assert.equal(h.overlay().stepIndex, 0);
  assert.equal(h.state.i18n.preference, "zh_cn");
  assert.equal(h.root.root.findByProps({ type: "radio", value: "zh_cn" }).props.checked, true);
  const names = h.root.root.findAllByType("span").filter((span) => span.props.lang).map((span) => span.children.join(""));
  assert.deepEqual(names, ["简体中文", "English", "日本語"]);
  assert.equal(h.overlay().steps[1].id, "welcome");
});

test("language selection immediately translates the whole active tour without changing its position", options, async (t) => {
  const h = await mountTour(t);
  const originalIds = h.overlay().steps.map((step) => step.id);
  await h.select("en");
  assert.equal(h.overlay().steps[0].title, onboardingTourCopy.en.language.title);
  assert.equal(h.overlay().steps[1].title, onboardingTourCopy.en.welcome.title);
  assert.equal(h.overlay().steps[2].title, onboardingTourCopy.en.routes.dashboard.features["dashboard-steam-scan"].title);
  assert.equal(h.overlay().stepIndex, 0);
  assert.deepEqual(h.overlay().steps.map((step) => step.id), originalIds);
  assert.equal(JSON.parse(h.state.storage.get("helsincy.localePreference")).preference, "en");
  assert.equal(globalThis.document.documentElement.lang, "en");
  await act(async () => h.overlay().onStepChange(2));
  await act(async () => h.state.i18n.setPreference("ja"));
  assert.equal(h.overlay().stepIndex, 2);
  assert.equal(h.overlay().steps[2].title, onboardingTourCopy.ja.routes.dashboard.features["dashboard-steam-scan"].title);
});

for (const preference of ["en", "ja", "system"]) {
  test(`onboarding preserves saved ${preference} preference`, options, async (t) => {
    const h = await mountTour(t, { preference });
    assert.equal(h.state.i18n.preference, preference);
    assert.equal(h.root.root.findByProps({ type: "radio", value: preference }).props.checked, true);
    assert.equal(JSON.parse(h.state.storage.get("helsincy.localePreference")).preference, preference);
  });
}

test("language radio arrow keys do not advance the tour or suppress native selection", options, async (t) => {
  const h = await mountTour(t);
  const fieldset = h.root.root.findByType("fieldset");
  for (const key of ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"]) {
    let stopped = false;
    fieldset.props.onKeyDown({ key, stopPropagation: () => { stopped = true; }, preventDefault: () => assert.fail("Keep native radio behavior") });
    assert.equal(stopped, true, key);
  }
  for (const key of ["Tab", "Escape", "Enter"]) {
    fieldset.props.onKeyDown({ key, stopPropagation: () => assert.fail(`${key} must reach the tour focus/exit controls`) });
  }
  assert.equal(h.overlay().stepIndex, 0);
});

test("skipping saves progress and language while manual reopening starts with language", options, async (t) => {
  const h = await mountTour(t);
  await h.select("ja");
  await act(async () => h.overlay().onFinish("skipped"));
  assert.equal(Boolean(h.overlay()), false);
  assert.equal(JSON.parse(h.state.storage.get("helsincy.onboarding")).tours["hmm.first-run"].outcome, "skipped");
  await h.flushFrames();
  assert.equal(Boolean(h.overlay()), false);
  await act(async () => h.state.tour.startTour());
  assert.equal(h.overlay().steps[0].id, "language");
  assert.equal(h.overlay().steps[1].id, "dashboard-steam-scan");
  assert.equal(h.state.i18n.preference, "ja");
});

for (const progress of ["completed", "skipped"]) {
  test(`adding language selection does not reopen a previously ${progress} tour`, options, async (t) => {
    const h = await mountTour(t, { progress });
    assert.equal(Boolean(h.overlay()), false);
    await act(async () => h.state.i18n.setPreference("en"));
    await h.flushFrames();
    assert.equal(Boolean(h.overlay()), false);
  });
}

test("changing language keeps route activation checks and the original route order", options, async (t) => {
  const h = await mountTour(t);
  const index = h.overlay().steps.findIndex((step) => step.id === "navigate-mods");
  await act(async () => h.overlay().onStepChange(index));
  await h.changeRoute("mods");
  assert.equal(h.overlay().stepIndex, index, "A route change alone must not advance");
  await act(async () => h.overlay().onTargetActivate("navigate-mods"));
  await act(async () => h.state.i18n.setPreference("en"));
  await h.flushFrames();
  assert.equal(h.overlay().stepIndex, index + 1);
  assert.equal(h.overlay().steps[index + 1].id, "mods-import");
  assert.equal(h.overlay().steps[2].id, "dashboard-steam-scan");
});
