import {
  AlertTriangle,
  FolderOpen,
  HardDrive,
  LoaderCircle,
  RefreshCcw,
  RotateCcw,
  XCircle,
} from "lucide-react";
import { useRef } from "react";
import { Dialog } from "../../shared/feedback";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { modStorageCopy, type ModStorageCopy } from "./modStorageCopy";
import {
  canCancelModStorageMigration,
  getModStorageMigrationPhaseLabel,
  isModStorageMigrationActive,
  type ModStorageMigrationTaskState,
} from "./modStorageMigrationTaskState";
import { useModStorageSettings } from "./ModStorageSettingsProvider";
import { getModStorageDegradedMessage, getModStorageErrorMessage } from "./modStorageTypes";
import type { ModStoragePendingChange } from "./useModStorageSettings";

/**
 * Settings section body for the Mod storage directory (#275). Every fact shown here comes
 * from the backend snapshot or task events; the panel only projects codes to copy.
 */
export function ModStorageSettingsPanel() {
  const { locale } = useI18n();
  const copy = resolveCopy(modStorageCopy, locale);
  const state = useModStorageSettings();
  const { loadState, migration } = state;

  if (loadState.status === "loading") {
    return (
      <div className="mod-storage-panel">
        <p className="mod-storage-panel__status" role="status" aria-live="polite">
          <LoaderCircle className="mod-storage-panel__spinner" size={14} aria-hidden="true" />
          {copy.current.loading}
        </p>
      </div>
    );
  }

  if (loadState.status === "error") {
    return (
      <div className="mod-storage-panel">
        <div className="settings-callout mod-storage-panel__callout" role="alert">
          <AlertTriangle size={16} strokeWidth={2.1} />
          <span>{getModStorageErrorMessage(loadState.errorCode, locale)}</span>
          <button type="button" onClick={state.reload}>
            <RefreshCcw size={13} strokeWidth={2.2} aria-hidden="true" />
            {copy.current.reload}
          </button>
        </div>
      </div>
    );
  }

  const settings = loadState.settings;
  const migrationActive = isModStorageMigrationActive(migration);
  const restartRequired = settings.restartRequired || settings.writesFrozen === "restart_required";
  const actionsDisabled = state.busy || migrationActive || settings.writesFrozen !== "none";
  const pendingConfigured =
    settings.configuredDir !== null && settings.configuredDir !== settings.effectiveDir;
  const pendingNote =
    settings.restartRequired && settings.configuredDir === null
      ? copy.current.pendingDefault
      : pendingConfigured
        ? copy.current.pendingChange(settings.configuredDir ?? "")
        : null;

  return (
    <div className="mod-storage-panel">
      <div className="mod-storage-panel__current">
        <div className="mod-storage-panel__current-head">
          <strong>{copy.current.title}</strong>
          <span
            className={
              "mod-storage-panel__badge" +
              (settings.source === "configured" ? " is-custom" : "")
            }
          >
            {settings.source === "configured" ? copy.current.customBadge : copy.current.defaultBadge}
          </span>
        </div>
        <code className="mod-storage-panel__path" title={settings.effectiveDir}>
          {settings.effectiveDir}
        </code>
        <span className="mod-storage-panel__hint">
          {settings.libraryEmpty ? copy.current.libraryEmpty : copy.current.libraryHasPackages}
        </span>
        {pendingNote ? <span className="mod-storage-panel__hint">{pendingNote}</span> : null}
      </div>

      {settings.degradedReason ? (
        <div className="settings-callout mod-storage-panel__callout" role="alert">
          <AlertTriangle size={16} strokeWidth={2.1} />
          <span>
            <strong>{copy.degraded.title}</strong>{" "}
            {getModStorageDegradedMessage(settings.degradedReason, locale)}
            {settings.degradedDetail ? ` ${getModStorageErrorMessage(settings.degradedDetail, locale)}` : ""}
          </span>
        </div>
      ) : null}

      {restartRequired && !migrationActive ? (
        <div className="settings-callout mod-storage-panel__callout" role="alert">
          <RotateCcw size={16} strokeWidth={2.1} />
          <span>
            <strong>{copy.restart.title}</strong> {copy.restart.message}
          </span>
        </div>
      ) : null}

      {migrationActive ? (
        <MigrationProgress
          migration={migration}
          copy={copy}
          cancelPending={state.cancelPending}
          onCancel={() => void state.cancelMigration()}
        />
      ) : null}

      {migration.status === "failed" ? (
        <div className="settings-callout mod-storage-panel__callout" role="alert">
          <AlertTriangle size={16} strokeWidth={2.1} />
          <span>
            <strong>{copy.migration.failedTitle}</strong>{" "}
            {getModStorageErrorMessage(migration.errorCode, locale)}
          </span>
          <button type="button" onClick={state.dismissMigrationResult}>
            {copy.actions.dismiss}
          </button>
        </div>
      ) : null}

      {state.actionError !== null ? (
        <div className="settings-callout mod-storage-panel__callout" role="alert">
          <AlertTriangle size={16} strokeWidth={2.1} />
          <span>{getModStorageErrorMessage(state.actionError, locale)}</span>
          <button type="button" onClick={state.dismissActionError}>
            {copy.actions.dismiss}
          </button>
        </div>
      ) : null}

      {state.listenerStatus === "failed" ? (
        <div className="settings-callout mod-storage-panel__callout" role="alert">
          <AlertTriangle size={16} strokeWidth={2.1} />
          <span>{getModStorageErrorMessage("mod_storage_migration_listener_unavailable", locale)}</span>
          <button type="button" onClick={state.retryListener}>
            <RefreshCcw size={13} strokeWidth={2.2} aria-hidden="true" />
            {copy.actions.retryListener}
          </button>
        </div>
      ) : null}

      <div className="mod-storage-panel__actions">
        <button
          type="button"
          className="mod-storage-panel__button is-primary"
          disabled={actionsDisabled}
          onClick={() => void state.chooseDirectory()}
        >
          {state.busy ? (
            <LoaderCircle className="mod-storage-panel__spinner" size={14} aria-hidden="true" />
          ) : (
            <FolderOpen size={14} strokeWidth={2.2} aria-hidden="true" />
          )}
          {state.busy ? copy.actions.busy : copy.actions.choose}
        </button>
        {settings.configuredDir !== null ? (
          <button
            type="button"
            className="mod-storage-panel__button"
            disabled={actionsDisabled}
            onClick={state.chooseDefault}
          >
            <HardDrive size={14} strokeWidth={2.2} aria-hidden="true" />
            {copy.actions.restoreDefault}
          </button>
        ) : null}
      </div>

      <ModStorageChangeDialog
        pending={state.pendingChange}
        defaultDir={settings.defaultDir}
        copy={copy}
        onCancel={state.dismissPendingChange}
        onConfirm={() => void state.confirmPendingChange()}
      />
    </div>
  );
}

