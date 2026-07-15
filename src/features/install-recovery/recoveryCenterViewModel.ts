import type {
  InstallRecoveryIssue,
  InstallRecoveryIssueSummary,
  InstallRecoveryStatus,
  InstallRecoverySummary,
  UnsafeInstallStatus,
} from "../mods/modInstallPlanTypes";

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

const issueMetadata: Record<
  InstallRecoveryIssue,
  {
    label: string;
    severity: RecoveryCenterIssueSeverity;
    guidance: string;
  }
> = {
  missing_installed_file_summary: {
    label: "摘要缺失",
    severity: "unknown",
    guidance: "旧安装缺少写入摘要，不能自动删除或恢复，需等待迁移或人工确认。",
  },
  target_missing: {
    label: "目标缺失",
    severity: "blocking",
    guidance: "暂停自动处理，等待受控恢复或重新安装流程确认缺失目标。",
  },
  target_changed: {
    label: "目标变更",
    severity: "blocking",
    guidance: "暂停自动安装/卸载，等待受控恢复或重新安装流程确认目标状态。",
  },
  target_read_failed: {
    label: "读取未知",
    severity: "unknown",
    guidance: "重新扫描；如果仍不可读，先检查权限或占用状态。",
  },
  backup_missing: {
    label: "备份缺失",
    severity: "blocking",
    guidance: "不要自动恢复或卸载，先保留当前文件并进入人工确认。",
  },
  backup_read_failed: {
    label: "备份未知",
    severity: "unknown",
    guidance: "重新扫描；如果备份仍不可读，暂停恢复并保留当前状态。",
  },
};

const statusLabels: Record<InstallRecoveryStatus, string> = {
  completed: "正常",
  not_installed: "未安装",
  committed_cleanup_pending: "重装待收尾",
  cleanup_pending: "恢复待清理",
  rollback_required: "需要回滚",
  repair_required: "需要修复",
  unknown: "状态未知",
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

export function deriveRecoveryCenterViewModel(summaries: InstallRecoverySummary[]): RecoveryCenterViewModel {
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
        statusLabel: statusLabels[summary.status],
        statusTone: statusTone(summary.status),
        managedFileCount: summary.managedFileCount,
        backupCount: summary.backupCount,
        issueCount: summary.issueCount,
        issues: withIssueLabels(summary.issues),
        repairSummary: deriveModRepairSummary(summary),
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
        return count > 0 ? [issueView(issue, count)] : [];
      }),
      repairSummary: deriveOverviewRepairSummary({
        scannedModCount: summaries.length,
        attentionModCount,
        unknownModCount,
      }),
      manualDecision: deriveManualDecision({
        attentionModCount,
        unknownModCount,
        rollbackRequiredModCount,
      }),
    },
    mods,
  };
}

function withIssueLabels(issues: InstallRecoveryIssueSummary[]): RecoveryCenterIssueView[] {
  const byIssue = new Map(issues.map((issue) => [issue.issue, issue.count]));

  return issueDisplayOrder.flatMap((issue) => {
    const count = byIssue.get(issue) ?? 0;
    return count > 0 ? [issueView(issue, count)] : [];
  });
}

function issueView(issue: InstallRecoveryIssue, count: number): RecoveryCenterIssueView {
  const metadata = issueMetadata[issue];
  return {
    issue,
    count,
    label: metadata.label,
    severity: metadata.severity,
    guidance: metadata.guidance,
  };
}

function deriveOverviewRepairSummary(input: {
  scannedModCount: number;
  attentionModCount: number;
  unknownModCount: number;
}): RecoveryCenterRepairSummary {
  if (input.scannedModCount === 0) {
    return {
      status: "clear",
      title: "无需处理",
      description: "当前配置档没有需要恢复中心处理的托管安装状态。",
      actionLabel: "保持观察",
      blockingReason: "没有托管安装记录",
    };
  }

  if (input.unknownModCount > 0) {
    return {
      status: "unknown",
      title: "恢复状态需要人工确认",
      description: "部分托管安装状态无法读取，自动安装、卸载和恢复都应保持阻断。",
      actionLabel: "刷新后仍异常则保留现场并人工处理",
      blockingReason: `存在 ${input.unknownModCount} 个状态未知 Mod 和 ${input.attentionModCount} 个需要修复 Mod`,
    };
  }

  if (input.attentionModCount > 0) {
    return {
      status: "manual_required",
      title: "发现需要人工处理的安装状态",
      description: "恢复中心发现 manifest、目标文件或备份状态不一致，暂不执行自动处理动作。",
      actionLabel: "保留现场，等待受控修复或重新安装流程",
      blockingReason: `存在 ${input.attentionModCount} 个需要修复 Mod`,
    };
  }

  return {
    status: "clear",
    title: "无需处理",
    description: "当前托管安装状态与 manifest 摘要一致。",
    actionLabel: "保持观察",
    blockingReason: "未发现需要阻断的恢复问题",
  };
}

