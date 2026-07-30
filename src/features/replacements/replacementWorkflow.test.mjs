import assert from "node:assert/strict";
import { test } from "node:test";

import {
  canCancelRetargetInstallTaskPhase,
  canStartRetargetReinstall,
  canStartInitialRetargetInstall,
  isRetargetInstallTaskPhase,
  isCurrentInstalledReplacementTarget,
  nextRetargetInstallTaskState,
  refreshRetargetInstallState,
  resolveInstalledReplacementTargetSelection,
} from "./replacementWorkflow.ts";

test("installed replacement target is restored as context, not an executable switch candidate", () => {
  const targets = [
    { id: "mhw:armor:fatalis-alpha" },
    { id: "mhw:armor:fatalis-beta" },
  ];

  assert.equal(
    resolveInstalledReplacementTargetSelection(targets, "mhw:armor:fatalis-beta"),
    "mhw:armor:fatalis-beta",
  );
  assert.equal(
    resolveInstalledReplacementTargetSelection(targets, "mhw:armor:missing"),
    null,
  );
  assert.equal(resolveInstalledReplacementTargetSelection(targets, undefined), null);
  assert.equal(
    isCurrentInstalledReplacementTarget(
      "mhw:armor:fatalis-beta",
      "mhw:armor:fatalis-beta",
    ),
    true,
  );
  assert.equal(
    isCurrentInstalledReplacementTarget(
      "mhw:armor:fatalis-alpha",
      "mhw:armor:fatalis-beta",
    ),
    false,
  );
});

test("retarget cancellation is offered only before the commit barrier", () => {
  for (const phase of [
    "install.retarget.queued",
    "install.retarget.plan.building",
    "install.reinstall.queued",
    "install.reinstall.plan.building",
    "install.reinstall.preflight.processing",
  ]) {
    assert.equal(canCancelRetargetInstallTaskPhase(phase), true);
  }
  for (const phase of [
    "install.retarget.commit.processing",
    "install.reinstall.commit.processing",
    "install.reinstall.rollback.processing",
  ]) {
    assert.equal(canCancelRetargetInstallTaskPhase(phase), false);
  }
});

test("installed retarget switch requires a ready preview, idle task, and listener", () => {
  const ready = {
    installStatus: "installed",
    previewStatus: "ready",
    taskActive: false,
    listenerReady: true,
  };
  assert.equal(canStartRetargetReinstall(ready), true);
  assert.equal(canStartRetargetReinstall({ ...ready, installStatus: "not_installed" }), false);
  assert.equal(canStartRetargetReinstall({ ...ready, previewStatus: "blocked" }), false);
  assert.equal(canStartRetargetReinstall({ ...ready, taskActive: true }), false);
  assert.equal(canStartRetargetReinstall({ ...ready, listenerReady: false }), false);
});

test("initial retarget install is enabled only for a safe preview and not-installed state", () => {
  const safe = {
    installStatus: "not_installed",
    completedLocally: false,
    hasPreview: true,
    hasBlockingConflicts: false,
    prerequisiteStatus: "ready",
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
  assert.equal(
    canStartInitialRetargetInstall({ ...safe, prerequisiteStatus: "blocked" }),
    false,
  );
  assert.equal(
    canStartInitialRetargetInstall({ ...safe, prerequisiteStatus: "warning" }),
    true,
  );
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
  for (const phase of [
    "install.reinstall.queued",
    "install.reinstall.plan.building",
    "install.reinstall.preflight.processing",
    "install.reinstall.commit.processing",
    "install.reinstall.rollback.processing",
    "install.reinstall.completed",
    "install.reinstall.failed",
    "install.reinstall.cancelled",
  ]) {
    assert.equal(isRetargetInstallTaskPhase(phase), true);
  }

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

test("retarget switch reducer ignores other task ids and accepts terminal reinstall phases", () => {
  const current = {
    status: "running",
    taskId: "switch-a",
    phase: "install.reinstall.commit.processing",
  };
  const otherTask = nextRetargetInstallTaskState(current, {
    taskId: "switch-b",
    kind: "install",
    status: "completed",
    phase: "install.reinstall.completed",
  });
  assert.equal(otherTask, current);

  assert.deepEqual(
    nextRetargetInstallTaskState(current, {
      taskId: "switch-a",
      kind: "install",
      status: "completed",
      phase: "install.reinstall.completed",
    }),
    { status: "completed", taskId: "switch-a", phase: "install.reinstall.completed" },
  );

  assert.deepEqual(
    nextRetargetInstallTaskState(current, {
      taskId: "switch-a",
      kind: "install",
      status: "failed",
      phase: "install.reinstall.failed",
      error: "install_reinstall_failed:manifest",
      message: "C:/Users/private/game/nativePC/file",
    }),
    {
      status: "failed",
      taskId: "switch-a",
      phase: "install.reinstall.failed",
      message: "目标切换失败，请刷新状态并重新生成预览",
    },
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
