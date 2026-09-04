import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useFeedback } from "../../shared/feedback";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "../mods/modImportTypes";
import {
  cancelModStorageMigrationTask,
  getModStorageSettings,
  setModStorageDir,
  startModStorageMigrationTask,
  validateModStorageDir,
} from "./modStorageApi";
import { modStorageCopy } from "./modStorageCopy";
import {
  getModStorageMigrationPhaseLabel,
  isModStorageMigrationActive,
  isModStorageMigrationTerminal,
  MOD_STORAGE_MIGRATION_PHASES,
  nextModStorageMigrationStateFromProgress,
  type ModStorageMigrationTaskState,
} from "./modStorageMigrationTaskState";
import {
  getModStorageErrorMessage,
  isModStorageDirValidationDto,
  isModStorageSettingsDto,
  modStorageErrorCodeFrom,
  type ModStorageSettingsDto,
  type ModStorageWritesFrozen,
} from "./modStorageTypes";

export type ModStorageLoadState =
  | { status: "loading" }
  | { status: "ready"; settings: ModStorageSettingsDto }
  | { status: "error"; errorCode: string };

/** A directory change the user still has to confirm; `directory: null` restores the default. */
export type ModStoragePendingChange = {
  directory: string | null;
  mode: "set" | "migrate";
};

export type ModStorageListenerStatus = "loading" | "ready" | "failed";

function isTaskStartedDto(value: unknown, kind: string): value is { taskId: string } {
  return (
    typeof value === "object" &&
    value !== null &&
    "taskId" in value &&
    typeof value.taskId === "string" &&
    value.taskId.trim() !== "" &&
    "kind" in value &&
    value.kind === kind
  );
}

/**
 * Shared state of the Mod storage directory (#275): the settings snapshot, the change flow
 * (pick → validate → confirm → set or migrate) and the migration task projection. It lives in
 * a provider above the router so the library page can read `writesFrozen` for its import /
 * delete entry points and a migration keeps reporting while the user is on another page.
 */
