import assert from "node:assert/strict";
import { test } from "node:test";

import {
  getManagedInstallTaskFailureMessage,
  getManagedInstallTaskPhaseLabel,
  isManagedInstallTaskPhase,
  nextManagedInstallTaskStateFromProgress,
} from "./modInstallTaskState.ts";
import { modLifecycleCopy } from "./modLifecycleCopy.ts";

const zhInstallTask = modLifecycleCopy.zh_cn.installTask;

test("managed install task phases include uninstall phases", () => {
  assert.equal(isManagedInstallTaskPhase("install.uninstall.queued"), true);
  assert.equal(isManagedInstallTaskPhase("install.uninstall.processing"), true);
  assert.equal(isManagedInstallTaskPhase("install.uninstall.completed"), true);
  assert.equal(isManagedInstallTaskPhase("install.uninstall.failed"), true);
  assert.equal(isManagedInstallTaskPhase("mod_import.prepare.completed"), false);
  assert.equal(getManagedInstallTaskPhaseLabel("install.uninstall.processing", zhInstallTask), "卸载中");
});

test("uninstall progress only updates the matching task id", () => {
  const current = {
    status: "running",
    operation: "uninstall",
    taskId: "task-a",
    profileId: "profile-a",
    modId: "mod-a",
    modName: "Mock Mod",
    phase: "install.uninstall.processing",
  };

  const next = nextManagedInstallTaskStateFromProgress(
    current,
    {
      taskId: "task-b",
      kind: "install",
      status: "completed",
      phase: "install.uninstall.completed",
    },
    zhInstallTask,
  );

  assert.equal(next, current);
});

test("uninstall completed and failed phases map to stable task states", () => {
  const current = {
    status: "running",
    operation: "uninstall",
    taskId: "task-a",
    profileId: "profile-a",
    modId: "mod-a",
    modName: "Mock Mod",
    phase: "install.uninstall.processing",
  };

  assert.deepEqual(
    nextManagedInstallTaskStateFromProgress(
      current,
      {
        taskId: "task-a",
        kind: "install",
        status: "completed",
        phase: "install.uninstall.completed",
      },
      zhInstallTask,
    ),
    {
      status: "completed",
      operation: "uninstall",
      taskId: "task-a",
      profileId: "profile-a",
      modId: "mod-a",
      modName: "Mock Mod",
      phase: "install.uninstall.completed",
    },
  );

  assert.deepEqual(
    nextManagedInstallTaskStateFromProgress(
      current,
      {
        taskId: "task-a",
        kind: "install",
        status: "failed",
        phase: "install.uninstall.failed",
        error: "install_uninstall_failed:uninstall",
      },
      zhInstallTask,
    ),
    {
      status: "failed",
      operation: "uninstall",
      taskId: "task-a",
      profileId: "profile-a",
      modId: "mod-a",
      modName: "Mock Mod",
      phase: "install.uninstall.failed",
      message: "卸载未完成，已重新检查安装状态",
    },
  );
});

test("managed install failures only expose mapped stable messages", () => {
  assert.equal(
    getManagedInstallTaskFailureMessage("install", "install_failed:commit", zhInstallTask),
    "安装未完成，已重新检查安装状态",
  );
  assert.equal(
    getManagedInstallTaskFailureMessage(
      "uninstall",
      "install_uninstall_failed:recovery_pending",
      zhInstallTask,
    ),
    "卸载被待处理的恢复状态阻断",
  );
  assert.equal(getManagedInstallTaskFailureMessage("install", "C:\\Users\\private\\raw-error", zhInstallTask), "安装失败");
  assert.equal(getManagedInstallTaskFailureMessage("uninstall", null, zhInstallTask), "卸载失败");
});
