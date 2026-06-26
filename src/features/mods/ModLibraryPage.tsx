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
import {
  getInstallManifestStatus,
  previewInstallPlanForImportedMod,
  scanInstallRecovery,
  startInstallTask,
  startUninstallTask,
} from "./modInstallPlanApi";
import {
  getManagedInstallTaskPhaseLabel,
  getManagedInstallTaskStartingLabel,
  isManagedInstallTaskPhase,
  nextManagedInstallTaskStateFromProgress,
  type ManagedInstallTaskOperation,
  type ManagedInstallTaskState,
  type ManagedInstallTaskStateUpdate,
} from "./modInstallTaskState";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "./modImportTypes";
import { getModLibraryBackToTopTarget, scrollModLibraryBackToTop } from "./modLibraryBackToTop";
import { getModLibrary } from "./modLibraryApi";
import {
  applyInstallManifestStatusSummaries,
  applyInstallRecoverySummaries,
  resolveLoadedModLibraryItems,
} from "./modLibraryLoadState";
import { getModLibraryScrollUiState } from "./modLibraryScrollUi";
import type { ModInstallStatus, ModInstallSummary, ModLibraryItem } from "./modLibraryTypes";
import { applyModSelection } from "./modSelection";
import { modLibraryItems as fallbackModLibraryItems } from "./modsLibraryData";
import { ModContextMenu } from "./ModContextMenu";

export type ModViewMode = "classic" | "grid" | "list" | "tech";

type ViewTransitionPhase = "idle" | "out" | "in";
type ViewTransitionVariant = "morph" | "wave" | "flip3d" | "blur";

const viewTransitionOutMs = 220;
const viewTransitionInMs = 420;
const DEFAULT_INSTALL_GAME_ID = "mhw";
const DEFAULT_INSTALL_PROFILE_ID = "default";

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

function installTaskErrorMessage(error: unknown, operation: ManagedInstallTaskOperation) {
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
      return operation === "uninstall" ? "当前游戏不支持卸载任务" : "当前游戏不支持安装任务";
    default:
      return operation === "uninstall" ? "卸载任务启动失败" : "安装任务启动失败";
  }
}

function installTaskPanelState(
  previewState: InstallPlanPreviewPanelState,
  installTaskState: ManagedInstallTaskState,
): InstallPlanPreviewPanelState {
  switch (installTaskState.status) {
    case "idle":
      return previewState;
    case "starting":
      return {
        status: installTaskState.operation === "uninstall" ? "uninstall-starting" : "install-starting",
        modName: installTaskState.modName,
        phaseLabel: getManagedInstallTaskStartingLabel(installTaskState.operation),
      };
    case "running":
      return {
        status: installTaskState.operation === "uninstall" ? "uninstall-running" : "install-running",
        modName: installTaskState.modName,
        phaseLabel: getManagedInstallTaskPhaseLabel(installTaskState.phase),
      };
    case "completed":
      return {
        status: installTaskState.operation === "uninstall" ? "uninstall-completed" : "install-completed",
        modName: installTaskState.modName,
        phaseLabel: getManagedInstallTaskPhaseLabel(installTaskState.phase),
      };
    case "failed":
      return {
        status: installTaskState.operation === "uninstall" ? "uninstall-failed" : "install-failed",
        modName: installTaskState.modName,
        phaseLabel: getManagedInstallTaskPhaseLabel(installTaskState.phase),
        message: installTaskState.message,
      };
    case "cancelled":
      return {
        status: "install-cancelled",
        modName: installTaskState.modName,
        phaseLabel: getManagedInstallTaskPhaseLabel(installTaskState.phase),
      };
  }
}

type UnsafeRecoverySummary = ModInstallSummary & {
  status: "repair_required" | "unknown";
};

function isUnsafeRecoverySummary(summary: ModInstallSummary | undefined): summary is UnsafeRecoverySummary {
  return summary?.status === "repair_required" || summary?.status === "unknown";
}