type MigrationProgressProps = {
  migration: ModStorageMigrationTaskState;
  copy: ModStorageCopy;
  cancelPending: boolean;
  onCancel: () => void;
};

function MigrationProgress({ migration, copy, cancelPending, onCancel }: MigrationProgressProps) {
  const phase =
    migration.status === "running" || migration.status === "cancelling" ? migration.phase : null;
  const current =
    migration.status === "running" || migration.status === "cancelling" ? migration.current : null;
  const total =
    migration.status === "running" || migration.status === "cancelling" ? migration.total : null;
  const percent = current !== null && total !== null && total > 0 ? Math.round((current / total) * 100) : null;
  const label =
    phase === null
      ? copy.migration.phases["mod_storage.migration.queued"]
      : getModStorageMigrationPhaseLabel(phase, copy.migration);
  const progressText =
    current !== null && total !== null ? copy.migration.progress(String(current), String(total)) : "";
  const canCancel = canCancelModStorageMigration(migration) && !cancelPending;
  const cancelHint =
    migration.status === "running" && !canCancelModStorageMigration(migration)
      ? copy.actions.cancelUnavailable
      : undefined;

  return (
    <div className="mod-storage-panel__migration" role="group" aria-label={copy.migration.title}>
      <div className="mod-storage-panel__migration-head">
        <LoaderCircle className="mod-storage-panel__spinner" size={14} aria-hidden="true" />
        <strong>{copy.migration.title}</strong>
        <span role="status" aria-live="polite">
          {label}
          {progressText}
        </span>
      </div>
      <div
        className="mod-storage-panel__progress"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={total ?? undefined}
        aria-valuenow={current ?? undefined}
        aria-valuetext={`${label}${progressText}`}
      >
        <span
          className="mod-storage-panel__progress-fill"
          style={{ width: percent === null ? undefined : `${percent}%` }}
          data-indeterminate={percent === null ? "true" : undefined}
        />
      </div>
      {migration.status === "running" ? (
        <div className="mod-storage-panel__actions">
          <button
            type="button"
            className="mod-storage-panel__button is-danger"
            disabled={!canCancel}
            title={cancelHint}
            onClick={onCancel}
          >
            {cancelPending ? (
              <LoaderCircle className="mod-storage-panel__spinner" size={14} aria-hidden="true" />
            ) : (
              <XCircle size={14} strokeWidth={2.2} aria-hidden="true" />
            )}
            {cancelPending ? copy.actions.cancelling : copy.actions.cancelMigration}
          </button>
          {cancelHint ? <span className="mod-storage-panel__hint">{cancelHint}</span> : null}
        </div>
      ) : null}
    </div>
  );
}

