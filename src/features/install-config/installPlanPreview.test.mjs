import assert from "node:assert/strict";
import { test } from "node:test";

import {
  classifyInstallPlanPreviewError,
  summarizeInstallTargets,
} from "./installPlanPreview.ts";

const actions = (...targetPaths) => targetPaths.map((targetPath) => ({ targetPath }));

/*
 * 内容根未定是**正常的待决状态**，不是错误。
 *
 * 玩家在同一个面板里已经看到内容根待指定了，再报一次「预览失败」既是噪音，又把一个
 * 正常状态说成出了问题。
 */
test("内容根未定与真失败必须分档", () => {
  assert.equal(
    classifyInstallPlanPreviewError({
      code: "install_planning_imported_mod_ambiguous_content_root",
    }),
    "needs-content-root",
  );
  assert.equal(
    classifyInstallPlanPreviewError({
      code: "install_planning_imported_mod_file_scan_unavailable",
    }),
    "generic",
  );
});

/*
 * 码字面量钉死。这一族没有契约门禁覆盖（`tauriContractCoverage` 只管新增命令），
 * 后端改了码而前端没跟，分档会**静默**退回 generic——界面把「去选内容根」说成「读不到包」。
 */
test("认的是后端那个全名，不是随手起的短名", () => {
  assert.equal(classifyInstallPlanPreviewError({ code: "ambiguous_content_root" }), "generic");
});

test("拿不到码的失败一律 generic，不猜", () => {
  for (const error of [null, undefined, "boom", new Error("boom"), {}, { code: 42 }]) {
    assert.equal(classifyInstallPlanPreviewError(error), "generic");
  }
});

test("空计划没有落点", () => {
  assert.deepEqual(summarizeInstallTargets([], 4), []);
});

test("落点按目标目录聚合并计数", () => {
  assert.deepEqual(
    summarizeInstallTargets(
      actions(
        "nativePC/wp/two/a.mod3",
        "nativePC/wp/two/b.mrl3",
        "nativePC/common/em/c.dtt",
      ),
      4,
    ),
    [
      { prefix: "nativePC/wp/two", fileCount: 2 },
      { prefix: "nativePC/common/em", fileCount: 1 },
    ],
  );
});

/*
 * 深度自适应：组数超上限就整体退一层。
 *
 * 固定深度对不同规模的包一个太粗一个太细——外观包集中在一层，全局资源包散布在几百个目录。
 * 这里 6 个各不相同的三段目录在 maxGroups=2 下必须退到两段，聚成 `nativePC/wp`。
 */
test("目录太多时整体退一层，直到装得下", () => {
  const spread = actions(
    "nativePC/wp/two/a.mod3",
    "nativePC/wp/swo/b.mod3",
    "nativePC/wp/bow/c.mod3",
    "nativePC/wp/lan/d.mod3",
    "nativePC/wp/gun/e.mod3",
    "nativePC/wp/ham/f.mod3",
  );

  assert.deepEqual(summarizeInstallTargets(spread, 2), [
    { prefix: "nativePC/wp", fileCount: 6 },
  ]);
  // 上限放宽到装得下时，就该停在更深、更有信息量的那一层。
  assert.equal(summarizeInstallTargets(spread, 6).length, 6);
});

/*
 * 取的是「组数 ≤ 上限」里**最深**的那一层，不是第一个可行的。
 * 停在浅层只会把不同的落点混成一堆，而那正是这块预览要回答的问题。
 */
test("装得下就尽量深，不停在浅层", () => {
  assert.deepEqual(
    summarizeInstallTargets(actions("nativePC/wp/two/bs_two012/mod/a.mod3"), 4),
    [{ prefix: "nativePC/wp/two/bs_two012/mod", fileCount: 1 }],
  );
});

/*
 * 深浅不一的路径：浅的那条到底了就保持原样，不会因为深的那条继续切分而被截断。
 */
test("深度不一的路径各自到底", () => {
  assert.deepEqual(
    summarizeInstallTargets(
      actions("nativePC/plugins/a.dll", "nativePC/wp/two/bs/b.mod3"),
      4,
    ),
    [
      { prefix: "nativePC/plugins", fileCount: 1 },
      { prefix: "nativePC/wp/two/bs", fileCount: 1 },
    ],
  );
});

/*
 * 顺序必须稳定：同一份计划两次渲染给出同一个顺序，否则重新预览一下顺序就变了，
 * 玩家会以为计划变了。文件多的在前，同数按路径。
 */
test("落点顺序稳定：先按文件数，再按路径", () => {
  const summary = summarizeInstallTargets(
    actions(
      "nativePC/b/x.mod3",
      "nativePC/a/y.mod3",
      "nativePC/c/z.mod3",
      "nativePC/c/w.mod3",
    ),
    4,
  );

  assert.deepEqual(summary.map((group) => group.prefix), [
    "nativePC/c",
    "nativePC/a",
    "nativePC/b",
  ]);
  assert.equal(summary[0].fileCount, 2);
});

/*
 * 上限是**闭区间**：正好等于上限的那一层留下，多一组才退。
 *
 * 这条是反向验证补出来的——原来的用例组数要么远超上限、要么远低于，把上限判据改成
 * `> maxGroups + 1` 一条都不转红。差一格的错误只有卡在边界上的用例抓得到。
 */
test("上限是闭区间：正好等于上限保留，多一组就退层", () => {
  const three = actions("nativePC/a/x.mod3", "nativePC/b/y.mod3", "nativePC/c/z.mod3");

  assert.equal(
    summarizeInstallTargets(three, 3).length,
    3,
    "正好等于上限，该停在这一层",
  );
  assert.deepEqual(
    summarizeInstallTargets(three, 2),
    [{ prefix: "nativePC", fileCount: 3 }],
    "多出一组就要整体退一层",
  );
});

test("落点计数之和等于计划里的动作数", () => {
  const plan = actions(
    "nativePC/wp/two/a.mod3",
    "nativePC/wp/two/b.mrl3",
    "nativePC/common/em/c.dtt",
    "nativePC/plugins/d.dll",
  );

  const total = summarizeInstallTargets(plan, 2).reduce(
    (sum, group) => sum + group.fileCount,
    0,
  );
  assert.equal(total, plan.length, "聚合不能丢文件，也不能重复计数");
});
