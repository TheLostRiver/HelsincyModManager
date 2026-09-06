import assert from "node:assert/strict";
import { test } from "node:test";

import { isActiveContentRoot, resolveContentRootDetailKind } from "./contentRootChoice.ts";

const ambiguous = { kind: "ambiguous", path: null, candidates: ["大剑", "太刀"] };
const fallback = { kind: "fallback", path: "", candidates: [] };
const single = { kind: "single", path: "大剑", candidates: [] };
/** 玩家显式选了沙箱根：后端报 `single` + 空串，而不是 `fallback`。 */
const singleAtRoot = { kind: "single", path: "", candidates: [] };

test("已确定的根与同名候选对上", () => {
  assert.equal(isActiveContentRoot(single, "大剑"), true);
  assert.equal(isActiveContentRoot(single, "太刀"), false);
});

test("待指定时一个候选都不选中——不替玩家预选", () => {
  assert.equal(isActiveContentRoot(ambiguous, "大剑"), false);
  assert.equal(isActiveContentRoot(ambiguous, "太刀"), false);
  assert.equal(isActiveContentRoot(ambiguous, ""), false);
});

test("空串候选（包的根目录）能被正确选中，不被当成「没有值」", () => {
  assert.equal(isActiveContentRoot(fallback, ""), true);
  assert.equal(isActiveContentRoot(singleAtRoot, ""), true);
  // 反面：根目录生效时，具名候选不该跟着高亮。
  assert.equal(isActiveContentRoot(fallback, "大剑"), false);
});

test("说明句按内容根的值分档，不按 kind", () => {
  assert.equal(resolveContentRootDetailKind(ambiguous), "ambiguous");
  assert.equal(resolveContentRootDetailKind(fallback), "fallback");
  assert.equal(resolveContentRootDetailKind(single), "path");
  // 显式选中沙箱根与自动落到沙箱根，对玩家是同一件事，说明句也该一样。
  assert.equal(resolveContentRootDetailKind(singleAtRoot), "fallback");
});

/*
 * 这条钉的是「空串 ≠ null」。用 `!path` 合并的话，fallback（已确定用包的根目录）
 * 会和 ambiguous（还没选）走进同一个分支，而这两句话对玩家的含义完全相反。
 */
test("空串与 null 走不同分支", () => {
  assert.notEqual(
    resolveContentRootDetailKind(fallback),
    resolveContentRootDetailKind(ambiguous),
    "「根就是包的根目录」与「还没选根」不能显示成同一句",
  );
});
