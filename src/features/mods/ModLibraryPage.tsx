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
import { InstallPlanPreviewPanel, type InstallPlanPreviewPanelState } from "./InstallPlanPreviewPanel";
import { LibraryToolbar } from "./LibraryToolbar";
import { ModPosterCard } from "./ModPosterCard";
import { previewInstallPlanForImportedMod } from "./modInstallPlanApi";
import { getModLibraryBackToTopTarget, scrollModLibraryBackToTop } from "./modLibraryBackToTop";
import { getModLibrary } from "./modLibraryApi";
import { resolveLoadedModLibraryItems } from "./modLibraryLoadState";
import { getModLibraryScrollUiState } from "./modLibraryScrollUi";
import type { ModInstallStatus, ModLibraryItem } from "./modLibraryTypes";
import { applyModSelection } from "./modSelection";
import { modLibraryItems as fallbackModLibraryItems } from "./modsLibraryData";
import { ModContextMenu } from "./ModContextMenu";

export type ModViewMode = "classic" | "grid" | "list" | "tech";

type ViewTransitionPhase = "idle" | "out" | "in";
type ViewTransitionVariant = "morph" | "wave" | "flip3d" | "blur";

const viewTransitionOutMs = 220;
const viewTransitionInMs = 420;

const viewTransitionVariantByMode: Record<ModViewMode, ViewTransitionVariant> = {
  classic: "morph",
  grid: "wave",
  list: "flip3d",
  tech: "blur",
};

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

function prefersReducedMotion() {
  return typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
}

function useModViewTransition(viewMode: ModViewMode, setViewMode: (mode: ModViewMode) => void) {
  const [viewTransitionPhase, setViewTransitionPhase] = useState<ViewTransitionPhase>("idle");
  const [viewTransitionVariant, setViewTransitionVariant] = useState<ViewTransitionVariant>(
    viewTransitionVariantByMode[viewMode],
  );
  const outTimeoutRef = useRef<number | null>(null);
  const inTimeoutRef = useRef<number | null>(null);

  const clearTransitionTimers = useCallback(() => {
    if (outTimeoutRef.current !== null) {
      window.clearTimeout(outTimeoutRef.current);
      outTimeoutRef.current = null;
    }
    if (inTimeoutRef.current !== null) {
      window.clearTimeout(inTimeoutRef.current);
      inTimeoutRef.current = null;
    }
  }, []);

  useEffect(() => clearTransitionTimers, [clearTransitionTimers]);

  const handleViewModeChange = useCallback(
    (nextViewMode: ModViewMode) => {
      if (nextViewMode === viewMode) {
        return;
      }

      clearTransitionTimers();

      if (prefersReducedMotion()) {
        setViewTransitionVariant(viewTransitionVariantByMode[nextViewMode]);
        setViewMode(nextViewMode);
        setViewTransitionPhase("idle");
        return;
      }

      setViewTransitionVariant(viewTransitionVariantByMode[nextViewMode]);
      setViewTransitionPhase("out");
      outTimeoutRef.current = window.setTimeout(() => {
        outTimeoutRef.current = null;
        setViewMode(nextViewMode);
        setViewTransitionPhase("in");

        inTimeoutRef.current = window.setTimeout(() => {
          inTimeoutRef.current = null;
          setViewTransitionPhase("idle");
        }, viewTransitionInMs);
      }, viewTransitionOutMs);
    },
    [clearTransitionTimers, setViewMode, viewMode],
  );

  return { handleViewModeChange, viewTransitionPhase, viewTransitionVariant };
}

const initialScrollUiState = getModLibraryScrollUiState({
  scrollTop: 0,
  scrollHeight: 0,
  clientHeight: 0,
});

function installPlanPreviewErrorMessage(error: unknown) {
  const code =
    typeof error === "object" && error !== null && "code" in error && typeof error.code === "string"
      ? error.code
      : null;

  switch (code) {
    case "install_planning_imported_mod_not_found":
      return "未找到已导入的 Mod";
    case "install_planning_imported_mod_analysis_unavailable":
      return "无法读取导入分析";
    case "install_planning_imported_mod_sandbox_unavailable":
    case "install_planning_imported_mod_file_scan_unavailable":
      return "无法读取导入文件";
    case "install_planning_game_adapter_not_found":
    case "game_id_invalid":
      return "当前游戏不支持安装计划预览";
    default:
      return "安装计划预览失败";
  }
}

