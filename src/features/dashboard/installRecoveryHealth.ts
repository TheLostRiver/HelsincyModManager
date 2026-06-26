import type { InstallRecoveryIssue, InstallRecoveryIssueSummary, InstallRecoverySummary } from "../mods/modInstallPlanTypes";

export type InstallRecoveryHealthStatus = "empty" | "healthy" | "attention";

export type InstallRecoveryHealth = {
  status: InstallRecoveryHealthStatus;
  scannedModCount: number;
  completedModCount: number;
  attentionModCount: number;
  unknownModCount: number;
  managedFileCount: number;
  backupCount: number;
  issueCount: number;
  issues: InstallRecoveryIssueSummary[];
};

const issueDisplayOrder: InstallRecoveryIssue[] = [
  "target_changed",
  "target_missing",
  "target_read_failed",
  "backup_missing",
  "backup_read_failed",
  "missing_installed_file_summary",
];

export function deriveInstallRecoveryHealth(summaries: InstallRecoverySummary[]): InstallRecoveryHealth {
  const issueCounts = new Map<InstallRecoveryIssue, number>();
  let completedModCount = 0;
  let attentionModCount = 0;
  let unknownModCount = 0;
  let managedFileCount = 0;
  let backupCount = 0;
  let issueCount = 0;

  for (const summary of summaries) {
    if (summary.status === "completed") {
      completedModCount += 1;
    } else if (summary.status === "repair_required") {
      attentionModCount += 1;
    } else if (summary.status === "unknown") {
      unknownModCount += 1;
    }

    managedFileCount += summary.managedFileCount;
    backupCount += summary.backupCount;
    issueCount += summary.issueCount;

    for (const issue of summary.issues) {
      issueCounts.set(issue.issue, (issueCounts.get(issue.issue) ?? 0) + issue.count);
    }
  }

  const issues = issueDisplayOrder.flatMap((issue) => {
    const count = issueCounts.get(issue) ?? 0;
    return count > 0 ? [{ issue, count }] : [];
  });

  return {
    status: summaries.length === 0 ? "empty" : attentionModCount > 0 || unknownModCount > 0 ? "attention" : "healthy",
    scannedModCount: summaries.length,
    completedModCount,
    attentionModCount,
    unknownModCount,
    managedFileCount,
    backupCount,
    issueCount,
    issues,
  };
}
