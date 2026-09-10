import type { AppUpdateStatusDto } from "./updateCheckTypes.ts";

export type UpdateCheckView =
  | { kind: "idle" | "checking" | "unavailable" | "up_to_date" | "no_release" }
  | { kind: "update_available"; version: string };

// 未知不是“最新”，失败后不继续显示旧成功结论。
export function projectUpdateCheckView(input: {
  checking: boolean;
  status: AppUpdateStatusDto | null;
  attemptFailed: boolean;
}): UpdateCheckView {
  if (input.checking) return { kind: "checking" };
  if (input.attemptFailed) return { kind: "unavailable" };
  if (input.status === null) return { kind: "idle" };
  switch (input.status.status) {
    case "update_available":
      return input.status.latestVersion?.trim()
        ? { kind: "update_available", version: input.status.latestVersion }
        : { kind: "unavailable" };
    case "up_to_date":
      return { kind: "up_to_date" };
    case "no_release":
      return { kind: "no_release" };
    default:
      return { kind: "unavailable" };
  }
}
