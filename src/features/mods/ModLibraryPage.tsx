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
import { LibraryToolbar } from "./LibraryToolbar";
import {
  InstallPlanDetailSheet,
  ManagedInstallTaskFeedback,
  UninstallConfirmationDialog,
  type InstallPlanDetailSheetState,
  type UninstallConfirmationState,
} from "./ModLifecycleFeedback";
import { ModDetailDialog, type ModDetailDialogTab } from "./ModDetailDialog";
import { ModLibraryPagination } from "./ModLibraryPagination";
import {
  ModLibraryEmptyState,
  ModLibraryInitialError,
  ModLibraryQueryBlockedState,
  ModLibraryQueryFeedback,
  ModLibrarySkeleton,
} from "./ModLibraryQueryFeedback";
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
  isManagedInstallTaskPhase,
  nextManagedInstallTaskStateFromProgress,
  type ManagedInstallTaskOperation,
  type ManagedInstallTaskState,
  type ManagedInstallTaskStateUpdate,
} from "./modInstallTaskState";
import {
  failClosedModInstallSummary,
  getManagedInstallTerminalToast,
  isManagedInstallTaskTerminal,
  shouldFailClosedManagedInstallTerminal,
  type ModLifecycleToast,
} from "./modLifecycleFeedbackState";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "./modImportTypes";
import { getModLibraryBackToTopTarget, scrollModLibraryBackToTop } from "./modLibraryBackToTop";
import { queryModLibrary } from "./modLibraryApi";
import { listCategories, type CategoryItem } from "./modCategoryApi";
import {
  allLibraryFilter,
  buildLibraryFilterChips,
  isSameLibraryFilter,
  normalizeLibraryFilter,
  type ModLibraryFilter,
} from "./modLibraryFilters";
import { isUnsafeInstallStatus } from "./modLibraryLoadState";
import { createDetailDialogState } from "./modLibraryRefresh";
import {
  isPlainBrowserDevRuntime,
  queryBrowserMockModLibrary,
} from "./modLibraryQueryState";
import {
  createModLibraryStatusProbe,
  refreshModLibraryDurableStatuses,
} from "./modLibraryRecoveryRefresh";
import { getModLibraryScrollUiState } from "./modLibraryScrollUi";
import type {
  ModInstallSummary,
  ModLibraryItem,
  ModLibraryPage as ModLibraryPageResult,
  ModLibraryProfileContext,
  QueryModLibraryInput,
} from "./modLibraryTypes";
import { applyModSelection } from "./modSelection";
import { modLibraryItems as fallbackModLibraryItems } from "./modsLibraryData";
import { ModContextMenu } from "./ModContextMenu";
import { useActiveProfile } from "../profiles/ActiveProfileProvider";
import { useModLibraryQuery } from "./useModLibraryQuery";
import { useModReinstallWorkflow } from "./useModReinstallWorkflow";
import { MOD_LIBRARY_QUERY_BUSY_MESSAGE } from "./compactActionAvailability";

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

