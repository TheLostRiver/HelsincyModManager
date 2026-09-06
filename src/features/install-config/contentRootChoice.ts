import type { PackageContentRoot } from "./packageContentsTypes";

/*
 * 内容根面板的两处判断（`#354` 切片 D4-3）。
 *
 * 抽出来单独测，是因为它们都踩在同一个陷阱上：**空串与 `null` 不是一回事**。
 * `fallback` 的 `path` 是空串（内容根就是沙箱根本身，已确定），`ambiguous` 的 `path` 是
 * `null`（还没有根，等玩家挑）。任何 `if (!path)` 式的合并都会把「已确定用沙箱根」
 * 显示成「还没选」，而这两句话对玩家的含义完全相反。
 */

/** 某个候选是不是当前生效的内容根。 */
export function isActiveContentRoot(contentRoot: PackageContentRoot, candidate: string): boolean {
  // 还没定的时候一个都不选中——如实呈现「等你挑」，而不是替玩家预选一个。
  if (contentRoot.kind === "ambiguous") {
    return false;
  }
  return contentRoot.path === candidate;
}

/** 说明句该用哪一句。 */
export type ContentRootDetailKind =
  /** 多个 `nativePC`，等玩家决定。 */
  | "ambiguous"
  /** 包里本来就没有别的层可选，根只能是沙箱根。 */
  | "fallback"
  /** 有别的层可选，但根目录是当前生效的那个。 */
  | "rootByChoice"
  /** 内容根是某个具体的子目录。 */
  | "path";

/**
 * @param candidates 顶层的 `PackageContents.candidates`（恒有的白名单全集），
 *   **不是** `contentRoot.candidates`（那个只在 `ambiguous` 时非空）。
 */
export function resolveContentRootDetailKind(
  contentRoot: PackageContentRoot,
  candidates: readonly string[],
): ContentRootDetailKind {
  /*
   * 先按 `kind` 把「还没定」分出去。它的 `path` 是 `null`，与根目录的空串在 falsy
   * 上不可区分——落进下面的值判断就会被显示成「根就是包的根目录」，与「等你挑」含义相反。
   */
  if (contentRoot.kind === "ambiguous") {
    return "ambiguous";
  }
  // 到这里 `path` 必有值。具体子目录直接报路径。
  if (contentRoot.path !== "") {
    return "path";
  }
  /*
   * 根目录生效时还要再分一次，判据是**这个包有没有别的层可选**。
   *
   * D4-3 原本把两者合成一句，理由是「对玩家是同一件事——都从包的根目录起算」。起算点确实
   * 一样，但那句文案还顺带断言了包的结构（「包里没有多余的包装目录」），而合集包选了根目录
   * 之后这句话就是假的：候选清单里明明还列着另外几层。D4-4 的合集包截图正是撞在这上面。
   *
   * 候选恒含沙箱根本身，所以「有别的层可选」是 `length > 1` 而不是 `> 0`。
   */
  return candidates.length > 1 ? "rootByChoice" : "fallback";
}
