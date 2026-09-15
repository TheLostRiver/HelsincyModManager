import assert from "node:assert/strict";
import { test } from "node:test";
import { act, mountDrop, options, loadFeature } from "./testing/reactHarness.mjs";

const { getDropRowNote } = await loadFeature("modImportDropState.ts");
const { dropBatchSummary } = await loadFeature("modImportDropSession.ts");
const noticeId = "mod-import.drop.batch";
const draftPaths = (api) => api.overlay.session.draft?.items.map((item) => item.archivePath) ?? [];
const batchRows = (api, index = 0) => api.overlay.session.batches[index].items.map((item) => item.row);
const finishedToasts = (api) => api.toasts.filter((toast) => toast.eventKey.startsWith("mod-import.drop.finished."));
const previews = (paths) => paths.map((archivePath) => ({ archivePath, fileName: archivePath, sizeBytes: 10, errorCode: null, warningCode: null }));

test("unavailable WebView disables native drop without crashing the app or routing", options, async (t) => {
  const { api, changeRoute } = await mountDrop(t, { webviewUnavailable: true });
  assert.equal(api.drop.size, 0);
  assert.equal(api.starts.length, 0);
  await changeRoute("about");
  assert.equal(api.dropContext.activeImportCount, 0);
});

test("StrictMode and two confirmations in one event start exactly one import", options, async (t) => {
  const { api } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => { api.overlay.onConfirm(); api.overlay.onConfirm(); });
  assert.equal(api.starts.length, 1);
  await act(async () => api.emit(api.starts[0].taskId));
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts.length, 1);
  assert.equal(batchRows(api)[0].phase, "succeeded");
});

test("closing an unsubmitted draft starts nothing and leaves no notice or history", options, async (t) => {
  const { api } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => api.overlay.onClose());
  assert.equal(api.overlay.visible, false);
  assert.equal(api.overlay.session.draft, null);
  assert.equal(api.notices.size, 0);
  assert.equal(api.overlay.session.batches.length, 0);
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts.length, 0);
  await act(async () => api.drag(["fixture-b.zip"]));
  assert.deepEqual(draftPaths(api), ["fixture-b.zip"]);
});

test("a fully blocked draft can be discarded without importing it", options, async (t) => {
  const { api } = await mountDrop(t);
  api.preview = async (paths) => previews(paths).map((row) => ({ ...row, errorCode: "mod_import_archive_encrypted" }));
  await act(async () => api.drag(["fixture-bad.zip"]));
  await act(async () => api.overlay.onSelectAll(true));
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts.length, 0);
  await act(async () => api.overlay.onClose());
  assert.equal(api.overlay.session.draft, null);
  assert.equal(api.notices.size, 0);
});

test("new batches append after confirmed work and survive route changes", options, async (t) => {
  const { api, changeRoute } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip", "fixture-b.zip"]));
  await act(async () => api.overlay.onConfirm());
  await changeRoute("settings");
  await act(async () => api.drag(["fixture-c.zip"]));
  assert.equal(api.starts.length, 1, "Dropping must not start an import");
  assert.deepEqual(draftPaths(api), ["fixture-c.zip"]);
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.overlay.session.batches.length, 2);
  assert.equal(dropBatchSummary(api.overlay.session.batches[0]).submitted, 2);
  assert.equal(dropBatchSummary(api.overlay.session.batches[1]).submitted, 1);
  for (let i = 0; i < 3; i += 1) await act(async () => api.emit(api.starts[i].taskId));
  assert.deepEqual(api.starts.map((task) => task.archivePath), ["fixture-a.zip", "fixture-b.zip", "fixture-c.zip"]);
  assert.equal(api.dropContext.activeImportCount, 0);
});

test("cancel queue preserves the running file and records queued files as cancelled", options, async (t) => {
  const { api } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip", "fixture-b.zip"]));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.drag(["fixture-c.zip"]));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.overlay.onCancelQueued());
  assert.deepEqual(batchRows(api).map((row) => row.phase), ["running", "cancelled"]);
  assert.equal(batchRows(api, 1)[0].phase, "cancelled");
  assert.equal(api.overlay.session.draft, null);
  await act(async () => api.emit(api.starts[0].taskId));
  assert.equal(api.starts.length, 1);
  assert.equal(api.dropContext.activeImportCount, 0);
  await act(async () => api.overlay.onClose());
  assert.equal(api.notices.size, 0);
});

