import type {
  InstallRecoveryIssue,
  InstallRecoveryIssueSummary,
  InstallRecoveryStatus,
  InstallRecoverySummary,
  UnsafeInstallStatus,
} from "../mods/modInstallPlanTypes";
import type { RecoveryCenterCopy } from "./recoveryCenterCopy";

export type RecoveryCenterStatus = "empty" | "healthy" | "attention";
export type RecoveryCenterRepairStatus = "clear" | "manual_required" | "unknown";
export type RecoveryCenterIssueSeverity = "blocking" | "unknown";
export type RecoveryCenterManualDecisionStatus = "clear" | "blocked";
export type RecoveryCenterManualActionId = "retry_scan" | "export_diagnostics" | "controlled_recovery";
export type RecoveryCenterManualActionState = "available" | "unavailable";

export type RecoveryCenterIssueView = InstallRecoveryIssueSummary & {
  label: string;
  severity: RecoveryCenterIssueSeverity;
  guidance: string;
};

export type RecoveryCenterRepairSummary = {
  status: RecoveryCenterRepairStatus;
  title: string;
  description: string;
  actionLabel: string;
  blockingReason: string;
};

export type RecoveryCenterManualAction = {
  id: RecoveryCenterManualActionId;
  label: string;
  description: string;
  state: RecoveryCenterManualActionState;
};

export type RecoveryCenterManualDecision = {
  status: RecoveryCenterManualDecisionStatus;
  title: string;
  description: string;
  recommendedAction: string;
  safeguards: string[];
  actions: RecoveryCenterManualAction[];
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
  repairSummary: RecoveryCenterRepairSummary;
  manualDecision: RecoveryCenterManualDecision;
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
  repairSummary: RecoveryCenterRepairSummary;
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

// severity 是语义分级，与展示语言无关；label/guidance 在派生时经 copy 取。
const issueSeverities: Record<InstallRecoveryIssue, RecoveryCenterIssueSeverity> = {
  missing_installed_file_summary: "unknown",
  target_missing: "blocking",
  target_changed: "blocking",
  target_read_failed: "unknown",
  backup_missing: "blocking",
  backup_read_failed: "unknown",
};

const statusSortRank: Record<InstallRecoveryStatus, number> = {
  rollback_required: 0,
  repair_required: 1,
  unknown: 2,
  committed_cleanup_pending: 3,
  cleanup_pending: 4,
  completed: 5,
  not_installed: 6,
};

function isUnsafeInstallStatus(status: string): status is UnsafeInstallStatus {
  return (
    status === "committed_cleanup_pending" ||
    status === "cleanup_pending" ||
    status === "rollback_required" ||
    status === "repair_required" ||
    status === "unknown"
  );
}

export function deriveRecoveryCenterViewModel(
  summaries: InstallRecoverySummary[],
  copy: RecoveryCenterCopy,
): RecoveryCenterViewModel {
  const issueCounts = new Map<InstallRecoveryIssue, number>();
  let completedModCount = 0;
  let attentionModCount = 0;
  let unknownModCount = 0;
  let rollbackRequiredModCount = 0;
  let managedFileCount = 0;
  let backupCount = 0;
  let issueCount = 0;

  const mods = summaries
    .map((summary): RecoveryCenterModView => {
      if (summary.status === "completed") {
        completedModCount += 1;
      } else if (summary.status === "unknown") {
        unknownModCount += 1;
      } else if (isUnsafeInstallStatus(summary.status)) {
        attentionModCount += 1;
        if (summary.status === "rollback_required") {
          rollbackRequiredModCount += 1;
        }
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
        statusLabel: copy.status[summary.status],
        statusTone: statusTone(summary.status),
        managedFileCount: summary.managedFileCount,
        backupCount: summary.backupCount,
        issueCount: summary.issueCount,
        issues: withIssueLabels(summary.issues, copy),
        repairSummary: deriveModRepairSummary(summary, copy),
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
        return count > 0 ? [issueView(issue, count, copy)] : [];
      }),
      repairSummary: deriveOverviewRepairSummary(
        {
          scannedModCount: summaries.length,
          attentionModCount,
          unknownModCount,
        },
        copy,
      ),
      manualDecision: deriveManualDecision(
        {
          attentionModCount,
          unknownModCount,
          rollbackRequiredModCount,
        },
        copy,
      ),
    },
    mods,
  };
}

