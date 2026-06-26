import type { InstallRecoveryHealthLoadState } from "./useInstallRecoveryHealth";

export type InstallRecoveryGlobalAlertStatus = "attention" | "unknown";

export type InstallRecoveryGlobalAlertView = {
  status: InstallRecoveryGlobalAlertStatus;
  title: string;
  description: string;
  actionLabel: string;
};

export function deriveInstallRecoveryGlobalAlert(
  state: InstallRecoveryHealthLoadState,
): InstallRecoveryGlobalAlertView | null {
  if (state.status === "idle" || state.status === "loading") {
    return null;
  }

  if (state.status === "unavailable") {
    return {
      status: "unknown",
      title: "恢复摘要暂时不可用",
      description: "无法确认当前配置档的托管安装状态。进入恢复中心后可重新扫描或导出诊断摘要。",
      actionLabel: "打开恢复中心",
    };
  }

  if (state.health.status !== "attention") {
    return null;
  }

  const parts: string[] = [];
  if (state.health.attentionModCount > 0) {
    parts.push(`${state.health.attentionModCount} 个需处理`);
  }
  if (state.health.unknownModCount > 0) {
    parts.push(`${state.health.unknownModCount} 个状态未知`);
  }
  if (state.health.issueCount > 0) {
    parts.push(`${state.health.issueCount} 个问题`);
  }
  const summary = parts.length > 0 ? parts.join("，") : "存在需要关注的托管安装状态";

  return {
    status: "attention",
    title: "托管安装需要处理",
    description: `当前配置档扫描到 ${summary}。恢复中心只会展示安全摘要，不会自动恢复或写入清单。`,
    actionLabel: "打开恢复中心",
  };
}
