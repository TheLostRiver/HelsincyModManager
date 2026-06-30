import { AlertTriangle, CheckCircle2, Loader2, Save, ShieldCheck } from "lucide-react";
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
  getProfileMetrics,
} from "./profileViewModel";

const CURRENT_GAME_ID = "mhw";
const PREVIEW_SAVE_SETTINGS: ProfileSaveSettingsDto = {
  profileId: "preview",
  saveDirectory: {
    mode: "unset",
    status: "unset",
    pathLabel: null,
    messages: ["选择配置档后可设置游戏存档目录"],
  },
  backupDirectory: {
    mode: "default",
    status: "defaulted",
    pathLabel: "HelsincyModManager/Backups",
    messages: ["选择配置档后可更改备份目录"],
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

function createPreviewSaveSettings(profileId = PREVIEW_SAVE_SETTINGS.profileId): ProfileSaveSettingsDto {
  return {
    ...PREVIEW_SAVE_SETTINGS,
    profileId,
    saveDirectory: {
      ...PREVIEW_SAVE_SETTINGS.saveDirectory,
      messages: [...PREVIEW_SAVE_SETTINGS.saveDirectory.messages],
    },
    backupDirectory: {
      ...PREVIEW_SAVE_SETTINGS.backupDirectory,
      messages: [...PREVIEW_SAVE_SETTINGS.backupDirectory.messages],
    },
    schedule: {
      ...PREVIEW_SAVE_SETTINGS.schedule,
      weekdays: [...PREVIEW_SAVE_SETTINGS.schedule.weekdays],
    },
    retention: { ...PREVIEW_SAVE_SETTINGS.retention },
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
  const [busyProfileId, setBusyProfileId] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [savingSettings, setSavingSettings] = useState(false);
  const [dirty, setDirty] = useState(false);
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
        if (!cancelled) {
          setProfileState((current) => ({ status: "error", profiles: current.profiles }));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [refreshToken]);

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

    void getProfileSaveSettings(selectedProfileId)
      .then((settings) => {
        if (!cancelled) {
          setSettingsState({ status: "ready", settings });
          setDraftSettings(settings);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setSettingsState({ status: "error", message: getErrorMessage(error, "存档设置不可用") });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [selectedProfileId]);

  const profiles = profileState.profiles;
  const metrics = useMemo(() => getProfileMetrics(profiles), [profiles]);
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
    <section className="profile-page" aria-labelledby="profile-page-title">
      <header className="profile-page__header">
        <div className="profile-page__title-block">
          <span className="profile-page__eyebrow">
            <ShieldCheck size={15} />
            Profile Workspace
          </span>
          <h1 id="profile-page-title">配置档设置</h1>
        </div>
        <div className="profile-page__summary" aria-label="配置档统计">
          <ProfileMetric label="总数" value={String(metrics.totalCount)} />
          <ProfileMetric label="备用" value={String(metrics.standbyCount)} />
          <ProfileMetric label="可删" value={String(metrics.deletableCount)} />
        </div>
      </header>

      <div className="profile-workspace">
        <ProfileListPanel
          profiles={profiles}
          status={profileState.status}
          selectedProfileId={selectedProfileId}
          busyProfileId={busyProfileId}
          onRefresh={refreshProfiles}
          onSelectProfile={setSelectedProfileId}
          onActivateProfile={handleActivateProfile}
          onProfilesChanged={() => {
            refreshProfiles();
            refreshActiveProfile();
          }}
        />

        <main className="profile-settings-stack" aria-live="polite">
          <ProfileOverview
            profile={selectedProfile}
            settings={settingsState.status === "ready" ? settingsState.settings : null}
          />

          {settingsState.status === "idle" ? (
            <div className="profile-settings-panel profile-settings-state" role="status">
              <span>选择配置档后显示存档设置</span>
            </div>
          ) : null}

          {settingsState.status === "loading" ? (
            <div className="profile-settings-panel profile-settings-state" role="status">
              <Loader2 className="profile-spinner" size={20} />
              <span>正在读取存档设置</span>
            </div>
          ) : null}

          {settingsState.status === "error" ? (
            <div className="profile-settings-panel profile-settings-state is-error" role="alert">
              <AlertTriangle size={20} />
              <span>{settingsState.message}</span>
              <button
                type="button"
                className="profile-action-button"
                onClick={() => selectedProfileId && setSelectedProfileId(selectedProfileId)}
              >
                重试
              </button>
            </div>
          ) : null}

          <SaveDirectoryPanel
            gameId={CURRENT_GAME_ID}
            profileId={selectedProfileId ?? PREVIEW_SAVE_SETTINGS.profileId}
            settings={visibleSettings}
            disabled={!settingsEditable}
            onSettingsChange={updateSettings}
            onDirectorySelected={(kind, directory) =>
              setPendingDirectories((current) => ({ ...current, [kind]: directory }))
            }
          />
          <BackupPolicyPanel
            settings={visibleSettings}
            onScheduleChange={updateSchedule}
            onRetentionChange={updateRetention}
          />

          {settingsEditable ? (
            <>
              <div className="profile-save-bar">
                {saveError ? (
                  <span className="profile-save-bar__error" role="alert">
                    <AlertTriangle size={14} />
                    {saveError}
                  </span>
                ) : dirty ? (
                  <span>有未保存的更改</span>
                ) : (
                  <span>设置已同步</span>
                )}
                <button
                  type="button"
                  className="profile-action-button is-primary"
                  onClick={() => void saveSettings()}
                  disabled={!dirty || savingSettings}
                >
                  <Save size={14} />
                  {savingSettings ? "保存中" : "保存设置"}
                </button>
              </div>
            </>
          ) : null}
        </main>
      </div>
    </section>
  );
}

function ProfileOverview({
  profile,
  settings,
}: {
  profile: Profile | null;
  settings: ProfileSaveSettingsDto | null;
}) {
  const saveStatus = settings ? formatDirectoryStatus(settings.saveDirectory) : null;
  const backupStatus = settings ? formatDirectoryStatus(settings.backupDirectory) : null;

  return (
    <section className="profile-settings-panel profile-overview" aria-label="当前配置档">
      <div className="profile-overview__identity">
        <span className="profile-overview__mark" aria-hidden="true">
          <CheckCircle2 size={20} />
        </span>
        <div>
          <h2>{profile?.name ?? "未选择配置档"}</h2>
          <p>{profile?.description || profile?.id || "选择一个配置档开始配置"}</p>
        </div>
      </div>
      <div className="profile-overview__facts">
        <ProfileMetric label="存档目录" value={saveStatus?.label ?? "-"} />
        <ProfileMetric label="备份目录" value={backupStatus?.label ?? "-"} />
        <ProfileMetric label="自动备份" value={settings ? formatBackupSchedule(settings.schedule) : "-"} />
      </div>
    </section>
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