function withIssueLabels(issues: InstallRecoveryIssueSummary[], copy: RecoveryCenterCopy): RecoveryCenterIssueView[] {
  const byIssue = new Map(issues.map((issue) => [issue.issue, issue.count]));

  return issueDisplayOrder.flatMap((issue) => {
    const count = byIssue.get(issue) ?? 0;
    return count > 0 ? [issueView(issue, count, copy)] : [];
  });
}

function issueView(issue: InstallRecoveryIssue, count: number, copy: RecoveryCenterCopy): RecoveryCenterIssueView {
  return {
    issue,
    count,
    label: copy.issues[issue].label,
    severity: issueSeverities[issue],
    guidance: copy.issues[issue].guidance,
  };
}

function deriveOverviewRepairSummary(
  input: {
    scannedModCount: number;
    attentionModCount: number;
    unknownModCount: number;
  },
  copy: RecoveryCenterCopy,
): RecoveryCenterRepairSummary {
  if (input.scannedModCount === 0) {
    return {
      status: "clear",
      title: copy.overviewRepair.empty.title,
      description: copy.overviewRepair.empty.description,
      actionLabel: copy.overviewRepair.empty.actionLabel,
      blockingReason: copy.overviewRepair.empty.blockingReason,
    };
  }

  if (input.unknownModCount > 0) {
    return {
      status: "unknown",
      title: copy.overviewRepair.unknown.title,
      description: copy.overviewRepair.unknown.description,
      actionLabel: copy.overviewRepair.unknown.actionLabel,
      blockingReason: copy.overviewRepair.unknown.blockingReason(input.unknownModCount, input.attentionModCount),
    };
  }

  if (input.attentionModCount > 0) {
    return {
      status: "manual_required",
      title: copy.overviewRepair.manualRequired.title,
      description: copy.overviewRepair.manualRequired.description,
      actionLabel: copy.overviewRepair.manualRequired.actionLabel,
      blockingReason: copy.overviewRepair.manualRequired.blockingReason(input.attentionModCount),
    };
  }

  return {
    status: "clear",
    title: copy.overviewRepair.clear.title,
    description: copy.overviewRepair.clear.description,
    actionLabel: copy.overviewRepair.clear.actionLabel,
    blockingReason: copy.overviewRepair.clear.blockingReason,
  };
}

