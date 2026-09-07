import assert from "node:assert/strict";
import { test } from "node:test";

import {
  archiveKeptCodeFrom,
  consumeReconnectImportRequest,
  failedMessageKindFrom,
  getModImportFailedMessage,
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

test("mod import phases use registered user-facing labels", async () => {
  const { modImportCopy } = await import("./modImportCopy.ts");
  assert.equal(isModImportTaskPhase("mod_import.unpack.started"), true);
  assert.equal(isModImportTaskPhase("mod_import.prepare.completed"), true);
  assert.equal(isModImportTaskPhase("install.queued"), false);
  assert.equal(
    getModImportTaskPhaseLabel("mod_import.preview_image.processing", modImportCopy.zh_cn.phases),
    "正在处理预览图",
  );
  assert.equal(
    getModImportTaskPhaseLabel("mod_import.future_phase", modImportCopy.zh_cn.phases),
    "正在导入",
  );
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
      archiveKept: null,
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
  // I18N-02 起失败态只带语义 kind，state 不携带任何文本——后端事件原文（含路径）结构上无处泄漏。
  assert.deepEqual(failed, {
    status: "failed",
    taskId: "task-a",
    phase: "mod_import.unpack.failed",
    messageKind: "retry-hint",
  });
  assert.equal("message" in failed, false);
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
      messageKind: "retry-hint",
    },
  ];

  for (const current of terminalStates) {
    assert.equal(nextModImportTaskStateFromProgress(current, progress()), current);
  }
});

test("a completed event carries the archive-kept degradation code only when it is a registered one", () => {
  const current = {
    status: "running",
    taskId: "task-a",
    phase: "mod_import.unpack.started",
  };

  const kept = nextModImportTaskStateFromProgress(
    current,
    progress({
      status: "completed",
      phase: "mod_import.prepare.completed",
      error: "mod_import_archive_kept_protected_location",
    }),
  );
  assert.deepEqual(kept, {
    status: "completed",
    taskId: "task-a",
    phase: "mod_import.prepare.completed",
    archiveKept: "mod_import_archive_kept_protected_location",
  });

  const leaked = nextModImportTaskStateFromProgress(
    current,
    progress({
      status: "completed",
      phase: "mod_import.prepare.completed",
      error: "C:\\Users\\private\\mod.zip",
    }),
  );
  assert.equal(leaked.status, "completed");
  assert.equal(leaked.archiveKept, null, "unknown strings are never treated as codes");
  assert.equal(archiveKeptCodeFrom("mod_import_archive_kept_changed"), "mod_import_archive_kept_changed");
  assert.equal(archiveKeptCodeFrom(null), null);
});

test("unpack failures carry their reason instead of collapsing into the retry hint", () => {
  const current = { status: "running", taskId: "task-a", phase: "mod_import.unpack.started" };

  const unsupported = nextModImportTaskStateFromProgress(
    current,
    progress({
      status: "failed",
      phase: "mod_import.unpack.failed",
      error: "mod_import_unsupported_archive_format",
    }),
  );
  assert.equal(unsupported.status, "failed");
  assert.equal(unsupported.messageKind, "unsupported-archive-format");

  const notAnArchive = nextModImportTaskStateFromProgress(
    current,
    progress({
      status: "failed",
      phase: "mod_import.unpack.failed",
      error: "mod_import_not_an_archive",
    }),
  );
  assert.equal(notAnArchive.messageKind, "not-an-archive");
});

test("a damaged archive stays in the retry hint tier and unknown codes fall back to it", () => {
  const current = { status: "running", taskId: "task-a", phase: "mod_import.unpack.started" };
  const failed = (error) =>
    nextModImportTaskStateFromProgress(
      current,
      progress({ status: "failed", phase: "mod_import.unpack.failed", error }),
    ).messageKind;

  // 硬边界：损坏的 zip 不能被新档位吃掉，否则只是把误导换了个方向。
  assert.equal(failed("mod_import_prepare_failed"), "retry-hint");
  // 前向兼容：后端将来新增码时，前端最差退回今天的行为，不显示空文案。
  assert.equal(failed("mod_import_some_future_code"), "retry-hint");
  assert.equal(failed(null), "retry-hint");
  assert.equal(failedMessageKindFrom("mod_import_unsupported_archive_format"), "unsupported-archive-format");
  assert.equal(failedMessageKindFrom(null), "retry-hint");
});

test("both new failure tiers have distinct, non-empty copy in all three languages", async () => {
  const { modImportCopy } = await import("./modImportCopy.ts");
  for (const locale of ["zh_cn", "en", "ja"]) {
    const copy = modImportCopy[locale];
    const unsupported = getModImportFailedMessage("unsupported-archive-format", copy);
    const notAnArchive = getModImportFailedMessage("not-an-archive", copy);
    const retryHint = getModImportFailedMessage("retry-hint", copy);

    for (const [kind, message] of [
      ["unsupported-archive-format", unsupported],
      ["not-an-archive", notAnArchive],
    ]) {
      assert.equal(typeof message, "string");
      assert.ok(message.trim().length > 0, `${locale}/${kind} must not be empty`);
    }
    // 三档必须互不相同——否则「说清楚」这件事等于没做。
    assert.notEqual(unsupported, retryHint, `${locale}: unsupported must differ from the retry hint`);
    assert.notEqual(notAnArchive, retryHint, `${locale}: not-an-archive must differ from the retry hint`);
    assert.notEqual(unsupported, notAnArchive, `${locale}: the two new tiers must differ`);
  }
});
