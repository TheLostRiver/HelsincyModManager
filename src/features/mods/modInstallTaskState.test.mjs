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

test("ambiguous content root failure is not flattened into the generic planning message", () => {
  // #284 R1 给合集包单独划了错误码，但安装任务层原本用 `Err(_)` 把规划失败一律压成
  // `planning`，玩家走右键菜单直接安装时只会看到「无法生成安装计划」，会以为包坏了。
  // 这条用例锁住「新 phase 有自己的文案」这一事实。
  assert.equal(
    getManagedInstallTaskFailureMessage(
      "install",
      "install_failed:ambiguous_content_root",
      zhInstallTask,
    ),
    "包内有多个 nativePC 目录，请拆分后分别导入",
  );

  // 防退化：分类要有区分度。若后端把所有规划失败都升级成新码，或前端把它写回
  // planning，这条就会红——那说明分类等于没做。
  assert.equal(
    getManagedInstallTaskFailureMessage("install", "install_failed:planning", zhInstallTask),
    "无法生成安装计划",
  );
});

test("admission and prerequisite failure phases have their own messages", () => {
  // #284 R5 时发现：phase 有 15 种，而 `installFailures` 只有 8 个 key。
  // 缺 key 时 `getManagedInstallTaskFailureMessage` 会静默回落到 `installFailedDefault`，
  // 于是玩家只看到「安装失败」——后端单测与三语 key 检查都全绿，只有真机才看得见。
  // 这条用例锁住本次补齐的 4 个 phase。**新增 phase 时请同步在这里登记**，
  // 否则又会退回到「代码对了、玩家看不懂」。
  const expected = {
    prerequisite: "前置环境检查未通过或已变化，安装已阻止，请到「前置环境」重新检查",
    write_safety_rejected: "当前配置不允许写入该游戏目录，安装已阻止",
    write_admission_busy: "另一项操作正在使用该游戏目录，请稍后重试",
    replacement_selection_pending: "替换目标尚未选择完成，请回到「替换目标」面板继续安装",
  };

  for (const [phase, message] of Object.entries(expected)) {
    assert.equal(
      getManagedInstallTaskFailureMessage("install", `install_failed:${phase}`, zhInstallTask),
      message,
      `${phase} 没有自己的文案，回落到默认提示了`,
    );
    assert.notEqual(
      message,
      zhInstallTask.installFailedDefault,
      `${phase} 的文案不能等于默认失败文案，否则等于没写`,
    );
  }

  // 防退化：分组 key 数应当与这里登记的 phase 一并增长。
  // 若有人加了 phase 却没补文案，这条会红。
  assert.ok(
    Object.keys(zhInstallTask.installFailures).length >= Object.keys(expected).length + 4,
    "installFailures 的 key 不应少于既有 4 条 + 本次补齐的 4 条",
  );
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
