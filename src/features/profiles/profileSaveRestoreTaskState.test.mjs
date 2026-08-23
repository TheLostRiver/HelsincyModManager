import assert from "node:assert/strict";
import { test } from "node:test";

import {
  ProfileSaveRestoreEarlyEventBuffer,
  attachProfileSaveRestoreTask,
  canCancelProfileSaveRestore,
  getProfileSaveRestoreErrorMessage,
  getProfileSaveRestoreWarningMessage,
  isProfileSaveRestoreProgressEvent,
  nextProfileSaveRestoreTaskStateFromProgress,
} from "./profileSaveRestoreTaskState.ts";
import { saveRestoreCopy } from "./saveRestoreCopy.ts";

// 功能测试固定使用 zh_cn 字典，断言中文值不回归。
const zhCopy = saveRestoreCopy.zh_cn;

function event({
  taskId = "save-restore-a",
  status = "running",
  phase = "save_restore.preparing",
  error = null,
  message = null,
} = {}) {
  return {
    taskId,
    kind: "save_restore",
    status,
    phase,
    current: null,
    total: null,
    message,
    error,
    resultRef: null,
  };
}

test("save restore progress requires exact kind, phase, status, and task id", () => {
  assert.equal(isProfileSaveRestoreProgressEvent(event()), true);
  assert.equal(isProfileSaveRestoreProgressEvent({ ...event(), kind: "save_backup" }), false);
  assert.equal(isProfileSaveRestoreProgressEvent(event({ status: "completed" })), false);
  assert.equal(isProfileSaveRestoreProgressEvent(event({ phase: "save_restore.unknown" })), false);

  const current = { status: "running", taskId: "save-restore-a", phase: "save_restore.preparing" };
  assert.equal(
    nextProfileSaveRestoreTaskStateFromProgress(current, event({ taskId: "save-restore-b" })),
    current,
  );
});

test("completed, failed, recovery-required, and cancelled events remain distinct", () => {
  const running = { status: "running", taskId: "save-restore-a", phase: "save_restore.committing" };
  assert.deepEqual(
    nextProfileSaveRestoreTaskStateFromProgress(running, event({
      status: "completed",
      phase: "save_restore.completed",
      error: "save_restore_evidence_degraded",
      message: "save_restore_recovery_cleanup_failed",
    })),
    {
      status: "completed",
      taskId: "save-restore-a",
      evidenceDegraded: true,
      warningCodes: ["save_restore_evidence_degraded", "save_restore_recovery_cleanup_failed"],
    },
  );

  assert.deepEqual(
    nextProfileSaveRestoreTaskStateFromProgress(running, event({
      status: "failed",
      phase: "save_restore.recovery_required",
      error: "save_restore_recovery_required",
    })),
    {
      status: "recovery_required",
      taskId: "save-restore-a",
      errorCode: "save_restore_recovery_required",
    },
  );
  // 语义/文本分离后 state 只存 errorCode，文本在渲染时取词。
  assert.equal(
    getProfileSaveRestoreErrorMessage("save_restore_recovery_required", zhCopy.errors),
    "恢复未能安全收敛，已保留恢复证据。",
  );

  const unstableFailed = nextProfileSaveRestoreTaskStateFromProgress(running, event({
    status: "failed",
    phase: "save_restore.failed",
    error: "not a stable code with spaces",
    message: "raw backend text",
  }));
  assert.equal(unstableFailed.errorCode, null);
  assert.equal(
    getProfileSaveRestoreErrorMessage(unstableFailed.errorCode, zhCopy.errors),
    "存档恢复失败，当前存档未被视为成功恢复。",
  );

  assert.deepEqual(
    nextProfileSaveRestoreTaskStateFromProgress(running, event({
      status: "failed",
      phase: "save_restore.failed",
      error: "save_restore_rolled_back",
      message: "save_restore_recovery_cleanup_failed",
    })),
    {
      status: "failed",
      taskId: "save-restore-a",
      errorCode: "save_restore_rolled_back",
      warningCodes: ["save_restore_recovery_cleanup_failed"],
    },
  );
  assert.equal(
    getProfileSaveRestoreErrorMessage("save_restore_rolled_back", zhCopy.errors),
    "恢复未完成，已自动恢复到操作前存档。",
  );

  assert.deepEqual(
    nextProfileSaveRestoreTaskStateFromProgress(running, event({
      status: "cancelled",
      phase: "save_restore.cancelled",
    })),
    { status: "cancelled", taskId: "save-restore-a" },
  );
});

