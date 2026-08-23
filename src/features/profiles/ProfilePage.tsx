import {
  AlertTriangle,
  Archive,
  ArchiveRestore,
  CheckCircle2,
  Database,
  History,
  Loader2,
  Plus,
  RefreshCw,
  Save,
  Search,
  Settings2,
  Shield,
  ShieldAlert,
  ShieldCheck,
  ShieldOff,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useAppRoute } from "../../app/routing/useAppRoute";
import { useFeedback, type FeedbackToastInput } from "../../shared/feedback";
import { localeMeta, resolveCopy, useI18n } from "../../shared/i18n";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "../mods/modImportTypes";
import { useActiveProfile } from "./ActiveProfileProvider";
import { BackupPolicyPanel } from "./BackupPolicyPanel";
import { ProfileListPanel } from "./ProfileListPanel";
import { ProfileSaveDirectoryCandidateList } from "./ProfileSaveDirectoryCandidateList";
import { useProfileSaveDirectoryDiscovery } from "./ProfileSaveDirectoryDiscoveryProvider";
import { SaveDirectoryPanel } from "./SaveDirectoryPanel";
import { SaveRestoreDialog } from "./SaveRestoreDialog";
import { listProfiles } from "./profileApi";
import { backupPolicyCopy } from "./backupPolicyCopy";
import { profilePageCopy, type ProfilePageCopy } from "./profilePageCopy";
import { saveBackupCopy, type SaveBackupCopy } from "./saveBackupCopy";
import { saveDirectoryCopy } from "./saveDirectoryCopy";
import {
  createPreviewProfiles,
  createPreviewSaveBackups,
  createPreviewSaveSettings,
  PREVIEW_SAVE_SETTINGS,
} from "./profilesPreviewData";
import {
  checkProfileAutoSaveBackup,
  getSaveBackupBackgroundStatus,
  listProfileSaveBackups,
  startProfileSaveBackup,
} from "./profileSaveBackupApi";
import {
  getProfileSaveBackupTaskPhaseLabel,
  getProfileSaveBackupTaskErrorCode,
  getProfileSaveBackupTaskErrorMessage,
  isProfileSaveBackupTaskPhase,
  nextProfileSaveBackupTaskStateFromProgress,
  shouldRefreshProfileSaveBackupHistory,
  type ProfileSaveBackupTaskState,
} from "./profileSaveBackupTaskState";
import type { Locale } from "../../shared/i18n";
import type {
  ProfileAutoSaveBackupCheckDto,
  SaveBackupBackgroundStatusDto,
  SaveBackupSummaryDto,
  TaskStartedDto,
} from "./profileSaveBackupTypes";
import {
  getProfileSaveSettings,
  setProfileSaveSettings,
} from "./profileSaveSettingsApi";
import type {
  ProfileBackupRetentionDto,
  ProfileBackupScheduleDto,
  ProfileSaveSettingsDto,
} from "./profileSaveSettingsTypes";
import type { Profile } from "./profileTypes";
import { formatBackupSchedule, formatDirectoryStatus } from "./profileViewModel";

const CURRENT_GAME_ID = "mhw";

function isPlainBrowserRuntime() {
  return typeof window !== "undefined" && !("__TAURI_INTERNALS__" in window);
}

type ProfileListState =
  | { status: "loading"; profiles: Profile[] }
  | { status: "ready"; profiles: Profile[] }
  | { status: "error"; profiles: Profile[] };

type SaveSettingsState =
  | { status: "idle" | "loading" }
  | { status: "ready"; settings: ProfileSaveSettingsDto }
  | { status: "error"; message: string };

type BackupHistoryState =
  | { status: "idle"; backups: SaveBackupSummaryDto[] }
  | { status: "loading"; backups: SaveBackupSummaryDto[] }
  | { status: "ready"; backups: SaveBackupSummaryDto[] }
  | { status: "error"; backups: SaveBackupSummaryDto[]; message: string };

type AutoBackupCheckState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "manual"; result: ProfileAutoSaveBackupCheckDto }
  | { status: "notDue"; result: ProfileAutoSaveBackupCheckDto }
  | { status: "due"; result: ProfileAutoSaveBackupCheckDto }
  | { status: "error"; message: string };

type BackgroundProtectionState =
  | { status: "loading" }
  | { status: "ready"; result: SaveBackupBackgroundStatusDto }
  | { status: "unavailable" };

