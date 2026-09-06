import type { PackageTreeRow } from "./packageContentTree";

/*
 * 树视图的两块易错逻辑：可见窗口的区间计算，与 WAI-ARIA tree 的键盘语义。
 *
 * 抽成纯函数是为了能直接喂输入断言输出——这两块塞进组件里就只能靠人眼看，而它们恰恰是
 * 「差一行」「方向键跑到隐藏行上」这类问题的高发区。
 */

export type VisibleWindow = {
  startIndex: number;
  /** 不含（exclusive）。 */
  endIndex: number;
};

/**
 * 当前该渲染哪一段行。
 *
 * 实测最大的包 7340 文件：玩家展开一个大目录就可能一次放出几千行，全量挂进 DOM 会卡住主
 * 线程。默认折叠先压掉大部分，窗口化兜住剩下的极端情况——两端用的是同一套可见行数组。
 */
export function resolveVisibleWindow(input: {
  scrollTop: number;
  viewportHeight: number;
  rowHeight: number;
  rowCount: number;
  overscan: number;
}): VisibleWindow {
  const { scrollTop, viewportHeight, rowHeight, rowCount, overscan } = input;

  if (rowCount <= 0 || rowHeight <= 0 || viewportHeight <= 0) {
    return { startIndex: 0, endIndex: 0 };
  }

  const firstVisible = Math.floor(scrollTop / rowHeight);
  const visibleCount = Math.ceil(viewportHeight / rowHeight);

  // clamp 到 [0, rowCount]：滚动条在惯性回弹时会给出负的或超界的 scrollTop。
  const startIndex = Math.max(0, Math.min(rowCount, firstVisible - overscan));
  const endIndex = Math.max(startIndex, Math.min(rowCount, firstVisible + visibleCount + overscan));

  return { startIndex, endIndex };
}

export type TreeKeyAction =
  | { kind: "move"; index: number }
  | { kind: "expand"; path: string }
  | { kind: "collapse"; path: string };

/**
 * 把一次按键翻译成树的动作，`null` 表示不处理（调用方因此不该 `preventDefault`）。
 *
 * 语义照 WAI-ARIA 的 tree pattern：右键在折叠目录上是展开、在已展开目录上是走进第一个子级；
 * 左键在展开目录上是折叠、在其余节点上是回到父级。索引一律基于**可见行**数组，所以永远
 * 不会把焦点送到一个折叠着的、DOM 里不存在的节点上。
 */
export function resolveTreeKeyAction(
  key: string,
  input: { rows: readonly PackageTreeRow[]; activeIndex: number },
): TreeKeyAction | null {
  const { rows, activeIndex } = input;

  if (rows.length === 0) {
    return null;
  }

  const current = rows[activeIndex];

  switch (key) {
    case "ArrowDown":
      return activeIndex < rows.length - 1 ? { kind: "move", index: activeIndex + 1 } : null;
    case "ArrowUp":
      return activeIndex > 0 ? { kind: "move", index: activeIndex - 1 } : null;
    case "Home":
      return { kind: "move", index: 0 };
    case "End":
      return { kind: "move", index: rows.length - 1 };
    case "ArrowRight": {
      if (!current || current.node.kind !== "directory") {
        return null;
      }
      if (!current.isExpanded) {
        return { kind: "expand", path: current.node.path };
      }
      // 已展开：走进第一个子级。它就是可见行里紧挨着的下一行。
      return activeIndex < rows.length - 1 ? { kind: "move", index: activeIndex + 1 } : null;
    }
    case "ArrowLeft": {
      if (!current) {
        return null;
      }
      if (current.node.kind === "directory" && current.isExpanded) {
        return { kind: "collapse", path: current.node.path };
      }
      const parentIndex = findParentIndex(rows, activeIndex);
      return parentIndex === null ? null : { kind: "move", index: parentIndex };
    }
    default:
      return null;
  }
}

/** 往上找第一个层级更浅的行；根层节点没有父级，返回 `null`。 */
function findParentIndex(rows: readonly PackageTreeRow[], activeIndex: number): number | null {
  const current = rows[activeIndex];
  if (!current || current.level <= 1) {
    return null;
  }

  for (let index = activeIndex - 1; index >= 0; index -= 1) {
    if (rows[index].level < current.level) {
      return index;
    }
  }

  return null;
}
