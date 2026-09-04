import assert from "node:assert/strict";
import { test } from "node:test";

import {
  beginDirectorySave,
  completeDirectorySave,
  failDirectorySave,
} from "./gameSetupSaveState.ts";

/*
 * #333 的行为断言。
 *
 * 本文件刻意驱动完整迁移序列，而不是匹配源码文本。#333 第一版修复带着绿灯合并，
 * 就是因为它的测试用 readFileSync + 正则断言 hook 源码里「存在某段三元表达式」——
 * 那段表达式确实存在，可它读的 current.status 早已被同一函数开头的乐观转场写成
 * validating，判断恒假。源码断言看不见运行时状态，行为断言才看得见。
 */

const gameId = "mhw";

const configured = Object.freeze({
  kind: "configured",
  gameId,
  displayName: "Monster Hunter: World - Iceborne",
  pathLabel: "C:\\Games\\MonsterHunterWorld",
});

const notConfigured = Object.freeze({ kind: "not_configured", gameId });

const invalid = Object.freeze({
  kind: "invalid",
  gameId,
  errorCode: "directory_not_found",
  backendMessage: null,
});

const missingExecutable = Object.freeze({
  code: "missing_executable",
  backendMessage: "directory validation failed",
});

/** 只带状态机关心的四个字段，外加一个 hook 独有字段用于验证透传不丢。 */
function sliceOf(status) {
  return {
    status,
    statusBeforeSave: null,
    lastSaveError: null,
    isBusy: false,
    candidates: ["sentinel"],
  };
}

test("a rejected save keeps the configured directory instead of reporting it invalid", () => {
  const saved = sliceOf(configured);

  const validating = beginDirectorySave(saved, gameId);
  const failed = failDirectorySave(validating, gameId, missingExecutable);

  /*
   * 缺陷本体：后端在落盘之前就拒绝了，磁盘配置原封不动，UI 不能假装它丢了。
   * 掉成 invalid 会连带禁掉一键启动、恢复中心与安装健康检测，且必须重启才恢复。
   */
  assert.deepEqual(failed.status, configured);
  // 失败原因只走正交字段。
  assert.equal(failed.lastSaveError, "missing_executable");
  assert.equal(failed.isBusy, false);
});

test("a rejected save reports invalid when there was no configuration to protect", () => {
  for (const status of [notConfigured, invalid]) {
    const failed = failDirectorySave(
      beginDirectorySave(sliceOf(status), gameId),
      gameId,
      missingExecutable,
    );

    assert.equal(failed.status.kind, "invalid");
    // 报的是本次失败的原因，不是上一次残留的。
    assert.equal(failed.status.errorCode, "missing_executable");
    assert.equal(failed.status.backendMessage, "directory validation failed");
    assert.equal(failed.lastSaveError, "missing_executable");
  }
});

test("the optimistic validating state never leaks into the restore decision", () => {
  /*
   * 重入保护：validating 一旦覆盖进 statusBeforeSave，还原判据就跟第一版一样被擦掉。
   * UI 上三个按钮都 disabled={isBusy} 已挡住重入，但状态机不该依赖 UI 兜底。
   */
  const reentered = beginDirectorySave(
    beginDirectorySave(sliceOf(configured), gameId),
    gameId,
  );

  assert.deepEqual(reentered.statusBeforeSave, configured);
  assert.deepEqual(
    failDirectorySave(reentered, gameId, missingExecutable).status,
    configured,
  );
});

test("a successful save adopts the backend status and clears the previous failure", () => {
  const relocated = {
    kind: "configured",
    gameId,
    displayName: "Monster Hunter: World - Iceborne",
    pathLabel: "D:\\Games\\MonsterHunterWorld",
  };

  const failed = failDirectorySave(
    beginDirectorySave(sliceOf(configured), gameId),
    gameId,
    missingExecutable,
  );
  const saved = completeDirectorySave(beginDirectorySave(failed, gameId), relocated);

  assert.deepEqual(saved.status, relocated);
  // 成功即证明上次失败不再是现状。
  assert.equal(saved.lastSaveError, null);
  assert.equal(saved.isBusy, false);
});

test("saving shows the validating state while it is in flight", () => {
  // 锁住观感：validating 仍由 status 承载，顶部栏 / hero 卡 / 状态面板 / 步骤条不变。
  const validating = beginDirectorySave(sliceOf(configured), gameId);

  assert.equal(validating.status.kind, "validating");
  assert.equal(validating.isBusy, true);
});

test("statusBeforeSave is scratch space that never survives a settled save", () => {
  const cases = [
    failDirectorySave(beginDirectorySave(sliceOf(configured), gameId), gameId, missingExecutable),
    failDirectorySave(beginDirectorySave(sliceOf(notConfigured), gameId), gameId, missingExecutable),
    completeDirectorySave(beginDirectorySave(sliceOf(configured), gameId), configured),
  ];

  for (const settled of cases) {
    // 泄漏到下一次操作就会拿旧配置去还原一次无关的失败。
    assert.equal(settled.statusBeforeSave, null);
    // 顺带确认三个迁移都不吞 hook 独有字段。
    assert.deepEqual(settled.candidates, ["sentinel"]);
  }
});
