import assert from "node:assert/strict";
import { test } from "node:test";

import { buildPackageContentTree, flattenVisibleRows } from "./packageContentTree.ts";
import { resolveTreeKeyAction, resolveVisibleWindow } from "./packageContentTreeInteraction.ts";

function entry(packageFileId) {
  return {
    packageFileId,
    sizeBytes: 1,
    targetPath: null,
    installable: true,
    rejectedByGame: false,
    excludedByPlayer: false,
  };
}

/** 固定夹具：dir 展开（含 sub 折叠 + 一个文件），外加一个根层文件。 */
function fixtureRows() {
  const nodes = buildPackageContentTree([
    entry("dir/sub/deep.txt"),
    entry("dir/a.txt"),
    entry("loose.txt"),
  ]);
  const rows = flattenVisibleRows(nodes, new Set(["dir"]));

  assert.deepEqual(
    rows.map((row) => `${row.node.name}@${row.level}`),
    ["dir@1", "sub@2", "a.txt@2", "loose.txt@1"],
    "夹具形状变了，下面的索引断言会失去意义",
  );

  return rows;
}

test("窗口区间覆盖视口并各留一段 overscan", () => {
  const window = resolveVisibleWindow({
    scrollTop: 320,
    viewportHeight: 320,
    rowHeight: 32,
    rowCount: 1000,
    overscan: 3,
  });

  // 首个可见行 = 320/32 = 10，视口容纳 10 行，两端各 3 行 overscan。
  assert.deepEqual(window, { startIndex: 7, endIndex: 23 });
});

test("滚到顶部时起点不为负", () => {
  const window = resolveVisibleWindow({
    scrollTop: 0,
    viewportHeight: 320,
    rowHeight: 32,
    rowCount: 1000,
    overscan: 5,
  });

  assert.equal(window.startIndex, 0);
  assert.equal(window.endIndex, 15);
});

test("惯性回弹给出负 scrollTop 时不产出负区间", () => {
  const window = resolveVisibleWindow({
    scrollTop: -240,
    viewportHeight: 320,
    rowHeight: 32,
    rowCount: 1000,
    overscan: 2,
  });

  assert.equal(window.startIndex, 0);
  assert.ok(window.endIndex >= window.startIndex);
});

test("滚到底部时终点不越过总行数", () => {
  const window = resolveVisibleWindow({
    scrollTop: 100_000,
    viewportHeight: 320,
    rowHeight: 32,
    rowCount: 50,
    overscan: 4,
  });

  assert.equal(window.endIndex, 50);
  assert.ok(window.startIndex <= 50);
});

test("没有行时窗口为空，不产出 0..overscan 的假区间", () => {
  assert.deepEqual(
    resolveVisibleWindow({ scrollTop: 0, viewportHeight: 320, rowHeight: 32, rowCount: 0, overscan: 5 }),
    { startIndex: 0, endIndex: 0 },
  );
});

test("视口尚未测量（高度 0）时窗口为空，避免首帧渲染整棵树", () => {
  assert.deepEqual(
    resolveVisibleWindow({ scrollTop: 0, viewportHeight: 0, rowHeight: 32, rowCount: 7340, overscan: 5 }),
    { startIndex: 0, endIndex: 0 },
  );
});

test("7340 行也只渲染视口那一小段", () => {
  const window = resolveVisibleWindow({
    scrollTop: 0,
    viewportHeight: 640,
    rowHeight: 32,
    rowCount: 7340,
    overscan: 6,
  });

  assert.ok(window.endIndex - window.startIndex < 40, "窗口不该随总行数增长");
});

test("上下键在可见行之间移动，到头就不再响应", () => {
  const rows = fixtureRows();

  assert.deepEqual(resolveTreeKeyAction("ArrowDown", { rows, activeIndex: 0 }), { kind: "move", index: 1 });
  assert.deepEqual(resolveTreeKeyAction("ArrowUp", { rows, activeIndex: 1 }), { kind: "move", index: 0 });
  assert.equal(resolveTreeKeyAction("ArrowUp", { rows, activeIndex: 0 }), null);
  assert.equal(resolveTreeKeyAction("ArrowDown", { rows, activeIndex: rows.length - 1 }), null);
});

test("Home/End 跳到首尾行", () => {
  const rows = fixtureRows();

  assert.deepEqual(resolveTreeKeyAction("Home", { rows, activeIndex: 2 }), { kind: "move", index: 0 });
  assert.deepEqual(resolveTreeKeyAction("End", { rows, activeIndex: 0 }), { kind: "move", index: 3 });
});

test("右键在折叠目录上是展开，在已展开目录上是走进第一个子级", () => {
  const rows = fixtureRows();

  // index 1 = sub（折叠中的目录）
  assert.deepEqual(resolveTreeKeyAction("ArrowRight", { rows, activeIndex: 1 }), {
    kind: "expand",
    path: "dir/sub",
  });
  // index 0 = dir（已展开）→ 走进紧挨着的下一行
  assert.deepEqual(resolveTreeKeyAction("ArrowRight", { rows, activeIndex: 0 }), { kind: "move", index: 1 });
});

test("右键在文件上不做任何事", () => {
  const rows = fixtureRows();

  // index 2 = a.txt
  assert.equal(resolveTreeKeyAction("ArrowRight", { rows, activeIndex: 2 }), null);
});

test("左键在已展开目录上是折叠", () => {
  const rows = fixtureRows();

  assert.deepEqual(resolveTreeKeyAction("ArrowLeft", { rows, activeIndex: 0 }), {
    kind: "collapse",
    path: "dir",
  });
});

test("左键在子级上回到父行，而不是回到上一行", () => {
  const rows = fixtureRows();

  // index 2 = a.txt（level 2），它的上一行是 sub（同为 level 2），父级是 index 0 的 dir。
  assert.deepEqual(resolveTreeKeyAction("ArrowLeft", { rows, activeIndex: 2 }), { kind: "move", index: 0 });
});

test("左键在根层节点上不做任何事", () => {
  const rows = fixtureRows();

  // index 3 = loose.txt（level 1，没有父级）
  assert.equal(resolveTreeKeyAction("ArrowLeft", { rows, activeIndex: 3 }), null);
});

test("无关按键一律不处理，调用方据此放行默认行为", () => {
  const rows = fixtureRows();

  for (const key of ["a", "Tab", "Enter", "PageDown", "Escape"]) {
    assert.equal(resolveTreeKeyAction(key, { rows, activeIndex: 0 }), null, `${key} 不该被拦截`);
  }
});

test("空树上任何按键都不处理", () => {
  assert.equal(resolveTreeKeyAction("ArrowDown", { rows: [], activeIndex: 0 }), null);
  assert.equal(resolveTreeKeyAction("Home", { rows: [], activeIndex: 0 }), null);
});
