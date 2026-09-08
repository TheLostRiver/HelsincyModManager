import assert from "node:assert/strict";
import { test } from "node:test";
import { act, mountDrop, options, loadFeature } from "./testing/reactHarness.mjs";

const { getDropRowNote } = await loadFeature("modImportDropState.ts");

test("unavailable WebView disables native drop without crashing the app or routing", options, async (t) => {
  const { api, changeRoute } = await mountDrop(t, { webviewUnavailable: true });
  assert.equal(api.drop.size, 0);
  assert.equal(api.starts.length, 0);
  await changeRoute("about");
  assert.equal(api.overlay.summary.active, false);
});

test("StrictMode one confirmation starts exactly one import", options, async (t) => {
  const { api } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts.length, 1);
  await act(async () => api.emit(api.starts[0].taskId));
  assert.equal(api.starts.length, 1, "Updater replay must not enqueue a second copy");
  assert.equal(api.overlay.summary.succeeded, 1);
});

test("a stopped queue preserves its running item and can restart", options, async (t) => {
  const { api } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip", "fixture-b.zip"]));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.overlay.onCancelQueued());
  assert.deepEqual(api.overlay.list.rows.map((row) => [row.phase, row.selected]), [["running", false], ["pending", true]]);
  await act(async () => api.emit(api.starts[0].taskId));
  assert.equal(api.starts.length, 1);
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.emit(api.starts[1].taskId));
  assert.equal(api.overlay.summary.succeeded, 2);
});

test("route replacement and appending preserve serial queue execution", options, async (t) => {
  const { api, changeRoute } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => api.overlay.onConfirm());
  await changeRoute("settings");
  await act(async () => api.drag(["fixture-b.zip"]));
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts.length, 1);
  await act(async () => api.emit(api.starts[0].taskId));
  assert.equal(api.starts.length, 2);
  await act(async () => api.emit(api.starts[1].taskId));
  assert.equal(api.overlay.summary.succeeded, 2);
});

test("early terminal events and repeated drops do not duplicate imports", options, async (t) => {
  const { api } = await mountDrop(t);
  const start = api.startImport;
  api.startImport = async (input) => { const task = await start(input); api.emit(task.taskId); return task; };
  await act(async () => api.drag(["fixture-fast.zip", "fixture-fast.zip"]));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts.length, 1);
  assert.equal(api.overlay.summary.succeeded, 1);
});

test("hidden failed batches retain their result and reopen action", options, async (t) => {
  const { api } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.overlay.onClose());
  await act(async () => api.emit(api.starts[0].taskId, "failed", "mod_import_archive_encrypted"));
  const notice = api.notices.get("mod-import.drop.batch");
  assert.ok(notice?.action?.onClick, "A terminal failure must not remove the only result entry");
  assert.equal(notice.tone, "danger");
  assert.equal(notice.message, api.overlay.copy.drop.doneAllFailed(1));
  await act(async () => notice.action.onClick());
  assert.equal(api.overlay.visible, true);
  assert.equal(getDropRowNote(api.overlay.list.rows[0], api.overlay.copy), api.overlay.copy.errors.archiveEncrypted);
  await act(async () => api.overlay.onClearFinished());
  await act(async () => api.overlay.onClose());
  assert.equal(api.notices.size, 0);
});

test("move import reports retained archives without changing success", options, async (t) => {
  const { api } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.emit(api.starts[0].taskId, "completed", "mod_import_archive_kept_remove_failed"));
  assert.equal(api.overlay.summary.succeeded, 1);
  assert.equal(getDropRowNote(api.overlay.list.rows[0], api.overlay.copy), api.overlay.copy.archiveKept.mod_import_archive_kept_remove_failed);
  assert.equal(api.toasts.at(-1).tone, "warning");
});

test("start rejection keeps its stable failure reason and continues the queue", options, async (t) => {
  const { api } = await mountDrop(t);
  const start = api.startImport;
  api.startImport = (input) => input.archivePath === "fixture-a.zip"
    ? Promise.reject({ code: "mod_storage_restart_required", message: "untrusted details" }) : start(input);
  await act(async () => api.drag(["fixture-a.zip", "fixture-b.zip"]));
  await act(async () => api.overlay.onConfirm());
  assert.equal(getDropRowNote(api.overlay.list.rows[0], api.overlay.copy), api.overlay.copy.errors.storageFrozenRestart);
  assert.equal(api.starts.length, 1);
  await act(async () => api.emit(api.starts[0].taskId));
  assert.equal(api.overlay.summary.failed, 1);
  assert.equal(api.overlay.summary.succeeded, 1);
});

test("a frozen storage root prevents confirming an already opened list", options, async (t) => {
  const { api, changeRoute } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip"]));
  api.frozen = "restart";
  await changeRoute("settings");
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts.length, 0);
  assert.equal(api.toasts.at(-1).message, "storage-frozen");
});

test("a hidden pending list remains reachable and follows locale changes", options, async (t) => {
  const { api, changeRoute } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => api.overlay.onClose());
  api.locale = "ja";
  await changeRoute("settings");
  assert.equal(api.notices.get("mod-import.drop.batch").action.label, api.overlay.copy.drop.reopenList);
});

test("the queue invalidates library queries even when the cache listener failed", options, async (t) => {
  const { api } = await mountDrop(t, { cacheListenerFails: true });
  const generation = api.cache.getGeneration();
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.emit(api.starts[0].taskId));
  assert.ok(api.cache.getGeneration() > generation);
  assert.equal(api.overlay.summary.succeeded, 1);
});
