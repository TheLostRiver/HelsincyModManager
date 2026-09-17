import assert from "node:assert/strict";
import { test } from "node:test";
import { loadModLibraryPageWithStatuses } from "./modLibraryPageLoader.ts";
import { createModLibrarySessionStore } from "./modLibrarySessionStore.ts";

const input = { profileContext: { gameId: "mhw", profileId: "scope" }, search: "", filter: { kind: "all" }, sort: "name_asc", page: 1, pageSize: 24 };
const page = { items: [{ id: "visible", name: "Visible", status: "not_installed" }], page: 1, pageSize: 24, libraryTotal: 2, matchingTotal: 1 };
let revision = 0;
const summaries = (ids) => ({ gameId: "mhw", profileId: "scope", epoch: "test", revision: ++revision, reset: false, available: true,
  modIds: ids, summaries: ids.map((modId) => ({ modId, profileId: "scope", status: "installed", managedFileCount: 1, backupCount: 0, adoptedFileCount: 0 })) });

test("a terminal target filtered off the page shares a metadata query and proof", async () => {
  const cache = createModLibrarySessionStore();
  const write = cache.beginWrite({ gameId: "mhw", profileId: "scope", modId: "off-page" });
  cache.finishWrite(write);
  const generation = cache.getGeneration();
  const calls = [];
  const result = await loadModLibraryPageWithStatuses(input, { generation, isCurrent: () => true }, {
    cache, query: async () => { calls.push("query"); return page; },
    states: async ({ modIds }) => { calls.push(modIds); return summaries(modIds); },
    scan: async () => assert.fail("ordinary writes must not hash the page"),
  });
  assert.deepEqual(calls, ["query", ["visible", "off-page"]]);
  assert.equal(result.statusVerified, true);
  assert.equal(result.items.length, 1);
  assert.equal(cache.readStatusSnapshot("mhw", "scope").summaries[1].modId, "off-page");
  assert.deepEqual(cache.pendingStatusModIds("mhw", "scope"), []);
  await loadModLibraryPageWithStatuses(input, { generation, isCurrent: () => true }, {
    cache, query: async () => ({ ...page, items: [{ id: "next-page", name: "Next page", status: "not_installed" }] }),
    states: async ({ modIds }) => summaries(modIds),
    scan: async () => assert.fail("ordinary navigation must not hash the page"),
  });
  assert.ok(cache.readStatusSnapshot("mhw", "scope").summaries.some((summary) => summary.modId === "off-page"),
    "query changes in the same write generation must not discard a verified terminal target");
  cache.invalidateAllPages();
  assert.equal(cache.readStatusSnapshot("mhw", "scope"), null);
});

test("invalidating a catalog request before its reply stops the following IPC", async () => {
  let current = true;
  let resolve;
  let scans = 0;
  const pending = loadModLibraryPageWithStatuses(input, { generation: 0, isCurrent: () => current }, {
    cache: createModLibrarySessionStore(), query: () => new Promise((yes) => { resolve = yes; }),
    states: async () => { scans++; return []; },
  });
  current = false;
  resolve(page);
  await pending;
  assert.equal(scans, 0);
});

test("failed metadata reads retain pending targets and cannot yield a terminal proof", async () => {
  const cache = createModLibrarySessionStore();
  cache.finishWrite(cache.beginWrite({ gameId: "mhw", profileId: "scope", modId: "off-page" }));
  const result = await loadModLibraryPageWithStatuses(input, { generation: cache.getGeneration(), isCurrent: () => true }, {
    cache, query: async () => page, states: async () => { throw new Error("unavailable"); },
  });
  assert.equal(result.statusVerified, false);
  assert.equal(cache.readStatusSnapshot("mhw", "scope").verified, false);
  assert.deepEqual(cache.pendingStatusModIds("mhw", "scope"), ["off-page"]);
});

