import type { RecoveryCenterCopy } from "./recoveryCenterCopy";
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
  copy: RecoveryCenterCopy["globalAlert"],
): InstallRecoveryGlobalAlertView | null {
  if (state.status === "idle" || state.status === "loading") {
    return null;
  }

  if (state.status === "unavailable") {
    return {
      status: "unknown",
      title: copy.unavailableTitle,
      description: copy.unavailableDescription,
      actionLabel: copy.openRecoveryCenter,
    };
  }

  if (state.health.status !== "attention") {
    return null;
  }

  const parts: string[] = [];
  if (state.health.attentionModCount > 0) {
    parts.push(copy.partAttention(state.health.attentionModCount));
  }
  if (state.health.unknownModCount > 0) {
    parts.push(copy.partUnknown(state.health.unknownModCount));
  }
  if (state.health.issueCount > 0) {
    parts.push(copy.partIssues(state.health.issueCount));
  }
  const summary = parts.length > 0 ? parts.join(copy.partJoin) : copy.fallbackSummary;

  return {
    status: "attention",
    title: copy.attentionTitle,
    description: copy.attentionDescription(summary),
    actionLabel: copy.openRecoveryCenter,
  };
}
