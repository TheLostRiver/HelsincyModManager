// 「检查更新」的前端类型。与 `docs/FRONTEND_BACKEND_CONTRACT.md` 的
// `check_app_update` 一节一一对应：状态值只有三个，`latestVersion` 只在
// `update_available` 时有值，且是**发布标签原文**（可能带 `v` 前缀），前端不解析。

export type AppUpdateStatus = "up_to_date" | "update_available" | "unknown";

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
