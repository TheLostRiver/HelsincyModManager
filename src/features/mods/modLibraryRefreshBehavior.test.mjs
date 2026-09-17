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
  test(`${operation} keeps the catalog and converges delayed events on one metadata read`, options, async (t) => {
    const calls = [];
    const scanReplies = [];
    const h = await mountQuery(t, { loadPage: (input, context, cache) => loadModLibraryPageWithStatuses(input, context, {
      cache, query: async () => { calls.push("query"); return pageOf("visible"); },
      states: ({ modIds }) => { calls.push(modIds); const result = deferred(); scanReplies.push({ ...result, modIds }); return result.promise; },
      scan: async () => assert.fail("ordinary writes do not scan the page"),
    }) });
    const reply = (scan) => scan.resolve({ gameId: target.gameId, profileId: target.profileId, epoch: "test", revision: scanReplies.indexOf(scan) + 1,
      reset: false, available: true, modIds: scan.modIds, summaries: scan.modIds.map((modId) => ({
      profileId: target.profileId, modId, status: operation === "install" ? "installed" : "not_installed",
      managedFileCount: 1, backupCount: 0, adoptedFileCount: 0, issueCount: 0, issues: [],
    })) });
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
    assert.deepEqual(calls, ["query", ["visible"], ["visible", "off-page"]]);
    await act(async () => { reply(scanReplies[1]); await synchronized; });
    assert.equal(h.state.query.statusTrusted, true);
    assert.equal(h.state.cache.readStatusSnapshot(target.gameId, target.profileId).summaries[1].modId, "off-page");
    await act(async () => { h.api.emit("one", "completed", null, "install"); await h.state.query.synchronize(); });
    assert.equal(calls.length, 3);
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

test("real provider notifications update each completed batch card without another IPC", options, async (t) => {
  let catalogReads = 0, stateReads = 0, revision = 0;
  const installed = new Set();
  const stateUpdate = (modIds) => ({ gameId: target.gameId, profileId: target.profileId, epoch: "live", revision: ++revision,
    reset: false, available: true, modIds, summaries: modIds.map((modId) => ({ profileId: target.profileId, modId,
      status: installed.has(modId) ? "installed" : "not_installed", managedFileCount: installed.has(modId) ? 1 : 0, backupCount: 0 })) });
  const h = await mountQuery(t, { loadPage: (input, context, cache) => loadModLibraryPageWithStatuses(input, context, {
    cache, query: async () => { catalogReads++; return { ...pageOf("a"), items: [pageOf("a").items[0], pageOf("b").items[0]] }; },
    states: async ({ modIds }) => { stateReads++; return stateUpdate(modIds); },
    scan: async () => assert.fail("no integrity scan for batch progress"),
  }) });
  const pending = deferred();
  let writing;
  await act(async () => { writing = trackModLibraryBatchWrite(() => pending.promise, { gameId: target.gameId, profileId: target.profileId, modIds: ["a", "b"] }); });
  const before = h.state.query.page.items;
  await act(async () => { installed.add("a"); h.api.emitInstallation({ taskId: "live-batch", ...stateUpdate(["a"]) }); });
  assert.equal(h.state.query.page.items[0].status, "installed");
  assert.equal(h.state.query.page.items[1], before[1]);
  assert.equal(h.state.query.writing, true);
  assert.equal(h.state.query.initialLoading, false);
  assert.equal(stateReads, 1);
  // Deliberately drop b's event. The terminal query must repair it.
  installed.add("b");
  await act(async () => { pending.resolve({ task: task("live-batch", "completed") }); await writing; });
  await act(async () => h.state.query.synchronize());
  assert.deepEqual(h.state.query.page.items.map((item) => item.status), ["installed", "installed"]);
  assert.equal(h.state.query.statusTrusted, true);
  assert.equal(catalogReads, 1);
  assert.equal(stateReads, 2);
});

test("an uninstall updates filtered membership and clamps the last page once", options, async (t) => {
  const ids = Array.from({ length: 25 }, (_, index) => `mod-${index}`);
  const installed = new Set(ids);
  let catalogReads = 0, revision = 0;
  const filter = { kind: "status", status: "installed" };
  const h = await mountQuery(t, { loadPage: (input, context, cache) => loadModLibraryPageWithStatuses(input, context, {
    cache, query: async () => {
      catalogReads++;
      const matching = input.filter.kind === "status" ? ids.filter((id) => installed.has(id)) : ids;
      const page = Math.min(input.page, Math.max(1, Math.ceil(matching.length / input.pageSize)));
      return { page, pageSize: input.pageSize, libraryTotal: ids.length, matchingTotal: matching.length,
        items: matching.slice((page - 1) * input.pageSize, page * input.pageSize).map((id) => pageOf(id).items[0]) };
    },
    states: async ({ modIds }) => ({ gameId: target.gameId, profileId: target.profileId, epoch: "filter", revision: ++revision,
      reset: false, available: true, modIds, summaries: modIds.map((modId) => ({ profileId: target.profileId, modId,
        status: installed.has(modId) ? "installed" : "not_installed", managedFileCount: installed.has(modId) ? 1 : 0, backupCount: 0 })) }),
    scan: async () => assert.fail("ordinary filtered refresh does not scan files"),
  }) });
  await h.update({ filter });
  await act(async () => h.state.query.setPage(2));
  assert.equal(h.state.query.page.items[0].id, "mod-24");
  const before = catalogReads;
  await act(async () => trackModLibraryTaskStart({ ...target, modId: "mod-24" }, async () => task("filtered")));
  installed.delete("mod-24");
  await act(async () => h.api.emit("filtered", "completed", null, "install"));
  await act(async () => h.state.query.synchronize());
  assert.equal(h.state.query.page.page, 1);
  assert.equal(h.state.query.page.matchingTotal, 24);
  assert.equal(h.state.query.page.items.length, 24);
  assert.equal(catalogReads, before + 1);
  assert.equal(h.state.query.statusTrusted, true);
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