export function useModStorageSettingsState() {
  const { locale } = useI18n();
  const copy = useMemo(() => resolveCopy(modStorageCopy, locale), [locale]);
  const { dismissTaskNotice, pushToast, showTaskNotice } = useFeedback();

  const [loadState, setLoadState] = useState<ModStorageLoadState>({ status: "loading" });
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [pendingChange, setPendingChange] = useState<ModStoragePendingChange | null>(null);
  const [migration, setMigration] = useState<ModStorageMigrationTaskState>({ status: "idle" });
  const [listenerStatus, setListenerStatus] = useState<ModStorageListenerStatus>("loading");
  const [listenerAttempt, setListenerAttempt] = useState(0);
  const [cancelPending, setCancelPending] = useState(false);

  const mountedRef = useRef(false);
  const busyRef = useRef(false);
  const migrationRef = useRef(migration);
  const listenerStatusRef = useRef(listenerStatus);
  const startPendingRef = useRef(false);
  const pendingEventsRef = useRef(new Map<string, TaskProgressEventDto>());
  const noticeTaskIdRef = useRef<string | null>(null);
  const terminalNoticeKeysRef = useRef(new Set<string>());
  const cancelPendingRef = useRef(false);

  migrationRef.current = migration;
  listenerStatusRef.current = listenerStatus;

  const applySettings = useCallback((settings: unknown) => {
    if (!mountedRef.current) {
      return;
    }
    if (isModStorageSettingsDto(settings)) {
      setLoadState({ status: "ready", settings });
    } else {
      setLoadState({ status: "error", errorCode: "app_settings_unavailable" });
    }
  }, []);

  const reload = useCallback(() => {
    setLoadState({ status: "loading" });
    void getModStorageSettings()
      .then(applySettings)
      .catch((error: unknown) => {
        if (mountedRef.current) {
          setLoadState({
            status: "error",
            errorCode: modStorageErrorCodeFrom(error, "app_settings_unavailable"),
          });
        }
      });
  }, [applySettings]);

  // Refresh without dropping the current snapshot (used after a migration ends).
  const refreshQuietly = useCallback(() => {
    void getModStorageSettings()
      .then(applySettings)
      .catch(() => {
        // The last snapshot stays on screen; the next explicit reload reports the failure.
      });
  }, [applySettings]);

  useEffect(() => {
    mountedRef.current = true;
    reload();
    return () => {
      mountedRef.current = false;
    };
  }, [reload]);

  const applyProgressEvent = useCallback((event: TaskProgressEventDto) => {
    const current = migrationRef.current;
    const next = nextModStorageMigrationStateFromProgress(current, event);
    if (next !== current) {
      migrationRef.current = next;
      setMigration(next);
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, (event) => {
      if (disposed || event.payload.kind !== "mod_storage_migration") {
        return;
      }
      const current = migrationRef.current;
      if (!("taskId" in current)) {
        // The runner may emit before the start invoke resolves; keep the event for replay.
        if (startPendingRef.current) {
          pendingEventsRef.current.set(event.payload.taskId, event.payload);
        }
        return;
      }
      if (event.payload.taskId === current.taskId) {
        applyProgressEvent(event.payload);
      }
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
  }, [applyProgressEvent, listenerAttempt]);

  // Progress notice while the migration runs; terminal toasts once, then refresh the gate state.
  useEffect(() => {
    if (migration.status === "running" || migration.status === "cancelling") {
      const previous = noticeTaskIdRef.current;
      if (previous && previous !== migration.taskId) {
        dismissTaskNotice(previous);
      }
      noticeTaskIdRef.current = migration.taskId;
      const progress =
        migration.current !== null && migration.total !== null
          ? copy.migration.progress(String(migration.current), String(migration.total))
          : "";
      showTaskNotice({
        taskId: migration.taskId,
        title: copy.migration.title,
        message: `${getModStorageMigrationPhaseLabel(migration.phase, copy.migration)}${progress}`,
        tone: "progress",
      });
      return;
    }
    const previous = noticeTaskIdRef.current;
    if (previous) {
      dismissTaskNotice(previous);
      noticeTaskIdRef.current = null;
    }
    if (!isModStorageMigrationTerminal(migration)) {
      return;
    }
    const noticeKey = `${migration.status}.${migration.taskId ?? "no-task"}`;
    if (terminalNoticeKeysRef.current.has(noticeKey)) {
      return;
    }
    terminalNoticeKeysRef.current.add(noticeKey);
    refreshQuietly();
    if (migration.status === "completed") {
      pushToast({
        eventKey: `mod-storage.migration.completed.${noticeKey}`,
        taskId: migration.taskId,
        title: copy.migration.completedTitle,
        message: copy.migration.completedMessage,
        tone: "success",
      });
    } else if (migration.status === "cancelled") {
      pushToast({
        eventKey: `mod-storage.migration.cancelled.${noticeKey}`,
        taskId: migration.taskId,
        title: copy.migration.cancelledTitle,
        message: copy.migration.cancelledMessage,
        tone: "neutral",
      });
    } else {
      pushToast({
        eventKey: `mod-storage.migration.failed.${noticeKey}`,
        taskId: migration.taskId ?? undefined,
        title: copy.migration.failedTitle,
        message: getModStorageErrorMessage(migration.errorCode, locale),
        tone: "danger",
      });
    }
  }, [copy, dismissTaskNotice, locale, migration, pushToast, refreshQuietly, showTaskNotice]);

  useEffect(
    () => () => {
      const taskId = noticeTaskIdRef.current;
      if (taskId) {
        dismissTaskNotice(taskId);
      }
    },
    [dismissTaskNotice],
  );

  const settings = loadState.status === "ready" ? loadState.settings : null;

  const beginBusy = useCallback(() => {
    if (busyRef.current || isModStorageMigrationActive(migrationRef.current)) {
      return false;
    }
    busyRef.current = true;
    setBusy(true);
    setActionError(null);
    return true;
  }, []);

  const endBusy = useCallback(() => {
    busyRef.current = false;
    if (mountedRef.current) {
      setBusy(false);
    }
  }, []);

  const proposeChange = useCallback(
    (directory: string | null) => {
      if (settings === null) {
        return;
      }
      setPendingChange({ directory, mode: settings.libraryEmpty ? "set" : "migrate" });
    },
    [settings],
  );

  /** Picker → read-only validation → confirmation. Nothing is persisted here. */
  const chooseDirectory = useCallback(async () => {
    if (settings === null || !beginBusy()) {
      return;
    }
    try {
      let selected: unknown;
      try {
        selected = await open({ directory: true, multiple: false, title: copy.actions.pickerTitle });
      } catch {
        setActionError("mod_storage_picker_failed");
        return;
      }
      const directory = Array.isArray(selected) ? selected[0] : selected;
      if (typeof directory !== "string" || directory.trim() === "") {
        return;
      }
      const verdict = await validateModStorageDir(directory);
      if (!isModStorageDirValidationDto(verdict)) {
        setActionError("mod_storage_dir_unavailable");
        return;
      }
      if (!verdict.ok) {
        setActionError(verdict.code ?? "mod_storage_dir_unavailable");
        return;
      }
      proposeChange(directory);
    } catch (error) {
      setActionError(modStorageErrorCodeFrom(error, "mod_storage_dir_unavailable"));
    } finally {
      endBusy();
    }
  }, [beginBusy, copy, endBusy, proposeChange, settings]);

  const chooseDefault = useCallback(() => {
    if (settings === null || settings.configuredDir === null || busyRef.current) {
      return;
    }
    setActionError(null);
    proposeChange(null);
  }, [proposeChange, settings]);

  const dismissPendingChange = useCallback(() => {
    setPendingChange(null);
  }, []);

  const dismissActionError = useCallback(() => {
    setActionError(null);
  }, []);

  const launchMigration = useCallback(
    async (directory: string | null) => {
      if (listenerStatusRef.current !== "ready") {
        setActionError("mod_storage_migration_listener_unavailable");
        return;
      }
      startPendingRef.current = true;
      pendingEventsRef.current.clear();
      migrationRef.current = { status: "starting" };
      setMigration(migrationRef.current);
      try {
        const started = await startModStorageMigrationTask(directory);
        if (!isTaskStartedDto(started, "mod_storage_migration")) {
          throw { code: "mod_storage_migration_task_unavailable" };
        }
        const running: ModStorageMigrationTaskState = {
          status: "running",
          taskId: started.taskId,
          phase: MOD_STORAGE_MIGRATION_PHASES.queued,
          current: null,
          total: null,
        };
        migrationRef.current = running;
        setMigration(running);
        const buffered = pendingEventsRef.current.get(started.taskId);
        if (buffered) {
          applyProgressEvent(buffered);
        }
        refreshQuietly();
      } catch (error) {
        migrationRef.current = { status: "idle" };
        setMigration(migrationRef.current);
        setActionError(modStorageErrorCodeFrom(error, "mod_storage_migration_task_unavailable"));
      } finally {
        startPendingRef.current = false;
        pendingEventsRef.current.clear();
      }
    },
    [applyProgressEvent, refreshQuietly],
  );

  const confirmPendingChange = useCallback(async () => {
    const change = pendingChange;
    if (change === null || !beginBusy()) {
      return;
    }
    setPendingChange(null);
    try {
      if (change.mode === "set") {
        const updated = await setModStorageDir(change.directory);
        applySettings(updated);
      } else {
        await launchMigration(change.directory);
      }
    } catch (error) {
      setActionError(modStorageErrorCodeFrom(error, "app_settings_unavailable"));
    } finally {
      endBusy();
    }
  }, [applySettings, beginBusy, endBusy, launchMigration, pendingChange]);

  const cancelMigration = useCallback(async () => {
    const current = migrationRef.current;
    if (current.status !== "running" || cancelPendingRef.current) {
      return;
    }
    cancelPendingRef.current = true;
    setCancelPending(true);
    try {
      const cancelled = await cancelModStorageMigrationTask(current.taskId);
      if (!isTaskStartedDto(cancelled, "mod_storage_migration") || cancelled.taskId !== current.taskId) {
        throw { code: "mod_storage_migration_task_unavailable" };
      }
    } catch (error) {
      pushToast({
        eventKey: `mod-storage.migration.cancel-failed.${current.taskId}`,
        taskId: current.taskId,
        title: copy.migration.cancelFailedTitle,
        message: getModStorageErrorMessage(
          modStorageErrorCodeFrom(error, "mod_storage_migration_task_unavailable"),
          locale,
        ),
        tone: "warning",
      });
    } finally {
      cancelPendingRef.current = false;
      if (mountedRef.current) {
        setCancelPending(false);
      }
    }
  }, [copy, locale, pushToast]);

  const dismissMigrationResult = useCallback(() => {
    if (isModStorageMigrationTerminal(migrationRef.current)) {
      migrationRef.current = { status: "idle" };
      setMigration(migrationRef.current);
    }
  }, []);

  const retryListener = useCallback(() => {
    if (listenerStatusRef.current !== "failed" || isModStorageMigrationActive(migrationRef.current)) {
      return;
    }
    setListenerStatus("loading");
    setListenerAttempt((attempt) => attempt + 1);
  }, []);

  // The gate fact comes from the backend snapshot only; while it is unknown nothing is disabled
  // in the UI — the backend still refuses frozen writes with its stable codes.
  const writesFrozen: ModStorageWritesFrozen = settings?.writesFrozen ?? "none";

  return {
    loadState,
    settings,
    reload,
    writesFrozen,
    busy,
    actionError,
    dismissActionError,
    pendingChange,
    chooseDirectory,
    chooseDefault,
    confirmPendingChange,
    dismissPendingChange,
    migration,
    cancelMigration,
    cancelPending,
    dismissMigrationResult,
    listenerStatus,
    retryListener,
  };
}

export type ModStorageSettingsState = ReturnType<typeof useModStorageSettingsState>;
