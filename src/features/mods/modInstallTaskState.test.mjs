import assert from "node:assert/strict";
import { test } from "node:test";

import {
  getManagedInstallTaskPhaseLabel,
  isManagedInstallTaskPhase,
  nextManagedInstallTaskStateFromProgress,
} from "./modInstallTaskState.ts";

test("managed install task phases include uninstall phases", () => {
  assert.equal(isManagedInstallTaskPhase("install.uninstall.queued"), true);
  assert.equal(isManagedInstallTaskPhase("install.uninstall.processing"), true);
  assert.equal(isManagedInstallTaskPhase("install.uninstall.completed"), true);
  assert.equal(isManagedInstallTaskPhase("install.uninstall.failed"), true);
  assert.equal(isManagedInstallTaskPhase("mod_import.prepare.completed"), false);
  assert.equal(getManagedInstallTaskPhaseLabel("install.uninstall.processing"), "卸载中");
});

test("uninstall progress only updates the matching task id", () => {
  const current = {
    status: "running",
    operation: "uninstall",
    taskId: "task-a",
    modName: "Mock Mod",
    phase: "install.uninstall.processing",
  };

  const next = nextManagedInstallTaskStateFromProgress(current, {
    taskId: "task-b",
    kind: "install",
    status: "completed",
    phase: "install.uninstall.completed",
  });

  assert.equal(next, current);
});

test("uninstall completed and failed phases map to stable task states", () => {
  const current = {
    status: "running",
    operation: "uninstall",
    taskId: "task-a",
    modName: "Mock Mod",
    phase: "install.uninstall.processing",
  };

  assert.deepEqual(
    nextManagedInstallTaskStateFromProgress(current, {
      taskId: "task-a",
      kind: "install",
      status: "completed",
      phase: "install.uninstall.completed",
    }),
    {
      status: "completed",
      operation: "uninstall",
      taskId: "task-a",
      modName: "Mock Mod",
      phase: "install.uninstall.completed",
    },
  );

  assert.deepEqual(
    nextManagedInstallTaskStateFromProgress(current, {
      taskId: "task-a",
      kind: "install",
      status: "failed",
      phase: "install.uninstall.failed",
      error: "install_uninstall_failed:uninstall",
    }),
    {
      status: "failed",
      operation: "uninstall",
      taskId: "task-a",
      modName: "Mock Mod",
      phase: "install.uninstall.failed",
      message: "install_uninstall_failed:uninstall",
    },
  );
});
