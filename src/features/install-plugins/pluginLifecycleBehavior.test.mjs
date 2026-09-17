import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "shared/i18n/index.ts": `export { resolveCopy } from "./locales.ts"; export const useI18n = () => ({ locale: "zh_cn" });`,
  "shared/feedback/index.ts": `import React from "react"; export const Dialog = (props) => React.createElement("section", null, React.createElement("button", { onClick: props.onClose, "data-close": true }, "close"), props.children, props.footer);`,
  "features/mods/ModInstallationProvider.tsx": `export const useModInstallation = () => ({ installationScopeId: globalThis.__pluginFlows.profileId });`,
  "features/mods/ModLifecycleFeedback.tsx": `import React from "react"; export const InstallPlanDetailSheet = ({ state, children }) => React.createElement("section", { "data-state": state.status }, children);`,
  // Floating positioning/focus is covered by the browser suite; keep open/closed content here.
  "features/replacements/RetargetPopover.tsx": `import React, { useState } from "react"; export function RetargetPopover({ trigger, children, feedback }) { const [open, setOpen] = useState(false); return React.createElement("section", null, React.createElement("button", { onClick: () => setOpen(!open), "aria-expanded": open }, trigger), feedback, open ? children : null); }`,
}, {
  "@tauri-apps/api/core": `export const invoke = (command, input) => globalThis.__pluginFlows.invoke(command, input);`,
  "@tauri-apps/api/event": `export const listen = async (_, callback) => { const api = globalThis.__pluginFlows; api.listeners.add(callback); return () => api.listeners.delete(callback); };`,
});

const { ModInstallPreview } = await import("../mods/ModInstallPreview.tsx");
const { InstallConfigOverlay } = await import("../install-config/InstallConfigOverlay.tsx");
const { PackageContentTreeView } = await import("../install-config/PackageContentTreeView.tsx");
const { useModReinstallWorkflow } = await import("../mods/useModReinstallWorkflow.ts");
const { useBatchModLifecycleWorkflow } = await import("../mods/batch-lifecycle/useBatchModLifecycleWorkflow.ts");
const { pluginSelectionCopy } = await import("./pluginSelectionCopy.ts");
const { installConfigCopy } = await import("../install-config/installConfigCopy.ts");
const { installConfigLayoutCopy } = await import("../install-config/installConfigLayoutCopy.ts");
const { ContentRootPanel } = await import("../install-config/ContentRootPanel.tsx");

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const options = { concurrency: false, timeout: 5000 };
const target = { gameId: "mhw", profileId: "profile-a", modId: "mod-a", modName: "Fixture", autoStartWithoutPlugins: true };
const prerequisiteDecision = { status: "ready", codes: [], rulesVersion: 1 };
const copy = pluginSelectionCopy.zh_cn;
const configCopy = installConfigCopy.zh_cn;
const layoutCopy = installConfigLayoutCopy.zh_cn;
const textOf = (node) => typeof node === "string" ? node : Array.isArray(node) ? node.map(textOf).join("") : (node?.children ?? []).map(textOf).join("");
const button = (root, text) => root.root.findAllByType("button").find((node) => textOf(node) === text);
const pluginCheckbox = (root) => root.root.findAllByType("input").find((node) => node.parent?.type === "label" && textOf(node.parent).includes("fixture.dll"));

function preview(revisionId = "revision-a") {
  return { status: "ready", planToken: `plan-${revisionId}`, prerequisiteDecision,
    installedRevision: { revisionId: "revision-a" }, candidateRevision: { revisionId },
    counts: { retained: 1, replaced: 0, added: 0, stale: 1 }, blockingReasons: [], fileEffects: [] };
}

