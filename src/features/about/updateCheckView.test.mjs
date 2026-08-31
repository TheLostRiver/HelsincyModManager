import assert from "node:assert/strict";
import { test } from "node:test";

import { projectUpdateCheckView } from "./updateCheckView.ts";

test("an available update carries the version to display", () => {
  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "update_available", currentVersion: "0.1.0-alpha.0", latestVersion: "v0.2.0" },
    }),
    { kind: "update_available", version: "v0.2.0" },
  );
});

test("update_available without a version stays silent instead of showing a blank hint", () => {
  // 契约说这个状态必带版本号；万一后端违约，宁可「不知道」也不显示空白的「可用」。
  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "update_available", currentVersion: "0.1.0", latestVersion: null },
    }),
    { kind: "unknown" },
  );
});

test("up to date is reported as such", () => {
  assert.deepEqual(
    projectUpdateCheckView({
      checking: false,
      status: { status: "up_to_date", currentVersion: "0.1.0", latestVersion: null },
    }),
    { kind: "up_to_date" },
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
      projectUpdateCheckView({ checking: false, status }),
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
    }),
    { kind: "checking" },
  );
});
