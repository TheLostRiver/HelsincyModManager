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

test("install failure messages keep the same keys across every locale", () => {
  // #285 加 `empty_plan` 时发现的缺口：`installFailures` / `uninstallFailures` 的类型是
  // `Record<string, string>`，于是 `satisfies LocaleDictionary<>` **无法**保证三语 key
  // 集合一致——实测删掉 ja 的 key 后 `tsc --noEmit` 照样通过，i18n 测试也不检查这一层。
  // 这条用例补上该检查，免得以后加文案时漏掉某个语言。
  for (const group of ["installFailures", "uninstallFailures"]) {
    const [zh, en, ja] = ["zh_cn", "en", "ja"].map((locale) =>
      Object.keys(modLifecycleCopy[locale].installTask[group]).sort(),
    );

    assert.ok(zh.length > 0, `${group} 至少应有一个 key`);
    assert.deepEqual(en, zh, `${group}: en 与 zh_cn 的 key 不一致`);
    assert.deepEqual(ja, zh, `${group}: ja 与 zh_cn 的 key 不一致`);
  }
});
