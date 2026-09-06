import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { ChevronDown, ChevronRight, File, Folder, FolderOpen } from "lucide-react";
import type { PackageTreeRow } from "./packageContentTree";
import { resolveTreeKeyAction, resolveVisibleWindow } from "./packageContentTreeInteraction";
import type { InstallConfigCopy } from "./installConfigCopy";
import type { PackageContentEntry } from "./packageContentsTypes";

/*
 * 包内容树的渲染层（`#354` 切片 D4）。
 *
 * 两条约束决定了这里的结构：
 *
 * 1. **窗口化**。实测最大的包 7340 文件，展开一个大目录就可能一次放出几千行。行用绝对定位
 *    摆放，容器撑到总高度——这样 `role="tree"` 的直接子元素**全是** `treeitem`，不必为了
 *    撑高度插一层包装元素破坏 a11y 结构。
 * 2. **扁平 treeitem + `aria-level`**。窗口化之后 DOM 里本来就只有可见的一小段，嵌套
 *    `group` 无从谈起；扁平结构配 `aria-level`/`aria-setsize`/`aria-posinset` 是 ARIA 为这种
 *    情况准备的标准形态，位置信息在 `flattenVisibleRows` 里已经算好。
 */

const ROW_HEIGHT = 32;
const INDENT_PER_LEVEL = 18;
const OVERSCAN = 6;

type PackageContentTreeViewProps = {
  rows: readonly PackageTreeRow[];
  onToggle: (path: string) => void;
  copy: InstallConfigCopy;
};

export function PackageContentTreeView({ rows, onToggle, copy }: PackageContentTreeViewProps) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);
  const [activeIndex, setActiveIndex] = useState(0);
  // 只在键盘驱动焦点时才把 DOM 焦点搬过去；否则每次滚动重渲染都会把焦点抢回树里。
  const shouldFocusRef = useRef(false);

  useLayoutEffect(() => {
    const element = scrollRef.current;
    if (!element) {
      return;
    }

    const measure = () => setViewportHeight(element.clientHeight);
    measure();

    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  // 行数变了（展开/折叠/换包）之后旧的 activeIndex 可能越界。
  useEffect(() => {
    setActiveIndex((current) => Math.max(0, Math.min(current, rows.length - 1)));
  }, [rows.length]);

  const window_ = resolveVisibleWindow({
    scrollTop,
    viewportHeight,
    rowHeight: ROW_HEIGHT,
    rowCount: rows.length,
    overscan: OVERSCAN,
  });

  const ensureIndexVisible = useCallback((index: number) => {
    const element = scrollRef.current;
    if (!element) {
      return;
    }

    const top = index * ROW_HEIGHT;
    const bottom = top + ROW_HEIGHT;
    if (top < element.scrollTop) {
      element.scrollTop = top;
    } else if (bottom > element.scrollTop + element.clientHeight) {
      element.scrollTop = bottom - element.clientHeight;
    }
  }, []);

  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const action = resolveTreeKeyAction(event.key, { rows, activeIndex });
    if (!action) {
      return;
    }

    event.preventDefault();
    if (action.kind === "move") {
      shouldFocusRef.current = true;
      setActiveIndex(action.index);
      ensureIndexVisible(action.index);
      return;
    }

    shouldFocusRef.current = true;
    onToggle(action.path);
  };

  useEffect(() => {
    if (!shouldFocusRef.current) {
      return;
    }
    shouldFocusRef.current = false;
    const element = scrollRef.current?.querySelector<HTMLElement>(`[data-row-index="${activeIndex}"]`);
    element?.focus();
  });

  const visibleRows = rows.slice(window_.startIndex, window_.endIndex);

  return (
    <div
      className="install-config-tree"
      ref={scrollRef}
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
      onKeyDown={handleKeyDown}
    >
      <div
        className="install-config-tree__canvas"
        role="tree"
        aria-label={copy.page.treeAria}
        style={{ height: `${rows.length * ROW_HEIGHT}px` }}
      >
        {visibleRows.map((row, offset) => {
          const index = window_.startIndex + offset;
          return (
            <TreeRow
              key={row.node.path}
              row={row}
              index={index}
              isActive={index === activeIndex}
              onActivate={() => {
                shouldFocusRef.current = true;
                setActiveIndex(index);
              }}
              onToggle={onToggle}
              copy={copy}
            />
          );
        })}
      </div>
    </div>
  );
}

type TreeRowProps = {
  row: PackageTreeRow;
  index: number;
  isActive: boolean;
  onActivate: () => void;
  onToggle: (path: string) => void;
  copy: InstallConfigCopy;
};

function TreeRow({ row, index, isActive, onActivate, onToggle, copy }: TreeRowProps) {
  const { node } = row;
  const isDirectory = node.kind === "directory";

  return (
    <div
      className="install-config-tree__row"
      data-row-index={index}
      role="treeitem"
      aria-level={row.level}
      aria-setsize={row.setSize}
      aria-posinset={row.posInSet}
      // 文件是叶子：输出 aria-expanded 会让读屏把它读成可展开节点。
      aria-expanded={isDirectory ? row.isExpanded : undefined}
      aria-label={
        isDirectory
          ? copy.tree.directoryAria({ name: node.name, fileCount: node.stats.fileCount })
          : node.name
      }
      tabIndex={isActive ? 0 : -1}
      style={{
        top: `${index * ROW_HEIGHT}px`,
        paddingInlineStart: `${row.level * INDENT_PER_LEVEL}px`,
      }}
      onClick={() => {
        onActivate();
        if (isDirectory) {
          onToggle(node.path);
        }
      }}
    >
      <span className="install-config-tree__twisty" aria-hidden="true">
        {isDirectory ? row.isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} /> : null}
      </span>
      <span className="install-config-tree__icon" aria-hidden="true">
        {isDirectory ? row.isExpanded ? <FolderOpen size={15} /> : <Folder size={15} /> : <File size={15} />}
      </span>
      <span className="install-config-tree__name">{node.name}</span>

      {isDirectory ? (
        <span className="install-config-tree__meta">{copy.tree.fileCount(node.stats.fileCount)}</span>
      ) : (
        <FileFacts entry={node.entry} copy={copy} />
      )}
    </div>
  );
}

/*
 * 三条事实各自成一枚徽章，不合并成单一的「会不会装」。
 *
 * 合并必然说反话：拒绝清单当前只在重定向链路上强制执行，普通安装链路尚未套用，
 * 同一个文件在两条链路上的结局不同。所以这里只陈述命中了哪条事实，由玩家自己看。
 */
function FileFacts({
  entry,
  copy,
}: {
  entry: PackageContentEntry;
  copy: InstallConfigCopy;
}) {
  return (
    <span className="install-config-tree__facts">
      {!entry.installable ? (
        <span
          className="install-config-fact install-config-fact--neutral"
          title={copy.facts.notInstallable.detail}
        >
          {copy.facts.notInstallable.label}
        </span>
      ) : null}
      {entry.rejectedByGame ? (
        <span
          className="install-config-fact install-config-fact--warning"
          title={copy.facts.rejectedByGame.detail}
        >
          {copy.facts.rejectedByGame.label}
        </span>
      ) : null}
      {entry.excludedByPlayer ? (
        <span
          className="install-config-fact install-config-fact--accent"
          title={copy.facts.excludedByPlayer.detail}
        >
          {copy.facts.excludedByPlayer.label}
        </span>
      ) : null}
    </span>
  );
}
