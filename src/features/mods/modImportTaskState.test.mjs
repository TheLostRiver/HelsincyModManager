import assert from "node:assert/strict";
import { test } from "node:test";

import {
  consumeReconnectImportRequest,
  getModImportTaskPhaseLabel,
  isModImportTaskPhase,
  nextModImportTaskStateFromProgress,
} from "./modImportTaskState.ts";

test("reconnect requests are consumed exactly once after the listener is ready", () => {
  let requested = true;
  let startCount = 0;
  const shouldStartResults = [];
  const nextRequestedResults = [];

  for (const listenerStatus of ["loading", "ready", "ready"]) {
    const result = consumeReconnectImportRequest(listenerStatus, requested);
    shouldStartResults.push(result.shouldStart);
    nextRequestedResults.push(result.nextRequested);
    requested = result.nextRequested;
    if (result.shouldStart) startCount += 1;
  }

  assert.deepEqual(shouldStartResults, [false, true, false]);
  assert.deepEqual(nextRequestedResults, [true, false, false]);
  assert.equal(startCount, 1);
  assert.equal(requested, false);
});

function progress(overrides = {}) {
  return {
    taskId: "task-a",
    kind: "mod_import",
    status: "running",
    phase: "mod_import.unpack.started",
    current: null,
    total: null,
    message: null,
    error: null,
    resultRef: null,
    ...overrides,
  };
}

test("mod import phases use registered user-facing labels", () => {
  assert.equal(isModImportTaskPhase("mod_import.unpack.started"), true);
  assert.equal(isModImportTaskPhase("mod_import.prepare.completed"), true);
  assert.equal(isModImportTaskPhase("install.queued"), false);
  assert.equal(getModImportTaskPhaseLabel("mod_import.preview_image.processing"), "正在处理预览图");
});

test("mod import progress only updates the matching task identity", () => {
  const current = {
    status: "running",
    taskId: "task-a",
    phase: "mod_import.queued",
  };

  assert.equal(
    nextModImportTaskStateFromProgress(current, progress({ taskId: "task-b" })),
    current,
  );
  assert.equal(
    nextModImportTaskStateFromProgress(current, progress({ kind: "install" })),
    current,
  );
  assert.equal(
    nextModImportTaskStateFromProgress(
      current,
      progress({ phase: "mod_import.unregistered.future_phase" }),
    ),
    current,
  );
});

test("mod import terminal events map to stable safe states", () => {
  const current = {
    status: "running",
    taskId: "task-a",
    phase: "mod_import.unpack.started",
  };

  assert.deepEqual(
    nextModImportTaskStateFromProgress(
      current,
      progress({ status: "completed", phase: "mod_import.prepare.completed" }),
    ),
    {
      status: "completed",
      taskId: "task-a",
      phase: "mod_import.prepare.completed",
    },
  );

  const failed = nextModImportTaskStateFromProgress(
    current,
    progress({
      status: "failed",
      phase: "mod_import.unpack.failed",
      error: "C:\\Users\\private\\unsafe.zip",
    }),
  );
  assert.deepEqual(failed, {
    status: "failed",
    taskId: "task-a",
    phase: "mod_import.unpack.failed",
    message: "导入失败，请检查压缩包后重试",
  });
  assert.doesNotMatch(failed.message, /Users|unsafe\.zip/);
});

test("mod import terminal states ignore late progress events", () => {
  const terminalStates = [
    {
      status: "completed",
      taskId: "task-a",
      phase: "mod_import.prepare.completed",
    },
    {
      status: "cancelled",
      taskId: "task-a",
      phase: "mod_import.cancelled",
    },
    {
      status: "failed",
      taskId: "task-a",
      phase: "mod_import.unpack.failed",
      message: "导入失败，请检查压缩包后重试",
    },
  ];

  for (const current of terminalStates) {
    assert.equal(nextModImportTaskStateFromProgress(current, progress()), current);
  }
});
