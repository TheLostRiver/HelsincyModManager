import assert from "node:assert/strict";
import { test } from "node:test";

import {
  getExternalImportScanErrorMessage,
  getExternalImportScanPhaseLabel,
  isExternalImportScanPhase,
  nextExternalImportScanTaskStateFromProgress,
} from "./externalImportScanState.ts";

function progress(overrides = {}) {
  return {
    taskId: "scan-task-a",
    kind: "mod_import",
    status: "running",
    phase: "external_import.scan.discovering",
    current: 1,
    total: 3,
    message: null,
    error: null,
    resultRef: "batch-a",
    ...overrides,
  };
}

test("external import scan phases use registered labels", () => {
  assert.equal(isExternalImportScanPhase("external_import.scan.fingerprinting"), true);
  assert.equal(isExternalImportScanPhase("mod_import.cancelled"), true);
  assert.equal(isExternalImportScanPhase("external_import.import.materializing"), false);
  assert.equal(getExternalImportScanPhaseLabel("external_import.scan.discovering"), "正在发现候选");
});

test("external import scan progress only accepts the owned task identity", () => {
  const current = {
    status: "running",
    taskId: "scan-task-a",
    phase: "external_import.scan.queued",
  };

  assert.equal(
    nextExternalImportScanTaskStateFromProgress(current, progress({ taskId: "scan-task-b" })),
    current,
  );
  assert.equal(
    nextExternalImportScanTaskStateFromProgress(current, progress({ kind: "install" })),
    current,
  );
});

test("external import scan terminal events remain redacted and fail closed", () => {
  const current = {
    status: "running",
    taskId: "scan-task-a",
    phase: "external_import.scan.fingerprinting",
  };
  const completed = nextExternalImportScanTaskStateFromProgress(
    current,
    progress({ status: "completed", phase: "external_import.scan.completed" }),
  );
  assert.deepEqual(completed, {
    status: "completed",
    taskId: "scan-task-a",
    phase: "external_import.scan.completed",
  });

  const failed = nextExternalImportScanTaskStateFromProgress(
    current,
    progress({
      status: "failed",
      phase: "external_import.scan.failed",
      error: "C:\\private\\untrusted-input",
    }),
  );
  assert.deepEqual(failed, {
    status: "failed",
    taskId: "scan-task-a",
    phase: "external_import.scan.failed",
    errorCode: "external_import_scan_failed",
  });
  assert.doesNotMatch(getExternalImportScanErrorMessage(failed.errorCode), /private|untrusted/i);
});

test("unknown scan phases and invalid terminal status fail closed", () => {
  const current = {
    status: "running",
    taskId: "scan-task-a",
    phase: "external_import.scan.queued",
  };

  assert.deepEqual(
    nextExternalImportScanTaskStateFromProgress(
      current,
      progress({ phase: "external_import.scan.future_phase" }),
    ),
    {
      status: "failed",
      taskId: "scan-task-a",
      phase: "external_import.scan.unrecognized",
      errorCode: "external_import_scan_failed",
    },
  );
  assert.deepEqual(
    nextExternalImportScanTaskStateFromProgress(
      current,
      progress({ status: "running", phase: "external_import.scan.completed" }),
    ),
    {
      status: "failed",
      taskId: "scan-task-a",
      phase: "external_import.scan.completed",
      errorCode: "external_import_scan_failed",
    },
  );
});

test("generic cancellation maps to the terminal scan state and ignores late events", () => {
  const current = {
    status: "running",
    taskId: "scan-task-a",
    phase: "external_import.scan.discovering",
  };
  const cancelled = nextExternalImportScanTaskStateFromProgress(
    current,
    progress({ status: "cancelled", phase: "mod_import.cancelled" }),
  );
  assert.deepEqual(cancelled, {
    status: "cancelled",
    taskId: "scan-task-a",
    phase: "mod_import.cancelled",
  });
  assert.equal(nextExternalImportScanTaskStateFromProgress(cancelled, progress()), cancelled);
});