function hasTauriRuntime() {
  return (
    typeof window !== "undefined"
    && "__TAURI_INTERNALS__" in window
    && typeof (window as { __TAURI_INTERNALS__?: { invoke?: unknown } }).__TAURI_INTERNALS__
      ?.invoke === "function"
  );
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

type UnsafeRecoverySummary = ModInstallSummary & {
  status: UnsafeInstallStatus;
};

type PendingUninstallConfirmation = UninstallConfirmationState & {
  profileId: string;
};

function isUnsafeRecoverySummary(summary: ModInstallSummary | undefined): summary is UnsafeRecoverySummary {
  return isUnsafeInstallStatus(summary?.status ?? "");
}

function recoveryPanelStateForItem(item: ModLibraryItem): InstallPlanDetailSheetState | null {
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
  const libraryItemsRef = useRef<ModLibraryItem[]>([]);
  const categoriesRef = useRef<CategoryItem[]>([]);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const renderedPageRef = useRef<number | null>(null);
  const [scrollUiState, setScrollUiState] = useState(initialScrollUiState);
  const [contextMenuState, setContextMenuState] = useState<{ x: number; y: number; modId: string } | null>(null);
  const [detailDialogState, setDetailDialogState] = useState<{
    modId: string;
    initialTab: ModDetailDialogTab;
    fallbackItem: ModLibraryItem | null;
  } | null>(null);
  const [installPlanDetailState, setInstallPlanDetailState] = useState<InstallPlanDetailSheetState>({
    status: "idle",
  });
  const [uninstallConfirmation, setUninstallConfirmation] = useState<PendingUninstallConfirmation | null>(null);
  const [installTaskState, setInstallTaskState] = useState<ManagedInstallTaskState>({ status: "idle" });
  const [lifecycleToast, setLifecycleToast] = useState<ModLifecycleToast | null>(null);
  const installTaskStateRef = useRef<ManagedInstallTaskState>(installTaskState);
  const activeProfileIdRef = useRef<string | null>(activeProfileId);
  const pageMountedRef = useRef(true);
  const handledInstallTerminalTaskIdsRef = useRef(new Set<string>());
  const startFailureToastSequenceRef = useRef(0);
  const pendingInstallProgressEventsRef = useRef<Map<string, TaskProgressEventDto>>(new Map());
  const installPlanPreviewGenerationRef = useRef(0);
  const categoriesRequestGenerationRef = useRef(0);

  useEffect(() => {
    activeProfileIdRef.current = activeProfile.status === "ready" ? activeProfileId : null;
  }, [activeProfile.status, activeProfileId]);

  const setTrackedInstallTaskState = useCallback((update: ManagedInstallTaskStateUpdate) => {
    const nextState = typeof update === "function" ? update(installTaskStateRef.current) : update;
    installTaskStateRef.current = nextState;
    setInstallTaskState(nextState);
  }, []);

  const resetContentScroll = useCallback(() => {
    contentRef.current?.scrollTo({ top: 0, behavior: "auto" });
  }, []);
  const resetPageInteraction = useCallback(() => {
    setSelectedIds(new Set());
    resetContentScroll();
  }, [resetContentScroll]);

  const profileContext = useMemo<ModLibraryProfileContext | null>(
    () => activeProfile.status === "ready" && activeProfileId !== null
      ? { gameId: DEFAULT_INSTALL_GAME_ID, profileId: activeProfileId }
      : null,
    [activeProfile.status, activeProfileId],
  );
  const browserPreviewEnabled = useMemo(
    () => isPlainBrowserDevRuntime({
      isDev: (import.meta as ImportMeta & { env?: { DEV?: boolean } }).env?.DEV === true,
      hasWindow: typeof window !== "undefined",
      hasTauriRuntime: hasTauriRuntime(),
    }),
    [],
  );
  const loadModLibraryPage = useCallback(async (input: QueryModLibraryInput): Promise<ModLibraryPageResult> => {
    const page = browserPreviewEnabled
      ? queryBrowserMockModLibrary(input, fallbackModLibraryItems, categoriesRef.current)
      : await queryModLibrary(input);
    if (input.profileContext === undefined) {
      return page;
    }

    const { profileId } = input.profileContext;
    const durableStatuses = await refreshModLibraryDurableStatuses(page.items, {
      loadManifestStatuses: (modIds) => getInstallManifestStatus({
        gameId: DEFAULT_INSTALL_GAME_ID,
        profileId,
        modIds,
      }),
      loadRecoveryStatuses: (modIds) => scanInstallRecovery({
        gameId: DEFAULT_INSTALL_GAME_ID,
        profileId,
        modIds,
      }),
    });
    return { ...page, items: durableStatuses.items };
  }, [browserPreviewEnabled]);
  const libraryQuery = useModLibraryQuery({
    rawSearch: query,
    filter: activeFilter,
    profileContext,
    loadPage: loadModLibraryPage,
  });
  const libraryPage = libraryQuery.page;
  const renderedPage = libraryPage?.page ?? null;
  const libraryItems = useMemo(() => libraryPage?.items ?? [], [libraryPage]);
  const {
    refresh: refreshLibraryPage,
    resetPage: resetLibraryPage,
    updateCurrentPageItems,
  } = libraryQuery;
  const libraryQueryBlocked = libraryQuery.blockedReason !== null;
  const libraryQueryBusy = !libraryQueryBlocked
    && (libraryQuery.initialLoading || libraryQuery.refreshing);
  const libraryQueryBlockedMessage = libraryQuery.blockedReason === "profile_context_required"
    ? "请先选择可用的配置档，再查看安装状态筛选。"
    : "当前安装状态筛选不受支持，请选择其他筛选条件。";
  const filterChips = useMemo(
    () => buildLibraryFilterChips(categories, {
      statusFiltersEnabled: profileContext !== null,
      statusDisabledReason: activeProfile.status === "loading" ? "配置档加载中" : "选择配置档后可用",
    }),
    [activeProfile.status, categories, profileContext],
  );

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

  const refreshCategories = useCallback(async () => {
    const generation = ++categoriesRequestGenerationRef.current;
    try {
      const loadedCategories = await listCategories();
      if (categoriesRequestGenerationRef.current === generation) {
        categoriesRef.current = loadedCategories;
        setCategories(loadedCategories);
      }
    } catch {
      // Category chips remain on their last successful snapshot.
    }
  }, []);

  const refreshModLibrary = useCallback(async () => {
    setSelectedIds(new Set());
    await Promise.all([refreshLibraryPage(), refreshCategories()]);
  }, [refreshCategories, refreshLibraryPage]);

  const refreshTerminalDurableStatus = useCallback(
    (profileId: string, modId: string, modName: string) =>
      refreshModLibraryDurableStatuses([createModLibraryStatusProbe(modId, modName)], {
        loadManifestStatuses: (modIds) => getInstallManifestStatus({
          gameId: DEFAULT_INSTALL_GAME_ID,
          profileId,
          modIds,
        }),
        loadRecoveryStatuses: (modIds) => scanInstallRecovery({
          gameId: DEFAULT_INSTALL_GAME_ID,
          profileId,
          modIds,
        }),
      }),
    [],
  );

  const reinstallWorkflow = useModReinstallWorkflow({
    gameId: DEFAULT_INSTALL_GAME_ID,
    profileId: activeProfile.status === "ready" ? activeProfileId : null,
    selectedItem,
    writeTaskActive: managedInstallTaskActive,
    refreshLibrary: refreshModLibrary,
  });
  const { openReinstall } = reinstallWorkflow;
  const uninstallBlockerMessage = useMemo(() => {
    if (uninstallConfirmation === null) {
      return null;
    }
    if (libraryQueryBusy) {
      return MOD_LIBRARY_QUERY_BUSY_MESSAGE;
    }
    if (
      activeProfile.status !== "ready"
      || activeProfileId === null
      || activeProfileId !== uninstallConfirmation.profileId
    ) {
      return "配置档状态已变化，当前不能安全卸载。";
    }

    const currentItem = libraryItems.find((item) => item.id === uninstallConfirmation.modId);
    const currentSummary = currentItem?.installSummary;
    if (currentSummary?.status !== "installed") {
      return "后端安装状态已变化，请关闭并刷新后重试。";
    }
    return currentSummary.managedFileCount === uninstallConfirmation.managedFileCount
      && currentSummary.backupCount === uninstallConfirmation.backupCount
      ? null
      : "后端安装摘要已变化，请关闭并刷新后重试。";
  }, [activeProfile.status, activeProfileId, libraryItems, libraryQueryBusy, uninstallConfirmation]);

  const confirmSelectedReinstall = () => {
    if (libraryQueryBusy) {
      return;
    }
    reinstallWorkflow.confirmReinstall();
  };

  useEffect(() => {
    libraryItemsRef.current = libraryItems;
  }, [libraryItems]);

  useEffect(() => {
    pageMountedRef.current = true;
    return () => {
      pageMountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    void refreshCategories();
  }, [refreshCategories]);

  useEffect(() => {
    const normalizedFilter = normalizeLibraryFilter(activeFilter, filterChips);
    if (normalizedFilter === activeFilter) {
      return;
    }
    if (!isSameLibraryFilter(activeFilter, normalizedFilter)) {
      resetLibraryPage();
      resetPageInteraction();
    }
    setActiveFilter(normalizedFilter);
  }, [activeFilter, filterChips, resetLibraryPage, resetPageInteraction]);

  useEffect(() => {
    setSelectedIds(new Set());
    resetLibraryPage();
    resetContentScroll();
  }, [activeProfile.status, activeProfileId, resetContentScroll, resetLibraryPage]);

  useEffect(() => {
    const previousPage = renderedPageRef.current;
    renderedPageRef.current = renderedPage;
    if (renderedPage !== null && previousPage !== null && renderedPage !== previousPage) {
      resetContentScroll();
    }
  }, [renderedPage, resetContentScroll]);

  useEffect(() => {
    setSelectedIds(new Set());
    setContextMenuState(null);
  }, [libraryPage]);

  useEffect(() => {
    if (libraryQueryBusy) {
      setContextMenuState(null);
    }
  }, [libraryQueryBusy]);

  useEffect(() => {
    if (!isManagedInstallTaskTerminal(installTaskState)) {
      return;
    }
    if (installTaskState.taskId === null) {
      return;
    }
    const terminalTask = installTaskState;
    const terminalTaskId: string = installTaskState.taskId;
    if (handledInstallTerminalTaskIdsRef.current.has(terminalTaskId)) {
      return;
    }

    handledInstallTerminalTaskIdsRef.current.add(terminalTaskId);

    const refreshTerminalFacts = async () => {
      if (activeProfileIdRef.current !== terminalTask.profileId) {
        setLifecycleToast(null);
        return;
      }

      resetContentScroll();
      const [pageRefresh, durableRefresh] = await Promise.allSettled([
        refreshModLibrary(),
        refreshTerminalDurableStatus(terminalTask.profileId, terminalTask.modId, terminalTask.modName),
      ]);
      if (!pageMountedRef.current) {
        return;
      }

      const currentProfileId = activeProfileIdRef.current;
      if (currentProfileId !== terminalTask.profileId) {
        setLifecycleToast(null);
        return;
      }

      const durableStatus = durableRefresh.status === "fulfilled" ? durableRefresh.value : null;
      const terminalRefresh = {
        verified: durableStatus?.verified ?? false,
        status: durableStatus?.items[0]?.installSummary?.status ?? null,
      };
      if (
        pageRefresh.status === "rejected"
        || shouldFailClosedManagedInstallTerminal(terminalTask, terminalRefresh)
      ) {
        updateCurrentPageItems((items) =>
          failClosedModInstallSummary(items, terminalTask.modId));
      }

      setLifecycleToast(getManagedInstallTerminalToast(terminalTask, terminalRefresh));
    };

    void refreshTerminalFacts();
  }, [
    installTaskState,
    refreshModLibrary,
    refreshTerminalDurableStatus,
    resetContentScroll,
    updateCurrentPageItems,
  ]);

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
  }, [libraryItems.length, scrollUiState.showScrollUi, updateScrollUiState]);

  const handleQueryChange = (nextQuery: string) => {
    setQuery(nextQuery);
    resetPageInteraction();
  };

  const handleFilterChange = (nextFilter: ModLibraryFilter) => {
    setActiveFilter(nextFilter);
    libraryQuery.resetPage();
    resetPageInteraction();
  };

  const handlePageChange = (nextPage: number) => {
    libraryQuery.setPage(nextPage);
    resetPageInteraction();
  };

  const handlePageSizeChange = (nextPageSize: Parameters<typeof libraryQuery.setPageSize>[0]) => {
    libraryQuery.setPageSize(nextPageSize);
    resetPageInteraction();
  };

  const resetSearchAndFilter = () => {
    setQuery("");
    setActiveFilter(allLibraryFilter);
    libraryQuery.resetPage();
    resetPageInteraction();
  };

  const retryLibraryQuery = () => {
    void libraryQuery.refresh().catch(() => undefined);
  };

  const selectCard = (id: string) => {
    if (libraryQueryBusy) {
      return;
    }
    setSelectedIds((prev) => applyModSelection(prev, id, "replace"));
  };

  const handleContextMenu = (modId: string, x: number, y: number) => {
    if (libraryQueryBusy) {
      return;
    }
    setContextMenuState({ x, y, modId });
    // If the card isn't selected, select it
    if (!selectedIds.has(modId)) {
      setSelectedIds((prev) => applyModSelection(prev, modId, "replace"));
    }
  };

  const selectAll = () => {
    if (libraryQueryBusy) {
      return;
    }
    setSelectedIds(new Set(libraryItems.map((item) => item.id)));
  };

  const invertSelection = () => {
    if (libraryQueryBusy) {
      return;
    }
    setSelectedIds((prev) => {
      const next = new Set<string>();
      for (const item of libraryItems) {
        if (!prev.has(item.id)) {
          next.add(item.id);
        }
      }
      return next;
    });
  };

  const previewSelectedInstallPlan = () => {
    if (libraryQueryBusy || selectedIds.size !== 1 || reinstallWorkflow.workflowActive) {
      return;
    }

    const previewGeneration = ++installPlanPreviewGenerationRef.current;
    const [modId] = Array.from(selectedIds);
    const item = libraryItems.find((candidate) => candidate.id === modId);
    const modName = item?.name ?? modId;
    const recoveryPanelState = item ? recoveryPanelStateForItem(item) : null;
    if (recoveryPanelState) {
      setInstallPlanDetailState(recoveryPanelState);
      return;
    }
    if (!canInstallSelected) {
      return;
    }

    setInstallPlanDetailState({ status: "loading", modName });
    void previewInstallPlanForImportedMod({
      gameId: DEFAULT_INSTALL_GAME_ID,
      modId,
      layerName: "base",
      layerPriority: 0,
    })
      .then((plan) => {
        if (installPlanPreviewGenerationRef.current !== previewGeneration) {
          return;
        }
        setInstallPlanDetailState({ status: "ready", modName, plan });
      })
      .catch((error: unknown) => {
        if (installPlanPreviewGenerationRef.current !== previewGeneration) {
          return;
        }
        setInstallPlanDetailState({
          status: "error",
          modName,
          message: installPlanPreviewErrorMessage(error),
        });
      });
  };

  const startSelectedInstallTask = () => {
    if (libraryQueryBusy || selectedIds.size !== 1 || reinstallWorkflow.workflowActive) {
      return;
    }

    const [modId] = Array.from(selectedIds);
    const item = libraryItems.find((candidate) => candidate.id === modId);
    const modName = item?.name ?? modId;
    const recoveryPanelState = item ? recoveryPanelStateForItem(item) : null;
    if (activeProfile.status !== "ready" || activeProfileId === null) {
      setInstallPlanDetailState({
        status: "error",
        modName,
        message: "配置档尚未就绪",
      });
      return;
    }
    if (!canInstallSelected || recoveryPanelState) {
      if (recoveryPanelState) {
        setInstallPlanDetailState(recoveryPanelState);
      }
      return;
    }

    setInstallPlanDetailState({ status: "idle" });
    setLifecycleToast(null);
    pendingInstallProgressEventsRef.current.clear();
    setTrackedInstallTaskState({
      status: "starting",
      operation: "install",
      profileId: activeProfileId,
      modId,
      modName,
    });
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
            profileId: activeProfileId,
            modId,
            modName,
            phase: "install.failed",
            message: "安装任务返回了无效类型",
          });
          setLifecycleToast({
            id: `install-start-${++startFailureToastSequenceRef.current}`,
            title: "安装任务启动失败",
            message: modName,
            tone: "danger",
          });
          return;
        }

        setTrackedInstallTaskState((current) => {
          const runningState: ManagedInstallTaskState = {
            status: "running",
            operation: "install",
            taskId: task.taskId,
            profileId: activeProfileId,
            modId,
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
          profileId: activeProfileId,
          modId,
          modName,
          phase: "install.failed",
          message: installTaskErrorMessage(error, "install"),
        });
        setLifecycleToast({
          id: `install-start-${++startFailureToastSequenceRef.current}`,
          title: "安装任务启动失败",
          message: installTaskErrorMessage(error, "install"),
          tone: "danger",
        });
      });
  };

  const promptSelectedUninstallTask = () => {
    if (
      libraryQueryBusy ||
      reinstallWorkflow.workflowActive ||
      activeProfileId === null ||
      !selectedItem ||
      selectedItem.installSummary?.status !== "installed"
    ) {
      return;
    }

    installPlanPreviewGenerationRef.current += 1;
    setInstallPlanDetailState({ status: "idle" });
    setUninstallConfirmation({
      profileId: activeProfileId,
      modId: selectedItem.id,
      modName: selectedItem.name,
      managedFileCount: selectedItem.installSummary.managedFileCount,
      backupCount: selectedItem.installSummary.backupCount,
    });
    setTrackedInstallTaskState({ status: "idle" });
  };

  const cancelUninstallConfirmation = () => {
    setUninstallConfirmation(null);
  };

  const startSelectedUninstallTask = () => {
    if (libraryQueryBusy || !uninstallConfirmation || uninstallBlockerMessage !== null) {
      return;
    }

    const { profileId, modId, modName } = uninstallConfirmation;
    if (activeProfile.status !== "ready" || activeProfileId !== profileId) {
      return;
    }

    setUninstallConfirmation(null);
    setLifecycleToast(null);
    pendingInstallProgressEventsRef.current.clear();
    setTrackedInstallTaskState({ status: "starting", operation: "uninstall", profileId, modId, modName });
    void startUninstallTask({
      gameId: DEFAULT_INSTALL_GAME_ID,
      modId,
      profileId,
    })
      .then((task) => {
        const pendingProgressEvent = pendingInstallProgressEventsRef.current.get(task.taskId) ?? null;
        pendingInstallProgressEventsRef.current.clear();

        if (task.kind !== "install") {
          setTrackedInstallTaskState({
            status: "failed",
            operation: "uninstall",
            taskId: null,
            profileId,
            modId,
            modName,
            phase: "install.uninstall.failed",
            message: "卸载任务返回了无效类型",
          });
          setLifecycleToast({
            id: `uninstall-start-${++startFailureToastSequenceRef.current}`,
            title: "卸载任务启动失败",
            message: modName,
            tone: "danger",
          });
          return;
        }

        setTrackedInstallTaskState((current) => {
          const runningState: ManagedInstallTaskState = {
            status: "running",
            operation: "uninstall",
            taskId: task.taskId,
            profileId,
            modId,
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
          profileId,
          modId,
          modName,
          phase: "install.uninstall.failed",
          message: installTaskErrorMessage(error, "uninstall"),
        });
        setLifecycleToast({
          id: `uninstall-start-${++startFailureToastSequenceRef.current}`,
          title: "卸载任务启动失败",
          message: installTaskErrorMessage(error, "uninstall"),
          tone: "danger",
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
      case "refresh":
        void refreshModLibrary().catch(() => undefined);
        break;
      case "preview-plan":
        previewSelectedInstallPlan();
        break;
      case "install":
        startSelectedInstallTask();
        break;
      case "reinstall":
        if (libraryQueryBusy) {
          break;
        }
        setUninstallConfirmation(null);
        installPlanPreviewGenerationRef.current += 1;
        setInstallPlanDetailState({ status: "idle" });
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
    if (libraryQueryBusy) {
      return;
    }
    switch (actionId) {
      case "info-settings":
        setDetailDialogState(createDetailDialogState(modId, libraryItemsRef.current, "details"));
        break;
      case "edit-files":
        setDetailDialogState(createDetailDialogState(modId, libraryItemsRef.current, "replacement"));
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
  const closeInstallPlanDetail = () => {
    installPlanPreviewGenerationRef.current += 1;
    setInstallPlanDetailState({ status: "idle" });
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
            onQueryChange={handleQueryChange}
            onQuerySubmit={libraryQuery.flushSearch}
            onFilterChange={handleFilterChange}
            onToggleCardCategoryLabels={toggleCardCategoryLabels}
            onViewModeChange={handleViewModeChange}
          />
        </div>

        <div className="mod-library__actions-slot">
          <CompactActionPanel
            selectedCount={selectedCount}
            totalCount={libraryItems.length}
            selectedModId={selectedItem?.id ?? null}
            installTaskActive={installTaskActive}
            libraryQueryBusy={libraryQueryBusy}
            profileReady={activeProfile.status === "ready" && activeProfileId !== null}
            canInstallSelection={canInstallSelected}
            canReinstallSelection={canReinstallSelected}
            canUninstallSelection={canUninstallSelected}
            onImportCompleted={refreshModLibrary}
            onAction={handleAction}
          />
        </div>
      </div>

      <InstallPlanDetailSheet
        state={installPlanDetailState}
        onClose={closeInstallPlanDetail}
      />

      <UninstallConfirmationDialog
        state={uninstallConfirmation}
        blockerMessage={uninstallBlockerMessage}
        onCancel={cancelUninstallConfirmation}
        onConfirm={startSelectedUninstallTask}
      />

      <ManagedInstallTaskFeedback
        taskState={installTaskState}
        toast={lifecycleToast}
        onDismissToast={() => setLifecycleToast(null)}
      />

      <ReinstallPlanPreviewPanel
        state={reinstallWorkflow.dialogState}
        taskState={reinstallWorkflow.taskState}
        listenerStatus={reinstallWorkflow.listenerStatus}
        canConfirm={reinstallWorkflow.canConfirm && !libraryQueryBusy}
        onClose={reinstallWorkflow.closeReinstall}
        onCandidateChange={reinstallWorkflow.selectCandidateRevision}
        onPreview={reinstallWorkflow.generatePreview}
        onConfirm={confirmSelectedReinstall}
        onRetryListener={reinstallWorkflow.retryTaskProgressListener}
      />

      {detailDialogState ? (
        <ModDetailDialog
          modId={detailDialogState.modId}
          fallbackItem={detailDialogState.fallbackItem}
          initialTab={detailDialogState.initialTab}
          gameId={DEFAULT_INSTALL_GAME_ID}
          profileId={activeProfile.status === "ready" ? activeProfileId : null}
          installStatus={detailDialogState.fallbackItem?.installSummary?.status}
          onClose={() => setDetailDialogState(null)}
          onSaved={refreshModLibrary}
        />
      ) : null}

      <div className="mod-library__content-shell" data-scroll-ui={showScrollUi ? "visible" : "hidden"}>
        <ModLibraryQueryFeedback
          busy={!libraryQueryBlocked && libraryQuery.refreshing}
          errorMessage={libraryPage === null ? null : libraryQuery.errorMessage}
          onRetry={retryLibraryQuery}
        />

        <div
          ref={contentRef}
          className="mod-library__content"
          aria-busy={libraryQueryBusy}
        >
          {showScrollUi ? (
            <div className="mod-library__main-floating-actions">
              <BackToTopButton onClick={handleBackToTop} />
            </div>
          ) : null}

          {libraryQueryBlocked ? (
            <ModLibraryQueryBlockedState
              message={libraryQueryBlockedMessage}
              onReset={resetSearchAndFilter}
            />
          ) : libraryQuery.initialLoading ? (
            <ModLibrarySkeleton viewMode={viewMode} />
          ) : libraryPage === null ? (
            <ModLibraryInitialError
              message={libraryQuery.errorMessage ?? "Mod 列表加载失败，请稍后重试"}
              onRetry={retryLibraryQuery}
            />
          ) : libraryPage.libraryTotal === 0 ? (
            <ModLibraryEmptyState kind="library" />
          ) : libraryPage.matchingTotal === 0 ? (
            <ModLibraryEmptyState kind="matches" onReset={resetSearchAndFilter} />
          ) : (
            <div
              className={`mod-grid view-${viewMode}`}
              role="list"
              data-view-transition={viewTransitionPhase}
              data-view-transition-variant={viewTransitionVariant}
            >
              {libraryItems.map((item, index) => (
                <ModPosterCard
                  key={item.id}
                  item={item}
                  selected={selectedIds.has(item.id)}
                  interactionDisabled={libraryQueryBusy}
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

      <ModLibraryPagination
        page={libraryPage?.page ?? 1}
        pageSize={libraryQuery.pageSize}
        matchingTotal={libraryPage?.matchingTotal ?? 0}
        busy={libraryQueryBusy}
        onPageChange={handlePageChange}
        onPageSizeChange={handlePageSizeChange}
      />

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