function deriveModRepairSummary(summary: InstallRecoverySummary, copy: RecoveryCenterCopy): RecoveryCenterRepairSummary {
  if (summary.status === "rollback_required") {
    return {
      status: "manual_required",
      title: copy.modRepair.rollbackRequired.title,
      description: copy.modRepair.rollbackRequired.description,
      actionLabel: copy.modRepair.rollbackRequired.actionLabel,
      blockingReason: copy.modRepair.rollbackRequired.blockingReason,
    };
  }

  if (summary.status === "committed_cleanup_pending") {
    return {
      status: "manual_required",
      title: copy.modRepair.committedCleanupPending.title,
      description: copy.modRepair.committedCleanupPending.description,
      actionLabel: copy.modRepair.committedCleanupPending.actionLabel,
      blockingReason: copy.modRepair.committedCleanupPending.blockingReason,
    };
  }

  if (summary.status === "cleanup_pending") {
    return {
      status: "manual_required",
      title: copy.modRepair.cleanupPending.title,
      description: copy.modRepair.cleanupPending.description,
      actionLabel: copy.modRepair.cleanupPending.actionLabel,
      blockingReason: copy.modRepair.cleanupPending.blockingReason,
    };
  }

  if (summary.status === "unknown") {
    return {
      status: "unknown",
      title: copy.modRepair.unknown.title,
      description: copy.modRepair.unknown.description,
      actionLabel: copy.modRepair.unknown.actionLabel,
      blockingReason: summary.issueCount > 0
        ? copy.modRepair.unknown.blockingReasonWithIssues(summary.issueCount)
        : copy.modRepair.unknown.blockingReasonDefault,
    };
  }

  if (summary.status === "repair_required") {
    return {
      status: "manual_required",
      title: copy.modRepair.repairRequired.title,
      description: copy.modRepair.repairRequired.description,
      actionLabel: copy.modRepair.repairRequired.actionLabel,
      blockingReason: summary.issueCount > 0
        ? copy.modRepair.repairRequired.blockingReasonWithIssues(summary.issueCount)
        : copy.modRepair.repairRequired.blockingReasonDefault,
    };
  }

  if (summary.status === "not_installed") {
    return {
      status: "clear",
      title: copy.modRepair.notInstalled.title,
      description: copy.modRepair.notInstalled.description,
      actionLabel: copy.modRepair.notInstalled.actionLabel,
      blockingReason: copy.modRepair.notInstalled.blockingReason,
    };
  }

  return {
    status: "clear",
    title: copy.modRepair.clear.title,
    description: copy.modRepair.clear.description,
    actionLabel: copy.modRepair.clear.actionLabel,
    blockingReason: copy.modRepair.clear.blockingReason,
  };
}

function deriveManualDecision(
  input: {
    attentionModCount: number;
    unknownModCount: number;
    rollbackRequiredModCount: number;
  },
  copy: RecoveryCenterCopy,
): RecoveryCenterManualDecision {
  const hasBlockedState = input.attentionModCount > 0 || input.unknownModCount > 0;

  if (!hasBlockedState) {
    return {
      status: "clear",
      title: copy.manualDecision.clearTitle,
      description: copy.manualDecision.clearDescription,
      recommendedAction: copy.manualDecision.clearRecommended,
      safeguards: [],
      actions: safeManualActions(copy),
    };
  }

  const actions = safeManualActions(copy);

  if (input.rollbackRequiredModCount > 0) {
    actions.push({
      id: "controlled_recovery",
      label: copy.manualDecision.controlledRollbackLabel,
      description: copy.manualDecision.controlledRollbackDescription(input.rollbackRequiredModCount),
      state: "available",
    });
  } else {
    actions.push({
      id: "controlled_recovery",
      label: copy.manualDecision.controlledRepairLabel,
      description: copy.manualDecision.controlledRepairUnavailableDescription,
      state: "unavailable",
    });
  }

  return {
    status: "blocked",
    title: copy.manualDecision.blockedTitle,
    description: copy.manualDecision.blockedDescription,
    recommendedAction: input.rollbackRequiredModCount > 0
      ? copy.manualDecision.recommendedRollback
      : copy.manualDecision.recommendedRescan,
    safeguards: [...copy.manualDecision.safeguards],
    actions,
  };
}

function safeManualActions(copy: RecoveryCenterCopy): RecoveryCenterManualAction[] {
  return [
    {
      id: "retry_scan",
      label: copy.manualDecision.retryScanLabel,
      description: copy.manualDecision.retryScanDescription,
      state: "available",
    },
    {
      id: "export_diagnostics",
      label: copy.manualDecision.exportDiagnosticsLabel,
      description: copy.manualDecision.exportDiagnosticsDescription,
      state: "available",
    },
  ];
}

function statusTone(status: InstallRecoveryStatus): RecoveryCenterModView["statusTone"] {
  if (status === "completed") {
    return "healthy";
  }

  if (status === "unknown") {
    return "unknown";
  }

  if (isUnsafeInstallStatus(status)) {
    return "attention";
  }

  return "empty";
}
