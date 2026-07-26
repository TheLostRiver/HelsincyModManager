import assert from "node:assert/strict";
import { test } from "node:test";

import {
  isExternalImportTaskTerminal,
  nextExternalImportTaskStateFromProgress,
} from "./externalImportProgressState.ts";

function event(overrides = {}) {
  return {
    taskId: "mod-import-1",
    kind: "mod_import",
    status: "running",
    phase: "external_import.import.materializing",
    current: 0,
    total: 3,
    message: null,
    error: null,
    resultRef: "batch-a",
    ...overrides,
  };
}

const running = {
  status: "running",
  taskId: "mod-import-1",
  phase: "external_import.import.queued",
  current: null,
  total: null,
};

test("import progress ignores foreign tasks and consumes only exact import phases", () => {
  assert.deepEqual(
    nextExternalImportTaskStateFromProgress(running, event({ taskId: "mod-import-2" })),
    running,
  );
  assert.deepEqual(
    nextExternalImportTaskStateFromProgress(running, event({ kind: "install" })),
    running,
  );

  const next = nextExternalImportTaskStateFromProgress(running, event());
  assert.deepEqual(next, {
    status: "running",
    taskId: "mod-import-1",
    phase: "external_import.import.materializing",
    current: 0,
    total: 3,
  });
});

test("generic cancellation waits for the external-import terminal event", () => {
  const cancelling = nextExternalImportTaskStateFromProgress(
    running,
    event({
      status: "cancelled",
      phase: "mod_import.cancelled",
      current: null,
      total: null,
    }),
  );
  assert.deepEqual(cancelling, {
    status: "cancelling",
    taskId: "mod-import-1",
    phase: "mod_import.cancelled",
  });
  assert.equal(isExternalImportTaskTerminal(cancelling), false);

  const cancelled = nextExternalImportTaskStateFromProgress(
    cancelling,
    event({
      status: "cancelled",
      phase: "external_import.import.cancelled",
      current: 0,
      total: 3,
    }),
  );
  assert.deepEqual(cancelled, {
    status: "cancelled",
    taskId: "mod-import-1",
    phase: "external_import.import.cancelled",
  });
  assert.equal(isExternalImportTaskTerminal(cancelled), true);
});

test("terminal counts remain aggregate hints and malformed progress fails closed", () => {
  const failed = nextExternalImportTaskStateFromProgress(
    running,
    event({
      status: "failed",
      phase: "external_import.import.failed",
      current: 0,
      total: 0,
      error: "external_import_catalog_unavailable",
    }),
  );
  assert.deepEqual(failed, {
    status: "failed",
    taskId: "mod-import-1",
    phase: "external_import.import.failed",
    errorCode: "external_import_catalog_unavailable",
  });

  const malformed = nextExternalImportTaskStateFromProgress(
    running,
    event({ current: 4, total: 3 }),
  );
  assert.equal(malformed.status, "failed");
  assert.equal(malformed.errorCode, "external_import_progress_unrecognized");

  const unknown = nextExternalImportTaskStateFromProgress(
    running,
    event({ phase: "external_import.import.future" }),
  );
  assert.equal(unknown.status, "failed");
  assert.equal(unknown.errorCode, "external_import_progress_unrecognized");

  const unlistedCode = nextExternalImportTaskStateFromProgress(
    running,
    event({
      status: "failed",
      phase: "external_import.import.failed",
      current: 0,
      total: 0,
      error: "some_backend_code_we_do_not_know",
    }),
  );
  assert.equal(unlistedCode.errorCode, "external_import_batch_failed");

  assert.deepEqual(
    nextExternalImportTaskStateFromProgress(unlistedCode, event()),
    unlistedCode,
  );
});
