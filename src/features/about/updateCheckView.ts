// 把后端事实投影成「界面该显示成什么样」。**纯逻辑、不含文案**：
// 文案一律在 `aboutPageCopy`，这里只决定形态（与项目「前端只投影」的约定一致）。
//
// 形态与验收标准的对应：
// - `update_available` → 明确显示新版本号（验收标准 1）
// - `up_to_date` → 一句「已是最新版本」，不打扰（验收标准 2）
// - `unknown` → **什么都不显示**：断网、超时、接口失败都落到这里，静默（验收标准 3）
//
// `stale`：上一次查询**失败**但手里还有旧结论时为 true。这时仍然展示旧结论
// （它曾是真的），但必须同时说明「上次检查失败」——否则用户点了「检查更新」
// 后会以为复查通过了，而这正是本功能要防的事（有新版本却以为没有）。

import type { AppUpdateStatusDto } from "./updateCheckTypes.ts";

export type UpdateCheckView =
  | { kind: "checking" }
  | { kind: "up_to_date"; stale: boolean }
  | { kind: "update_available"; version: string; stale: boolean }
  | { kind: "unknown" };

export function projectUpdateCheckView(input: {
  checking: boolean;
  status: AppUpdateStatusDto | null;
  /** 最近一次查询是否失败（有旧结论时才会用到）。 */
  attemptFailed: boolean;
}): UpdateCheckView {
  if (input.checking) {
    return { kind: "checking" };
  }

  const status = input.status;
  if (status === null) {
    // 非 Tauri 环境、或调用本身没拿到结论。
    return { kind: "unknown" };
  }

  switch (status.status) {
    case "update_available":
      // 契约保证这个状态必带 latestVersion；万一没带，宁可显示「不知道」，
      // 也不能出现一个空白的「可用」。
      return status.latestVersion
        ? { kind: "update_available", version: status.latestVersion, stale: input.attemptFailed }
        : { kind: "unknown" };
    case "up_to_date":
      return { kind: "up_to_date", stale: input.attemptFailed };
    default:
      // 含 `unknown`，以及将来新增的任何未知状态值。
      return { kind: "unknown" };
  }
}
