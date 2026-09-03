import { listen } from "@tauri-apps/api/event";
import {
  AlertTriangle,
  CheckCircle2,
  Copy,
  Eye,
  LoaderCircle,
  RefreshCw,
  RotateCcw,
  Search,
  ShieldAlert,
  Target,
  XCircle,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useFeedback } from "../../shared/feedback";
import { resolveCopy, useI18n } from "../../shared/i18n";
import {
  matchedHiddenReplacementTargetNames,
  replacementTargetSearchHit,
} from "./replacementTargetMatch";
import { replacementTargetSearchValues, resolveReplacementTargetNames } from "./replacementTargetNames";
import type { GameId } from "../game-setup/gameSetupTypes";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "../mods/modImportTypes";
import type { InstallManifestStatus } from "../mods/modInstallPlanTypes";
import {
  getPrerequisiteDecisionCodeLabel,
  getPrerequisiteDecisionMessage,
} from "../mods/modPrerequisiteDecision";
import { modLifecycleCopy } from "../mods/modLifecycleCopy";
import { modReinstallCopy, type ModReinstallCopy } from "../mods/modReinstallCopy";
import { getReinstallBlockingReasonLabel } from "../mods/modReinstallTaskState";
import type { ReinstallPlanPreview } from "../mods/modReinstallTypes";
import {
  analyzeImportedModReplacement,
  cancelRetargetInstallTask,
  listReplacementTargetOccupancy,
  listReplacementTargets,
  previewInitialRetargetInstall,
  previewRetargetReinstall,
  startRetargetInstallTask,
  startRetargetReinstallTask,
} from "./replacementApi";
import { replacementCopy, type ReplacementCopy } from "./replacementCopy";
import { replacementErrorMessage } from "./replacementErrorText";
import type {
  InitialRetargetInstallPreview,
  OccupiedReplacementTarget,
  ReplacementAnalysis,
  ReplacementTarget,
} from "./replacementTypes";
import {
  canCancelRetargetInstallTaskPhase,
  canStartInitialRetargetInstall,
  canStartRetargetReinstall,
  isCurrentInstalledReplacementTarget,
  isRetargetInstallTaskPhase,
  nextRetargetInstallTaskState,
  refreshRetargetInstallState,
  resolveInstalledReplacementTargetSelection,
  retargetInstallTaskPhaseLabel,
  type RetargetInstallRefreshState,
  type RetargetInstallTaskState,
} from "./replacementWorkflow";
import "./ReplacementTargetPanel.css";

type ReplacementTargetPanelProps = {
  gameId: GameId;
  modId: string;
  profileId: string | null;
  installStatus: InstallManifestStatus | undefined;
  completedLocally: boolean;
  onBusyChange: (busy: boolean) => void;
  onInstallCompleted: () => Promise<void> | void;
};

type LoadState =
  | { status: "loading" }
  | {
      status: "ready";
      analysis: ReplacementAnalysis;
      targets: ReplacementTarget[];
      occupancy: OccupiedReplacementTarget[];
    }
  | { status: "error"; message: string };

type PreviewState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; mode: "initial"; preview: InitialRetargetInstallPreview }
  | { status: "ready"; mode: "switch"; preview: ReinstallPlanPreview }
  | { status: "error"; message: string };

type TaskStateUpdate =
  | RetargetInstallTaskState
  | ((current: RetargetInstallTaskState) => RetargetInstallTaskState);

type CancellationState =
  | { status: "idle" }
  | { status: "requesting"; taskId: string }
  | { status: "error"; taskId: string; message: string };

function installBlockMessage(
  profileId: string | null,
  installStatus: InstallManifestStatus | undefined,
  completedLocally: boolean,
  block: ReplacementCopy["block"],
) {
  if (profileId === null) {
    return block.profileUnavailable;
  }
  if (completedLocally) {
    return block.completedRefreshing;
  }
  switch (installStatus) {
    case "not_installed":
    case "installed":
      return null;
    case "committed_cleanup_pending":
    case "cleanup_pending":
      return block.cleanupPending;
    case "rollback_required":
      return block.rollbackRequired;
    case "repair_required":
      return block.repairRequired;
    case "unknown":
    case undefined:
      return block.statusUnknown;
  }
}