function environment(override) {
  const api = { profileId: "profile-a", installed: false, calls: [], installs: [], listeners: new Set(), closed: 0, refreshed: 0, selections: new Map(), packageExcluded: [], inventoryEpoch: 0 };
  const key = (scope) => JSON.stringify([scope.profileId, scope.modId, scope.revisionId ?? "revision-a"]);
  api.inventory = (scope) => {
    const selection = api.selections.get(key(scope));
    return { gameId: "mhw", profileId: scope.profileId, modId: scope.modId, revisionId: scope.revisionId ?? "revision-a",
      inventoryId: `${key(scope)}:${api.inventoryEpoch}`, confirmationRequired: selection === undefined,
      files: [{ fileId: "plugin-a", relativePath: "nativePC/plugins/fixture.dll", sizeBytes: 512, check: "supported",
        selected: selection === undefined || selection.includes("plugin-a"), selectable: true, managed: api.installed, retainOnly: false, excludedByPackage: false }] };
  };
  api.contents = () => ({ contentRoot: { kind: "fallback", path: "", candidates: [] }, candidates: [""], excludedFiles: api.packageExcluded,
    entries: [{ packageFileId: "nativePC/resource.bin", targetPath: "nativePC/resource.bin", sizeBytes: 3, installable: true, rejectedByGame: false, excludedByPlayer: false }] });
  api.revisions = (modId) => ({ modId, originRevisionId: "revision-a", displayRevisionId: "revision-b", revisions: [{ revisionId: "revision-a" }, { revisionId: "revision-b" }] });
  api.emit = (payload) => api.listeners.forEach((callback) => callback({ payload }));
  api.invoke = async (command, input = {}) => {
    api.calls.push({ command, input });
    const overridden = override?.(command, input, api);
    if (overridden !== undefined) return overridden;
    const request = input.request ?? input;
    switch (command) {
      case "get_mod_plugin_selection": return api.inventory(request);
      case "set_mod_plugin_selection": {
        if (request.inventoryId !== api.inventory(request).inventoryId) throw { code: "plugin_inventory_changed" };
        api.selections.set(key(request), request.selectedFileIds);
        return api.inventory(request);
      }
      case "preview_imported_mod_install_plan": return { actions: [{ targetPath: "nativePC/resource.bin" }], conflicts: [], hasBlockingConflicts: false, prerequisiteDecision };
      case "get_mod_package_contents": return api.contents();
      case "set_mod_package_file_selection": api.packageExcluded = request.excludedFiles; api.inventoryEpoch++; return api.contents();
      case "clear_mod_package_file_selection": api.packageExcluded = []; api.inventoryEpoch++; return api.contents();
      case "get_install_manifest_status": return request.modIds.map((modId) => ({ modId, status: api.installed ? "installed" : "not_installed", installedRevisionId: "revision-a" }));
      case "get_mod_revisions": return api.revisions(request.modId);
      case "preview_reinstall_plan": return preview(request.candidateRevisionId);
      case "preview_equipment_reapply": return preview();
      case "start_reinstall_task":
      case "start_equipment_reapply_task": return { taskId: "fixture-task", kind: "install", status: "queued" };
      case "preview_batch_mod_lifecycle": return { status: "ready", operation: request.operation, executionPolicy: request.executionPolicy,
        previewToken: `preview-${JSON.stringify([...api.selections])}`, readyItemCount: request.items.length, blockedItemCount: 0,
        actionSummary: { actions: 1, retained: 0, replaced: 0, added: 1, stale: 0 }, itemReasons: [], globalReasons: [] };
      case "seal_batch_mod_lifecycle": return { batchId: "batch-a", planToken: "sealed-a" };
      case "start_batch_mod_lifecycle": return { batchId: "batch-a", attemptNumber: 0 };
      case "get_batch_mod_lifecycle_result": return { status: "completed", items: [], nextCursor: null };
      default: throw new Error(`unexpected command ${command}`);
    }
  };
  return api;
}

async function mount(t, render, initial = {}, override) {
  const api = environment(override);
  globalThis.__pluginFlows = api;
  const tree = (props) => React.createElement(React.StrictMode, null, render(api, props));
  let root;
  await act(async () => { root = TestRenderer.create(tree(initial)); });
  t.after(async () => { await act(async () => root.unmount()); delete globalThis.__pluginFlows; });
  return { api, root, update: async (props) => { await act(async () => root.update(tree(props))); } };
}

