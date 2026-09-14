import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "shared/i18n/index.ts": `export { resolveCopy } from "./locales.ts"; export const useI18n = () => ({ locale: globalThis.__plugins.locale });`,
}, { "@tauri-apps/api/core": `export const invoke = (command, input) => globalThis.__plugins.invoke(command, input.request);` });
const { usePluginSelection } = await import("./usePluginSelection.ts");
const { PluginSelectionPanel } = await import("./PluginSelectionPanel.tsx");
const { getModPluginSelection, setModPluginSelection } = await import("./pluginSelectionApi.ts");
const { pluginSelectionCopy } = await import("./pluginSelectionCopy.ts");
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const options = { concurrency: false, timeout: 5000 };
const scope = { gameId: "mhw", profileId: "profile-a", modId: "mod-a", revisionId: "revision-a" };
const inventory = (request = scope) => ({ ...request, revisionId: request.revisionId ?? "revision-a", inventoryId: `inventory-${request.profileId}`, confirmationRequired: true, files: [
  { fileId: "plugin-a", relativePath: "nativePC/plugins/fixture.dll", sizeBytes: 512, check: "supported", selected: true, selectable: true, managed: false, retainOnly: false, excludedByPackage: false },
  { fileId: "tool-a", relativePath: "nativePC/tools/fixture.exe", sizeBytes: 128, check: "policy_excluded", selected: false, selectable: false, managed: false, retainOnly: false, excludedByPackage: false },
] });
const content = (node) => typeof node === "string" ? node : Array.isArray(node) ? node.map(content).join("") : (node?.children ?? []).map(content).join("");

async function mount(t, hookOptions = {}, customInvoke) {
  const api = { locale: "zh_cn", calls: [], controller: null, invalidated: 0, saved: 0 };
  api.invoke = async (command, request) => {
    api.calls.push({ command, request });
    if (customInvoke) return customInvoke(command, request);
    const value = inventory(request);
    if (command === "set_mod_plugin_selection") {
      value.confirmationRequired = false;
      value.files = value.files.map((file) => ({ ...file, selected: request.selectedFileIds.includes(file.fileId) }));
    }
    return value;
  };
  globalThis.__plugins = api;
  function Harness({ currentScope }) {
    api.controller = usePluginSelection(currentScope, { onInvalidated: () => api.invalidated++, onSaved: () => api.saved++, ...hookOptions });
    return React.createElement(PluginSelectionPanel, { controller: api.controller });
  }
  const tree = (currentScope = scope) => React.createElement(React.StrictMode, null, React.createElement(Harness, { currentScope }));
  let root;
  await act(async () => { root = TestRenderer.create(tree()); });
  t.after(async () => { await act(async () => root.unmount()); delete globalThis.__plugins; });
  return { api, root, update: async (value) => { await act(async () => root.update(tree(value))); } };
}

test("supported default choices need one confirmation and wrapper submits only stable IDs", options, async (t) => {
  const { api, root } = await mount(t);
  assert.ok(api.controller.ready);
  assert.equal(api.calls.filter((call) => call.command === "set_mod_plugin_selection").length, 0);
  assert.equal(root.root.findAllByType("input").length, 1);
  assert.ok(content(root.toJSON()).includes("fixture.exe"));
  assert.ok(content(root.toJSON()).includes(pluginSelectionCopy.zh_cn.checks.policy_excluded));
  await act(async () => { await api.controller.confirm(); await api.controller.confirm(); });
  const saves = api.calls.filter((call) => call.command === "set_mod_plugin_selection");
  assert.equal(saves.length, 1);
  assert.deepEqual(saves[0].request, { ...scope, inventoryId: "inventory-profile-a", selectedFileIds: ["plugin-a"] });
  await setModPluginSelection({ ...saves[0].request, force: true, targetPath: "outside", bytes: [1], sha256: "fake" });
  assert.deepEqual(api.calls.at(-1).request, saves[0].request);
  await getModPluginSelection({ ...scope, gameRoot: "outside" });
  assert.deepEqual(api.calls.at(-1).request, scope);
});

