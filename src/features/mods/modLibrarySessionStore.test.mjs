import assert from "node:assert/strict";
import { test } from "node:test";
import { createModLibrarySessionStore } from "./modLibrarySessionStore.ts";

const page = { items: [], page: 1, pageSize: 24, libraryTotal: 0, matchingTotal: 0 };
const event = (taskId, status, kind = "install") => ({ taskId, status, kind });
function readyStore() { const store = createModLibrarySessionStore(); store.setAvailable(true); return store; }

test("cache rejects stale generation writes even when the cache was empty", () => {
  const store = readyStore();
  const generation = store.getGeneration();
  store.invalidateAllPages();
  store.writePage("p", "q", page, generation);
  assert.equal(store.readPage("p", "q"), null);
  store.writePage("p", "q", page, store.getGeneration());
  assert.equal(store.readPage("p", "q"), page);
});

test("cache writes and query-error evictions do not notify query subscribers", () => {
  const store = readyStore();
  let notifications = 0;
  const stop = store.subscribe(() => { notifications += 1; });
  store.writePage("p", "q", page, store.getGeneration());
  store.invalidatePage("p", "q");
  assert.equal(notifications, 0);
  store.invalidateAllPages();
  assert.equal(notifications, 1);
  stop();
  store.invalidateAllPages();
  assert.equal(notifications, 1);
});

test("overlapping task identities keep caching disabled until both settle", () => {
  const store = readyStore();
  store.observeTask(event("one", "queued"));
  store.observeTask(event("two", "running"));
  store.observeTask(event("one", "completed"));
  store.writePage("p", "q", page, store.getGeneration());
  assert.equal(store.readPage("p", "q"), null);
  store.observeTask(event("two", "failed"));
  store.writePage("p", "q", page, store.getGeneration());
  assert.equal(store.readPage("p", "q"), page);
});

test("duplicate and reordered task events cannot re-open completed write scopes", () => {
  const store = readyStore();
  store.observeTask(event("one", "completed"));
  const generation = store.getGeneration();
  store.observeTask(event("one", "queued"));
  store.observeTask(event("one", "completed"));
  assert.equal(store.getGeneration(), generation);
  store.writePage("p", "q", page, generation);
  assert.equal(store.readPage("p", "q"), page);
});

test("read-only task events do not invalidate library pages", () => {
  const store = readyStore();
  const generation = store.getGeneration();
  store.observeTask(event("scan", "running", "external_state_scan"));
  store.observeTask(event("scan", "completed", "external_state_scan"));
  assert.equal(store.getGeneration(), generation);
});

test("external import discovery stays read-only despite sharing the import task kind", () => {
  const store = readyStore();
  const generation = store.getGeneration();
  store.observeTask({ ...event("scan", "queued", "mod_import"), phase: "external_import.scan.queued" });
  store.observeTask({ ...event("scan", "completed", "mod_import"), phase: "external_import.scan.completed" });
  assert.equal(store.getGeneration(), generation);
});

test("category replies from an invalidated generation cannot restore old labels", () => {
  const store = readyStore();
  const generation = store.getGeneration();
  store.invalidateAllPages();
  store.writeCategories([{ id: "old", name: "old" }], generation);
  assert.equal(store.readCategories(), null);
});

test("loss of task observation invalidates and disables cached snapshots", () => {
  const store = readyStore();
  store.writePage("p", "q", page, store.getGeneration());
  store.setAvailable(false);
  store.writePage("p", "q", page, store.getGeneration());
  assert.equal(store.readPage("p", "q"), null);
});
