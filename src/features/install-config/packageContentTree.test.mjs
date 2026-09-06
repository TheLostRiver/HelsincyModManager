import assert from "node:assert/strict";
import { test } from "node:test";

import {
  buildPackageContentTree,
  flattenVisibleRows,
  resolveInitialExpandedPaths,
  summarizeTree,
} from "./packageContentTree.ts";

/*
 * 这些用例喂输入断言输出，不去正则扫源码：树模型是后面所有交互（勾选级联、内容根切换、
 * 计划预览）的地基，「文件里出现了某个标识符」证明不了它算得对。
 */

function entry(packageFileId, overrides = {}) {
  return {
    packageFileId,
    sizeBytes: 1,
    targetPath: null,
    installable: true,
    rejectedByGame: false,
    excludedByPlayer: false,
    ...overrides,
  };
}

function childNamed(nodes, name) {
  const found = nodes.find((node) => node.name === name);
  assert.ok(found, `expected a node named ${name}, got: ${nodes.map((node) => node.name).join(", ")}`);
  return found;
}

test("按 packageFileId 的路径分段建出目录树，叶子挂在正确的目录下", () => {
  const nodes = buildPackageContentTree([
    entry("nativePC/wp/two/two003.mod3"),
    entry("nativePC/wp/two/two003.mrl3"),
    entry("readme.txt"),
  ]);

  // 根层：目录 nativePC + 文件 readme.txt
  assert.deepEqual(
    nodes.map((node) => `${node.kind}:${node.name}`),
    ["directory:nativePC", "file:readme.txt"],
  );

  const two = childNamed(childNamed(childNamed(nodes, "nativePC").children, "wp").children, "two");
  assert.deepEqual(
    two.children.map((node) => node.name),
    ["two003.mod3", "two003.mrl3"],
  );
  // 目录节点的 path 是完整的沙箱相对路径——展开状态与级联操作都用它当键。
  assert.equal(two.path, "nativePC/wp/two");
  assert.equal(two.children[0].path, "nativePC/wp/two/two003.mod3");
});

test("目录聚合子孙统计，折叠时也能看出里面有什么", () => {
  const nodes = buildPackageContentTree([
    entry("pkg/a.mod3", { sizeBytes: 100, installable: true }),
    entry("pkg/nested/b.tex", { sizeBytes: 20, installable: false, rejectedByGame: true }),
    entry("pkg/nested/c.dll", { sizeBytes: 3, installable: true, excludedByPlayer: true }),
  ]);

  const pkg = childNamed(nodes, "pkg");
  assert.deepEqual(pkg.stats, {
    fileCount: 3,
    installableCount: 2,
    rejectedByGameCount: 1,
    excludedByPlayerCount: 1,
    totalSizeBytes: 123,
  });

  // 子目录只统计自己的子孙，不把兄弟算进来。
  const nested = childNamed(pkg.children, "nested");
  assert.deepEqual(nested.stats, {
    fileCount: 2,
    installableCount: 1,
    rejectedByGameCount: 1,
    excludedByPlayerCount: 1,
    totalSizeBytes: 23,
  });
});

test("三条事实各自独立统计，一个文件可以同时被游戏拒绝和被玩家勾掉", () => {
  const nodes = buildPackageContentTree([
    entry("pkg/x.exe", { installable: true, rejectedByGame: true, excludedByPlayer: true }),
  ]);

  const pkg = childNamed(nodes, "pkg");
  assert.equal(pkg.stats.installableCount, 1);
  assert.equal(pkg.stats.rejectedByGameCount, 1);
  assert.equal(pkg.stats.excludedByPlayerCount, 1);
});

test("同层排序：目录在前、文件在后，各自按名字排", () => {
  const nodes = buildPackageContentTree([
    entry("zeta.txt"),
    entry("alpha.txt"),
    entry("zdir/inner.txt"),
    entry("adir/inner.txt"),
  ]);

  assert.deepEqual(
    nodes.map((node) => `${node.kind}:${node.name}`),
    ["directory:adir", "directory:zdir", "file:alpha.txt", "file:zeta.txt"],
  );
});

test("非 ASCII 目录名照常建树（真实语料里的包装目录就是中文）", () => {
  const nodes = buildPackageContentTree([
    entry("黑骑士大剑/nativePC/models/player.mod3"),
    entry("大剑/nativePC/wp/two/two003.mod3"),
  ]);

  assert.deepEqual(
    nodes.map((node) => node.name),
    ["大剑", "黑骑士大剑"],
  );
  const daijian = childNamed(nodes, "大剑");
  assert.equal(daijian.stats.fileCount, 1);
  assert.equal(childNamed(daijian.children, "nativePC").path, "大剑/nativePC");
});

