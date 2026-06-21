import { useMemo, useState, type CSSProperties } from "react";
import { BackToTopButton } from "./BackToTopButton";
import { CompactActionPanel } from "./CompactActionPanel";
import { LibraryToolbar } from "./LibraryToolbar";
import { ModPosterCard } from "./ModPosterCard";
import { getModLibraryBackToTopTarget, scrollModLibraryBackToTop } from "./modLibraryBackToTop";
import { applyModSelection } from "./modSelection";
import { modLibraryItems, type ModInstallStatus, type ModLibraryItem } from "./modsLibraryData";

type ModLibraryPageProps = {
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

function staggerStyle(index: number) {
  return { "--stagger-idx": index } as CSSProperties;
}

export function ModLibraryPage({ onAction }: ModLibraryPageProps) {
  const [query, setQuery] = useState("");
  const [activeFilter, setActiveFilter] = useState<string>("全部");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const visibleItems = useMemo(() => {
    const keyword = query.trim().toLowerCase();
    return modLibraryItems.filter((item) => {
      const matchesKeyword = !keyword || item.name.toLowerCase().includes(keyword);
      return matchesKeyword && matchesActiveFilter(item, activeFilter);
    });
  }, [activeFilter, query]);

  const selectedCount = selectedIds.size;

  const selectCard = (id: string) => {
    setSelectedIds((prev) => applyModSelection(prev, id, "replace"));
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
        onAction?.(actionId);
        break;
      default:
        onAction?.(actionId);
        break;
    }
  };

  const handleBackToTop = () => {
    if (typeof document === "undefined") {
      return;
    }

    const fallbackTarget = document.scrollingElement ?? document.documentElement;
    const target = getModLibraryBackToTopTarget(document, fallbackTarget);
    scrollModLibraryBackToTop(target);
  };

  return (
    <section className="mod-library" aria-label="模组库">
      <div className="mod-library__sticky-controls anim-stagger-item" style={staggerStyle(0)}>
        <div className="mod-library__toolbar-slot">
          <LibraryToolbar
            query={query}
            activeFilter={activeFilter}
            onQueryChange={setQuery}
            onFilterChange={setActiveFilter}
          />
        </div>

        <div className="mod-library__actions-slot">
          <CompactActionPanel selectedCount={selectedCount} onAction={handleAction} />
        </div>
      </div>

      <div className="mod-library__content">
        <div className="mod-library__main-floating-actions">
          <BackToTopButton onClick={handleBackToTop} />
        </div>

        {visibleItems.length === 0 ? (
          <div className="mod-library__empty anim-stagger-item" style={staggerStyle(1)} role="status">
            <strong>没有匹配的 Mod</strong>
            <p>试试调整搜索关键词或筛选条件。</p>
          </div>
        ) : (
          <div className="mod-grid" role="list">
            {visibleItems.map((item, index) => (
              <ModPosterCard
                key={item.id}
                item={item}
                selected={selectedIds.has(item.id)}
                onSelect={selectCard}
                index={index}
              />
            ))}
          </div>
        )}
      </div>
    </section>
  );
}