function recoveryPanelStateForItem(item: ModLibraryItem): InstallPlanPreviewPanelState | null {
  const summary = item.installSummary;
  if (!isUnsafeRecoverySummary(summary)) {
    return null;
  }

  return {
    status: "recovery-required",
    modName: item.name,
    recoveryStatus: summary.status,
    managedFileCount: summary.managedFileCount,
    backupCount: summary.backupCount,
    issueCount: summary.issueCount ?? 0,
    issues: summary.issues ?? [],
  };
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
  const [installTaskState, setInstallTaskState] = useState<ManagedInstallTaskState>({ status: "idle" });
  const installTaskStateRef = useRef<ManagedInstallTaskState>(installTaskState);
  const lastInstallStatusRefreshTaskIdRef = useRef<string | null>(null);
  const pendingInstallProgressEventsRef = useRef<Map<string, TaskProgressEventDto>>(new Map());
  const pendingUninstallRef = useRef<{ modId: string; modName: string } | null>(null);

  const setTrackedInstallTaskState = useCallback((update: ManagedInstallTaskStateUpdate) => {
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
  const selectedItem = useMemo(() => {
    if (selectedIds.size !== 1) {
      return null;
    }

    const [selectedId] = Array.from(selectedIds);
    return libraryItems.find((item) => item.id === selectedId) ?? null;
  }, [libraryItems, selectedIds]);
  const canUninstallSelected = selectedItem?.installSummary?.status === "installed";
  const canInstallSelected =
    selectedItem !== null &&
    selectedItem.installSummary?.status !== "repair_required" &&
    selectedItem.installSummary?.status !== "unknown";
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
      .then((itemsWithManifestStatus) =>
        scanInstallRecovery({
          gameId: DEFAULT_INSTALL_GAME_ID,
          profileId: DEFAULT_INSTALL_PROFILE_ID,
          modIds,
        })
          .then((summaries) => applyInstallRecoverySummaries(itemsWithManifestStatus, summaries))
          .catch(() => itemsWithManifestStatus),
      )
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
      if (!isManagedInstallTaskPhase(phase)) {
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
        return nextManagedInstallTaskStateFromProgress(current, event.payload);
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
    const recoveryPanelState = item ? recoveryPanelStateForItem(item) : null;
    if (recoveryPanelState) {
      setInstallPlanPreviewState(recoveryPanelState);
      return;
    }

    setInstallPlanPreviewState({ status: "loading", modName });
    void previewInstallPlanForImportedMod({
      gameId: DEFAULT_INSTALL_GAME_ID,
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
    const recoveryPanelState = item ? recoveryPanelStateForItem(item) : null;
    if (!canInstallSelected || recoveryPanelState) {
      if (recoveryPanelState) {
        setInstallPlanPreviewState(recoveryPanelState);
      }
      return;
    }

    setInstallPlanPreviewState({ status: "idle" });
    pendingInstallProgressEventsRef.current.clear();
    setTrackedInstallTaskState({ status: "starting", operation: "install", modName });
    void startInstallTask({
      gameId: DEFAULT_INSTALL_GAME_ID,
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
            operation: "install",
            taskId: null,
            modName,
            phase: "install.failed",
            message: "安装任务返回了无效类型",
          });
          return;
        }

        setTrackedInstallTaskState((current) => {
          const runningState: ManagedInstallTaskState = {
            status: "running",
            operation: "install",
            taskId: task.taskId,
            modName,
            phase: "install.queued",
          };

          if (current.status !== "starting") {
            return current;
          }

          return pendingProgressEvent
            ? nextManagedInstallTaskStateFromProgress(runningState, pendingProgressEvent)
            : runningState;
        });
      })
      .catch((error: unknown) => {
        pendingInstallProgressEventsRef.current.clear();
        setTrackedInstallTaskState({
          status: "failed",
          operation: "install",
          taskId: null,
          modName,
          phase: "install.failed",
          message: installTaskErrorMessage(error, "install"),
        });
      });
  };

  const promptSelectedUninstallTask = () => {
    if (!selectedItem || selectedItem.installSummary?.status !== "installed") {
      return;
    }

    pendingUninstallRef.current = { modId: selectedItem.id, modName: selectedItem.name };
    setTrackedInstallTaskState({ status: "idle" });
    setInstallPlanPreviewState({
      status: "uninstall-confirming",
      modName: selectedItem.name,
      managedFileCount: selectedItem.installSummary.managedFileCount,
      backupCount: selectedItem.installSummary.backupCount,
    });
  };

  const cancelUninstallConfirmation = () => {
    pendingUninstallRef.current = null;
    setInstallPlanPreviewState({ status: "idle" });
  };

  const startSelectedUninstallTask = () => {
    const pendingUninstall = pendingUninstallRef.current;
    if (!pendingUninstall) {
      return;
    }

    const { modId, modName } = pendingUninstall;
    pendingUninstallRef.current = null;
    setInstallPlanPreviewState({ status: "idle" });
    pendingInstallProgressEventsRef.current.clear();
    setTrackedInstallTaskState({ status: "starting", operation: "uninstall", modName });
    void startUninstallTask({
      gameId: DEFAULT_INSTALL_GAME_ID,
      modId,
      profileId: DEFAULT_INSTALL_PROFILE_ID,
    })
      .then((task) => {
        const pendingProgressEvent = pendingInstallProgressEventsRef.current.get(task.taskId) ?? null;
        pendingInstallProgressEventsRef.current.clear();

        if (task.kind !== "install") {
          setTrackedInstallTaskState({
            status: "failed",
            operation: "uninstall",
            taskId: null,
            modName,
            phase: "install.uninstall.failed",
            message: "卸载任务返回了无效类型",
          });
          return;
        }

        setTrackedInstallTaskState((current) => {
          const runningState: ManagedInstallTaskState = {
            status: "running",
            operation: "uninstall",
            taskId: task.taskId,
            modName,
            phase: "install.uninstall.queued",
          };

          if (current.status !== "starting") {
            return current;
          }

          return pendingProgressEvent
            ? nextManagedInstallTaskStateFromProgress(runningState, pendingProgressEvent)
            : runningState;
        });
      })
      .catch((error: unknown) => {
        pendingInstallProgressEventsRef.current.clear();
        setTrackedInstallTaskState({
          status: "failed",
          operation: "uninstall",
          taskId: null,
          modName,
          phase: "install.uninstall.failed",
          message: installTaskErrorMessage(error, "uninstall"),
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
        promptSelectedUninstallTask();
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
    pendingUninstallRef.current = null;
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
            canInstallSelection={canInstallSelected}
            canUninstallSelection={canUninstallSelected}
            onAction={handleAction}
          />
        </div>
      </div>

      <InstallPlanPreviewPanel
        state={activeInstallPanelState}
        onClose={closeInstallPlanPanel}
        onConfirmUninstall={startSelectedUninstallTask}
        onCancelUninstall={cancelUninstallConfirmation}
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