test("discarding additions leaves the previously confirmed queue running", options, async (t) => {
  const { api } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip", "fixture-b.zip"]));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.drag(["fixture-c.zip"]));
  await act(async () => api.overlay.onClose());
  assert.equal(api.overlay.session.draft, null);
  assert.equal(api.notices.has(noticeId), true);
  await act(async () => api.emit(api.starts[0].taskId));
  assert.equal(api.starts[1].archivePath, "fixture-b.zip");
  await act(async () => api.emit(api.starts[1].taskId));
  assert.equal(api.starts.length, 2);
  assert.equal(api.notices.size, 0);
});

test("early terminal events and repeated drops do not duplicate imports", options, async (t) => {
  const { api } = await mountDrop(t);
  const start = api.startImport;
  api.startImport = async (input) => { const task = await start(input); api.emit(task.taskId); return task; };
  await act(async () => api.drag(["fixture-fast.zip", "fixture-fast.zip"]));
  await act(async () => { api.overlay.onConfirm(); api.overlay.onConfirm(); });
  assert.equal(api.starts.length, 1);
  assert.equal(batchRows(api)[0].phase, "succeeded");
});

test("terminal failure removes the progress notice but stays reachable through history", options, async (t) => {
  const { api } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.overlay.onClose());
  await act(async () => api.emit(api.starts[0].taskId, "failed", "mod_import_archive_encrypted"));
  assert.equal(api.notices.size, 0);
  const toast = finishedToasts(api)[0];
  assert.equal(toast.tone, "danger");
  assert.equal(toast.durationMs, 5000);
  await act(async () => toast.action.onSelect());
  assert.equal(api.overlay.visible, true);
  assert.equal(api.overlay.tab, "history");
  assert.equal(getDropRowNote(batchRows(api)[0], api.overlay.copy), api.overlay.copy.errors.archiveEncrypted);
  await act(async () => api.overlay.onClearHistory());
  assert.equal(api.overlay.session.batches.length, 0);
});

test("move import retains its archive warning and independent successful outcome", options, async (t) => {
  const { api } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.emit(api.starts[0].taskId, "completed", "mod_import_archive_kept_remove_failed"));
  assert.equal(batchRows(api)[0].phase, "succeeded");
  assert.equal(getDropRowNote(batchRows(api)[0], api.overlay.copy), api.overlay.copy.archiveKept.mod_import_archive_kept_remove_failed);
  assert.ok(api.toasts.some((toast) => toast.tone === "warning" && toast.eventKey.startsWith("mod-import.archive-kept.")));
});

test("start rejection keeps its stable failure reason and continues the queue", options, async (t) => {
  const { api } = await mountDrop(t);
  const start = api.startImport;
  api.startImport = (input) => input.archivePath === "fixture-a.zip"
    ? Promise.reject({ code: "mod_storage_restart_required", message: "untrusted details" }) : start(input);
  await act(async () => api.drag(["fixture-a.zip", "fixture-b.zip"]));
  await act(async () => api.overlay.onConfirm());
  assert.equal(getDropRowNote(batchRows(api)[0], api.overlay.copy), api.overlay.copy.errors.storageFrozenRestart);
  assert.equal(api.starts.length, 1);
  await act(async () => api.emit(api.starts[0].taskId));
  assert.deepEqual(batchRows(api).map((row) => row.phase), ["failed", "succeeded"]);
});

test("frozen storage prevents confirming or adding to an already open draft", options, async (t) => {
  const { api, changeRoute } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip"]));
  api.frozen = "restart";
  await changeRoute("settings");
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.drag(["fixture-b.zip"]));
  assert.equal(api.starts.length, 0);
  assert.deepEqual(draftPaths(api), ["fixture-a.zip"]);
  assert.equal(api.toasts.at(-1).message, "storage-frozen");
});

test("missing progress subscription prevents confirmation but permits closing the draft", options, async (t) => {
  const { api } = await mountDrop(t, { listenerFails: true });
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts.length, 0);
  await act(async () => api.overlay.onClose());
  assert.equal(api.notices.size, 0);
});

test("late preview success after close cannot repopulate the list or notification", options, async (t) => {
  const { api } = await mountDrop(t);
  let resolve;
  api.preview = () => new Promise((done) => { resolve = done; });
  await act(async () => api.drag(["fixture-slow.zip"]));
  await act(async () => api.overlay.onClose());
  await act(async () => resolve(previews(["fixture-slow.zip"])));
  assert.equal(api.overlay.session.draft, null);
  assert.equal(api.overlay.visible, false);
  assert.equal(api.notices.size, 0);
  assert.equal(api.starts.length, 0);
});

