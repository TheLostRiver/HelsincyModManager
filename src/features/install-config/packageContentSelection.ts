import type { PackageTreeDirectoryNode, PackageTreeNode } from "./packageContentTree";

/*
 * 逐文件勾选的选择模型（`#354` 切片 D3 的前端一半）。
 *
 * **存的是「排除集合」不是「包含集合」**，与后端契约一致：空集合 = 整包都装 = 恒等变换，
 * `plan_hash` 与 facts 逐字不变。反过来用包含集合的话，包重新解压出的新文件会**静默不装**
 * ——而少装一个文件装完是不报错的。
 *
 * **不可安装的文件不参与勾选**。`installable: false` 意味着它不在内容根之下、或路径不被本
 * 游戏接受，本来就进不了安装计划，勾不勾都一样。把它们塞进排除集合只是噪音。UI 那边也
 * **不给它们渲染 disabled 的勾选框**——一个灰着的勾选框会暗示「想办法就能启用」，而这里
 * 根本没有「想办法」可言。
 *
 * 注意 `rejectedByGame` **不**影响可勾选性：拒绝清单目前只在重定向链路上强制执行，普通安装
 * 链路仍会装，所以玩家必须能勾掉它。
 */

export type SelectionState = "checked" | "unchecked" | "indeterminate";

/**
 * 每个目录的三态。
 *
 * 只包含**含有可勾选文件**的目录——一个目录若整个装不了，它就没有勾选框可言，
 * 不在这张表里。文件不进这张表：文件状态直接看它在不在排除集合里。
 */
export function computeDirectorySelection(
  nodes: readonly PackageTreeNode[],
  excluded: ReadonlySet<string>,
): Map<string, SelectionState> {
  const states = new Map<string, SelectionState>();

  const walk = (node: PackageTreeNode): { selectable: number; excluded: number } => {
    if (node.kind === "file") {
      if (!node.entry.installable) {
        return { selectable: 0, excluded: 0 };
      }
      return { selectable: 1, excluded: excluded.has(node.path) ? 1 : 0 };
    }

    let selectableCount = 0;
    let excludedCount = 0;
    for (const child of node.children) {
      const result = walk(child);
      selectableCount += result.selectable;
      excludedCount += result.excluded;
    }

    if (selectableCount > 0) {
      states.set(
        node.path,
        excludedCount === 0
          ? "checked"
          : excludedCount === selectableCount
            ? "unchecked"
            : "indeterminate",
      );
    }

    return { selectable: selectableCount, excluded: excludedCount };
  };

  for (const node of nodes) {
    walk(node);
  }

  return states;
}

/**
 * 切换一个节点之后的新排除集合。
 *
 * 目录按**级联**处理：整棵子树跟着走。三态里的 `indeterminate` 点一下是**全选**而不是
 * 全不选——玩家在半选状态下点勾选框，想要的是「都要」，这是勾选框的通行语义。
 */
export function toggleSelection(
  node: PackageTreeNode,
  excluded: ReadonlySet<string>,
): Set<string> {
  const next = new Set(excluded);

  if (node.kind === "file") {
    // 不可安装的文件没有勾选框，也就没有「切换」可言。
    if (!node.entry.installable) {
      return next;
    }
    if (next.has(node.path)) {
      next.delete(node.path);
    } else {
      next.add(node.path);
    }
    return next;
  }

  const fileIds = selectableFileIdsUnder(node);
  if (fileIds.length === 0) {
    return next;
  }

  const noneExcluded = fileIds.every((fileId) => !next.has(fileId));
  if (noneExcluded) {
    // 全选 → 全不选
    for (const fileId of fileIds) {
      next.add(fileId);
    }
  } else {
    // 全不选或半选 → 全选
    for (const fileId of fileIds) {
      next.delete(fileId);
    }
  }

  return next;
}

/** 目录之下所有**可勾选**文件的 `packageFileId`。 */
export function selectableFileIdsUnder(directory: PackageTreeDirectoryNode): string[] {
  const fileIds: string[] = [];

  const walk = (nodes: readonly PackageTreeNode[]): void => {
    for (const node of nodes) {
      if (node.kind === "directory") {
        walk(node.children);
      } else if (node.entry.installable) {
        fileIds.push(node.path);
      }
    }
  };

  walk(directory.children);
  return fileIds;
}

/**
 * 排除集合是否与后端记录的一致。
 *
 * 用来判断「有没有未保存的改动」。比较的是集合内容而不是引用——每次勾选都会造一个新
 * Set，靠引用判断会把「勾掉再勾回来」误报成有改动。
 */
export function isSameSelection(left: ReadonlySet<string>, right: readonly string[]): boolean {
  if (left.size !== right.length) {
    return false;
  }
  return right.every((fileId) => left.has(fileId));
}
