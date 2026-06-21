import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { BackToTopButton } from "./BackToTopButton";
import { CompactActionPanel } from "./CompactActionPanel";
import { LibraryToolbar } from "./LibraryToolbar";
import { ModPosterCard } from "./ModPosterCard";
import { getModLibraryBackToTopTarget, scrollModLibraryBackToTop } from "./modLibraryBackToTop";
import { getModLibraryScrollUiState } from "./modLibraryScrollUi";
import { applyModSelection } from "./modSelection";
import { modLibraryItems, type ModInstallStatus, type ModLibraryItem } from "./modsLibraryData";

export type ModViewMode = "classic" | "grid" | "list" | "tech";

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

const initialScrollUiState = getModLibraryScrollUiState({
  scrollTop: 0,
  scrollHeight: 0,
  clientHeight: 0,
});

export function ModLibraryPage({ onAction }: ModLibraryPageProps) {
  const [query, setQuery] = useState("");
  const [activeFilter, setActiveFilter] = useState<string>("全部");
  const [viewMode, setViewMode] = useState<ModViewMode>("classic");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const contentRef = useRef<HTMLDivElement | null>(null);
  const [scrollUiState, setScrollUiState] = useState(initialScrollUiState);

  const visibleItems = useMemo(() => {
    const keyword = query.trim().toLowerCase();
    return modLibraryItems.filter((item) => {
      const matchesKeyword = !keyword || item.name.toLowerCase().includes(keyword);
      return matchesKeyword && matchesActiveFilter(item, activeFilter);
    });
  }, [activeFilter, query]);

  const selectedCount = selectedIds.size;

  const updateScrollUiState = useCallback(() => {
    const content = contentRef.current;

    if (!content) {
      setScrollUiState(initialScrollUiState);
      return;
    }

    setScrollUiState(
      getModLibraryScrollUiState({
        scrollTop: content.scrollTop,
        scrollHeight: content.scrollHeight,
        clientHeight: content.clientHeight,
      }),
    );
  }, []);

  useEffect(() => {
    const content = contentRef.current;

    if (!content) {
      return undefined;
    }

    let frameId = 0;
    const requestUpdate = () => {
      if (frameId !== 0) {
        return;
      }

      frameId = window.requestAnimationFrame(() => {
        frameId = 0;
        updateScrollUiState();
      });
    };

    const resizeObserver = new ResizeObserver(requestUpdate);
    resizeObserver.observe(content);
    for (const child of Array.from(content.children)) {
      resizeObserver.observe(child);
    }

    content.addEventListener("scroll", requestUpdate, { passive: true });
    requestUpdate();

    return () => {
      content.removeEventListener("scroll", requestUpdate);
      resizeObserver.disconnect();
      if (frameId !== 0) {
        window.cancelAnimationFrame(frameId);
      }
    };
  }, [updateScrollUiState, visibleItems.length, scrollUiState.showScrollUi]);

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
    const target = contentRef.current ?? getModLibraryBackToTopTarget(document, fallbackTarget);
    scrollModLibraryBackToTop(target);
  };

  const handleScrollbarPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    const content = contentRef.current;

    if (!content) {
      return;
    }

    event.preventDefault();
    const startY = event.clientY;
    const startScrollTop = content.scrollTop;
    const maxScrollTop = Math.max(0, content.scrollHeight - content.clientHeight);
    const thumbHeight = Number.parseFloat(scrollUiState.thumbStyle.height);
    const maxThumbTop = Math.max(1, content.clientHeight - thumbHeight);

    const handlePointerMove = (moveEvent: PointerEvent) => {
      const deltaY = moveEvent.clientY - startY;
      content.scrollTop = startScrollTop + (deltaY / maxThumbTop) * maxScrollTop;
    };

    const stopDragging = () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", stopDragging);
      window.removeEventListener("pointercancel", stopDragging);
    };

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", stopDragging);
    window.addEventListener("pointercancel", stopDragging);
  };

  const { showScrollUi, thumbStyle } = scrollUiState;

  return (
    <section className="mod-library" aria-label="模组库">
      <div className="mod-library__sticky-controls anim-stagger-item" style={staggerStyle(0)}>
        <div className="mod-library__toolbar-slot">
          <LibraryToolbar
            query={query}
            activeFilter={activeFilter}
            viewMode={viewMode}
            onQueryChange={setQuery}
            onFilterChange={setActiveFilter}
            onViewModeChange={setViewMode}
          />
        </div>

        <div className="mod-library__actions-slot">
          <CompactActionPanel selectedCount={selectedCount} totalCount={visibleItems.length} onAction={handleAction} />
        </div>
      </div>

      <div className="mod-library__content-shell" data-scroll-ui={showScrollUi ? "visible" : "hidden"}>
        <div ref={contentRef} className="mod-library__content">
          {showScrollUi ? (
            <div className="mod-library__main-floating-actions">
              <BackToTopButton onClick={handleBackToTop} />
            </div>
          ) : null}

          {visibleItems.length === 0 ? (
            <div className="mod-library__empty anim-stagger-item" style={staggerStyle(1)} role="status">
              <strong>没有匹配的 Mod</strong>
              <p>试试调整搜索关键词或筛选条件。</p>
            </div>
          ) : (
            <div className={`mod-grid view-${viewMode}`} role="list">
              {visibleItems.map((item, index) => (
                <ModPosterCard
                  key={item.id}
                  item={item}
                  selected={selectedIds.has(item.id)}
                  viewMode={viewMode}
                  onSelect={selectCard}
                  index={index}
                />
              ))}
            </div>
          )}
        </div>

        {showScrollUi ? (
          <div className="mod-library__scrollbar" aria-hidden="true">
            <div
              className="mod-library__scrollbar-thumb"
              style={thumbStyle}
              onPointerDown={handleScrollbarPointerDown}
            />
          </div>
        ) : null}
      </div>
    </section>
  );
}