test("空清单建出空树，摘要全零", () => {
  const nodes = buildPackageContentTree([]);

  assert.deepEqual(nodes, []);
  assert.deepEqual(summarizeTree(nodes), {
    fileCount: 0,
    installableCount: 0,
    rejectedByGameCount: 0,
    excludedByPlayerCount: 0,
    totalSizeBytes: 0,
  });
});

test("常见规模的包默认全展开——不该为了看清 28 个文件去逐层点开", () => {
  // 形状照真实外观包：包装层之下两个并列的资源目录，其中一个还有两层嵌套。
  const nodes = buildPackageContentTree([
    entry("nativePC/pl/f_equip/mod_pl_rosedress/hand000_BM.tex"),
    entry("nativePC/pl/f_equip/mod_pl_rosedress/hand000_CMM.tex"),
    entry("nativePC/pl/f_equip/pl078_0000/arm/mod/f_arm078_0000.mod3"),
    entry("nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3"),
  ]);

  const rows = flattenVisibleRows(nodes, resolveInitialExpandedPaths(nodes));

  // 每个文件都直接可见，一次点击都不需要。
  assert.deepEqual(
    rows
      .filter((row) => row.node.kind === "file")
      .map((row) => row.node.name)
      .sort(),
    ["f_arm078_0000.mod3", "f_body078_0000.mod3", "hand000_BM.tex", "hand000_CMM.tex"],
  );
});

test("单目录包装链不计预算——再小的预算也展开，否则只看见一个孤零零的目录", () => {
  const nodes = buildPackageContentTree([
    entry("wrapper/inner/nativePC/wp/a.mod3"),
    entry("wrapper/inner/nativePC/pl/b.mod3"),
  ]);

  const expanded = resolveInitialExpandedPaths(nodes, { rowBudget: 1 });

  // wrapper → inner → nativePC 三层各只有一个目录、没有文件，是纯包装层。
  // 预算只有 1 行，但它们仍然展开；再往下的 pl / wp 才受预算约束。
  assert.deepEqual([...expanded].sort(), ["wrapper", "wrapper/inner", "wrapper/inner/nativePC"]);
});

test("预算够展开浅层就展开浅层，深层留给玩家", () => {
  // a/ 与 b/ 各含一个 inner/，inner 里各 10 个文件。
  // 展开根层 = 2 + 2 = 4 行（≤10，放）；再展开 inner 层 = 4 + 20 = 24 行（>10，停）。
  const entries = [];
  for (const directory of ["a", "b"]) {
    for (let file = 0; file < 10; file += 1) {
      entries.push(entry(`${directory}/inner/f${file}.txt`));
    }
  }
  const nodes = buildPackageContentTree(entries);

  assert.deepEqual([...resolveInitialExpandedPaths(nodes, { rowBudget: 10 })].sort(), ["a", "b"]);
});

test("整层放或整层不放，不出现半开半闭的同级目录", () => {
  // 根下 5 个目录各 10 个文件：展开根层会得到 5 + 50 = 55 行，超出预算 20。
  const entries = [];
  for (let directory = 0; directory < 5; directory += 1) {
    for (let file = 0; file < 10; file += 1) {
      entries.push(entry(`d${directory}/f${file}.txt`));
    }
  }
  const nodes = buildPackageContentTree(entries);

  const expanded = resolveInitialExpandedPaths(nodes, { rowBudget: 20 });

  // 不会「展开前两个、剩下三个折叠着」——同一深度的兄弟同进同退。
  assert.deepEqual([...expanded], []);
  assert.equal(flattenVisibleRows(nodes, expanded).length, 5);
});

test("默认折叠：未展开的目录不产出任何子孙行", () => {
  const nodes = buildPackageContentTree([
    entry("pkg/a.mod3"),
    entry("pkg/nested/b.tex"),
    entry("top.txt"),
  ]);

  const rows = flattenVisibleRows(nodes, new Set());

  assert.deepEqual(
    rows.map((row) => row.node.name),
    ["pkg", "top.txt"],
  );
});

