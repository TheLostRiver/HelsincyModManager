import assert from "node:assert/strict";
import { test } from "node:test";
import { createModInstallationStateCache } from "./modInstallationStateCache.ts";
import { createModLibrarySessionStore } from "./modLibrarySessionStore.ts";

const scope = { gameId: "mhw", profileId: "scope" };
const summary = (modId, status = "installed") => ({ profileId: scope.profileId, modId, status,
  managedFileCount: status === "not_installed" ? 0 : 1, backupCount: 0, adoptedFileCount: 0, installedRevisionId: "revision" });
const update = (revision, ids, extra = {}) => ({ ...scope, epoch: "first", revision, reset: false, available: true,
  modIds: ids, summaries: ids.map((id) => summary(id)), ...extra });
const page = () => ({ page: 1, pageSize: 24, libraryTotal: 2, matchingTotal: 2,
  items: ["a", "b"].map((id) => ({ id, name: id, status: "not_installed", categoryLabels: [], sizeLabel: "" })) });
const statuses = (cache, input) => cache.project(input, scope.gameId, scope.profileId).items.map((item) => item.status);
const recovery = (modId, status) => ({ profileId: scope.profileId, modId, status, managedFileCount: 1,
  backupCount: 0, adoptedFileCount: 0, issueCount: status === "repair_required" ? 1 : 0, issues: [] });

test("per-Mod revisions accept an older event for another Mod and reject regressions", () => {
  const cache = createModInstallationStateCache();
  const input = page();
  cache.apply(update(1, ["a", "b"], { reset: true, summaries: [summary("a", "not_installed"), summary("b", "not_installed")] }), "query");
  cache.apply(update(3, ["b"]), "event");
  cache.apply(update(2, ["a"]), "event");
  assert.deepEqual(statuses(cache, input), ["installed", "installed"]);
  cache.apply(update(2, ["b"], { summaries: [summary("b", "not_installed")] }), "event");
  assert.deepEqual(statuses(cache, input), ["installed", "installed"]);
});

test("a delayed reset query cannot overwrite a newer Mod event", () => {
  const cache = createModInstallationStateCache();
  cache.apply(update(3, ["b"]), "event");
  cache.apply(update(2, ["a", "b"], { reset: true, summaries: [summary("a"), summary("b", "not_installed")] }), "query");
  assert.deepEqual(statuses(cache, page()), ["installed", "installed"]);
  cache.apply(update(1, ["a"], { reset: true, available: false, summaries: [] }), "event");
  assert.deepEqual(statuses(cache, page()), ["installed", "installed"]);
});

test("reset makes omitted cards unknown; only a query can establish another epoch", () => {
  const cache = createModInstallationStateCache();
  cache.apply(update(1, ["a", "b"]), "query");
  const input = cache.project(page(), scope.gameId, scope.profileId);
  cache.apply(update(2, ["a"], { reset: true }), "event");
  assert.deepEqual(statuses(cache, input), ["installed", "unknown"]);
  assert.equal(cache.apply(update(1, ["a"], { epoch: "second" }), "event"), false);
  cache.apply(update(1, ["a", "b"], { epoch: "second", reset: true, summaries: [summary("a", "not_installed"), summary("b", "not_installed")] }), "query");
  cache.apply(update(100, ["a", "b"]), "event");
  assert.deepEqual(statuses(cache, input), ["not_installed", "not_installed"]);
  assert.equal(cache.apply(update(101, ["a", "b"]), "query"), false);
});

test("late unavailability cannot poison a newer confirmed query", () => {
  const cache = createModInstallationStateCache();
  cache.apply(update(4, ["a", "b"]), "query");
  cache.apply(update(3, ["a"], { reset: true, available: false, summaries: [] }), "event");
  assert.deepEqual(statuses(cache, page()), ["installed", "installed"]);
  cache.apply(update(5, ["a"], { reset: true, available: false, summaries: [] }), "event");
  assert.deepEqual(statuses(cache, page()), ["unknown", "unknown"]);
});

