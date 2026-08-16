import { listen } from "@tauri-apps/api/event";
import { AlertTriangle, ArchiveRestore, CheckCircle2, Loader2, ShieldCheck, XCircle } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Dialog, useFeedback } from "../../shared/feedback";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "../mods/modImportTypes";
import type { SaveBackupSummaryDto } from "./profileSaveBackupTypes";
import {
  cancelProfileSaveRestoreTask,
  previewProfileSaveRestore,
  startProfileSaveRestore,
} from "./profileSaveRestoreApi";
import {
  ProfileSaveRestoreEarlyEventBuffer,
  attachProfileSaveRestoreTask,
  canCancelProfileSaveRestore,
  getProfileSaveRestoreErrorCode,
  getProfileSaveRestoreErrorMessage,
  getProfileSaveRestorePhaseLabel,
  getProfileSaveRestoreWarningMessage,
  isProfileSaveRestoreProgressEvent,
  isProfileSaveRestoreTaskStarted,
  markProfileSaveRestoreCancelling,
  nextProfileSaveRestoreTaskStateFromProgress,
  type ProfileSaveRestoreTaskState,
} from "./profileSaveRestoreTaskState";
import type { SaveRestorePreviewDto } from "./profileSaveRestoreTypes";

type RestorePreviewState =
  | { status: "idle" }
  | { status: "previewing" }
  | { status: "ready"; preview: SaveRestorePreviewDto }
  | { status: "error"; message: string };

type ListenerStatus = "preparing" | "ready" | "error";