test("展开一个目录只放出它的直接子级，孙级仍然折叠", () => {
  const nodes = buildPackageContentTree([
    entry("pkg/a.mod3"),
    entry("pkg/nested/b.tex"),
  ]);

  const rows = flattenVisibleRows(nodes, new Set(["pkg"]));

  assert.deepEqual(
    rows.map((row) => `${row.node.name}@${row.level}`),
    ["pkg@1", "nested@2", "a.mod3@2"],
  );

  const deeper = flattenVisibleRows(nodes, new Set(["pkg", "pkg/nested"]));
  assert.deepEqual(
    deeper.map((row) => `${row.node.name}@${row.level}`),
    ["pkg@1", "nested@2", "b.tex@3", "a.mod3@2"],
  );
});

test("可见行带齐 a11y 位置信息，且文件行不产出 isExpanded", () => {
  const nodes = buildPackageContentTree([
    entry("dir/one.txt"),
    entry("dir/two.txt"),
    entry("loose.txt"),
  ]);

  const rows = flattenVisibleRows(nodes, new Set(["dir"]));

  // 根层两项：dir(1/2)、loose.txt(2/2)；dir 的两个子级各自 1/2、2/2。
  assert.deepEqual(
    rows.map((row) => [row.node.name, row.level, row.posInSet, row.setSize]),
    [
      ["dir", 1, 1, 2],
      ["one.txt", 2, 1, 2],
      ["two.txt", 2, 2, 2],
      ["loose.txt", 1, 2, 2],
    ],
  );

  const directoryRow = rows[0];
  const fileRow = rows[1];
  assert.equal(directoryRow.isExpanded, true);
  // 文件没有展开态：渲染时不应输出 aria-expanded，否则读屏会把叶子读成可展开节点。
  assert.equal(fileRow.isExpanded, undefined);
  assert.equal("isExpanded" in fileRow, true, "字段存在但为 undefined，渲染层据此跳过属性");
});

test("折叠一个已展开目录的祖先，整棵子树都退出可见行", () => {
  const nodes = buildPackageContentTree([entry("a/b/c.txt")]);

  // 只展开 a/b 而不展开 a：a 折叠着，b 与 c 都不该出现。
  const rows = flattenVisibleRows(nodes, new Set(["a/b"]));

  assert.deepEqual(
    rows.map((row) => row.node.name),
    ["a"],
  );
});

test("整包摘要等于各根节点统计之和", () => {
  const nodes = buildPackageContentTree([
    entry("pkg/a.mod3", { sizeBytes: 10, installable: true }),
    entry("pkg/b.exe", { sizeBytes: 5, installable: false, rejectedByGame: true }),
    entry("loose.txt", { sizeBytes: 2, installable: true, excludedByPlayer: true }),
  ]);

  assert.deepEqual(summarizeTree(nodes), {
    fileCount: 3,
    installableCount: 2,
    rejectedByGameCount: 1,
    excludedByPlayerCount: 1,
    totalSizeBytes: 17,
  });
});

test("扛得住实测最大的包：7340 文件、深度 10", () => {
  // 形状照真实的大包：一个顶层目录包住全部内容，往下才开始分叉。
  const entries = [];
  for (let index = 0; index < 7340; index += 1) {
    const segments = ["nativePC"];
    for (let depth = 1; depth < 9; depth += 1) {
      segments.push(`d${depth}_${index % (depth + 1)}`);
    }
    segments.push(`file${index}.tex`);
    entries.push(entry(segments.join("/"), { sizeBytes: 2 }));
  }
  assert.equal(entries[0].packageFileId.split("/").length, 10, "夹具本身必须是深度 10");

  const nodes = buildPackageContentTree(entries);
  const summary = summarizeTree(nodes);

  assert.equal(summary.fileCount, 7340);
  assert.equal(summary.totalSizeBytes, 14680);

  // 全折叠时可见行只有根层那一个目录——窗口化之前，默认折叠本身就是第一道闸。
  assert.equal(flattenVisibleRows(nodes, new Set()).length, 1);

  // 自动展开在预算处停手：顶层包装目录照常展开，但绝不会把 7340 行一次放出来。
  const expanded = resolveInitialExpandedPaths(nodes);
  const visibleCount = flattenVisibleRows(nodes, expanded).length;

  assert.ok(expanded.has("nativePC"), "顶层包装目录仍应展开");
  assert.ok(visibleCount <= 300, `自动展开不该超出行预算，实际 ${visibleCount}`);
  assert.ok(visibleCount > 1, "也不该退化成只剩一个孤零零的目录");
});
