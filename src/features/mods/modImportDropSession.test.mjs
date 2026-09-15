import assert from "node:assert/strict";
import { test } from "node:test";
import {
  activeDropBatches, beginDropPreview, cancelDropQueued, clearDropHistory, completeDropPreview,
  confirmDropDraft, discardDropDraft, dropBatchSummary, dropDraftChecking, emptyDropImportSession,
  failedDropBatchPaths, failDropPreview, finishedDropBatches, MAX_DROP_HISTORY_BATCHES,
  removeDropDraftItem, selectDropDraft, settleDropBatchItem, startDropBatchItem,
} from "./modImportDropSession.ts";

const previews = (paths) => paths.map((archivePath) => ({ archivePath, fileName: archivePath, sizeBytes: 8, errorCode: null, warningCode: null }));
const outcome = (status = "completed", taskId = "fixture-task") => status === "completed"
  ? { status, taskId, archiveKept: null }
  : { status, taskId, phase: "mod_import.unpack.failed", messageKind: "archive-encrypted" };
function draft(paths, state = emptyDropImportSession, requestId = "fixture-preview") {
  const started = beginDropPreview(state, paths, requestId);
  return completeDropPreview(started.session, started.request, previews(started.request.paths));
}
function submit(paths, state = emptyDropImportSession, id = "fixture-batch") {
  return confirmDropDraft(draft(paths, state, id + "-preview"), id, 1);
}
function finish(state, item, status = "completed") {
  return settleDropBatchItem(startDropBatchItem(state, item), item, outcome(status));
}

test("closing a draft invalidates successful and failed preview responses", () => {
  const a = beginDropPreview(emptyDropImportSession, ["a.zip"], "a");
  const closed = discardDropDraft(a.session);
  const b = beginDropPreview(closed, ["b.zip"], "b");
  assert.equal(completeDropPreview(b.session, a.request, previews(["a.zip"])), b.session);
  assert.equal(failDropPreview(b.session, a.request, "retry-hint"), b.session);
  assert.equal(dropDraftChecking(b.session.draft), 1);
});

test("concurrent previews reserve unique paths and preserve drop order and selection", () => {
  const a = beginDropPreview(emptyDropImportSession, ["a.zip", "a.zip"], "a");
  const b = beginDropPreview(a.session, ["a.zip", "b.zip"], "b");
  assert.deepEqual(b.request.paths, ["b.zip"]);
  assert.equal(b.session.draft.duplicateCount, 1);
  let state = completeDropPreview(b.session, b.request, previews(["b.zip"]));
  state = selectDropDraft(state, false, state.draft.items[1].id);
  assert.equal(confirmDropDraft(state, "batch", 1).queued.length, 0, "All previews must settle before confirmation");
  state = completeDropPreview(state, a.request, previews(["a.zip"]));
  assert.deepEqual(state.draft.items.map((item) => [item.row.archivePath, item.row.selected]), [["a.zip", true], ["b.zip", false]]);
});

test("removing a checking item cannot resurrect it or block submitting other files", () => {
  const pending = beginDropPreview(draft(["ready.zip"]), ["slow.zip"], "slow");
  const removed = removeDropDraftItem(pending.session, pending.session.draft.items[1].id);
  assert.equal(dropDraftChecking(removed.draft), 0);
  assert.equal(completeDropPreview(removed, pending.request, previews(["slow.zip"])), removed);
  assert.deepEqual(confirmDropDraft(removed, "batch", 1).queued.map((item) => item.archivePath), ["ready.zip"]);
});

test("preview failure blocks only its items and remains removable", () => {
  const pending = beginDropPreview(draft(["ready.zip"]), ["bad.zip"], "bad");
  const failed = failDropPreview(pending.session, pending.request, "preview-limit");
  assert.equal(failed.draft.items[0].row.selected, true);
  assert.equal(failed.draft.items[1].row.status, "blocked");
  assert.equal(failed.draft.items[1].row.messageKind, "preview-limit");
  assert.equal(dropDraftChecking(failed.draft), 0);
  assert.equal(discardDropDraft(failed).draft, null);
});

test("missing preview entries cannot become importable and unsolicited paths are ignored", () => {
  const pending = beginDropPreview(emptyDropImportSession, ["a.zip"], "preview");
  const completed = completeDropPreview(pending.session, pending.request, previews(["unexpected.zip"]));
  assert.equal(completed.draft.items.length, 1);
  assert.equal(completed.draft.items[0].row.archivePath, "a.zip");
  assert.equal(completed.draft.items[0].row.status, "blocked");
  assert.equal(confirmDropDraft(completed, "batch", 1).queued.length, 0);
});

test("confirmation consumes the draft once and marks unchecked rows skipped", () => {
  let state = draft(["yes.zip", "no.zip"]);
  state = selectDropDraft(state, false, state.draft.items[1].id);
  const submitted = confirmDropDraft(state, "batch", 1);
  assert.equal(submitted.session.draft, null);
  assert.deepEqual(submitted.queued.map((item) => item.archivePath), ["yes.zip"]);
  assert.deepEqual(submitted.session.batches[0].items.map((item) => item.row.phase), ["queued", "skipped"]);
  assert.equal(dropBatchSummary(submitted.session.batches[0]).submitted, 1);
  assert.equal(confirmDropDraft(submitted.session, "again", 2).queued.length, 0);
});

