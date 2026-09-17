import assert from "node:assert/strict";
import { test } from "node:test";
import { act, mountQuery, pageOf, profileKey, options, loadAfterWriteCallback } from "./testing/reactHarness.mjs";

test("sort changes preserve search/filter/capacity, return to page one and reject old responses", options, async (t) => {
  const h = await mountQuery(t);
  await h.resolve(h.pending[0], pageOf("first"));
  await act(async () => h.state.query.setPage(2));
  const oldPage = h.pending.at(-1);
  await act(async () => h.state.query.setSort("size_desc"));
  const largeFirst = h.pending.at(-1);
  assert.deepEqual(largeFirst.input, { ...oldPage.input, page: 1, sort: "size_desc" });
  await act(async () => h.state.query.setSort("name_desc"));
  const nameDescending = h.pending.at(-1);
  await h.resolve(nameDescending, pageOf("sorted"));
  await h.resolve(largeFirst, pageOf("outdated-size"));
  await h.resolve(oldPage, pageOf("outdated-page", 2));
  assert.equal(h.state.query.page.items[0].id, "sorted");
  assert.equal(h.state.query.sort, "name_desc");
  const count = h.pending.length;
  await act(async () => h.state.query.setSort("name_desc"));
  assert.equal(h.pending.length, count, "Choosing the current rule does not requery");
});

test("background statistics invalidation rejects a pending page and refreshes size ordering", options, async (t) => {
  const h = await mountQuery(t);
  const old = h.pending[0];
  await act(async () => { for (const callback of h.api.statistics) callback({ payload: null }); });
  const fresh = h.pending.at(-1);
  assert.notEqual(fresh, old);
  await h.resolve(old, pageOf("unknown-size"));
  await h.resolve(fresh, pageOf("known-size"));
  assert.equal(h.state.query.page.items[0].id, "known-size");
});

test("cache hit still revalidates and then settles", options, async (t) => {
  const h = await mountQuery(t);
  const old = pageOf("old");
  await h.resolve(h.pending[0], old);
  await h.update({ show: false });
  await h.update({ show: true });
  assert.equal(h.state.query.page, old);
  assert.equal(h.state.query.refreshing, true);
  await h.resolve(h.pending[1], pageOf("fresh"));
  assert.equal(h.state.query.page.items[0].id, "fresh");
  assert.equal(h.state.query.refreshing, false);
});

test("invalidation rejects old responses and finishes a fresh query", options, async (t) => {
  const h = await mountQuery(t);
  const old = h.pending[0];
  await act(async () => h.state.cache.invalidateAllPages());
  assert.equal(h.pending.length, 2, "Invalidation must schedule a replacement query");
  await h.resolve(old, pageOf("before-write"));
  assert.equal(h.state.cache.readPage(profileKey, JSON.stringify(old.input)), null);
  assert.equal(h.state.query.page, null);
  await h.resolve(h.pending[1], pageOf("after-write"));
  assert.equal(h.state.query.page.items[0].id, "after-write");
  assert.equal(h.state.query.initialLoading || h.state.query.refreshing, false);
});

test("query failure keeps display content but never trusts stale facts or loops retries", options, async (t) => {
  const h = await mountQuery(t);
  await h.resolve(h.pending[0], pageOf("old-status"));
  await h.update({ show: false });
  await h.update({ show: true });
  await act(async () => h.pending[1].reject({ code: "mod_library_status_unavailable" }));
  assert.equal(h.state.query.page.items[0].id, "old-status");
  assert.equal(h.state.query.statusTrusted, false, "Unverified facts must not unlock card actions");
  assert.equal(h.state.query.errorCode, "mod_library_status_unavailable");
  assert.equal(h.state.cache.readPage(profileKey, JSON.stringify(h.pending[1].input)), null);
  assert.equal(h.pending.length, 2);
  let retry;
  await act(async () => { retry = h.state.query.refresh(); });
  await h.resolve(h.pending[2], pageOf("recovered"));
  await retry;
  assert.equal(h.state.query.page.items[0].id, "recovered");
});