test("ordinary install confirms defaults once and pins the inventory revision", options, async (t) => {
  const { api, root } = await mount(t, (api) => React.createElement(ModInstallPreview, { target, onClose() {}, onInstall: (revision) => api.installs.push(revision) }));
  assert.deepEqual(api.installs, []);
  assert.ok(api.calls.some((call) => call.command === "preview_imported_mod_install_plan" && call.input.request.profileId === "profile-a"));
  await act(async () => { button(root, copy.confirm).props.onClick(); button(root, copy.confirm).props.onClick(); });
  assert.deepEqual(api.installs, ["revision-a"]);
  assert.equal(api.calls.filter((call) => call.command === "set_mod_plugin_selection").length, 1);
});

test("ordinary install without policy files proceeds once without opening another confirmation", options, async (t) => {
  const { api } = await mount(t, (api) => React.createElement(ModInstallPreview, { target, onClose() {}, onInstall: (revision) => api.installs.push(revision) }), {},
    (command) => command === "get_mod_plugin_selection" ? null : undefined);
  assert.deepEqual(api.installs, [undefined]);
  assert.equal(api.calls.some((call) => call.command === "set_mod_plugin_selection"), false);
});

test("ordinary preview cannot confirm an empty install plan", options, async (t) => {
  const { api, root } = await mount(t, (api) => React.createElement(ModInstallPreview, { target, onClose() {}, onInstall: (revision) => api.installs.push(revision) }), {},
    (command) => command === "preview_imported_mod_install_plan" ? { actions: [], conflicts: [], hasBlockingConflicts: false, prerequisiteDecision } : undefined);
  assert.equal(button(root, copy.confirm).props.disabled, true);
  await act(async () => button(root, copy.confirm).props.onClick());
  assert.deepEqual(api.installs, []);
  assert.equal(api.calls.some((call) => call.command === "set_mod_plugin_selection"), false);
});

test("configuration keeps plugin edits as drafts and discard restores the saved choice", options, async (t) => {
  const { api, root } = await mount(t, (api) => React.createElement(InstallConfigOverlay, { target, onClose: () => api.closed++ }));
  await act(async () => button(root, layoutCopy.attachments(1)).props.onClick());
  await act(async () => pluginCheckbox(root).props.onChange({ target: { checked: false } }));
  assert.equal(api.calls.some((call) => call.command === "set_mod_plugin_selection"), false);
  assert.ok(textOf(root.toJSON()).includes(configCopy.plan.stale(1)));
  await act(async () => button(root, configCopy.actions.discard).props.onClick());
  assert.equal(pluginCheckbox(root).props.checked, true);
  await act(async () => pluginCheckbox(root).props.onChange({ target: { checked: false } }));
  await act(async () => button(root, configCopy.actions.save).props.onClick());
  assert.deepEqual(api.calls.filter((call) => call.command === "set_mod_plugin_selection").at(-1).input.request.selectedFileIds, []);
  assert.equal(api.calls.some((call) => call.command.startsWith("start_")), false);
});

test("configuration blocks closing during an unsaved choice and does not hide a stale save", options, async (t) => {
  const { api, root } = await mount(t, (api) => React.createElement(InstallConfigOverlay, { target, onClose: () => api.closed++ }));
  await act(async () => button(root, layoutCopy.attachments(1)).props.onClick());
  await act(async () => pluginCheckbox(root).props.onChange({ target: { checked: false } }));
  await act(async () => root.root.findByProps({ "data-close": true }).props.onClick());
  assert.equal(api.closed, 0);
  assert.ok(button(root, configCopy.actions.saveAndClose));
  api.inventoryEpoch++;
  await act(async () => button(root, configCopy.actions.saveAndClose).props.onClick());
  assert.equal(api.closed, 0);
  assert.ok(textOf(root.toJSON()).includes(copy.errors.plugin_inventory_changed));
  assert.equal(pluginCheckbox(root).props.checked, false);
});