export function ModLibraryPage({ onAction }: ModLibraryPageProps) {
  const [query, setQuery] = useState("");
  const [activeFilter, setActiveFilter] = useState<string>("全部");
  const [viewMode, setViewMode] = useState<ModViewMode>("classic");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [libraryItems, setLibraryItems] = useState<ModLibraryItem[]>(fallbackModLibraryItems);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const [scrollUiState, setScrollUiState] = useState(initialScrollUiState);
  const [contextMenuState, setContextMenuState] = useState<{ x: number; y: number; modId: string } | null>(null);
  const [installPlanPreviewState, setInstallPlanPreviewState] = useState<InstallPlanPreviewPanelState>({
    status: "idle",
  });

  const visibleItems = useMemo(() => {
    const keyword = query.trim().toLowerCase();
    return libraryItems.filter((item) => {
      const matchesKeyword = !keyword || item.name.toLowerCase().includes(keyword);
      return matchesKeyword && matchesActiveFilter(item, activeFilter);
    });
  }, [activeFilter, libraryItems, query]);

  const selectedCount = selectedIds.size;
  const { handleViewModeChange, viewTransitionPhase, viewTransitionVariant } = useModViewTransition(
    viewMode,
    setViewMode,
  );

  useEffect(() => {
    let cancelled = false;

    void getModLibrary()
      .then((items) => {
        if (!cancelled) {
          setLibraryItems(
            resolveLoadedModLibraryItems({
              backendItems: items,
              fallbackItems: fallbackModLibraryItems,
            }),
          );
        }
      })
      .catch(() => {
        if (!cancelled) {
          setLibraryItems(
            resolveLoadedModLibraryItems({
              backendItems: null,
              fallbackItems: fallbackModLibraryItems,
            }),
          );
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

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

  const handleContextMenu = (modId: string, x: number, y: number) => {
    setContextMenuState({ x, y, modId });
    // If the card isn't selected, select it
    if (!selectedIds.has(modId)) {
      setSelectedIds((prev) => applyModSelection(prev, modId, "replace"));
    }
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

  const previewSelectedInstallPlan = () => {
    if (selectedIds.size !== 1) {
      return;
    }

    const [modId] = Array.from(selectedIds);
    const item = libraryItems.find((candidate) => candidate.id === modId);
    const modName = item?.name ?? modId;

    setInstallPlanPreviewState({ status: "loading", modName });
    void previewInstallPlanForImportedMod({
      gameId: "mhw",
      modId,
      layerName: "base",
      layerPriority: 0,
    })
      .then((plan) => {
        setInstallPlanPreviewState({ status: "ready", modName, plan });
      })
      .catch((error: unknown) => {
        setInstallPlanPreviewState({
          status: "error",
          modName,
          message: installPlanPreviewErrorMessage(error),
        });
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
      case "preview-plan":
        previewSelectedInstallPlan();
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
            onViewModeChange={handleViewModeChange}
          />
        </div>

        <div className="mod-library__actions-slot">
          <CompactActionPanel selectedCount={selectedCount} totalCount={visibleItems.length} onAction={handleAction} />
        </div>
      </div>

      <InstallPlanPreviewPanel
        state={installPlanPreviewState}
        onClose={() => setInstallPlanPreviewState({ status: "idle" })}
      />

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
            <div
              className={`mod-grid view-${viewMode}`}
              role="list"
              data-view-transition={viewTransitionPhase}
              data-view-transition-variant={viewTransitionVariant}
            >
              {visibleItems.map((item, index) => (
                <ModPosterCard
                  key={item.id}
                  item={item}
                  selected={selectedIds.has(item.id)}
                  viewMode={viewMode}
                  onSelect={selectCard}
                  onContextMenu={handleContextMenu}
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

      {contextMenuState && (
        <ModContextMenu
          x={contextMenuState.x}
          y={contextMenuState.y}
          modId={contextMenuState.modId}
          onClose={() => setContextMenuState(null)}
          onAction={(actionId, modId) => {
            console.log(`Context Menu Action: ${actionId} for Mod: ${modId}`);
            // In a real app, you would handle the specific actions here or pass them up via onAction prop
          }}
        />
      )}
    </section>
  );
}
