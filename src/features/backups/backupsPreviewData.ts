import type { SaveBackupSummaryDto } from "../profiles/profileSaveBackupTypes";
import type {
  SaveBackupCenterPageDto,
  SaveBackupCenterProfileSummaryDto,
} from "./backupCenterTypes";

// 纯浏览器预览环境的备份中心模拟数据（mock 内容不翻译，不进入 i18n sweep 的无中文清单）。

const PREVIEW_GAME_ID = "mhw";
const PREVIEW_PAGE_LIMIT = 12;

type PreviewQuery = {
  profileId: string | null;
  trigger: string | null;
  status: string | null;
  search: string;
  offset: number;
};

export function createPreviewPage(query: PreviewQuery): SaveBackupCenterPageDto {
  const backups: SaveBackupSummaryDto[] = [
    {
      backupId: "mhw:default:20260815-120000:manual",
      gameId: PREVIEW_GAME_ID,
      profileId: "default",
      trigger: "manual",
      status: "completed",
      fileName: "20260815-120000_mhw_profile-default_manual.zip",
      createdAt: Date.now() - 45 * 60_000,
      sizeBytes: 18_482_944,
      fileCount: 2,
      sourcePathLabel: "synthetic save",
      notes: "Fatalis 配装前",
    },
    {
      backupId: "mhw:default:20260814-090000:pre_restore",
      gameId: PREVIEW_GAME_ID,
      profileId: "default",
      trigger: "pre_restore",
      status: "completed",
      fileName: "20260814-090000_mhw_profile-default_pre_restore.zip",
      createdAt: Date.now() - 26 * 60 * 60_000,
      sizeBytes: 17_965_120,
      fileCount: 2,
      sourcePathLabel: "synthetic save",
      notes: "恢复前保护点",
    },
    {
      backupId: "mhw:taichi:20260813-230000:auto",
      gameId: PREVIEW_GAME_ID,
      profileId: "taichi",
      trigger: "auto",
      status: "retention_partial",
      fileName: "20260813-230000_mhw_profile-taichi_auto.zip",
      createdAt: Date.now() - 42 * 60 * 60_000,
      sizeBytes: 20_125_696,
      fileCount: 2,
      sourcePathLabel: "synthetic save",
      notes: "等待下次整理重试",
    },
  ];
  const profiles: SaveBackupCenterProfileSummaryDto[] = [
    {
      profileId: "default",
      profileName: "Default 配置档",
      isActive: true,
      steamAccount: {
        accountName: "Synthetic Hunter",
        avatarUrl: null,
        accountLabel: "Steam 12****34",
      },
      retention: { maxCount: 20, maxAgeDays: 30, maxTotalBytes: null },
      backupCount: 2,
      archiveBytes: 36_448_064,
      protectedCount: 1,
      attentionCount: 0,
      budgetSatisfied: true,
    },
    {
      profileId: "taichi",
      profileName: "太刀毕业档",
      isActive: false,
      steamAccount: {
        accountName: null,
        avatarUrl: null,
        accountLabel: "Steam 56****78",
      },
      retention: { maxCount: 12, maxAgeDays: 14, maxTotalBytes: 64 * 1024 * 1024 },
      backupCount: 1,
      archiveBytes: 20_125_696,
      protectedCount: 0,
      attentionCount: 1,
      budgetSatisfied: true,
    },
  ];
  const filtered = backups.filter((backup) => {
    if (query.profileId && backup.profileId !== query.profileId) return false;
    if (query.trigger && backup.trigger !== query.trigger) return false;
    if (query.status && backup.status !== query.status) return false;
    if (query.search) {
      const profile = profiles.find((item) => item.profileId === backup.profileId);
      const haystack = `${profile?.profileName ?? ""} ${backup.notes ?? ""}`.toLowerCase();
      if (!haystack.includes(query.search.toLowerCase())) return false;
    }
    return true;
  });
  return {
    offset: query.offset,
    limit: PREVIEW_PAGE_LIMIT,
    totalCount: filtered.length,
    summary: {
      backupCount: filtered.length,
      archiveBytes: filtered.reduce((sum, backup) => sum + backup.sizeBytes, 0),
      protectedCount: filtered.filter((backup) => backup.trigger === "pre_restore").length,
      attentionCount: filtered.filter((backup) => backup.status === "retention_partial").length,
    },
    profiles,
    items: filtered.slice(query.offset, query.offset + PREVIEW_PAGE_LIMIT).map((backup) => ({
      profileName: profiles.find((profile) => profile.profileId === backup.profileId)?.profileName ?? backup.profileId,
      backup,
    })),
  };
}
