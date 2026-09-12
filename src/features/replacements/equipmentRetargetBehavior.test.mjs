import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "shared/i18n/index.ts": `export { resolveCopy } from "./locales.ts"; export const useI18n = () => ({ locale: globalThis.__equipment.locale });`,
  "shared/feedback/index.ts": `export const useFeedback = () => ({ pushToast: () => {} });`,
  "app/routing/useAppRoute.ts": `export const useAppRoute = () => ({ navigate: () => {} });`,
  "features/replacements/equipmentRetargetApi.ts": `
    export const getEquipmentRetargetConfiguration = (input) => globalThis.__equipment.request("configuration", input);
    export const previewEquipmentRetargetInstall = (input) => globalThis.__equipment.request("preview", input);
    export const previewEquipmentRetargetReinstall = (input) => globalThis.__equipment.request("switchPreview", input);
    export const previewEquipmentReapply = (input) => globalThis.__equipment.request("reapplyPreview", input);
    export const startEquipmentReapply = (input, token) => globalThis.__equipment.request("reapplyStart", { ...input, token });
    export const startEquipmentRetargetInstall = (input) => globalThis.__equipment.request("start", input);
    export const startEquipmentRetargetReinstall = (input, token) => globalThis.__equipment.request("switchStart", { ...input, token });`,
  "features/replacements/replacementApi.ts": `
    export const cancelRetargetInstallTask = (input) => globalThis.__equipment.request("cancel", input);
    export const analyzeImportedModReplacement = async () => {};
    export const listReplacementTargetOccupancy = async () => [];
    export const listReplacementTargets = async () => [];
    export const getModReplacementSummary = async () => {};
    export const previewInitialRetargetInstall = async () => {};
    export const previewRetargetReinstall = async () => {};
    export const startRetargetInstallTask = async () => {};
    export const startRetargetReinstallTask = async () => {};`,
}, { "@tauri-apps/api/event": "export const listen = (_name, callback) => globalThis.__equipment.listen(callback);" });

const { EquipmentRetargetGroup } = await import("./EquipmentRetargetPanel.tsx");
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const options = { concurrency: false, timeout: 5000 };
const text = (node) => typeof node === "string" ? node : Array.isArray(node) ? node.map(text).join("") : (node?.children ?? []).map(text).join("");
const target = (id, type = "weapon") => ({ id, gameId: "mhw", targetType: type, internalId: id,
  displayNames: { zh_cn: `名称${id}`, en: `Name ${id}`, ja: `名前${id}` }, aliases: [], aliasesByLocale: {} });
const weapon = { ...target("weapon-b"), aliases: ["升级武器", "Upgraded Weapon", "強化武器"], aliasesByLocale: { zh_cn: ["升级武器"], en: ["Upgraded Weapon"], ja: ["強化武器"] } };
const configuration = {
  gameId: "mhw", modId: "mod-a", installedTargets: {}, warnings: [],
  sources: [
    { source: { id: "source-weapon", internalId: "weapon-a", sourceType: "weapon", supported: true, displayNames: target("weapon-a").displayNames }, originalTargetId: "weapon-a", targets: [target("weapon-a"), weapon, target("weapon-c")] },
    { source: { id: "source-armor", internalId: "armor-a", sourceType: "armor", supported: true, displayNames: target("armor-a").displayNames }, originalTargetId: "armor-a", targets: [target("armor-a", "armor"), target("armor-b", "armor")] },
  ],
};

