// 「检查更新」的前端类型。与 `docs/FRONTEND_BACKEND_CONTRACT.md` 的
// `check_app_update` 一节一一对应，`latestVersion` 只在
// `update_available` 时有值，且是**发布标签原文**（可能带 `v` 前缀），前端不解析。

export type AppUpdateStatus = "up_to_date" | "update_available" | "no_release" | "unknown";

export type AppUpdateStatusDto = {
  status: AppUpdateStatus;
  currentVersion: string;
  latestVersion: string | null;
};

export type UpdateCheckPreference = {
  /** 是否自动检查。关掉后前端不再发起查询（后端不保存这个状态）。 */
  autoCheckEnabled: boolean;
  /** 上次查询的时刻（epoch 毫秒）；`null` 表示从未查过。 */
  lastCheckedAt: number | null;
};

export const DEFAULT_UPDATE_CHECK_PREFERENCE: UpdateCheckPreference = {
  autoCheckEnabled: true,
  lastCheckedAt: null,
};

export function normalizeAppUpdateStatus(value: unknown): AppUpdateStatusDto | null {
  if (typeof value !== "object" || value === null) return null;
  const item = value as Partial<AppUpdateStatusDto>;
  if (typeof item.currentVersion !== "string" || !item.currentVersion.trim()) return null;
  if (item.status === "update_available") {
    return typeof item.latestVersion === "string" && item.latestVersion.trim()
      ? item as AppUpdateStatusDto
      : null;
  }
  return ["up_to_date", "no_release", "unknown"].includes(item.status ?? "") && item.latestVersion === null
    ? item as AppUpdateStatusDto
    : null;
}
