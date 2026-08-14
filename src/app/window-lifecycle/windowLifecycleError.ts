const FALLBACK_WINDOW_LIFECYCLE_ERROR_MESSAGE = "窗口关闭操作失败";

const WINDOW_LIFECYCLE_ERROR_MESSAGES = {
  exit_confirmation_required: "退出前需要确认后台保护状态。",
  exit_authorization_unavailable: "退出确认状态不可用，请暂时留在托盘或重启应用后再试。",
  window_hide_failed: "窗口隐藏失败，请重试。",
} as const;

type WindowLifecycleErrorCode = keyof typeof WINDOW_LIFECYCLE_ERROR_MESSAGES;

type ErrorCodeShape = {
  code?: unknown;
};

export function getWindowLifecycleErrorCode(error: unknown): string | null {
  if (!error || typeof error !== "object") return null;
  const code = (error as ErrorCodeShape).code;
  return typeof code === "string" && code.trim() ? code : null;
}

export function getWindowLifecycleErrorMessage(error: unknown): string {
  const code = getWindowLifecycleErrorCode(error);
  return code && Object.prototype.hasOwnProperty.call(WINDOW_LIFECYCLE_ERROR_MESSAGES, code)
    ? WINDOW_LIFECYCLE_ERROR_MESSAGES[code as WindowLifecycleErrorCode]
    : FALLBACK_WINDOW_LIFECYCLE_ERROR_MESSAGE;
}
