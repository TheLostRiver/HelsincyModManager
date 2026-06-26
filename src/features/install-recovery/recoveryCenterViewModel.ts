import type {
  InstallRecoveryIssue,
  InstallRecoveryIssueSummary,
  InstallRecoveryStatus,
  InstallRecoverySummary,
} from "../mods/modInstallPlanTypes";

export type RecoveryCenterStatus = "empty" | "healthy" | "attention";

export type RecoveryCenterIssueView = InstallRecoveryIssueSummary & {
  label: string;
};

export type RecoveryCenterOverview = {
  status: RecoveryCenterStatus;
  scannedModCount: number;
  completedModCount: number;
  attentionModCount: number;
  unknownModCount: number;
  managedFileCount: number;
  backupCount: number;
  issueCount: number;
  issues: RecoveryCenterIssueView[];
};

export type RecoveryCenterModView = {
  modId: string;
  status: InstallRecoveryStatus;
  statusLabel: string;
  statusTone: "healthy" | "attention" | "unknown" | "empty";
  managedFileCount: number;
  backupCount: number;
  issueCount: number;
  issues: RecoveryCenterIssueView[];
};

export type RecoveryCenterViewModel = {
  overview: RecoveryCenterOverview;
  mods: RecoveryCenterModView[];
};

const issueDisplayOrder: InstallRecoveryIssue[] = [
  "target_changed",
  "target_missing",
  "target_read_failed",
  "backup_missing",
  "backup_read_failed",
  "missing_installed_file_summary",
];

const issueLabels: Record<InstallRecoveryIssue, string> = {
  missing_installed_file_summary: "摘要缺失",
  target_missing: "目标缺失",
  target_changed: "目标变更",
  target_read_failed: "读取未知",
  backup_missing: "备份缺失",
  backup_read_failed: "备份未知",
};

const statusLabels: Record<InstallRecoveryStatus, string> = {
  completed: "正常",
  not_installed: "未安装",
  repair_required: "需要修复",
  unknown: "状态未知",
};

const statusSortRank: Record<InstallRecoveryStatus, number> = {
  repair_required: 0,
  unknown: 1,
  completed: 2,
  not_installed: 3,
};

export function deriveRecoveryCenterViewModel(summaries: InstallRecoverySummary[]): RecoveryCenterViewModel {
  const issueCounts = new Map<InstallRecoveryIssue, number>();
  let completedModCount = 0;
  let attentionModCount = 0;
  let unknownModCount = 0;
  let managedFileCount = 0;
  let backupCount = 0;
  let issueCount = 0;

  const mods = summaries
    .map((summary): RecoveryCenterModView => {
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

      return {
        modId: summary.modId,
        status: summary.status,
        statusLabel: statusLabels[summary.status],
        statusTone: statusTone(summary.status),
        managedFileCount: summary.managedFileCount,
        backupCount: summary.backupCount,
        issueCount: summary.issueCount,
        issues: withIssueLabels(summary.issues),
      };
    })
    .sort((left, right) => {
      const rankDelta = statusSortRank[left.status] - statusSortRank[right.status];
      return rankDelta === 0 ? left.modId.localeCompare(right.modId) : rankDelta;
    });

  return {
    overview: {
      status: summaries.length === 0 ? "empty" : attentionModCount > 0 || unknownModCount > 0 ? "attention" : "healthy",
      scannedModCount: summaries.length,
      completedModCount,
      attentionModCount,
      unknownModCount,
      managedFileCount,
      backupCount,
      issueCount,
      issues: issueDisplayOrder.flatMap((issue) => {
        const count = issueCounts.get(issue) ?? 0;
        return count > 0 ? [{ issue, count, label: issueLabels[issue] }] : [];
      }),
    },
    mods,
  };
}

function withIssueLabels(issues: InstallRecoveryIssueSummary[]): RecoveryCenterIssueView[] {
  const byIssue = new Map(issues.map((issue) => [issue.issue, issue.count]));

  return issueDisplayOrder.flatMap((issue) => {
    const count = byIssue.get(issue) ?? 0;
    return count > 0 ? [{ issue, count, label: issueLabels[issue] }] : [];
  });
}

function statusTone(status: InstallRecoveryStatus): RecoveryCenterModView["statusTone"] {
  if (status === "completed") {
    return "healthy";
  }

  if (status === "repair_required") {
    return "attention";
  }

  if (status === "unknown") {
    return "unknown";
  }

  return "empty";
}
