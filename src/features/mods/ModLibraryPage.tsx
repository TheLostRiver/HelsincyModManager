import { useMemo, useState } from "react";
import { CompactActionPanel } from "./CompactActionPanel";
import { LibraryToolbar } from "./LibraryToolbar";
import { ModPosterCard } from "./ModPosterCard";
import { modLibraryItems, type ModInstallStatus, type ModLibraryItem } from "./modsLibraryData";

type ModLibraryPageProps = {
  /** 快捷操作回调。点击行为由上层（路由/容器）接入业务，页面本身只负责展示与局部 UI 状态。 */
  onAction?: (actionId: string) => void;
};

const statusFilterByLabel: Partial<Record<string, ModInstallStatus>> = {
  已安装: "installed",
  已禁用: "disabled",
  存在冲突: "conflict",
};

function matchesActiveFilter(item: ModLibraryItem, activeFilter: string) {
  if (activeFilter === "全部") {
    return true;
  }

  const statusFilter = statusFilterByLabel[activeFilter];
  if (statusFilter) {
    return item.status === statusFilter;
  }

  return item.categoryLabels.includes(activeFilter);
}

export function ModLibraryPage({ onAction }: ModLibraryPageProps) {
  const [query, setQuery] = useState("");
  const [activeFilter, setActiveFilter] = useState<string>("全部");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  // 当前为展示层过滤。真实数据接入后应由仓储视图模型提供同名字段。
  const visibleItems = useMemo(() => {
    const keyword = query.trim().toLowerCase();
    return modLibraryItems.filter((item) => {
      const matchesKeyword = !keyword || item.name.toLowerCase().includes(keyword);
      return matchesKeyword && matchesActiveFilter(item, activeFilter);
    });
  }, [activeFilter, query]);

  const selectedCount = selectedIds.size;

  const toggleSelect = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const selectAll = () => {
    setSelectedIds(new Set(visibleItems.map((item) => item.id)));
  };

  const invertSelection = () => {
    setSelectedIds((prev) => {
      const next = new Set<string>();
      for (const item of visibleItems) {
        if (!prev.has(item.id)) {
          next.add(item.id);
        }
      }
      return next;
    });
  };

  const handleAction = (actionId: string) => {
    switch (actionId) {
      case "select-all":
        selectAll();
        break;
      case "invert":
        invertSelection();
        break;
      case "uninstall":
      case "reinstall":
        // 需要选中项的操作，清空选择前先回调上层；此处仅交由业务层处理。
        onAction?.(actionId);
        break;
      default:
        onAction?.(actionId);
        break;
    }
  };

  return (
    <section className="mod-library" aria-label="模组库">
      <div className="mod-library__body">
        <div className="mod-library__main">
          <LibraryToolbar
            query={query}
            activeFilter={activeFilter}
            onQueryChange={setQuery}
            onFilterChange={setActiveFilter}
          />

          {visibleItems.length === 0 ? (
            <div className="mod-library__empty" role="status">
              <strong>没有匹配的 Mod</strong>
              <p>试试调整搜索关键词或筛选条件。</p>
            </div>
          ) : (
            <div className="mod-grid" role="list">
              {visibleItems.map((item) => (
                <ModPosterCard
                  key={item.id}
                  item={item}
                  selected={selectedIds.has(item.id)}
                  onSelect={toggleSelect}
                />
              ))}
            </div>
          )}
        </div>

        <CompactActionPanel selectedCount={selectedCount} onAction={handleAction} />
      </div>
    </section>
  );
}
