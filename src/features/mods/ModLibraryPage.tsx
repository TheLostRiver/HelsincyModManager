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
import { ModDetailDialog } from "./ModDetailDialog";
import { ModPosterCard } from "./ModPosterCard";
import { ReinstallPlanPreviewPanel } from "./ReinstallPlanPreviewPanel";
import {
  getInstallManifestStatus,
  previewInstallPlanForImportedMod,
  scanInstallRecovery,
  startInstallTask,
  startUninstallTask,
} from "./modInstallPlanApi";
import type { UnsafeInstallStatus } from "./modInstallPlanTypes";
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
import { listCategories, type CategoryItem } from "./modCategoryApi";
import {
  allLibraryFilter,
  buildLibraryFilterChips,
  matchesLibraryFilter,
  normalizeLibraryFilter,
  type ModLibraryFilter,
} from "./modLibraryFilters";
import {
  applyInstallManifestStatusSummaries,
  applyInstallRecoveryUnavailable,
  applyInstallRecoverySummaries,
  isUnsafeInstallStatus,
} from "./modLibraryLoadState";
import {
  createDetailDialogState,
  loadModLibraryItemsForMode,
  preserveItemsOnRefreshFailure,
  type ModLibraryLoadMode,
} from "./modLibraryRefresh";
import { getModLibraryScrollUiState } from "./modLibraryScrollUi";
import type { ModInstallSummary, ModLibraryItem } from "./modLibraryTypes";
import { applyModSelection } from "./modSelection";
import { modLibraryItems as fallbackModLibraryItems } from "./modsLibraryData";
import { ModContextMenu } from "./ModContextMenu";
import { useActiveProfile } from "../profiles/ActiveProfileProvider";
import { useModReinstallWorkflow } from "./useModReinstallWorkflow";

export type ModViewMode = "classic" | "grid" | "list" | "tech";

type ViewTransitionPhase = "idle" | "out" | "in";
type ViewTransitionVariant = "morph" | "wave" | "flip3d" | "blur";

const viewTransitionOutMs = 220;
const viewTransitionInMs = 420;
const DEFAULT_INSTALL_GAME_ID = "mhw";
const CARD_CATEGORY_LABELS_STORAGE_KEY = "hmm.modLibrary.showCardCategoryLabels";

const viewTransitionVariantByMode: Record<ModViewMode, ViewTransitionVariant> = {
  classic: "morph",
  grid: "wave",
  list: "flip3d",
  tech: "blur",
};

type ModLibraryPageProps = {
  onAction?: (actionId: string) => void;
};

function staggerStyle(index: number) {
  return { "--stagger-idx": index } as CSSProperties;
}

function prefersReducedMotion() {
  return typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
}

function readInitialCardCategoryLabelsVisibility() {
  if (typeof window === "undefined") {
    return true;
  }

  try {
    return window.localStorage.getItem(CARD_CATEGORY_LABELS_STORAGE_KEY) !== "false";
  } catch {
    return true;
  }
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
  status: UnsafeInstallStatus;
};

