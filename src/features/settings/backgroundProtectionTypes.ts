// 直连 locales 而不是 shared/i18n barrel，且带 .ts 显式扩展：本模块会被 node --test 直接
// import（type stripping 不处理 JSX、不做扩展名推断），barrel 里的 I18nProvider.tsx 与
// 无扩展 specifier 都会让功能测试无法加载。
import { resolveCopy, type Locale } from "../../shared/i18n/locales.ts";
import { backgroundProtectionCopy } from "./backgroundProtectionCopy.ts";

export type BackgroundProtectionStatus =
  | "not_enabled"
  | "starting"
  | "protected"
  | "registration_failed"
  | "worker_unhealthy"
  | "permission_required"
  | "unsupported_platform";

export type BackgroundProtectionControlDto = {
  desiredEnabled: boolean;
  status: BackgroundProtectionStatus;
  enabledAt: number | null;
  lastHeartbeatAt: number | null;
  lastErrorCode: string | null;
};

export type BackgroundProtectionTone = "neutral" | "warning" | "success" | "danger";
export type BackgroundProtectionAction = "none" | "retry";

export type BackgroundProtectionCopy = {
  label: string;
  description: string;
  tone: BackgroundProtectionTone;
  action: BackgroundProtectionAction;
};

// 文案在 backgroundProtectionCopy.ts；这里只保留语义（tone/action）与取词组装。
const statusSemantics: Record<
  BackgroundProtectionStatus | "unknown",
  { tone: BackgroundProtectionTone; action: BackgroundProtectionAction }
> = {
  not_enabled: { tone: "neutral", action: "none" },
  starting: { tone: "warning", action: "none" },
  protected: { tone: "success", action: "none" },
  registration_failed: { tone: "danger", action: "retry" },
  worker_unhealthy: { tone: "danger", action: "retry" },
  permission_required: { tone: "warning", action: "retry" },
  unsupported_platform: { tone: "neutral", action: "none" },
  unknown: { tone: "danger", action: "retry" },
};

export function getBackgroundProtectionCopy(
  status: BackgroundProtectionStatus,
  locale: Locale,
): BackgroundProtectionCopy {
  // 后端可能给出未来版本的新状态：语义与文案都按 unknown fail closed。
  const key: BackgroundProtectionStatus | "unknown" = status in statusSemantics ? status : "unknown";
  const text = resolveCopy(backgroundProtectionCopy, locale).status[key];
  return { ...text, ...statusSemantics[key] };
}

export function getBackgroundProtectionErrorCode(error: unknown): string {
  if (!error || typeof error !== "object" || !("code" in error)) {
    return "unknown";
  }

  const code = (error as { code?: unknown }).code;
  return typeof code === "string" && code.trim() ? code : "unknown";
}

export function getBackgroundProtectionErrorMessage(
  code: string | null | undefined,
  locale: Locale,
): string {
  const errors = resolveCopy(backgroundProtectionCopy, locale).errors;
  switch (code) {
    case "save_backup_background_permission_required":
      return errors.permissionRequired;
    case "save_backup_background_unsupported_platform":
      return errors.unsupportedPlatform;
    case "save_backup_background_not_registered":
      return errors.notRegistered;
    case "save_backup_background_configuration_drift":
      return errors.configurationDrift;
    case "save_backup_background_registration_failed":
      return errors.registrationFailed;
    case "save_backup_background_worker_unhealthy":
      return errors.workerUnhealthy;
    case "save_backup_background_settings_unavailable":
      return errors.settingsUnavailable;
    case "save_backup_scheduler_unavailable":
      return errors.schedulerUnavailable;
    case "save_backup_clock_unavailable":
    case "save_backup_background_audit_unavailable":
    case "save_backup_background_status_unavailable":
      return errors.statusUnavailable;
    default:
      // 未知错误码只返回通用文案，绝不把 code 本身拼进消息（可能含路径等敏感内容）。
      return errors.unknown;
  }
}

export function hasBackgroundProtectionConverged(
  control: BackgroundProtectionControlDto,
  desiredEnabled: boolean,
): boolean {
  if (desiredEnabled) {
    return (
      control.desiredEnabled &&
      (control.status === "starting" || control.status === "protected")
    );
  }

  return !control.desiredEnabled && control.status === "not_enabled";
}

export function formatBackgroundProtectionDuration(elapsedMs: number, locale: Locale): string {
  const duration = resolveCopy(backgroundProtectionCopy, locale).duration;
  if (!Number.isFinite(elapsedMs) || elapsedMs <= 0) return duration.underTenth;
  return duration.seconds(Math.max(0.1, elapsedMs / 1_000).toFixed(1));
}
