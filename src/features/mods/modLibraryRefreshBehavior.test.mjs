import assert from "node:assert/strict";
import { test } from "node:test";
import { setTimeout as delay } from "node:timers/promises";
import { act, loadFeature, mountQuery, pageOf, options } from "./testing/reactHarness.mjs";

const { trackModLibraryTaskStart, trackModLibraryBatchWrite } = await loadFeature("modLibraryWriteTracking.ts");
const { loadModLibraryPageWithStatuses } = await loadFeature("modLibraryPageLoader.ts");
const target = { gameId: "mhw", profileId: "test-profile", modId: "off-page" };
const task = (id, status = "queued") => ({ taskId: id, kind: "install", status });

function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

for (const operation of ["install", "uninstall"]) {
  test(`${operation} keeps cards and converges delayed events and callbacks on one page/target scan`, options, async (t) => {
    const calls = [];
    const scanReplies = [];
    const h = await mountQuery(t, { loadPage: (input, context, cache) => loadModLibraryPageWithStatuses(input, context, {
      cache, query: async () => { calls.push("query"); return pageOf("visible"); },
      scan: ({ modIds }) => { calls.push(modIds); const result = deferred(); scanReplies.push({ ...result, modIds }); return result.promise; },
    }) });
    const reply = (scan) => scan.resolve(scan.modIds.map((modId) => ({
      profileId: target.profileId, modId, status: operation === "install" ? "completed" : "not_installed",
      managedFileCount: 1, backupCount: 0, adoptedFileCount: 0, issueCount: 0, issues: [],
    })));
    await act(async () => reply(scanReplies[0]));
    const displayed = h.state.query.page;
    const started = deferred();
    let start;
    await act(async () => { start = trackModLibraryTaskStart(target, () => started.promise); });
    assert.equal(h.state.query.page, displayed);
    assert.equal(h.state.query.initialLoading, false);
    assert.equal(h.state.query.statusTrusted, false);
    assert.equal(calls.length, 2);
    await act(async () => h.api.emit("one", "queued", null, "install"));
    await act(async () => h.api.emit("one", "completed", null, "install"));
    assert.equal(calls.length, 2, "terminal-before-reply does not race the pending writer");
    await act(async () => { started.resolve(task("one")); await start; });
    let synchronized;
    await act(async () => { synchronized = h.state.query.synchronize(); });
    assert.deepEqual(calls, ["query", ["visible"], "query", ["visible", "off-page"]]);
    await act(async () => { reply(scanReplies[1]); await synchronized; });
    assert.equal(h.state.query.statusTrusted, true);
    assert.equal(h.state.cache.readStatusSnapshot(target.gameId, target.profileId).summaries[1].modId, "off-page");
    await act(async () => { h.api.emit("one", "completed", null, "install"); await h.state.query.synchronize(); });
    assert.equal(calls.length, 4);
  });
}

test("a batch and its retry never query during execution and each synchronize only once", options, async (t) => {
  const h = await mountQuery(t);
  await h.resolve(h.pending[0], pageOf("visible"));
  for (const attemptNumber of [0, 1]) {
    const done = deferred();
    let writing;
    await act(async () => { writing = trackModLibraryBatchWrite(() => done.promise); });
    assert.equal(h.state.query.page.items[0].id, "visible");
    assert.equal(h.state.query.initialLoading, false);
    assert.equal(h.pending.length, attemptNumber + 1);
    await act(async () => h.api.emit(`batch-${attemptNumber}`, "completed", null, "install"));
    assert.equal(h.pending.length, attemptNumber + 1);
    await act(async () => { done.resolve({ task: task(`batch-${attemptNumber}`, "completed"), batchId: "batch", attemptNumber }); await writing; });
    await h.resolve(h.pending.at(-1), pageOf("visible"));
    await act(async () => h.state.query.synchronize());
    assert.equal(h.pending.length, attemptNumber + 2);
  }
});

test("failed start and route changes cannot strand an occupancy or accept stale responses", options, async (t) => {
  const h = await mountQuery(t);
  await h.resolve(h.pending[0], pageOf("old"));
  const started = deferred();
  let start;
  await act(async () => { start = trackModLibraryTaskStart(target, () => started.promise).catch(() => {}); });
  await h.update({ show: false });
  await h.update({ show: true });
  assert.equal(h.state.query.page.items[0].id, "old");
  assert.equal(h.pending.length, 1);
  await act(async () => { started.reject(new Error("failed start")); await start; });
  assert.equal(h.pending.length, 2);
  await h.resolve(h.pending[1], pageOf("fresh"));
  assert.equal(h.state.query.statusTrusted, true);
});

test("a failed event connection recovers only after backend progress confirms termination", options, async (t) => {
  const h = await mountQuery(t, { listenerFails: true });
  await h.resolve(h.pending[0], pageOf("visible"));
  await act(async () => trackModLibraryTaskStart(target, async () => task("lost-terminal")));
  assert.equal(h.state.query.writing, true);
  let polls = 0;
  h.api.getTaskProgress = async (taskId) => {
    polls++;
    assert.equal(taskId, "lost-terminal");
    return { ...task(taskId, "completed"), phase: "install.completed" };
  };
  await act(async () => delay(1600));
  assert.equal(polls, 1);
  assert.equal(h.state.query.writing, false);
  assert.equal(h.pending.length, 2);
  await h.resolve(h.pending[1], pageOf("verified"));
  assert.equal(h.state.query.statusTrusted, true);
});
