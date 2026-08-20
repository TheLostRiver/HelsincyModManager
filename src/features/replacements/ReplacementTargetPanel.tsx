import { listen } from "@tauri-apps/api/event";
import {
  AlertTriangle,
  CheckCircle2,
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
import type { GameId } from "../game-setup/gameSetupTypes";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "../mods/modImportTypes";
import type { InstallManifestStatus } from "../mods/modInstallPlanTypes";
import {
  getPrerequisiteDecisionCodeLabel,
  getPrerequisiteDecisionMessage,
} from "../mods/modPrerequisiteDecision";
import { getReinstallBlockingReasonLabel } from "../mods/modReinstallTaskState";
import type { ReinstallPlanPreview } from "../mods/modReinstallTypes";
import {
  analyzeImportedModReplacement,
  cancelRetargetInstallTask,
  listReplacementTargets,
  previewInitialRetargetInstall,
  previewRetargetReinstall,
  startRetargetInstallTask,
  startRetargetReinstallTask,
} from "./replacementApi";
import { replacementErrorMessage } from "./replacementErrorText";
import type {
  InitialRetargetInstallPreview,
  ReplacementAnalysis,
  ReplacementTarget,
  ReplacementWarning,
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
  | { status: "ready"; analysis: ReplacementAnalysis; targets: ReplacementTarget[] }
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

const warningLabels: Record<ReplacementWarning, string> = {
  no_supported_assets: "未检测到受支持的外观资源",
  multiple_sources: "检测到多个源槽位，当前版本不会自动拆分",
  unsupported_source: "包内包含当前版本不支持的源槽位",
  source_matches_target: "源槽位与目标槽位相同",
  weapon_partial_part_set: "武器包只包含部分可选部件，将仅处理已检测到的完整文件对",
};

function installBlockMessage(
  profileId: string | null,
  installStatus: InstallManifestStatus | undefined,
  completedLocally: boolean,
) {
  if (profileId === null) {
    return "当前 Profile 不可用。";
  }
  if (completedLocally) {
    return "写入已完成，正在刷新安装状态。";
  }
  switch (installStatus) {
    case "not_installed":
    case "installed":
      return null;
    case "committed_cleanup_pending":
    case "cleanup_pending":
      return "当前 Profile 有待收尾的重装事务。";
    case "rollback_required":
      return "当前 Profile 需要先完成安装回滚。";
    case "repair_required":
      return "当前 Profile 需要先完成人工修复。";
    case "unknown":
    case undefined:
      return "安装状态未知，替换目标写入已阻止。";
  }
}

function targetSwitchBlockingLabel(code: ReinstallPlanPreview["blockingReasons"][number]["code"]) {
  return code === "candidate_already_installed"
    ? "当前目标已安装"
    : getReinstallBlockingReasonLabel(code);
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
    void refreshRetargetInstallState(onInstallCompleted).then((next) => {
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
    ])
      .then(([analysis, targets]) => {
        if (!cancelled) {
          setSelectedTargetId(
            resolveInstalledReplacementTargetSelection(targets, analysis.installedTargetId),
          );
          setLoadState({ status: "ready", analysis, targets });
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
            message: replacementErrorMessage(error, "替换目标信息读取失败"),
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
      setTrackedTaskState((state) => nextRetargetInstallTaskState(state, event.payload));
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
      [target.displayName, target.secondaryName, target.internalId, ...target.aliases]
        .filter((value): value is string => Boolean(value))
        .some((value) => value.toLocaleLowerCase().includes(keyword)),
    );
  }, [query, targets]);
  const selectedTarget = targets.find((target) => target.id === selectedTargetId) ?? null;
  const installCompletedLocally = completedLocally || taskState.status === "completed";
  const blockMessage = installBlockMessage(profileId, installStatus, installCompletedLocally);
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

  const createPreview = () => {
    if (
      !selectedTarget ||
      profileId === null ||
      blockMessage !== null ||
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
            message: replacementErrorMessage(error, "替换目标预览失败"),
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
            message: "后端返回了无效任务类型",
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
        setTrackedTaskState(pending ? nextRetargetInstallTaskState(running, pending) : running);
      })
      .catch((error: unknown) => {
        pendingEventsRef.current.clear();
        setTrackedTaskState({
          status: "failed",
          taskId: null,
          phase: failedPhase,
          message: replacementErrorMessage(error, "替换目标写入任务启动失败"),
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
            message: "后端返回了无效取消结果，请等待任务状态更新",
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
            message: replacementErrorMessage(error, "无法取消任务，请等待执行结果"),
          });
        }
      });
  };

  if (loadState.status === "loading") {
    return (
      <div className="replacement-panel__state" role="status">
        <LoaderCircle className="replacement-panel__spinner" size={20} aria-hidden="true" />
        <span>正在分析替换资源</span>
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
          重试
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
          <h3 id="replacement-source-title">检测结果</h3>
          <span>{analysis?.matchedAssetCount ?? 0} 个资源</span>
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
          <p className="replacement-panel__empty">未检测到可替换的外观槽位。</p>
        )}
        {analysis?.warnings.length ? (
          <ul className="replacement-panel__warnings" aria-label="分析警告">
            {analysis.warnings.map((warning) => (
              <li key={warning}>
                <AlertTriangle size={14} aria-hidden="true" />
                {warningLabels[warning]}
              </li>
            ))}
          </ul>
        ) : null}
      </section>

      <section className="replacement-panel__catalog" aria-labelledby="replacement-catalog-title">
        <div className="replacement-panel__section-heading">
          <h3 id="replacement-catalog-title">替换目标</h3>
          <span>{filteredTargets.length} 项</span>
        </div>
        <label className="replacement-panel__search">
          <Search size={16} aria-hidden="true" />
          <input
            type="search"
            aria-label="搜索替换目标"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索名称、别名或槽位"
            disabled={taskActive}
          />
        </label>
        {filteredTargets.length ? (
          <div className="replacement-panel__target-list" role="radiogroup" aria-label="替换目标">
            {filteredTargets.map((target) => {
              const currentInstalled = isCurrentInstalledReplacementTarget(
                target.id,
                installedTargetId,
              );
              return (
                <label
                  className="replacement-panel__target-row"
                  data-installed={currentInstalled}
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
                    <strong>{target.displayName}</strong>
                    {target.secondaryName ? <small>{target.secondaryName}</small> : null}
                    {currentInstalled ? (
                      <span className="replacement-panel__target-status">
                        <CheckCircle2 size={13} aria-hidden="true" />
                        当前已安装
                      </span>
                    ) : null}
                  </span>
                  <span className="replacement-panel__target-facts">
                    <code>{target.internalId}</code>
                    {target.catalogScope === "developer_sandbox" ? <small>人工目录</small> : null}
                  </span>
                </label>
              );
            })}
          </div>
        ) : (
          <p className="replacement-panel__empty">没有匹配的替换目标。</p>
        )}
      </section>

      {previewState.status !== "idle" ? (
        <section className="replacement-panel__preview" aria-live="polite">
          {previewState.status === "loading" ? (
            <div className="replacement-panel__inline-state">
              <LoaderCircle className="replacement-panel__spinner" size={17} aria-hidden="true" />
              正在生成预览
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
                <h3>{previewState.mode === "switch" ? "目标切换预览" : "写入预览"}</h3>
                {previewState.mode === "initial" ? (
                  <span>{previewState.preview.actions.length} 个动作</span>
                ) : null}
              </div>
              {previewState.mode === "initial" ? (
                <>
                  <dl className="replacement-panel__preview-facts">
                    <div>
                      <dt>资源类型</dt>
                      <dd>{previewState.preview.target.targetType}</dd>
                    </div>
                    <div>
                      <dt>目标编号</dt>
                      <dd>{previewState.preview.target.internalId}</dd>
                    </div>
                    <div>
                      <dt>写入动作</dt>
                      <dd>{previewState.preview.actions.length}</dd>
                    </div>
                  </dl>
                  {previewState.preview.installPlan.hasBlockingConflicts ? (
                    <div className="replacement-panel__inline-state is-error">
                      <ShieldAlert size={17} aria-hidden="true" />
                      检测到 {previewState.preview.installPlan.conflicts.length} 个阻断冲突
                    </div>
                  ) : (
                    <div className="replacement-panel__inline-state is-success">
                      <CheckCircle2 size={17} aria-hidden="true" />
                      未检测到阻断冲突
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
                    )}
                  </div>
                  {previewState.preview.prerequisiteDecision.codes.length > 0 ? (
                    <ul
                      className="replacement-panel__blocking-list"
                      aria-label="安装前置检查结果"
                    >
                      {previewState.preview.prerequisiteDecision.codes.map((code) => (
                        <li key={code}>
                          <AlertTriangle size={15} aria-hidden="true" />
                          <span>{getPrerequisiteDecisionCodeLabel(code)}</span>
                        </li>
                      ))}
                    </ul>
                  ) : null}
                </>
              ) : (
                <>
                  <dl className="replacement-panel__counts">
                    <div data-kind="retained">
                      <dt>保留</dt>
                      <dd>{previewState.preview.counts.retained}</dd>
                    </div>
                    <div data-kind="replaced">
                      <dt>替换</dt>
                      <dd>{previewState.preview.counts.replaced}</dd>
                    </div>
                    <div data-kind="added">
                      <dt>新增</dt>
                      <dd>{previewState.preview.counts.added}</dd>
                    </div>
                    <div data-kind="stale">
                      <dt>移除旧项</dt>
                      <dd>{previewState.preview.counts.stale}</dd>
                    </div>
                  </dl>
                  {previewState.preview.status === "ready" ? (
                    <div className="replacement-panel__inline-state is-success">
                      <CheckCircle2 size={17} aria-hidden="true" />
                      安全预检通过
                    </div>
                  ) : (
                    <ul className="replacement-panel__blocking-list" aria-label="目标切换阻断项">
                      {previewState.preview.blockingReasons.map((reason) => (
                        <li key={reason.code}>
                          <ShieldAlert size={15} aria-hidden="true" />
                          <span>{targetSwitchBlockingLabel(reason.code)}</span>
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
          <span>任务状态监听不可用</span>
          <button type="button" onClick={() => setListenerAttempt((value) => value + 1)}>
            重试监听
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
              正在启动安装任务
            </>
          ) : null}
          {taskState.status === "running" ? (
            <>
              <LoaderCircle className="replacement-panel__spinner" size={17} aria-hidden="true" />
              <span>{retargetInstallTaskPhaseLabel(taskState.phase)}</span>
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
                  {cancellationState.status === "requesting" ? "正在取消" : "取消任务"}
                </button>
              ) : null}
            </>
          ) : null}
          {taskState.status === "completed" ? (
            <>
              <CheckCircle2 size={17} aria-hidden="true" />
              {retargetInstallTaskPhaseLabel(taskState.phase)}
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
              {retargetInstallTaskPhaseLabel(taskState.phase)}
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
          <span>正在刷新安装状态</span>
        </div>
      ) : null}

      {refreshState.status === "failed" ? (
        <div className="replacement-panel__notice is-blocked" role="alert">
          <AlertTriangle size={17} aria-hidden="true" />
          <span>{refreshState.message}</span>
          <button type="button" onClick={refreshCompletedInstall}>
            <RefreshCw size={15} aria-hidden="true" />
            重试刷新
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
            !analysis?.retargetable ||
            blockMessage !== null ||
            previewState.status === "loading" ||
            taskActive
          }
        >
          <Eye size={16} aria-hidden="true" />
          {targetSwitch ? "预览目标切换" : "生成预览"}
        </button>
        <button
          type="button"
          className="is-primary"
          onClick={startInstall}
          disabled={
            previewState.status !== "ready" ||
            blockMessage !== null ||
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
          {targetSwitch ? "确认重装并切换" : "安装到此目标"}
        </button>
      </div>
    </div>
  );
}