test("write completion invalidates all slots and does not duplicate refresh requests", options, async (t) => {
  const h = await mountQuery(t);
  const firstInput = h.pending[0].input;
  await h.resolve(h.pending[0], pageOf("first"));
  await act(async () => h.state.query.setPage(2));
  await h.resolve(h.pending[1], pageOf("second", 2));
  const afterWrite = loadAfterWriteCallback(() => h.state.query.synchronize());
  await act(async () => h.api.emit("writer", "queued", null, "install"));
  assert.equal(h.pending.length, 2);
  assert.equal(h.state.query.page.items[0].id, "second");
  assert.equal(h.state.query.initialLoading, false);
  await act(async () => h.api.emit("writer", "completed", null, "install"));
  let refreshed;
  await act(async () => { refreshed = afterWrite(); });
  assert.equal(h.pending.length, 3, "Explicit refresh and invalidation effect must share this request");
  await h.resolve(h.pending[2], pageOf("after-write", 2));
  await refreshed;
  assert.equal(h.state.cache.readPage(profileKey, JSON.stringify(firstInput)), null);
  assert.equal(h.state.query.page.items[0].id, "after-write");
  assert.equal(h.state.query.refreshing, false);
  await act(async () => afterWrite());
  assert.equal(h.pending.length, 3, "A later terminal callback reuses the completed synchronization");
});

test("task completion invalidates cache while the library page is unmounted", options, async (t) => {
  const h = await mountQuery(t);
  const input = h.pending[0].input;
  await h.resolve(h.pending[0], pageOf("before-install"));
  await h.update({ show: false });
  await act(async () => h.api.emit("installation", "queued", null, "install"));
  await act(async () => h.api.emit("installation", "completed", null, "install"));
  assert.equal(h.state.cache.readPage(profileKey, JSON.stringify(input)), null);
  await h.update({ show: true });
  assert.equal(h.state.query.page.items[0].id, "before-install");
  assert.equal(h.state.query.statusTrusted, false);
  await h.resolve(h.pending.at(-1), pageOf("installed"));
  assert.equal(h.state.query.page.items[0].id, "installed");
});

test("active writes do not repopulate cache before their terminal event", options, async (t) => {
  const h = await mountQuery(t);
  await h.resolve(h.pending[0], pageOf("before"));
  await act(async () => h.api.emit("writer", "queued", null, "install"));
  assert.equal(h.pending.length, 1, "No query is started during an active write");
  assert.equal(h.state.query.page.items[0].id, "before");
  assert.equal(h.state.query.statusTrusted, false);
  assert.equal(h.state.cache.readPage(profileKey, JSON.stringify(h.pending[0].input)), null);
  await act(async () => h.api.emit("writer", "completed", null, "install"));
  await h.resolve(h.pending[1], pageOf("after"));
  assert.equal(h.state.query.page.items[0].id, "after");
});

test("failed event subscription disables caching but not queries", options, async (t) => {
  const h = await mountQuery(t, { listenerFails: true });
  await h.resolve(h.pending[0], pageOf("live"));
  assert.equal(h.state.query.page.items[0].id, "live");
  assert.equal(h.state.cache.readPage(profileKey, JSON.stringify(h.pending[0].input)), null);
});

test("profile changes and late responses cannot mix snapshots", options, async (t) => {
  const h = await mountQuery(t);
  await h.resolve(h.pending[0], pageOf("profile-one"));
  await h.update({ profileContext: { gameId: "mhw", profileId: "two" } });
  assert.equal(h.state.query.page, null);
  await h.resolve(h.pending[1], pageOf("profile-two"));
  assert.equal(h.state.query.page.items[0].id, "profile-two");
});