test("a package change cannot silently approve a plugin draft against a new inventory", options, async (t) => {
  const { api, root } = await mount(t, (api) => React.createElement(InstallConfigOverlay, { target, onClose: () => api.closed++ }));
  await act(async () => button(root, layoutCopy.attachments(1)).props.onClick());
  const planReadsBefore = api.calls.filter((call) => call.command === "preview_imported_mod_install_plan").length;
  await act(async () => root.root.findByType(PackageContentTreeView).props.onToggleSelection("nativePC/resource.bin"));
  await act(async () => pluginCheckbox(root).props.onChange({ target: { checked: false } }));
  await act(async () => button(root, configCopy.actions.save).props.onClick());
  assert.deepEqual(api.packageExcluded, ["nativePC/resource.bin"]);
  assert.equal(api.selections.size, 0);
  assert.equal(api.calls.filter((call) => call.command === "preview_imported_mod_install_plan").length, planReadsBefore + 1);
  assert.ok(textOf(root.toJSON()).includes(copy.errors.plugin_inventory_changed));
  assert.equal(pluginCheckbox(root).props.checked, false);
});

test("reapply waits for the selected plan, accepts early terminal events and refreshes facts", options, async (t) => {
  const { api, root } = await mount(t, (api) => {
    api.installed = true;
    return React.createElement(InstallConfigOverlay, { target, onClose() {} });
  }, {}, (command, _input, api) => {
    if (command === "start_equipment_reapply_task") {
      api.emit({ taskId: "fixture-task", kind: "install", status: "completed", phase: "install.reinstall.completed" });
      return { taskId: "fixture-task", kind: "install", status: "queued" };
    }
  });
  assert.equal(api.calls.some((call) => call.command === "start_equipment_reapply_task"), false);
  await act(async () => button(root, copy.preview).props.onClick());
  const readsBefore = api.calls.filter((call) => call.command === "get_mod_plugin_selection").length;
  await act(async () => { button(root, copy.apply).props.onClick(); button(root, copy.apply).props.onClick(); });
  assert.equal(api.calls.filter((call) => call.command === "start_equipment_reapply_task").length, 1);
  assert.ok(api.calls.filter((call) => call.command === "get_mod_plugin_selection").length > readsBefore);
  assert.equal(button(root, copy.apply), undefined);
});

function ReinstallHarness({ api, profileId }) {
  api.controller = useModReinstallWorkflow({ gameId: "mhw", profileId, selectedItem: { id: "mod-a", name: "Fixture", installSummary: { status: "installed" } },
    writeTaskActive: false, refreshLibrary: () => { api.refreshed++; } });
  return null;
}

test("configuration details and preview preserve the file search without rescanning or saving", options, async (t) => {
  const { api, root } = await mount(t, (api) => { api.installed = true; return React.createElement(InstallConfigOverlay, { target, onClose() {} }); });
  const before = api.calls.filter((call) => call.command === "get_mod_package_contents" || call.command === "get_mod_plugin_selection").length;
  assert.equal(root.root.findByType(ContentRootPanel).findAllByType("fieldset").length, 0);
  await act(async () => button(root, layoutCopy.rootSettings).props.onClick());
  assert.equal(root.root.findByType(ContentRootPanel).findAllByType("fieldset").length, 1);
  await act(async () => button(root, layoutCopy.closeRoot).props.onClick());
  await act(async () => root.root.findByProps({ type: "search", "aria-label": layoutCopy.search }).props.onChange({ target: { value: "resource" } }));
  await act(async () => button(root, copy.preview).props.onClick());
  assert.equal(root.root.findByProps({ className: "replacement-panel retarget-workspace" }).props["data-preview-open"], true);
  await act(async () => button(root, layoutCopy.workspace.collapsePreview).props.onClick());
  assert.equal(root.root.findByProps({ className: "replacement-panel retarget-workspace" }).props["data-preview-open"], false);
  assert.equal(root.root.findByProps({ type: "search", "aria-label": layoutCopy.search }).props.value, "resource");
  assert.equal(api.calls.filter((call) => call.command === "get_mod_package_contents" || call.command === "get_mod_plugin_selection").length, before);
  assert.equal(api.calls.some((call) => /^(set_|start_)/.test(call.command)), false);
});

