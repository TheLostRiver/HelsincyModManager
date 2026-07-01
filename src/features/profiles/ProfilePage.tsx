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
  ShieldCheck,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useActiveProfile } from "./ActiveProfileProvider";
import { BackupPolicyPanel } from "./BackupPolicyPanel";
import { ProfileListPanel } from "./ProfileListPanel";
import { SaveDirectoryPanel } from "./SaveDirectoryPanel";
import { listProfiles } from "./profileApi";
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
import {
  formatBackupSchedule,
  formatDirectoryStatus,
} from "./profileViewModel";

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

export function ProfilePage() {
  const { activeProfile, refreshActiveProfile, setActiveProfile } = useActiveProfile();
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

  const profiles = profileState.profiles;
  const selectedProfile = useMemo(
    () => profiles.find((profile) => profile.id === selectedProfileId) ?? null,
    [profiles, selectedProfileId],
  );
  const visibleSettings = draftSettings;
  const settingsEditable = settingsState.status === "ready" && selectedProfileId !== null;

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

  return (
    <section className="profile-page" data-preview-mode={previewMode ? "true" : undefined} aria-labelledby="profile-page-title">
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
        <div className="profile-page__summary header-status-deck" aria-label="配置档操作">
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
            className="profile-action-button is-primary"
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
          <section className="profile-settings-panel glass-card profile-detail-console" aria-label="配置档详情与存档目录">
            <ProfileOverview
              profile={selectedProfile}
              settings={settingsState.status === "ready" ? settingsState.settings : null}
              dirty={dirty}
              saveError={saveError}
              savingSettings={savingSettings}
              settingsEditable={settingsEditable}
              onSave={() => void saveSettings()}
            />

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

            {settingsState.status === "ready" ? (
              <SaveDirectoryPanel
                gameId={CURRENT_GAME_ID}
                profileId={selectedProfileId ?? PREVIEW_SAVE_SETTINGS.profileId}
                settings={visibleSettings}
                previewMode={previewMode}
                disabled={!settingsEditable}
                onSettingsChange={updateSettings}
                onDirectorySelected={(kind, directory) =>
                  setPendingDirectories((current) => ({ ...current, [kind]: directory }))
                }
              />
            ) : null}
          </section>

          {settingsState.status === "ready" ? (
            <>
              <div className="profile-save-manager-deck save-manager-deck">
                <div className="profile-save-strategy-stack profile-settings-panel glass-card strategy-card">
                  <ActiveSavePanel profile={selectedProfile} settings={visibleSettings} />
                  <BackupPolicyPanel
                    settings={visibleSettings}
                    onScheduleChange={updateSchedule}
                    onRetentionChange={updateRetention}
                  />
                  <button type="button" className="profile-action-button is-primary profile-create-backup-button" disabled>
                    <Save size={14} />
                    立即归档当前存档
                  </button>
                </div>
                <BackupHistoryPanel profile={selectedProfile} previewMode={previewMode} />
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
  const scheduleLabel = formatBackupSchedule(settings.schedule);
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

        <div className="profile-policy-flags" aria-label="配置档策略摘要">
          <PolicyFlag label="存档沙盒隔离" value={ready ? "已就绪" : "待配置"} active={ready} />
          <PolicyFlag label="安装 Mod 前备份" value="建议开启" active />
          <PolicyFlag label="自动归档计划" value={scheduleLabel} active={settings.schedule.cadence !== "manual"} />
        </div>
      </div>
    </section>
  );
}

function PolicyFlag({ label, value, active }: { label: string; value: string; active: boolean }) {
  return (
    <div className="profile-policy-flag">
      <div>
        <strong>{label}</strong>
        <span>{value}</span>
      </div>
      <span className={`profile-policy-switch ${active ? "is-on" : ""}`} aria-hidden="true" />
    </div>
  );
}

function BackupHistoryPanel({
  profile,
  previewMode,
}: {
  profile: Profile | null;
  previewMode: boolean;
}) {
  const rows = getBackupHistoryRows(profile?.id ?? null, previewMode);

  return (
    <section className="profile-settings-panel glass-card history-card profile-history-card" aria-labelledby="profile-history-title">
      <div className="profile-settings-panel__header profile-history-header">
        <div>
          <h2 id="profile-history-title">备份历史点</h2>
          <span>{rows.length} 个归档包</span>
        </div>
        <History size={18} aria-hidden="true" />
      </div>

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
                    <span>{row.hash}</span>
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

function getBackupHistoryRows(profileId: string | null, previewMode: boolean) {
  if (!previewMode || profileId === "preview-online-test" || profileId === null) return [];

  const rows = [
    {
      id: "preview-backup-fatalis",
      name: "讨伐黑龙前夕",
      size: "3.8 MB",
      createdAt: "1 小时前",
      hash: "Hash 已校验",
    },
    {
      id: "preview-backup-iceborne",
      name: "冰原通关节点",
      size: "3.6 MB",
      createdAt: "昨天",
      hash: "Hash 已校验",
    },
  ];

  if (profileId === "preview-taichi") {
    return [
      {
        id: "preview-backup-taichi",
        name: "迅龙速刷备份",
        size: "3.4 MB",
        createdAt: "12 小时前",
        hash: "Hash 已校验",
      },
    ];
  }

  return rows;
}

function ProfileOverview({
  profile,
  settings,
  dirty,
  saveError,
  savingSettings,
  settingsEditable,
  onSave,
}: {
  profile: Profile | null;
  settings: ProfileSaveSettingsDto | null;
  dirty: boolean;
  saveError: string | null;
  savingSettings: boolean;
  settingsEditable: boolean;
  onSave: () => void;
}) {
  const saveStatus = settings ? formatDirectoryStatus(settings.saveDirectory) : null;
  const backupStatus = settings ? formatDirectoryStatus(settings.backupDirectory) : null;

  return (
    <div className="profile-overview profile-info-deck" aria-label="当前配置档">
      <div className="profile-overview__identity">
        <span className="profile-overview__mark" aria-hidden="true">
          <CheckCircle2 size={20} />
        </span>
        <div>
          <h2>{profile?.name ?? "未选择配置档"}</h2>
          <p>{profile?.description || profile?.id || "选择一个配置档开始配置"}</p>
        </div>
      </div>
      <div className="profile-overview__right">
        <div className="profile-overview__facts">
          <ProfileMetric label="存档目录" value={saveStatus?.label ?? "-"} />
          <ProfileMetric label="备份目录" value={backupStatus?.label ?? "-"} />
          <ProfileMetric label="自动备份" value={settings ? formatBackupSchedule(settings.schedule) : "-"} />
        </div>
        {settingsEditable ? (
          <div className="profile-save-bar profile-toolbar-save-box">
            {saveError ? (
              <span className="profile-save-bar__error" role="alert">
                <AlertTriangle size={14} />
                {saveError}
              </span>
            ) : dirty ? (
              <span className="profile-save-pulse-tip">有未保存的更改</span>
            ) : (
              <span className="profile-save-synced-tip">设置已同步</span>
            )}
            <button
              type="button"
              className={`profile-action-button is-primary ${dirty ? "is-pulse-glow" : ""}`}
              onClick={onSave}
              disabled={!dirty || savingSettings}
            >
              <Save size={14} />
              {savingSettings ? "保存中" : "保存设置"}
            </button>
          </div>
        ) : null}
      </div>
    </div>
  );
}

function ProfileMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="profile-metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
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
