import assert from "node:assert/strict";
import { test } from "node:test";

import { buildPackageContentTree } from "./packageContentTree.ts";
import {
  computeDirectorySelection,
  isSameSelection,
  selectableFileIdsUnder,
  toggleSelection,
} from "./packageContentSelection.ts";

function entry(packageFileId, overrides = {}) {
  return {
    packageFileId,
    sizeBytes: 1,
    targetPath: packageFileId,
    installable: true,
    rejectedByGame: false,
    excludedByPlayer: false,
    ...overrides,
  };
}

function findNode(nodes, path) {
  // 递归部分必须能「没找到就返回 null」，否则搜完第一个兄弟子树就抛，
  // 根本轮不到第二个——根层不止一个目录时这个夹具会假报错。
  const search = (list) => {
    for (const node of list) {
      if (node.path === path) {
        return node;
      }
      if (node.kind === "directory") {
        const found = search(node.children);
        if (found) {
          return found;
        }
      }
    }
    return null;
  };

  const found = search(nodes);
  assert.ok(found, `node not found: ${path}`);
  return found;
}

test("没有排除项时每个目录都是全选", () => {
  const nodes = buildPackageContentTree([entry("pkg/a.txt"), entry("pkg/nested/b.txt")]);

  const states = computeDirectorySelection(nodes, new Set());

  assert.equal(states.get("pkg"), "checked");
  assert.equal(states.get("pkg/nested"), "checked");
});

test("子孙全被排除时目录是全不选，部分被排除时是半选", () => {
  const nodes = buildPackageContentTree([
    entry("pkg/a.txt"),
    entry("pkg/nested/b.txt"),
    entry("pkg/nested/c.txt"),
  ]);

  const partial = computeDirectorySelection(nodes, new Set(["pkg/nested/b.txt"]));
  assert.equal(partial.get("pkg/nested"), "indeterminate");
  assert.equal(partial.get("pkg"), "indeterminate", "半选要一路冒到祖先");

  const nestedAll = computeDirectorySelection(
    nodes,
    new Set(["pkg/nested/b.txt", "pkg/nested/c.txt"]),
  );
  assert.equal(nestedAll.get("pkg/nested"), "unchecked");
  assert.equal(nestedAll.get("pkg"), "indeterminate", "pkg 下的 a.txt 还留着");

  const everything = computeDirectorySelection(
    nodes,
    new Set(["pkg/a.txt", "pkg/nested/b.txt", "pkg/nested/c.txt"]),
  );
  assert.equal(everything.get("pkg"), "unchecked");
});

test("不可安装的文件不算进三态——它本来就进不了计划", () => {
  const nodes = buildPackageContentTree([
    entry("pkg/ok.txt"),
    entry("pkg/outside.txt", { installable: false, targetPath: null }),
  ]);

  // 只勾掉唯一可选的那个，目录就该是全不选，而不是因为还有个装不了的文件卡在半选。
  const states = computeDirectorySelection(nodes, new Set(["pkg/ok.txt"]));
  assert.equal(states.get("pkg"), "unchecked");
});

test("整个目录都装不了时它没有勾选框——不渲染灰着的勾选框", () => {
  const nodes = buildPackageContentTree([
    entry("docs/readme.txt", { installable: false, targetPath: null }),
    entry("pkg/a.txt"),
  ]);

  const states = computeDirectorySelection(nodes, new Set());

  assert.equal(states.has("docs"), false, "没有可勾选文件的目录不该出现在三态表里");
  assert.equal(states.get("pkg"), "checked");
});

test("被游戏拒绝的文件照常可勾选——普通安装链路仍会装它", () => {
  const nodes = buildPackageContentTree([
    entry("pkg/tool.exe", { rejectedByGame: true }),
  ]);

  const states = computeDirectorySelection(nodes, new Set());
  assert.equal(states.get("pkg"), "checked");

  const file = findNode(nodes, "pkg/tool.exe");
  assert.deepEqual([...toggleSelection(file, new Set())], ["pkg/tool.exe"]);
});

