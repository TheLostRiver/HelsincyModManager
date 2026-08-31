// 把后端事实投影成「界面该显示成什么样」。**纯逻辑、不含文案**：
// 文案一律在 `aboutPageCopy`，这里只决定形态（与项目「前端只投影」的约定一致）。
//
// 形态与验收标准的对应：
// - `update_available` → 明确显示新版本号（验收标准 1）
// - `up_to_date` → 一句「已是最新版本」，不打扰（验收标准 2）
// - `unknown` → **什么都不显示**：断网、超时、接口失败都落到这里，静默（验收标准 3）

import type { AppUpdateStatusDto } from "./updateCheckTypes.ts";

export type UpdateCheckView =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "up_to_date" }
  | { kind: "update_available"; version: string }
  | { kind: "unknown" };

export function projectUpdateCheckView(input: {
  checking: boolean;
  status: AppUpdateStatusDto | null;
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
        ? { kind: "update_available", version: status.latestVersion }
        : { kind: "unknown" };
    case "up_to_date":
      return { kind: "up_to_date" };
    default:
      // 含 `unknown`，以及将来新增的任何未知状态值。
      return { kind: "unknown" };
  }
}