function isUnsafeRecoverySummary(summary: ModInstallSummary | undefined): summary is UnsafeRecoverySummary {
  return isUnsafeInstallStatus(summary?.status ?? "");
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
  const { activeProfile, activeProfileId } = useActiveProfile();
  const [query, setQuery] = useState("");
  const [activeFilter, setActiveFilter] = useState<ModLibraryFilter>(allLibraryFilter);
  const [categories, setCategories] = useState<CategoryItem[]>([]);
  const [viewMode, setViewMode] = useState<ModViewMode>("classic");
  const [showCardCategoryLabels, setShowCardCategoryLabels] = useState(readInitialCardCategoryLabelsVisibility);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [libraryItems, setLibraryItems] = useState<ModLibraryItem[]>(fallbackModLibraryItems);
  const libraryItemsRef = useRef<ModLibraryItem[]>(fallbackModLibraryItems);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const [scrollUiState, setScrollUiState] = useState(initialScrollUiState);
  const [contextMenuState, setContextMenuState] = useState<{ x: number; y: number; modId: string } | null>(null);
  const [detailDialogState, setDetailDialogState] = useState<{
    modId: string;
    fallbackItem: ModLibraryItem | null;
  } | null>(null);
  const [contextNotice, setContextNotice] = useState<string | null>(null);
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
      return matchesKeyword && matchesLibraryFilter(item, activeFilter);
    });
  }, [activeFilter, libraryItems, query]);

  const filterChips = useMemo(() => buildLibraryFilterChips(categories), [categories]);

  const selectedCount = selectedIds.size;
  const selectedItem = useMemo(() => {
    if (selectedIds.size !== 1) {
      return null;
    }

    const [selectedId] = Array.from(selectedIds);
    return libraryItems.find((item) => item.id === selectedId) ?? null;
  }, [libraryItems, selectedIds]);
  const managedInstallTaskActive = installTaskState.status === "starting" || installTaskState.status === "running";
  const canUninstallSelected =
    activeProfile.status === "ready" && selectedItem?.installSummary?.status === "installed";
  const canReinstallSelected =
    activeProfile.status === "ready" && selectedItem?.installSummary?.status === "installed";
  const canInstallSelected =
    selectedItem !== null &&
    activeProfile.status === "ready" &&
    selectedItem.installSummary?.status === "not_installed";
  const { handleViewModeChange, viewTransitionPhase, viewTransitionVariant } = useModViewTransition(
    viewMode,
    setViewMode,
  );

  const toggleCardCategoryLabels = useCallback(() => {
    setShowCardCategoryLabels((currentValue) => {
      const nextValue = !currentValue;

      try {
        window.localStorage.setItem(CARD_CATEGORY_LABELS_STORAGE_KEY, String(nextValue));
      } catch {
        // The in-memory UI state still works if storage is unavailable.
      }

      return nextValue;
    });
  }, []);

  const refreshInstallManifestStatuses = useCallback((items: ModLibraryItem[]) => {
    if (activeProfile.status !== "ready" || activeProfileId === null) {
      return Promise.resolve(items);
    }

    const modIds = Array.from(new Set(items.map((item) => item.id))).filter((id) => id.length > 0);
    if (modIds.length === 0) {
      return Promise.resolve(items);
    }

    return getInstallManifestStatus({
      gameId: DEFAULT_INSTALL_GAME_ID,
      profileId: activeProfileId,
      modIds,
    })
      .then((summaries) => applyInstallManifestStatusSummaries(items, summaries))
      .then((itemsWithManifestStatus) =>
        scanInstallRecovery({
          gameId: DEFAULT_INSTALL_GAME_ID,
          profileId: activeProfileId,
          modIds,
        })
          .then((summaries) => applyInstallRecoverySummaries(itemsWithManifestStatus, summaries))
          .catch(() => applyInstallRecoveryUnavailable(itemsWithManifestStatus)),
      )
      .catch(() => applyInstallRecoveryUnavailable(items));
  }, [activeProfile.status, activeProfileId]);

  const loadModLibraryItems = useCallback((mode: ModLibraryLoadMode) => {
    return loadModLibraryItemsForMode({
      mode,
      fallbackItems: fallbackModLibraryItems,
      getModLibrary,
      refreshInstallManifestStatuses,
    });
  }, [refreshInstallManifestStatuses]);

  const refreshModLibrary = useCallback(() => {
    return Promise.all([
      loadModLibraryItems("refresh").then((result) => {
        setLibraryItems((currentItems) => preserveItemsOnRefreshFailure(currentItems, result.items));
      }),
      listCategories()
        .then(setCategories)
        .catch(() => undefined),
    ]).then(() => undefined);
  }, [loadModLibraryItems]);

  const reinstallWorkflow = useModReinstallWorkflow({
    gameId: DEFAULT_INSTALL_GAME_ID,
    profileId: activeProfile.status === "ready" ? activeProfileId : null,
    selectedItem,
    writeTaskActive: managedInstallTaskActive,
    refreshLibrary: refreshModLibrary,
  });
  const { openReinstall } = reinstallWorkflow;

  useEffect(() => {
    libraryItemsRef.current = libraryItems;
  }, [libraryItems]);

  useEffect(() => {
    let cancelled = false;

    void loadModLibraryItems("initial").then((result) => {
      if (!cancelled && result.items) {
        setLibraryItems(result.items);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [loadModLibraryItems]);

  useEffect(() => {
    let cancelled = false;

    void listCategories()
      .then((loadedCategories) => {
        if (!cancelled) {
          setCategories(loadedCategories);
        }
      })
      .catch(() => undefined);

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    setActiveFilter((currentFilter) => normalizeLibraryFilter(currentFilter, filterChips));
  }, [filterChips]);

  useEffect(() => {
    if (!contextNotice) {
      return undefined;
    }

    const timeoutId = window.setTimeout(() => setContextNotice(null), 2600);
    return () => window.clearTimeout(timeoutId);
  }, [contextNotice]);

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
    if (selectedIds.size !== 1 || reinstallWorkflow.workflowActive) {
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
    if (!canInstallSelected) {
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
    if (selectedIds.size !== 1 || reinstallWorkflow.workflowActive) {
      return;
    }

    const [modId] = Array.from(selectedIds);
    const item = libraryItems.find((candidate) => candidate.id === modId);
    const modName = item?.name ?? modId;
    const recoveryPanelState = item ? recoveryPanelStateForItem(item) : null;
    if (activeProfile.status !== "ready" || activeProfileId === null) {
      setInstallPlanPreviewState({
        status: "error",
        modName,
        message: "配置档尚未就绪",
      });
      return;
    }
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
      profileId: activeProfileId,
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
    if (
      reinstallWorkflow.workflowActive ||
      !selectedItem ||
      selectedItem.installSummary?.status !== "installed"
    ) {
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
    if (activeProfile.status !== "ready" || activeProfileId === null) {
      setInstallPlanPreviewState({
        status: "error",
        modName,
        message: "配置档尚未就绪",
      });
      return;
    }

    pendingUninstallRef.current = null;
    setInstallPlanPreviewState({ status: "idle" });
    pendingInstallProgressEventsRef.current.clear();
    setTrackedInstallTaskState({ status: "starting", operation: "uninstall", modName });
    void startUninstallTask({
      gameId: DEFAULT_INSTALL_GAME_ID,
      modId,
      profileId: activeProfileId,
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
      case "install":
        startSelectedInstallTask();
        break;
      case "reinstall":
        pendingUninstallRef.current = null;
        setInstallPlanPreviewState({ status: "idle" });
        openReinstall();
        break;
      case "uninstall":
        promptSelectedUninstallTask();
        break;
      default:
        onAction?.(actionId);
        break;
    }
  };

  const handleContextMenuAction = (actionId: string, modId: string) => {
    switch (actionId) {
      case "info-settings":
        setDetailDialogState(createDetailDialogState(modId, libraryItemsRef.current));
        break;
      case "edit-files":
        setContextNotice("MOD 文件修改功能开发中");
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

  const installTaskActive = managedInstallTaskActive || reinstallWorkflow.workflowActive;
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
            filterChips={filterChips}
            viewMode={viewMode}
            showCardCategoryLabels={showCardCategoryLabels}
            onQueryChange={setQuery}
            onFilterChange={setActiveFilter}
            onToggleCardCategoryLabels={toggleCardCategoryLabels}
            onViewModeChange={handleViewModeChange}
          />
        </div>

        <div className="mod-library__actions-slot">
          <CompactActionPanel
            selectedCount={selectedCount}
            totalCount={visibleItems.length}
            selectedModId={selectedItem?.id ?? null}
            installTaskActive={installTaskActive}
            canInstallSelection={canInstallSelected}
            canReinstallSelection={canReinstallSelected}
            canUninstallSelection={canUninstallSelected}
            onImportCompleted={refreshModLibrary}
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

      <ReinstallPlanPreviewPanel
        state={reinstallWorkflow.dialogState}
        taskState={reinstallWorkflow.taskState}
        listenerStatus={reinstallWorkflow.listenerStatus}
        canConfirm={reinstallWorkflow.canConfirm}
        onClose={reinstallWorkflow.closeReinstall}
        onCandidateChange={reinstallWorkflow.selectCandidateRevision}
        onPreview={reinstallWorkflow.generatePreview}
        onConfirm={reinstallWorkflow.confirmReinstall}
        onRetryListener={reinstallWorkflow.retryTaskProgressListener}
      />

      {detailDialogState ? (
        <ModDetailDialog
          modId={detailDialogState.modId}
          fallbackItem={detailDialogState.fallbackItem}
          onClose={() => setDetailDialogState(null)}
          onSaved={refreshModLibrary}
        />
      ) : null}

      {contextNotice ? (
        <div className="mod-library__context-notice" role="status">
          {contextNotice}
        </div>
      ) : null}

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
                  showCategoryLabels={showCardCategoryLabels}
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
          onAction={handleContextMenuAction}
        />
      )}
    </section>
  );
}
