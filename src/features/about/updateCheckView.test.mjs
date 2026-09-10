import assert from "node:assert/strict";
import { test } from "node:test";

import { projectUpdateCheckView } from "./updateCheckView.ts";
import { normalizeAppUpdateStatus } from "./updateCheckTypes.ts";

test("an available update carries the version to display", () => {
  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "update_available", currentVersion: "0.1.0-alpha.0", latestVersion: "v0.2.0" },
      attemptFailed: false,
    }),
    { kind: "update_available", version: "v0.2.0" },
  );
});

test("update_available without a version reports unavailable instead of a blank hint", () => {
  // 契约说这个状态必带版本号；万一后端违约，宁可「不知道」也不显示空白的「可用」。
  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "update_available", currentVersion: "0.1.0", latestVersion: null },
      attemptFailed: false,
    }),
    { kind: "unavailable" },
  );
});

test("up to date is reported as such", () => {
  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "up_to_date", currentVersion: "0.1.0", latestVersion: null },
      attemptFailed: false,
    }),
    { kind: "up_to_date" },
  );
});

for (const status of ["unknown", "something_new"]) {
  test(`${status} status reports that the update check is unavailable`, () => {
    assert.deepEqual(
      projectUpdateCheckView({ checking: false, status: { status, currentVersion: "0.1.0", latestVersion: null }, attemptFailed: false }),
      { kind: "unavailable" },
    );
  });
}

test("not checked and failed without a result are distinct", () => {
  assert.deepEqual(projectUpdateCheckView({ checking: false, status: null, attemptFailed: false }), { kind: "idle" });
  assert.deepEqual(projectUpdateCheckView({ checking: false, status: null, attemptFailed: true }), { kind: "unavailable" });
});

test("checking wins over a stale result", () => {
  // 正在查的时候不能同时显示上一次的结论。
  assert.deepEqual(
    projectUpdateCheckView({
      checking: true,
      status: { status: "up_to_date", currentVersion: "0.1.0", latestVersion: null },
      attemptFailed: true,
    }),
    { kind: "checking" },
  );
});

test("a failed re-check cannot present a previous success as current", () => {
  // 用户点了「检查更新」后断网：旧的「已是最新版本」不能假装是这次复查的结论，
  // 否则用户会以为了解到最新情况——那正是本功能要防的「有新版本却以为没有」。
  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "up_to_date", currentVersion: "0.1.0", latestVersion: null },
      attemptFailed: true,
    }),
    { kind: "unavailable" },
  );

  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "update_available", currentVersion: "0.1.0", latestVersion: "v0.2.0" },
      attemptFailed: true,
    }),
    { kind: "unavailable" },
  );
});

test("a successful re-check restores the result", () => {
  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "up_to_date", currentVersion: "0.1.0", latestVersion: null },
      attemptFailed: false,
    }),
    { kind: "up_to_date" },
  );
});

test("an empty usable release list is not reported as the latest version", () => {
  const status = { status: "no_release", currentVersion: "1.0.0", latestVersion: null };
  assert.equal(normalizeAppUpdateStatus(status), status);
  assert.deepEqual(projectUpdateCheckView({ checking: false, status, attemptFailed: false }), { kind: "no_release" });
});

for (const [label, value] of [
  ["null response", null],
  ["empty response", {}],
  ["unknown status", { status: "other", currentVersion: "1.0.0", latestVersion: null }],
  ["empty update version", { status: "update_available", currentVersion: "1.0.0", latestVersion: " " }],
  ["non-string update version", { status: "update_available", currentVersion: "1.0.0", latestVersion: 12 }],
  ["missing current version", { status: "up_to_date", latestVersion: null }],
  ["inconsistent latest version", { status: "up_to_date", currentVersion: "1.0.0", latestVersion: "2.0.0" }],
]) {
  test(`invalid update DTO fails closed: ${label}`, () => assert.equal(normalizeAppUpdateStatus(value), null));
}
