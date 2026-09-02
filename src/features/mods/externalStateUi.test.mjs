import assert from "node:assert/strict";
import { test } from "node:test";
import {
  externalStatusAriaLabel,
  projectExternalStatusBadge,
} from "./externalInstallStatusView.ts";
import {
  externalStateCopy,
  externalStateErrorMessage,
} from "./externalStateCopy.ts";

const zh = externalStateCopy.zh_cn;

function summaryOf(state, counts, files = []) {
  return {
    state,
    matchedFileCount: counts.matched ?? 0,
    missingFileCount: counts.missing ?? 0,
    changedFileCount: counts.changed ?? 0,
    unreadableFileCount: counts.unreadable ?? 0,
    files,
  };
}

test("完整档按决策关键度排序：改动在前，缺失在后", () => {
  const badge = projectExternalStatusBadge(
    summaryOf("mixed", { changed: 1, unreadable: 1, missing: 3 }),
    "tech",
    zh.badge,
  );

  assert.equal(badge.tier, "full");
  assert.equal(badge.text, "已被改动 · 2 个文件 · 另有 3 个缺失");
});

test("精简档与极简档按视图降级，极简档不假装知道分类", () => {
  const summary = summaryOf("mixed", { changed: 1, unreadable: 1, missing: 3 });

  const compact = projectExternalStatusBadge(summary, "grid", zh.badge);
  assert.equal(compact.tier, "compact");
  assert.equal(compact.text, "已改动 2 · 缺失 3");

  const minimal = projectExternalStatusBadge(summary, "list", zh.badge);
  assert.equal(minimal.tier, "minimal");
  // 极简档只报总数，不声称「N 个缺失」——96px 里放不下分类，宁可少说不说错。
  assert.equal(minimal.text, "需注意 5");
});

test("mixed 无缺失时不渲染「另有 0 个缺失」尾段", () => {
  const badge = projectExternalStatusBadge(
    summaryOf("mixed", { changed: 0, unreadable: 2, missing: 0 }),
    "tech",
    zh.badge,
  );

  assert.equal(badge.text, "已被改动 · 2 个文件");
});

test("aria 标签带外部来源前缀且用完整档事实", () => {
  const badge = projectExternalStatusBadge(
    summaryOf("partial", { missing: 3 }),
    "list",
    zh.badge,
  );

  assert.equal(badge.text, "需注意 3");
  assert.equal(
    externalStatusAriaLabel(badge, zh.badge),
    "外部 · 部分安装 · 3 个文件缺失",
  );
});

test("错误码映射：已知码取词，未知码保留原码可见", () => {
  assert.equal(externalStateErrorMessage("external_state_scan_stale", zh), zh.errors.stale);
  assert.equal(
    externalStateErrorMessage("external_state_scan_cancelled", zh),
    zh.errors.cancelled,
  );
  const unknown = externalStateErrorMessage("external_state_scan_totally_new", zh);
  assert.match(unknown, /external_state_scan_totally_new/);
});

test("三语字典的徽标函数在同一输入下都产出非空文案", () => {
  const numbers = { changed: 1, unreadable: 0, missing: 2 };
  for (const locale of ["zh_cn", "en", "ja"]) {
    const copy = externalStateCopy[locale];
    for (const badgeCase of ["partial", "changed", "mixed"]) {
      for (const tier of ["full", "compact", "minimal"]) {
        const text = copy.badge[badgeCase][tier](numbers);
        assert.ok(text.length > 0, `${locale}.${badgeCase}.${tier} 不得为空`);
      }
    }
  }
});
