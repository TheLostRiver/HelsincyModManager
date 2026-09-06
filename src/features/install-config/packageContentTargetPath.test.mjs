import assert from "node:assert/strict";
import { test } from "node:test";

import {
  isNoteworthyTargetPathState,
  resolveTargetPathState,
} from "./packageContentTargetPath.ts";

/** 内容根之下、路径也被游戏接受的常态文件。 */
const installable = { targetPath: "nativePC/wp/two/two003.mod3", installable: true };
/** 内容根之下，但路径不以允许的安装根打头——包装目录里的说明文件就是这一档。 */
const notAccepted = { targetPath: "说明.txt", installable: false };
/** 内容根之外：后端算不出目标路径。 */
const outside = { targetPath: null, installable: false };

test("内容根之下且游戏接受的文件报 resolved，并带着算出来的路径", () => {
  assert.deepEqual(resolveTargetPathState(installable, "single"), {
    kind: "resolved",
    targetPath: "nativePC/wp/two/two003.mod3",
  });
});

test("算得出路径但游戏不收，仍要把算出来的路径带上", () => {
  assert.deepEqual(resolveTargetPathState(notAccepted, "single"), {
    kind: "pathNotAccepted",
    targetPath: "说明.txt",
  });
});

/*
 * 本模块存在的理由：同样是 `targetPath === null`，两种成因对玩家的含义完全不同。
 *
 * 「内容根未定」的出路是去挑一个根（合集包的常态），「不在内容根之下」的出路是换一个
 * 更浅的根。报成同一句「不在安装范围」，等于把这两条出路都藏起来。
 */
test("同一个 null，内容根未定与不在内容根之下必须分档", () => {
  assert.equal(resolveTargetPathState(outside, "ambiguous").kind, "contentRootUndecided");
  assert.equal(resolveTargetPathState(outside, "single").kind, "outsideContentRoot");
  assert.equal(resolveTargetPathState(outside, "fallback").kind, "outsideContentRoot");

  assert.notEqual(
    resolveTargetPathState(outside, "ambiguous").kind,
    resolveTargetPathState(outside, "single").kind,
    "两种成因的出路不同，不能合并成一句",
  );
});

/*
 * 内容根未定时后端**整包**都给不出 targetPath（`content_root_prefix` 为 None），
 * 所以这一档与「这个文件在不在某个根之下」无关——包里每一个文件都走这里。
 */
test("内容根未定时，包内每个文件都是 contentRootUndecided", () => {
  for (const entry of [outside, { targetPath: null, installable: false }]) {
    assert.equal(resolveTargetPathState(entry, "ambiguous").kind, "contentRootUndecided");
  }
});

/*
 * 钉「空串 ≠ null」。
 *
 * 后端不会给出空串 targetPath（文件路径至少一段），但整个 feature 对 falsy 合并是硬纪律：
 * 写成 `!entry.targetPath` 的话，空串会被误判成「算不出路径」，而它其实是一个算出来了的、
 * 只是不被接受的路径。
 */
test("空串 targetPath 不走 null 分支", () => {
  const emptyTarget = { targetPath: "", installable: false };

  assert.deepEqual(resolveTargetPathState(emptyTarget, "single"), {
    kind: "pathNotAccepted",
    targetPath: "",
  });
  assert.notEqual(
    resolveTargetPathState(emptyTarget, "ambiguous").kind,
    "contentRootUndecided",
    "空串是算出来的路径，不是「还没算出来」",
  );
});

/*
 * `installable` 一律听后端的。
 *
 * 前端不照着「以 nativePC 打头」再判一遍——那个判据在 `#292` 之后含大小写归一化，
 * 复制一份必然分叉。这里用一个前端「看起来该可以装」的路径配 `installable: false`，
 * 钉住前端不会自作主张翻案。
 */
test("installable 听后端的，前端不按路径长相翻案", () => {
  const backendSaysNo = { targetPath: "nativePC/wp/two/two003.mod3", installable: false };

  assert.equal(resolveTargetPathState(backendSaysNo, "single").kind, "pathNotAccepted");
});

test("只有 resolved 不值得报徽章，其余三档都要报", () => {
  assert.equal(isNoteworthyTargetPathState({ kind: "resolved", targetPath: "a" }), false);

  for (const kind of ["pathNotAccepted", "outsideContentRoot", "contentRootUndecided"]) {
    assert.equal(
      isNoteworthyTargetPathState({ kind, targetPath: "a" }),
      true,
      `${kind} 该报徽章`,
    );
  }
});