function deriveModRepairSummary(summary: InstallRecoverySummary): RecoveryCenterRepairSummary {
  if (summary.status === "rollback_required") {
    return {
      status: "manual_required",
      title: "需要回滚",
      description: "该 Mod 有未完成写入窗口的持久化恢复记录，自动安装、卸载和恢复动作必须保持阻断。",
      actionLabel: "保留现场，等待受控回滚流程",
      blockingReason: "恢复记录要求回滚",
    };
  }

  if (summary.status === "committed_cleanup_pending") {
    return {
      status: "manual_required",
      title: "重装待收尾",
      description: "新版本已提交，但完成记录尚未收敛。收尾完成前，新的安装、卸载和重装保持阻断。",
      actionLabel: "保留现场，重新扫描或导出诊断",
      blockingReason: "重装提交记录尚未完成收敛",
    };
  }

  if (summary.status === "cleanup_pending") {
    return {
      status: "manual_required",
      title: "恢复待清理",
      description: "重装事务已完成，但恢复快照或事务记录尚未清理。清理完成前，新的安装、卸载和重装保持阻断。",
      actionLabel: "保留现场，重新扫描或导出诊断",
      blockingReason: "重装恢复数据尚待清理",
    };
  }

  if (summary.status === "unknown") {
    return {
      status: "unknown",
      title: "状态未知",
      description: "该 Mod 的目标或备份状态无法确认，不能自动安装、卸载或恢复。",
      actionLabel: "重新扫描后仍异常则人工处理",
      blockingReason: summary.issueCount > 0 ? `检测到 ${summary.issueCount} 个未知恢复问题` : "恢复状态不可确认",
    };
  }

  if (summary.status === "repair_required") {
    return {
      status: "manual_required",
      title: "需要人工处理",
      description: "该 Mod 的受控安装事实与当前状态不一致，自动破坏性操作应保持阻断。",
      actionLabel: "保留现场，等待受控恢复或重新安装流程",
      blockingReason: summary.issueCount > 0 ? `检测到 ${summary.issueCount} 个恢复问题` : "恢复扫描要求人工确认",
    };
  }

  if (summary.status === "not_installed") {
    return {
      status: "clear",
      title: "未安装",
      description: "当前 profile 没有该 Mod 的托管安装记录。",
      actionLabel: "无需处理",
      blockingReason: "未发现托管安装事实",
    };
  }

  return {
    status: "clear",
    title: "状态正常",
    description: "该 Mod 的托管安装摘要与当前状态一致。",
    actionLabel: "无需处理",
    blockingReason: "未发现恢复问题",
  };
}

function deriveManualDecision(input: {
  attentionModCount: number;
  unknownModCount: number;
  rollbackRequiredModCount: number;
}): RecoveryCenterManualDecision {
  const hasBlockedState = input.attentionModCount > 0 || input.unknownModCount > 0;

  if (!hasBlockedState) {
    return {
      status: "clear",
      title: "无需人工处理",
      description: "当前没有需要恢复中心人工处理的托管安装状态。",
      recommendedAction: "保持观察。",
      safeguards: [],
      actions: safeManualActions(),
    };
  }

  const actions = safeManualActions();

  if (input.rollbackRequiredModCount > 0) {
    actions.push({
      id: "controlled_recovery",
      label: "受控回滚",
      description: `${input.rollbackRequiredModCount} 个 Mod 可在下方列表中使用逐 Mod 受控回滚。`,
      state: "available",
    });
  } else {
    actions.push({
      id: "controlled_recovery",
      label: "受控修复",
      description: "当前没有可执行受控回滚的 Mod，请保留现场并等待后续恢复能力。",
      state: "unavailable",
    });
  }

  return {
    status: "blocked",
    title: "需要人工处理",
    description: "恢复中心已阻断自动安装、卸载和恢复动作，当前只能执行只读复查或导出诊断。",
    recommendedAction: input.rollbackRequiredModCount > 0
      ? "在下方 Mod 列表中对需要回滚的 Mod 使用受控回滚按钮。"
      : "先重新扫描；如果仍异常，导出诊断并保留现场。",
    safeguards: [
      "不删除未知文件",
      "不根据当前 Mod 包猜测恢复动作",
      "不写入 manifest 或 backup 状态",
    ],
    actions,
  };
}

function safeManualActions(): RecoveryCenterManualAction[] {
  return [
    {
      id: "retry_scan",
      label: "重新扫描",
      description: "重新读取后端只读恢复摘要。",
      state: "available",
    },
    {
      id: "export_diagnostics",
      label: "导出诊断",
      description: "生成已脱敏的支持诊断包。",
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
