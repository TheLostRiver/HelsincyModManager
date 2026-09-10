import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "shared/i18n/index.ts": `export { resolveCopy } from "./locales.ts"; export const useI18n = () => ({ locale: globalThis.__weaponOptions.locale });`,
  "shared/feedback/index.ts": `export const useFeedback = () => ({ pushToast: () => {} });`,
  "features/replacements/replacementApi.ts": `
    export const analyzeImportedModReplacement = (input) => globalThis.__weaponOptions.request("analysis", input);
    export const listReplacementTargets = (input) => globalThis.__weaponOptions.request("targets", input);
    export const listReplacementTargetOccupancy = (input) => globalThis.__weaponOptions.request("occupancy", input);
    export const previewInitialRetargetInstall = (input) => globalThis.__weaponOptions.request("preview", input);
    export const previewRetargetReinstall = (input) => globalThis.__weaponOptions.request("switchPreview", input);
    export const startRetargetInstallTask = (input) => globalThis.__weaponOptions.request("start", input);
    export const startRetargetReinstallTask = (input) => globalThis.__weaponOptions.request("switchStart", input);
    export const cancelRetargetInstallTask = () => Promise.resolve();`,
}, { "@tauri-apps/api/event": "export const listen = (_name, callback) => globalThis.__weaponOptions.listen(callback);" });
const { ReplacementTargetPanel } = await import("./ReplacementTargetPanel.tsx");
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const options = { concurrency: false, timeout: 5000 };
const weapon = {
  id: "physical-model-a", gameId: "mhw", targetType: "weapon", internalId: "bs_swo001",
  displayNames: { zh_cn: "原型刀", en: "Base Sword", ja: "元の刀" },
  aliases: ["最终刀", "升级刀", "Final Sword", "Upgrade Sword", "最終刀", "強化刀"],
  aliasesByLocale: { zh_cn: ["升级刀", "最终刀"], en: ["Final Sword", "Upgrade Sword"], ja: ["強化刀", "最終刀"] },
};
const another = { ...weapon, id: "physical-model-b", internalId: "swo002", displayNames: { zh_cn: "另一把刀", en: "Another Sword", ja: "別の刀" }, aliases: [], aliasesByLocale: {} };
const text = (node) => typeof node === "string" ? node : Array.isArray(node) ? node.map(text).join("") : (node?.children ?? []).map(text).join("");

async function mount(t, { installed = false, occupied = false } = {}) {
  const api = { locale: "zh_cn", calls: [], previews: [], listeners: new Set(), holdPreview: false };
  api.listen = async (callback) => { api.listeners.add(callback); return () => api.listeners.delete(callback); };
  api.preview = (input) => ({ analysis: { gameId: "mhw", sources: [], warnings: [], retargetable: true, matchedAssetCount: 1 },
    target: input.targetId === weapon.id ? weapon : another, actions: [{ sourceInternalId: "swo099", targetInternalId: "bs_swo001" }], warnings: [],
    installPlan: { hasBlockingConflicts: false, actions: [], conflicts: [] }, prerequisiteDecision: { status: "ready", codes: [] } });
  api.request = async (kind, input) => {
    api.calls.push({ kind, input });
    if (kind === "analysis") return { gameId: "mhw", installedTargetId: installed ? weapon.id : undefined,
      retargetable: true, matchedAssetCount: 1, sources: [{ id: "source", sourceType: "weapon", internalId: "swo099", supported: true }], warnings: [] };
    if (kind === "targets") return [weapon, another];
    if (kind === "occupancy") return occupied ? [{ targetId: weapon.id, modId: "other-mod", displayName: "占用者" }] : [];
    if (kind === "preview") {
      if (api.holdPreview) return new Promise((resolve) => api.previews.push({ input, resolve }));
      return api.preview(input);
    }
    if (kind === "switchPreview") return { status: "ready", counts: { retained: 1, replaced: 0, added: 0, stale: 0 }, blockingReasons: [], planToken: "fixture-plan" };
    if (kind === "start" || kind === "switchStart") return { kind: "install", taskId: "task-fixture", status: "queued" };
    throw new Error("Unexpected fixture call: " + kind);
  };
  globalThis.__weaponOptions = api;
  let root;
  let props = { gameId: "mhw", modId: "mod-a", profileId: "profile-a", installStatus: installed ? "installed" : "not_installed",
    completedLocally: false, onBusyChange: () => {}, onInstallCompleted: () => {} };
  const tree = () => React.createElement(React.StrictMode, null, React.createElement(ReplacementTargetPanel, props));
  await act(async () => { root = TestRenderer.create(tree()); });
  t.after(async () => { await act(async () => root.unmount()); delete globalThis.__weaponOptions; });
  return { api, get root() { return root; },
    radios: () => root.root.findAll((node) => node.type === "input" && node.props.type === "radio"),
    buttons: () => root.root.findByProps({ className: "replacement-panel__actions" }).findAllByType("button"),
    choose: async (alias, targetId = weapon.id) => {
      const radio = root.root.findAll((node) => node.type === "input" && node.props.value === JSON.stringify([targetId, alias]))[0];
      await act(async () => radio.props.onChange());
    },
    update: async (nextProps = {}) => { props = { ...props, ...nextProps }; await act(async () => root.update(tree())); },
  };
}