test("automatic and concurrent manual refreshes share one in-flight query", options, async (t) => {
  const h = await mountQuery(t);
  let refreshed;
  await act(async () => { refreshed = h.state.query.refresh(); });
  assert.equal(h.pending.length, 1);
  await h.resolve(h.pending[0], pageOf("new"));
  await refreshed;
  assert.equal(h.state.query.page.items[0].id, "new");
});

test("unmounted hooks reject pending responses and later refresh callbacks", options, async (t) => {
  const h = await mountQuery(t);
  const refresh = h.state.query.refresh;
  await h.update({ show: false });
  await h.resolve(h.pending[0], pageOf("late"));
  assert.equal(h.state.cache.readPage(profileKey, JSON.stringify(h.pending[0].input)), null);
  let lateRefresh;
  await act(async () => { lateRefresh = refresh(); });
  assert.equal(h.pending.length, 1);
  assert.equal(await lateRefresh, null);
});

test("backend clamping caches the actual page without a duplicate query", options, async (t) => {
  const h = await mountQuery(t);
  await h.resolve(h.pending[0], pageOf("first"));
  await act(async () => h.state.query.setPage(8));
  await h.resolve(h.pending[1], pageOf("clamped", 2));
  assert.equal(h.pending.length, 2);
  assert.equal(h.state.query.page.page, 2);
  assert.equal(h.state.cache.readPage(profileKey, JSON.stringify(h.pending[1].input)), null);
  assert.equal(h.state.cache.readPage(profileKey, JSON.stringify({ ...h.pending[1].input, page: 2 })).items[0].id, "clamped");
});

test("fail-closed item updates stop pending refreshes without getting stuck", options, async (t) => {
  const h = await mountQuery(t);
  await h.resolve(h.pending[0], pageOf("old"));
  let refreshed;
  await act(async () => { refreshed = h.state.query.refresh(); });
  await act(async () => h.state.query.updateCurrentPageItems((items) => items.map((item) => ({ ...item, status: "unknown" }))));
  assert.equal(h.state.query.refreshing, false);
  assert.equal(h.state.query.page.items[0].status, "unknown");
  await h.resolve(h.pending[1], pageOf("stale-response"));
  await refreshed;
  assert.equal(h.state.query.page.items[0].status, "unknown");
  assert.equal(h.state.cache.readPage(profileKey, JSON.stringify(h.pending[1].input)), null);
});

test("fail-closed marking without a page produces a retryable error", options, async (t) => {
  const h = await mountQuery(t);
  await act(async () => h.state.query.updateCurrentPageItems((items) => items));
  assert.equal(h.state.query.initialLoading, false);
  assert.equal(h.state.query.errorCode, "mod_library_status_unavailable");
  await h.resolve(h.pending[0], pageOf("late"));
  assert.equal(h.state.query.page, null);
});

test("stale profile callbacks cannot cancel the current profile query", options, async (t) => {
  const h = await mountQuery(t);
  await h.resolve(h.pending[0], pageOf("one"));
  const staleUpdate = h.state.query.updateCurrentPageItems;
  await h.update({ profileContext: { gameId: "mhw", profileId: "two" } });
  await act(async () => staleUpdate((items) => items));
  await h.resolve(h.pending[1], pageOf("two"));
  assert.equal(h.state.query.page.items[0].id, "two");
});

test("a synchronous loader failure releases the in-flight request for explicit retry", options, async (t) => {
  let calls = 0;
  const h = await mountQuery(t, { loadPage: () => {
    calls++;
    if (calls === 1) throw { code: "mod_library_status_unavailable" };
    return Promise.resolve(pageOf("recovered"));
  } });
  assert.equal(h.state.query.statusTrusted, false);
  assert.equal(h.state.query.errorCode, "mod_library_status_unavailable");
  await act(async () => h.state.query.refresh());
  assert.equal(h.state.query.statusTrusted, true);
  assert.equal(calls, 2);
});
