import assert from "node:assert/strict";
import { test } from "node:test";

import {
  canConfirmReinstall,
  canPreviewReinstall,
  getReinstallBlockingReasonLabel,
  getReinstallPreviewErrorMessage,
  getReinstallStartErrorMessage,
  getReinstallTaskPhaseLabel,
  isReinstallTaskPhase,
  nextReinstallTaskStateFromProgress,
  refreshReinstallDurableFacts,
} from "./modReinstallTaskState.ts";
import { modReinstallCopy } from "./modReinstallCopy.ts";

const zhTask = modReinstallCopy.zh_cn.task;

function progress(overrides = {}) {
  return {
    taskId: "task-a",
    kind: "install",
    status: "running",
    phase: "install.reinstall.plan.building",
    current: null,
    total: null,
    message: null,
    error: null,
    resultRef: null,
    ...overrides,
  };
}

const running = {
  status: "running",
  taskId: "task-a",
  modId: "mod-a",
  modName: "Alpha",
  candidateRevisionId: "revision-v2",
  phase: "install.reinstall.queued",
};

const readyPreview = {
  status: "ready",
  prerequisiteDecision: {
    status: "ready",
    rulesVersion: 1,
    codes: [],
  },
  planToken: "opaque-token",
  installedRevision: { revisionId: "revision-v1" },
  candidateRevision: { revisionId: "revision-v2" },
  counts: { retained: 1, replaced: 2, added: 3, stale: 4 },
  blockingReasons: [],
};

test("reinstall task phases accept only the registered reinstall namespace", () => {
  assert.equal(isReinstallTaskPhase("install.reinstall.queued"), true);
  assert.equal(isReinstallTaskPhase("install.reinstall.preflight.processing"), true);
  assert.equal(isReinstallTaskPhase("install.reinstall.rollback.processing"), true);
  assert.equal(isReinstallTaskPhase("install.reinstall.completed"), true);
  assert.equal(isReinstallTaskPhase("install.queued"), false);
  assert.equal(isReinstallTaskPhase("install.uninstall.processing"), false);
  assert.equal(getReinstallTaskPhaseLabel("install.reinstall.commit.processing", zhTask), "提交新版本");
});

test("reinstall progress requires matching task id, kind, and phase", () => {
  assert.equal(nextReinstallTaskStateFromProgress(running, progress({ taskId: "task-b" }), zhTask), running);
  assert.equal(nextReinstallTaskStateFromProgress(running, progress({ kind: "mod_import" }), zhTask), running);
  assert.equal(
    nextReinstallTaskStateFromProgress(running, progress({ phase: "install.completed" }), zhTask),
    running,
  );
});

test("reinstall terminal states are stable and never expose raw backend error text", () => {
  assert.deepEqual(
    nextReinstallTaskStateFromProgress(
      running,
      progress({ status: "completed", phase: "install.reinstall.completed" }),
      zhTask,
    ),
    {
      ...running,
      status: "completed",
      phase: "install.reinstall.completed",
    },
  );

  const failed = nextReinstallTaskStateFromProgress(
    running,
    progress({
      status: "failed",
      phase: "install.reinstall.failed",
      error: "install_reinstall_failed:post_commit",
      message: "C:\\Users\\private\\unsafe.zip",
    }),
    zhTask,
  );
  assert.equal(failed.status, "failed");
  assert.equal(failed.failurePhase, "post_commit");
  assert.match(failed.message, /新版本已提交/);
  assert.doesNotMatch(failed.message, /Users|unsafe\.zip/);

  assert.deepEqual(
    nextReinstallTaskStateFromProgress(
      running,
      progress({ status: "cancelled", phase: "install.reinstall.cancelled" }),
      zhTask,
    ),
    {
      ...running,
      status: "cancelled",
      phase: "install.reinstall.cancelled",
    },
  );
});