export function SaveRestoreDialog({
  backup,
  profileId,
  previewMode,
  onClose,
  onCompleted,
}: {
  backup: SaveBackupSummaryDto | null;
  profileId: string | null;
  previewMode: boolean;
  onClose: () => void;
  onCompleted: () => void;
}) {
  const { pushToast } = useFeedback();
  const [previewState, setPreviewState] = useState<RestorePreviewState>({ status: "idle" });
  const [taskState, setTaskState] = useState<ProfileSaveRestoreTaskState>({ status: "idle" });
  const [listenerStatus, setListenerStatus] = useState<ListenerStatus>(previewMode ? "ready" : "preparing");
  const [confirmedWithoutPreRestore, setConfirmedWithoutPreRestore] = useState(false);
  const taskStateRef = useRef(taskState);
  const taskIdRef = useRef<string | null>(null);
  const startPendingRef = useRef(false);
  const pendingEventsRef = useRef(new ProfileSaveRestoreEarlyEventBuffer());
  const terminalEffectKeyRef = useRef<string | null>(null);
  const onCompletedRef = useRef(onCompleted);
  taskStateRef.current = taskState;
  onCompletedRef.current = onCompleted;

  const applyTaskEvent = useCallback((event: TaskProgressEventDto) => {
    if (!isProfileSaveRestoreProgressEvent(event)) return;
    if (startPendingRef.current && taskIdRef.current === null) {
      pendingEventsRef.current.push(event);
      return;
    }
    if (taskIdRef.current !== event.taskId) return;
    setTaskState((current) => nextProfileSaveRestoreTaskStateFromProgress(current, event));
  }, []);

  useEffect(() => {
    setConfirmedWithoutPreRestore(false);
    setTaskState({ status: "idle" });
    setListenerStatus(previewMode ? "ready" : "preparing");
    taskIdRef.current = null;
    startPendingRef.current = false;
    pendingEventsRef.current.clear();
    terminalEffectKeyRef.current = null;

    if (!backup || !profileId) {
      setPreviewState({ status: "idle" });
      return undefined;
    }

    let cancelled = false;
    setPreviewState({ status: "previewing" });
    const request = previewMode
      ? Promise.resolve(createPreview(backup))
      : previewProfileSaveRestore({
          gameId: backup.gameId,
          profileId,
          backupId: backup.backupId,
        });
    void request
      .then((preview) => {
        if (!cancelled) setPreviewState({ status: "ready", preview });
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setPreviewState({ status: "error", message: getProfileSaveRestoreErrorMessage(error) });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [backup, previewMode, profileId]);

  useEffect(() => {
    if (!backup || !profileId || previewMode) {
      setListenerStatus("ready");
      return undefined;
    }

    let disposed = false;
    let unlisten: (() => void) | null = null;
    setListenerStatus("preparing");
    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, (event) => {
      if (!disposed) applyTaskEvent(event.payload);
    })
      .then((value) => {
        if (disposed) {
          value();
          return;
        }
        unlisten = value;
        setListenerStatus("ready");
      })
      .catch(() => {
        if (!disposed) setListenerStatus("error");
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [applyTaskEvent, backup, previewMode, profileId]);

  useEffect(() => {
    if (!("taskId" in taskState)) return;
    const effectKey = `${taskState.status}:${taskState.taskId}`;
    if (terminalEffectKeyRef.current === effectKey) return;

    if (taskState.status === "completed") {
      terminalEffectKeyRef.current = effectKey;
      pushToast({
        eventKey: `profile.save-restore.completed.${taskState.taskId}`,
        taskId: taskState.taskId,
        tone: taskState.evidenceDegraded || taskState.warningCodes.length > 0 ? "warning" : "success",
        title: taskState.evidenceDegraded ? "存档已恢复，证据需检查" : "存档恢复完成",
        message: taskState.warningCodes.length > 0
          ? getProfileSaveRestoreWarningMessage(taskState.warningCodes[0])
          : "目标存档已通过校验并完成替换。",
      });
      onCompletedRef.current();
      return;
    }

    if (taskState.status === "recovery_required") {
      terminalEffectKeyRef.current = effectKey;
      pushToast({
        eventKey: `profile.save-restore.recovery-required.${taskState.taskId}`,
        taskId: taskState.taskId,
        tone: "danger",
        title: "存档恢复需要人工处理",
        message: taskState.message,
      });
      return;
    }

    if (taskState.status === "failed") {
      terminalEffectKeyRef.current = effectKey;
      const warning = taskState.warningCodes[0];
      pushToast({
        eventKey: `profile.save-restore.failed.${taskState.taskId ?? "unstarted"}`,
        taskId: taskState.taskId ?? undefined,
        tone: "danger",
        title: "存档恢复失败",
        message: warning
          ? `${taskState.message} ${getProfileSaveRestoreWarningMessage(warning)}`
          : taskState.message,
      });
      return;
    }

    if (taskState.status === "cancelled") {
      terminalEffectKeyRef.current = effectKey;
      pushToast({
        eventKey: `profile.save-restore.cancelled.${taskState.taskId}`,
        taskId: taskState.taskId,
        tone: "neutral",
        title: "已取消存档恢复",
        message: "未进入提交阶段的恢复工作已停止。",
      });
    }
  }, [pushToast, taskState]);

  if (!backup || !profileId) return null;
  const preview = previewState.status === "ready" ? previewState.preview : null;
  const taskBusy = taskState.status === "starting"
    || taskState.status === "running"
    || taskState.status === "cancelling";
  const canConfirm = previewState.status === "ready"
    && taskState.status === "idle"
    && listenerStatus === "ready"
    && (!previewState.preview.requiresAdditionalConfirmation || confirmedWithoutPreRestore);

  async function startRestore() {
    const selectedBackup = backup;
    const selectedProfileId = profileId;
    if (!preview || !canConfirm || !selectedBackup || !selectedProfileId) return;
    startPendingRef.current = true;
    taskIdRef.current = null;
    pendingEventsRef.current.clear();
    setTaskState({ status: "starting" });

    if (previewMode) {
      const taskId = `preview-save-restore-${Date.now()}`;
      startPendingRef.current = false;
      taskIdRef.current = taskId;
      setTaskState({ status: "completed", taskId, evidenceDegraded: false, warningCodes: [] });
      return;
    }

    try {
      const task = await startProfileSaveRestore({
        gameId: selectedBackup.gameId,
        profileId: selectedProfileId,
        backupId: selectedBackup.backupId,
        previewToken: preview.previewToken,
        confirmedWithoutPreRestore,
      });
      if (!isProfileSaveRestoreTaskStarted(task)) {
        throw { code: "save_restore_transaction_unavailable" };
      }
      taskIdRef.current = task.taskId;
      startPendingRef.current = false;
      const attached = attachProfileSaveRestoreTask(
        task.taskId,
        pendingEventsRef.current.take(task.taskId),
      );
      setTaskState(attached);
    } catch (error) {
      startPendingRef.current = false;
      taskIdRef.current = null;
      pendingEventsRef.current.clear();
      setTaskState({
        status: "failed",
        taskId: null,
        errorCode: getProfileSaveRestoreErrorCode(error),
        message: getProfileSaveRestoreErrorMessage(error),
        warningCodes: [],
      });
    }
  }

  async function cancelRestore() {
    const current = taskStateRef.current;
    if (current.status !== "running" || !canCancelProfileSaveRestore(current)) return;
    const cancelling = markProfileSaveRestoreCancelling(current);
    taskStateRef.current = cancelling;
    setTaskState(cancelling);
    try {
      const cancelled = await cancelProfileSaveRestoreTask(current.taskId);
      if (cancelled.kind !== "save_restore" || cancelled.status !== "cancelled" || cancelled.taskId !== current.taskId) {
        throw { code: "task_cannot_be_cancelled" };
      }
      applyTaskEvent({
        taskId: cancelled.taskId,
        kind: "save_restore",
        status: "cancelled",
        phase: "save_restore.cancelled",
        current: null,
        total: null,
        message: null,
        error: null,
        resultRef: null,
      });
    } catch (error) {
      pushToast({
        eventKey: `profile.save-restore.cancel-rejected.${current.taskId}`,
        taskId: current.taskId,
        tone: "warning",
        title: "当前阶段无法取消",
        message: getCancelErrorMessage(error),
      });
      setTaskState((latest) => latest.status === "cancelling"
        ? { status: "running", taskId: latest.taskId, phase: latest.phase }
        : latest);
    }
  }

  return (
    <Dialog
      open
      role="alertdialog"
      title="恢复存档"
      description="恢复会替换当前配置档的存档内容，请核对备份点与保护策略。"
      icon={<ArchiveRestore size={18} />}
      busy={taskBusy}
      onClose={onClose}
      footer={renderFooter()}
    >
      <div className="profile-save-restore-dialog">
        {previewState.status === "previewing" ? (
          <div className="profile-restore-status">
            <Loader2 className="profile-spinner" />
            <span>正在校验归档与目标存档...</span>
          </div>
        ) : null}
        {listenerStatus === "preparing" && previewState.status === "ready" ? (
          <div className="profile-restore-status">
            <Loader2 className="profile-spinner" />
            <span>正在建立恢复进度通道...</span>
          </div>
        ) : null}
        {listenerStatus === "error" ? (
          <div className="profile-restore-status is-error" role="alert">
            <AlertTriangle />
            <span>无法订阅恢复进度，恢复尚未启动。请关闭面板后重试。</span>
          </div>
        ) : null}
        {previewState.status === "error" ? (
          <div className="profile-restore-status is-error" role="alert">
            <AlertTriangle />
            <span>{previewState.message}</span>
          </div>
        ) : null}
        {preview ? (
          <>
            <dl className="profile-restore-facts">
              <div><dt>备份点</dt><dd>{preview.backup.notes?.trim() || preview.backup.fileName}</dd></div>
              <div><dt>文件</dt><dd>{preview.fileCount} 个</dd></div>
              <div><dt>解压大小</dt><dd>{formatBytes(preview.totalUncompressedBytes)}</dd></div>
            </dl>
            <div className={`profile-restore-protection ${preview.preRestoreBackupEnabled ? "is-enabled" : "is-disabled"}`}>
              {preview.preRestoreBackupEnabled ? <ShieldCheck size={18} /> : <AlertTriangle size={18} />}
              <div>
                <strong>{preview.preRestoreBackupEnabled ? "恢复前安全备份已开启" : "恢复前安全备份已关闭"}</strong>
                <span>{preview.preRestoreBackupEnabled ? "提交前会先创建独立保护点，失败时停止恢复。" : "本次恢复没有自动保护点，风险更高。"}</span>
              </div>
            </div>
            {preview.requiresAdditionalConfirmation ? (
              <label className="profile-restore-high-risk-confirmation">
                <input
                  type="checkbox"
                  checked={confirmedWithoutPreRestore}
                  onChange={(event) => setConfirmedWithoutPreRestore(event.target.checked)}
                />
                <span>我理解当前未启用恢复前安全备份，并确认继续。</span>
              </label>
            ) : null}
          </>
        ) : null}
        {taskState.status === "starting" ? (
          <RestoreStatus icon={<Loader2 className="profile-spinner" />} label="正在启动恢复任务" />
        ) : null}
        {taskState.status === "running" || taskState.status === "cancelling" ? (
          <RestoreStatus
            icon={<Loader2 className="profile-spinner" />}
            label={taskState.status === "cancelling" ? "正在取消恢复任务" : getProfileSaveRestorePhaseLabel(taskState.phase)}
          />
        ) : null}
        {taskState.status === "completed" ? (
          <>
            <RestoreStatus
              tone="success"
              icon={<CheckCircle2 />}
              label="恢复完成，当前存档已经过提交后校验。"
            />
            {taskState.warningCodes.map((code) => (
              <RestoreStatus
                key={code}
                tone="warning"
                icon={<AlertTriangle />}
                label={getProfileSaveRestoreWarningMessage(code)}
              />
            ))}
          </>
        ) : null}
        {taskState.status === "recovery_required" ? (
          <div className="profile-restore-status is-danger" role="alert">
            <AlertTriangle />
            <div>
              <strong>恢复需要人工收敛</strong>
              <span>{taskState.message} 请保留当前现场并联系支持，暂不要继续恢复。</span>
            </div>
          </div>
        ) : null}
        {taskState.status === "failed" ? (
          <>
            <RestoreStatus tone="error" icon={<AlertTriangle />} label={taskState.message} alert />
            {taskState.warningCodes.map((code) => (
              <RestoreStatus
                key={code}
                tone="warning"
                icon={<AlertTriangle />}
                label={getProfileSaveRestoreWarningMessage(code)}
              />
            ))}
          </>
        ) : null}
        {taskState.status === "cancelled" ? (
          <RestoreStatus icon={<XCircle />} label="恢复任务已取消，未继续进入玩家文件提交。" />
        ) : null}
      </div>
    </Dialog>
  );

  function renderFooter() {
    if (taskState.status === "completed"
      || taskState.status === "failed"
      || taskState.status === "recovery_required"
      || taskState.status === "cancelled") {
      return <button type="button" className="profile-action-button is-primary" onClick={onClose}>完成</button>;
    }

    if (taskState.status === "running" || taskState.status === "cancelling" || taskState.status === "starting") {
      const cancellable = taskState.status === "running" && canCancelProfileSaveRestore(taskState);
      return (
        <button
          type="button"
          className="profile-action-button"
          disabled={!cancellable}
          onClick={() => void cancelRestore()}
        >
          {taskState.status === "cancelling" ? <Loader2 className="profile-spinner" size={15} /> : <XCircle size={15} />}
          {taskState.status === "cancelling"
            ? "正在取消"
            : taskState.status === "starting"
              ? "正在启动"
              : taskState.phase === "save_restore.committing"
                ? "正在提交"
                : "取消恢复"}
        </button>
      );
    }

    return (
      <>
        <button type="button" className="profile-action-button" onClick={onClose}>取消</button>
        <button
          type="button"
          className="profile-action-button is-primary"
          disabled={!canConfirm}
          onClick={() => void startRestore()}
        >
          <ArchiveRestore size={15} />
          确认恢复
        </button>
      </>
    );
  }
}

function RestoreStatus({
  icon,
  label,
  tone = "neutral",
  alert = false,
}: {
  icon: React.ReactNode;
  label: string;
  tone?: "neutral" | "success" | "warning" | "error";
  alert?: boolean;
}) {
  return (
    <div className={`profile-restore-status${tone === "neutral" ? "" : ` is-${tone}`}`} role={alert ? "alert" : "status"}>
      {icon}
      <span>{label}</span>
    </div>
  );
}

function createPreview(backup: SaveBackupSummaryDto): SaveRestorePreviewDto {
  return {
    backup,
    fileCount: backup.fileCount,
    totalUncompressedBytes: backup.sizeBytes,
    preRestoreBackupEnabled: true,
    requiresAdditionalConfirmation: false,
    warningCodes: [],
    previewToken: "preview-token",
    expiresAt: Date.now() + 300_000,
  };
}

function getCancelErrorMessage(error: unknown) {
  const code = getProfileSaveRestoreErrorCode(error);
  if (code === "task_cannot_be_cancelled") return "恢复已进入提交阶段，必须先完成提交或回滚收尾。";
  if (code === "task_not_found") return "恢复任务已结束或不再可取消。";
  return "取消请求未被接受，恢复任务仍按当前状态继续。";
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
}
