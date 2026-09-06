import assert from "node:assert/strict";
import { test } from "node:test";

import { isActiveContentRoot, resolveContentRootDetailKind } from "./contentRootChoice.ts";

const ambiguous = { kind: "ambiguous", path: null, candidates: ["大剑", "太刀"] };
const fallback = { kind: "fallback", path: "", candidates: [] };
const single = { kind: "single", path: "大剑", candidates: [] };
/** 玩家显式选了沙箱根：后端报 `single` + 空串，而不是 `fallback`。 */
const singleAtRoot = { kind: "single", path: "", candidates: [] };

/*
 * 顶层 `PackageContents.candidates`（恒有的白名单全集），与 `contentRoot.candidates` 不是
 * 一回事。恒含沙箱根本身的空串。
 */
/** 没有包装目录的普通包：唯一能当内容根的就是根本身。 */
const onlyRoot = [""];
/** 合集包：根本身 ＋ 两个子包各自的那一层。 */
const rootAndTwoWrappers = ["", "怪物尺寸", "联机解锁"];

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
  assert.equal(resolveContentRootDetailKind(ambiguous, rootAndTwoWrappers), "ambiguous");
  assert.equal(resolveContentRootDetailKind(fallback, onlyRoot), "fallback");
  assert.equal(resolveContentRootDetailKind(single, ["", "大剑"]), "path");
  // 自动落到沙箱根与显式选中沙箱根，在「没有别的层可选」的包上是同一件事。
  assert.equal(resolveContentRootDetailKind(singleAtRoot, onlyRoot), "fallback");
});

/*
 * 这条钉的是「空串 ≠ null」。用 `!path` 合并的话，根目录（已确定）会和 ambiguous
 * （还没选）走进同一个分支，而这两句话对玩家的含义完全相反。
 */
test("空串与 null 走不同分支", () => {
  assert.notEqual(
    resolveContentRootDetailKind(fallback, onlyRoot),
    resolveContentRootDetailKind(ambiguous, rootAndTwoWrappers),
    "「根就是包的根目录」与「还没选根」不能显示成同一句",
  );
});

/*
 * D4-4 真机截图逮到的回归：合集包选了「包的根目录」之后，说明句显示成
 * 「包里没有多余的包装目录」——而候选清单就在正下方列着另外两层，这句话是假的。
 *
 * 起算点确实都是根目录，但 `fallbackDetail` 除此之外还断言了包的结构，那部分只对
 * 「没有别的层可选」的包成立。判据因此是候选数，不是 path 的值。
 */
test("根目录生效时，有没有别的层可选必须分档", () => {
  assert.equal(
    resolveContentRootDetailKind(singleAtRoot, rootAndTwoWrappers),
    "rootByChoice",
    "包里还有别的层可选时，不能说「包里没有多余的包装目录」",
  );
  assert.notEqual(
    resolveContentRootDetailKind(singleAtRoot, rootAndTwoWrappers),
    resolveContentRootDetailKind(singleAtRoot, onlyRoot),
    "同样是从根目录起算，包有没有别的层可选是两句话",
  );
});

/*
 * 候选**恒含**沙箱根本身，所以判据是 `> 1` 而不是 `> 0`。
 * 写成 `length > 0` 的话，普通包也会被说成「另有 0 层可以当作内容根」。
 */
test("候选只有根目录本身时仍是 fallback", () => {
  assert.equal(resolveContentRootDetailKind(fallback, [""]), "fallback");
  assert.equal(resolveContentRootDetailKind(singleAtRoot, [""]), "fallback");
});

/*
 * 具体子目录这一档不受候选数影响：报的是「从 X 开始算」，本来就没断言包的结构。
 */
test("选中具体子目录时，候选多少都报路径", () => {
  assert.equal(resolveContentRootDetailKind(single, ["", "大剑"]), "path");
  assert.equal(resolveContentRootDetailKind(single, ["", "大剑", "太刀"]), "path");
});
