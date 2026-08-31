// 「检查更新」的 feature-local typed API。命令契约见
// `docs/FRONTEND_BACKEND_CONTRACT.md` 的「检查更新（只告知，不下载）」一节。

import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AppUpdateStatusDto } from "./updateCheckTypes.ts";

/**
 * 查询是否有可用更新。
 *
 * 返回 `null` 表示「没有结论」：非 Tauri 环境（浏览器预览）或调用本身出了意外。
 * 契约保证这个 command 不会失败，所以这里的 `catch` 只是兜底——
 * **任何异常都按「不知道」处理，绝不弹错误**、不让页面进入降级状态。
 */
export async function checkAppUpdate(): Promise<AppUpdateStatusDto | null> {
  if (!isTauri()) {
    return null;
  }

  try {
    return await invoke<AppUpdateStatusDto>("check_app_update");
  } catch {
    return null;
  }
}
