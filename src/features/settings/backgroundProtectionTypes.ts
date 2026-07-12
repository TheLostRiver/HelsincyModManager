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

const UNKNOWN_ERROR_MESSAGE = "后台保护操作未完成，请重新检查状态后重试。";

export function getBackgroundProtectionCopy(status: BackgroundProtectionStatus): BackgroundProtectionCopy {
  switch (status) {
    case "not_enabled":
      return {
        label: "未启用",
        description: "自动备份只会在客户端运行期间检查。",
        tone: "neutral",
        action: "none",
      };
    case "starting":
      return {
        label: "正在验证后台保护",
        description: "后台任务已注册，正在等待首次运行验证。",
        tone: "warning",
        action: "none",
      };
    case "protected":
      return {
        label: "已保护",
        description: "后台任务与最近一次运行均已验证，退出客户端后仍会继续检查。",
        tone: "success",
        action: "none",
      };
    case "registration_failed":
      return {
        label: "注册未完成",
        description: "后台任务未通过完整注册检查，当前不能确认退出后仍受保护。",
        tone: "danger",
        action: "retry",
      };
    case "worker_unhealthy":
      return {
        label: "后台运行异常",
        description: "后台任务存在，但最近一次运行验证不可用或已经过期。",
        tone: "danger",
        action: "retry",
      };
    case "permission_required":
      return {
        label: "需要系统权限",
        description: "当前账户无法完成后台任务注册或检查。",
        tone: "warning",
        action: "retry",
      };
    case "unsupported_platform":
      return {
        label: "当前平台不支持",
        description: "此平台暂不支持退出客户端后的系统后台保护。",
        tone: "neutral",
        action: "none",
      };
  }
}

export function getBackgroundProtectionErrorCode(error: unknown): string {
  if (!error || typeof error !== "object" || !("code" in error)) {
    return "unknown";
  }

  const code = (error as { code?: unknown }).code;
  return typeof code === "string" && code.trim() ? code : "unknown";
}

export function getBackgroundProtectionErrorMessage(code: string | null | undefined): string {
  switch (code) {
    case "save_backup_background_permission_required":
      return "系统拒绝更新后台任务，请检查当前账户权限后重试。";
    case "save_backup_background_unsupported_platform":
      return "当前平台不支持此后台保护方式。";
    case "save_backup_background_not_registered":
      return "系统后台任务尚未完成注册，请重试启用。";
    case "save_backup_background_configuration_drift":
      return "后台任务配置与当前版本不一致，请重试启用。";
    case "save_backup_background_registration_failed":
      return "系统后台任务注册失败，请稍后重试。";
    case "save_backup_background_worker_unhealthy":
      return "后台运行验证不可用或已经过期，请重试启用。";
    case "save_backup_background_settings_unavailable":
      return "后台保护设置暂时无法读取，请重新检查。";
    case "save_backup_scheduler_unavailable":
      return "自动备份调度状态暂时不可用，请重新检查。";
    case "save_backup_clock_unavailable":
    case "save_backup_background_audit_unavailable":
    case "save_backup_background_status_unavailable":
      return "后台保护状态暂时不可用，请重新检查。";
    default:
      return UNKNOWN_ERROR_MESSAGE;
  }
}