test("only installed plus ready preview and an inactive task can confirm", () => {
  assert.equal(canConfirmReinstall("installed", readyPreview, { status: "idle" }), true);
  assert.equal(canConfirmReinstall("committed_cleanup_pending", readyPreview, { status: "idle" }), false);
  assert.equal(canConfirmReinstall("cleanup_pending", readyPreview, { status: "idle" }), false);
  assert.equal(canConfirmReinstall("rollback_required", readyPreview, { status: "idle" }), false);
  assert.equal(canConfirmReinstall("repair_required", readyPreview, { status: "idle" }), false);
  assert.equal(canConfirmReinstall("unknown", readyPreview, { status: "idle" }), false);
  assert.equal(
    canConfirmReinstall(
      "installed",
      { ...readyPreview, status: "blocked", planToken: null, candidateRevision: null },
      { status: "idle" },
    ),
    false,
  );
  assert.equal(
    canConfirmReinstall(
      "installed",
      {
        ...readyPreview,
        prerequisiteDecision: {
          status: "warning",
          rulesVersion: 1,
          codes: ["signature_unverified"],
        },
      },
      { status: "idle" },
    ),
    true,
  );
  assert.equal(
    canConfirmReinstall(
      "installed",
      {
        ...readyPreview,
        prerequisiteDecision: {
          status: "blocked",
          rulesVersion: 1,
          codes: ["missing_required_file"],
        },
      },
      { status: "idle" },
    ),
    false,
  );
  assert.equal(canConfirmReinstall("installed", readyPreview, running), false);
});

test("preview is fail-closed for unsafe durable states and active tasks", () => {
  assert.equal(canPreviewReinstall("installed", "revision-v2", { status: "idle" }), true);
  assert.equal(canPreviewReinstall("installed", "", { status: "idle" }), false);
  assert.equal(canPreviewReinstall("committed_cleanup_pending", "revision-v2", { status: "idle" }), false);
  assert.equal(canPreviewReinstall("cleanup_pending", "revision-v2", { status: "idle" }), false);
  assert.equal(canPreviewReinstall("rollback_required", "revision-v2", { status: "idle" }), false);
  assert.equal(canPreviewReinstall("repair_required", "revision-v2", { status: "idle" }), false);
  assert.equal(canPreviewReinstall("unknown", "revision-v2", { status: "idle" }), false);
  assert.equal(canPreviewReinstall("installed", "revision-v2", running), false);
});

test("blocking reasons use stable codes, including null-candidate and stale preview cases", () => {
  assert.equal(getReinstallBlockingReasonLabel("candidate_not_found", zhTask), "候选版本不存在");
  assert.equal(getReinstallBlockingReasonLabel("preview_stale", zhTask), "预览已过期，请重新生成");
});

test("command errors use the registered stable codes without exposing backend messages", () => {
  const unsafeMessage = "C:\\Users\\private\\unsafe.zip";
  const previewMessages = [
    getReinstallPreviewErrorMessage({ code: "game_id_invalid", message: unsafeMessage }, zhTask),
    getReinstallPreviewErrorMessage({ code: "profile_id_empty", message: unsafeMessage }, zhTask),
    getReinstallPreviewErrorMessage({ code: "mod_id_empty", message: unsafeMessage }, zhTask),
    getReinstallPreviewErrorMessage(
      { code: "candidate_revision_id_empty", message: unsafeMessage },
      zhTask,
    ),
    getReinstallPreviewErrorMessage({ code: "layer_name_empty", message: unsafeMessage }, zhTask),
  ];
  const expiredTokenMessage = getReinstallStartErrorMessage(
    {
      code: "plan_token_invalid",
      message: unsafeMessage,
    },
    zhTask,
  );

  assert.equal(previewMessages[0], "当前游戏不支持重装");
  for (const message of previewMessages.slice(1)) {
    assert.equal(message, "重装请求已失效，请重新选择");
  }
  assert.equal(expiredTokenMessage, "重装预览已失效，请重新生成");
  assert.doesNotMatch([...previewMessages, expiredTokenMessage].join(" "), /Users|unsafe\.zip/);
  assert.equal(
    getReinstallStartErrorMessage({ code: "reinstall_preview_token_invalid" }, zhTask),
    "无法启动重装任务，请重新生成预览后重试",
  );
});

test("terminal durable refresh attempts catalog and manifest facts independently", async () => {
  const calls = [];
  const result = await refreshReinstallDurableFacts({
    loadRevisions: async () => {
      calls.push("revisions");
      throw new Error("catalog unavailable");
    },
    loadInstallStatus: async () => {
      calls.push("manifest");
      return "committed_cleanup_pending";
    },
  });

  assert.deepEqual(calls.sort(), ["manifest", "revisions"]);
  assert.equal(result.revisions, null);
  assert.equal(result.installStatus, "committed_cleanup_pending");
  assert.equal(result.status, "partial");
});
