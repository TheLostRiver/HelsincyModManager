import assert from "node:assert/strict";
import { test } from "node:test";

import {
  badgeTierForViewMode,
  externalStatusAriaLabel,
  externalStatusCase,
  projectExternalStatusBadge,
} from "./externalInstallStatusView.ts";

// 假 copy：只关心「用了哪条、带什么数字、什么顺序」，所以文案里把数字拼出来。
const copy = {
  externalOrigin: "外部",
  installed: "已安装",
  notInstalled: "未安装",
  unknown: "状态未知",
  partial: {
    full: (n) => `FULL:partial(missing=${n.missing})`,
    compact: (n) => `COMPACT:partial(missing=${n.missing})`,
    minimal: (n) => `MINIMAL:partial(${n.missing})`,
  },
  changed: {
    full: (n) => `FULL:changed(changed=${n.changed},unreadable=${n.unreadable})`,
    compact: (n) => `COMPACT:changed(changed=${n.changed},unreadable=${n.unreadable})`,
    minimal: (n) => `MINIMAL:changed(${n.changed + n.unreadable})`,
  },
  mixed: {
    // 顺序在这里定死：changed/unreadable 在前，missing 在后。
    full: (n) =>
      `FULL:mixed(changed=${n.changed},unreadable=${n.unreadable},missing=${n.missing})`,
    compact: (n) =>
      `COMPACT:mixed(changed=${n.changed},unreadable=${n.unreadable},missing=${n.missing})`,
    minimal: (n) => `MINIMAL:mixed(${n.changed + n.unreadable + n.missing})`,
  },
};

function summary(overrides) {
  return {
    state: "installed",
    matchedFileCount: 0,
    missingFileCount: 0,
    changedFileCount: 0,
    unreadableFileCount: 0,
    files: [],
    ...overrides,
  };
}

test("tier follows the measured pill width, not a guess", () => {
  // tech 有独立全宽状态行；list 海报固定 120px 是最窄的。
  assert.equal(badgeTierForViewMode("tech"), "full");
  assert.equal(badgeTierForViewMode("list"), "minimal");
  assert.equal(badgeTierForViewMode("classic"), "compact");
  assert.equal(badgeTierForViewMode("grid"), "compact");
});

test("backend states map to badge cases without re-deciding", () => {
  assert.equal(externalStatusCase("installed"), "installed");
  assert.equal(externalStatusCase("not_installed"), "not_installed");
  assert.equal(externalStatusCase("partial"), "partial");
  assert.equal(externalStatusCase("changed"), "changed");
  assert.equal(externalStatusCase("mixed"), "mixed");
  // 后端新增状态值时前端不能崩，按「未知」处理。
  assert.equal(externalStatusCase("something_new"), "unknown");
});

test("clean states need no counts", () => {
  assert.equal(projectExternalStatusBadge(summary({ state: "installed" }), "grid", copy).text, "已安装");
  assert.equal(projectExternalStatusBadge(summary({ state: "not_installed" }), "list", copy).text, "未安装");
  assert.equal(projectExternalStatusBadge(summary({ state: "unknown" }), "tech", copy).text, "状态未知");
});

test("each tier produces its own text", () => {
  const state = summary({ state: "partial", missingFileCount: 3 });

  assert.equal(projectExternalStatusBadge(state, "tech", copy).text, "FULL:partial(missing=3)");
  assert.equal(projectExternalStatusBadge(state, "grid", copy).text, "COMPACT:partial(missing=3)");
  assert.equal(projectExternalStatusBadge(state, "list", copy).text, "MINIMAL:partial(3)");
});

test("mixed puts changed before missing so truncation drops the less critical part", () => {
  // 这是「顺序即优先级」的核心断言：被截断时先保留「已改动」。
  const state = summary({
    state: "mixed",
    changedFileCount: 2,
    missingFileCount: 3,
    matchedFileCount: 28,
  });

  const compact = projectExternalStatusBadge(state, "grid", copy).text;
  assert.ok(
    compact.indexOf("changed=") < compact.indexOf("missing="),
    `精简档必须「已改动」在前：${compact}`,
  );
});

test("the minimal tier reports a total instead of pretending to know the split", () => {
  // 96px 放不下分类，所以只报总数——它不声称「3 个缺失」，因此不撒谎。
  const state = summary({
    state: "mixed",
    changedFileCount: 2,
    missingFileCount: 3,
  });
  assert.equal(projectExternalStatusBadge(state, "list", copy).text, "MINIMAL:mixed(5)");
});

test("unreadable counts as attention, never as clean", () => {
  const state = summary({
    state: "mixed",
    matchedFileCount: 32,
    unreadableFileCount: 1,
  });
  const badge = projectExternalStatusBadge(state, "grid", copy);

  assert.equal(badge.case, "mixed");
  assert.match(badge.text, /unreadable=1/);
});

test("detail always carries the full text regardless of tier", () => {
  // 窄视图显示极简文案，但 title/aria 必须仍是完整事实。
  const state = summary({
    state: "mixed",
    changedFileCount: 2,
    missingFileCount: 3,
  });

  for (const viewMode of ["list", "grid", "tech"]) {
    const badge = projectExternalStatusBadge(state, viewMode, copy);
    assert.equal(
      badge.detail,
      "FULL:mixed(changed=2,unreadable=0,missing=3)",
      `${viewMode} 的 detail 应为完整档`,
    );
  }
});

test("aria label is the external origin plus the full fact", () => {
  const state = summary({ state: "partial", missingFileCount: 3 });
  const badge = projectExternalStatusBadge(state, "list", copy);

  assert.equal(externalStatusAriaLabel(badge, copy), "外部 · FULL:partial(missing=3)");
});
