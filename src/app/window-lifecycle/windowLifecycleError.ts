const FALLBACK_WINDOW_LIFECYCLE_ERROR_MESSAGE = "窗口关闭操作失败";

type ErrorMessageShape = {
  message?: unknown;
};

function messageFromObject(error: object): string | null {
  const message = (error as ErrorMessageShape).message;
  return typeof message === "string" && message.trim() ? message : null;
}

export function getWindowLifecycleErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message;
  if (error && typeof error === "object") return messageFromObject(error) ?? FALLBACK_WINDOW_LIFECYCLE_ERROR_MESSAGE;
  if (typeof error === "string" && error.trim()) return error;
  return FALLBACK_WINDOW_LIFECYCLE_ERROR_MESSAGE;
}