function targetSwitchBlockingLabel(
  code: ReinstallPlanPreview["blockingReasons"][number]["code"],
  panel: ReplacementCopy["panel"],
  reinstallTask: ModReinstallCopy["task"],
) {
  return code === "candidate_already_installed"
    ? panel.candidateAlreadyInstalled
    : getReinstallBlockingReasonLabel(code, reinstallTask);
}

/**
 * 占用查询只服务于提示，失败一律 fail-open 返回空列表。
 *
 * 硬门禁在预览、任务期计划构建和 commit 三层，不依赖这份数据；查询失败
 * 只是少一条提示，不该让整个替换目标面板打不开。
 */
function loadOccupancy(
  gameId: GameId,
  profileId: string | null,
  modId: string,
): Promise<OccupiedReplacementTarget[]> {
  if (profileId === null) {
    return Promise.resolve([]);
  }
  return listReplacementTargetOccupancy({ gameId, profileId, modId }).catch(() => []);
}

export function ReplacementTargetPanel({
  gameId,
  modId,
  profileId,
  installStatus,
  completedLocally,
  onBusyChange,
  onInstallCompleted,
}: ReplacementTargetPanelProps) {
  const { locale } = useI18n();
  const { pushToast } = useFeedback();
  const rCopy = resolveCopy(replacementCopy, locale);
  const reinstallTask = resolveCopy(modReinstallCopy, locale).task;
  const prerequisite = resolveCopy(modLifecycleCopy, locale).prerequisite;
  // 事件监听回调经 ref 取词，避免语言切换导致监听器重建。
  const rCopyRef = useRef(rCopy);
  rCopyRef.current = rCopy;
  const [retryToken, setRetryToken] = useState(0);
  const [loadState, setLoadState] = useState<LoadState>({ status: "loading" });
  const [query, setQuery] = useState("");
  const [selectedTargetId, setSelectedTargetId] = useState<string | null>(null);
  const [previewState, setPreviewState] = useState<PreviewState>({ status: "idle" });
  const [taskState, setTaskState] = useState<RetargetInstallTaskState>({ status: "idle" });
  const taskStateRef = useRef<RetargetInstallTaskState>(taskState);
  const previewRequestGenerationRef = useRef(0);
  const pendingEventsRef = useRef(new Map<string, TaskProgressEventDto>());
  const completedTaskRef = useRef<string | null>(null);
  const refreshGenerationRef = useRef(0);
  const completionReloadPendingRef = useRef(false);
  const [refreshState, setRefreshState] = useState<RetargetInstallRefreshState>({ status: "idle" });
  const [listenerAttempt, setListenerAttempt] = useState(0);
  const [listenerStatus, setListenerStatus] = useState<"connecting" | "ready" | "failed">(
    "connecting",
  );
  const [cancellationState, setCancellationState] = useState<CancellationState>({ status: "idle" });

  const setTrackedTaskState = useCallback((update: TaskStateUpdate) => {
    const next = typeof update === "function" ? update(taskStateRef.current) : update;
    taskStateRef.current = next;
    setTaskState(next);
  }, []);

  const refreshCompletedInstall = useCallback(() => {
    const generation = ++refreshGenerationRef.current;
    setRefreshState({ status: "refreshing" });
    void refreshRetargetInstallState(onInstallCompleted, rCopyRef.current.events).then((next) => {
      if (refreshGenerationRef.current === generation) {
        if (next.status === "ready") {
          completionReloadPendingRef.current = true;
          setRetryToken((value) => value + 1);
        } else {
          setRefreshState(next);
        }
      }
    });
  }, [onInstallCompleted]);

  useEffect(() => {
    refreshGenerationRef.current += 1;
    completedTaskRef.current = null;
    completionReloadPendingRef.current = false;
    setRefreshState({ status: "idle" });
  }, [gameId, modId, profileId]);

  useEffect(() => {
    let cancelled = false;
    previewRequestGenerationRef.current += 1;
    setLoadState({ status: "loading" });
    setSelectedTargetId(null);
    setPreviewState({ status: "idle" });

    void Promise.all([
      analyzeImportedModReplacement({ gameId, profileId, modId }),
      listReplacementTargets({ gameId, modId }),
      loadOccupancy(gameId, profileId, modId),
    ])
      .then(([analysis, targets, occupancy]) => {
        if (!cancelled) {
          setSelectedTargetId(
            resolveInstalledReplacementTargetSelection(targets, analysis.installedTargetId),
          );
          setLoadState({ status: "ready", analysis, targets, occupancy });
          if (completionReloadPendingRef.current) {
            completionReloadPendingRef.current = false;
            setRefreshState({ status: "ready" });
            setTrackedTaskState({ status: "idle" });
          }
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setLoadState({
            status: "error",
            message: replacementErrorMessage(
              error,
              rCopyRef.current.events.analysisFallback,
              rCopyRef.current.errors,
            ),
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [gameId, modId, profileId, retryToken, setTrackedTaskState]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    setListenerStatus("connecting");

    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, (event) => {
      if (disposed || event.payload.kind !== "install" || !isRetargetInstallTaskPhase(event.payload.phase)) {
        return;
      }
      const current = taskStateRef.current;
      if (!("taskId" in current) || current.taskId === null) {
        if (current.status === "starting") {
          pendingEventsRef.current.set(event.payload.taskId, event.payload);
        }
        return;
      }
      if (event.payload.taskId !== current.taskId) {
        return;
      }
      setTrackedTaskState((state) =>
        nextRetargetInstallTaskState(state, event.payload, rCopyRef.current.events),
      );
    })
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        unlisten = dispose;
        setListenerStatus("ready");
      })
      .catch(() => {
        if (!disposed) {
          setListenerStatus("failed");
        }
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [listenerAttempt, setTrackedTaskState]);

  const taskActive = taskState.status === "starting" || taskState.status === "running";
  const refreshPending =
    taskState.status === "completed" && completedTaskRef.current !== taskState.taskId;
  const panelBusy = taskActive || refreshPending || refreshState.status === "refreshing";
  useEffect(() => onBusyChange(panelBusy), [onBusyChange, panelBusy]);
  useEffect(() => () => onBusyChange(false), [onBusyChange]);

  useEffect(() => {
    if (taskState.status !== "completed" || completedTaskRef.current === taskState.taskId) {
      return;
    }
    completedTaskRef.current = taskState.taskId;
    refreshCompletedInstall();
  }, [refreshCompletedInstall, taskState]);

  useEffect(() => {
    if (taskState.status !== "running") {
      setCancellationState({ status: "idle" });
    }
  }, [taskState.status]);

  const targets = useMemo(
    () => (loadState.status === "ready" ? loadState.targets : []),
    [loadState],
  );
  const analysis = loadState.status === "ready" ? loadState.analysis : null;
  const installedTargetId = analysis?.installedTargetId;
  const filteredTargets = useMemo(() => {
    const keyword = query.trim().toLocaleLowerCase();
    if (!keyword) {
      return targets;
    }
    return targets.filter((target) =>
      [...replacementTargetSearchValues(target.displayNames), target.internalId, ...target.aliases]
        .filter((value): value is string => Boolean(value))
        .some((value) => replacementTargetSearchHit(value, keyword)),
    );
  }, [query, targets]);
  const selectedTarget = targets.find((target) => target.id === selectedTargetId) ?? null;
  const occupancyByTarget = useMemo(
    () =>
      new Map(
        (loadState.status === "ready" ? loadState.occupancy : []).map((item) => [
          item.targetId,
          item,
        ]),
      ),
    [loadState],
  );
  const selectedOccupancy =
    selectedTargetId === null ? null : (occupancyByTarget.get(selectedTargetId) ?? null);
  const installCompletedLocally = completedLocally || taskState.status === "completed";
  const blockMessage = installBlockMessage(
    profileId,
    installStatus,
    installCompletedLocally,
    rCopy.block,
  );
  const targetSwitch = installStatus === "installed";

  const selectTarget = (targetId: string) => {
    if (
      taskActive ||
      installCompletedLocally ||
      isCurrentInstalledReplacementTarget(targetId, installedTargetId)
    ) {
      return;
    }
    previewRequestGenerationRef.current += 1;
    setSelectedTargetId(targetId);
    setPreviewState({ status: "idle" });
    setTrackedTaskState({ status: "idle" });
  };

  // 占用方名称要能被玩家复制去 Mod 库里搜索，所以复制失败也要有反馈。
  const copyOccupantName = (occupancy: OccupiedReplacementTarget) => {
    void navigator.clipboard
      .writeText(occupancy.displayName)
      .then(() =>
        pushToast({
          eventKey: `replacement.occupancy.copied.${occupancy.modId}`,
          title: rCopy.panel.occupantNameCopiedTitle,
          message: occupancy.displayName,
          tone: "success",
        }),
      )
      .catch(() =>
        pushToast({
          eventKey: "replacement.occupancy.copy.failed",
          title: rCopy.panel.occupantNameCopyFailedTitle,
          message: rCopy.panel.occupantNameCopyFailedMessage,
          tone: "danger",
        }),
      );
  };

  const createPreview = () => {
    if (
      !selectedTarget ||
      profileId === null ||
      blockMessage !== null ||
      selectedOccupancy !== null ||
      isCurrentInstalledReplacementTarget(selectedTarget.id, installedTargetId)
    ) {
      return;
    }
    const requestGeneration = ++previewRequestGenerationRef.current;
    setPreviewState({ status: "loading" });
    const request = {
      gameId,
      profileId,
      modId,
      targetId: selectedTarget.id,
      layerName: "base",
      layerPriority: 0,
    };
    const preview = targetSwitch
      ? previewRetargetReinstall(request).then(
          (result) => ({ mode: "switch", preview: result }) as const,
        )
      : previewInitialRetargetInstall(request).then(
          (result) => ({ mode: "initial", preview: result }) as const,
        );
    void preview
      .then((result) => {
        if (previewRequestGenerationRef.current === requestGeneration) {
          setPreviewState({ status: "ready", ...result });
        }
      })
      .catch((error: unknown) => {
        if (previewRequestGenerationRef.current === requestGeneration) {
          setPreviewState({
            status: "error",
            message: replacementErrorMessage(error, rCopy.events.previewFallback, rCopy.errors),
          });
        }
      });
  };

  const startInstall = () => {
    const switchPreviewStatus =
      previewState.status === "ready" && previewState.mode === "switch"
        ? previewState.preview.status
        : undefined;
    const canStart =
      previewState.status === "ready" && previewState.mode === "switch"
        ? canStartRetargetReinstall({
            installStatus,
            previewStatus: switchPreviewStatus,
            taskActive,
            listenerReady: listenerStatus === "ready",
          })
        : previewState.status === "ready" && previewState.mode === "initial"
          ? canStartInitialRetargetInstall({
              installStatus,
              completedLocally: installCompletedLocally,
              hasPreview: true,
              hasBlockingConflicts: previewState.preview.installPlan.hasBlockingConflicts,
              prerequisiteStatus: previewState.preview.prerequisiteDecision.status,
              taskActive,
              listenerReady: listenerStatus === "ready",
            })
          : false;
    if (
      profileId === null ||
      selectedTarget === null ||
      previewState.status !== "ready" ||
      blockMessage !== null ||
      selectedOccupancy !== null ||
      !canStart
    ) {
      return;
    }

    pendingEventsRef.current.clear();
    setTrackedTaskState({ status: "starting" });
    const request = {
      gameId,
      profileId,
      modId,
      targetId: selectedTarget.id,
      layerName: "base",
      layerPriority: 0,
    };
    const start =
      previewState.mode === "switch" && previewState.preview.status === "ready"
        ? startRetargetReinstallTask({
            ...request,
            planToken: previewState.preview.planToken,
          })
        : startRetargetInstallTask(request);
    const queuedPhase =
      previewState.mode === "switch" ? "install.reinstall.queued" : "install.retarget.queued";
    const failedPhase =
      previewState.mode === "switch" ? "install.reinstall.failed" : "install.retarget.failed";
    void start
      .then((task) => {
        if (task.kind !== "install" || task.status !== "queued") {
          setTrackedTaskState({
            status: "failed",
            taskId: null,
            phase: failedPhase,
            message: rCopy.events.invalidTaskType,
          });
          return;
        }
        const running: RetargetInstallTaskState = {
          status: "running",
          taskId: task.taskId,
          phase: queuedPhase,
        };
        const pending = pendingEventsRef.current.get(task.taskId);
        pendingEventsRef.current.clear();
        setTrackedTaskState(
          pending ? nextRetargetInstallTaskState(running, pending, rCopy.events) : running,
        );
      })
      .catch((error: unknown) => {
        pendingEventsRef.current.clear();
        setTrackedTaskState({
          status: "failed",
          taskId: null,
          phase: failedPhase,
          message: replacementErrorMessage(error, rCopy.events.startFailed, rCopy.errors),
        });
      });
  };

  const cancelCurrentTask = () => {
    const current = taskStateRef.current;
    if (current.status !== "running" || cancellationState.status === "requesting") {
      return;
    }

    const taskId = current.taskId;
    const cancelledPhase = current.phase.startsWith("install.reinstall.")
      ? "install.reinstall.cancelled"
      : "install.cancelled";
    setCancellationState({ status: "requesting", taskId });
    void cancelRetargetInstallTask({ taskId })
      .then((task) => {
        if (task.taskId !== taskId || task.kind !== "install" || task.status !== "cancelled") {
          setCancellationState({
            status: "error",
            taskId,
            message: rCopy.events.invalidCancelResult,
          });
          return;
        }
        setTrackedTaskState((state) =>
          state.status === "running" && state.taskId === taskId
            ? { status: "cancelled", taskId, phase: cancelledPhase }
            : state,
        );
        setCancellationState({ status: "idle" });
      })
      .catch((error: unknown) => {
        if (taskStateRef.current.status === "running" && taskStateRef.current.taskId === taskId) {
          setCancellationState({
            status: "error",
            taskId,
            message: replacementErrorMessage(error, rCopy.events.cancelFailed, rCopy.errors),
          });
        }
      });
  };

  if (loadState.status === "loading") {
    return (
      <div className="replacement-panel__state" role="status">
        <LoaderCircle className="replacement-panel__spinner" size={20} aria-hidden="true" />
        <span>{rCopy.panel.analyzing}</span>
      </div>
    );
  }

  if (loadState.status === "error") {
    return (
      <div className="replacement-panel__state is-error" role="alert">
        <ShieldAlert size={20} aria-hidden="true" />
        <span>{loadState.message}</span>
        <button type="button" onClick={() => setRetryToken((value) => value + 1)}>
          <RefreshCw size={15} aria-hidden="true" />
          {rCopy.panel.retry}
        </button>
      </div>
    );
  }

  return (
    <div className="replacement-panel">
      {blockMessage ? (
        <div className="replacement-panel__notice is-blocked" role="status">
          <ShieldAlert size={18} aria-hidden="true" />
          <span>{blockMessage}</span>
        </div>
      ) : null}

      <section className="replacement-panel__source" aria-labelledby="replacement-source-title">
        <div className="replacement-panel__section-heading">
          <Target size={17} aria-hidden="true" />
          <h3 id="replacement-source-title">{rCopy.panel.detectionTitle}</h3>
          <span>{rCopy.panel.resourceCount(analysis?.matchedAssetCount ?? 0)}</span>
        </div>
        {analysis?.sources.length ? (
          <dl className="replacement-panel__source-facts">
            {analysis.sources.map((source) => (
              <div key={source.id}>
                <dt>{source.sourceType}</dt>
                <dd>{source.internalId}</dd>
              </div>
            ))}
          </dl>
        ) : (
          <p className="replacement-panel__empty">{rCopy.panel.noSources}</p>
        )}
        {analysis?.warnings.length ? (
          <ul className="replacement-panel__warnings" aria-label={rCopy.panel.warningsAria}>
            {analysis.warnings.map((warning) => (
              <li key={warning}>
                <AlertTriangle size={14} aria-hidden="true" />
                {rCopy.warnings[warning]}
              </li>
            ))}
          </ul>
        ) : null}
      </section>

      <section className="replacement-panel__catalog" aria-labelledby="replacement-catalog-title">
        <div className="replacement-panel__section-heading">
          <h3 id="replacement-catalog-title">{rCopy.panel.targetsTitle}</h3>
          <span>{rCopy.panel.targetCount(filteredTargets.length)}</span>
        </div>
        <label className="replacement-panel__search">
          <Search size={16} aria-hidden="true" />
          <input
            type="search"
            aria-label={rCopy.panel.searchAria}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={rCopy.panel.searchPlaceholder}
            disabled={taskActive}
          />
        </label>
        {filteredTargets.length ? (
          <div className="replacement-panel__target-list" role="radiogroup" aria-label={rCopy.panel.targetsAria}>
            {filteredTargets.map((target) => {
              const currentInstalled = isCurrentInstalledReplacementTarget(
                target.id,
                installedTargetId,
              );
              const occupied = occupancyByTarget.get(target.id) ?? null;
              const names = resolveReplacementTargetNames(target.displayNames, locale);
              // #274: the filter also matches aliases and other locales' display names,
              // none of which the row renders; say what was hit so the row does not look wrong.
              const matchHint = matchedHiddenReplacementTargetNames(target, names, query);
              return (
                <label
                  className="replacement-panel__target-row"
                  data-installed={currentInstalled}
                  data-occupied={occupied ? "true" : "false"}
                  data-selected={target.id === selectedTargetId}
                  key={target.id}
                >
                  <input
                    type="radio"
                    name="replacement-target"
                    value={target.id}
                    checked={target.id === selectedTargetId}
                    onChange={() => selectTarget(target.id)}
                    disabled={
                      !analysis?.retargetable ||
                      previewState.status === "loading" ||
                      taskActive ||
                      installCompletedLocally ||
                      currentInstalled
                    }
                  />
                  <span className="replacement-panel__target-name">
                    <strong>{names.displayName}</strong>
                    {names.secondaryName ? <small>{names.secondaryName}</small> : null}
                    {matchHint ? (
                      <small className="replacement-panel__target-match">
                        <Search size={11} aria-hidden="true" />
                        <span>{rCopy.panel.matchedNames(matchHint.names)}</span>
                        {matchHint.hiddenCount > 0 ? (
                          <em>{rCopy.panel.matchedNamesMore(matchHint.hiddenCount)}</em>
                        ) : null}
                      </small>
                    ) : null}
                  </span>
                  <span className="replacement-panel__target-facts">
                    {currentInstalled ? (
                      <span className="replacement-panel__target-status is-installed">
                        <CheckCircle2 size={13} aria-hidden="true" />
                        {rCopy.panel.currentInstalled}
                      </span>
                    ) : null}
                    {occupied ? (
                      <span className="replacement-panel__target-status is-occupied">
                        <ShieldAlert size={13} aria-hidden="true" />
                        {rCopy.panel.targetOccupiedTag}
                      </span>
                    ) : null}
                    <code>{target.internalId}</code>
                  </span>
                </label>
              );
            })}
          </div>
        ) : (
          <p className="replacement-panel__empty">{rCopy.panel.noMatches}</p>
        )}
      </section>

      {previewState.status !== "idle" ? (
        <section className="replacement-panel__preview" aria-live="polite">
          {previewState.status === "loading" ? (
            <div className="replacement-panel__inline-state">
              <LoaderCircle className="replacement-panel__spinner" size={17} aria-hidden="true" />
              {rCopy.panel.previewLoading}
            </div>
          ) : null}
          {previewState.status === "error" ? (
            <div className="replacement-panel__inline-state is-error">
              <AlertTriangle size={17} aria-hidden="true" />
              {previewState.message}
            </div>
          ) : null}
          {previewState.status === "ready" ? (
            <>
              <div className="replacement-panel__section-heading">
                <Eye size={17} aria-hidden="true" />
                <h3>
                  {previewState.mode === "switch"
                    ? rCopy.panel.switchPreviewTitle
                    : rCopy.panel.initialPreviewTitle}
                </h3>
                {previewState.mode === "initial" ? (
                  <span>{rCopy.panel.actionCount(previewState.preview.actions.length)}</span>
                ) : null}
              </div>
              {previewState.mode === "initial" ? (
                <>
                  <dl className="replacement-panel__preview-facts">
                    <div>
                      <dt>{rCopy.panel.factResourceType}</dt>
                      <dd>{previewState.preview.target.targetType}</dd>
                    </div>
                    <div>
                      <dt>{rCopy.panel.factTargetId}</dt>
                      <dd>{previewState.preview.target.internalId}</dd>
                    </div>
                    <div>
                      <dt>{rCopy.panel.factActions}</dt>
                      <dd>{previewState.preview.actions.length}</dd>
                    </div>
                  </dl>
                  {previewState.preview.installPlan.hasBlockingConflicts ? (
                    <>
                      <div className="replacement-panel__inline-state is-error">
                        <ShieldAlert size={17} aria-hidden="true" />
                        {rCopy.panel.blockingConflicts(previewState.preview.installPlan.conflicts.length)}
                      </div>
                      <div className="replacement-panel__inline-state is-warning" role="status">
                        <AlertTriangle size={17} aria-hidden="true" />
                        {rCopy.panel.blockingConflictHint}
                      </div>
                    </>
                  ) : (
                    <div className="replacement-panel__inline-state is-success">
                      <CheckCircle2 size={17} aria-hidden="true" />
                      {rCopy.panel.noBlockingConflicts}
                    </div>
                  )}
                  <div
                    className={[
                      "replacement-panel__inline-state",
                      previewState.preview.prerequisiteDecision.status === "blocked"
                        ? "is-error"
                        : previewState.preview.prerequisiteDecision.status === "warning"
                          ? "is-warning"
                          : "is-success",
                    ].join(" ")}
                    role={
                      previewState.preview.prerequisiteDecision.status === "ready"
                        ? "status"
                        : "alert"
                    }
                  >
                    {previewState.preview.prerequisiteDecision.status === "ready" ? (
                      <CheckCircle2 size={17} aria-hidden="true" />
                    ) : (
                      <ShieldAlert size={17} aria-hidden="true" />
                    )}
                    {getPrerequisiteDecisionMessage(
                      previewState.preview.prerequisiteDecision,
                      prerequisite,
                    )}
                  </div>
                  {previewState.preview.prerequisiteDecision.codes.length > 0 ? (
                    <ul
                      className="replacement-panel__blocking-list"
                      aria-label={rCopy.panel.prerequisiteResultsAria}
                    >
                      {previewState.preview.prerequisiteDecision.codes.map((code) => (
                        <li key={code}>
                          <AlertTriangle size={15} aria-hidden="true" />
                          <span>{getPrerequisiteDecisionCodeLabel(code, prerequisite)}</span>
                        </li>
                      ))}
                    </ul>
                  ) : null}
                </>
              ) : (
                <>
                  <dl className="replacement-panel__counts">
                    <div data-kind="retained">
                      <dt>{rCopy.panel.countRetained}</dt>
                      <dd>{previewState.preview.counts.retained}</dd>
                    </div>
                    <div data-kind="replaced">
                      <dt>{rCopy.panel.countReplaced}</dt>
                      <dd>{previewState.preview.counts.replaced}</dd>
                    </div>
                    <div data-kind="added">
                      <dt>{rCopy.panel.countAdded}</dt>
                      <dd>{previewState.preview.counts.added}</dd>
                    </div>
                    <div data-kind="stale">
                      <dt>{rCopy.panel.countStale}</dt>
                      <dd>{previewState.preview.counts.stale}</dd>
                    </div>
                  </dl>
                  {previewState.preview.status === "ready" ? (
                    <div className="replacement-panel__inline-state is-success">
                      <CheckCircle2 size={17} aria-hidden="true" />
                      {rCopy.panel.preflightPassed}
                    </div>
                  ) : (
                    <ul className="replacement-panel__blocking-list" aria-label={rCopy.panel.switchBlockedAria}>
                      {previewState.preview.blockingReasons.map((reason) => (
                        <li key={reason.code}>
                          <ShieldAlert size={15} aria-hidden="true" />
                          <span>{targetSwitchBlockingLabel(reason.code, rCopy.panel, reinstallTask)}</span>
                          <strong>{reason.count}</strong>
                        </li>
                      ))}
                    </ul>
                  )}
                </>
              )}
            </>
          ) : null}
        </section>
      ) : null}

      {listenerStatus === "failed" ? (
        <div className="replacement-panel__notice is-blocked" role="alert">
          <AlertTriangle size={17} aria-hidden="true" />
          <span>{rCopy.panel.listenerUnavailable}</span>
          <button type="button" onClick={() => setListenerAttempt((value) => value + 1)}>
            {rCopy.panel.retryListener}
          </button>
        </div>
      ) : null}

      {taskState.status !== "idle" ? (
        <div
          className={`replacement-panel__task-state is-${taskState.status}`}
          role={taskState.status === "failed" ? "alert" : "status"}
        >
          {taskState.status === "starting" ? (
            <>
              <LoaderCircle className="replacement-panel__spinner" size={17} aria-hidden="true" />
              {rCopy.panel.startingInstall}
            </>
          ) : null}
          {taskState.status === "running" ? (
            <>
              <LoaderCircle className="replacement-panel__spinner" size={17} aria-hidden="true" />
              <span>{retargetInstallTaskPhaseLabel(taskState.phase, rCopy.phases)}</span>
              {canCancelRetargetInstallTaskPhase(taskState.phase) ? (
                <button
                  type="button"
                  onClick={cancelCurrentTask}
                  disabled={cancellationState.status === "requesting"}
                >
                  {cancellationState.status === "requesting" ? (
                    <LoaderCircle
                      className="replacement-panel__spinner"
                      size={15}
                      aria-hidden="true"
                    />
                  ) : (
                    <XCircle size={15} aria-hidden="true" />
                  )}
                  {cancellationState.status === "requesting"
                    ? rCopy.panel.cancelling
                    : rCopy.panel.cancelTask}
                </button>
              ) : null}
            </>
          ) : null}
          {taskState.status === "completed" ? (
            <>
              <CheckCircle2 size={17} aria-hidden="true" />
              {retargetInstallTaskPhaseLabel(taskState.phase, rCopy.phases)}
            </>
          ) : null}
          {taskState.status === "failed" ? (
            <>
              <AlertTriangle size={17} aria-hidden="true" />
              {taskState.message}
            </>
          ) : null}
          {taskState.status === "cancelled" ? (
            <>
              <AlertTriangle size={17} aria-hidden="true" />
              {retargetInstallTaskPhaseLabel(taskState.phase, rCopy.phases)}
            </>
          ) : null}
        </div>
      ) : null}

      {cancellationState.status === "error" ? (
        <div className="replacement-panel__notice is-blocked" role="alert">
          <AlertTriangle size={17} aria-hidden="true" />
          <span>{cancellationState.message}</span>
        </div>
      ) : null}

      {refreshState.status === "refreshing" ? (
        <div className="replacement-panel__notice" role="status">
          <LoaderCircle className="replacement-panel__spinner" size={17} aria-hidden="true" />
          <span>{rCopy.panel.refreshing}</span>
        </div>
      ) : null}

      {refreshState.status === "failed" ? (
        <div className="replacement-panel__notice is-blocked" role="alert">
          <AlertTriangle size={17} aria-hidden="true" />
          <span>{refreshState.message}</span>
          <button type="button" onClick={refreshCompletedInstall}>
            <RefreshCw size={15} aria-hidden="true" />
            {rCopy.panel.retryRefresh}
          </button>
        </div>
      ) : null}

      {selectedOccupancy ? (
        <div className="replacement-panel__notice is-blocked" role="status">
          <ShieldAlert size={18} aria-hidden="true" />
          <span>{rCopy.panel.targetOccupied(selectedOccupancy.displayName)}</span>
          <button
            type="button"
            className="replacement-panel__copy-name"
            aria-label={rCopy.panel.copyOccupantName}
            onClick={() => copyOccupantName(selectedOccupancy)}
          >
            <Copy size={14} aria-hidden="true" />
            {rCopy.panel.copyOccupantName}
          </button>
        </div>
      ) : null}

      <div className="replacement-panel__actions">
        <button
          type="button"
          className="is-secondary"
          onClick={createPreview}
          disabled={
            selectedTarget === null ||
            isCurrentInstalledReplacementTarget(selectedTarget.id, installedTargetId) ||
            selectedOccupancy !== null ||
            !analysis?.retargetable ||
            blockMessage !== null ||
            previewState.status === "loading" ||
            taskActive
          }
        >
          <Eye size={16} aria-hidden="true" />
          {targetSwitch ? rCopy.panel.previewSwitch : rCopy.panel.generatePreview}
        </button>
        <button
          type="button"
          className="is-primary"
          onClick={startInstall}
          disabled={
            previewState.status !== "ready" ||
            blockMessage !== null ||
            selectedOccupancy !== null ||
            (previewState.mode === "switch"
              ? !canStartRetargetReinstall({
                  installStatus,
                  previewStatus: previewState.preview.status,
                  taskActive,
                  listenerReady: listenerStatus === "ready",
                })
              : !canStartInitialRetargetInstall({
                  installStatus,
                  completedLocally: installCompletedLocally,
                  hasPreview: true,
                  hasBlockingConflicts: previewState.preview.installPlan.hasBlockingConflicts,
                  prerequisiteStatus: previewState.preview.prerequisiteDecision.status,
                  taskActive,
                  listenerReady: listenerStatus === "ready",
                }))
          }
        >
          {targetSwitch ? (
            <RotateCcw size={16} aria-hidden="true" />
          ) : (
            <Target size={16} aria-hidden="true" />
          )}
          {targetSwitch ? rCopy.panel.confirmSwitch : rCopy.panel.installToTarget}
        </button>
      </div>
    </div>
  );
}
