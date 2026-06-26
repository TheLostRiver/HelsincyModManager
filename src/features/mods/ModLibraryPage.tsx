import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { BackToTopButton } from "./BackToTopButton";
import { CompactActionPanel } from "./CompactActionPanel";
import { InstallPlanPreviewPanel, type InstallPlanPreviewPanelState } from "./InstallPlanPreviewPanel";
import { LibraryToolbar } from "./LibraryToolbar";
import { ModPosterCard } from "./ModPosterCard";
import { getInstallManifestStatus, previewInstallPlanForImportedMod, startInstallTask } from "./modInstallPlanApi";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "./modImportTypes";
import { getModLibraryBackToTopTarget, scrollModLibraryBackToTop } from "./modLibraryBackToTop";
import { getModLibrary } from "./modLibraryApi";
import { applyInstallManifestStatusSummaries, resolveLoadedModLibraryItems } from "./modLibraryLoadState";
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
const DEFAULT_INSTALL_PROFILE_ID = "default";

type InstallTaskPhase =
  | "install.queued"
  | "install.plan.building"
  | "install.commit.processing"
  | "install.completed"
  | "install.failed"
  | "install.cancelled";

type InstallTaskState =
  | { status: "idle" }
  | { status: "starting"; modName: string }
  | { status: "running"; taskId: string; modName: string; phase: InstallTaskPhase }
  | { status: "completed"; taskId: string; modName: string; phase: "install.completed" }
  | { status: "failed"; taskId: string | null; modName: string; phase: "install.failed"; message: string }
  | { status: "cancelled"; taskId: string; modName: string; phase: "install.cancelled" };
type InstallTaskStateUpdate = InstallTaskState | ((current: InstallTaskState) => InstallTaskState);

const installTaskPhaseLabels: Record<InstallTaskPhase, string> = {
  "install.queued": "等待安装",
  "install.plan.building": "生成安装计划",
  "install.commit.processing": "写入中",
  "install.completed": "安装完成",
  "install.failed": "安装失败",
  "install.cancelled": "已取消",
};

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

function isInstallTaskPhase(phase: string): phase is InstallTaskPhase {
  return Object.prototype.hasOwnProperty.call(installTaskPhaseLabels, phase);
}

function installTaskErrorMessage(error: unknown) {
  const code =
    typeof error === "object" && error !== null && "code" in error && typeof error.code === "string"
      ? error.code
      : null;

  switch (code) {
    case "install_planning_imported_mod_not_found":
      return "未找到已导入的 Mod";
    case "install_planning_imported_mod_analysis_unavailable":
      return "无法读取导入分析";
    case "install_planning_game_adapter_not_found":
    case "game_id_invalid":
      return "当前游戏不支持安装任务";
    default:
      return "安装任务启动失败";
  }
}

function installTaskPanelState(
  previewState: InstallPlanPreviewPanelState,
  installTaskState: InstallTaskState,
): InstallPlanPreviewPanelState {
  switch (installTaskState.status) {
    case "idle":
      return previewState;
    case "starting":
      return {
        status: "install-starting",
        modName: installTaskState.modName,
        phaseLabel: "启动安装任务",
      };
    case "running":
      return {
        status: "install-running",
        modName: installTaskState.modName,
        phaseLabel: installTaskPhaseLabels[installTaskState.phase],
      };
    case "completed":
      return {
        status: "install-completed",
        modName: installTaskState.modName,
        phaseLabel: installTaskPhaseLabels[installTaskState.phase],
      };
    case "failed":
      return {
        status: "install-failed",
        modName: installTaskState.modName,
        phaseLabel: installTaskPhaseLabels[installTaskState.phase],
        message: installTaskState.message,
      };
    case "cancelled":
      return {
        status: "install-cancelled",
        modName: installTaskState.modName,
        phaseLabel: installTaskPhaseLabels[installTaskState.phase],
      };
  }
}

