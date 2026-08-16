import assert from "node:assert/strict";
import { test } from "node:test";

import {
  getProfileSaveBackupTaskErrorCode,
  getProfileSaveBackupTaskErrorMessage,
  getProfileSaveBackupTaskPhaseLabel,
  isProfileSaveBackupTaskPhase,
  nextProfileSaveBackupTaskStateFromProgress,
  shouldRefreshProfileSaveBackupHistory,
} from "./profileSaveBackupTaskState.ts";

test("profile save backup task phases map to user-facing labels", () => {
  assert.equal(isProfileSaveBackupTaskPhase("save_backup.queued"), true);
  assert.equal(isProfileSaveBackupTaskPhase("save_backup.scanning"), true);
  assert.equal(isProfileSaveBackupTaskPhase("save_backup.archiving"), true);
  assert.equal(isProfileSaveBackupTaskPhase("save_backup.manifest_writing"), true);
  assert.equal(isProfileSaveBackupTaskPhase("save_backup.retention_pruning"), true);
  assert.equal(isProfileSaveBackupTaskPhase("save_backup.completed"), true);
  assert.equal(isProfileSaveBackupTaskPhase("save_backup.failed"), true);
  assert.equal(isProfileSaveBackupTaskPhase("save_backup.cancelled"), true);
  assert.equal(isProfileSaveBackupTaskPhase("install.completed"), false);
  assert.equal(getProfileSaveBackupTaskPhaseLabel("save_backup.manifest_writing"), "写入备份清单");
});

test("profile save backup progress ignores unrelated task ids and task kinds", () => {
  const current = {
    status: "running",
    taskId: "save-backup-a",
    phase: "save_backup.scanning",
  };

  assert.equal(
    nextProfileSaveBackupTaskStateFromProgress(current, {
      taskId: "save-backup-b",
      kind: "save_backup",
      status: "running",
      phase: "save_backup.archiving",
      current: null,
      total: null,
      message: null,
      error: null,
      resultRef: null,
    }),
    current,
  );

  assert.equal(
    nextProfileSaveBackupTaskStateFromProgress(current, {
      taskId: "install-a",
      kind: "install",
      status: "completed",
      phase: "install.completed",
      current: null,
      total: null,
      message: null,
      error: null,
      resultRef: null,
    }),
    current,
  );
});

test("profile save backup completed and failed progress map to stable UI states", () => {
  const current = {
    status: "running",
    taskId: "save-backup-a",
    phase: "save_backup.archiving",
  };

  const completed = nextProfileSaveBackupTaskStateFromProgress(current, {
    taskId: "save-backup-a",
    kind: "save_backup",
    status: "completed",
    phase: "save_backup.completed",
    current: 1,
    total: 1,
    message: null,
    error: null,
    resultRef: "backup-1",
  });

  assert.deepEqual(completed, {
    status: "completed",
    taskId: "save-backup-a",
    phase: "save_backup.completed",
    resultRef: "backup-1",
  });
  assert.equal(shouldRefreshProfileSaveBackupHistory(completed), true);

  assert.deepEqual(
    nextProfileSaveBackupTaskStateFromProgress(current, {
      taskId: "save-backup-a",
      kind: "save_backup",
      status: "failed",
      phase: "save_backup.failed",
      current: null,
      total: null,
      message: "save_backup_source_unset",
      error: "save_backup_source_unset",
      resultRef: null,
    }),
    {
      status: "failed",
      taskId: "save-backup-a",
      phase: "save_backup.failed",
      errorCode: "save_backup_source_unset",
      message: "当前配置档尚未设置存档目录。",
    },
  );
});

test("profile save backup failures map stable codes without exposing raw backend text", () => {
  assert.equal(
    getProfileSaveBackupTaskErrorCode("save_backup_failed:write_admission_busy"),
    "write_admission_busy",
  );
  assert.equal(
    getProfileSaveBackupTaskErrorMessage("save_backup_failed:write_admission_busy"),
    "另一项存档操作正在进行，请稍后再试。",
  );
  assert.equal(
    getProfileSaveBackupTaskErrorMessage("save_backup_failed:future_internal_error"),
    "存档备份失败，请稍后重试。",
  );
  assert.equal(getProfileSaveBackupTaskErrorCode("raw backend failure with spaces"), null);
});
