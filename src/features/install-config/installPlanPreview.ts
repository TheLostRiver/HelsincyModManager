/*
 * 带 `.ts` 扩展名是**故意**的：这个模块要能被 `node --test` 直接加载，而 Node 的 ESM 解析
 * 不做无扩展名补全。`allowImportingTsExtensions` 已开，仓库里 `features/about/`
 * 等处也是这么写的。同目录其余纯函数模块只做 `import type`（编译期擦除）所以没踩到这条。
 */
import { packageContentsErrorCode } from "./packageContentsError.ts";

/*
 * 安装计划预览的模型层（`#354` 切片 D4-4b）。
 *
 * 这块预览回答的是树回答不了的三个问题：
 *
 * 1. **实际会装几个文件。** 摘要条的 `installableCount` 是「能不能装」，**不减**玩家勾掉的
 *    （那是 D3 拍板的口径，勾选是「要不要」）。计划里的动作数才是「实际会执行几条」，
 *    这是全界面唯一报告它的地方。
 * 2. **装到游戏目录的哪些位置。** 树显示的是**包内**结构，落点要剥掉内容根前缀才看得出。
 * 3. **前置条件过不过。** 游戏目录配没配好、可不可写，树完全不知道。
 */

/** 包内有多个 `nativePC` 且玩家还没挑时，建计划这一步直接失败（`install_commands.rs:535`）。 */
const AMBIGUOUS_CONTENT_ROOT_CODE = "install_planning_imported_mod_ambiguous_content_root";

export type InstallPlanPreviewFailure =
  /**
   * 内容根未定。
   *
   * **不当错误报**：玩家在同一个面板里已经看到内容根待指定了，再报一次「预览失败」是噪音，
   * 而且会把一个正常的待决状态说成出了问题。这一档要显示成「先选内容根」。
   */
  | "needs-content-root"
  /** 其余失败：沙箱读不到、Mod 不存在、game adapter 缺失…… 只能重试。 */
  | "generic";

export function classifyInstallPlanPreviewError(error: unknown): InstallPlanPreviewFailure {
  return packageContentsErrorCode(error) === AMBIGUOUS_CONTENT_ROOT_CODE
    ? "needs-content-root"
    : "generic";
}

/** 一组落点：`prefix` 之下会装 `fileCount` 个文件。 */
export type InstallTargetGroup = {
  prefix: string;
  fileCount: number;
};

/**
 * 把计划动作按目标目录聚合成不超过 `maxGroups` 组。
 *
 * **深度自适应**，与树的展开预算同源：先试最深的目录层级，组数超了就整体退一层，直到
 * 装得下。固定深度不行——外观包集中在 `nativePC/wp/two` 这一层，而全局资源包散布在几百个
 * 目录里，同一个深度对两者一个太粗一个太细。
 *
 * 取「组数 ≤ 上限」里**最深**的那一层：深一层就多一分信息，浅一层只是把不同的落点混成一堆。
 */
export function summarizeInstallTargets(
  actions: readonly { targetPath: string }[],
  maxGroups: number,
): InstallTargetGroup[] {
  if (actions.length === 0 || maxGroups <= 0) {
    return [];
  }

  /*
   * 去掉最后一段（文件名）得到目录。
   *
   * `targetPath` 至少两段——它得以 game adapter 的某个允许安装根打头才进得了计划，
   * 而那些根都是目录名。所以目录段不会为空。
   */
  const directories = actions.map((action) => action.targetPath.split("/").slice(0, -1));
  const maxDepth = directories.reduce((deepest, segments) => Math.max(deepest, segments.length), 0);

  // 组数随深度单调不减，所以从浅往深走，第一次超限就停在上一层。
  let depth = 1;
  for (let candidate = 1; candidate <= maxDepth; candidate += 1) {
    const distinct = new Set(directories.map((segments) => segments.slice(0, candidate).join("/")));
    if (distinct.size > maxGroups) {
      break;
    }
    depth = candidate;
  }

  const counts = new Map<string, number>();
  for (const segments of directories) {
    const prefix = segments.slice(0, depth).join("/");
    counts.set(prefix, (counts.get(prefix) ?? 0) + 1);
  }

  /*
   * 文件多的排前面，同数按路径排——同一份计划两次渲染必须给出同一个顺序，
   * 否则重新预览一下顺序就变了，玩家会以为计划变了。
   */
  return [...counts]
    .map(([prefix, fileCount]) => ({ prefix, fileCount }))
    .sort((left, right) =>
      right.fileCount - left.fileCount || left.prefix.localeCompare(right.prefix),
    );
}
