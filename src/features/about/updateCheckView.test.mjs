import assert from "node:assert/strict";
import { test } from "node:test";

import { projectUpdateCheckView } from "./updateCheckView.ts";

test("an available update carries the version to display", () => {
  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "update_available", currentVersion: "0.1.0-alpha.0", latestVersion: "v0.2.0" },
      attemptFailed: false,
    }),
    { kind: "update_available", version: "v0.2.0", stale: false },
  );
});

test("update_available without a version stays silent instead of showing a blank hint", () => {
  // 契约说这个状态必带版本号；万一后端违约，宁可「不知道」也不显示空白的「可用」。
  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "update_available", currentVersion: "0.1.0", latestVersion: null },
      attemptFailed: false,
    }),
    { kind: "unknown" },
  );
});

test("up to date is reported as such", () => {
  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "up_to_date", currentVersion: "0.1.0", latestVersion: null },
      attemptFailed: false,
    }),
    { kind: "up_to_date", stale: false },
  );
});

test("failures and unknown statuses render nothing", () => {
  // 断网 / 超时 / 接口失败 → 静默，不显示任何提示。
  for (const status of [
    null,
    { status: "unknown", currentVersion: "0.1.0", latestVersion: null },
    // 将来新增的状态值也按「不知道」处理，不会漏出未处理的分支。
    { status: "something_new", currentVersion: "0.1.0", latestVersion: null },
  ]) {
    assert.deepEqual(
      projectUpdateCheckView({ checking: false, status, attemptFailed: false }),
      { kind: "unknown" },
      `${JSON.stringify(status)} 应静默`,
    );
  }
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

test("a failed re-check keeps the previous result but marks it stale", () => {
  // 用户点了「检查更新」后断网：旧的「已是最新版本」不能假装是这次复查的结论，
  // 否则用户会以为了解到最新情况——那正是本功能要防的「有新版本却以为没有」。
  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "up_to_date", currentVersion: "0.1.0", latestVersion: null },
      attemptFailed: true,
    }),
    { kind: "up_to_date", stale: true },
  );

  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "update_available", currentVersion: "0.1.0", latestVersion: "v0.2.0" },
      attemptFailed: true,
    }),
    { kind: "update_available", version: "v0.2.0", stale: true },
  );
});

test("a successful re-check clears the stale mark", () => {
  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "up_to_date", currentVersion: "0.1.0", latestVersion: null },
      attemptFailed: false,
    }),
    { kind: "up_to_date", stale: false },
  );
});
