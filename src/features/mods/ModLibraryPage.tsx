import {
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { BackToTopButton } from "./BackToTopButton";
import { CompactActionPanel } from "./CompactActionPanel";
import { LibraryToolbar } from "./LibraryToolbar";
import {
  modLibraryCopy,
  renderModSelectionNotice,
  type ModLibraryCopy,
} from "./modLibraryCopy";
import { modLifecycleCopy } from "./modLifecycleCopy";
import { useBatchModLifecycleWorkflow } from "./batch-lifecycle/useBatchModLifecycleWorkflow.ts";
import {
  useBatchModLifecycleCapability,
} from "./batch-lifecycle/useBatchModLifecycleCapability.ts";
import {
  batchModLifecycleCopy,
  getBatchCapabilityUnavailableLabel,
} from "./batch-lifecycle/batchModLifecycleCopy.ts";
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
import { BatchModLifecyclePreviewPanel } from "./batch-lifecycle/BatchModLifecyclePreviewPanel.tsx";
import {
  BatchModLifecycleResultPanel,
  BatchModLifecycleRunningPanel,
} from "./batch-lifecycle/BatchModLifecycleResultPanel.tsx";
import { DEFAULT_BATCH_EXECUTION_POLICY } from "./batch-lifecycle/useBatchModLifecycleWorkflow.ts";
import type { BatchModLifecycleReplacementTargetFacts } from "./batch-lifecycle/batchModLifecycleTypes.ts";
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
import { getModRevisions, queryModLibrary } from "./modLibraryApi";
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
import {
  countSelectedOnPage,
  createInitialModSelectionState,
  reduceModSelection,
  type ModCardSelectionIntent,
  type ModSelectionResetReason,
} from "./modSelection";
import { modLibraryItems as fallbackModLibraryItems } from "./modsLibraryData";
import { ModContextMenu } from "./ModContextMenu";
import { useActiveProfile } from "../profiles/ActiveProfileProvider";
import { useModLibraryQuery } from "./useModLibraryQuery";
import { useModReinstallWorkflow } from "./useModReinstallWorkflow";
import {
  analyzeImportedModReplacement,
  listReplacementTargets,
} from "../replacements/replacementApi";

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

function installPlanPreviewErrorMessage(
  error: unknown,
  planPreview: ModLibraryCopy["page"]["planPreview"],
) {
  const code =
    typeof error === "object" && error !== null && "code" in error && typeof error.code === "string"
      ? error.code
      : null;

  switch (code) {
    case "install_planning_imported_mod_not_found":
      return planPreview.modNotFound;
    case "install_planning_imported_mod_analysis_unavailable":
      return planPreview.analysisUnavailable;
    case "install_planning_imported_mod_sandbox_unavailable":
    case "install_planning_imported_mod_file_scan_unavailable":
      return planPreview.archiveUnavailable;
    case "install_planning_game_adapter_not_found":
    case "game_id_invalid":
      return planPreview.unsupportedGame;
    default:
      return planPreview.failed;
  }
}

function installTaskErrorMessage(
  error: unknown,
  operation: ManagedInstallTaskOperation,
  lifecycleStart: ModLibraryCopy["page"]["lifecycleStart"],
) {
  const code =
    typeof error === "object" && error !== null && "code" in error && typeof error.code === "string"
      ? error.code
      : null;

  switch (code) {
    case "install_planning_imported_mod_not_found":
      return lifecycleStart.modNotFound;
    case "install_planning_imported_mod_analysis_unavailable":
      return lifecycleStart.analysisUnavailable;
    case "install_planning_game_adapter_not_found":
    case "game_id_invalid":
      return operation === "uninstall"
        ? lifecycleStart.unsupportedUninstall
        : lifecycleStart.unsupportedInstall;
    default:
      return operation === "uninstall"
        ? lifecycleStart.uninstallStartFailed
        : lifecycleStart.installStartFailed;
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
  const { locale } = useI18n();
  const copy = resolveCopy(modLibraryCopy, locale);
  const lifecycleCopy = resolveCopy(modLifecycleCopy, locale);
  const batchCopy = resolveCopy(batchModLifecycleCopy, locale);
  // 事件监听回调经 ref 取词，避免语言切换导致监听器重建。
  const lifecycleCopyRef = useRef(lifecycleCopy);
  lifecycleCopyRef.current = lifecycleCopy;
  const { activeProfile, activeProfileId } = useActiveProfile();
  const [query, setQuery] = useState("");
  const [activeFilter, setActiveFilter] = useState<ModLibraryFilter>(allLibraryFilter);
  const [categories, setCategories] = useState<CategoryItem[]>([]);
  const [viewMode, setViewMode] = useState<ModViewMode>("classic");
  const [showCardCategoryLabels, setShowCardCategoryLabels] = useState(readInitialCardCategoryLabelsVisibility);
  const [selectionState, dispatchSelection] = useReducer(
    reduceModSelection,
    createInitialModSelectionState(),
  );
  const {
    mode: selectionMode,
    selectedIds,
    notice: selectionNotice,
  } = selectionState;
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
  const handledBatchTerminalAttemptsRef = useRef(new Set<string>());
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
  const resetPageInteraction = useCallback((reason: ModSelectionResetReason = "query-changed") => {
    dispatchSelection({ type: "reset-context", reason });
    resetContentScroll();
  }, [resetContentScroll]);

  const profileContext = useMemo<ModLibraryProfileContext | null>(
    () => activeProfile.status === "ready" && activeProfileId !== null
      ? { gameId: DEFAULT_INSTALL_GAME_ID, profileId: activeProfileId }
      : null,
    [activeProfile.status, activeProfileId],
  );
  const batchCapability = useBatchModLifecycleCapability();
  const batchCapabilityUnavailableReason = getBatchCapabilityUnavailableLabel(
    batchCapability.capability,
    batchCopy.capability,
  );
  const batchPreviewUnavailableReason =
    batchCapability.status === "loading" || !batchCapability.capability?.previewAvailable
      ? batchCapabilityUnavailableReason
      : undefined;
  const batchWriteUnavailableReason =
    batchCapability.status === "loading" || !batchCapability.capability?.writeAvailable
      ? batchCapabilityUnavailableReason
      : undefined;
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
    if (browserPreviewEnabled || input.profileContext === undefined) {
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
  const loadBatchReplacementTargetFacts = useCallback(
    async (modIds: string[]): Promise<BatchModLifecycleReplacementTargetFacts[]> => {
      if (profileContext === null) {
        throw new Error("profile context required");
      }
      return Promise.all(
        modIds.map(async (modId) => {
          try {
            const [analysis, targets] = await Promise.all([
              analyzeImportedModReplacement({
                gameId: DEFAULT_INSTALL_GAME_ID,
                profileId: profileContext.profileId,
                modId,
              }),
              listReplacementTargets({ gameId: DEFAULT_INSTALL_GAME_ID, modId }),
            ]);
            return {
              modId,
              retargetable: analysis.retargetable,
              installedTargetId: analysis.installedTargetId ?? null,
              targets: targets.map(({ id, displayName, secondaryName }) => ({
                id,
                displayName,
                secondaryName,
              })),
            };
          } catch {
            return {
              modId,
              retargetable: false,
              installedTargetId: null,
              targets: [],
            };
          }
        }),
      );
    },
    [profileContext],
  );
  const batchWorkflow = useBatchModLifecycleWorkflow({
    gameId: profileContext === null ? null : DEFAULT_INSTALL_GAME_ID,
    profileId: profileContext?.profileId ?? null,
    loadManifestStatuses: (modIds) =>
      profileContext === null
        ? Promise.reject(new Error("profile context required"))
        : getInstallManifestStatus({
            // Batch inputs require the exact installed revision from manifest facts.
            // Recovery and manifest safety are revalidated by the backend preview.
            profileId: profileContext.profileId,
            modIds,
          }),
    loadRevisions: (modId) => getModRevisions({ modId }),
    loadReplacementTargetFacts: loadBatchReplacementTargetFacts,
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
  const libraryQueryErrorMessage =
    libraryQuery.errorCode === null
      ? null
      : libraryQuery.errorCode === "unknown"
        ? copy.page.loadFailedFallback
        : copy.page.queryErrors[libraryQuery.errorCode];
  const libraryQueryBlockedMessage = libraryQuery.blockedReason === "profile_context_required"
    ? copy.page.statusFilter.needProfile
    : copy.page.statusFilter.unsupported;
  const filterChips = useMemo(
    () => buildLibraryFilterChips(categories, {
      statusFiltersEnabled: profileContext !== null,
      statusDisabledReason:
        activeProfile.status === "loading"
          ? copy.page.statusFilter.profileLoading
          : copy.page.statusFilter.selectProfile,
      filterLabels: copy.filters,
    }),
    [activeProfile.status, categories, copy, profileContext],
  );

  const selectedCount = selectedIds.size;
  const selectedPageCount = useMemo(
    () => countSelectedOnPage(selectedIds, libraryItems.map((item) => item.id)),
    [libraryItems, selectedIds],
  );
  const selectedItem = useMemo(() => {
    if (selectionMode !== "single" || selectedIds.size !== 1) {
      return null;
    }

    const [selectedId] = Array.from(selectedIds);
    return libraryItems.find((item) => item.id === selectedId) ?? null;
  }, [libraryItems, selectedIds, selectionMode]);
  const managedInstallTaskActive = installTaskState.status === "starting" || installTaskState.status === "running";
  const batchWriteAvailable = batchCapability.capability?.writeAvailable === true;
  const batchPreviewAvailable = batchCapability.capability?.previewAvailable === true;
  const canUninstallSelected =
    activeProfile.status === "ready"
    && (selectionMode === "batch"
      ? selectedIds.size > 0 && batchWriteAvailable
      : selectedItem?.installSummary?.status === "installed");
  const canReinstallSelected =
    activeProfile.status === "ready"
    && (selectionMode === "batch"
      ? selectedIds.size > 0 && batchWriteAvailable
      : selectedItem?.installSummary?.status === "installed");
  const canInstallSelected =
    activeProfile.status === "ready"
    && (selectionMode === "batch"
      ? selectedIds.size > 0 && batchWriteAvailable
      : selectedItem !== null && selectedItem.installSummary?.status === "not_installed");
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
    dispatchSelection({ type: "reset-context", reason: "library-refreshed" });
    await Promise.all([refreshLibraryPage(), refreshCategories()]);
  }, [refreshCategories, refreshLibraryPage]);

  const refreshModLibraryAfterWrite = useCallback(async () => {
    resetContentScroll();
    await refreshModLibrary();
  }, [refreshModLibrary, resetContentScroll]);

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
    refreshLibrary: refreshModLibraryAfterWrite,
  });
  const { openReinstall } = reinstallWorkflow;
  const uninstallBlockerMessage = useMemo(() => {
    if (uninstallConfirmation === null) {
      return null;
    }
    if (libraryQueryBusy) {
      return copy.page.queryBusy;
    }
    if (
      activeProfile.status !== "ready"
      || activeProfileId === null
      || activeProfileId !== uninstallConfirmation.profileId
    ) {
      return copy.page.uninstallBlocked.profileChanged;
    }

    const currentItem = libraryItems.find((item) => item.id === uninstallConfirmation.modId);
    const currentSummary = currentItem?.installSummary;
    if (currentSummary?.status !== "installed") {
      return copy.page.uninstallBlocked.backendStatusChanged;
    }
    return currentSummary.managedFileCount === uninstallConfirmation.managedFileCount
      && currentSummary.backupCount === uninstallConfirmation.backupCount
      ? null
      : copy.page.uninstallBlocked.backendSummaryChanged;
  }, [activeProfile.status, activeProfileId, copy, libraryItems, libraryQueryBusy, uninstallConfirmation]);

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
      resetPageInteraction("filters-changed");
    }
    setActiveFilter(normalizedFilter);
  }, [activeFilter, filterChips, resetLibraryPage, resetPageInteraction]);

  useEffect(() => {
    dispatchSelection({ type: "reset-context", reason: "profile-changed" });
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
    setContextMenuState(null);
  }, [libraryPage]);

  useEffect(() => {
    if (libraryQueryBusy) {
      setContextMenuState(null);
    }
  }, [libraryQueryBusy]);

  useEffect(() => {
    if (selectionMode !== "batch") {
      return undefined;
    }

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented || contextMenuState !== null) {
        return;
      }
      if (document.querySelector('[role="dialog"][aria-modal="true"]') !== null) {
        return;
      }

      dispatchSelection({ type: "exit-batch" });
    };

    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [contextMenuState, selectionMode]);

  useEffect(() => {
    if (selectionNotice === null) {
      return undefined;
    }

    const noticeTimer = window.setTimeout(() => {
      dispatchSelection({ type: "dismiss-notice" });
    }, 4000);
    return () => window.clearTimeout(noticeTimer);
  }, [selectionNotice]);

  // Selection changes invalidate any in-flight batch preview; the next action starts fresh.
  useEffect(() => {
    batchWorkflow.reset();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedIds, selectionMode]);

  useEffect(() => {
    if (batchWorkflow.state.status !== "result") {
      return;
    }
    const batchAttemptKey = `${batchWorkflow.state.batchId}:${batchWorkflow.state.attemptNumber}`;
    if (handledBatchTerminalAttemptsRef.current.has(batchAttemptKey)) {
      return;
    }

    handledBatchTerminalAttemptsRef.current.add(batchAttemptKey);
    void refreshLibraryPage().catch(() => undefined);
  }, [batchWorkflow.state, refreshLibraryPage]);

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

      const [pageRefresh, durableRefresh] = await Promise.allSettled([
        refreshModLibraryAfterWrite(),
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

      setLifecycleToast(getManagedInstallTerminalToast(terminalTask, terminalRefresh, lifecycleCopyRef.current.terminalToasts));
    };

    void refreshTerminalFacts();
  }, [
    installTaskState,
    refreshModLibraryAfterWrite,
    refreshTerminalDurableStatus,
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
        return nextManagedInstallTaskStateFromProgress(current, event.payload, lifecycleCopyRef.current.installTask);
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

  const selectionInteractionDisabledReason = libraryQueryBusy
    ? copy.page.queryBusy
    : managedInstallTaskActive || reinstallWorkflow.workflowActive
      ? copy.page.cardAction.waitInstallTask
      : batchWorkflow.state.status !== "idle"
        ? copy.page.cardAction.closeBatchFirst
        : undefined;
  const selectionInteractionLocked = selectionInteractionDisabledReason !== undefined;
  const contextMenuLifecycleAction = useMemo(() => {
    const item = contextMenuState === null
      ? null
      : libraryItems.find((candidate) => candidate.id === contextMenuState.modId) ?? null;
    const status = item?.installSummary?.status;
    const label = status === "installed"
      ? copy.page.cardAction.uninstallLabel
      : status === "not_installed"
        ? copy.page.cardAction.installLabel
        : copy.page.cardAction.installOrUninstallLabel;

    if (item === null) {
      return { actionId: null, label, disabledReason: copy.page.cardAction.notInList } as const;
    }
    if (selectionMode === "batch") {
      return { actionId: null, label, disabledReason: copy.page.cardAction.batchSelecting } as const;
    }
    if (selectionInteractionDisabledReason !== undefined) {
      return { actionId: null, label, disabledReason: selectionInteractionDisabledReason } as const;
    }
    if (activeProfile.status !== "ready" || activeProfileId === null) {
      return { actionId: null, label, disabledReason: copy.page.cardAction.selectProfileFirst } as const;
    }
    if (recoveryPanelStateForItem(item) !== null) {
      return { actionId: null, label, disabledReason: copy.page.cardAction.resolveRecoveryFirst } as const;
    }
    if (status === "installed") {
      return { actionId: "uninstall", label, tone: "danger" } as const;
    }
    if (status === "not_installed") {
      return { actionId: "install", label, tone: "neutral" } as const;
    }
    return { actionId: null, label, disabledReason: copy.page.cardAction.statusNotActionable } as const;
  }, [
    activeProfile.status,
    activeProfileId,
    contextMenuState,
    copy,
    libraryItems,
    selectionInteractionDisabledReason,
    selectionMode,
  ]);

  const handleQueryChange = (nextQuery: string) => {
    setQuery(nextQuery);
    resetPageInteraction("search-changed");
  };

  const handleFilterChange = (nextFilter: ModLibraryFilter) => {
    setActiveFilter(nextFilter);
    libraryQuery.resetPage();
    resetPageInteraction("filters-changed");
  };

  const handlePageChange = (nextPage: number) => {
    libraryQuery.setPage(nextPage);
    resetContentScroll();
  };

  const handlePageSizeChange = (nextPageSize: Parameters<typeof libraryQuery.setPageSize>[0]) => {
    libraryQuery.setPageSize(nextPageSize);
    resetContentScroll();
  };

  const resetSearchAndFilter = () => {
    setQuery("");
    setActiveFilter(allLibraryFilter);
    libraryQuery.resetPage();
    resetPageInteraction("query-reset");
  };

  const retryLibraryQuery = () => {
    void libraryQuery.refresh().catch(() => undefined);
  };

  const selectCard = (intent: ModCardSelectionIntent) => {
    if (selectionInteractionLocked) {
      return;
    }
    dispatchSelection({ type: "apply-intent", intent });
  };

  const handleContextMenu = (modId: string, x: number, y: number) => {
    if (libraryQueryBusy) {
      return;
    }
    setContextMenuState({ x, y, modId });
    if (selectionMode === "single" && !selectedIds.has(modId)) {
      dispatchSelection({
        type: "apply-intent",
        intent: { kind: "primary", modId, source: "pointer" },
      });
    }
  };

  const selectAll = () => {
    if (selectionInteractionLocked) {
      return;
    }
    if (selectionMode === "single") {
      dispatchSelection({ type: "enter-batch" });
    }
    dispatchSelection({ type: "select-page", modIds: libraryItems.map((item) => item.id) });
  };

  const invertSelection = () => {
    if (selectionInteractionLocked) {
      return;
    }
    if (selectionMode === "single") {
      dispatchSelection({ type: "enter-batch" });
    }
    dispatchSelection({ type: "invert-page", modIds: libraryItems.map((item) => item.id) });
  };

  const previewSelectedInstallPlan = () => {
    if (
      libraryQueryBusy
      || selectionMode !== "single"
      || selectedIds.size !== 1
      || reinstallWorkflow.workflowActive
    ) {
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
          message: installPlanPreviewErrorMessage(error, copy.page.planPreview),
        });
      });
  };

  const startSelectedInstallTask = (requestedModId?: string) => {
    if (
      libraryQueryBusy
      || selectionMode !== "single"
      || (requestedModId === undefined && selectedIds.size !== 1)
      || selectionInteractionLocked
      || reinstallWorkflow.workflowActive
    ) {
      return;
    }

    const modId = requestedModId ?? Array.from(selectedIds)[0];
    if (modId === undefined) {
      return;
    }
    const item = libraryItems.find((candidate) => candidate.id === modId);
    const modName = item?.name ?? modId;
    const recoveryPanelState = item ? recoveryPanelStateForItem(item) : null;
    if (activeProfile.status !== "ready" || activeProfileId === null) {
      setInstallPlanDetailState({
        status: "error",
        modName,
        message: copy.page.toasts.profileNotReady,
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
            message: copy.page.toasts.installInvalidType,
          });
          setLifecycleToast({
            id: `install-start-${++startFailureToastSequenceRef.current}`,
            title: copy.page.toasts.installStartFailedTitle,
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
            ? nextManagedInstallTaskStateFromProgress(
                runningState,
                pendingProgressEvent,
                lifecycleCopyRef.current.installTask,
              )
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
          message: installTaskErrorMessage(error, "install", copy.page.lifecycleStart),
        });
        setLifecycleToast({
          id: `install-start-${++startFailureToastSequenceRef.current}`,
          title: copy.page.toasts.installStartFailedTitle,
          message: installTaskErrorMessage(error, "install", copy.page.lifecycleStart),
          tone: "danger",
        });
      });
  };

  const promptSelectedUninstallTask = (requestedModId?: string) => {
    if (
      libraryQueryBusy ||
      selectionMode !== "single" ||
      (requestedModId === undefined && selectedIds.size !== 1) ||
      selectionInteractionLocked ||
      reinstallWorkflow.workflowActive ||
      activeProfileId === null ||
      (requestedModId === undefined && !selectedItem)
    ) {
      return;
    }

    const item = requestedModId === undefined
      ? selectedItem
      : libraryItems.find((candidate) => candidate.id === requestedModId) ?? null;
    if (!item || item.installSummary?.status !== "installed") {
      return;
    }

    installPlanPreviewGenerationRef.current += 1;
    setInstallPlanDetailState({ status: "idle" });
    setUninstallConfirmation({
      profileId: activeProfileId,
      modId: item.id,
      modName: item.name,
      managedFileCount: item.installSummary.managedFileCount,
      backupCount: item.installSummary.backupCount,
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
            message: copy.page.toasts.uninstallInvalidType,
          });
          setLifecycleToast({
            id: `uninstall-start-${++startFailureToastSequenceRef.current}`,
            title: copy.page.toasts.uninstallStartFailedTitle,
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
            ? nextManagedInstallTaskStateFromProgress(
                runningState,
                pendingProgressEvent,
                lifecycleCopyRef.current.installTask,
              )
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
          message: installTaskErrorMessage(error, "uninstall", copy.page.lifecycleStart),
        });
        setLifecycleToast({
          id: `uninstall-start-${++startFailureToastSequenceRef.current}`,
          title: copy.page.toasts.uninstallStartFailedTitle,
          message: installTaskErrorMessage(error, "uninstall", copy.page.lifecycleStart),
          tone: "danger",
        });
      });
  };

  const handleAction = (actionId: string) => {
    switch (actionId) {
      case "enter-batch-selection":
        if (!selectionInteractionLocked) {
          dispatchSelection({ type: "enter-batch" });
        }
        break;
      case "exit-batch-selection":
        if (!selectionInteractionLocked) {
          dispatchSelection({ type: "exit-batch" });
        }
        break;
      case "clear-selection":
        if (!selectionInteractionLocked) {
          dispatchSelection({ type: "clear-selection" });
        }
        break;
      case "select-all":
        selectAll();
        break;
      case "invert":
        invertSelection();
        break;
      case "refresh":
        if (!selectionInteractionLocked) {
          void refreshModLibrary().catch(() => undefined);
        }
        break;
      case "preview-plan":
        if (selectionMode === "single") {
          previewSelectedInstallPlan();
        } else if (!selectionInteractionLocked && batchPreviewAvailable) {
          void batchWorkflow.prepare("install", Array.from(selectedIds));
        }
        break;
      case "install":
        if (selectionMode === "single") {
          startSelectedInstallTask();
        } else if (!selectionInteractionLocked && batchWriteAvailable) {
          void batchWorkflow.prepare("install", Array.from(selectedIds));
        }
        break;
      case "reinstall":
        if (libraryQueryBusy) {
          break;
        }
        setUninstallConfirmation(null);
        installPlanPreviewGenerationRef.current += 1;
        setInstallPlanDetailState({ status: "idle" });
        if (selectionMode === "single") {
          openReinstall();
        } else if (!selectionInteractionLocked && batchWriteAvailable) {
          void batchWorkflow.prepare("reinstall", Array.from(selectedIds));
        }
        break;
      case "uninstall":
        if (selectionMode === "single") {
          promptSelectedUninstallTask();
        } else if (!selectionInteractionLocked && batchWriteAvailable) {
          void batchWorkflow.prepare("uninstall", Array.from(selectedIds));
        }
        break;
      default:
        onAction?.(actionId);
        break;
    }
  };

  const handleContextMenuAction = (actionId: string, modId: string) => {
    if (libraryQueryBusy || selectionInteractionLocked) {
      return;
    }
    switch (actionId) {
      case "install":
        startSelectedInstallTask(modId);
        break;
      case "uninstall":
        promptSelectedUninstallTask(modId);
        break;
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
  const closeBatchResult = () => {
    const completed =
      batchWorkflow.state.status === "result"
      && batchWorkflow.state.result.status === "completed";
    batchWorkflow.reset();
    if (completed) {
      dispatchSelection({ type: "reset-context", reason: "batch-completed" });
    }
  };
  const closeInstallPlanDetail = () => {
    installPlanPreviewGenerationRef.current += 1;
    setInstallPlanDetailState({ status: "idle" });
  };
  const { showScrollUi, thumbStyle } = scrollUiState;

  return (
    <section className="mod-library" aria-label={copy.page.regionLabel}>
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
            selectionMode={selectionMode}
            selectedCount={selectedCount}
            selectedPageCount={selectedPageCount}
            pageCount={libraryItems.length}
            selectionNotice={
              selectionNotice ? renderModSelectionNotice(selectionNotice, copy.selection) : null
            }
            selectionInteractionDisabledReason={selectionInteractionDisabledReason}
            batchPreviewUnavailableReason={batchPreviewUnavailableReason}
            batchWriteUnavailableReason={batchWriteUnavailableReason}
            selectedModId={selectionMode === "single" ? selectedItem?.id ?? null : null}
            installTaskActive={installTaskActive}
            libraryQueryBusy={libraryQueryBusy}
            profileReady={activeProfile.status === "ready" && activeProfileId !== null}
            canInstallSelection={canInstallSelected}
            canReinstallSelection={canReinstallSelected}
            canUninstallSelection={canUninstallSelected}
            onImportCompleted={refreshModLibraryAfterWrite}
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

      {(batchWorkflow.state.status === "resolving"
        || batchWorkflow.state.status === "target-selection"
        || batchWorkflow.state.status === "preview-loading"
        || batchWorkflow.state.status === "preview-ready"
        || batchWorkflow.state.status === "preview-error"
        || batchWorkflow.state.status === "confirming") && (
        <BatchModLifecyclePreviewPanel
          workflowState={batchWorkflow.state}
          resolution={batchWorkflow.resolution}
          policy={
            batchWorkflow.state.status === "resolving"
              ? DEFAULT_BATCH_EXECUTION_POLICY
              : batchWorkflow.state.policy
          }
          onPolicyChange={batchWorkflow.setPolicy}
          onReplacementTargetChange={batchWorkflow.setReplacementTarget}
          onPreviewWithReplacementTargets={batchWorkflow.previewWithReplacementTargets}
          onConfirm={() => void batchWorkflow.confirmAndStart()}
          onClose={batchWorkflow.reset}
        />
      )}

      {batchWorkflow.state.status === "result" && (
        <BatchModLifecycleResultPanel
          workflowState={batchWorkflow.state}
          onRetry={() => void batchWorkflow.retry()}
          onLoadMore={() => void batchWorkflow.loadMoreResult()}
          onClose={closeBatchResult}
        />
      )}

      {(batchWorkflow.state.status === "starting"
        || batchWorkflow.state.status === "result-error") && (
        <BatchModLifecycleRunningPanel
          workflowState={batchWorkflow.state}
          onClose={batchWorkflow.reset}
        />
      )}

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
          onSaved={refreshModLibraryAfterWrite}
        />
      ) : null}

      <div
        className="mod-library__content-shell"
        data-scroll-ui={showScrollUi ? "visible" : "hidden"}
        data-tour-id="mods.library"
      >
        <ModLibraryQueryFeedback
          busy={!libraryQueryBlocked && libraryQuery.refreshing}
          errorMessage={libraryPage === null ? null : libraryQueryErrorMessage}
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
              message={libraryQueryErrorMessage ?? copy.page.loadFailedFallback}
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
                  selectionMode={selectionMode}
                  interactionDisabled={selectionInteractionLocked}
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
          lifecycleAction={contextMenuLifecycleAction}
          onClose={() => setContextMenuState(null)}
          onAction={handleContextMenuAction}
        />
      )}
    </section>
  );
}