test("configuration reapply ignores a late preview after the profile changes", options, async (t) => {
  let finishPreview;
  const { api, root, update } = await mount(t, (api) => { api.installed = true; return React.createElement(InstallConfigOverlay, { target, onClose() {} }); }, {},
    (command) => command === "preview_equipment_reapply" ? new Promise((resolve) => { finishPreview = resolve; }) : undefined);
  await act(async () => button(root, copy.preview).props.onClick());
  api.profileId = "profile-b";
  await update({});
  await act(async () => finishPreview(preview()));
  assert.equal(button(root, copy.apply), undefined);
  assert.equal(root.root.findByProps({ className: "replacement-panel retarget-workspace" }).props["data-preview-open"], false);
  assert.equal(api.calls.some((call) => call.command.startsWith("start_")), false);
});

test("changing the starting directory prevents closing until the saved result returns", options, async (t) => {
  let finishRoot;
  const base = { contentRoot: { kind: "fallback", path: "", candidates: [] }, candidates: ["", "wrapper"], excludedFiles: [], entries: [] };
  const { api, root } = await mount(t, (api) => React.createElement(InstallConfigOverlay, { target, onClose: () => api.closed++ }), {},
    (command) => command === "get_mod_package_contents" ? base : command === "set_mod_package_content_root" ? new Promise((resolve) => { finishRoot = resolve; }) : undefined);
  await act(async () => button(root, layoutCopy.rootSettings).props.onClick());
  await act(async () => { root.root.findByProps({ type: "radio", value: "wrapper" }).props.onChange(); });
  await act(async () => root.root.findByProps({ "data-close": true }).props.onClick());
  assert.equal(api.closed, 0);
  assert.equal(button(root, configCopy.actions.saving).props.disabled, true);
  await act(async () => finishRoot({ ...base, contentRoot: { kind: "single", path: "wrapper", candidates: [] } }));
  await act(async () => root.root.findByProps({ "data-close": true }).props.onClick());
  assert.equal(api.closed, 1);
});

test("reinstall queries the candidate revision and drops a late preview after a profile switch", options, async (t) => {
  let finishPreview;
  const { api, update } = await mount(t, (api, props) => React.createElement(ReinstallHarness, { api, ...props }), { profileId: "profile-a" },
    (command) => command === "preview_reinstall_plan" ? new Promise((resolve) => { finishPreview = resolve; }) : undefined);
  await act(async () => api.controller.openReinstall());
  assert.equal(api.controller.plugins.inventory.revisionId, "revision-b");
  await act(async () => api.controller.selectCandidateRevision("revision-a"));
  assert.equal(api.controller.plugins.inventory.revisionId, "revision-a");
  await act(async () => api.controller.generatePreview());
  await update({ profileId: "profile-b" });
  await act(async () => finishPreview(preview()));
  assert.equal(api.controller.dialogState.status, "closed");
  assert.equal(api.controller.canConfirm, false);
  assert.equal(api.calls.some((call) => call.command === "start_reinstall_task"), false);
});

test("reinstall does not start after confirmation returns to a different profile", options, async (t) => {
  let finishSave;
  const { api, update } = await mount(t, (api, props) => React.createElement(ReinstallHarness, { api, ...props }), { profileId: "profile-a" },
    (command, input, api) => command === "set_mod_plugin_selection" ? new Promise((resolve) => { finishSave = () => resolve({ ...api.inventory(input.request), confirmationRequired: false }); }) : undefined);
  await act(async () => api.controller.openReinstall());
  await act(async () => api.controller.generatePreview());
  await act(async () => api.controller.confirmReinstall());
  await update({ profileId: "profile-b" });
  await act(async () => finishSave());
  assert.equal(api.controller.dialogState.status, "closed");
  assert.equal(api.calls.some((call) => call.command === "start_reinstall_task"), false);
});

