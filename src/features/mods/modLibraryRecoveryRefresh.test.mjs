import assert from "node:assert/strict";
import { test } from "node:test";
import { createModLibraryStatusProbe, refreshModLibraryDurableStatuses } from "./modLibraryRecoveryRefresh.ts";

const item = (id) => ({ ...createModLibraryStatusProbe(id, id), status: "not_installed" });
const summary = (modId, status = "completed") => ({ profileId: "scope", modId, status,
  managedFileCount: 2, backupCount: 1, adoptedFileCount: 1, issueCount: 0, issues: [] });

test("one recovery scan covers unique page and off-page terminal targets with complete card facts", async () => {
  const calls = [];
  const result = await refreshModLibraryDurableStatuses([item("a"), item("a")], {
    profileId: "scope", loadRecoveryStatuses: async (ids) => { calls.push(ids); return ids.map((id) => summary(id)); },
  }, ["b", "a"]);
  assert.deepEqual(calls, [["a", "b"]]);
  assert.equal(result.verified, true);
  assert.equal(result.items[0].status, "installed");
  assert.deepEqual(result.items[0].installSummary, {
    status: "installed", managedFileCount: 2, backupCount: 1, adoptedFileCount: 1,
    recoveryStatus: "completed", issueCount: 0, issues: [],
  });
  assert.deepEqual(result.summaries.map((s) => s.modId), ["a", "b"]);
});

test("empty target set never invokes the full-scope empty-id scan", async () => {
  const result = await refreshModLibraryDurableStatuses([], {
    profileId: "scope", loadRecoveryStatuses: () => { throw new Error("must not scan"); },
  });
  assert.deepEqual(result, { items: [], verified: true, summaries: [] });
});

test("scan failure fails closed even for a previously not-installed item", async () => {
  const result = await refreshModLibraryDurableStatuses([item("a")], {
    profileId: "scope", loadRecoveryStatuses: async () => { throw new Error("unavailable"); },
  });
  assert.equal(result.verified, false);
  assert.equal(result.items[0].installSummary.status, "unknown");
  assert.equal(result.items[0].name, "a");
});

test("missing, duplicate, and cross-scope scan facts cannot verify stale page data", async () => {
  for (const summaries of [[], [summary("other")], [summary("a"), summary("a")], [{ ...summary("a"), profileId: "other" }]]) {
    const result = await refreshModLibraryDurableStatuses([item("a")], {
      profileId: "scope", loadRecoveryStatuses: async () => summaries,
    });
    assert.equal(result.verified, false);
    assert.equal(result.items[0].installSummary.status, "unknown");
  }
});

test("unsafe states and issues are retained from the single scan", async () => {
  const unsafe = { ...summary("a", "repair_required"), issueCount: 1, issues: [{ issue: "target_changed", count: 1 }] };
  const result = await refreshModLibraryDurableStatuses([item("a")], {
    profileId: "scope", loadRecoveryStatuses: async () => [unsafe],
  });
  assert.equal(result.items[0].installSummary.status, "repair_required");
  assert.deepEqual(result.items[0].installSummary.issues, unsafe.issues);
});

test("superseded requests do not start another scan", async () => {
  let calls = 0;
  const result = await refreshModLibraryDurableStatuses([item("a")], {
    profileId: "scope", isCurrent: () => false, loadRecoveryStatuses: async () => { calls++; return [summary("a")]; },
  });
  assert.equal(result.verified, false);
  assert.equal(calls, 0);
});
