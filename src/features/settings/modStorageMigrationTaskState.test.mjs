import assert from "node:assert/strict";
import { test } from "node:test";

import {
  canCancelModStorageMigration,
  getModStorageMigrationPhaseLabel,
  isModStorageMigrationActive,
  isModStorageMigrationTerminal,
  MOD_STORAGE_MIGRATION_PHASES,
  MOD_STORAGE_MIGRATION_UNRECOGNIZED_CODE,
  nextModStorageMigrationStateFromProgress,
} from "./modStorageMigrationTaskState.ts";

function event(overrides = {}) {
  return {
    taskId: "mod-storage-migration-1",
    kind: "mod_storage_migration",
    status: "running",
    phase: MOD_STORAGE_MIGRATION_PHASES.copying,
    current: 0,
    total: 3,
    message: null,
    error: null,
    resultRef: null,
    ...overrides,
  };
}

const running = {
  status: "running",
  taskId: "mod-storage-migration-1",
  phase: MOD_STORAGE_MIGRATION_PHASES.queued,
  current: null,
  total: null,
};

test("progress phases advance the running state with package counts", () => {
  let state = running;
  for (const phase of [
    MOD_STORAGE_MIGRATION_PHASES.copying,
    MOD_STORAGE_MIGRATION_PHASES.verifying,
    MOD_STORAGE_MIGRATION_PHASES.switching,
  ]) {
    state = nextModStorageMigrationStateFromProgress(state, event({ phase, current: 2, total: 3 }));
    assert.deepEqual(state, {
      status: "running",
      taskId: "mod-storage-migration-1",
      phase,
      current: 2,
      total: 3,
    });
  }
  assert.equal(isModStorageMigrationActive(state), true);
  assert.equal(isModStorageMigrationTerminal(state), false);
});

test("completed carries the package count and is terminal", () => {
  const state = nextModStorageMigrationStateFromProgress(
    running,
    event({ status: "completed", phase: MOD_STORAGE_MIGRATION_PHASES.completed, current: 3, total: 3 }),
  );
  assert.deepEqual(state, { status: "completed", taskId: "mod-storage-migration-1", packageCount: 3 });
  assert.equal(isModStorageMigrationTerminal(state), true);
  assert.equal(
    nextModStorageMigrationStateFromProgress(state, event({ phase: MOD_STORAGE_MIGRATION_PHASES.copying })),
    state,
    "a terminal state ignores late events",
  );
});

test("cancelling is not terminal; the runner's cancelled event is", () => {
  const cancelling = nextModStorageMigrationStateFromProgress(
    { ...running, phase: MOD_STORAGE_MIGRATION_PHASES.copying, current: 1, total: 3 },
    event({ status: "cancelled", phase: MOD_STORAGE_MIGRATION_PHASES.cancelling, current: null, total: null }),
  );
  assert.deepEqual(cancelling, {
    status: "cancelling",
    taskId: "mod-storage-migration-1",
    phase: MOD_STORAGE_MIGRATION_PHASES.cancelling,
    current: 1,
    total: 3,
  });
  assert.equal(isModStorageMigrationTerminal(cancelling), false);
  assert.equal(isModStorageMigrationActive(cancelling), true);
  assert.equal(canCancelModStorageMigration(cancelling), false);

  const cancelled = nextModStorageMigrationStateFromProgress(
    cancelling,
    event({ status: "cancelled", phase: MOD_STORAGE_MIGRATION_PHASES.cancelled, current: null, total: null }),
  );
  assert.deepEqual(cancelled, { status: "cancelled", taskId: "mod-storage-migration-1" });
  assert.equal(isModStorageMigrationTerminal(cancelled), true);
});

test("failed keeps only registered stable codes", () => {
  const known = nextModStorageMigrationStateFromProgress(
    running,
    event({
      status: "failed",
      phase: MOD_STORAGE_MIGRATION_PHASES.failed,
      current: null,
      total: null,
      error: "mod_storage_migration_verify_mismatch",
    }),
  );
  assert.deepEqual(known, {
    status: "failed",
    taskId: "mod-storage-migration-1",
    errorCode: "mod_storage_migration_verify_mismatch",
  });

  const unknown = nextModStorageMigrationStateFromProgress(
    running,
    event({
      status: "failed",
      phase: MOD_STORAGE_MIGRATION_PHASES.failed,
      current: null,
      total: null,
      error: "C:/leaked/path",
    }),
  );
  assert.equal(unknown.errorCode, MOD_STORAGE_MIGRATION_UNRECOGNIZED_CODE);
});

test("events for other tasks or kinds are ignored; malformed ones fail closed", () => {
  assert.equal(
    nextModStorageMigrationStateFromProgress(running, event({ taskId: "mod-storage-migration-2" })),
    running,
  );
  assert.equal(
    nextModStorageMigrationStateFromProgress(running, event({ kind: "mod_import" })),
    running,
  );
  const starting = { status: "starting" };
  assert.equal(
    nextModStorageMigrationStateFromProgress(starting, event()),
    starting,
    "without a bound task id nothing is applied (the hook buffers instead)",
  );

  const unknownPhase = nextModStorageMigrationStateFromProgress(running, event({ phase: "mod_storage.migration.teleporting" }));
  assert.deepEqual(unknownPhase, {
    status: "failed",
    taskId: "mod-storage-migration-1",
    errorCode: MOD_STORAGE_MIGRATION_UNRECOGNIZED_CODE,
  });
  const overflow = nextModStorageMigrationStateFromProgress(running, event({ current: 5, total: 3 }));
  assert.equal(overflow.status, "failed");
  const halfProgress = nextModStorageMigrationStateFromProgress(running, event({ current: 1, total: null }));
  assert.equal(halfProgress.status, "failed");
  const wrongStatus = nextModStorageMigrationStateFromProgress(
    running,
    event({ status: "completed", phase: MOD_STORAGE_MIGRATION_PHASES.copying }),
  );
  assert.equal(wrongStatus.status, "failed");
});

test("cancel is offered until the switching phase (backend barrier)", () => {
  assert.equal(canCancelModStorageMigration(running), true);
  assert.equal(
    canCancelModStorageMigration({ ...running, phase: MOD_STORAGE_MIGRATION_PHASES.switching, current: 3, total: 3 }),
    false,
  );
  assert.equal(canCancelModStorageMigration({ status: "starting" }), false);
});

test("phase labels fall back for unregistered phases", () => {
  const migrationCopy = {
    phases: Object.fromEntries(Object.values(MOD_STORAGE_MIGRATION_PHASES).map((phase) => [phase, `label:${phase}`])),
    unrecognizedPhase: "unrecognized",
  };
  assert.equal(
    getModStorageMigrationPhaseLabel(MOD_STORAGE_MIGRATION_PHASES.verifying, migrationCopy),
    `label:${MOD_STORAGE_MIGRATION_PHASES.verifying}`,
  );
  assert.equal(getModStorageMigrationPhaseLabel("mod_storage.migration.nope", migrationCopy), "unrecognized");
});