test("切换文件就是在排除集合里进出", () => {
  const nodes = buildPackageContentTree([entry("pkg/a.txt"), entry("pkg/b.txt")]);
  const file = findNode(nodes, "pkg/a.txt");

  const excluded = toggleSelection(file, new Set());
  assert.deepEqual([...excluded], ["pkg/a.txt"]);

  const restored = toggleSelection(file, excluded);
  assert.deepEqual([...restored], []);
});

test("切换不可安装的文件是空操作", () => {
  const nodes = buildPackageContentTree([
    entry("pkg/outside.txt", { installable: false, targetPath: null }),
  ]);
  const file = findNode(nodes, "pkg/outside.txt");

  assert.deepEqual([...toggleSelection(file, new Set())], []);
});

test("切换目录级联到整棵子树", () => {
  const nodes = buildPackageContentTree([
    entry("pkg/a.txt"),
    entry("pkg/nested/b.txt"),
    entry("pkg/nested/deep/c.txt"),
    entry("other/d.txt"),
  ]);
  const pkg = findNode(nodes, "pkg");

  const excluded = toggleSelection(pkg, new Set());

  assert.deepEqual(
    [...excluded].sort(),
    ["pkg/a.txt", "pkg/nested/b.txt", "pkg/nested/deep/c.txt"],
  );
  // 只影响自己的子树，兄弟目录不动。
  assert.equal(excluded.has("other/d.txt"), false);
});

test("半选的目录点一下是全选，不是全不选", () => {
  const nodes = buildPackageContentTree([entry("pkg/a.txt"), entry("pkg/b.txt")]);
  const pkg = findNode(nodes, "pkg");

  const partial = new Set(["pkg/a.txt"]);
  assert.equal(computeDirectorySelection(nodes, partial).get("pkg"), "indeterminate");

  // 玩家在半选状态下点勾选框，想要的是「都要」。
  assert.deepEqual([...toggleSelection(pkg, partial)], []);
});

test("全不选的目录点一下恢复全选", () => {
  const nodes = buildPackageContentTree([entry("pkg/a.txt"), entry("pkg/b.txt")]);
  const pkg = findNode(nodes, "pkg");

  const allExcluded = new Set(["pkg/a.txt", "pkg/b.txt"]);
  assert.deepEqual([...toggleSelection(pkg, allExcluded)], []);
});

test("级联时不把装不了的文件卷进排除集合", () => {
  const nodes = buildPackageContentTree([
    entry("pkg/a.txt"),
    entry("pkg/outside.txt", { installable: false, targetPath: null }),
  ]);
  const pkg = findNode(nodes, "pkg");

  assert.deepEqual([...toggleSelection(pkg, new Set())], ["pkg/a.txt"]);
});

test("selectableFileIdsUnder 只收可安装的子孙文件", () => {
  const nodes = buildPackageContentTree([
    entry("pkg/a.txt"),
    entry("pkg/nested/b.txt"),
    entry("pkg/nested/skip.txt", { installable: false, targetPath: null }),
  ]);

  assert.deepEqual(selectableFileIdsUnder(findNode(nodes, "pkg")).sort(), [
    "pkg/a.txt",
    "pkg/nested/b.txt",
  ]);
});

test("比较排除集合看内容不看引用——勾掉再勾回来不算改动", () => {
  assert.equal(isSameSelection(new Set(), []), true);
  assert.equal(isSameSelection(new Set(["a"]), ["a"]), true);
  assert.equal(isSameSelection(new Set(["a", "b"]), ["b", "a"]), true, "顺序无关");
  assert.equal(isSameSelection(new Set(["a"]), []), false);
  assert.equal(isSameSelection(new Set(), ["a"]), false);
  assert.equal(isSameSelection(new Set(["a"]), ["b"]), false);
  // 长度相同但内容不同：靠 size 比较会漏判，必须逐项查。
  assert.equal(isSameSelection(new Set(["a", "b"]), ["a", "c"]), false);
});