type ModStorageChangeDialogProps = {
  pending: ModStoragePendingChange | null;
  defaultDir: string;
  copy: ModStorageCopy;
  onCancel: () => void;
  onConfirm: () => void;
};

/**
 * Both outcomes are hard to undo (restart required; a migration copies the whole library), so
 * the dialog is an alertdialog that cannot be dismissed by clicking outside and lands focus on
 * Cancel.
 */
function ModStorageChangeDialog({ pending, defaultDir, copy, onCancel, onConfirm }: ModStorageChangeDialogProps) {
  const cancelButtonRef = useRef<HTMLButtonElement | null>(null);
  if (pending === null) {
    return null;
  }
  const directoryLabel = pending.directory ?? `${copy.confirm.defaultDirectoryLabel} (${defaultDir})`;
  const migrate = pending.mode === "migrate";

  return (
    <Dialog
      open
      role="alertdialog"
      title={migrate ? copy.confirm.migrateTitle : copy.confirm.setTitle}
      description={migrate ? copy.confirm.migrateBody(directoryLabel) : copy.confirm.setBody(directoryLabel)}
      icon={<HardDrive size={20} />}
      onClose={onCancel}
      closeLabel={copy.confirm.closeAria}
      closeOnBackdrop={false}
      initialFocusRef={cancelButtonRef}
      footer={
        <>
          <button ref={cancelButtonRef} type="button" className="mod-storage-panel__button" onClick={onCancel}>
            {copy.confirm.cancel}
          </button>
          <button type="button" className="mod-storage-panel__button is-primary" onClick={onConfirm}>
            {migrate ? copy.confirm.migrateConfirm : copy.confirm.setConfirm}
          </button>
        </>
      }
    >
      {migrate ? (
        <ul className="mod-storage-panel__steps">
          <li>{copy.confirm.migrateStepCopy}</li>
          <li>{copy.confirm.migrateStepFreeze}</li>
          <li>{copy.confirm.migrateStepRestart}</li>
        </ul>
      ) : null}
    </Dialog>
  );
}