test("expanded names select one row, preview the chosen name, and start using the canonical target ID", options, async (t) => {
  const h = await mount(t);
  assert.equal(h.radios().length, 4);
  assert.equal(new Set(h.radios().map((radio) => radio.props.value)).size, 4);
  await h.choose("最终刀");
  assert.equal(h.radios().filter((radio) => radio.props.checked).length, 1);
  assert.equal(h.buttons()[1].props.disabled, true);
  await act(async () => h.buttons()[0].props.onClick());
  assert.equal(h.api.calls.find((call) => call.kind === "preview").input.targetId, weapon.id);
  assert.ok(text(h.root.root.findByProps({ className: "replacement-panel__preview-facts" })).includes("最终刀 (bs_swo001)"));
  const impact = text(h.root.root.findByProps({ className: "replacement-panel__aliases" }));
  for (const name of ["原型刀", "升级刀", "最终刀"]) assert.ok(impact.includes(name));
  assert.equal(h.buttons()[1].props.disabled, false);
  await act(async () => h.buttons()[1].props.onClick());
  assert.deepEqual(h.api.calls.find((call) => call.kind === "start").input, {
    gameId: "mhw", profileId: "profile-a", modId: "mod-a", targetId: weapon.id, layerName: "base", layerPriority: 0,
  });
});

test("another name on the installed model cannot bypass the same-target switch guard", options, async (t) => {
  const h = await mount(t, { installed: true });
  assert.equal(h.radios().filter((radio) => radio.props.disabled).length, 3);
  await h.choose("最终刀");
  assert.equal(h.buttons()[0].props.disabled, true);
  await act(async () => h.buttons()[0].props.onClick());
  assert.equal(h.api.calls.filter((call) => call.kind === "switchPreview").length, 0);
  await h.choose(null, another.id);
  await act(async () => h.buttons()[0].props.onClick());
  await act(async () => h.buttons()[1].props.onClick());
  assert.equal(h.api.calls.find((call) => call.kind === "switchStart").input.targetId, another.id);
  assert.equal(h.api.calls.find((call) => call.kind === "switchStart").input.planToken, "fixture-plan");
});

test("all names of an occupied model inherit the occupancy guard", options, async (t) => {
  const h = await mount(t, { occupied: true });
  assert.equal(h.root.root.findAll((node) => node.type === "label" && node.props["data-occupied"] === "true").length, 3);
  await h.choose("升级刀");
  assert.equal(h.buttons()[0].props.disabled, true);
  assert.equal(h.buttons()[1].props.disabled, true);
  await act(async () => h.buttons()[0].props.onClick());
  assert.equal(h.api.calls.filter((call) => call.kind === "preview").length, 0);
});

test("locale change preserves the selected model and proven alias instead of guessing a translation", options, async (t) => {
  const h = await mount(t);
  await h.choose("最终刀");
  const queryCount = h.api.calls.length;
  h.api.locale = "en";
  await h.update();
  assert.equal(h.api.calls.length, queryCount, "language changes do not rescan the Mod");
  await act(async () => h.buttons()[0].props.onClick());
  assert.equal(h.api.calls.find((call) => call.kind === "preview").input.targetId, weapon.id);
  assert.ok(text(h.root.root.findByProps({ className: "replacement-panel__preview-facts" })).includes("最终刀 (bs_swo001)"));
});

test("changing profile clears the selected name and rejects late preview results", options, async (t) => {
  const h = await mount(t);
  await h.choose("最终刀");
  h.api.holdPreview = true;
  await act(async () => h.buttons()[0].props.onClick());
  await h.update({ profileId: "profile-b" });
  await act(async () => h.api.previews[0].resolve(h.api.preview(h.api.previews[0].input)));
  assert.equal(h.radios().filter((radio) => radio.props.checked).length, 0);
  assert.equal(h.root.root.findAllByProps({ className: "replacement-panel__preview" }).length, 0);
  assert.equal(h.buttons()[1].props.disabled, true);
});