test("known integrity failures survive ordinary metadata updates until a newer scan", () => {
  const cache = createModInstallationStateCache();
  const input = page();
  cache.apply(update(1, ["a", "b"]), "query");
  cache.recordIntegrity(scope.gameId, scope.profileId, ["a"], 2, [recovery("a", "repair_required")]);
  cache.apply(update(3, ["a"]), "event");
  assert.deepEqual(statuses(cache, input), ["repair_required", "installed"]);
  cache.recordIntegrity(scope.gameId, scope.profileId, [], 1, [recovery("a", "completed")]);
  assert.equal(statuses(cache, input)[0], "repair_required");
  cache.recordIntegrity(scope.gameId, scope.profileId, ["a"], 3, [recovery("a", "completed")]);
  assert.equal(statuses(cache, input)[0], "installed");
  cache.recordIntegrity(scope.gameId, scope.profileId, [], 4, null);
  cache.apply(update(5, ["a", "b"]), "query");
  assert.deepEqual(statuses(cache, input), ["unknown", "unknown"]);
  cache.recordIntegrity(scope.gameId, scope.profileId, [], 5, []);
  assert.deepEqual(statuses(cache, input), ["installed", "installed"]);
});

test("an old successful scan does not turn a subsequently uninstalled Mod into installed", () => {
  const cache = createModInstallationStateCache();
  cache.apply(update(1, ["a"], { summaries: [summary("a", "not_installed")] }), "query");
  cache.recordIntegrity(scope.gameId, scope.profileId, ["a"], 1, [recovery("a", "completed")]);
  assert.equal(statuses(cache, page())[0], "not_installed");
});

test("batch item notifications update cards while writing without invalidating catalog queries", () => {
  const store = createModLibrarySessionStore();
  store.setAvailable(true);
  store.acceptInstallationStates(update(1, ["a", "b"], { summaries: [summary("a", "not_installed"), summary("b", "not_installed")] }), store.getGeneration());
  const input = page();
  const before = store.projectPage(input, scope);
  const writer = store.beginWrite({ ...scope, modIds: ["a", "b"] });
  const generation = store.getGeneration();
  store.observeInstallationState({ taskId: "batch", ...update(2, ["a"]) });
  const first = store.projectPage(input, scope);
  assert.equal(first.items[0].status, "installed");
  assert.equal(first.items[1], before.items[1]);
  store.observeInstallationState({ taskId: "batch", ...update(3, ["b"]) });
  const second = store.projectPage(input, scope);
  assert.equal(second.items[0], first.items[0]);
  assert.equal(second.items[1].status, "installed");
  assert.equal(store.getGeneration(), generation);
  assert.equal(store.isWriting(), true);
  store.finishWrite(writer);
});

test("both successful and failed scans crossing a write are discarded", () => {
  const store = createModLibrarySessionStore();
  const finish = store.beginIntegrityScan(scope.gameId, scope.profileId, ["a"]);
  const failed = store.beginIntegrityScan(scope.gameId, scope.profileId, []);
  const writer = store.beginWrite({ ...scope, modId: "a" });
  const during = store.beginIntegrityScan(scope.gameId, scope.profileId, ["a"]);
  store.finishWrite(writer);
  store.acceptInstallationStates(update(1, ["a", "b"]), store.getGeneration());
  finish([recovery("a", "repair_required")]);
  failed(null);
  during([recovery("a", "repair_required")]);
  assert.deepEqual(store.projectPage(page(), scope).items.map((item) => item.status), ["installed", "installed"]);
  store.beginIntegrityScan(scope.gameId, scope.profileId, ["a"])([recovery("a", "repair_required")]);
  assert.equal(store.projectPage(page(), scope).items[0].status, "repair_required");
});

test("other scopes never inherit status or integrity findings", () => {
  const cache = createModInstallationStateCache();
  cache.apply(update(1, ["a"]), "query");
  cache.recordIntegrity(scope.gameId, scope.profileId, ["a"], 1, [recovery("a", "repair_required")]);
  assert.equal(cache.project(page(), "mhw", "other").items[0].status, "not_installed");
});
