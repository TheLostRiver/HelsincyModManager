import { listen } from "@tauri-apps/api/event";
import {
  AlertTriangle,
  CheckCircle2,
  Eye,
  LoaderCircle,
  RefreshCw,
  Search,
  ShieldAlert,
  Target,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { GameId } from "../game-setup/gameSetupTypes";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "../mods/modImportTypes";
import type { InstallManifestStatus } from "../mods/modInstallPlanTypes";
import {
  analyzeImportedModReplacement,
  listReplacementTargets,
  previewInitialRetargetInstall,
  startRetargetInstallTask,
} from "./replacementApi";
import type {
  InitialRetargetInstallPreview,
  ReplacementAnalysis,
  ReplacementTarget,
  ReplacementWarning,
} from "./replacementTypes";
import {
  canStartInitialRetargetInstall,
  isRetargetInstallTaskPhase,
  nextRetargetInstallTaskState,
  refreshRetargetInstallState,
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
  | { status: "ready"; preview: InitialRetargetInstallPreview }
  | { status: "error"; message: string };

type TaskStateUpdate =
  | RetargetInstallTaskState
  | ((current: RetargetInstallTaskState) => RetargetInstallTaskState);

const warningLabels: Record<ReplacementWarning, string> = {
  no_supported_assets: "未检测到受支持的外观资源",
  multiple_sources: "检测到多个源槽位，当前版本不会自动拆分",
  unsupported_source: "包内包含当前版本不支持的源槽位",
  source_matches_target: "源槽位与目标槽位相同",
};

function replacementErrorMessage(error: unknown, fallback: string) {
  const code =
    typeof error === "object" && error !== null && "code" in error && typeof error.code === "string"
      ? error.code
      : null;
  switch (code) {
    case "replacement_mod_not_found":
      return "未找到已导入的 Mod";
    case "replacement_package_unavailable":
      return "导入包当前不可用";
    case "replacement_source_not_retargetable":
      return "该 Mod 不是当前可自动处理的单源外观包";
    case "replacement_target_catalog_unavailable":
      return "替换目标目录暂不可用";
    case "replacement_target_not_found":
      return "所选替换目标已不存在";
    case "replacement_install_state_unavailable":
      return "无法确认当前安装状态";
    case "replacement_initial_install_blocked":
      return "当前安装或恢复状态不允许首次替换安装";
    case "replacement_unsupported_game":
      return "当前游戏不支持替换目标";
    default:
      return fallback;
  }
}

function installBlockMessage(
  profileId: string | null,
  installStatus: InstallManifestStatus | undefined,
  completedLocally: boolean,
) {
  if (completedLocally || installStatus === "installed") {
    return "该 Mod 已安装。切换目标将在下一阶段通过真正重装完成。";
  }
  if (profileId === null) {
    return "当前 Profile 不可用。";
  }
  switch (installStatus) {
    case "not_installed":
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
      return "安装状态未知，首次替换安装已阻止。";
  }
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
  const [refreshState, setRefreshState] = useState<RetargetInstallRefreshState>({ status: "idle" });
  const [listenerAttempt, setListenerAttempt] = useState(0);
  const [listenerStatus, setListenerStatus] = useState<"connecting" | "ready" | "failed">(
    "connecting",
  );

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
        setRefreshState(next);
      }
    });
  }, [onInstallCompleted]);

  useEffect(() => {
    refreshGenerationRef.current += 1;
    completedTaskRef.current = null;
    setRefreshState({ status: "idle" });
  }, [gameId, modId]);

  useEffect(() => {
    let cancelled = false;
    previewRequestGenerationRef.current += 1;
    setLoadState({ status: "loading" });
    setSelectedTargetId(null);
    setPreviewState({ status: "idle" });

    void Promise.all([
      analyzeImportedModReplacement({ gameId, modId }),
      listReplacementTargets({ gameId }),
    ])
      .then(([analysis, targets]) => {
        if (!cancelled) {
          setLoadState({ status: "ready", analysis, targets });
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
  }, [gameId, modId, retryToken]);

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

  const targets = useMemo(
    () => (loadState.status === "ready" ? loadState.targets : []),
    [loadState],
  );
  const analysis = loadState.status === "ready" ? loadState.analysis : null;
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

  const selectTarget = (targetId: string) => {
    if (taskActive || installCompletedLocally) {
      return;
    }
    previewRequestGenerationRef.current += 1;
    setSelectedTargetId(targetId);
    setPreviewState({ status: "idle" });
    setTrackedTaskState({ status: "idle" });
  };

  const createPreview = () => {
    if (!selectedTarget || profileId === null || blockMessage !== null) {
      return;
    }
    const requestGeneration = ++previewRequestGenerationRef.current;
    setPreviewState({ status: "loading" });
    void previewInitialRetargetInstall({
      gameId,
      profileId,
      modId,
      targetId: selectedTarget.id,
      layerName: "base",
      layerPriority: 0,
    })
      .then((preview) => {
        if (previewRequestGenerationRef.current === requestGeneration) {
          setPreviewState({ status: "ready", preview });
        }
      })
      .catch((error: unknown) => {
        if (previewRequestGenerationRef.current === requestGeneration) {
          setPreviewState({
            status: "error",
            message: replacementErrorMessage(error, "替换安装预览失败"),
          });
        }
      });
  };

  const startInstall = () => {
    if (
      profileId === null ||
      selectedTarget === null ||
      previewState.status !== "ready" ||
      blockMessage !== null ||
      !canStartInitialRetargetInstall({
        installStatus,
        completedLocally: installCompletedLocally,
        hasPreview: true,
        hasBlockingConflicts: previewState.preview.installPlan.hasBlockingConflicts,
        taskActive,
        listenerReady: listenerStatus === "ready",
      })
    ) {
      return;
    }

    pendingEventsRef.current.clear();
    setTrackedTaskState({ status: "starting" });
    void startRetargetInstallTask({
      gameId,
      profileId,
      modId,
      targetId: selectedTarget.id,
      layerName: "base",
      layerPriority: 0,
    })
      .then((task) => {
        if (task.kind !== "install") {
          setTrackedTaskState({
            status: "failed",
            taskId: null,
            phase: "install.retarget.failed",
            message: "后端返回了无效任务类型",
          });
          return;
        }
        const running: RetargetInstallTaskState = {
          status: "running",
          taskId: task.taskId,
          phase: "install.retarget.queued",
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
          phase: "install.retarget.failed",
          message: replacementErrorMessage(error, "替换目标安装任务启动失败"),
        });
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
                <dd>{source.pathFamily}</dd>
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
            {filteredTargets.map((target) => (
              <label
                className="replacement-panel__target-row"
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
                    installCompletedLocally
                  }
                />
                <span className="replacement-panel__target-name">
                  <strong>{target.displayName}</strong>
                  {target.secondaryName ? <small>{target.secondaryName}</small> : null}
                </span>
                <code>{target.internalId}</code>
              </label>
            ))}
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
                <h3>写入预览</h3>
                <span>{previewState.preview.actions.length} 个动作</span>
              </div>
              <ul className="replacement-panel__action-list">
                {previewState.preview.actions.map((action) => (
                  <li key={`${action.sourceRelativePath}:${action.targetRelativePath}`}>
                    <code>{action.targetRelativePath}</code>
                  </li>
                ))}
              </ul>
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
              {retargetInstallTaskPhaseLabel(taskState.phase)}
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
            !analysis?.retargetable ||
            blockMessage !== null ||
            previewState.status === "loading" ||
            taskActive
          }
        >
          <Eye size={16} aria-hidden="true" />
          生成预览
        </button>
        <button
          type="button"
          className="is-primary"
          onClick={startInstall}
          disabled={
            previewState.status !== "ready" ||
            blockMessage !== null ||
            !canStartInitialRetargetInstall({
              installStatus,
              completedLocally: installCompletedLocally,
              hasPreview: previewState.status === "ready",
              hasBlockingConflicts:
                previewState.status === "ready" && previewState.preview.installPlan.hasBlockingConflicts,
              taskActive,
              listenerReady: listenerStatus === "ready",
            })
          }
        >
          <Target size={16} aria-hidden="true" />
          安装到此目标
        </button>
      </div>
    </div>
  );
}