test("late preview rejection after close produces no stale error toast", options, async (t) => {
  const { api } = await mountDrop(t);
  let reject;
  api.preview = () => new Promise((_, fail) => { reject = fail; });
  await act(async () => api.drag(["fixture-slow.zip"]));
  await act(async () => api.overlay.onClose());
  await act(async () => reject({ code: "mod_import_prepare_failed" }));
  assert.equal(api.overlay.session.draft, null);
  assert.equal(api.notices.size, 0);
  assert.equal(api.toasts.length, 0);
});

test("an old preview cannot settle a new draft even with the same path", options, async (t) => {
  const { api } = await mountDrop(t);
  const requests = [];
  api.preview = (paths) => new Promise((resolve) => requests.push({ paths, resolve }));
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => api.overlay.onClose());
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => requests[0].resolve(previews(requests[0].paths)));
  assert.equal(api.overlay.session.draft.items[0].row, null);
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts.length, 0);
  await act(async () => requests[1].resolve(previews(requests[1].paths)));
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts.length, 1);
});

test("overlapping drops preflight each path once and preserve selection despite response order", options, async (t) => {
  const { api } = await mountDrop(t);
  const requests = [];
  api.preview = (paths) => new Promise((resolve) => requests.push({ paths, resolve }));
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => api.drag(["fixture-a.zip", "fixture-b.zip"]));
  assert.deepEqual(requests.map((request) => request.paths), [["fixture-a.zip"], ["fixture-b.zip"]]);
  await act(async () => requests[1].resolve(previews(requests[1].paths)));
  const b = api.overlay.session.draft.items[1];
  await act(async () => api.overlay.onSelectItem(b.id, false));
  await act(async () => requests[0].resolve(previews(requests[0].paths)));
  assert.deepEqual(draftPaths(api), ["fixture-a.zip", "fixture-b.zip"]);
  assert.equal(api.overlay.session.draft.items[1].row.selected, false);
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts.length, 1);
  assert.deepEqual(batchRows(api).map((row) => row.phase), ["running", "skipped"]);
});

test("removing a checking item allows ready items to start and rejects the late row", options, async (t) => {
  const { api } = await mountDrop(t);
  await act(async () => api.drag(["fixture-ready.zip"]));
  let resolve;
  api.preview = () => new Promise((done) => { resolve = done; });
  await act(async () => api.drag(["fixture-slow.zip"]));
  await act(async () => api.overlay.onRemoveItem(api.overlay.session.draft.items[1].id));
  await act(async () => api.overlay.onConfirm());
  await act(async () => resolve(previews(["fixture-slow.zip"])));
  assert.deepEqual(api.starts.map((task) => task.archivePath), ["fixture-ready.zip"]);
  assert.equal(api.overlay.session.draft, null);
  assert.equal(batchRows(api).length, 1);
});

test("dismissing progress stays hidden through updates and locale changes, with a fixed reopen entry", options, async (t) => {
  const { api, changeRoute } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip", "fixture-b.zip"]));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.overlay.onClose());
  await act(async () => api.notices.get(noticeId).dismiss.onClick());
  assert.equal(api.notices.size, 0);
  api.locale = "ja";
  await changeRoute("settings");
  await act(async () => api.emit(api.starts[0].taskId));
  assert.equal(api.starts.length, 2);
  assert.equal(api.notices.size, 0);
  await act(async () => api.dropContext.openDropList());
  assert.equal(api.overlay.visible, true);
  assert.equal(api.overlay.tab, "active");
  await act(async () => api.overlay.onClose());
  assert.equal(api.notices.get(noticeId).action.label, api.overlay.copy.drop.reopenList);
  await act(async () => api.emit(api.starts[1].taskId));
  assert.equal(api.notices.size, 0);
});

test("finished history and unchecked files never join the next draft", options, async (t) => {
  const { api } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip", "fixture-skipped.zip"]));
  await act(async () => api.overlay.onSelectItem(api.overlay.session.draft.items[1].id, false));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.emit(api.starts[0].taskId));
  await act(async () => api.overlay.onClose());
  await act(async () => api.drag(["fixture-new.zip"]));
  assert.deepEqual(draftPaths(api), ["fixture-new.zip"]);
  assert.deepEqual(batchRows(api).map((row) => row.phase), ["succeeded", "skipped"]);
  await act(async () => api.overlay.onSelectAll(true));
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts[1].archivePath, "fixture-new.zip");
  assert.equal(api.starts.length, 2);
});