async function mount(t, overrides = {}) {
  const api = { locale: "zh_cn", calls: [], listeners: new Set(), pending: [], completed: 0, failListen: false, earlyComplete: false, failRefresh: false,
    config: structuredClone(configuration), ...overrides };
  if (api.installed && !api.legacy) api.config.installedTargets = { "source-weapon": "weapon-b", "source-armor": "armor-b" };
  api.listen = async (callback) => {
    if (api.failListen) throw new Error("fixture listener failure");
    api.listeners.add(callback); return () => api.listeners.delete(callback);
  };
  api.emit = (status, phase, taskId = "equipment-task") => {
    for (const callback of api.listeners) callback({ payload: { kind: "install", taskId, status, phase } });
  };
  api.preview = () => ({ analysis: { sources: [], warnings: [] }, targets: [], warnings: [],
    installPlan: { actions: [{}, {}], conflicts: api.blocked ? [{}] : [], hasBlockingConflicts: Boolean(api.blocked) },
    prerequisiteDecision: { status: "ready", codes: [] } });
  api.reapplyPreview = () => ({ status: api.noChanges ? "no_changes" : api.blocked ? "blocked" : "ready", planToken: api.noChanges || api.blocked ? null : "reapply-plan",
    counts: { retained: 2, replaced: 0, added: api.noChanges ? 0 : 1, stale: api.noChanges ? 0 : 1 }, blockingReasons: api.blocked ? [{ code: "target_changed", count: 1 }] : [],
    prerequisiteDecision: { status: "ready", codes: [] }, fileEffects: api.fileEffects });
  api.request = async (kind, input) => {
    api.calls.push({ kind, input });
    if (kind === "configuration") return api.config;
    if (kind === "reapplyPreview") return api.holdReapply ? new Promise((resolve) => api.pending.push(resolve)) : api.reapplyPreview();
    if (kind === "preview") return api.holdPreview ? new Promise((resolve) => api.pending.push(resolve)) : api.preview();
    if (kind === "switchPreview") return { status: api.blocked ? "blocked" : "ready", planToken: api.blocked ? null : "equipment-plan", counts: { retained: 2, replaced: 0, added: 1, stale: 1 },
      attachmentCounts: api.attachmentCounts,
      blockingReasons: api.blocked ? [{ code: "original_install_unverified", count: 1 }] : [], prerequisiteDecision: { status: "ready", codes: [] } };
    if (kind === "cancel") return { taskId: input.taskId, kind: "install", status: "cancelled" };
    if (kind === "start" || kind === "switchStart" || kind === "reapplyStart") {
      if (api.earlyComplete) {
        for (let index = 0; index < (api.noise ?? 0); index += 1) api.emit("completed", "install.retarget.completed", `unrelated-${index}`);
        api.emit("completed", "install.retarget.completed");
        api.emit("running", "install.retarget.plan.building");
      }
      return { taskId: "equipment-task", kind: "install", status: "queued" };
    }
    throw new Error(kind);
  };
  globalThis.__equipment = api;
  let root;
  let props = { gameId: "mhw", modId: "mod-a", profileId: "default", installStatus: api.installed ? "installed" : "not_installed",
    completedLocally: false, initialConfiguration: api.config, onBusyChange: (busy) => { api.busy = busy; },
    onInstallCompleted: async () => { api.completed += 1; if (api.failRefresh) throw new Error("fixture refresh failure"); } };
  const tree = () => React.createElement(React.StrictMode, null, React.createElement(EquipmentRetargetGroup, { ...props, key: JSON.stringify([props.gameId, props.profileId, props.modId]) }));
  await act(async () => { root = TestRenderer.create(tree()); });
  t.after(async () => { await act(async () => root.unmount()); delete globalThis.__equipment; });
  const buttons = () => root.root.findByProps({ className: "replacement-panel__actions" }).findAllByType("button");
  return { api, root, buttons,
    choose: async (index, id, alias = null) => act(async () => root.root.findAllByType("select")[index].props.onChange({ target: { value: JSON.stringify([id, alias]) } })),
    click: async (index) => act(async () => { await buttons()[index].props.onClick(); }),
    update: async (next = {}) => act(async () => { props = { ...props, ...next }; root.update(tree()); }),
  };
}

test("group reapply submits no edited target choices and no-changes stays read-only", options, async (t) => {
  const h = await mount(t, { installed: true, noChanges: true });
  await h.choose(0, "weapon-c");
  await act(async () => h.buttons().find((button) => text(button) === "重新应用当前目标").props.onClick());
  assert.deepEqual(h.api.calls.find((call) => call.kind === "reapplyPreview").input, { gameId: "mhw", profileId: "default", modId: "mod-a" });
  assert.ok(text(h.root.toJSON()).includes("文件已符合当前规则，无需更新"));
  assert.equal(h.buttons()[1].props.disabled, true);
  await h.click(1);
  assert.equal(h.api.calls.filter((call) => call.kind.endsWith("Start") || call.kind === "start").length, 0);
});

test("group reapply requires a ready preview and uses the same task completion lifecycle", options, async (t) => {
  const h = await mount(t, { installed: true });
  await act(async () => h.buttons().find((button) => text(button) === "重新应用当前目标").props.onClick());
  await h.click(1);
  assert.deepEqual(h.api.calls.find((call) => call.kind === "reapplyStart").input, { gameId: "mhw", profileId: "default", modId: "mod-a", token: "reapply-plan" });
  await act(async () => h.api.emit("completed", "install.reinstall.completed"));
  assert.equal(h.api.completed, 1);
});

test("changing profile discards an in-flight group reapply preview", options, async (t) => {
  const h = await mount(t, { installed: true, holdReapply: true });
  await act(async () => h.buttons().find((button) => text(button) === "重新应用当前目标").props.onClick());
  assert.equal(h.api.pending.length, 1);
  await h.update({ profileId: "other-profile" });
  await act(async () => h.api.pending[0](h.api.reapplyPreview()));
  assert.ok(!text(h.root.toJSON()).includes("重新应用预览"));
  assert.equal(h.buttons()[1].props.disabled, true);
});

test("equipment preview shows attachment retention and discards its notice on a new selection", options, async (t) => {
  const h = await mount(t, { installed: true, attachmentCounts: { retained: 1, excluded: 2 } });
  await h.choose(0, "weapon-c");
  await h.click(0);
  assert.ok(text(h.root.toJSON()).includes("本次保留 1 个已安装附件"));
  assert.ok(text(h.root.toJSON()).includes("本次未包含 2 个随包插件或工具"));
  await h.choose(0, "weapon-b");
  assert.ok(!text(h.root.toJSON()).includes("本次保留 1 个已安装附件"));
  assert.equal(h.buttons()[1].props.disabled, true);
});

