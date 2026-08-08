export type DebugLogSettingsDto = {
  enabled: boolean;
};

export type DebugLogSettingsState =
  | { status: "loading" }
  | { status: "ready"; settings: DebugLogSettingsDto; errorCode: string | null }
  | { status: "error"; errorCode: string };

export function getDebugLogErrorCode(error: unknown): string {
  if (!error || typeof error !== "object" || !("code" in error)) {
    return "unknown";
  }

  const code = (error as { code?: unknown }).code;
  return typeof code === "string" && code.trim() ? code : "unknown";
}

export function getDebugLogErrorMessage(code: string): string {
  switch (code) {
    case "configuration_database_unavailable":
    case "app_settings_unavailable":
      return "调试日志设置暂时不可用，请稍后重试。";
    case "app_settings_save_failed":
      return "调试日志设置保存失败，当前运行状态未改变。";
    default:
      return "调试日志设置暂时不可用，请重新检查。";
  }
}