function BatchHarness({ api, profileId = "profile-a", sameRevision = false }) {
  api.controller = useBatchModLifecycleWorkflow({ gameId: "mhw", profileId,
    loadManifestStatuses: async (modIds) => modIds.map((modId) => ({ modId, status: sameRevision ? "installed" : "not_installed", installedRevisionId: sameRevision ? "revision-a" : null })),
    loadRevisions: async (modId) => ({ ...api.revisions(modId), displayRevisionId: sameRevision ? "revision-a" : "revision-b" }),
    loadReplacementTargetFacts: async (modIds) => modIds.map((modId) => ({ modId, retargetable: false, installedTargetId: null, targets: [] })),
  });
  return null;
}

test("batch plugin changes invalidate the old confirmation and seal only the refreshed preview", options, async (t) => {
  let releaseSave;
  const { api } = await mount(t, (api) => React.createElement(BatchHarness, { api }), {}, (command, input, api) => {
    if (command === "set_mod_plugin_selection") return new Promise((resolve) => { releaseSave = () => {
      api.selections.set(JSON.stringify([input.request.profileId, input.request.modId, input.request.revisionId]), input.request.selectedFileIds);
      resolve(api.inventory(input.request));
    }; });
  });
  await act(async () => api.controller.prepare("install", ["mod-a"]));
  const oldToken = api.controller.state.preview.previewToken;
  let change;
  await act(async () => { change = api.controller.changePlugin(api.controller.pluginChoices[0], "plugin-a", false); });
  await act(async () => api.controller.confirmAndStart());
  assert.equal(api.calls.some((call) => call.command === "seal_batch_mod_lifecycle"), false);
  await act(async () => { releaseSave(); await change; });
  assert.notEqual(api.controller.state.preview.previewToken, oldToken);
  const newToken = api.controller.state.preview.previewToken;
  await act(async () => api.controller.confirmAndStart());
  assert.equal(api.calls.find((call) => call.command === "seal_batch_mod_lifecycle").input.previewToken, newToken);
  assert.equal(api.controller.state.status, "result");
});

test("batch supports explicit reapply without equipment and resets its preview on scope changes", options, async (t) => {
  const { api, update } = await mount(t, (api, props) => React.createElement(BatchHarness, { api, ...props }), { sameRevision: true });
  await act(async () => api.controller.prepare("reinstall", ["mod-a"]));
  assert.equal(api.controller.state.status, "target-selection");
  await act(async () => api.controller.setReapplyTarget("mod-a"));
  await act(async () => api.controller.previewWithReplacementTargets());
  const request = api.calls.find((call) => call.command === "preview_batch_mod_lifecycle").input.request;
  assert.equal(request.items[0].intent, "reapply_equipment_targets");
  assert.equal(request.replacementTargets, undefined);
  await update({ profileId: "profile-b", sameRevision: true });
  assert.equal(api.controller.state.status, "idle");
  assert.deepEqual(api.controller.pluginChoices, []);
});

test("batch inventory scans stop before the next Mod after switching game installations", options, async (t) => {
  let finishScan;
  const { api, update } = await mount(t, (api, props) => React.createElement(BatchHarness, { api, ...props }), {},
    (command, input, api) => command === "get_mod_plugin_selection" ? new Promise((resolve) => { finishScan = () => resolve(api.inventory(input.request)); }) : undefined);
  let preparing;
  await act(async () => { preparing = api.controller.prepare("install", ["mod-a", "mod-b"]); });
  assert.equal(api.calls.filter((call) => call.command === "get_mod_plugin_selection").length, 1);
  await update({ profileId: "profile-b" });
  await act(async () => { finishScan(); await preparing; });
  assert.equal(api.calls.filter((call) => call.command === "get_mod_plugin_selection").length, 1);
  assert.equal(api.calls.some((call) => call.command === "preview_batch_mod_lifecycle"), false);
  assert.deepEqual(api.controller.pluginChoices, []);
});