test("group selection sends every source and all names still map to the real target", options, async (t) => {
  const h = await mount(t);
  const labels = h.root.root.findAllByType("option").map(text);
  assert.ok(labels.includes("升级武器 (weapon-b)"));
  await h.choose(0, "weapon-b", "升级武器");
  await h.click(0);
  const request = h.api.calls.find((call) => call.kind === "preview").input;
  assert.deepEqual(request.slots, [
    { action: "retarget", sourceId: "source-weapon", targetId: "weapon-b" },
    { action: "keep", sourceId: "source-armor" },
  ]);
  h.api.locale = "en";
  await h.update();
  assert.equal(h.api.calls.filter((call) => call.kind === "configuration").length, 0);
  await h.click(1);
  assert.deepEqual(h.api.calls.find((call) => call.kind === "start").input, request);
});

test("switching one source preserves the other installed target and carries the preview token", options, async (t) => {
  const h = await mount(t, { installed: true });
  await h.choose(0, "weapon-c");
  await h.click(0);
  await h.click(1);
  const request = h.api.calls.find((call) => call.kind === "switchStart").input;
  assert.equal(request.token, "equipment-plan");
  assert.deepEqual(request.slots, [
    { action: "retarget", sourceId: "source-weapon", targetId: "weapon-c" },
    { action: "retarget", sourceId: "source-armor", targetId: "armor-b" },
  ]);
});

test("legacy equipment verification is explained and a failed proof never enables writing", options, async (t) => {
  const h = await mount(t, { installed: true, legacy: true, blocked: true });
  assert.ok(text(h.root.toJSON()).includes("预览会核对原包和已安装文件"));
  await h.choose(0, "weapon-b");
  await h.click(0);
  assert.ok(text(h.root.toJSON()).includes("无法证明旧安装与原包一致"));
  assert.equal(h.buttons()[1].props.disabled, true);
  await h.click(1);
  assert.equal(h.api.calls.filter((call) => call.kind === "switchStart").length, 0);
});

test("changed selection and changed Mod both discard pending previews", options, async (t) => {
  const h = await mount(t, { holdPreview: true });
  await h.choose(0, "weapon-b");
  await h.click(0);
  await h.choose(0, "weapon-c");
  await act(async () => h.api.pending.shift()(h.api.preview()));
  assert.equal(h.buttons()[1].props.disabled, true);
  await h.click(0);
  await h.update({ modId: "mod-b" });
  await act(async () => h.api.pending.shift()(h.api.preview()));
  assert.equal(h.buttons()[1].props.disabled, true);
  assert.equal(h.api.calls.filter((call) => call.kind === "start").length, 0);
});

test("an early completion survives later progress and refreshes installed facts once", options, async (t) => {
  const h = await mount(t, { earlyComplete: true, noise: 80 });
  await h.choose(0, "weapon-b");
  await h.click(0);
  await h.click(1);
  assert.equal(h.api.completed, 1);
  assert.equal(h.api.calls.filter((call) => call.kind === "configuration").length, 1);
  assert.equal(h.api.busy, false);
});

test("listener failure prevents writes until an explicit listener retry succeeds", options, async (t) => {
  const h = await mount(t, { failListen: true });
  await h.choose(0, "weapon-b");
  await h.click(0);
  assert.equal(h.buttons()[1].props.disabled, true);
  await h.click(1);
  assert.equal(h.api.calls.filter((call) => call.kind === "start").length, 0);
  h.api.failListen = false;
  const retry = h.root.root.findAllByType("button").find((button) => text(button).includes("重试监听"));
  assert.ok(retry);
  await act(async () => retry.props.onClick());
  assert.equal(h.buttons()[1].props.disabled, false);
});

test("blocking conflicts prevent writes even with a working listener", options, async (t) => {
  const h = await mount(t, { blocked: true });
  await h.choose(0, "weapon-b");
  await h.click(0);
  assert.equal(h.buttons()[1].props.disabled, true);
  await h.click(1);
  assert.equal(h.api.calls.filter((call) => call.kind === "start").length, 0);
});

test("cancellation waits for a matching acknowledgement and ignores another task", options, async (t) => {
  const h = await mount(t);
  await h.choose(0, "weapon-b");
  await h.click(0);
  await h.click(1);
  await act(async () => h.api.emit("completed", "install.retarget.completed", "another-task"));
  assert.equal(h.api.completed, 0);
  await h.click(2);
  assert.equal(h.api.busy, false);
  assert.match(text(h.root.toJSON()), /已取消/);
});

test("failed completion refresh prevents repeat writes until explicit refresh succeeds", options, async (t) => {
  const h = await mount(t, { earlyComplete: true, failRefresh: true });
  await h.choose(0, "weapon-b");
  await h.click(0);
  await h.click(1);
  assert.equal(h.api.busy, false);
  assert.equal(h.buttons()[0].props.disabled, true);
  h.api.failRefresh = false;
  const retry = h.root.root.findAllByType("button").find((button) => text(button).includes("刷新") && !button.props.disabled);
  assert.ok(retry);
  await act(async () => retry.props.onClick());
  assert.equal(h.api.completed, 2);
  assert.equal(h.buttons()[0].props.disabled, false);
});
