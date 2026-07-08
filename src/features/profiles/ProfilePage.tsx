import {
  AlertTriangle,
  Archive,
  CheckCircle2,
  Database,
  History,
  Loader2,
  Plus,
  RefreshCw,
  Save,
  Search,
  Shield,
  ShieldAlert,
  ShieldCheck,
  ShieldOff,
  X,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "../mods/modImportTypes";
import { useActiveProfile } from "./ActiveProfileProvider";
import { BackupPolicyPanel } from "./BackupPolicyPanel";
import { ProfileListPanel } from "./ProfileListPanel";
import { ProfileSaveDirectoryCandidateList } from "./ProfileSaveDirectoryCandidateList";
import { useProfileSaveDirectoryDiscovery } from "./ProfileSaveDirectoryDiscoveryProvider";
import { SaveDirectoryPanel } from "./SaveDirectoryPanel";
import { listProfiles } from "./profileApi";
import {
  checkProfileAutoSaveBackup,
  getSaveBackupBackgroundStatus,
  listProfileSaveBackups,
  startProfileSaveBackup,
} from "./profileSaveBackupApi";
import {
  getProfileSaveBackupTaskPhaseLabel,
  isProfileSaveBackupTaskPhase,
  nextProfileSaveBackupTaskStateFromProgress,
  shouldRefreshProfileSaveBackupHistory,
  type ProfileSaveBackupTaskState,
} from "./profileSaveBackupTaskState";
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
const PREVIEW_PROFILES: Profile[] = [
  {
    id: "preview-default",
    name: "Default (主游戏配置)",
    description: "主要玩大剑的主存档",
    isActive: true,
    createdAt: 1719665600000,
    updatedAt: 1719765600000,
  },
  {
    id: "preview-taichi",
    name: "太刀毕业档",
    description: "独立存档，目前全武器毕业阶段",
    isActive: false,
    createdAt: 1717065600000,
    updatedAt: 1719565600000,
  },
  {
    id: "preview-online-test",
    name: "联机修改测试档",
    description: "用于 Mod 联机修改装备测试备份",
    isActive: false,
    createdAt: 1714465600000,
    updatedAt: 1719465600000,
  },
];
const PREVIEW_SAVE_SETTINGS: ProfileSaveSettingsDto = {
  profileId: "preview-default",
  saveDirectory: {
    mode: "custom",
    status: "valid",
    pathLabel: "Steam/userdata/<steam-id>/582010/remote",
    messages: ["已验证存档结构和读取权限"],
  },
  backupDirectory: {
    mode: "default",
    status: "defaulted",
    pathLabel: "HelsincyModManager/Backups/MHW",
    messages: ["将按配置档自动归档备份"],
  },
  schedule: {
    cadence: "daily",
    hour: 3,
    minute: 0,
    weekdays: [],
  },
  retention: {
    maxCount: 20,
    maxAgeDays: 30,
  },
  updatedAt: 0,
};
const PREVIEW_SAVE_SETTINGS_BY_PROFILE: Record<string, ProfileSaveSettingsDto> = {
  "preview-default": PREVIEW_SAVE_SETTINGS,
  "preview-taichi": {
    ...PREVIEW_SAVE_SETTINGS,
    profileId: "preview-taichi",
    schedule: { cadence: "weekly", hour: 2, minute: 30, weekdays: [1, 3, 5] },
    retention: { maxCount: 36, maxAgeDays: 60 },
    saveDirectory: {
      mode: "custom",
      status: "valid",
      pathLabel: "Steam/userdata/<steam-id>/582010/remote-taichi",
      messages: ["独立配置槽已关联存档源"],
    },
  },
  "preview-online-test": {
    ...PREVIEW_SAVE_SETTINGS,
    profileId: "preview-online-test",
    schedule: { cadence: "manual", hour: null, minute: null, weekdays: [] },
    retention: { maxCount: 12, maxAgeDays: 14 },
    saveDirectory: {
      mode: "unset",
      status: "unset",
      pathLabel: null,
      messages: ["等待关联游戏存档源目录"],
    },
  },
};

function isPlainBrowserRuntime() {
  return typeof window !== "undefined" && !("__TAURI_INTERNALS__" in window);
}

function createPreviewProfiles(): Profile[] {
  return PREVIEW_PROFILES.map((profile) => ({ ...profile }));
}

function createPreviewSaveSettings(profileId = PREVIEW_SAVE_SETTINGS.profileId): ProfileSaveSettingsDto {
  const template = PREVIEW_SAVE_SETTINGS_BY_PROFILE[profileId] ?? {
    ...PREVIEW_SAVE_SETTINGS,
    profileId,
  };

  return {
    ...template,
    profileId,
    saveDirectory: {
      ...template.saveDirectory,
      messages: [...template.saveDirectory.messages],
    },
    backupDirectory: {
      ...template.backupDirectory,
      messages: [...template.backupDirectory.messages],
    },
    schedule: {
      ...template.schedule,
      weekdays: [...template.schedule.weekdays],
    },
    retention: { ...template.retention },
  };
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

type ManualBackupNotice = {
  id: string;
  tone: "success" | "warning";
  title: string;
  message: string;
};

export function ProfilePage() {
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
  const [saveBackupTaskState, setSaveBackupTaskState] = useState<ProfileSaveBackupTaskState>({ status: "idle" });
  const saveBackupTaskStateRef = useRef<ProfileSaveBackupTaskState>({ status: "idle" });
  const pendingSaveBackupProgressEventsRef = useRef<Map<string, TaskProgressEventDto>>(new Map());
  const lastBackupHistoryRefreshTaskIdRef = useRef<string | null>(null);
  const [manualBackupNotice, setManualBackupNotice] = useState<ManualBackupNotice | null>(null);
  const [autoBackupCheckState, setAutoBackupCheckState] = useState<AutoBackupCheckState>({ status: "idle" });
  const [autoBackupCheckRefreshToken, setAutoBackupCheckRefreshToken] = useState(0);
  const [backgroundProtectionState, setBackgroundProtectionState] = useState<BackgroundProtectionState>({
    status: "loading",
  });
  const previewAutoBackupSettings =
    previewMode && settingsState.status === "ready" ? settingsState.settings : null;

  const attachStartedSaveBackupTask = useCallback((task: TaskStartedDto) => {
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
          setSettingsState({ status: "error", message: getErrorMessage(error, "存档设置不可用") });
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
        backups: createPreviewSaveBackups(selectedProfileId),
      });
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
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setBackupHistoryState((current) => ({
            status: "error",
            backups: current.backups,
            message: getErrorMessage(error, "备份历史不可用"),
          }));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [backupHistoryRefreshToken, previewMode, selectedProfileId]);

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
          attachStartedSaveBackupTask(result.startedTask);
        }
      } catch (error) {
        if (!cancelled) {
          setAutoBackupCheckState({ status: "error", message: getErrorMessage(error, "自动备份检查失败") });
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
        setBackupHistoryRefreshToken((current) => current + 1);
        setAutoBackupCheckRefreshToken((current) => current + 1);
        setManualBackupNotice({
          id: `save-backup-completed-${saveBackupTaskState.taskId}`,
          tone: "success",
          title: "存档备份完成",
          message: "新的备份历史点已经写入当前配置档。",
        });
      }
      return;
    }

    if (saveBackupTaskState.status === "failed") {
      setManualBackupNotice({
        id: `save-backup-failed-${saveBackupTaskState.taskId ?? "start"}`,
        tone: "warning",
        title: "存档备份失败",
        message: saveBackupTaskState.message,
      });
    }
  }, [saveBackupTaskState]);

  useEffect(() => {
    if (!manualBackupNotice) return undefined;

    const dismissTimer = window.setTimeout(() => setManualBackupNotice(null), 6000);
    return () => window.clearTimeout(dismissTimer);
  }, [manualBackupNotice]);

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
  });
  const canStartManualSaveBackup = manualBackupBlockedReason === null;
  const autoBackupCheckBlockedReason = getAutoBackupCheckBlockedReason(saveBackupTaskState);

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
      });
      setSettingsState({ status: "ready", settings });
      setDraftSettings(settings);
      setPendingDirectories({});
      setDirty(false);
    } catch (error) {
      setSaveError(getErrorMessage(error, "保存失败"));
    } finally {
      setSavingSettings(false);
    }
  };

  const startManualSaveBackup = useCallback(async () => {
    if (!selectedProfileId || !canStartManualSaveBackup) return;

    setSaveBackupTaskState({ status: "starting" });
    setManualBackupNotice(null);

    if (previewMode) {
      const taskId = `preview-save-backup-${Date.now()}`;
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
        note: selectedProfile ? `手动备份：${selectedProfile.name}` : null,
      });
      attachStartedSaveBackupTask(task);
    } catch (error) {
      setSaveBackupTaskState({
        status: "failed",
        taskId: null,
        phase: "save_backup.failed",
        message: getErrorMessage(error, "启动备份失败"),
      });
    }
  }, [attachStartedSaveBackupTask, canStartManualSaveBackup, previewMode, selectedProfile, selectedProfileId]);

  return (
    <section className="profile-page" data-preview-mode={previewMode ? "true" : undefined} aria-labelledby="profile-page-title">
      <ProfileManualBackupFloatingNotice
        notice={manualBackupNotice}
        onDismiss={() => setManualBackupNotice(null)}
      />
      <header className="profile-page__header">
        <div className="profile-page__title-block">
          <span className="profile-page__eyebrow">
            <ShieldCheck size={15} />
            Profile Workspace
          </span>
          <h1 id="profile-page-title">配置档控制台</h1>
          <p className="profile-page__subtitle">
            管理当前游戏实例的多套存档配置、目录映射与自动备份策略
          </p>
        </div>
        <div className="profile-page__actions header-status-deck" aria-label="配置档操作">
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
            同步刷新
          </button>
          <button
            type="button"
            className="profile-action-button"
            onClick={() => setCreateProfileRequestToken((current) => current + 1)}
          >
            <Plus size={14} />
            新建配置槽
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

        <main className="profile-settings-stack detail-column" aria-live="polite">
          {settingsState.status !== "ready" ? (
            <section className="profile-settings-panel glass-card profile-detail-console" aria-label="配置档详情与存档目录">
            {settingsState.status === "idle" ? (
              <div className="profile-settings-state" role="status">
                <span>选择配置档后显示存档设置</span>
              </div>
            ) : null}

            {settingsState.status === "loading" ? (
              <div className="profile-settings-state" role="status">
                <Loader2 className="profile-spinner" size={20} />
                <span>正在读取存档设置</span>
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
                  重试
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
                  />
                  <ActiveSavePanel profile={selectedProfile} settings={visibleSettings} />
                  <BackupPolicyPanel
                    settings={visibleSettings}
                    onScheduleChange={updateSchedule}
                    onRetentionChange={updateRetention}
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
                />
              </div>
            </>
          ) : null}
        </main>
      </div>
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
  const saveStatus = formatDirectoryStatus(settings.saveDirectory);
  const ready = settings.saveDirectory.status === "valid";

  return (
    <section className="profile-active-save-card" aria-labelledby="profile-active-save-title">
      <div className="profile-settings-panel__header">
        <div>
          <h2 id="profile-active-save-title">活动存档与自动策略</h2>
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
            <strong>{profile?.name ?? "未选择配置档"}</strong>
            <span>{ready ? saveStatus.label : "等待关联存档源目录"}</span>
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
  const statusCopy = getManualBackupStatusCopy(taskState, disabledReason);
  const running = taskState.status === "starting" || taskState.status === "running";

  return (
    <section className="profile-manual-backup-card" aria-labelledby="profile-manual-backup-title">
      <div className="profile-manual-backup-card__copy">
        <h2 id="profile-manual-backup-title">手动备份</h2>
        <p>立即为当前配置档创建一个受控存档归档点。</p>
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
        {running ? "备份中" : "立即归档当前存档"}
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
}: {
  settings: ProfileSaveSettingsDto;
  checkState: AutoBackupCheckState;
  backgroundState: BackgroundProtectionState;
  disabledReason: string | null;
  onCheck: () => void;
}) {
  const checking = checkState.status === "checking";
  const disabled = checking || disabledReason !== null;
  const statusCopy = getAutoBackupStatusCopy(checkState);
  const protectionCopy = getBackgroundProtectionCopy(backgroundState);
  const lastAutoCheck = "result" in checkState ? formatAutoBackupTimestamp(checkState.result.checkedAt) : "尚未检查";
  const nextDue = "result" in checkState ? formatAutoBackupTimestamp(checkState.result.nextDueAt) : "等待调度信息";

  return (
    <section className="profile-auto-backup-card" aria-labelledby="profile-auto-backup-title">
      <div className="profile-auto-backup-card__header">
        <div>
          <h2 id="profile-auto-backup-title">自动备份运行期</h2>
          <p>{formatBackupSchedule(settings.schedule)}</p>
        </div>
        <span className="profile-auto-backup-card__badge">仅在客户端运行时</span>
      </div>

      <div className={`profile-auto-backup-status is-${statusCopy.tone}`} role="status" aria-live="polite">
        {checking ? <Loader2 className="profile-spinner" size={16} /> : statusCopy.icon}
        <span>{statusCopy.label}</span>
      </div>

      <div className="profile-auto-backup-card__meta">
        <span>最近检查：{lastAutoCheck}</span>
        <span>下次计划：{nextDue}</span>
      </div>

      <div className={`profile-auto-backup-protection is-${protectionCopy.tone}`} role="status">
        {protectionCopy.icon}
        <div className="profile-auto-backup-protection__copy">
          <strong>{protectionCopy.label}</strong>
          <span>{protectionCopy.hint}</span>
        </div>
      </div>

      <button
        type="button"
        className="profile-action-button"
        disabled={disabled}
        onClick={onCheck}
      >
        {checking ? <Loader2 className="profile-spinner" size={14} /> : <RefreshCw size={14} />}
        {disabledReason ?? "立即检查"}
      </button>
    </section>
  );
}

function BackupHistoryPanel({
  profile,
  historyState,
  onRefresh,
}: {
  profile: Profile | null;
  historyState: BackupHistoryState;
  onRefresh: () => void;
}) {
  const rows = historyState.backups.map(toBackupHistoryRow);
  const countLabel = historyState.status === "loading" ? "刷新中" : `${rows.length} 个归档包`;

  return (
    <section className="profile-settings-panel glass-card history-card profile-history-card" aria-labelledby="profile-history-title">
      <div className="profile-settings-panel__header profile-history-header">
        <div>
          <h2 id="profile-history-title">备份历史点</h2>
          <span>{profile?.name ? `${profile.name} · ${countLabel}` : countLabel}</span>
        </div>
        <button type="button" className="profile-icon-button" aria-label="刷新备份历史" onClick={onRefresh}>
          {historyState.status === "loading" ? <Loader2 className="profile-spinner" size={16} /> : <History size={16} />}
        </button>
      </div>

      {historyState.status === "error" ? (
        <div className="profile-history-error" role="alert">
          <AlertTriangle size={16} />
          <span>{historyState.message}</span>
        </div>
      ) : null}

      <label className="profile-history-search search-row">
        <Search size={14} aria-hidden="true" />
        <span className="sr-only">筛选备份历史</span>
        <input type="search" placeholder="输入备份备注以筛选历史..." disabled />
      </label>

      {rows.length > 0 ? (
        <div className="profile-backup-table-wrapper">
          <table className="profile-backup-table">
            <thead>
              <tr>
                <th>存档点</th>
                <th>大小</th>
                <th>归档时间</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.id}>
                  <td>
                    <strong>{row.name}</strong>
                    <span>{row.detail}</span>
                  </td>
                  <td>{row.size}</td>
                  <td>{row.createdAt}</td>
                  <td>
                    <button type="button" className="profile-action-button is-primary" disabled>
                      恢复
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="profile-history-empty">
          <Archive size={24} aria-hidden="true" />
          <strong>暂无存档备份</strong>
          <span>完成首次归档后会在这里显示历史点。</span>
        </div>
      )}
    </section>
  );
}

function createPreviewSaveBackups(profileId: string | null): SaveBackupSummaryDto[] {
  if (profileId === "preview-online-test" || profileId === null) return [];

  const now = Date.now();
  const rows: SaveBackupSummaryDto[] = [
    {
      backupId: "preview-backup-fatalis",
      gameId: CURRENT_GAME_ID,
      profileId,
      trigger: "manual",
      status: "completed",
      fileName: "mhw-preview-default-20260707-150000.zip",
      createdAt: now - 60 * 60 * 1000,
      sizeBytes: 3_800_000,
      fileCount: 8,
      sourcePathLabel: "Steam/userdata/<steam-id>/582010/remote",
      notes: "讨伐黑龙前夕",
    },
    {
      backupId: "preview-backup-iceborne",
      gameId: CURRENT_GAME_ID,
      profileId,
      trigger: "manual",
      status: "completed",
      fileName: "mhw-preview-default-20260706-210000.zip",
      createdAt: now - 24 * 60 * 60 * 1000,
      sizeBytes: 3_600_000,
      fileCount: 8,
      sourcePathLabel: "Steam/userdata/<steam-id>/582010/remote",
      notes: "冰原通关节点",
    },
  ];

  if (profileId === "preview-taichi") {
    return [
      {
        backupId: "preview-backup-taichi",
        gameId: CURRENT_GAME_ID,
        profileId,
        trigger: "manual",
        status: "completed",
        fileName: "mhw-preview-taichi-20260707-030000.zip",
        createdAt: now - 12 * 60 * 60 * 1000,
        sizeBytes: 3_400_000,
        fileCount: 7,
        sourcePathLabel: "Steam/userdata/<steam-id>/582010/remote-taichi",
        notes: "迅龙速刷备份",
      },
    ];
  }

  return rows;
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
    startedTask: null,
  };

  return autoBackupCheckStateFromResult(result);
}

function autoBackupCheckStateFromResult(result: ProfileAutoSaveBackupCheckDto): AutoBackupCheckState {
  if (result.status === "manual_only") return { status: "manual", result };
  if (result.status === "due") return { status: "due", result };
  return { status: "notDue", result };
}

function toBackupHistoryRow(backup: SaveBackupSummaryDto) {
  return {
    id: backup.backupId,
    name: backup.notes?.trim() || backup.fileName,
    size: formatBytes(backup.sizeBytes),
    createdAt: formatRelativeTime(backup.createdAt),
    detail: `${formatBackupTrigger(backup.trigger)} · ${formatBackupStatus(backup.status)} · ${backup.fileCount} 个文件`,
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
  const tone = saveError ? "error" : dirty ? "dirty" : settingsEditable ? "synced" : "disabled";
  const label = saveError
    ? saveError
    : savingSettings
      ? "正在保存设置"
      : dirty
        ? "有未保存的更改"
        : settingsEditable
          ? "设置已同步"
          : "设置未就绪";
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
        {savingSettings ? "保存中" : "保存设置"}
      </button>
    </div>
  );
}

function ProfileManualBackupFloatingNotice({
  notice,
  onDismiss,
}: {
  notice: ManualBackupNotice | null;
  onDismiss: () => void;
}) {
  if (!notice) return null;

  return (
    <aside className={`profile-manual-backup-floating-notice is-${notice.tone}`} role="status" aria-live="polite">
      <div className="profile-manual-backup-floating-notice__copy">
        <strong>{notice.title}</strong>
        <span>{notice.message}</span>
      </div>
      <button
        type="button"
        className="profile-manual-backup-floating-notice__dismiss"
        aria-label="关闭备份提示"
        onClick={onDismiss}
      >
        <X size={16} aria-hidden="true" />
      </button>
    </aside>
  );
}

function getManualBackupBlockedReason({
  dirty,
  selectedProfileId,
  settingsState,
  taskState,
}: {
  dirty: boolean;
  selectedProfileId: string | null;
  settingsState: SaveSettingsState;
  taskState: ProfileSaveBackupTaskState;
}) {
  if (!selectedProfileId) return "请选择配置档";
  if (settingsState.status !== "ready") {
    return settingsState.status === "error" ? "存档设置不可用" : "读取存档设置后可备份";
  }
  if (settingsState.settings.saveDirectory.status !== "valid") return "请先关联有效存档目录";
  if (dirty) return "请先保存存档设置";
  if (taskState.status === "starting" || taskState.status === "running") return "备份任务正在执行";
  return null;
}

function getAutoBackupCheckBlockedReason(taskState: ProfileSaveBackupTaskState) {
  if (taskState.status === "starting" || taskState.status === "running") return "备份任务正在执行";
  return null;
}

function getManualBackupStatusCopy(taskState: ProfileSaveBackupTaskState, disabledReason: string | null) {
  if (taskState.status === "starting") {
    return {
      tone: "running",
      label: "正在启动备份任务",
      icon: null,
    };
  }

  if (taskState.status === "running") {
    return {
      tone: "running",
      label: getProfileSaveBackupTaskPhaseLabel(taskState.phase),
      icon: null,
    };
  }

  if (taskState.status === "completed") {
    return {
      tone: "success",
      label: "最近一次备份完成",
      icon: <CheckCircle2 size={16} aria-hidden="true" />,
    };
  }

  if (taskState.status === "failed") {
    return {
      tone: "warning",
      label: taskState.message,
      icon: <AlertTriangle size={16} aria-hidden="true" />,
    };
  }

  if (taskState.status === "cancelled") {
    return {
      tone: "warning",
      label: "备份任务已取消",
      icon: <AlertTriangle size={16} aria-hidden="true" />,
    };
  }

  return {
    tone: disabledReason ? "waiting" : "ready",
    label: disabledReason ?? "可以创建手动备份",
    icon: disabledReason ? <AlertTriangle size={16} aria-hidden="true" /> : <Archive size={16} aria-hidden="true" />,
  };
}

function getAutoBackupStatusCopy(checkState: AutoBackupCheckState) {
  if (checkState.status === "checking") {
    return {
      tone: "running",
      label: "正在检查自动备份计划",
      icon: null,
    };
  }

  if (checkState.status === "manual") {
    return {
      tone: "waiting",
      label: "当前配置为仅手动备份",
      icon: <AlertTriangle size={16} aria-hidden="true" />,
    };
  }

  if (checkState.status === "due") {
    return {
      tone: checkState.result.startedTask ? "running" : "warning",
      label: checkState.result.startedTask ? "自动备份已排队" : "自动备份计划已到期",
      icon: checkState.result.startedTask ? <Archive size={16} aria-hidden="true" /> : <AlertTriangle size={16} aria-hidden="true" />,
    };
  }

  if (checkState.status === "notDue") {
    return {
      tone: "success",
      label: "自动备份计划尚未到期",
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
    label: "等待自动备份检查",
    icon: <Archive size={16} aria-hidden="true" />,
  };
}

function getBackgroundProtectionCopy(state: BackgroundProtectionState) {
  if (state.status === "loading") {
    return {
      tone: "waiting",
      label: "正在读取后台保护状态",
      hint: "查询后台备份保障的最近记录",
      icon: <Shield size={16} aria-hidden="true" />,
    };
  }

  if (state.status === "unavailable") {
    return {
      tone: "warning",
      label: "后台保护状态不可用",
      hint: "暂时无法读取调度状态，自动备份仍按客户端计划执行",
      icon: <ShieldAlert size={16} aria-hidden="true" />,
    };
  }

  const { result } = state;
  const lastSuccess =
    result.lastSuccessAt !== null ? `上次成功备份：${formatAutoBackupTimestamp(result.lastSuccessAt)}` : null;

  switch (result.status) {
    case "protected":
      return {
        tone: "success",
        label: "已受后台保护",
        hint: lastSuccess ?? "退出主客户端后仍会继续检查备份计划",
        icon: <ShieldCheck size={16} aria-hidden="true" />,
      };
    case "tray_only":
      return {
        tone: "waiting",
        label: "仅客户端运行期保护",
        hint: lastSuccess ?? "退出主客户端后自动备份暂不受后台保障",
        icon: <Shield size={16} aria-hidden="true" />,
      };
    case "registration_failed":
      return {
        tone: "warning",
        label: "后台保护注册失败",
        hint: "计划任务或自启动注册失败，退出客户端后不会自动备份",
        icon: <ShieldAlert size={16} aria-hidden="true" />,
      };
    case "worker_unhealthy":
      return {
        tone: "warning",
        label: "后台保护异常",
        hint: "后台守护最近没有心跳，请重新检查备份计划",
        icon: <ShieldAlert size={16} aria-hidden="true" />,
      };
    case "permission_required":
      return {
        tone: "warning",
        label: "需要系统权限",
        hint: "当前环境需要额外权限才能启用后台保护",
        icon: <ShieldAlert size={16} aria-hidden="true" />,
      };
    case "unsupported_platform":
      return {
        tone: "waiting",
        label: "当前平台暂不支持后台保护",
        hint: "自动备份仅在客户端运行时执行",
        icon: <ShieldOff size={16} aria-hidden="true" />,
      };
    case "not_enabled":
    default:
      return {
        tone: "waiting",
        label: "未启用后台保护",
        hint: "自动备份仅在客户端运行时执行",
        icon: <ShieldOff size={16} aria-hidden="true" />,
      };
  }
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

function formatAutoBackupTimestamp(timestamp: number | null) {
  if (!timestamp) return "暂无";
  if (timestamp > Date.now()) {
    return new Date(timestamp).toLocaleString("zh-CN", {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  return formatRelativeTime(timestamp);
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

function formatRelativeTime(timestamp: number) {
  const diffMs = Date.now() - timestamp;
  if (!Number.isFinite(diffMs) || diffMs < 0) return "刚刚";
  const minutes = Math.floor(diffMs / 60_000);
  if (minutes < 1) return "刚刚";
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days} 天前`;
  return new Date(timestamp).toLocaleDateString("zh-CN");
}

function formatBackupTrigger(trigger: SaveBackupSummaryDto["trigger"]) {
  if (trigger === "auto") return "自动备份";
  if (trigger === "pre_install") return "安装前备份";
  return "手动备份";
}

function formatBackupStatus(status: SaveBackupSummaryDto["status"]) {
  if (status === "deleted_by_retention") return "已按保留策略清理";
  if (status === "missing") return "文件缺失";
  if (status === "invalid") return "需要检查";
  return "已完成";
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