test("early event buffer is task-scoped, ordered, and bounded", () => {
  const buffer = new ProfileSaveRestoreEarlyEventBuffer(2, 2);
  buffer.push(event({ taskId: "task-a", phase: "save_restore.preparing" }));
  buffer.push(event({ taskId: "task-a", phase: "save_restore.revalidating" }));
  buffer.push(event({ taskId: "task-a", phase: "save_restore.pre_restore_backup" }));
  buffer.push(event({ taskId: "task-b", phase: "save_restore.preparing" }));
  buffer.push(event({ taskId: "task-c", phase: "save_restore.preparing" }));

  assert.deepEqual(buffer.take("task-a"), []);
  assert.deepEqual(buffer.take("task-b").map((item) => item.phase), ["save_restore.preparing"]);
  assert.deepEqual(buffer.take("task-c").map((item) => item.phase), ["save_restore.preparing"]);

  buffer.push(event({ taskId: "task-d", phase: "save_restore.preparing" }));
  buffer.push(event({ taskId: "task-d", phase: "save_restore.revalidating" }));
  buffer.push(event({ taskId: "task-d", phase: "save_restore.pre_restore_backup" }));
  assert.deepEqual(
    buffer.take("task-d").map((item) => item.phase),
    ["save_restore.revalidating", "save_restore.pre_restore_backup"],
  );
});

test("attaching a task replays early terminal events and commit cannot be cancelled", () => {
  const attached = attachProfileSaveRestoreTask("save-restore-a", [
    event({ taskId: "other-task", phase: "save_restore.preparing" }),
    event({ status: "completed", phase: "save_restore.completed" }),
  ]);
  assert.deepEqual(attached, {
    status: "completed",
    taskId: "save-restore-a",
    evidenceDegraded: false,
    warningCodes: [],
  });
  assert.equal(
    canCancelProfileSaveRestore({
      status: "running",
      taskId: "save-restore-a",
      phase: "save_restore.committing",
    }),
    false,
  );
  assert.equal(
    getProfileSaveRestoreErrorMessage({ code: "save_restore_game_running" }, zhCopy.errors),
    "游戏仍在运行，请完全退出游戏后重试。",
  );
  assert.equal(
    getProfileSaveRestoreWarningMessage("save_restore_unknown_warning", zhCopy.warnings),
    "恢复收尾证据需要检查，请保留现场并联系支持。",
  );
});

test("terminal states absorb late running events for the same task", () => {
  const terminalStates = [
    {
      status: "completed",
      taskId: "save-restore-a",
      evidenceDegraded: false,
      warningCodes: [],
    },
    {
      status: "failed",
      taskId: "save-restore-a",
      errorCode: "save_restore_commit_failed",
      warningCodes: [],
    },
    {
      status: "recovery_required",
      taskId: "save-restore-a",
      errorCode: "save_restore_recovery_required",
    },
    { status: "cancelled", taskId: "save-restore-a" },
  ];

  for (const terminal of terminalStates) {
    assert.equal(
      nextProfileSaveRestoreTaskStateFromProgress(
        terminal,
        event({ phase: "save_restore.revalidating" }),
      ),
      terminal,
    );
  }
});

test("recovery-required overrides an optimistic cancelled event", () => {
  const cancelled = { status: "cancelled", taskId: "save-restore-a" };
  assert.deepEqual(
    nextProfileSaveRestoreTaskStateFromProgress(cancelled, event({
      status: "failed",
      phase: "save_restore.recovery_required",
      error: "save_restore_transaction_unavailable",
    })),
    {
      status: "recovery_required",
      taskId: "save-restore-a",
      errorCode: "save_restore_transaction_unavailable",
    },
  );
  assert.equal(
    getProfileSaveRestoreErrorMessage("save_restore_transaction_unavailable", zhCopy.errors),
    "无法持久化恢复事务，恢复已安全停止。",
  );
});