export function ProfilePage() {
  const { navigate } = useAppRoute();
  const { pushToast } = useFeedback();
  const { locale } = useI18n();
  const pageCopy = resolveCopy(profilePageCopy, locale);
  const backupCopy = resolveCopy(saveBackupCopy, locale);
  // 数据加载 effect 与终态 toast effect 经 ref 取词：语言切换既不能触发重新拉取，
  // 也不能让已消费的终态重复推送 toast。
  const pageCopyRef = useRef(pageCopy);
  pageCopyRef.current = pageCopy;
  const backupCopyRef = useRef(backupCopy);
  backupCopyRef.current = backupCopy;
  const { activeProfile, refreshActiveProfile, setActiveProfile } = useActiveProfile();
  const { latestDiscovery, isDiscovering, discoveringTarget, runDiscovery } = useProfileSaveDirectoryDiscovery();
  const previewMode = isPlainBrowserRuntime();
  const [profileState, setProfileState] = useState<ProfileListState>({
    status: "loading",
    profiles: [],
  });
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [settingsState, setSettingsState] = useState<SaveSettingsState>({ status: "idle" });
  const [draftSettings, setDraftSettings] = useState<ProfileSaveSettingsDto>(() =>
    createPreviewSaveSettings(),
  );
  const [refreshToken, setRefreshToken] = useState(0);
  const [settingsRefreshToken, setSettingsRefreshToken] = useState(0);
  const [busyProfileId, setBusyProfileId] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [savingSettings, setSavingSettings] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [createProfileRequestToken, setCreateProfileRequestToken] = useState(0);
  const [pendingDirectories, setPendingDirectories] = useState<{
    saveDirectory?: string;
    backupDirectory?: string;
  }>({});
  const [backupHistoryState, setBackupHistoryState] = useState<BackupHistoryState>({
    status: "idle",
    backups: [],
  });
  const [backupHistoryRefreshToken, setBackupHistoryRefreshToken] = useState(0);
  const [restoreBackup, setRestoreBackup] = useState<SaveBackupSummaryDto | null>(null);
  const [saveBackupTaskState, setSaveBackupTaskState] = useState<ProfileSaveBackupTaskState>({ status: "idle" });
  const saveBackupTaskStateRef = useRef<ProfileSaveBackupTaskState>({ status: "idle" });
  const pendingSaveBackupProgressEventsRef = useRef<Map<string, TaskProgressEventDto>>(new Map());
  const saveBackupTaskProfileIdsRef = useRef<Map<string, string>>(new Map());
  const lastBackupHistoryRefreshTaskIdRef = useRef<string | null>(null);
  const pendingBackupCompletionToastRef = useRef<{ taskId: string; profileId: string } | null>(null);
  const [autoBackupCheckState, setAutoBackupCheckState] = useState<AutoBackupCheckState>({ status: "idle" });
  const [autoBackupCheckRefreshToken, setAutoBackupCheckRefreshToken] = useState(0);
  const [backgroundProtectionState, setBackgroundProtectionState] = useState<BackgroundProtectionState>({
    status: "loading",
  });
  const previewAutoBackupSettings =
    previewMode && settingsState.status === "ready" ? settingsState.settings : null;

  const attachStartedSaveBackupTask = useCallback((task: TaskStartedDto, profileId: string) => {
    saveBackupTaskProfileIdsRef.current.set(task.taskId, profileId);
    const initialTaskState: ProfileSaveBackupTaskState = {
      status: "running",
      taskId: task.taskId,
      phase: "save_backup.queued",
    };
    const pendingEvent = pendingSaveBackupProgressEventsRef.current.get(task.taskId);
    pendingSaveBackupProgressEventsRef.current.delete(task.taskId);
    setSaveBackupTaskState(
      pendingEvent
        ? nextProfileSaveBackupTaskStateFromProgress(initialTaskState, pendingEvent)
        : initialTaskState,
    );
  }, []);

  const refreshProfiles = useCallback(() => {
    setRefreshToken((current) => current + 1);
  }, []);

  useEffect(() => {
    let cancelled = false;
    setProfileState((current) => ({ status: "loading", profiles: current.profiles }));

    void listProfiles()
      .then((profiles) => {
        if (cancelled) return;
        setProfileState({ status: "ready", profiles });
        setSelectedProfileId((current) => {
          if (current && profiles.some((profile) => profile.id === current)) return current;
          const active = profiles.find((profile) => profile.isActive);
          return active?.id ?? profiles[0]?.id ?? null;
        });
      })
      .catch(() => {
        if (cancelled) return;
        if (previewMode) {
          const previewProfiles = createPreviewProfiles();
          setProfileState({ status: "ready", profiles: previewProfiles });
          setSelectedProfileId((current) => {
            if (current && previewProfiles.some((profile) => profile.id === current)) return current;
            return previewProfiles.find((profile) => profile.isActive)?.id ?? previewProfiles[0]?.id ?? null;
          });
          return;
        }
        if (!cancelled) {
          setProfileState((current) => ({ status: "error", profiles: current.profiles }));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [previewMode, refreshToken]);

  useEffect(() => {
    if (selectedProfileId) return;
    if (activeProfile.status === "ready") {
      setSelectedProfileId(activeProfile.profile.id);
    }
  }, [activeProfile, selectedProfileId]);

  useEffect(() => {
    if (!selectedProfileId) {
      setSettingsState({ status: "idle" });
      setDraftSettings(createPreviewSaveSettings());
      setDirty(false);
      setSaveError(null);
      setPendingDirectories({});
      return;
    }

    let cancelled = false;
    setSettingsState({ status: "loading" });
    setDraftSettings(createPreviewSaveSettings(selectedProfileId));
    setDirty(false);
    setSaveError(null);
    setPendingDirectories({});

    void getProfileSaveSettings({ gameId: CURRENT_GAME_ID, profileId: selectedProfileId })
      .then((settings) => {
        if (!cancelled) {
          setSettingsState({ status: "ready", settings });
          setDraftSettings(settings);
        }
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        if (previewMode) {
          const settings = createPreviewSaveSettings(selectedProfileId);
          setSettingsState({ status: "ready", settings });
          setDraftSettings(settings);
          return;
        }
        if (!cancelled) {
          setSettingsState({ status: "error", message: getErrorMessage(error, pageCopyRef.current.settingsStates.unavailableFallback) });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [previewMode, selectedProfileId, settingsRefreshToken]);

  useEffect(() => {
    if (latestDiscovery?.outcome !== "auto_saved") return;
    if (selectedProfileId && latestDiscovery.profileId !== selectedProfileId) return;

    setSettingsRefreshToken((current) => current + 1);
  }, [latestDiscovery, selectedProfileId]);

  useEffect(() => {
    saveBackupTaskStateRef.current = saveBackupTaskState;
  }, [saveBackupTaskState]);

  useEffect(() => {
    setSaveBackupTaskState({ status: "idle" });
    pendingSaveBackupProgressEventsRef.current.clear();
    lastBackupHistoryRefreshTaskIdRef.current = null;
  }, [selectedProfileId]);

  useEffect(() => {
    if (!selectedProfileId) {
      setBackupHistoryState({ status: "idle", backups: [] });
      return;
    }

    let cancelled = false;
    setBackupHistoryState((current) => ({ status: "loading", backups: current.backups }));

    if (previewMode) {
      setBackupHistoryState({
        status: "ready",
        backups: createPreviewSaveBackups(CURRENT_GAME_ID, selectedProfileId),
      });
      publishPendingBackupCompletionToast(
        pendingBackupCompletionToastRef,
        saveBackupTaskProfileIdsRef,
        selectedProfileId,
        pushToast,
        pageCopyRef.current.toasts,
      );
      return;
    }

    void listProfileSaveBackups({
      gameId: CURRENT_GAME_ID,
      profileId: selectedProfileId,
      limit: 12,
    })
      .then((backups) => {
        if (!cancelled) {
          setBackupHistoryState({ status: "ready", backups });
          publishPendingBackupCompletionToast(
            pendingBackupCompletionToastRef,
            saveBackupTaskProfileIdsRef,
            selectedProfileId,
            pushToast,
            pageCopyRef.current.toasts,
          );
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setBackupHistoryState((current) => ({
            status: "error",
            backups: current.backups,
            message: getErrorMessage(error, pageCopyRef.current.history.unavailableFallback),
          }));
          const pending = pendingBackupCompletionToastRef.current;
          if (pending?.profileId === selectedProfileId) {
            pendingBackupCompletionToastRef.current = null;
            saveBackupTaskProfileIdsRef.current.delete(pending.taskId);
            pushToast({
              eventKey: `profile.save-backup.refresh-failed.${pending.taskId}`,
              taskId: pending.taskId,
              title: pageCopyRef.current.toasts.refreshFailedTitle,
              message: pageCopyRef.current.toasts.refreshFailedMessage,
              tone: "warning",
            });
          }
        }
      });

    return () => {
      cancelled = true;
    };
  }, [backupHistoryRefreshToken, previewMode, pushToast, selectedProfileId]);

  useEffect(() => {
    if (!selectedProfileId || settingsState.status !== "ready") {
      setAutoBackupCheckState({ status: "idle" });
      return;
    }

    let cancelled = false;
    setAutoBackupCheckState({ status: "checking" });

    if (previewMode) {
      if (!previewAutoBackupSettings) {
        setAutoBackupCheckState({ status: "idle" });
        return;
      }
      setAutoBackupCheckState(createPreviewAutoBackupCheckState(CURRENT_GAME_ID, selectedProfileId, previewAutoBackupSettings));
      return;
    }

    void (async () => {
      try {
        const result = await checkProfileAutoSaveBackup({
          gameId: CURRENT_GAME_ID,
          profileId: selectedProfileId,
        });
        if (cancelled) return;
        setAutoBackupCheckState(autoBackupCheckStateFromResult(result));
        if (result.startedTask) {
          attachStartedSaveBackupTask(result.startedTask, selectedProfileId);
        }
      } catch (error) {
        if (!cancelled) {
          setAutoBackupCheckState({ status: "error", message: getErrorMessage(error, pageCopyRef.current.autoBackup.checkFailedFallback) });
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [
    attachStartedSaveBackupTask,
    autoBackupCheckRefreshToken,
    previewAutoBackupSettings,
    previewMode,
    selectedProfileId,
    settingsState.status,
  ]);

  useEffect(() => {
    if (!selectedProfileId || settingsState.status !== "ready") {
      setBackgroundProtectionState({ status: "loading" });
      return;
    }

    if (previewMode) {
      if (!previewAutoBackupSettings) {
        setBackgroundProtectionState({ status: "loading" });
        return;
      }
      setBackgroundProtectionState({
        status: "ready",
        result: createPreviewBackgroundStatus(CURRENT_GAME_ID, selectedProfileId, previewAutoBackupSettings),
      });
      return;
    }

    let cancelled = false;
    setBackgroundProtectionState({ status: "loading" });

    void getSaveBackupBackgroundStatus({
      gameId: CURRENT_GAME_ID,
      profileId: selectedProfileId,
    })
      .then((result) => {
        if (!cancelled) setBackgroundProtectionState({ status: "ready", result });
      })
      .catch(() => {
        // 状态查询失败静默降级，不阻塞 Profile 页其它内容。
        if (!cancelled) setBackgroundProtectionState({ status: "unavailable" });
      });

    return () => {
      cancelled = true;
    };
  }, [
    autoBackupCheckRefreshToken,
    previewAutoBackupSettings,
    previewMode,
    selectedProfileId,
    settingsState.status,
  ]);

  useEffect(() => {
    if (previewMode) return undefined;

    let disposed = false;
    let unlistenTaskProgress: (() => void) | null = null;

    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, (event) => {
      if (disposed) return;
      if (event.payload.kind !== "save_backup") return;
      if (!isProfileSaveBackupTaskPhase(event.payload.phase)) return;

      const currentTaskState = saveBackupTaskStateRef.current;
      if (!("taskId" in currentTaskState) || currentTaskState.taskId === null) {
        if (currentTaskState.status === "starting") {
          pendingSaveBackupProgressEventsRef.current.set(event.payload.taskId, event.payload);
        }
        return;
      }

      if (event.payload.taskId !== currentTaskState.taskId) return;

      setSaveBackupTaskState((current) => nextProfileSaveBackupTaskStateFromProgress(current, event.payload));
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
  }, [previewMode]);

  useEffect(() => {
    if (shouldRefreshProfileSaveBackupHistory(saveBackupTaskState)) {
      if (lastBackupHistoryRefreshTaskIdRef.current !== saveBackupTaskState.taskId) {
        lastBackupHistoryRefreshTaskIdRef.current = saveBackupTaskState.taskId;
        const taskProfileId = saveBackupTaskProfileIdsRef.current.get(saveBackupTaskState.taskId);
        if (taskProfileId) {
          pendingBackupCompletionToastRef.current = {
            taskId: saveBackupTaskState.taskId,
            profileId: taskProfileId,
          };
          if (selectedProfileId === taskProfileId) {
            setBackupHistoryRefreshToken((current) => current + 1);
            setAutoBackupCheckRefreshToken((current) => current + 1);
          }
        }
      }
      return;
    }

    if (saveBackupTaskState.status === "failed") {
      if (saveBackupTaskState.taskId) saveBackupTaskProfileIdsRef.current.delete(saveBackupTaskState.taskId);
      const admissionBusy = saveBackupTaskState.errorCode === "write_admission_busy";
      pushToast({
        eventKey: `profile.save-backup.failed.${saveBackupTaskState.taskId ?? "start"}`,
        taskId: saveBackupTaskState.taskId ?? undefined,
        tone: admissionBusy ? "warning" : "danger",
        title: admissionBusy ? pageCopyRef.current.toasts.admissionBusyTitle : pageCopyRef.current.toasts.failedTitle,
        message: getProfileSaveBackupTaskErrorMessage(saveBackupTaskState.errorCode, backupCopyRef.current.errors),
      });
    }
  }, [pushToast, saveBackupTaskState, selectedProfileId]);

  const profiles = profileState.profiles;
  const selectedProfile = useMemo(
    () => profiles.find((profile) => profile.id === selectedProfileId) ?? null,
    [profiles, selectedProfileId],
  );
  const visibleSettings = draftSettings;
  const settingsEditable = settingsState.status === "ready" && selectedProfileId !== null;
  const manualBackupBlockedReason = getManualBackupBlockedReason({
    dirty,
    selectedProfileId,
    settingsState,
    taskState: saveBackupTaskState,
    reasons: pageCopy.blockedReasons,
  });
  const canStartManualSaveBackup = manualBackupBlockedReason === null;
  const autoBackupCheckBlockedReason = getAutoBackupCheckBlockedReason(saveBackupTaskState, pageCopy.blockedReasons);
  const saveRestoreBlockedReason = getSaveRestoreBlockedReason({
    dirty,
    savingSettings,
    selectedProfileId,
    settingsState,
    reasons: pageCopy.blockedReasons,
  });

  const updateSettings = (settings: ProfileSaveSettingsDto) => {
    setDraftSettings(settings);
    if (settingsEditable) {
      setSettingsState({ status: "ready", settings });
      setDirty(true);
      setSaveError(null);
    }
  };

  const updateSchedule = (schedule: ProfileBackupScheduleDto) => {
    updateSettings({ ...draftSettings, schedule });
  };

  const updateRetention = (retention: ProfileBackupRetentionDto) => {
    updateSettings({ ...draftSettings, retention });
  };

  const handleActivateProfile = async (profileId: string) => {
    setBusyProfileId(profileId);
    if (previewMode) {
      setProfileState((current) => ({
        status: "ready",
        profiles: (current.profiles.length > 0 ? current.profiles : createPreviewProfiles()).map((profile) => ({
          ...profile,
          isActive: profile.id === profileId,
        })),
      }));
      setSelectedProfileId(profileId);
      setBusyProfileId(null);
      return;
    }
    try {
      await setActiveProfile(profileId);
      setSelectedProfileId(profileId);
      refreshProfiles();
    } finally {
      setBusyProfileId(null);
    }
  };

  const saveSettings = async () => {
    if (settingsState.status !== "ready" || !selectedProfileId) return;
    setSavingSettings(true);
    setSaveError(null);
    if (previewMode) {
      const settings = { ...draftSettings, updatedAt: Date.now() };
      setSettingsState({ status: "ready", settings });
      setDraftSettings(settings);
      setPendingDirectories({});
      setDirty(false);
      setSavingSettings(false);
      return;
    }
    try {
      const settings = await setProfileSaveSettings({
        gameId: CURRENT_GAME_ID,
        profileId: selectedProfileId,
        saveDirectory: pendingDirectories.saveDirectory,
        backupDirectory: pendingDirectories.backupDirectory,
        schedule: draftSettings.schedule,
        retention: draftSettings.retention,
        preRestoreBackupEnabled: draftSettings.preRestoreBackupEnabled,
      });
      setSettingsState({ status: "ready", settings });
      setDraftSettings(settings);
      setPendingDirectories({});
      setDirty(false);
    } catch (error) {
      setSaveError(getErrorMessage(error, pageCopy.settingsStates.saveFailedFallback));
    } finally {
      setSavingSettings(false);
    }
  };

  const startManualSaveBackup = useCallback(async () => {
    if (!selectedProfileId || !canStartManualSaveBackup) return;

    setSaveBackupTaskState({ status: "starting" });
    if (previewMode) {
      const taskId = `preview-save-backup-${Date.now()}`;
      saveBackupTaskProfileIdsRef.current.set(taskId, selectedProfileId);
      setSaveBackupTaskState({
        status: "completed",
        taskId,
        phase: "save_backup.completed",
        resultRef: `preview-backup-${taskId}`,
      });
      return;
    }

    try {
      const task = await startProfileSaveBackup({
        gameId: CURRENT_GAME_ID,
        profileId: selectedProfileId,
        note: selectedProfile ? pageCopy.manualBackup.noteTemplate(selectedProfile.name) : null,
      });
      attachStartedSaveBackupTask(task, selectedProfileId);
    } catch (error) {
      const errorCode = getProfileSaveBackupTaskErrorCode(error);
      setSaveBackupTaskState({
        status: "failed",
        taskId: null,
        phase: "save_backup.failed",
        errorCode,
      });
    }
  }, [attachStartedSaveBackupTask, canStartManualSaveBackup, pageCopy, previewMode, selectedProfile, selectedProfileId]);

  return (
    <section className="profile-page" data-preview-mode={previewMode ? "true" : undefined} aria-labelledby="profile-page-title">
      <header className="profile-page__header">
        <div className="profile-page__title-block">
          <span className="profile-page__eyebrow">
            <ShieldCheck size={15} />
            Profile Workspace
          </span>
          <h1 id="profile-page-title">{pageCopy.header.title}</h1>
          <p className="profile-page__subtitle">
            {pageCopy.header.subtitle}
          </p>
        </div>
        <div className="profile-page__actions header-status-deck" aria-label={pageCopy.header.actionsAria}>
          <ProfileHeaderSaveAction
            dirty={dirty}
            saveError={saveError}
            savingSettings={savingSettings}
            settingsEditable={settingsEditable}
            onSave={() => void saveSettings()}
          />
          <button
            type="button"
            className="profile-action-button"
            onClick={() => {
              refreshProfiles();
              setSettingsRefreshToken((current) => current + 1);
            }}
          >
            <RefreshCw size={14} />
            {pageCopy.header.syncRefresh}
          </button>
          <button
            type="button"
            className="profile-action-button"
            onClick={() => setCreateProfileRequestToken((current) => current + 1)}
          >
            <Plus size={14} />
            {pageCopy.header.createSlot}
          </button>
        </div>
      </header>

      <div className="profile-workspace">
        <ProfileListPanel
          profiles={profiles}
          status={profileState.status}
          selectedProfileId={selectedProfileId}
          busyProfileId={busyProfileId}
          onRefresh={refreshProfiles}
          createRequestToken={createProfileRequestToken}
          onSelectProfile={setSelectedProfileId}
          onActivateProfile={handleActivateProfile}
          onProfilesChanged={() => {
            refreshProfiles();
            refreshActiveProfile();
          }}
        />

        <main
          className="profile-settings-stack detail-column"
          aria-live="polite"
          data-tour-id="profiles.settings"
        >
          {settingsState.status !== "ready" ? (
            <section className="profile-settings-panel glass-card profile-detail-console" aria-label={pageCopy.settingsStates.detailAria}>
            {settingsState.status === "idle" ? (
              <div className="profile-settings-state" role="status">
                <span>{pageCopy.settingsStates.idle}</span>
              </div>
            ) : null}

            {settingsState.status === "loading" ? (
              <div className="profile-settings-state" role="status">
                <Loader2 className="profile-spinner" size={20} />
                <span>{pageCopy.settingsStates.loading}</span>
              </div>
            ) : null}

            {settingsState.status === "error" ? (
              <div className="profile-settings-state is-error" role="alert">
                <AlertTriangle size={20} />
                <span>{settingsState.message}</span>
                <button
                  type="button"
                  className="profile-action-button"
                  onClick={() => setSettingsRefreshToken((current) => current + 1)}
                >
                  {pageCopy.settingsStates.retry}
                </button>
              </div>
            ) : null}
            </section>
          ) : null}

          {settingsState.status === "ready" ? (
            <>
              <div className="profile-save-manager-deck save-manager-deck">
                <div className="profile-save-strategy-stack profile-settings-panel glass-card strategy-card">
                  <ManualSaveBackupPanel
                    taskState={saveBackupTaskState}
                    disabledReason={manualBackupBlockedReason}
                    canStartManualSaveBackup={canStartManualSaveBackup}
                    onClick={() => void startManualSaveBackup()}
                  />
                  <AutoSaveBackupRuntimePanel
                    settings={visibleSettings}
                    checkState={autoBackupCheckState}
                    backgroundState={backgroundProtectionState}
                    disabledReason={autoBackupCheckBlockedReason}
                    onCheck={() => setAutoBackupCheckRefreshToken((current) => current + 1)}
                    onOpenSettings={() => navigate("/settings")}
                  />
                  <ActiveSavePanel profile={selectedProfile} settings={visibleSettings} />
                  <BackupPolicyPanel
                    settings={visibleSettings}
                    onScheduleChange={updateSchedule}
                    onRetentionChange={updateRetention}
                    onPreRestoreBackupEnabledChange={(enabled) =>
                      updateSettings({ ...draftSettings, preRestoreBackupEnabled: enabled })
                    }
                  />
                </div>
                <div className="profile-directory-zone">
                  <SaveDirectoryPanel
                    gameId={CURRENT_GAME_ID}
                    profileId={selectedProfileId ?? PREVIEW_SAVE_SETTINGS.profileId}
                    settings={visibleSettings}
                    previewMode={previewMode}
                    disabled={!settingsEditable}
                    autoDetecting={isDiscovering && discoveringTarget?.profileId === selectedProfileId}
                    hasDiscoveryCandidates={
                      latestDiscovery?.outcome === "confirmation_required" &&
                      latestDiscovery.profileId === selectedProfileId
                    }
                    onAutoDetect={() => {
                      if (!selectedProfileId) return;
                      void runDiscovery({
                        gameId: CURRENT_GAME_ID,
                        profileId: selectedProfileId,
                        reason: "manual",
                      });
                    }}
                    onSettingsChange={updateSettings}
                    onDirectorySelected={(kind, directory) =>
                      setPendingDirectories((current) => ({ ...current, [kind]: directory }))
                    }
                  />
                  <ProfileSaveDirectoryCandidateList />
                </div>
                <BackupHistoryPanel
                  profile={selectedProfile}
                  historyState={backupHistoryState}
                  onRefresh={() => setBackupHistoryRefreshToken((current) => current + 1)}
                  onRestore={setRestoreBackup}
                  restoreBlockedReason={saveRestoreBlockedReason}
                />
              </div>
            </>
          ) : null}
        </main>
      </div>
      <SaveRestoreDialog
        backup={restoreBackup}
        profileId={selectedProfileId}
        previewMode={previewMode}
        onClose={() => setRestoreBackup(null)}
        onCompleted={() => {
          setBackupHistoryRefreshToken((current) => current + 1);
        }}
      />
    </section>
  );
}

function ActiveSavePanel({
  profile,
  settings,
}: {
  profile: Profile | null;
  settings: ProfileSaveSettingsDto;
}) {
  const { locale } = useI18n();
  const copy = resolveCopy(profilePageCopy, locale).activeSave;
  const statusLabels = resolveCopy(saveDirectoryCopy, locale).directoryStatus;
  const saveStatus = formatDirectoryStatus(settings.saveDirectory, statusLabels);
  const ready = settings.saveDirectory.status === "valid";

  return (
    <section className="profile-active-save-card" aria-labelledby="profile-active-save-title">
      <div className="profile-settings-panel__header">
        <div>
          <h2 id="profile-active-save-title">{copy.title}</h2>
          <span>Active save channel</span>
        </div>
        <Database size={18} aria-hidden="true" />
      </div>

      <div className="profile-active-save-body">
        <div className={`active-save-banner ${ready ? "is-ready" : "is-waiting"}`}>
          <span className="active-save-pulse" aria-hidden="true" />
          <span className="save-avatar-box" aria-hidden="true">
            <Database size={18} />
          </span>
          <div className="active-save-copy">
            <strong>{profile?.name ?? copy.noProfile}</strong>
            <span>{ready ? saveStatus.label : copy.waitingDirectory}</span>
          </div>
        </div>
      </div>
    </section>
  );
}

function ManualSaveBackupPanel({
  taskState,
  disabledReason,
  canStartManualSaveBackup,
  onClick,
}: {
  taskState: ProfileSaveBackupTaskState;
  disabledReason: string | null;
  canStartManualSaveBackup: boolean;
  onClick: () => void;
}) {
  const { locale } = useI18n();
  const copy = resolveCopy(profilePageCopy, locale).manualBackup;
  const backupCopy = resolveCopy(saveBackupCopy, locale);
  const statusCopy = getManualBackupStatusCopy(taskState, disabledReason, copy, backupCopy);
  const running = taskState.status === "starting" || taskState.status === "running";

  return (
    <section
      className="profile-manual-backup-card"
      aria-labelledby="profile-manual-backup-title"
      data-tour-id="profiles.manual-backup"
    >
      <div className="profile-manual-backup-card__copy">
        <h2 id="profile-manual-backup-title">{copy.title}</h2>
        <p>{copy.hint}</p>
      </div>
      <div className={`profile-manual-backup-status is-${statusCopy.tone}`} role="status" aria-live="polite">
        {running ? <Loader2 className="profile-spinner" size={16} /> : statusCopy.icon}
        <span>{statusCopy.label}</span>
      </div>
      <button
        type="button"
        className="profile-action-button is-primary profile-create-backup-button"
        disabled={!canStartManualSaveBackup}
        onClick={onClick}
      >
        <Save size={14} />
        {running ? copy.runningButton : copy.startButton}
      </button>
    </section>
  );
}

function AutoSaveBackupRuntimePanel({
  settings,
  checkState,
  backgroundState,
  disabledReason,
  onCheck,
  onOpenSettings,
}: {
  settings: ProfileSaveSettingsDto;
  checkState: AutoBackupCheckState;
  backgroundState: BackgroundProtectionState;
  disabledReason: string | null;
  onCheck: () => void;
  onOpenSettings: () => void;
}) {
  const { locale } = useI18n();
  const copy = resolveCopy(profilePageCopy, locale);
  const scheduleCopy = resolveCopy(backupPolicyCopy, locale).schedule;
  const checking = checkState.status === "checking";
  const disabled = checking || disabledReason !== null;
  const statusCopy = getAutoBackupStatusCopy(checkState, copy.autoBackup);
  const protectionCopy = getBackgroundProtectionCopy(settings.schedule.cadence, backgroundState, copy.background, locale);
  const protectionBadge = getBackgroundProtectionBadge(settings.schedule.cadence, backgroundState, copy.background);
  const showSettingsLink = shouldOfferBackgroundSettingsNavigation(
    settings.schedule.cadence,
    backgroundState,
  );
  const lastAutoCheck = "result" in checkState ? formatAutoBackupTimestamp(checkState.result.checkedAt, copy.time, locale) : copy.autoBackup.neverChecked;
  const nextDue = "result" in checkState ? formatAutoBackupTimestamp(checkState.result.nextDueAt, copy.time, locale) : copy.autoBackup.waitingSchedule;

  return (
    <section
      className="profile-auto-backup-card"
      aria-labelledby="profile-auto-backup-title"
      data-tour-id="profiles.auto-backup"
    >
      <div className="profile-auto-backup-card__header">
        <div>
          <h2 id="profile-auto-backup-title">{copy.autoBackup.title}</h2>
          <p>{formatBackupSchedule(settings.schedule, scheduleCopy)}</p>
        </div>
        <span className={`profile-auto-backup-card__badge is-${protectionBadge.tone}`}>
          {protectionBadge.label}
        </span>
      </div>

      <div className={`profile-auto-backup-status is-${statusCopy.tone}`} role="status" aria-live="polite">
        {checking ? <Loader2 className="profile-spinner" size={16} /> : statusCopy.icon}
        <span>{statusCopy.label}</span>
      </div>

      <div className="profile-auto-backup-card__meta">
        <span>{copy.autoBackup.lastCheck(lastAutoCheck)}</span>
        <span>{copy.autoBackup.nextDue(nextDue)}</span>
      </div>

      <div className={`profile-auto-backup-protection is-${protectionCopy.tone}`} role="status">
        {protectionCopy.icon}
        <div className="profile-auto-backup-protection__copy">
          <strong>{protectionCopy.label}</strong>
          <span>{protectionCopy.hint}</span>
        </div>
      </div>

      {showSettingsLink ? (
        <button
          type="button"
          className="profile-action-button profile-background-settings-link"
          onClick={onOpenSettings}
        >
          <Settings2 size={14} aria-hidden="true" />
          {copy.autoBackup.goToSettings}
        </button>
      ) : null}

      <button
        type="button"
        className="profile-action-button"
        disabled={disabled}
        onClick={onCheck}
      >
        {checking ? <Loader2 className="profile-spinner" size={14} /> : <RefreshCw size={14} />}
        {disabledReason ?? copy.autoBackup.checkNow}
      </button>
    </section>
  );
}

function BackupHistoryPanel({
  profile,
  historyState,
  onRefresh,
  onRestore,
  restoreBlockedReason,
}: {
  profile: Profile | null;
  historyState: BackupHistoryState;
  onRefresh: () => void;
  onRestore: (backup: SaveBackupSummaryDto) => void;
  restoreBlockedReason: string | null;
}) {
  const { locale } = useI18n();
  const copy = resolveCopy(profilePageCopy, locale);
  const rows = historyState.backups.map((backup) => ({ backup, ...toBackupHistoryRow(backup, copy, locale) }));
  const countLabel = historyState.status === "loading" ? copy.history.refreshing : copy.history.count(rows.length);

  return (
    <section
      className="profile-settings-panel glass-card history-card profile-history-card"
      aria-labelledby="profile-history-title"
      data-tour-id="profiles.backup-history"
    >
      <div className="profile-settings-panel__header profile-history-header">
        <div>
          <h2 id="profile-history-title">{copy.history.title}</h2>
          <span>{profile?.name ? `${profile.name} · ${countLabel}` : countLabel}</span>
        </div>
        <button type="button" className="profile-icon-button" aria-label={copy.history.refreshAria} onClick={onRefresh}>
          {historyState.status === "loading" ? <Loader2 className="profile-spinner" size={16} /> : <History size={16} />}
        </button>
      </div>

      {historyState.status === "error" ? (
        <div className="profile-history-error" role="alert">
          <AlertTriangle size={16} />
          <span>{historyState.message}</span>
        </div>
      ) : null}

      {restoreBlockedReason ? (
        <div className="profile-history-restore-blocked" role="status">
          <AlertTriangle size={16} />
          <span>{copy.history.restoreBlocked(restoreBlockedReason)}</span>
        </div>
      ) : null}

      <label className="profile-history-search search-row">
        <Search size={14} aria-hidden="true" />
        <span className="sr-only">{copy.history.filterSr}</span>
        <input type="search" placeholder={copy.history.filterPlaceholder} disabled />
      </label>

      {rows.length > 0 ? (
        <div className="profile-backup-list" role="list" aria-label={copy.history.listAria}>
          {rows.map((row) => (
            <article className="profile-backup-item" key={row.id} role="listitem">
              <div className="profile-backup-item__summary">
                <strong title={row.name}>{row.name}</strong>
                <span>{row.detail}</span>
              </div>

              <dl className="profile-backup-item__meta">
                <div>
                  <dt>{copy.history.metaSize}</dt>
                  <dd>{row.size}</dd>
                </div>
                <div>
                  <dt>{copy.history.metaCreatedAt}</dt>
                  <dd>{row.createdAt}</dd>
                </div>
              </dl>

              <div className="profile-backup-item__actions">
                <button
                  type="button"
                  className="profile-action-button is-primary profile-backup-restore-button"
                  aria-label={copy.history.restoreAria(row.name)}
                  title={row.backup.status !== "completed"
                    ? copy.history.notCompletedTitle
                    : restoreBlockedReason ?? copy.history.restoreTitle}
                  disabled={row.backup.status !== "completed" || restoreBlockedReason !== null}
                  onClick={() => onRestore(row.backup)}
                >
                  <ArchiveRestore size={15} aria-hidden="true" />
                  {copy.history.restore}
                </button>
              </div>
            </article>
          ))}
        </div>
      ) : (
        <div className="profile-history-empty">
          <Archive size={24} aria-hidden="true" />
          <strong>{copy.history.emptyTitle}</strong>
          <span>{copy.history.emptyHint}</span>
        </div>
      )}
    </section>
  );
}

function createPreviewAutoBackupCheckState(
  gameId: string,
  profileId: string,
  settings: ProfileSaveSettingsDto,
): AutoBackupCheckState {
  const now = Date.now();
  const result: ProfileAutoSaveBackupCheckDto = {
    gameId,
    profileId,
    clientRuntimeOnly: true,
    status: settings.schedule.cadence === "manual" ? "manual_only" : "not_due",
    checkedAt: now,
    lastDueAt: settings.schedule.cadence === "manual" ? null : now - 60 * 60 * 1000,
    nextDueAt: settings.schedule.cadence === "manual" ? null : now + 2 * 60 * 60 * 1000,
    lastAutoBackupAt: null,
    pendingReason: null,
    startedTask: null,
  };

  return autoBackupCheckStateFromResult(result);
}

function autoBackupCheckStateFromResult(result: ProfileAutoSaveBackupCheckDto): AutoBackupCheckState {
  if (result.status === "manual_only") return { status: "manual", result };
  if (result.status === "due") return { status: "due", result };
  return { status: "notDue", result };
}

function toBackupHistoryRow(backup: SaveBackupSummaryDto, copy: ProfilePageCopy, locale: Locale) {
  return {
    id: backup.backupId,
    name: backup.notes?.trim() || backup.fileName,
    size: formatBytes(backup.sizeBytes),
    createdAt: formatRelativeTime(backup.createdAt, copy.time, locale),
    detail: `${copy.trigger[backup.trigger]} · ${copy.backupStatus[backup.status]} · ${copy.history.fileCount(backup.fileCount)}`,
  };
}

function ProfileHeaderSaveAction({
  dirty,
  saveError,
  savingSettings,
  settingsEditable,
  onSave,
}: {
  dirty: boolean;
  saveError: string | null;
  savingSettings: boolean;
  settingsEditable: boolean;
  onSave: () => void;
}) {
  const { locale } = useI18n();
  const copy = resolveCopy(profilePageCopy, locale).saveAction;
  const tone = saveError ? "error" : dirty ? "dirty" : settingsEditable ? "synced" : "disabled";
  const label = saveError
    ? saveError
    : savingSettings
      ? copy.saving
      : dirty
        ? copy.dirty
        : settingsEditable
          ? copy.synced
          : copy.notReady;
  const icon = savingSettings ? (
    <Loader2 className="profile-spinner" size={15} aria-hidden="true" />
  ) : saveError || dirty ? (
    <AlertTriangle size={15} aria-hidden="true" />
  ) : (
    <CheckCircle2 size={15} aria-hidden="true" />
  );

  return (
    <div className={`profile-header-save-action is-${tone}`} role={saveError ? "alert" : "status"} aria-live="polite">
      <span className="profile-header-save-action__status">
        {icon}
        <span>{label}</span>
      </span>
      <button
        type="button"
        className={`profile-action-button profile-header-save-action__button ${dirty ? "is-primary is-pulse-glow" : ""}`}
        onClick={onSave}
        disabled={!settingsEditable || !dirty || savingSettings}
      >
        <Save size={14} />
        {savingSettings ? copy.savingButton : copy.saveButton}
      </button>
    </div>
  );
}

function getManualBackupBlockedReason({
  dirty,
  selectedProfileId,
  settingsState,
  taskState,
  reasons,
}: {
  dirty: boolean;
  selectedProfileId: string | null;
  settingsState: SaveSettingsState;
  taskState: ProfileSaveBackupTaskState;
  reasons: ProfilePageCopy["blockedReasons"];
}) {
  if (!selectedProfileId) return reasons.selectProfile;
  if (settingsState.status !== "ready") {
    return settingsState.status === "error" ? reasons.settingsUnavailable : reasons.settingsLoadingBackup;
  }
  if (settingsState.settings.saveDirectory.status !== "valid") return reasons.linkValidDirectory;
  if (dirty) return reasons.saveSettingsFirst;
  if (taskState.status === "starting" || taskState.status === "running") return reasons.backupTaskRunning;
  return null;
}

function getSaveRestoreBlockedReason({
  dirty,
  savingSettings,
  selectedProfileId,
  settingsState,
  reasons,
}: {
  dirty: boolean;
  savingSettings: boolean;
  selectedProfileId: string | null;
  settingsState: SaveSettingsState;
  reasons: ProfilePageCopy["blockedReasons"];
}) {
  if (!selectedProfileId) return reasons.selectProfile;
  if (savingSettings) return reasons.savingSettings;
  if (settingsState.status !== "ready") {
    return settingsState.status === "error" ? reasons.settingsUnavailable : reasons.settingsLoadingRestore;
  }
  if (dirty) return reasons.saveSettingsFirst;
  if (settingsState.settings.saveDirectory.status !== "valid") return reasons.linkValidDirectory;
  return null;
}

function publishPendingBackupCompletionToast(
  pendingRef: React.MutableRefObject<{ taskId: string; profileId: string } | null>,
  taskProfileIdsRef: React.MutableRefObject<Map<string, string>>,
  profileId: string,
  pushToast: (input: FeedbackToastInput) => void,
  toasts: ProfilePageCopy["toasts"],
) {
  const pending = pendingRef.current;
  if (!pending || pending.profileId !== profileId) return;

  pendingRef.current = null;
  taskProfileIdsRef.current.delete(pending.taskId);
  pushToast({
    eventKey: `profile.save-backup.completed.${pending.taskId}`,
    taskId: pending.taskId,
    tone: "success",
    title: toasts.completedTitle,
    message: toasts.completedMessage,
  });
}

function getAutoBackupCheckBlockedReason(
  taskState: ProfileSaveBackupTaskState,
  reasons: ProfilePageCopy["blockedReasons"],
) {
  if (taskState.status === "starting" || taskState.status === "running") return reasons.backupTaskRunning;
  return null;
}

function getManualBackupStatusCopy(
  taskState: ProfileSaveBackupTaskState,
  disabledReason: string | null,
  copy: ProfilePageCopy["manualBackup"],
  backupCopy: SaveBackupCopy,
) {
  if (taskState.status === "starting") {
    return {
      tone: "running",
      label: copy.starting,
      icon: null,
    };
  }

  if (taskState.status === "running") {
    return {
      tone: "running",
      label: getProfileSaveBackupTaskPhaseLabel(taskState.phase, backupCopy.phases),
      icon: null,
    };
  }

  if (taskState.status === "completed") {
    return {
      tone: "success",
      label: copy.lastCompleted,
      icon: <CheckCircle2 size={16} aria-hidden="true" />,
    };
  }

  if (taskState.status === "failed") {
    return {
      tone: "warning",
      label: getProfileSaveBackupTaskErrorMessage(taskState.errorCode, backupCopy.errors),
      icon: <AlertTriangle size={16} aria-hidden="true" />,
    };
  }

  if (taskState.status === "cancelled") {
    return {
      tone: "warning",
      label: copy.cancelled,
      icon: <AlertTriangle size={16} aria-hidden="true" />,
    };
  }

  return {
    tone: disabledReason ? "waiting" : "ready",
    label: disabledReason ?? copy.ready,
    icon: disabledReason ? <AlertTriangle size={16} aria-hidden="true" /> : <Archive size={16} aria-hidden="true" />,
  };
}

function getAutoBackupStatusCopy(checkState: AutoBackupCheckState, copy: ProfilePageCopy["autoBackup"]) {
  if (checkState.status === "checking") {
    return {
      tone: "running",
      label: copy.checking,
      icon: null,
    };
  }

  if (checkState.status === "manual") {
    return {
      tone: "waiting",
      label: copy.manualOnly,
      icon: <AlertTriangle size={16} aria-hidden="true" />,
    };
  }

  if (checkState.status === "due") {
    if (checkState.result.pendingReason === "game_running") {
      return {
        tone: "waiting",
        label: copy.deferredGameRunning,
        icon: <AlertTriangle size={16} aria-hidden="true" />,
      };
    }
    if (checkState.result.pendingReason === "game_running_unknown") {
      return {
        tone: "waiting",
        label: copy.deferredGameUnknown,
        icon: <AlertTriangle size={16} aria-hidden="true" />,
      };
    }
    return {
      tone: checkState.result.startedTask ? "running" : "warning",
      label: checkState.result.startedTask ? copy.queued : copy.due,
      icon: checkState.result.startedTask ? <Archive size={16} aria-hidden="true" /> : <AlertTriangle size={16} aria-hidden="true" />,
    };
  }

  if (checkState.status === "notDue") {
    return {
      tone: "success",
      label: copy.notDue,
      icon: <CheckCircle2 size={16} aria-hidden="true" />,
    };
  }

  if (checkState.status === "error") {
    return {
      tone: "warning",
      label: checkState.message,
      icon: <AlertTriangle size={16} aria-hidden="true" />,
    };
  }

  return {
    tone: "waiting",
    label: copy.waiting,
    icon: <Archive size={16} aria-hidden="true" />,
  };
}

function getBackgroundProtectionBadge(
  cadence: ProfileBackupScheduleDto["cadence"],
  state: BackgroundProtectionState,
  copy: ProfilePageCopy["background"],
) {
  if (cadence === "manual") {
    return { tone: "manual", label: copy.badgeManual };
  }

  if (state.status === "ready" && state.result.status === "protected") {
    return { tone: "protected", label: copy.badgeProtected };
  }

  if (state.status === "ready" && state.result.status === "starting") {
    return { tone: "starting", label: copy.badgeStarting };
  }

  return { tone: "client-only", label: copy.badgeClientOnly };
}

function getBackgroundProtectionCopy(
  cadence: ProfileBackupScheduleDto["cadence"],
  state: BackgroundProtectionState,
  copy: ProfilePageCopy["background"],
  locale: Locale,
) {
  if (cadence === "manual") {
    return {
      tone: "waiting",
      label: copy.manualLabel,
      hint: copy.manualHint,
      icon: <ShieldOff size={16} aria-hidden="true" />,
    };
  }

  if (state.status === "loading") {
    return {
      tone: "waiting",
      label: copy.loadingLabel,
      hint: copy.loadingHint,
      icon: <Shield size={16} aria-hidden="true" />,
    };
  }

  if (state.status === "unavailable") {
    return {
      tone: "warning",
      label: copy.unavailableLabel,
      hint: copy.unavailableHint,
      icon: <ShieldAlert size={16} aria-hidden="true" />,
    };
  }

  const { result } = state;
  const timeCopy = resolveCopy(profilePageCopy, locale).time;
  const lastSuccess =
    result.lastSuccessAt !== null ? copy.lastSuccess(formatAutoBackupTimestamp(result.lastSuccessAt, timeCopy, locale)) : null;

  switch (result.status) {
    case "protected":
      return {
        tone: "success",
        label: copy.protectedLabel,
        hint: lastSuccess ?? copy.protectedHint,
        icon: <ShieldCheck size={16} aria-hidden="true" />,
      };
    case "starting":
      return {
        tone: "waiting",
        label: copy.startingLabel,
        hint: copy.startingHint,
        icon: <Shield size={16} aria-hidden="true" />,
      };
    case "tray_only":
      return {
        tone: "waiting",
        label: copy.trayOnlyLabel,
        hint: lastSuccess ?? copy.trayOnlyHint,
        icon: <Shield size={16} aria-hidden="true" />,
      };
    case "registration_failed":
      return {
        tone: "warning",
        label: copy.registrationFailedLabel,
        hint: copy.registrationFailedHint,
        icon: <ShieldAlert size={16} aria-hidden="true" />,
      };
    case "worker_unhealthy":
      return {
        tone: "warning",
        label: copy.workerUnhealthyLabel,
        hint: copy.workerUnhealthyHint,
        icon: <ShieldAlert size={16} aria-hidden="true" />,
      };
    case "permission_required":
      return {
        tone: "warning",
        label: copy.permissionRequiredLabel,
        hint: copy.permissionRequiredHint,
        icon: <ShieldAlert size={16} aria-hidden="true" />,
      };
    case "unsupported_platform":
      return {
        tone: "waiting",
        label: copy.unsupportedLabel,
        hint: copy.unsupportedHint,
        icon: <ShieldOff size={16} aria-hidden="true" />,
      };
    case "not_enabled":
    default:
      return {
        tone: "waiting",
        label: copy.notEnabledLabel,
        hint: copy.notEnabledHint,
        icon: <ShieldOff size={16} aria-hidden="true" />,
      };
  }
}

function shouldOfferBackgroundSettingsNavigation(
  cadence: ProfileBackupScheduleDto["cadence"],
  state: BackgroundProtectionState,
) {
  if (cadence === "manual" || state.status !== "ready") return false;

  return (
    state.result.status === "registration_failed" ||
    state.result.status === "worker_unhealthy" ||
    state.result.status === "permission_required"
  );
}

function createPreviewBackgroundStatus(
  gameId: string,
  profileId: string,
  settings: ProfileSaveSettingsDto,
): SaveBackupBackgroundStatusDto {
  const autoEnabled = settings.schedule.cadence !== "manual";
  return {
    gameId,
    profileId,
    status: autoEnabled ? "tray_only" : "not_enabled",
    backgroundProtectionEnabled: false,
    lastCheckedAt: autoEnabled ? Date.now() - 5 * 60_000 : null,
    lastAttemptAt: null,
    lastSuccessAt: autoEnabled ? Date.now() - 6 * 60 * 60_000 : null,
    nextDueAt: autoEnabled ? Date.now() + 18 * 60 * 60_000 : null,
    pendingReason: null,
    lastErrorCode: null,
  };
}

function formatAutoBackupTimestamp(timestamp: number | null, timeCopy: ProfilePageCopy["time"], locale: Locale) {
  if (!timestamp) return timeCopy.none;
  if (timestamp > Date.now()) {
    return new Date(timestamp).toLocaleString(localeMeta[locale].bcp47, {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  return formatRelativeTime(timestamp, timeCopy, locale);
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value.toFixed(unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

function formatRelativeTime(timestamp: number, timeCopy: ProfilePageCopy["time"], locale: Locale) {
  const diffMs = Date.now() - timestamp;
  if (!Number.isFinite(diffMs) || diffMs < 0) return timeCopy.justNow;
  const minutes = Math.floor(diffMs / 60_000);
  if (minutes < 1) return timeCopy.justNow;
  if (minutes < 60) return timeCopy.minutesAgo(minutes);
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return timeCopy.hoursAgo(hours);
  const days = Math.floor(hours / 24);
  if (days < 7) return timeCopy.daysAgo(days);
  return new Date(timestamp).toLocaleDateString(localeMeta[locale].bcp47);
}

function getErrorMessage(error: unknown, fallback: string) {
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = String((error as { message?: unknown }).message ?? "").trim();
    if (message) return message;
  }
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  return fallback;
}