test("new drafts and history cannot alter an older batch's denominator", () => {
  const first = submit(["a.zip", "b.zip"]);
  const running = startDropBatchItem(first.session, first.queued[0]);
  const next = draft(["c.zip"], running, "new");
  assert.equal(next.draft.items.length, 1);
  assert.equal(dropBatchSummary(next.batches[0]).submitted, 2);
  const second = confirmDropDraft(next, "second", 2);
  assert.equal(second.session.batches.length, 2);
  assert.equal(dropBatchSummary(second.session.batches[0]).submitted, 2);
  assert.equal(dropBatchSummary(second.session.batches[1]).submitted, 1);
});

test("active paths cannot be requeued, while a finished path gets a new attempt", () => {
  const submitted = submit(["a.zip"]);
  const duplicate = beginDropPreview(submitted.session, ["a.zip"], "duplicate");
  assert.equal(duplicate.request, null);
  assert.equal(duplicate.session.draft.duplicateCount, 1);
  const done = finish(submitted.session, submitted.queued[0]);
  const again = submit(["a.zip"], done, "second");
  assert.equal(again.session.batches[0].items[0].row.phase, "succeeded");
  assert.equal(again.session.batches[1].items[0].row.phase, "queued");
  const late = settleDropBatchItem(again.session, submitted.queued[0], outcome("failed"));
  assert.equal(late.batches[0].items[0].row.phase, "succeeded");
  assert.equal(late.batches[1].items[0].row.phase, "queued");
});

test("same filenames from different paths are separate selections", () => {
  const state = draft(["one/mod.zip", "two/mod.zip"]);
  assert.equal(confirmDropDraft(state, "batch", 1).queued.length, 2);
});

test("cancel queue leaves its running item and draft intact without making pending history", () => {
  const submitted = submit(["a.zip", "b.zip"]);
  const withDraft = draft(["c.zip"], startDropBatchItem(submitted.session, submitted.queued[0]), "c");
  const cancelled = cancelDropQueued(withDraft);
  assert.equal(cancelled.draft, withDraft.draft);
  assert.deepEqual(cancelled.batches[0].items.map((item) => item.row.phase), ["running", "cancelled"]);
  const done = settleDropBatchItem(cancelled, submitted.queued[0], outcome());
  const summary = dropBatchSummary(done.batches[0]);
  assert.equal(summary.succeeded, 1);
  assert.equal(summary.cancelled, 1);
  assert.equal(summary.failed, 0);
  assert.equal(activeDropBatches(done).length, 0);
});

test("backend cancellation is distinct from failure and success", () => {
  const submitted = submit(["a.zip"]);
  const done = finish(submitted.session, submitted.queued[0], "cancelled");
  assert.equal(dropBatchSummary(done.batches[0]).cancelled, 1);
  assert.equal(dropBatchSummary(done.batches[0]).failed, 0);
  assert.equal(failedDropBatchPaths(done, done.batches[0].id).length, 0);
});

test("history retry selects only failed paths without changing old outcomes", () => {
  const submitted = submit(["a.zip", "b.zip"]);
  const partial = finish(finish(submitted.session, submitted.queued[0]), submitted.queued[1], "failed");
  const paths = failedDropBatchPaths(partial, partial.batches[0].id);
  assert.deepEqual(paths, ["b.zip"]);
  const retry = draft(paths, partial, "retry");
  assert.equal(retry.batches[0], partial.batches[0]);
  assert.equal(retry.draft.items[0].row.phase, "pending");
});

test("clearing history never removes active tasks or a draft", () => {
  const first = submit(["a.zip"]);
  const done = finish(first.session, first.queued[0]);
  const next = submit(["b.zip"], done, "second");
  const current = draft(["c.zip"], next.session, "current");
  const cleared = clearDropHistory(current);
  assert.equal(cleared.draft, current.draft);
  assert.deepEqual(cleared.batches.map((batch) => batch.id), ["second"]);
  assert.equal(cleared.nextBatchNumber, current.nextBatchNumber);
});

test("bounded session history preserves all active batches and increasing numbers", () => {
  let state = submit(["keep.zip"], emptyDropImportSession, "keep").session;
  for (let i = 0; i < MAX_DROP_HISTORY_BATCHES + 2; i += 1) {
    const next = submit(["completed-" + i + ".zip"], state, "batch-" + i);
    state = finish(next.session, next.queued[0]);
  }
  assert.equal(activeDropBatches(state)[0].id, "keep");
  assert.equal(finishedDropBatches(state).length, MAX_DROP_HISTORY_BATCHES);
  assert.equal(state.batches[1].id, "batch-2");
  assert.equal(state.nextBatchNumber, MAX_DROP_HISTORY_BATCHES + 4);
});
