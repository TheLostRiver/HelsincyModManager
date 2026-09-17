import assert from "node:assert/strict";
import { test } from "node:test";
import { loadModLibraryPageWithStatuses } from "./modLibraryPageLoader.ts";
import { createModLibrarySessionStore } from "./modLibrarySessionStore.ts";

const input = { profileContext: { gameId: "mhw", profileId: "scope" }, search: "", filter: { kind: "all" }, sort: "name_asc", page: 1, pageSize: 24 };
const page = { items: [{ id: "visible", name: "Visible", status: "not_installed" }], page: 1, pageSize: 24, libraryTotal: 2, matchingTotal: 1 };
const summaries = (ids) => ids.map((modId) => ({ modId, profileId: "scope", status: "completed", managedFileCount: 1, backupCount: 0, adoptedFileCount: 0, issueCount: 0, issues: [] }));

test("a terminal target filtered off the page shares the page scan and proof", async () => {
  const cache = createModLibrarySessionStore();
  const write = cache.beginWrite({ gameId: "mhw", profileId: "scope", modId: "off-page" });
  cache.finishWrite(write);
  const generation = cache.getGeneration();
  const calls = [];
  const result = await loadModLibraryPageWithStatuses(input, { generation, isCurrent: () => true }, {
    cache, query: async () => { calls.push("query"); return page; },
    scan: async ({ modIds }) => { calls.push(modIds); return summaries(modIds); },
  });
  assert.deepEqual(calls, ["query", ["visible", "off-page"]]);
  assert.equal(result.statusVerified, true);
  assert.equal(result.items.length, 1);
  assert.equal(cache.readStatusSnapshot("mhw", "scope").summaries[1].modId, "off-page");
  assert.deepEqual(cache.pendingStatusModIds("mhw", "scope"), []);
  await loadModLibraryPageWithStatuses(input, { generation, isCurrent: () => true }, {
    cache, query: async () => ({ ...page, items: [{ id: "next-page", name: "Next page", status: "not_installed" }] }),
    scan: async ({ modIds }) => summaries(modIds),
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
    scan: async () => { scans++; return []; },
  });
  current = false;
  resolve(page);
  await pending;
  assert.equal(scans, 0);
});

test("failed scans retain pending targets and cannot yield a terminal proof", async () => {
  const cache = createModLibrarySessionStore();
  cache.finishWrite(cache.beginWrite({ gameId: "mhw", profileId: "scope", modId: "off-page" }));
  const result = await loadModLibraryPageWithStatuses(input, { generation: cache.getGeneration(), isCurrent: () => true }, {
    cache, query: async () => page, scan: async () => { throw new Error("unavailable"); },
  });
  assert.equal(result.statusVerified, false);
  assert.equal(cache.readStatusSnapshot("mhw", "scope").verified, false);
  assert.deepEqual(cache.pendingStatusModIds("mhw", "scope"), ["off-page"]);
});
