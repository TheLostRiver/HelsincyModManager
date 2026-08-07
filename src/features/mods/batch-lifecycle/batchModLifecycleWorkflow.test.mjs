import assert from "node:assert/strict";
import { test } from "node:test";
import {
  batchModLifecycleRequestExceedsLimit,
  buildBatchModLifecycleRequest,
  preferNewestBatchRevision,
  resolveBatchModLifecycleItems,
} from "./batchModLifecycleWorkflow.ts";
import { BATCH_MOD_LIFECYCLE_MAX_ITEMS } from "./batchModLifecycleTypes.ts";
import { commandErrorCode } from "./useBatchModLifecycleWorkflow.ts";

const revisions = (modId, displayRevisionId, revisionIds) => ({
  modId,
  originRevisionId: "origin-1",
  displayRevisionId,
  revisions: revisionIds.map((revisionId) => ({ revisionId })),
});

const manifestStatus = (modId, status, installedRevisionId = null) => ({
  profileId: "default",
  modId,
  status,
  managedFileCount: status === "installed" ? 1 : 0,
  backupCount: 0,
  installedRevisionId,
});

test("install resolution includes only not-installed mods with a resolvable revision", () => {
  const resolution = resolveBatchModLifecycleItems({
    operation: "install",
    selectedModIds: ["mod-a", "mod-b", "mod-c", "mod-d"],
    manifestStatuses: [
      manifestStatus("mod-a", "not_installed"),
      manifestStatus("mod-b", "installed", "rev-1"),
      manifestStatus("mod-c", "not_installed"),
      manifestStatus("mod-d", "not_installed"),
    ],
    revisionsByMod: {
      "mod-a": revisions("mod-a", "rev-2", [{ revisionId: "rev-1" }, { revisionId: "rev-2" }]),
      "mod-c": revisions("mod-c", "", []),
      "mod-d": revisions("mod-d", "rev-3", [{ revisionId: "rev-3" }]),
    },
    preferRevision: (list) => {
      if (list.displayRevisionId.length > 0) return list.displayRevisionId;
      return list.revisions[0]?.revisionId ?? null;
    },
  });

  assert.deepEqual(resolution.items, [
    {
      operation: "install",
      modId: "mod-a",
      revisionId: "rev-2",
      layer: { name: "base", priority: 0 },
    },
    {
      operation: "install",
      modId: "mod-d",
      revisionId: "rev-3",
      layer: { name: "base", priority: 0 },
    },
  ]);
  assert.deepEqual(resolution.excluded, [{ modId: "mod-b", reason: "already_installed" }]);
  assert.deepEqual(resolution.unresolvable, ["mod-c"]);
});

test("uninstall resolution requires installed revision facts", () => {
  const resolution = resolveBatchModLifecycleItems({
    operation: "uninstall",
    selectedModIds: ["mod-a", "mod-b", "mod-c"],
    manifestStatuses: [
      manifestStatus("mod-a", "installed", "rev-1"),
      manifestStatus("mod-b", "installed", null),
      manifestStatus("mod-c", "not_installed"),
    ],
    revisionsByMod: {},
    preferRevision: () => null,
  });

  assert.deepEqual(resolution.items, [
    {
      operation: "uninstall",
      modId: "mod-a",
      expectedInstalledRevisionId: "rev-1",
    },
  ]);
  assert.deepEqual(resolution.excluded, [
    { modId: "mod-b", reason: "installed_revision_unavailable" },
    { modId: "mod-c", reason: "not_installed" },
  ]);
  assert.deepEqual(resolution.unresolvable, []);
});

test("reinstall resolution pairs installed and candidate revisions", () => {
  const resolution = resolveBatchModLifecycleItems({
    operation: "reinstall",
    selectedModIds: ["mod-a", "mod-b"],
    manifestStatuses: [
      manifestStatus("mod-a", "installed", "rev-1"),
      manifestStatus("mod-b", "installed", "rev-2"),
    ],
    revisionsByMod: {
      "mod-a": revisions("mod-a", "rev-3", [{ revisionId: "rev-3" }]),
      "mod-b": revisions("mod-b", "", []),
    },
    preferRevision: (list) => {
      if (list.displayRevisionId.length > 0) return list.displayRevisionId;
      return list.revisions[0]?.revisionId ?? null;
    },
  });

  assert.deepEqual(resolution.items, [
    {
      operation: "reinstall",
      modId: "mod-a",
      installedRevisionId: "rev-1",
      candidateRevisionId: "rev-3",
      layer: { name: "base", priority: 0 },
    },
  ]);
  assert.deepEqual(resolution.unresolvable, ["mod-b"]);
});

test("request building keeps stable schema, policy and controlled ids", () => {
  const request = buildBatchModLifecycleRequest({
    operation: "uninstall",
    gameId: "mhw",
    profileId: "default",
    policy: "continue_on_item_failure",
    items: [
      {
        operation: "uninstall",
        modId: "mod-a",
        expectedInstalledRevisionId: "rev-1",
      },
    ],
  });

  assert.deepEqual(request, {
    schemaVersion: 1,
    operation: "uninstall",
    gameId: "mhw",
    profileId: "default",
    executionPolicy: "continue_on_item_failure",
    items: [
      {
        operation: "uninstall",
        modId: "mod-a",
        expectedInstalledRevisionId: "rev-1",
      },
    ],
  });
  assert.equal(batchModLifecycleRequestExceedsLimit(request), false);
});

test("request building carries only explicit replacement target selections", () => {
  const request = buildBatchModLifecycleRequest({
    operation: "reinstall",
    gameId: "mhw",
    profileId: "default",
    policy: "stop_on_failure",
    items: [
      {
        operation: "reinstall",
        modId: "armor-mod",
        installedRevisionId: "rev-1",
        candidateRevisionId: "rev-1",
        layer: { name: "base", priority: 0 },
      },
    ],
    replacementTargets: [{ modId: "armor-mod", targetId: "mhw:armor:fatalis-beta" }],
  });

  assert.deepEqual(request.replacementTargets, [
    { modId: "armor-mod", targetId: "mhw:armor:fatalis-beta" },
  ]);
});

test("request limit guards the documented batch maximum", () => {
  const request = buildBatchModLifecycleRequest({
    operation: "install",
    gameId: "mhw",
    profileId: "default",
    policy: "stop_on_failure",
    items: Array.from({ length: BATCH_MOD_LIFECYCLE_MAX_ITEMS + 1 }, (_, index) => ({
      operation: "install",
      modId: `mod-${index}`,
      revisionId: "rev-1",
      layer: { name: "base", priority: 0 },
    })),
  });

  assert.equal(batchModLifecycleRequestExceedsLimit(request), true);
});

test("newest revision preference falls back safely", () => {
  assert.equal(preferNewestBatchRevision([]), null);
  assert.equal(
    preferNewestBatchRevision([{ revisionId: "rev-1" }, { revisionId: "rev-2" }]),
    "rev-1",
  );
});

test("command error codes stay stable and redacted", () => {
  assert.equal(commandErrorCode({ code: "batch_plan_stale" }), "batch_plan_stale");
  assert.equal(commandErrorCode("batch_retry_unavailable"), "batch_retry_unavailable");
  assert.equal(commandErrorCode({ message: "raw backend text" }), "batch_internal_error");
  assert.equal(commandErrorCode(undefined), "batch_internal_error");
});
