import assert from "node:assert/strict";
import { test } from "node:test";

import {
  canStartInitialRetargetInstall,
  isRetargetInstallTaskPhase,
  nextRetargetInstallTaskState,
  refreshRetargetInstallState,
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
    "install.cancelled",
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

  const cancelled = nextRetargetInstallTaskState(current, {
    taskId: "task-a",
    kind: "install",
    status: "cancelled",
    phase: "install.cancelled",
  });
  assert.deepEqual(
    cancelled,
    { status: "cancelled", taskId: "task-a", phase: "install.cancelled" },
  );
  assert.equal(
    nextRetargetInstallTaskState(cancelled, {
      taskId: "task-a",
      kind: "install",
      status: "running",
      phase: "install.retarget.commit.processing",
    }),
    cancelled,
  );

  assert.deepEqual(
    nextRetargetInstallTaskState(current, {
      taskId: "task-b",
      kind: "install",
      status: "cancelled",
      phase: "install.cancelled",
    }),
    current,
  );
});

test("completed retarget install keeps success while durable refresh can be retried", async () => {
  let attempts = 0;
  const failed = await refreshRetargetInstallState(async () => {
    attempts += 1;
    throw new Error("refresh unavailable");
  });
  assert.deepEqual(failed, {
    status: "failed",
    message: "安装已完成，但状态刷新失败，请重试。",
  });

  const ready = await refreshRetargetInstallState(async () => {
    attempts += 1;
  });
  assert.deepEqual(ready, { status: "ready" });
  assert.equal(attempts, 2);
});