test("write synchronization retains the catalog and repairs missed changes on cached pages", async () => {
  const cache = createModLibrarySessionStore();
  cache.setAvailable(true);
  const profileKey = "profile:mhw\u0000scope";
  cache.writePage(profileKey, JSON.stringify(input), page, cache.getGeneration());
  cache.writePage(profileKey, JSON.stringify({ ...input, page: 2 }), { ...page, page: 2, items: [{ id: "shared-owner", status: "installed" }] }, cache.getGeneration());
  cache.finishWrite(cache.beginWrite({ gameId: "mhw", profileId: "scope", modIds: ["off-page"] }));
  const result = await loadModLibraryPageWithStatuses(input, { generation: cache.getGeneration(), isCurrent: () => true }, {
    cache, query: async () => assert.fail("unchanged catalog must be retained"),
    scan: async () => assert.fail("must not scan files"),
    states: async ({ modIds }) => {
      assert.deepEqual(modIds, ["visible", "shared-owner", "off-page"]);
      const update = summaries(modIds);
      update.summaries[1].status = "not_installed";
      return update;
    },
  });
  assert.equal(result.statusVerified, true);
  assert.equal(cache.readDisplayPage(profileKey, JSON.stringify({ ...input, page: 2 })).items[0].status, "not_installed");
});

test("explicit refresh queries the catalog and requests integrity verification", async () => {
  const cache = createModLibrarySessionStore();
  const calls = [];
  const result = await loadModLibraryPageWithStatuses(input, { generation: 0, isCurrent: () => true, refresh: true }, {
    cache, query: async () => { calls.push("catalog"); return page; },
    states: async ({ modIds }) => { calls.push("metadata"); return summaries(modIds); },
    scan: async ({ modIds }) => { calls.push("integrity"); return modIds.map((id) => ({ modId: id, profileId: "scope", status: "completed" })); },
  });
  assert.equal(result.statusVerified, true);
  assert.deepEqual(calls, ["catalog", "metadata", "integrity"]);
});

test("status filters always requery membership and keep backend page clamping", async () => {
  const cache = createModLibrarySessionStore();
  cache.setAvailable(true);
  const filtered = { ...input, filter: { kind: "status", status: "installed" }, page: 2 };
  cache.writePage("profile:mhw\u0000scope", JSON.stringify(filtered), page, cache.getGeneration());
  let calls = 0;
  const result = await loadModLibraryPageWithStatuses(filtered, { generation: cache.getGeneration(), isCurrent: () => true }, {
    cache, query: async () => { calls++; return { ...page, page: 1, matchingTotal: 1 }; },
    states: async ({ modIds }) => summaries(modIds), scan: async () => assert.fail("no scan"),
  });
  assert.equal(calls, 1);
  assert.equal(result.page, 1);
  assert.equal(result.matchingTotal, 1);
});

test("missing, duplicate, and cross-scope metadata cannot verify stale page data", async () => {
  for (const changed of [[], ["other"], ["visible", "visible"], ["wrong-profile"]]) {
    const cache = createModLibrarySessionStore();
    const result = await loadModLibraryPageWithStatuses(input, { generation: 0, isCurrent: () => true }, {
      cache, query: async () => page,
      states: async () => {
        const update = summaries(["visible"]);
        update.summaries = changed.map((id) => ({ ...update.summaries[0], modId: id === "wrong-profile" ? "visible" : id,
          profileId: id === "wrong-profile" ? "other" : "scope" }));
        return update;
      },
    });
    assert.equal(result.statusVerified, false);
    assert.equal(result.items[0].status, "unknown");
    assert.equal(result.items[0].name, "Visible");
  }
});

test("an empty page needs no state query or integrity scan", async () => {
  const result = await loadModLibraryPageWithStatuses(input, { generation: 0, isCurrent: () => true }, {
    cache: createModLibrarySessionStore(), query: async () => ({ ...page, items: [] }),
    states: async () => assert.fail("no ids"), scan: async () => assert.fail("no ids"),
  });
  assert.equal(result.statusVerified, true);
});

test("state replies from before a write cannot clear its dirty target or mark it verified", async () => {
  const cache = createModLibrarySessionStore();
  let respond;
  const generation = cache.getGeneration();
  const pending = loadModLibraryPageWithStatuses(input, { generation, isCurrent: () => generation === cache.getGeneration() }, {
    cache, query: async () => page, states: () => new Promise((resolve) => { respond = resolve; }),
  });
  await Promise.resolve();
  const token = cache.beginWrite({ gameId: "mhw", profileId: "scope", modId: "visible" });
  respond(summaries(["visible"]));
  await pending;
  cache.finishWrite(token);
  assert.equal(cache.readStatusSnapshot("mhw", "scope"), null);
  assert.deepEqual(cache.pendingStatusModIds("mhw", "scope"), ["visible"]);
});