test("a tools-only inventory has a reason instead of disabled choices or plugin dependency warnings", options, async (t) => {
  const { root } = await mount(t, {}, (_command, request) => ({ ...inventory(request), files: inventory(request).files.filter((file) => file.check === "policy_excluded") }));
  assert.equal(root.root.findAllByType("input").length, 0);
  assert.ok(content(root.toJSON()).includes(pluginSelectionCopy.zh_cn.checks.policy_excluded));
  assert.equal(content(root.toJSON()).includes(pluginSelectionCopy.zh_cn.dependency), false);
});

test("draft changes can be reversed or discarded without saving and only Save persists", options, async (t) => {
  const { api } = await mount(t, { draft: true });
  await act(async () => { await api.controller.choose("plugin-a", false); });
  assert.equal(api.controller.dirtyCount, 1);
  assert.equal(api.calls.some((call) => call.command === "set_mod_plugin_selection"), false);
  await act(async () => { await api.controller.choose("plugin-a", true); });
  assert.equal(api.controller.dirty, false);
  await act(async () => { await api.controller.choose("plugin-a", false); api.controller.discard(); });
  assert.equal(api.controller.inventory.files[0].selected, true);
  await act(async () => { await api.controller.choose("plugin-a", false); await api.controller.confirm(); });
  assert.equal(api.controller.dirty, false);
  assert.deepEqual(api.calls.at(-1).request.selectedFileIds, []);
});

test("late profile responses never replace the current inventory", options, async (t) => {
  const pending = [];
  const { api, update } = await mount(t, {}, (command, request) => new Promise((resolve) => pending.push({ command, request, resolve })));
  const other = { ...scope, profileId: "profile-b", revisionId: "revision-b" };
  await update(other);
  const newest = pending.at(-1);
  await act(async () => newest.resolve(inventory(newest.request)));
  assert.equal(api.controller.inventory.profileId, "profile-b");
  await act(async () => pending.slice(0, -1).forEach((call) => call.resolve(inventory(call.request))));
  assert.equal(api.controller.inventory.profileId, "profile-b");
  await update(null);
  assert.equal(api.controller.inventory, null);
});

test("saving invalidates an old preview immediately and duplicate writes are blocked", options, async (t) => {
  let complete;
  const { api } = await mount(t, {}, (command, request) => command === "get_mod_plugin_selection" ? inventory(request) : new Promise((resolve) => { complete = () => resolve({ ...inventory(request), confirmationRequired: false }); }));
  let first;
  await act(async () => { first = api.controller.choose("plugin-a", false); });
  assert.equal(api.controller.ready, false);
  assert.equal(api.invalidated, 1);
  await act(async () => { await api.controller.choose("plugin-a", false); });
  assert.equal(api.calls.filter((call) => call.command === "set_mod_plugin_selection").length, 1);
  await act(async () => { complete(); await first; });
  assert.equal(api.saved, 1);
});

test("stale selection failure blocks confirmation until the user checks again", options, async (t) => {
  const { api, root } = await mount(t, {}, (command, request) => {
    if (command === "set_mod_plugin_selection") throw { code: "plugin_inventory_changed" };
    return inventory(request);
  });
  await act(async () => { await assert.rejects(api.controller.choose("plugin-a", false)); });
  assert.equal(api.controller.ready, false);
  assert.ok(content(root.toJSON()).includes(pluginSelectionCopy.zh_cn.errors.plugin_inventory_changed));
  await assert.rejects(api.controller.confirm());
  await act(async () => api.controller.reload());
  assert.equal(api.controller.ready, true);
});

test("plugin facts use the same labels in all languages without additional scans", options, async (t) => {
  const { api, root, update } = await mount(t);
  const scans = api.calls.length;
  for (const locale of ["zh_cn", "en", "ja"]) {
    api.locale = locale;
    await update(scope);
    assert.ok(content(root.toJSON()).includes(pluginSelectionCopy[locale].checks.supported));
    assert.ok(content(root.toJSON()).includes(pluginSelectionCopy[locale].checks.policy_excluded));
  }
  assert.equal(api.calls.length, scans);
});
