import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "features/profiles/ActiveProfileProvider.tsx": "export const useActiveProfile = () => globalThis.__recovery340.profile;",
  "features/mods/modInstallPlanApi.ts": `
    export const previewRecoveryAction = (input) => globalThis.__recovery340.request("previews", input);
    export const startRecoveryActionTask = (input) => globalThis.__recovery340.request("starts", input);
    export const scanInstallRecovery = (input) => globalThis.__recovery340.request("scans", input);`,
  "features/mods/modLibraryApi.ts": "export const getModDetail = (input) => globalThis.__recovery340.request(\"names\", input);",
  "features/install-recovery/installRecoveryRefresh.ts": "export const notifyInstallRecoveryRefresh = () => { globalThis.__recovery340.refreshes++; };",
}, { "@tauri-apps/api/event": "export const listen = (_name, callback) => globalThis.__recovery340.listen(callback);" });
const { useRecoveryRollback } = await import("./useRecoveryRollback.ts");
const { useRecoveryCenterScan } = await import("./useRecoveryCenterScan.ts");
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const options = { concurrency: false, timeout: 5000 };
const preview = (overrides = {}) => ({ profileId: "one", modId: "mod-a", actionKind: "uninstall_missing_targets", availability: "available",
  removeFileCount: 1, restoreFileCount: 0, backupCount: 0, missingFileCount: 1, blockingIssueCount: 0, blockingReasons: [], planToken: "fixture-preview-token", ...overrides });

async function mount(t, { scan = false, listenerFails = false } = {}) {
  let current, root, version = 0, completions = 0;
  const api = { previews: [], starts: [], scans: [], names: [], listeners: new Set(), refreshes: 0,
    profile: { activeProfile: { status: "ready" }, activeProfileId: "one" } };
  api.request = (kind, input) => new Promise((resolve, reject) => api[kind].push({ input, resolve, reject }));
  api.listen = async (callback) => { if (listenerFails) throw new Error("fixture listener failure"); api.listeners.add(callback); return () => api.listeners.delete(callback); };
  globalThis.__recovery340 = api;
  function ActionCapture() { current = useRecoveryRollback({ gameId: "mhw", onCompleted: () => completions++ }); return null; }
  function ScanCapture() { current = useRecoveryCenterScan({ gameId: "mhw", enabled: true }); return null; }
  const tree = () => React.createElement(React.StrictMode, null, React.createElement(scan ? ScanCapture : ActionCapture, { version }));
  await act(async () => { root = TestRenderer.create(tree()); });
  t.after(async () => { await act(async () => root.unmount()); delete globalThis.__recovery340; });
  return { api, get current() { return current; }, get completions() { return completions; },
    request: async (kind = "uninstall_missing_targets") => { await act(async () => current.requestRollback("mod-a", kind)); },
    resolve: async (request, value) => { await act(async () => request.resolve(value)); },
    profile: async (id) => { api.profile = { ...api.profile, activeProfileId: id }; version++; await act(async () => root.update(tree())); },
    emit: async (taskId, phase = "install.recovery.completed", error = null) => { await act(async () => { for (const callback of api.listeners) callback({ payload: { taskId, kind: "install", phase, error, message: null } }); }); },
  };
}

test("missing target recovery requires confirmation and forwards the exact preview token once", options, async (t) => {
  const h = await mount(t);
  await h.request();
  assert.equal(h.api.previews[0].input.actionKind, "uninstall_missing_targets");
  assert.equal(h.api.starts.length, 0);
  await h.resolve(h.api.previews[0], preview());
  assert.equal(h.current.state.status, "confirming");
  await act(async () => { h.current.confirmRollback(); h.current.confirmRollback(); });
  assert.equal(h.api.starts.length, 1);
  assert.deepEqual(h.api.starts[0].input, { gameId: "mhw", profileId: "one", modId: "mod-a", actionKind: "uninstall_missing_targets", planToken: "fixture-preview-token" });
  await h.emit("unrelated-task");
  await h.emit("task-a");
  await h.resolve(h.api.starts[0], { kind: "install", status: "queued", taskId: "task-a" });
  await h.emit("task-a");
  assert.equal(h.current.state.status, "completed");
  assert.equal(h.completions, 1);
  assert.equal(h.api.refreshes, 1);
});

for (const [name, changed] of [["missing token", { planToken: undefined }], ["wrong profile", { profileId: "two" }],
  ["wrong mod", { modId: "mod-b" }], ["wrong action", { actionKind: "rollback_install" }], ["no missing files", { missingFileCount: 0 }]]) {
  test(`recovery rejects an available preview with ${name}`, options, async (t) => {
    const h = await mount(t);
    await h.request();
    await h.resolve(h.api.previews[0], preview(changed));
    assert.equal(h.current.state.status, "failed");
    await act(async () => h.current.confirmRollback());
    assert.equal(h.api.starts.length, 0);
  });
}

test("blocked recovery previews never launch an action", options, async (t) => {
  const h = await mount(t);
  await h.request();
  await h.resolve(h.api.previews[0], preview({ availability: "blocked", planToken: undefined, blockingIssueCount: 1, blockingReasons: [{ reason: "recovery_pending", count: 1 }] }));
  assert.equal(h.current.state.status, "blocked");
  await act(async () => h.current.confirmRollback());
  assert.equal(h.api.starts.length, 0);
});

test("switching profiles invalidates old previews and confirmation", options, async (t) => {
  const h = await mount(t);
  await h.request();
  await h.profile("two");
  await h.request();
  await h.resolve(h.api.previews[0], preview());
  assert.equal(h.current.state.status, "previewing");
  await h.resolve(h.api.previews[1], preview({ profileId: "two" }));
  await h.profile("three");
  await act(async () => h.current.confirmRollback());
  assert.equal(h.api.starts.length, 0);
});

test("legacy rollback remains available without a missing-target token", options, async (t) => {
  const h = await mount(t);
  await h.request("rollback_install");
  await h.resolve(h.api.previews[0], preview({ actionKind: "rollback_install", planToken: undefined, missingFileCount: undefined }));
  await act(async () => h.current.confirmRollback());
  assert.equal(h.api.starts[0].input.actionKind, "rollback_install");
  assert.equal("planToken" in h.api.starts[0].input, false);
});

test("listener failure blocks recovery before any task can be started", options, async (t) => {
  const h = await mount(t, { listenerFails: true });
  await h.request();
  assert.equal(h.current.actionKind, "uninstall_missing_targets");
  assert.equal(h.current.state.reason, "listener_unavailable");
  assert.equal(h.api.previews.length, 0);
  assert.equal(h.api.starts.length, 0);
});

test("recovery scans show facts first and ignore previous-profile names", options, async (t) => {
  const h = await mount(t, { scan: true });
  assert.equal(h.api.scans.length, 1, "StrictMode must not duplicate the filesystem scan");
  await h.resolve(h.api.scans[0], [{ modId: "mod-a", status: "repair_required" }]);
  assert.equal(h.current.state.status, "ready");
  assert.equal(h.api.names.length, 1);
  await h.profile("two");
  await h.resolve(h.api.scans[1], [{ modId: "mod-b", status: "repair_required" }]);
  await h.resolve(h.api.names[1], { id: "mod-b", name: "Current Mod" });
  await h.resolve(h.api.names[0], { id: "mod-a", name: "Old profile Mod" });
  assert.deepEqual(h.current.modNames, { "mod-b": "Current Mod" });
});
