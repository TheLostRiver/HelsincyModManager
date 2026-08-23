import { listen } from "@tauri-apps/api/event";
import { AlertTriangle, ArchiveRestore, CheckCircle2, Loader2, ShieldCheck, XCircle } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Dialog, useFeedback } from "../../shared/feedback";
import { resolveCopy, useI18n } from "../../shared/i18n";
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
import { saveRestoreCopy, type SaveRestoreCodeCopy } from "./saveRestoreCopy";

type RestorePreviewState =
  | { status: "idle" }
  | { status: "previewing" }
  | { status: "ready"; preview: SaveRestorePreviewDto }
  | { status: "error"; errorCode: string | null };

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
  const { locale } = useI18n();
  const copy = resolveCopy(saveRestoreCopy, locale);
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
          setPreviewState({ status: "error", errorCode: getProfileSaveRestoreErrorCode(error) });
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
        title: taskState.evidenceDegraded ? copy.toasts.completedEvidenceTitle : copy.toasts.completedTitle,
        message: taskState.warningCodes.length > 0
          ? getProfileSaveRestoreWarningMessage(taskState.warningCodes[0], copy.warnings)
          : copy.toasts.completedMessage,
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
        title: copy.toasts.recoveryRequiredTitle,
        message: getProfileSaveRestoreErrorMessage(taskState.errorCode, copy.errors),
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
        title: copy.toasts.failedTitle,
        message: warning
          ? `${getProfileSaveRestoreErrorMessage(taskState.errorCode, copy.errors)} ${getProfileSaveRestoreWarningMessage(warning, copy.warnings)}`
          : getProfileSaveRestoreErrorMessage(taskState.errorCode, copy.errors),
      });
      return;
    }

    if (taskState.status === "cancelled") {
      terminalEffectKeyRef.current = effectKey;
      pushToast({
        eventKey: `profile.save-restore.cancelled.${taskState.taskId}`,
        taskId: taskState.taskId,
        tone: "neutral",
        title: copy.toasts.cancelledTitle,
        message: copy.toasts.cancelledMessage,
      });
    }
  }, [copy, pushToast, taskState]);

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
        title: copy.toasts.cancelRejectedTitle,
        message: getCancelErrorMessage(error, copy.cancelErrors),
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
      title={copy.dialog.title}
      description={copy.dialog.description}
      icon={<ArchiveRestore size={18} />}
      busy={taskBusy}
      onClose={onClose}
      footer={renderFooter()}
    >
      <div className="profile-save-restore-dialog">
        {previewState.status === "previewing" ? (
          <div className="profile-restore-status">
            <Loader2 className="profile-spinner" />
            <span>{copy.dialog.previewing}</span>
          </div>
        ) : null}
        {listenerStatus === "preparing" && previewState.status === "ready" ? (
          <div className="profile-restore-status">
            <Loader2 className="profile-spinner" />
            <span>{copy.dialog.preparingChannel}</span>
          </div>
        ) : null}
        {listenerStatus === "error" ? (
          <div className="profile-restore-status is-error" role="alert">
            <AlertTriangle />
            <span>{copy.dialog.listenerFailed}</span>
          </div>
        ) : null}
        {previewState.status === "error" ? (
          <div className="profile-restore-status is-error" role="alert">
            <AlertTriangle />
            <span>{getProfileSaveRestoreErrorMessage(previewState.errorCode, copy.errors)}</span>
          </div>
        ) : null}
        {preview ? (
          <>
            <dl className="profile-restore-facts">
              <div><dt>{copy.dialog.factBackupPoint}</dt><dd>{preview.backup.notes?.trim() || preview.backup.fileName}</dd></div>
              <div><dt>{copy.dialog.factFiles}</dt><dd>{copy.dialog.factFileCount(preview.fileCount)}</dd></div>
              <div><dt>{copy.dialog.factUncompressedSize}</dt><dd>{formatBytes(preview.totalUncompressedBytes)}</dd></div>
            </dl>
            <div className={`profile-restore-protection ${preview.preRestoreBackupEnabled ? "is-enabled" : "is-disabled"}`}>
              {preview.preRestoreBackupEnabled ? <ShieldCheck size={18} /> : <AlertTriangle size={18} />}
              <div>
                <strong>{preview.preRestoreBackupEnabled ? copy.dialog.protectionOnTitle : copy.dialog.protectionOffTitle}</strong>
                <span>{preview.preRestoreBackupEnabled ? copy.dialog.protectionOnHint : copy.dialog.protectionOffHint}</span>
              </div>
            </div>
            {preview.requiresAdditionalConfirmation ? (
              <label className="profile-restore-high-risk-confirmation">
                <input
                  type="checkbox"
                  checked={confirmedWithoutPreRestore}
                  onChange={(event) => setConfirmedWithoutPreRestore(event.target.checked)}
                />
                <span>{copy.dialog.highRiskConfirmLabel}</span>
              </label>
            ) : null}
          </>
        ) : null}
        {taskState.status === "starting" ? (
          <RestoreStatus icon={<Loader2 className="profile-spinner" />} label={copy.dialog.startingTask} />
        ) : null}
        {taskState.status === "running" || taskState.status === "cancelling" ? (
          <RestoreStatus
            icon={<Loader2 className="profile-spinner" />}
            label={taskState.status === "cancelling" ? copy.dialog.cancellingTask : getProfileSaveRestorePhaseLabel(taskState.phase, copy.phases)}
          />
        ) : null}
        {taskState.status === "completed" ? (
          <>
            <RestoreStatus
              tone="success"
              icon={<CheckCircle2 />}
              label={copy.dialog.completedInline}
            />
            {taskState.warningCodes.map((code) => (
              <RestoreStatus
                key={code}
                tone="warning"
                icon={<AlertTriangle />}
                label={getProfileSaveRestoreWarningMessage(code, copy.warnings)}
              />
            ))}
          </>
        ) : null}
        {taskState.status === "recovery_required" ? (
          <div className="profile-restore-status is-danger" role="alert">
            <AlertTriangle />
            <div>
              <strong>{copy.dialog.recoveryRequiredTitle}</strong>
              <span>{getProfileSaveRestoreErrorMessage(taskState.errorCode, copy.errors)} {copy.dialog.recoveryRequiredSuffix}</span>
            </div>
          </div>
        ) : null}
        {taskState.status === "failed" ? (
          <>
            <RestoreStatus tone="error" icon={<AlertTriangle />} label={getProfileSaveRestoreErrorMessage(taskState.errorCode, copy.errors)} alert />
            {taskState.warningCodes.map((code) => (
              <RestoreStatus
                key={code}
                tone="warning"
                icon={<AlertTriangle />}
                label={getProfileSaveRestoreWarningMessage(code, copy.warnings)}
              />
            ))}
          </>
        ) : null}
        {taskState.status === "cancelled" ? (
          <RestoreStatus icon={<XCircle />} label={copy.dialog.cancelledInline} />
        ) : null}
      </div>
    </Dialog>
  );

  function renderFooter() {
    if (taskState.status === "completed"
      || taskState.status === "failed"
      || taskState.status === "recovery_required"
      || taskState.status === "cancelled") {
      return <button type="button" className="profile-action-button is-primary" onClick={onClose}>{copy.dialog.footerDone}</button>;
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
            ? copy.dialog.footerCancelling
            : taskState.status === "starting"
              ? copy.dialog.footerStarting
              : taskState.phase === "save_restore.committing"
                ? copy.dialog.footerCommitting
                : copy.dialog.footerCancelRestore}
        </button>
      );
    }

    return (
      <>
        <button type="button" className="profile-action-button" onClick={onClose}>{copy.dialog.footerCancel}</button>
        <button
          type="button"
          className="profile-action-button is-primary"
          disabled={!canConfirm}
          onClick={() => void startRestore()}
        >
          <ArchiveRestore size={15} />
          {copy.dialog.footerConfirm}
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

function getCancelErrorMessage(error: unknown, cancelErrors: SaveRestoreCodeCopy) {
  const code = getProfileSaveRestoreErrorCode(error);
  return code ? cancelErrors.byCode[code] ?? cancelErrors.fallback : cancelErrors.fallback;
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
}
