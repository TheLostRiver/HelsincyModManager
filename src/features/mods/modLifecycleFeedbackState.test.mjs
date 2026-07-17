import assert from "node:assert/strict";
import { test } from "node:test";

import {
  failClosedModInstallSummary,
  getManagedInstallTerminalToast,
  isManagedInstallTerminalRefreshCurrent,
  isManagedInstallTaskTerminal,
  shouldFailClosedManagedInstallTerminal,
} from "./modLifecycleFeedbackState.ts";

const completedInstall = {
  status: "completed",
  operation: "install",
  taskId: "task-install",
  profileId: "profile-a",
  modId: "mod-a",
  modName: "Example Mod",
  phase: "install.completed",
};

const failedUninstall = {
  status: "failed",
  operation: "uninstall",
  taskId: "task-uninstall",
  profileId: "profile-a",
  modId: "mod-a",
  modName: "Example Mod",
  phase: "install.uninstall.failed",
  message: "卸载未完成，已重新检查安装状态",
};

test("managed install terminal states exclude starting and running tasks", () => {
  assert.equal(isManagedInstallTaskTerminal(completedInstall), true);
  assert.equal(isManagedInstallTaskTerminal(failedUninstall), true);
  assert.equal(
    isManagedInstallTaskTerminal({
      status: "running",
      operation: "install",
      taskId: "task-install",
      profileId: "profile-a",
      modId: "mod-a",
      modName: "Example Mod",
      phase: "install.commit.processing",
    }),
    false,
  );
});

test("success toast requires a verified durable status matching the completed operation", () => {
  assert.deepEqual(
    getManagedInstallTerminalToast(completedInstall, { verified: true, status: "installed" }),
    {
      id: "task-install",
      title: "安装完成",
      message: "Example Mod",
      tone: "success",
    },
  );
  assert.equal(
    getManagedInstallTerminalToast(completedInstall, { verified: false, status: "installed" }),
    null,
  );
  assert.equal(
    getManagedInstallTerminalToast(completedInstall, { verified: true, status: "not_installed" }),
    null,
  );
});

test("terminal refresh can publish only for the starting profile and unchanged library revision", () => {
  assert.equal(isManagedInstallTerminalRefreshCurrent(completedInstall, "profile-a", true), true);
  assert.equal(isManagedInstallTerminalRefreshCurrent(completedInstall, "profile-b", true), false);
  assert.equal(isManagedInstallTerminalRefreshCurrent(completedInstall, "profile-a", false), false);
  assert.equal(isManagedInstallTerminalRefreshCurrent(completedInstall, null, true), false);
});

test("ordinary failure toast is suppressed for every persistent recovery state", () => {
  for (const status of [
    "committed_cleanup_pending",
    "cleanup_pending",
    "rollback_required",
    "repair_required",
    "unknown",
  ]) {
    assert.equal(
      getManagedInstallTerminalToast(failedUninstall, { verified: true, status }),
      null,
      status,
    );
  }

  assert.deepEqual(
    getManagedInstallTerminalToast(failedUninstall, { verified: true, status: "installed" }),
    {
      id: "task-uninstall",
      title: "卸载失败",
      message: "卸载未完成，已重新检查安装状态",
      tone: "danger",
    },
  );
});

test("cancelled install only becomes a toast after not-installed is verified", () => {
  const cancelled = {
    status: "cancelled",
    operation: "install",
    taskId: "task-cancelled",
    profileId: "profile-a",
    modId: "mod-a",
    modName: "Example Mod",
    phase: "install.cancelled",
  };

  assert.equal(
    getManagedInstallTerminalToast(cancelled, { verified: true, status: "installed" }),
    null,
  );
  assert.deepEqual(
    getManagedInstallTerminalToast(cancelled, { verified: true, status: "not_installed" }),
    {
      id: "task-cancelled",
      title: "安装已取消",
      message: "Example Mod",
      tone: "neutral",
    },
  );
});

test("unverified and contradictory terminal facts fail closed to a persistent unknown summary", () => {
  assert.equal(
    shouldFailClosedManagedInstallTerminal(completedInstall, { verified: false, status: null }),
    true,
  );
  assert.equal(
    shouldFailClosedManagedInstallTerminal(completedInstall, { verified: true, status: "not_installed" }),
    true,
  );
  assert.equal(
    shouldFailClosedManagedInstallTerminal(completedInstall, { verified: true, status: "rollback_required" }),
    false,
  );

  const items = failClosedModInstallSummary(
    [
      {
        id: "mod-a",
        name: "Example Mod",
        status: "not_installed",
        sizeLabel: "1 KB",
        categoryLabels: [],
      },
      {
        id: "mod-b",
        name: "Other Mod",
        status: "not_installed",
        sizeLabel: "1 KB",
        categoryLabels: [],
      },
    ],
    "mod-a",
  );

  assert.equal(items[0].status, "unknown");
  assert.equal(items[0].installSummary.status, "unknown");
  assert.equal(items[1].status, "not_installed");
});
