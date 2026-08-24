import { DEFAULT_PROFILE_BACKUP_RETENTION } from "./profileSaveSettingsDefaults";
import type { SaveBackupSummaryDto } from "./profileSaveBackupTypes";
import type { SaveDirectoryDiscoveryDto } from "./profileSaveDirectoryDiscoveryTypes";
import type {
  ProfileDirectorySelectionDto,
  ProfileSaveSettingsDto,
} from "./profileSaveSettingsTypes";
import type { Profile } from "./profileTypes";

// 纯浏览器预览环境的模拟数据（配置档、存档设置、备份历史、目录选择结果）。
// 与 modsLibraryData.ts 同性质：mock 内容不属于 UI 文案，不做翻译，
// 也不进入 i18n sweep 的无中文清单。

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

export const PREVIEW_SAVE_SETTINGS: ProfileSaveSettingsDto = {
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
  retention: { ...DEFAULT_PROFILE_BACKUP_RETENTION },
  steamAccount: null,
  preRestoreBackupEnabled: true,
  updatedAt: 0,
};

const PREVIEW_SAVE_SETTINGS_BY_PROFILE: Record<string, ProfileSaveSettingsDto> = {
  "preview-default": PREVIEW_SAVE_SETTINGS,
  "preview-taichi": {
    ...PREVIEW_SAVE_SETTINGS,
    profileId: "preview-taichi",
    schedule: { cadence: "weekly", hour: 2, minute: 30, weekdays: [1, 3, 5] },
    retention: { maxCount: 36, maxAgeDays: 60, maxTotalBytes: null },
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
    retention: { maxCount: 12, maxAgeDays: 14, maxTotalBytes: null },
    saveDirectory: {
      mode: "unset",
      status: "unset",
      pathLabel: null,
      messages: ["等待关联游戏存档源目录"],
    },
  },
};

export function createPreviewProfiles(): Profile[] {
  return PREVIEW_PROFILES.map((profile) => ({ ...profile }));
}

export function createPreviewSaveSettings(profileId = PREVIEW_SAVE_SETTINGS.profileId): ProfileSaveSettingsDto {
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

export function createPreviewSaveBackups(gameId: string, profileId: string | null): SaveBackupSummaryDto[] {
  if (profileId === "preview-online-test" || profileId === null) return [];

  const now = Date.now();
  const rows: SaveBackupSummaryDto[] = [
    {
      backupId: "preview-backup-fatalis",
      gameId,
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
      gameId,
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
        gameId,
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

/** 预览环境的 Steam 多账号发现结果：让候选选择 UI 在纯浏览器下可见、可调整。
 *  真实链路里账号/路径由后端脱敏投影，这里的标签只模仿投影后的形态。 */
export function createPreviewSaveDirectoryDiscovery(
  gameId: string,
  profileId: string,
): SaveDirectoryDiscoveryDto {
  const now = Date.now();

  return {
    discoveryId: `preview-discovery-${profileId}`,
    gameId,
    profileId,
    outcome: "confirmation_required",
    recommendedCandidateId: "preview-candidate-recent",
    savedSettings: null,
    candidates: [
      {
        candidateId: "preview-candidate-recent",
        source: "steam_userdata",
        confidence: "high",
        recommended: true,
        accountName: "Hunter-Rathalos",
        avatarUrl: null,
        accountLabel: "Steam 账号 ····2481",
        pathLabel: "userdata/····2481/582010/remote",
        lastModifiedAt: now - 2 * 60 * 60 * 1000,
        evidence: ["预览示例：检测到 MHW:I 存档文件", "最近游玩过本作"],
      },
      {
        candidateId: "preview-candidate-secondary",
        source: "steam_userdata",
        confidence: "medium",
        recommended: false,
        accountName: null,
        avatarUrl: null,
        accountLabel: "Steam 账号 ····0937",
        pathLabel: "userdata/····0937/582010/remote",
        lastModifiedAt: now - 26 * 60 * 60 * 1000,
        evidence: ["预览示例：存在存档目录，公开资料未能补全"],
      },
      {
        candidateId: "preview-candidate-stale",
        source: "steam_userdata",
        confidence: "low",
        recommended: false,
        accountName: "Palico-Fan",
        avatarUrl: null,
        accountLabel: "Steam 账号 ····5610",
        pathLabel: "userdata/····5610/582010/remote",
        lastModifiedAt: now - 40 * 24 * 60 * 60 * 1000,
        evidence: ["预览示例：目录较旧，可能来自弃用账号"],
      },
    ],
  };
}

/** 预览环境确认候选后的推进结果：仿真真实链路的 auto_saved 收尾。 */
export function createPreviewSaveDirectoryConfirmation(
  discovery: SaveDirectoryDiscoveryDto,
  candidateId: string,
): SaveDirectoryDiscoveryDto {
  const chosen = discovery.candidates.find(
    (candidate) => candidate.candidateId === candidateId,
  );

  return {
    ...discovery,
    outcome: "auto_saved",
    recommendedCandidateId: null,
    candidates: chosen ? [chosen] : [],
    savedSettings: {
      mode: "custom",
      status: "valid",
      pathLabel: chosen?.pathLabel ?? "userdata/····2481/582010/remote",
      messages: ["预览环境已模拟保存所选账号的存档目录"],
    },
  };
}

export function createPreviewDirectorySelection(
  kind: "saveDirectory" | "backupDirectory",
): { directory: string; selection: ProfileDirectorySelectionDto } {
  const directory =
    kind === "saveDirectory"
      ? "Steam/userdata/<steam-id>/582010/remote"
      : "HelsincyModManager/Backups/MHW";

  return {
    directory,
    selection: {
      mode: "custom",
      status: "valid",
      pathLabel: directory,
      messages: ["预览环境已模拟校验通过"],
    },
  };
}