test("retry preflights only failed files and keeps the original batch result", options, async (t) => {
  const { api } = await mountDrop(t);
  await act(async () => api.drag(["fixture-ok.zip", "fixture-bad.zip"]));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.emit(api.starts[0].taskId));
  await act(async () => api.emit(api.starts[1].taskId, "failed", "mod_import_prepare_failed"));
  const originalId = api.overlay.session.batches[0].id;
  const calls = [];
  api.preview = async (paths) => { calls.push(paths); return previews(paths); };
  await act(async () => api.overlay.onRetryBatch(originalId));
  assert.deepEqual(calls, [["fixture-bad.zip"]]);
  assert.equal(api.starts.length, 2, "Retry still needs explicit confirmation");
  assert.equal(api.overlay.tab, "pending");
  assert.equal(batchRows(api)[1].phase, "failed");
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts[2].archivePath, "fixture-bad.zip");
  assert.equal(api.overlay.session.batches.length, 2);
  assert.equal(batchRows(api)[1].phase, "failed");
});

test("retrying a batch built from multiple drops respects each preview request limit", options, async (t) => {
  const { api } = await mountDrop(t);
  const paths = Array.from({ length: 101 }, (_, index) => `fixture-${index}.zip`);
  const start = api.startImport;
  api.startImport = async (input) => {
    const task = await start(input);
    api.emit(task.taskId, "failed", "mod_import_prepare_failed");
    return task;
  };
  await act(async () => api.drag(paths.slice(0, 100)));
  await act(async () => api.drag(paths.slice(100)));
  await act(async () => api.overlay.onConfirm());
  assert.equal(dropBatchSummary(api.overlay.session.batches[0]).failed, 101);
  const originalId = api.overlay.session.batches[0].id;
  const requests = [];
  api.preview = (requestPaths) => new Promise((resolve) => requests.push({ paths: requestPaths, resolve }));
  await act(async () => api.overlay.onRetryBatch(originalId));
  assert.deepEqual(requests.map((request) => request.paths.length), [100, 1]);
  assert.deepEqual(draftPaths(api), paths);
  assert.equal(api.overlay.session.draft.addedCount, 101);
  await act(async () => requests[1].resolve(previews(requests[1].paths)));
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts.length, 101, "A partially checked retry still needs all preview results");
  await act(async () => api.overlay.onClose());
  await act(async () => requests[0].resolve(previews(requests[0].paths)));
  assert.equal(api.overlay.session.draft, null, "Late retry chunks cannot recreate a closed draft");
  assert.equal(api.overlay.visible, false);
  assert.equal(api.notices.size, 0);
  assert.equal(dropBatchSummary(api.overlay.session.batches[0]).failed, 101);
  await act(async () => api.overlay.onRetryBatch(originalId));
  await act(async () => requests[3].resolve(previews(requests[3].paths)));
  await act(async () => requests[2].resolve(previews(requests[2].paths)));
  assert.equal(api.starts.length, 101, "A complete retry still requires confirmation");
  await act(async () => api.overlay.onConfirm());
  assert.equal(api.starts.length, 202);
  assert.equal(api.overlay.session.batches.length, 2);
  assert.equal(dropBatchSummary(api.overlay.session.batches[0]).failed, 101);
  assert.equal(dropBatchSummary(api.overlay.session.batches[1]).failed, 101);
});

test("one oversized native drop is rejected without starting preview or import", options, async (t) => {
  const { api } = await mountDrop(t);
  const calls = [];
  api.preview = async (paths) => { calls.push(paths); return previews(paths); };
  await act(async () => api.drag(Array.from({ length: 101 }, (_, index) => `fixture-${index}.zip`)));
  assert.equal(calls.length, 0);
  assert.equal(api.starts.length, 0);
  assert.equal(api.overlay.session.draft, null);
  assert.equal(api.toasts.at(-1).eventKey, "mod-import.drop.too-many");
});

test("completed batches issue one short summary despite rerenders and language changes", options, async (t) => {
  const { api, changeRoute } = await mountDrop(t);
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.emit(api.starts[0].taskId));
  assert.equal(finishedToasts(api).length, 1);
  assert.equal(finishedToasts(api)[0].durationMs, 3000);
  api.locale = "ja";
  await changeRoute("settings");
  await act(async () => api.dropContext.openDropList());
  assert.equal(api.overlay.tab, "history");
  assert.equal(finishedToasts(api).length, 1);
});

test("the queue invalidates library queries even when the cache listener failed", options, async (t) => {
  const { api } = await mountDrop(t, { cacheListenerFails: true });
  const generation = api.cache.getGeneration();
  await act(async () => api.drag(["fixture-a.zip"]));
  await act(async () => api.overlay.onConfirm());
  await act(async () => api.emit(api.starts[0].taskId));
  assert.ok(api.cache.getGeneration() > generation);
  assert.equal(batchRows(api)[0].phase, "succeeded");
});