function nextInstallTaskStateFromProgress(
  current: InstallTaskState,
  event: TaskProgressEventDto,
): InstallTaskState {
  if (!("taskId" in current) || current.taskId !== event.taskId) {
    return current;
  }

  const phase = event.phase;
  if (!isInstallTaskPhase(phase)) {
    return current;
  }

  switch (phase) {
    case "install.completed":
      return {
        status: "completed",
        taskId: event.taskId,
        modName: current.modName,
        phase,
      };
    case "install.failed":
      return {
        status: "failed",
        taskId: event.taskId,
        modName: current.modName,
        phase,
        message: event.error ?? event.message ?? "安装失败",
      };
    case "install.cancelled":
      return {
        status: "cancelled",
        taskId: event.taskId,
        modName: current.modName,
        phase,
      };
    default:
      return {
        status: "running",
        taskId: event.taskId,
        modName: current.modName,
        phase,
      };
  }
}

export function ModLibraryPage({ onAction }: ModLibraryPageProps) {
  const [query, setQuery] = useState("");
  const [activeFilter, setActiveFilter] = useState<string>("全部");
  const [viewMode, setViewMode] = useState<ModViewMode>("classic");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [libraryItems, setLibraryItems] = useState<ModLibraryItem[]>(fallbackModLibraryItems);
  const libraryItemsRef = useRef<ModLibraryItem[]>(fallbackModLibraryItems);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const [scrollUiState, setScrollUiState] = useState(initialScrollUiState);
  const [contextMenuState, setContextMenuState] = useState<{ x: number; y: number; modId: string } | null>(null);
  const [installPlanPreviewState, setInstallPlanPreviewState] = useState<InstallPlanPreviewPanelState>({
    status: "idle",
  });
  const [installTaskState, setInstallTaskState] = useState<InstallTaskState>({ status: "idle" });
  const installTaskStateRef = useRef<InstallTaskState>(installTaskState);
  const lastInstallStatusRefreshTaskIdRef = useRef<string | null>(null);
  const pendingInstallProgressEventsRef = useRef<Map<string, TaskProgressEventDto>>(new Map());

  const setTrackedInstallTaskState = useCallback((update: InstallTaskStateUpdate) => {
    const nextState = typeof update === "function" ? update(installTaskStateRef.current) : update;
    installTaskStateRef.current = nextState;
    setInstallTaskState(nextState);
  }, []);

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

  const refreshInstallManifestStatuses = useCallback((items: ModLibraryItem[]) => {
    const modIds = Array.from(new Set(items.map((item) => item.id))).filter((id) => id.length > 0);
    if (modIds.length === 0) {
      return Promise.resolve(items);
    }

    return getInstallManifestStatus({
      profileId: DEFAULT_INSTALL_PROFILE_ID,
      modIds,
    })
      .then((summaries) => applyInstallManifestStatusSummaries(items, summaries))
      .catch(() => items);
  }, []);

  useEffect(() => {
    libraryItemsRef.current = libraryItems;
  }, [libraryItems]);

  useEffect(() => {
    let cancelled = false;

    void getModLibrary()
      .then((items) => {
        const resolvedItems = resolveLoadedModLibraryItems({
          backendItems: items,
          fallbackItems: fallbackModLibraryItems,
        });

        return refreshInstallManifestStatuses(resolvedItems).then((itemsWithStatus) => {
          if (!cancelled) {
            setLibraryItems(itemsWithStatus);
          }
        });
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
  }, [refreshInstallManifestStatuses]);

  useEffect(() => {
    if (installTaskState.status !== "completed") {
      return;
    }
    if (lastInstallStatusRefreshTaskIdRef.current === installTaskState.taskId) {
      return;
    }

    lastInstallStatusRefreshTaskIdRef.current = installTaskState.taskId;
    const itemsAtRefreshStart = libraryItemsRef.current;

    void refreshInstallManifestStatuses(itemsAtRefreshStart).then((itemsWithStatus) => {
      if (libraryItemsRef.current === itemsAtRefreshStart) {
        setLibraryItems(itemsWithStatus);
      }
    });
  }, [installTaskState, refreshInstallManifestStatuses]);

  useEffect(() => {
    let disposed = false;
    let unlistenTaskProgress: (() => void) | null = null;

    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, (event) => {
      if (disposed) {
        return;
      }

      const installTaskState = installTaskStateRef.current;
      if (event.payload.kind !== "install") {
        return;
      }

      const phase = event.payload.phase;
      if (!isInstallTaskPhase(phase)) {
        return;
      }

      if (!("taskId" in installTaskState) || installTaskState.taskId === null) {
        if (installTaskState.status === "starting") {
          pendingInstallProgressEventsRef.current.set(event.payload.taskId, event.payload);
        }
        return;
      }
      if (event.payload.taskId !== installTaskState.taskId) {
        return;
      }

      setTrackedInstallTaskState((current) => {
        return nextInstallTaskStateFromProgress(current, event.payload);
      });
    }).then((unlisten) => {
      if (disposed) {
        unlisten();
        return;
      }

      unlistenTaskProgress = unlisten;
    });

    return () => {
      disposed = true;
      unlistenTaskProgress?.();
    };
  }, [setTrackedInstallTaskState]);

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

  const startSelectedInstallTask = () => {
    if (selectedIds.size !== 1) {
      return;
    }

    const [modId] = Array.from(selectedIds);
    const item = libraryItems.find((candidate) => candidate.id === modId);
    const modName = item?.name ?? modId;

    setInstallPlanPreviewState({ status: "idle" });
    pendingInstallProgressEventsRef.current.clear();
    setTrackedInstallTaskState({ status: "starting", modName });
    void startInstallTask({
      gameId: "mhw",
      modId,
      profileId: DEFAULT_INSTALL_PROFILE_ID,
      layerName: "base",
      layerPriority: 0,
    })
      .then((task) => {
        const pendingProgressEvent = pendingInstallProgressEventsRef.current.get(task.taskId) ?? null;
        pendingInstallProgressEventsRef.current.clear();

        if (task.kind !== "install") {
          setTrackedInstallTaskState({
            status: "failed",
            taskId: null,
            modName,
            phase: "install.failed",
            message: "安装任务返回了无效类型",
          });
          return;
        }

        setTrackedInstallTaskState((current) => {
          const runningState: InstallTaskState = {
            status: "running",
            taskId: task.taskId,
            modName,
            phase: "install.queued",
          };

          if (current.status !== "starting") {
            return current;
          }

          return pendingProgressEvent
            ? nextInstallTaskStateFromProgress(runningState, pendingProgressEvent)
            : runningState;
        });
      })
      .catch((error: unknown) => {
        pendingInstallProgressEventsRef.current.clear();
        setTrackedInstallTaskState({
          status: "failed",
          taskId: null,
          modName,
          phase: "install.failed",
          message: installTaskErrorMessage(error),
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
      case "reinstall":
        startSelectedInstallTask();
        break;
      case "uninstall":
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

  const installTaskActive = installTaskState.status === "starting" || installTaskState.status === "running";
  const activeInstallPanelState = installTaskPanelState(installPlanPreviewState, installTaskState);
  const closeInstallPlanPanel = () => {
    setInstallPlanPreviewState({ status: "idle" });
    setTrackedInstallTaskState((current) =>
      current.status === "starting" || current.status === "running" ? current : { status: "idle" },
    );
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
          <CompactActionPanel
            selectedCount={selectedCount}
            totalCount={visibleItems.length}
            installTaskActive={installTaskActive}
            onAction={handleAction}
          />
        </div>
      </div>

      <InstallPlanPreviewPanel
        state={activeInstallPanelState}
        onClose={closeInstallPlanPanel}
        closeDisabled={installTaskActive}
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
