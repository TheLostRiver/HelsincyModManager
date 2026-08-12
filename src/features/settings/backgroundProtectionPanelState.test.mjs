import assert from "node:assert/strict";
import { test } from "node:test";

import {
  preserveBackgroundProtectionStateAfterRefreshFailure,
  readyBackgroundProtectionPanelState,
} from "./backgroundProtectionPanelState.ts";

const startingControl = {
  desiredEnabled: true,
  status: "starting",
  enabledAt: 1,
  lastHeartbeatAt: null,
  lastErrorCode: null,
};

test("refresh failure preserves the most recent authoritative control status", () => {
  const ready = readyBackgroundProtectionPanelState(startingControl);
  const next = preserveBackgroundProtectionStateAfterRefreshFailure(
    ready,
    "save_backup_background_status_unavailable",
  );

  assert.equal(next.status, "ready");
  assert.equal(next.control, startingControl);
  assert.equal(next.control.desiredEnabled, true);
  assert.equal(next.control.status, "starting");
  assert.equal(next.refreshWarningCode, "save_backup_background_status_unavailable");
});

test("successful status read clears a previous refresh warning", () => {
  const warned = preserveBackgroundProtectionStateAfterRefreshFailure(
    readyBackgroundProtectionPanelState(startingControl),
    "save_backup_background_status_unavailable",
  );
  assert.equal(warned.status, "ready");

  const refreshed = readyBackgroundProtectionPanelState(startingControl);
  assert.equal(refreshed.status, "ready");
  assert.equal(refreshed.refreshWarningCode, null);
});

test("first status read failure remains a global unavailable state", () => {
  assert.deepEqual(
    preserveBackgroundProtectionStateAfterRefreshFailure(
      { status: "loading" },
      "save_backup_background_status_unavailable",
    ),
    {
      status: "error",
      errorCode: "save_backup_background_status_unavailable",
    },
  );
});
