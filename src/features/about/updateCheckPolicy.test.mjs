import assert from "node:assert/strict";
import { test } from "node:test";

import {
  sanitizeUpdateCheckPreference,
  shouldCheckForUpdate,
  UPDATE_CHECK_MIN_INTERVAL_MILLIS,
} from "./updateCheckPolicy.ts";
import { DEFAULT_UPDATE_CHECK_PREFERENCE } from "./updateCheckTypes.ts";

const HOUR = 60 * 60 * 1000;

test("a disabled auto check never queries", () => {
  // 关掉之后，无论「从未查过」还是「早就过了间隔」都不该发请求。
  assert.equal(
    shouldCheckForUpdate({ autoCheckEnabled: false, lastCheckedAt: null }, 1_000),
    false,
  );
  assert.equal(
    shouldCheckForUpdate({ autoCheckEnabled: false, lastCheckedAt: 0 }, 1_000_000),
    false,
  );
});

test("the very first visit queries", () => {
  assert.equal(
    shouldCheckForUpdate({ autoCheckEnabled: true, lastCheckedAt: null }, 1_000),
    true,
  );
});

test("within the minimum interval stays quiet", () => {
  assert.equal(
    shouldCheckForUpdate(
      { autoCheckEnabled: true, lastCheckedAt: 1_000 },
      1_000 + UPDATE_CHECK_MIN_INTERVAL_MILLIS - 1,
    ),
    false,
  );
});

test("exactly at the minimum interval queries", () => {
  // 边界：正好间隔 24 小时算「已过期」——用 `>=` 而不是 `>`。
  assert.equal(
    shouldCheckForUpdate(
      { autoCheckEnabled: true, lastCheckedAt: 1_000 },
      1_000 + UPDATE_CHECK_MIN_INTERVAL_MILLIS,
    ),
    true,
  );
});

test("beyond the minimum interval queries", () => {
  assert.equal(
    shouldCheckForUpdate(
      { autoCheckEnabled: true, lastCheckedAt: 0 },
      UPDATE_CHECK_MIN_INTERVAL_MILLIS + HOUR,
    ),
    true,
  );
});

test("a clock moved backwards does not get stuck", () => {
  // lastCheckedAt 在 now 之后（系统时钟回拨 / 手工改过存档）会让间隔变成负数。
  // 这时按「该查」处理，否则用户会永久卡在「刚查过」。
  assert.equal(
    shouldCheckForUpdate({ autoCheckEnabled: true, lastCheckedAt: 10_000 }, 1_000),
    true,
  );
});

test("unusable persisted values fall back to the default preference", () => {
  for (const value of [null, undefined, "nonsense", 42, []]) {
    assert.deepEqual(
      sanitizeUpdateCheckPreference(value),
      DEFAULT_UPDATE_CHECK_PREFERENCE,
      `${String(value)} 应退回默认偏好`,
    );
  }
});

test("a partially stored preference keeps what it can and defaults the rest", () => {
  assert.deepEqual(sanitizeUpdateCheckPreference({ autoCheckEnabled: false }), {
    autoCheckEnabled: false,
    lastCheckedAt: null,
  });
});

test("non-finite timestamps are discarded instead of poisoning the interval", () => {
  for (const value of ["1700000000000", Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.equal(
      sanitizeUpdateCheckPreference({ autoCheckEnabled: true, lastCheckedAt: value })
        .lastCheckedAt,
      null,
      `${String(value)} 不是合法时间戳`,
    );
  }
});

test("unknown extra fields are ignored", () => {
  assert.deepEqual(
    sanitizeUpdateCheckPreference({
      autoCheckEnabled: true,
      lastCheckedAt: 5,
      somethingElse: "ignored",
    }),
    { autoCheckEnabled: true, lastCheckedAt: 5 },
  );
});
