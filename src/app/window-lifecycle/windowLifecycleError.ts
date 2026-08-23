import type { AppShellCopy } from "../appShellCopy";

// 稳定码 -> 文案的映射经 appShellCopy.windowLifecycle 取（语义/文本分离）。
type WindowLifecycleErrorCode = keyof AppShellCopy["windowLifecycle"]["errors"];

type ErrorCodeShape = {
  code?: unknown;
};

export function getWindowLifecycleErrorCode(error: unknown): string | null {
  if (!error || typeof error !== "object") return null;
  const code = (error as ErrorCodeShape).code;
  return typeof code === "string" && code.trim() ? code : null;
}

export function getWindowLifecycleErrorMessage(
  error: unknown,
  copy: AppShellCopy["windowLifecycle"],
): string {
  const code = getWindowLifecycleErrorCode(error);
  return code && Object.prototype.hasOwnProperty.call(copy.errors, code)
    ? copy.errors[code as WindowLifecycleErrorCode]
    : copy.errorFallback;
}
