import type { PackageContentEntry, PackageContentRootKind } from "./packageContentsTypes";

/*
 * 「这个文件会装到哪」的分档（`#354` 切片 D4-4）。
 *
 * D4-1 只报了一条笼统的「不在安装范围」，可它掩盖了三种成因完全不同的情况——
 * 其中两种玩家**有办法可想**，第三种没有：
 *
 * - 内容根未定：挑一个内容根就有目标路径了（合集包的常态）。
 * - 不在内容根之下：换一个更浅的内容根就能把它纳进来。
 * - 路径不被本游戏接受：内容根之下的 readme、说明图之类，怎么挑都装不进去。
 *
 * 合并成一句「不在安装范围」等于把前两种的出路藏起来，而那正是玩家打开这个面板要找的。
 */

/**
 * 一个文件的目标路径分档。
 *
 * `resolved` 与 `pathNotAccepted` 都带着算出来的 `targetPath`——**能算出路径**与
 * **这个路径能不能装**是两件事，后端也是分两步判的（先剥内容根前缀，再过 game adapter
 * 的允许根）。带上它，界面才能如实告诉玩家「算出来是这个，但本游戏不收」。
 */
export type TargetPathState =
  | { kind: "resolved"; targetPath: string }
  | { kind: "pathNotAccepted"; targetPath: string }
  | { kind: "outsideContentRoot" }
  | { kind: "contentRootUndecided" };

/**
 * 判别一个文件的目标路径分档。
 *
 * 两种 `targetPath === null` 的成因**在文件这一级分不开**，只能靠包级的内容根状态区分：
 * 内容根未定时后端整包都给不出目标路径（`package_contents_query.rs` 里
 * `content_root_prefix` 为 `None`），内容根已定时 `null` 才意味着这个文件在它之外。
 *
 * 判 `null` 而不是判 falsy：内容根为沙箱根本身时它的相对路径是**空串**，`targetPath`
 * 虽不会取到空串，但整个 feature 对「空串 ≠ null」是一条硬纪律，这里不开先例。
 */
export function resolveTargetPathState(
  entry: Pick<PackageContentEntry, "targetPath" | "installable">,
  contentRootKind: PackageContentRootKind,
): TargetPathState {
  if (entry.targetPath === null) {
    return contentRootKind === "ambiguous"
      ? { kind: "contentRootUndecided" }
      : { kind: "outsideContentRoot" };
  }

  /*
   * `installable` 由后端说了算，前端不自己判路径。
   *
   * 它的判据是 game adapter 声明的允许安装根，那是后端对着适配器做的判断，前端推演不出
   * ——照着「以 nativePC 打头」写一遍，就会在 `#292` 那种大小写归一化上分叉。
   */
  return entry.installable
    ? { kind: "resolved", targetPath: entry.targetPath }
    : { kind: "pathNotAccepted", targetPath: entry.targetPath };
}

/**
 * 这一档要不要在行上报一枚事实徽章。
 *
 * `resolved` 是常态，不报——每一行都挂一枚「正常」徽章只会淹没真正需要注意的那几行。
 *
 * 写成类型谓词，好让调用方在早退之后拿到收窄的类型：将来多一档而忘了写文案时，
 * 这里会在 `tsc` 上转红，而不是在界面上默默漏一枚徽章。
 */
export function isNoteworthyTargetPathState(
  state: TargetPathState,
): state is Exclude<TargetPathState, { kind: "resolved" }> {
  return state.kind !== "resolved";
}
