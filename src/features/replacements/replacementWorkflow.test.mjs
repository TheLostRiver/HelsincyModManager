import assert from "node:assert/strict";
import { test } from "node:test";

import {
  canStartInitialRetargetInstall,
  isRetargetInstallTaskPhase,
  nextRetargetInstallTaskState,
} from "./replacementWorkflow.ts";

test("initial retarget install is enabled only for a safe preview and not-installed state", () => {
  const safe = {
    installStatus: "not_installed",
    completedLocally: false,
    hasPreview: true,
    hasBlockingConflicts: false,
    taskActive: false,
    listenerReady: true,
  };
  assert.equal(canStartInitialRetargetInstall(safe), true);

  for (const installStatus of [
    "installed",
    "committed_cleanup_pending",
    "cleanup_pending",
    "rollback_required",
    "repair_required",
    "unknown",
    undefined,
  ]) {
    assert.equal(canStartInitialRetargetInstall({ ...safe, installStatus }), false);
  }
  assert.equal(canStartInitialRetargetInstall({ ...safe, hasBlockingConflicts: true }), false);
  assert.equal(canStartInitialRetargetInstall({ ...safe, listenerReady: false }), false);
  assert.equal(canStartInitialRetargetInstall({ ...safe, completedLocally: true }), false);
});

test("retarget task state consumes only matching install retarget phases", () => {
  for (const phase of [
    "install.retarget.queued",
    "install.retarget.plan.building",
    "install.retarget.commit.processing",
    "install.retarget.completed",
    "install.retarget.failed",
  ]) {
    assert.equal(isRetargetInstallTaskPhase(phase), true);
  }
  assert.equal(isRetargetInstallTaskPhase("install.completed"), false);

  const current = {
    status: "running",
    taskId: "task-a",
    phase: "install.retarget.plan.building",
  };
  const otherTask = nextRetargetInstallTaskState(current, {
    taskId: "task-b",
    kind: "install",
    status: "completed",
    phase: "install.retarget.completed",
  });
  assert.equal(otherTask, current);

  assert.deepEqual(
    nextRetargetInstallTaskState(current, {
      taskId: "task-a",
      kind: "install",
      status: "completed",
      phase: "install.retarget.completed",
    }),
    { status: "completed", taskId: "task-a", phase: "install.retarget.completed" },
  );
});
