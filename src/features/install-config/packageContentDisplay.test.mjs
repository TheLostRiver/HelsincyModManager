import assert from "node:assert/strict";
import { test } from "node:test";
import { buildPackageContentTree, flattenVisibleRows, indexNodesByPath } from "./packageContentTree.ts";
import { collectDirectoryPaths, projectPackageContentTree } from "./packageContentDisplay.ts";
import { toggleSelection } from "./packageContentSelection.ts";
import { resolveTreeKeyAction } from "./packageContentTreeInteraction.ts";

const entry = (path) => ({ packageFileId: path, targetPath: path, sizeBytes: 1, installable: true, rejectedByGame: false, excludedByPlayer: false });
const display = (tree, options = {}) => projectPackageContentTree(tree, { compact: true, query: "", filter: "all", excludedFiles: new Set(), ...options });
const rows = (tree) => flattenVisibleRows(tree, collectDirectoryPaths(tree));

test("compact chains retain original IDs, entries and selection scope with contiguous ARIA levels", () => {
  const paths = ["nativePC/wp/two/two003/mod/a.tex", "nativePC/wp/two/two003/mod/b.tex"];
  const tree = buildPackageContentTree(paths.map(entry));
  const projected = display(tree);
  assert.equal(projected[0].name, "nativePC / wp / two / two003 / mod");
  assert.equal(projected[0].path, "nativePC/wp/two/two003/mod");
  assert.deepEqual(rows(projected).map((row) => row.level), [1, 2, 2]);
  assert.equal(rows(projected)[1].node.entry, indexNodesByPath(tree).get(paths[0]).entry);
  assert.deepEqual([...toggleSelection(indexNodesByPath(tree).get(projected[0].path), new Set())], paths);
  assert.equal(tree[0].name, "nativePC");
});

test("folders with sibling files or branches never disappear into another directory", () => {
  const tree = buildPackageContentTree(["nativePC/readme.txt", "nativePC/wp/two/model.tex", "nativePC/armor/body/model.tex"].map(entry));
  const projected = display(tree);
  assert.equal(projected[0].name, "nativePC");
  assert.deepEqual(projected[0].children.map((node) => node.name), ["armor / body", "wp / two", "readme.txt"]);
  const filtered = display(tree, { query: "wp" });
  assert.equal(filtered[0].name, "nativePC", "filtering a sibling does not turn the original branch into a chain");
});

test("search normalizes the display query and retains ancestors without changing original selection", () => {
  const paths = ["nativePC/wp/a.TEX", "nativePC/wp/b.tex"];
  const tree = buildPackageContentTree(paths.map(entry));
  const projected = display(tree, { query: " NATIVEpc\\WP\\A.tex " });
  assert.deepEqual(rows(projected).filter((row) => row.node.kind === "file").map((row) => row.node.path), [paths[0]]);
  assert.equal(projected[0].stats.fileCount, 2);
  assert.deepEqual([...toggleSelection(indexNodesByPath(tree).get(projected[0].path), new Set())], paths);
  assert.deepEqual(display(tree, { query: "missing" }), []);
});

test("excluded view follows the current draft and leaves hidden files untouched", () => {
  const paths = ["pkg/folder/a.tex", "pkg/folder/b.tex"];
  const tree = buildPackageContentTree(paths.map(entry));
  const excluded = new Set([paths[1], "old/stale.tex"]);
  const projected = display(tree, { filter: "excluded", excludedFiles: excluded });
  assert.deepEqual(rows(projected).filter((row) => row.node.kind === "file").map((row) => row.node.path), [paths[1]]);
  assert.equal(tree[0].stats.excludedByPlayerCount, 0);
  assert.deepEqual([...excluded], [paths[1], "old/stale.tex"]);
});

test("full hierarchy and compact rows use correct parent navigation and the same leaf IDs", () => {
  const tree = buildPackageContentTree([entry("pkg/nativePC/wp/model.tex")]);
  const compactRows = rows(display(tree));
  const fullRows = rows(display(tree, { compact: false }));
  assert.equal(fullRows.length, 4);
  assert.equal(compactRows.length, 2);
  assert.equal(compactRows.at(-1).node.path, fullRows.at(-1).node.path);
  assert.deepEqual(resolveTreeKeyAction("ArrowLeft", { rows: compactRows, activeIndex: 1 }), { kind: "move", index: 0 });
  assert.deepEqual(resolveTreeKeyAction(" ", { rows: compactRows, activeIndex: 0 }), { kind: "toggle-selection", path: "pkg/nativePC/wp" });
});
