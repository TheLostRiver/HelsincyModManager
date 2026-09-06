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
  /** 内容根就是沙箱根本身。 */
  | "fallback"
  /** 内容根是某个具体的子目录。 */
  | "path";

export function resolveContentRootDetailKind(contentRoot: PackageContentRoot): ContentRootDetailKind {
  /*
   * 先按 `kind` 把「还没定」分出去。它的 `path` 是 `null`，与 `fallback` 的空串在 falsy
   * 上不可区分——落进下面的值判断就会被显示成「根就是包的根目录」，与「等你挑」含义相反。
   */
  if (contentRoot.kind === "ambiguous") {
    return "ambiguous";
  }
  /*
   * 到这里 `path` 必有值。按**值**而不是 `kind` 分档：玩家显式选定沙箱根时后端报的是
   * `single` + 空串而不是 `fallback`，但对玩家是同一件事——安装路径从包的根目录起算。
   */
  return contentRoot.path === "" ? "fallback" : "path";
}
