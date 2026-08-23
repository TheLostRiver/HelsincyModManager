import type { LocaleDictionary } from "../../shared/i18n";
import type { InstallRecoveryIssue, InstallRecoveryStatus } from "../mods/modInstallPlanTypes";
import type { RecoveryRollbackPhase } from "./useRecoveryRollback";

// 恢复中心（页面、聚合/逐 Mod 视图模型、受控回滚、诊断导出、全局告警）的
// 全部用户可见文案。语义（severity/排序/状态推进）留在 viewModel 与 hooks，
// 文本在派生或渲染时经本字典取。

export type RecoveryRepairSummaryCopy = {
  title: string;
  description: string;
  actionLabel: string;
};

export type RecoveryCenterCopy = {
  issues: Record<InstallRecoveryIssue, { label: string; guidance: string }>;
  status: Record<InstallRecoveryStatus, string>;
  overviewRepair: {
    empty: RecoveryRepairSummaryCopy & { blockingReason: string };
    unknown: RecoveryRepairSummaryCopy & {
      blockingReason: (unknownCount: number, attentionCount: number) => string;
    };
    manualRequired: RecoveryRepairSummaryCopy & { blockingReason: (attentionCount: number) => string };
    clear: RecoveryRepairSummaryCopy & { blockingReason: string };
  };
  modRepair: {
    rollbackRequired: RecoveryRepairSummaryCopy & { blockingReason: string };
    committedCleanupPending: RecoveryRepairSummaryCopy & { blockingReason: string };
    cleanupPending: RecoveryRepairSummaryCopy & { blockingReason: string };
    unknown: RecoveryRepairSummaryCopy & {
      blockingReasonWithIssues: (issueCount: number) => string;
      blockingReasonDefault: string;
    };
    repairRequired: RecoveryRepairSummaryCopy & {
      blockingReasonWithIssues: (issueCount: number) => string;
      blockingReasonDefault: string;
    };
    notInstalled: RecoveryRepairSummaryCopy & { blockingReason: string };
    clear: RecoveryRepairSummaryCopy & { blockingReason: string };
  };
  manualDecision: {
    clearTitle: string;
    clearDescription: string;
    clearRecommended: string;
    blockedTitle: string;
    blockedDescription: string;
    recommendedRollback: string;
    recommendedRescan: string;
    safeguards: string[];
    retryScanLabel: string;
    retryScanDescription: string;
    exportDiagnosticsLabel: string;
    exportDiagnosticsDescription: string;
    controlledRollbackLabel: string;
    controlledRollbackDescription: (count: number) => string;
    controlledRepairLabel: string;
    controlledRepairUnavailableDescription: string;
  };
  rollback: {
    phases: Record<RecoveryRollbackPhase, string>;
    failures: {
      profileNotReady: string;
      previewFailed: string;
      startFailed: string;
      taskFallback: string;
    };
  };
  blockReasons: Record<string, string>;
  page: {
    eyebrow: string;
    title: string;
    subtitle: string;
    exporting: string;
    exportDiagnostics: string;
    refresh: string;
    rollbackPanel: {
      statusAria: string;
      progressAria: string;
      previewingTitle: string;
      startingTitle: string;
      blockedTitle: string;
      blockedDetail: (modId: string) => string;
      close: string;
      confirmTitle: string;
      confirmBody: (modId: string) => string;
      confirmAction: string;
      cancel: string;
      completedTitle: string;
      completedBody: (modId: string) => string;
      failedTitle: string;
      failedBody: (modId: string, message: string) => string;
      statsRemove: (count: number) => string;
      statsRestore: (count: number) => string;
      statsBackups: (count: number) => string;
    };
    diagnostics: {
      statusAria: string;
      confirmTitle: string;
      confirmBody: string;
      bulletContents: string;
      bulletPrivacy: string;
      start: string;
      cancel: string;
      exportingTitle: string;
      exportingBody: string;
    };
    notConfigured: { title: string; body: string };
    loading: { aria: string; title: string; body: string };
    unavailable: { title: string; body: string };
    overview: {
      emptyTitle: string;
      emptyBadge: string;
      emptyDescription: string;
      attentionTitle: string;
      attentionBadge: string;
      attentionDescriptionUnknown: string;
      attentionDescriptionManual: string;
      healthyTitle: string;
      healthyBadge: string;
      healthyDescription: (count: number) => string;
    };
    metricsAria: string;
    metricScanned: string;
    metricCompleted: string;
    metricAttention: string;
    metricUnknown: string;
    metricManagedFiles: string;
    metricIssues: string;
    manualAria: string;
    issuesAggregateAria: string;
    modIssuesAria: (modId: string) => string;
    modsTitle: string;
    modsCount: (count: number) => string;
    modEmpty: string;
    modMetricsAria: (modId: string) => string;
    modRollbackBusy: string;
    modRollbackAction: string;
    modFiles: (count: number) => string;
    modBackups: (count: number) => string;
    modIssues: (count: number) => string;
    repairAria: string;
    repairBlockingReason: string;
    repairNextStep: string;
  };
  globalAlert: {
    panelAria: string;
    unavailableTitle: string;
    unavailableDescription: string;
    attentionTitle: string;
    attentionDescription: (summary: string) => string;
    openRecoveryCenter: string;
    partAttention: (count: number) => string;
    partUnknown: (count: number) => string;
    partIssues: (count: number) => string;
    partJoin: string;
    fallbackSummary: string;
  };
  diagnosticsToasts: {
    exportedTitle: string;
    exportedMessage: (facts: {
      fileName: string;
      size: string;
      appLogLineCount: number;
      debugLogLineCount: number;
      taskLogLineCount: number;
      auditEventCount: number;
    }) => string;
    failedTitle: string;
    failedMessage: string;
  };
};

export const recoveryCenterCopy = {
  zh_cn: {
    issues: {
      missing_installed_file_summary: {
        label: "摘要缺失",
        guidance: "旧安装缺少写入摘要，不能自动删除或恢复，需等待迁移或人工确认。",
      },
      target_missing: {
        label: "目标缺失",
        guidance: "暂停自动处理，等待受控恢复或重新安装流程确认缺失目标。",
      },
      target_changed: {
        label: "目标变更",
        guidance: "暂停自动安装/卸载，等待受控恢复或重新安装流程确认目标状态。",
      },
      target_read_failed: {
        label: "读取未知",
        guidance: "重新扫描；如果仍不可读，先检查权限或占用状态。",
      },
      backup_missing: {
        label: "备份缺失",
        guidance: "不要自动恢复或卸载，先保留当前文件并进入人工确认。",
      },
      backup_read_failed: {
        label: "备份未知",
        guidance: "重新扫描；如果备份仍不可读，暂停恢复并保留当前状态。",
      },
    },
    status: {
      completed: "正常",
      not_installed: "未安装",
      committed_cleanup_pending: "重装待收尾",
      cleanup_pending: "恢复待清理",
      rollback_required: "需要回滚",
      repair_required: "需要修复",
      unknown: "状态未知",
    },
    overviewRepair: {
      empty: {
        title: "无需处理",
        description: "当前配置档没有需要恢复中心处理的托管安装状态。",
        actionLabel: "保持观察",
        blockingReason: "没有托管安装记录",
      },
      unknown: {
        title: "恢复状态需要人工确认",
        description: "部分托管安装状态无法读取，自动安装、卸载和恢复都应保持阻断。",
        actionLabel: "刷新后仍异常则保留现场并人工处理",
        blockingReason: (unknownCount: number, attentionCount: number) =>
          `存在 ${unknownCount} 个状态未知 Mod 和 ${attentionCount} 个需要修复 Mod`,
      },
      manualRequired: {
        title: "发现需要人工处理的安装状态",
        description: "恢复中心发现 manifest、目标文件或备份状态不一致，暂不执行自动处理动作。",
        actionLabel: "保留现场，等待受控修复或重新安装流程",
        blockingReason: (attentionCount: number) => `存在 ${attentionCount} 个需要修复 Mod`,
      },
      clear: {
        title: "无需处理",
        description: "当前托管安装状态与 manifest 摘要一致。",
        actionLabel: "保持观察",
        blockingReason: "未发现需要阻断的恢复问题",
      },
    },
    modRepair: {
      rollbackRequired: {
        title: "需要回滚",
        description: "该 Mod 有未完成写入窗口的持久化恢复记录，自动安装、卸载和恢复动作必须保持阻断。",
        actionLabel: "保留现场，等待受控回滚流程",
        blockingReason: "恢复记录要求回滚",
      },
      committedCleanupPending: {
        title: "重装待收尾",
        description: "新版本已提交，但完成记录尚未收敛。收尾完成前，新的安装、卸载和重装保持阻断。",
        actionLabel: "保留现场，重新扫描或导出诊断",
        blockingReason: "重装提交记录尚未完成收敛",
      },
      cleanupPending: {
        title: "恢复待清理",
        description: "重装事务已完成，但恢复快照或事务记录尚未清理。清理完成前，新的安装、卸载和重装保持阻断。",
        actionLabel: "保留现场，重新扫描或导出诊断",
        blockingReason: "重装恢复数据尚待清理",
      },
      unknown: {
        title: "状态未知",
        description: "该 Mod 的目标或备份状态无法确认，不能自动安装、卸载或恢复。",
        actionLabel: "重新扫描后仍异常则人工处理",
        blockingReasonWithIssues: (issueCount: number) => `检测到 ${issueCount} 个未知恢复问题`,
        blockingReasonDefault: "恢复状态不可确认",
      },
      repairRequired: {
        title: "需要人工处理",
        description: "该 Mod 的受控安装事实与当前状态不一致，自动破坏性操作应保持阻断。",
        actionLabel: "保留现场，等待受控恢复或重新安装流程",
        blockingReasonWithIssues: (issueCount: number) => `检测到 ${issueCount} 个恢复问题`,
        blockingReasonDefault: "恢复扫描要求人工确认",
      },
      notInstalled: {
        title: "未安装",
        description: "当前 profile 没有该 Mod 的托管安装记录。",
        actionLabel: "无需处理",
        blockingReason: "未发现托管安装事实",
      },
      clear: {
        title: "状态正常",
        description: "该 Mod 的托管安装摘要与当前状态一致。",
        actionLabel: "无需处理",
        blockingReason: "未发现恢复问题",
      },
    },
    manualDecision: {
      clearTitle: "无需人工处理",
      clearDescription: "当前没有需要恢复中心人工处理的托管安装状态。",
      clearRecommended: "保持观察。",
      blockedTitle: "需要人工处理",
      blockedDescription: "恢复中心已阻断自动安装、卸载和恢复动作，当前只能执行只读复查或导出诊断。",
      recommendedRollback: "在下方 Mod 列表中对需要回滚的 Mod 使用受控回滚按钮。",
      recommendedRescan: "先重新扫描；如果仍异常，导出诊断并保留现场。",
      safeguards: [
        "不删除未知文件",
        "不根据当前 Mod 包猜测恢复动作",
        "不写入 manifest 或 backup 状态",
      ],
      retryScanLabel: "重新扫描",
      retryScanDescription: "重新读取后端只读恢复摘要。",
      exportDiagnosticsLabel: "导出诊断",
      exportDiagnosticsDescription: "生成已脱敏的支持诊断包。",
      controlledRollbackLabel: "受控回滚",
      controlledRollbackDescription: (count: number) =>
        `${count} 个 Mod 可在下方列表中使用逐 Mod 受控回滚。`,
      controlledRepairLabel: "受控修复",
      controlledRepairUnavailableDescription: "当前没有可执行受控回滚的 Mod，请保留现场并等待后续恢复能力。",
    },
    rollback: {
      phases: {
        "install.recovery.queued": "排队中",
        "install.recovery.planning": "分析中",
        "install.recovery.processing": "回滚中",
        "install.recovery.completed": "回滚完成",
        "install.recovery.failed": "回滚失败",
      },
      failures: {
        profileNotReady: "配置档尚未就绪",
        previewFailed: "预览回滚动作时出错",
        startFailed: "启动回滚任务时出错",
        taskFallback: "回滚失败",
      },
    },
    blockReasons: {
      rollback_state_missing: "回滚状态缺失",
      missing_installed_file_summary: "摘要缺失",
      target_missing: "目标缺失",
      target_changed: "目标变更",
      target_read_failed: "目标读取失败",
      backup_missing: "备份缺失",
      backup_read_failed: "备份读取失败",
    },
    page: {
      eyebrow: "受控恢复中心",
      title: "恢复中心",
      subtitle: "查看当前配置档的托管安装健康状态，先定位需要人工处理的条目。",
      exporting: "导出中",
      exportDiagnostics: "导出诊断",
      refresh: "刷新",
      rollbackPanel: {
        statusAria: "回滚状态",
        progressAria: "回滚进度",
        previewingTitle: "正在检查回滚条件",
        startingTitle: "正在启动回滚任务",
        blockedTitle: "受控回滚不可执行",
        blockedDetail: (modId: string) => `${modId} — 后端预检发现阻断条件，当前无法安全回滚。`,
        close: "关闭",
        confirmTitle: "确认受控回滚",
        confirmBody: (modId: string) => `将对 ${modId} 执行受控回滚，恢复到安装前状态。`,
        confirmAction: "确认回滚",
        cancel: "取消",
        completedTitle: "回滚完成",
        completedBody: (modId: string) => `${modId} 已恢复到安装前状态。已触发重新扫描。`,
        failedTitle: "回滚失败",
        failedBody: (modId: string, message: string) => `${modId} — ${message}`,
        statsRemove: (count: number) => `将删除 ${count} 个文件`,
        statsRestore: (count: number) => `将恢复 ${count} 个文件`,
        statsBackups: (count: number) => `涉及 ${count} 个备份`,
      },
      diagnostics: {
        statusAria: "诊断导出状态",
        confirmTitle: "确认导出诊断包",
        confirmBody: "导出包会由后端生成已脱敏的支持材料，页面只显示安全摘要。",
        bulletContents: "包含平台摘要、已校验 App 日志、已校验任务日志和已校验审计事件。",
        bulletPrivacy: "页面不展示日志正文、审计正文、本地路径或第三方 Mod 内容。",
        start: "开始导出",
        cancel: "取消",
        exportingTitle: "正在导出诊断包",
        exportingBody: "正在生成已脱敏的支持诊断摘要。",
      },
      notConfigured: {
        title: "等待游戏目录配置",
        body: "恢复中心需要先有受控游戏实例，才能读取当前配置档的托管安装摘要。",
      },
      loading: {
        aria: "恢复扫描状态",
        title: "正在读取恢复摘要",
        body: "正在从后端读取当前配置档的托管安装状态。",
      },
      unavailable: {
        title: "恢复摘要不可用",
        body: "无法确认当前托管安装状态。请稍后刷新，或先回到 Mod 管理页避免继续安装/卸载。",
      },
      overview: {
        emptyTitle: "没有托管安装记录",
        emptyBadge: "空记录",
        emptyDescription: "当前配置档还没有由 Helsincy 托管的安装项。",
        attentionTitle: "发现需要关注的安装状态",
        attentionBadge: "需要处理",
        attentionDescriptionUnknown: "部分托管安装状态无法确认，恢复中心会先阻断自动处理动作。",
        attentionDescriptionManual: "部分托管安装状态需要人工处理，自动安装/卸载入口应保持阻断。",
        healthyTitle: "托管安装状态正常",
        healthyBadge: "正常",
        healthyDescription: (count: number) => `${count} 个托管 Mod 与 manifest 摘要一致。`,
      },
      metricsAria: "恢复扫描聚合摘要",
      metricScanned: "扫描 Mod",
      metricCompleted: "状态正常",
      metricAttention: "需处理",
      metricUnknown: "未知",
      metricManagedFiles: "托管文件",
      metricIssues: "问题",
      manualAria: "人工处理决策",
      issuesAggregateAria: "恢复问题聚合",
      modIssuesAria: (modId: string) => `${modId} 恢复问题`,
      modsTitle: "托管 Mod 状态",
      modsCount: (count: number) => `${count} 项`,
      modEmpty: "当前配置档没有托管安装记录。",
      modMetricsAria: (modId: string) => `${modId} 恢复摘要`,
      modRollbackBusy: "处理中",
      modRollbackAction: "回滚",
      modFiles: (count: number) => `${count} 文件`,
      modBackups: (count: number) => `${count} 备份`,
      modIssues: (count: number) => `${count} 问题`,
      repairAria: "恢复处理摘要",
      repairBlockingReason: "阻断原因",
      repairNextStep: "下一步",
    },
    globalAlert: {
      panelAria: "安装恢复全局告警",
      unavailableTitle: "恢复摘要暂时不可用",
      unavailableDescription: "无法确认当前配置档的托管安装状态。进入恢复中心后可重新扫描或导出诊断摘要。",
      attentionTitle: "托管安装需要处理",
      attentionDescription: (summary: string) =>
        `当前配置档扫描到 ${summary}。恢复中心只会展示安全摘要，不会自动恢复或写入清单。`,
      openRecoveryCenter: "打开恢复中心",
      partAttention: (count: number) => `${count} 个需处理`,
      partUnknown: (count: number) => `${count} 个状态未知`,
      partIssues: (count: number) => `${count} 个问题`,
      partJoin: "，",
      fallbackSummary: "存在需要关注的托管安装状态",
    },
    diagnosticsToasts: {
      exportedTitle: "诊断包已导出",
      exportedMessage: (facts) =>
        `${facts.fileName}，${facts.size}；App 日志 ${facts.appLogLineCount} 行，Debug 日志 ${facts.debugLogLineCount} 行，任务日志 ${facts.taskLogLineCount} 行，审计事件 ${facts.auditEventCount} 条。`,
      failedTitle: "诊断导出失败",
      failedMessage: "诊断包暂时不可用，请稍后重试并保留当前恢复中心状态。",
    },
  },
  en: {
    issues: {
      missing_installed_file_summary: {
        label: "Summary missing",
        guidance: "A legacy install lacks its write summary; it cannot be auto-deleted or restored. Wait for migration or confirm manually.",
      },
      target_missing: {
        label: "Target missing",
        guidance: "Pause automatic handling and let the controlled recovery or reinstall flow confirm the missing target.",
      },
      target_changed: {
        label: "Target changed",
        guidance: "Pause automatic install/uninstall and let the controlled recovery or reinstall flow confirm the target state.",
      },
      target_read_failed: {
        label: "Read unknown",
        guidance: "Rescan; if it still cannot be read, check permissions or file locks first.",
      },
      backup_missing: {
        label: "Backup missing",
        guidance: "Do not auto-restore or uninstall. Keep the current files and move to manual confirmation.",
      },
      backup_read_failed: {
        label: "Backup unknown",
        guidance: "Rescan; if the backup still cannot be read, pause restores and keep the current state.",
      },
    },
    status: {
      completed: "Healthy",
      not_installed: "Not installed",
      committed_cleanup_pending: "Reinstall finalizing",
      cleanup_pending: "Recovery cleanup pending",
      rollback_required: "Rollback required",
      repair_required: "Repair required",
      unknown: "State unknown",
    },
    overviewRepair: {
      empty: {
        title: "Nothing to handle",
        description: "The current profile has no managed install state that needs the Recovery Center.",
        actionLabel: "Keep observing",
        blockingReason: "No managed install records",
      },
      unknown: {
        title: "Recovery state needs manual confirmation",
        description: "Some managed install states cannot be read. Automatic install, uninstall, and restore should stay blocked.",
        actionLabel: "If still abnormal after refresh, preserve the scene and handle manually",
        blockingReason: (unknownCount: number, attentionCount: number) =>
          `${unknownCount} mod(s) with unknown state and ${attentionCount} mod(s) needing repair`,
      },
      manualRequired: {
        title: "Install states need manual handling",
        description: "The Recovery Center found manifest, target file, or backup inconsistencies. Automatic actions are on hold.",
        actionLabel: "Preserve the scene and wait for the controlled repair or reinstall flow",
        blockingReason: (attentionCount: number) => `${attentionCount} mod(s) needing repair`,
      },
      clear: {
        title: "Nothing to handle",
        description: "Managed install states match the manifest summaries.",
        actionLabel: "Keep observing",
        blockingReason: "No blocking recovery issues found",
      },
    },
    modRepair: {
      rollbackRequired: {
        title: "Rollback required",
        description: "This mod has a persisted recovery record from an unfinished write window. Automatic install, uninstall, and restore actions must stay blocked.",
        actionLabel: "Preserve the scene and wait for the controlled rollback flow",
        blockingReason: "The recovery record requires a rollback",
      },
      committedCleanupPending: {
        title: "Reinstall finalizing",
        description: "The new version is committed, but the completion record has not converged. New installs, uninstalls, and reinstalls stay blocked until finalization.",
        actionLabel: "Preserve the scene; rescan or export diagnostics",
        blockingReason: "The reinstall commit record has not finished converging",
      },
      cleanupPending: {
        title: "Recovery cleanup pending",
        description: "The reinstall transaction finished, but its recovery snapshot or transaction record is not cleaned up yet. New installs, uninstalls, and reinstalls stay blocked until cleanup.",
        actionLabel: "Preserve the scene; rescan or export diagnostics",
        blockingReason: "Reinstall recovery data is pending cleanup",
      },
      unknown: {
        title: "State unknown",
        description: "The target or backup state of this mod cannot be confirmed. It cannot be automatically installed, uninstalled, or restored.",
        actionLabel: "If still abnormal after a rescan, handle manually",
        blockingReasonWithIssues: (issueCount: number) => `${issueCount} unknown recovery issue(s) detected`,
        blockingReasonDefault: "Recovery state cannot be confirmed",
      },
      repairRequired: {
        title: "Manual handling required",
        description: "The controlled install facts of this mod do not match its current state. Automatic destructive actions should stay blocked.",
        actionLabel: "Preserve the scene and wait for the controlled recovery or reinstall flow",
        blockingReasonWithIssues: (issueCount: number) => `${issueCount} recovery issue(s) detected`,
        blockingReasonDefault: "The recovery scan requires manual confirmation",
      },
      notInstalled: {
        title: "Not installed",
        description: "The current profile has no managed install record for this mod.",
        actionLabel: "Nothing to do",
        blockingReason: "No managed install facts found",
      },
      clear: {
        title: "Healthy",
        description: "The managed install summary of this mod matches its current state.",
        actionLabel: "Nothing to do",
        blockingReason: "No recovery issues found",
      },
    },
    manualDecision: {
      clearTitle: "No manual handling needed",
      clearDescription: "There is no managed install state that needs manual handling in the Recovery Center.",
      clearRecommended: "Keep observing.",
      blockedTitle: "Manual handling required",
      blockedDescription: "The Recovery Center has blocked automatic install, uninstall, and restore actions. Only read-only review and diagnostics export are available.",
      recommendedRollback: "Use the controlled rollback button on the mods below that require a rollback.",
      recommendedRescan: "Rescan first; if still abnormal, export diagnostics and preserve the scene.",
      safeguards: [
        "Never delete unknown files",
        "Never guess recovery actions from the current mod package",
        "Never write manifest or backup state",
      ],
      retryScanLabel: "Rescan",
      retryScanDescription: "Re-read the backend read-only recovery summaries.",
      exportDiagnosticsLabel: "Export diagnostics",
      exportDiagnosticsDescription: "Generate a redacted support diagnostics bundle.",
      controlledRollbackLabel: "Controlled rollback",
      controlledRollbackDescription: (count: number) =>
        `${count} mod(s) can use per-mod controlled rollback in the list below.`,
      controlledRepairLabel: "Controlled repair",
      controlledRepairUnavailableDescription: "No mod is currently eligible for controlled rollback. Preserve the scene and wait for upcoming recovery capabilities.",
    },
    rollback: {
      phases: {
        "install.recovery.queued": "Queued",
        "install.recovery.planning": "Analyzing",
        "install.recovery.processing": "Rolling back",
        "install.recovery.completed": "Rollback completed",
        "install.recovery.failed": "Rollback failed",
      },
      failures: {
        profileNotReady: "The profile is not ready yet",
        previewFailed: "Failed to preview the rollback action",
        startFailed: "Failed to start the rollback task",
        taskFallback: "Rollback failed",
      },
    },
    blockReasons: {
      rollback_state_missing: "Rollback state missing",
      missing_installed_file_summary: "Summary missing",
      target_missing: "Target missing",
      target_changed: "Target changed",
      target_read_failed: "Target read failed",
      backup_missing: "Backup missing",
      backup_read_failed: "Backup read failed",
    },
    page: {
      eyebrow: "Controlled Recovery Center",
      title: "Recovery Center",
      subtitle: "Review the managed install health of the current profile and locate items that need manual handling first.",
      exporting: "Exporting",
      exportDiagnostics: "Export diagnostics",
      refresh: "Refresh",
      rollbackPanel: {
        statusAria: "Rollback status",
        progressAria: "Rollback progress",
        previewingTitle: "Checking rollback preconditions",
        startingTitle: "Starting rollback task",
        blockedTitle: "Controlled rollback unavailable",
        blockedDetail: (modId: string) => `${modId} — the backend preflight found blocking conditions; a safe rollback is not possible right now.`,
        close: "Close",
        confirmTitle: "Confirm controlled rollback",
        confirmBody: (modId: string) => `${modId} will be rolled back to its pre-install state via controlled rollback.`,
        confirmAction: "Confirm rollback",
        cancel: "Cancel",
        completedTitle: "Rollback completed",
        completedBody: (modId: string) => `${modId} was restored to its pre-install state. A rescan has been triggered.`,
        failedTitle: "Rollback failed",
        failedBody: (modId: string, message: string) => `${modId} — ${message}`,
        statsRemove: (count: number) => `${count} file(s) will be removed`,
        statsRestore: (count: number) => `${count} file(s) will be restored`,
        statsBackups: (count: number) => `${count} backup(s) involved`,
      },
      diagnostics: {
        statusAria: "Diagnostics export status",
        confirmTitle: "Confirm diagnostics export",
        confirmBody: "The backend generates a redacted support bundle; this page only shows a safe summary.",
        bulletContents: "Includes the platform summary, verified app logs, verified task logs, and verified audit events.",
        bulletPrivacy: "The page never shows log bodies, audit bodies, local paths, or third-party mod content.",
        start: "Start export",
        cancel: "Cancel",
        exportingTitle: "Exporting diagnostics bundle",
        exportingBody: "Generating the redacted support diagnostics summary.",
      },
      notConfigured: {
        title: "Waiting for game directory setup",
        body: "The Recovery Center needs a controlled game instance before it can read the managed install summaries of the current profile.",
      },
      loading: {
        aria: "Recovery scan status",
        title: "Reading recovery summaries",
        body: "Reading the managed install state of the current profile from the backend.",
      },
      unavailable: {
        title: "Recovery summaries unavailable",
        body: "The managed install state cannot be confirmed. Refresh later, or go back to the mod library and avoid further installs/uninstalls.",
      },
      overview: {
        emptyTitle: "No managed install records",
        emptyBadge: "Empty",
        emptyDescription: "The current profile has no installs managed by Helsincy yet.",
        attentionTitle: "Install states need attention",
        attentionBadge: "Action needed",
        attentionDescriptionUnknown: "Some managed install states cannot be confirmed. The Recovery Center blocks automatic handling first.",
        attentionDescriptionManual: "Some managed install states need manual handling. Automatic install/uninstall entries should stay blocked.",
        healthyTitle: "Managed installs are healthy",
        healthyBadge: "Healthy",
        healthyDescription: (count: number) => `${count} managed mod(s) match their manifest summaries.`,
      },
      metricsAria: "Recovery scan aggregate summary",
      metricScanned: "Scanned mods",
      metricCompleted: "Healthy",
      metricAttention: "Action needed",
      metricUnknown: "Unknown",
      metricManagedFiles: "Managed files",
      metricIssues: "Issues",
      manualAria: "Manual handling decision",
      issuesAggregateAria: "Aggregated recovery issues",
      modIssuesAria: (modId: string) => `Recovery issues of ${modId}`,
      modsTitle: "Managed mod states",
      modsCount: (count: number) => `${count} item(s)`,
      modEmpty: "The current profile has no managed install records.",
      modMetricsAria: (modId: string) => `Recovery summary of ${modId}`,
      modRollbackBusy: "Working",
      modRollbackAction: "Roll back",
      modFiles: (count: number) => `${count} file(s)`,
      modBackups: (count: number) => `${count} backup(s)`,
      modIssues: (count: number) => `${count} issue(s)`,
      repairAria: "Recovery handling summary",
      repairBlockingReason: "Blocking reason",
      repairNextStep: "Next step",
    },
    globalAlert: {
      panelAria: "Install recovery global alert",
      unavailableTitle: "Recovery summaries temporarily unavailable",
      unavailableDescription: "The managed install state of the current profile cannot be confirmed. Open the Recovery Center to rescan or export diagnostics.",
      attentionTitle: "Managed installs need handling",
      attentionDescription: (summary: string) =>
        `The current profile scan found ${summary}. The Recovery Center only shows safe summaries and never auto-restores or writes manifests.`,
      openRecoveryCenter: "Open Recovery Center",
      partAttention: (count: number) => `${count} needing action`,
      partUnknown: (count: number) => `${count} with unknown state`,
      partIssues: (count: number) => `${count} issue(s)`,
      partJoin: ", ",
      fallbackSummary: "managed install states that need attention",
    },
    diagnosticsToasts: {
      exportedTitle: "Diagnostics bundle exported",
      exportedMessage: (facts) =>
        `${facts.fileName}, ${facts.size}; app log ${facts.appLogLineCount} lines, debug log ${facts.debugLogLineCount} lines, task log ${facts.taskLogLineCount} lines, audit events ${facts.auditEventCount}.`,
      failedTitle: "Diagnostics export failed",
      failedMessage: "The diagnostics bundle is temporarily unavailable. Try again later and keep the current Recovery Center state.",
    },
  },
  ja: {
    issues: {
      missing_installed_file_summary: {
        label: "サマリー欠落",
        guidance: "旧インストールに書き込みサマリーがないため、自動削除・自動復元はできません。移行を待つか人手で確認してください。",
      },
      target_missing: {
        label: "対象欠落",
        guidance: "自動処理を一時停止し、管理された復旧または再インストールフローで欠落対象を確認してください。",
      },
      target_changed: {
        label: "対象変更",
        guidance: "自動インストール/アンインストールを一時停止し、管理された復旧または再インストールフローで対象状態を確認してください。",
      },
      target_read_failed: {
        label: "読み取り不明",
        guidance: "再スキャンしてください。まだ読み取れない場合は、権限や使用中の状態を先に確認してください。",
      },
      backup_missing: {
        label: "バックアップ欠落",
        guidance: "自動復元・自動アンインストールは行わず、現在のファイルを保全して人手確認に進んでください。",
      },
      backup_read_failed: {
        label: "バックアップ不明",
        guidance: "再スキャンしてください。バックアップがまだ読み取れない場合は復元を一時停止し、現状を保全してください。",
      },
    },
    status: {
      completed: "正常",
      not_installed: "未インストール",
      committed_cleanup_pending: "再インストール終了処理待ち",
      cleanup_pending: "復旧クリーンアップ待ち",
      rollback_required: "ロールバックが必要",
      repair_required: "修復が必要",
      unknown: "状態不明",
    },
    overviewRepair: {
      empty: {
        title: "対応不要",
        description: "現在のプロファイルには、リカバリーセンターでの対応が必要な管理対象インストール状態はありません。",
        actionLabel: "経過観察",
        blockingReason: "管理対象インストール記録なし",
      },
      unknown: {
        title: "復旧状態に人手確認が必要",
        description: "一部の管理対象インストール状態を読み取れません。自動インストール・アンインストール・復元は遮断を維持すべきです。",
        actionLabel: "更新後も異常なら現場を保全して人手対応",
        blockingReason: (unknownCount: number, attentionCount: number) =>
          `状態不明の Mod が ${unknownCount} 件、修復が必要な Mod が ${attentionCount} 件あります`,
      },
      manualRequired: {
        title: "人手対応が必要なインストール状態を検出",
        description: "リカバリーセンターは manifest・対象ファイル・バックアップ状態の不整合を検出しました。自動処理は保留します。",
        actionLabel: "現場を保全し、管理された修復または再インストールフローを待つ",
        blockingReason: (attentionCount: number) => `修復が必要な Mod が ${attentionCount} 件あります`,
      },
      clear: {
        title: "対応不要",
        description: "現在の管理対象インストール状態は manifest サマリーと一致しています。",
        actionLabel: "経過観察",
        blockingReason: "遮断が必要な復旧問題は見つかりませんでした",
      },
    },
    modRepair: {
      rollbackRequired: {
        title: "ロールバックが必要",
        description: "この Mod には未完了の書き込みウィンドウの永続復旧記録があります。自動インストール・アンインストール・復元は遮断を維持しなければなりません。",
        actionLabel: "現場を保全し、管理されたロールバックフローを待つ",
        blockingReason: "復旧記録がロールバックを要求しています",
      },
      committedCleanupPending: {
        title: "再インストール終了処理待ち",
        description: "新バージョンはコミット済みですが、完了記録が収束していません。終了処理まで新規のインストール・アンインストール・再インストールは遮断されます。",
        actionLabel: "現場を保全し、再スキャンまたは診断エクスポート",
        blockingReason: "再インストールのコミット記録が収束していません",
      },
      cleanupPending: {
        title: "復旧クリーンアップ待ち",
        description: "再インストールのトランザクションは完了しましたが、復旧スナップショットまたはトランザクション記録が未整理です。整理完了まで新規のインストール・アンインストール・再インストールは遮断されます。",
        actionLabel: "現場を保全し、再スキャンまたは診断エクスポート",
        blockingReason: "再インストールの復旧データが整理待ちです",
      },
      unknown: {
        title: "状態不明",
        description: "この Mod の対象またはバックアップ状態を確認できないため、自動インストール・アンインストール・復元はできません。",
        actionLabel: "再スキャン後も異常なら人手対応",
        blockingReasonWithIssues: (issueCount: number) => `不明な復旧問題を ${issueCount} 件検出`,
        blockingReasonDefault: "復旧状態を確認できません",
      },
      repairRequired: {
        title: "人手対応が必要",
        description: "この Mod の管理されたインストール事実が現在の状態と一致しません。自動の破壊的操作は遮断を維持すべきです。",
        actionLabel: "現場を保全し、管理された復旧または再インストールフローを待つ",
        blockingReasonWithIssues: (issueCount: number) => `復旧問題を ${issueCount} 件検出`,
        blockingReasonDefault: "復旧スキャンが人手確認を要求しています",
      },
      notInstalled: {
        title: "未インストール",
        description: "現在のプロファイルにはこの Mod の管理対象インストール記録がありません。",
        actionLabel: "対応不要",
        blockingReason: "管理対象インストール事実は見つかりませんでした",
      },
      clear: {
        title: "状態は正常",
        description: "この Mod の管理対象インストールサマリーは現在の状態と一致しています。",
        actionLabel: "対応不要",
        blockingReason: "復旧問題は見つかりませんでした",
      },
    },
    manualDecision: {
      clearTitle: "人手対応は不要",
      clearDescription: "現在、リカバリーセンターでの人手対応が必要な管理対象インストール状態はありません。",
      clearRecommended: "経過観察してください。",
      blockedTitle: "人手対応が必要",
      blockedDescription: "リカバリーセンターは自動インストール・アンインストール・復元を遮断しました。現在は読み取り専用の再確認と診断エクスポートのみ実行できます。",
      recommendedRollback: "下の Mod 一覧で、ロールバックが必要な Mod に管理されたロールバックボタンを使用してください。",
      recommendedRescan: "まず再スキャンし、異常が続く場合は診断をエクスポートして現場を保全してください。",
      safeguards: [
        "不明なファイルを削除しない",
        "現在の Mod パッケージから復旧動作を推測しない",
        "manifest や backup の状態を書き込まない",
      ],
      retryScanLabel: "再スキャン",
      retryScanDescription: "バックエンドの読み取り専用復旧サマリーを再読込します。",
      exportDiagnosticsLabel: "診断をエクスポート",
      exportDiagnosticsDescription: "マスキング済みのサポート診断バンドルを生成します。",
      controlledRollbackLabel: "管理されたロールバック",
      controlledRollbackDescription: (count: number) =>
        `${count} 件の Mod は下の一覧で Mod ごとの管理されたロールバックを使用できます。`,
      controlledRepairLabel: "管理された修復",
      controlledRepairUnavailableDescription: "現在、管理されたロールバックを実行できる Mod はありません。現場を保全し、今後の復旧機能を待ってください。",
    },
    rollback: {
      phases: {
        "install.recovery.queued": "待機中",
        "install.recovery.planning": "分析中",
        "install.recovery.processing": "ロールバック中",
        "install.recovery.completed": "ロールバック完了",
        "install.recovery.failed": "ロールバック失敗",
      },
      failures: {
        profileNotReady: "プロファイルが未準備です",
        previewFailed: "ロールバック動作のプレビューでエラーが発生しました",
        startFailed: "ロールバックタスクの開始でエラーが発生しました",
        taskFallback: "ロールバックに失敗しました",
      },
    },
    blockReasons: {
      rollback_state_missing: "ロールバック状態欠落",
      missing_installed_file_summary: "サマリー欠落",
      target_missing: "対象欠落",
      target_changed: "対象変更",
      target_read_failed: "対象読み取り失敗",
      backup_missing: "バックアップ欠落",
      backup_read_failed: "バックアップ読み取り失敗",
    },
    page: {
      eyebrow: "管理されたリカバリーセンター",
      title: "リカバリーセンター",
      subtitle: "現在のプロファイルの管理対象インストールの健全性を確認し、人手対応が必要な項目を先に特定します。",
      exporting: "エクスポート中",
      exportDiagnostics: "診断をエクスポート",
      refresh: "更新",
      rollbackPanel: {
        statusAria: "ロールバック状態",
        progressAria: "ロールバック進捗",
        previewingTitle: "ロールバック条件を確認中",
        startingTitle: "ロールバックタスクを開始中",
        blockedTitle: "管理されたロールバックは実行不可",
        blockedDetail: (modId: string) => `${modId} — バックエンドの事前チェックで遮断条件が見つかり、現在は安全にロールバックできません。`,
        close: "閉じる",
        confirmTitle: "管理されたロールバックの確認",
        confirmBody: (modId: string) => `${modId} に管理されたロールバックを実行し、インストール前の状態へ戻します。`,
        confirmAction: "ロールバックを確定",
        cancel: "キャンセル",
        completedTitle: "ロールバック完了",
        completedBody: (modId: string) => `${modId} はインストール前の状態へ戻りました。再スキャンを開始しました。`,
        failedTitle: "ロールバック失敗",
        failedBody: (modId: string, message: string) => `${modId} — ${message}`,
        statsRemove: (count: number) => `${count} 件のファイルを削除予定`,
        statsRestore: (count: number) => `${count} 件のファイルを復元予定`,
        statsBackups: (count: number) => `${count} 件のバックアップが関与`,
      },
      diagnostics: {
        statusAria: "診断エクスポート状態",
        confirmTitle: "診断バンドルのエクスポート確認",
        confirmBody: "エクスポートはバックエンドがマスキング済みサポート資料を生成し、この画面は安全なサマリーのみ表示します。",
        bulletContents: "プラットフォームサマリー、検証済み App ログ、検証済みタスクログ、検証済み監査イベントを含みます。",
        bulletPrivacy: "この画面はログ本文・監査本文・ローカルパス・サードパーティ Mod の内容を表示しません。",
        start: "エクスポート開始",
        cancel: "キャンセル",
        exportingTitle: "診断バンドルをエクスポート中",
        exportingBody: "マスキング済みのサポート診断サマリーを生成しています。",
      },
      notConfigured: {
        title: "ゲームディレクトリの設定待ち",
        body: "リカバリーセンターは、管理されたゲームインスタンスがないと現在のプロファイルの管理対象インストールサマリーを読み取れません。",
      },
      loading: {
        aria: "復旧スキャン状態",
        title: "復旧サマリーを読み込み中",
        body: "バックエンドから現在のプロファイルの管理対象インストール状態を読み込んでいます。",
      },
      unavailable: {
        title: "復旧サマリーを利用できません",
        body: "現在の管理対象インストール状態を確認できません。後で更新するか、まず Mod ライブラリへ戻り、これ以上のインストール/アンインストールを避けてください。",
      },
      overview: {
        emptyTitle: "管理対象インストール記録なし",
        emptyBadge: "記録なし",
        emptyDescription: "現在のプロファイルには Helsincy が管理するインストール項目はまだありません。",
        attentionTitle: "注意が必要なインストール状態を検出",
        attentionBadge: "要対応",
        attentionDescriptionUnknown: "一部の管理対象インストール状態を確認できません。リカバリーセンターはまず自動処理を遮断します。",
        attentionDescriptionManual: "一部の管理対象インストール状態に人手対応が必要です。自動インストール/アンインストールの入口は遮断を維持すべきです。",
        healthyTitle: "管理対象インストールは正常",
        healthyBadge: "正常",
        healthyDescription: (count: number) => `${count} 件の管理対象 Mod が manifest サマリーと一致しています。`,
      },
      metricsAria: "復旧スキャン集計サマリー",
      metricScanned: "スキャン済み Mod",
      metricCompleted: "正常",
      metricAttention: "要対応",
      metricUnknown: "不明",
      metricManagedFiles: "管理対象ファイル",
      metricIssues: "問題",
      manualAria: "人手対応の判断",
      issuesAggregateAria: "復旧問題の集計",
      modIssuesAria: (modId: string) => `${modId} の復旧問題`,
      modsTitle: "管理対象 Mod の状態",
      modsCount: (count: number) => `${count} 件`,
      modEmpty: "現在のプロファイルには管理対象インストール記録がありません。",
      modMetricsAria: (modId: string) => `${modId} の復旧サマリー`,
      modRollbackBusy: "処理中",
      modRollbackAction: "ロールバック",
      modFiles: (count: number) => `${count} ファイル`,
      modBackups: (count: number) => `${count} バックアップ`,
      modIssues: (count: number) => `${count} 問題`,
      repairAria: "復旧対応サマリー",
      repairBlockingReason: "遮断理由",
      repairNextStep: "次のステップ",
    },
    globalAlert: {
      panelAria: "インストール復旧のグローバル警告",
      unavailableTitle: "復旧サマリーを一時的に利用できません",
      unavailableDescription: "現在のプロファイルの管理対象インストール状態を確認できません。リカバリーセンターで再スキャンまたは診断エクスポートができます。",
      attentionTitle: "管理対象インストールに対応が必要",
      attentionDescription: (summary: string) =>
        `現在のプロファイルのスキャンで${summary}を検出しました。リカバリーセンターは安全なサマリーのみ表示し、自動復元やマニフェスト書き込みは行いません。`,
      openRecoveryCenter: "リカバリーセンターを開く",
      partAttention: (count: number) => `要対応 ${count} 件`,
      partUnknown: (count: number) => `状態不明 ${count} 件`,
      partIssues: (count: number) => `問題 ${count} 件`,
      partJoin: "、",
      fallbackSummary: "注意が必要な管理対象インストール状態",
    },
    diagnosticsToasts: {
      exportedTitle: "診断バンドルをエクスポート済み",
      exportedMessage: (facts) =>
        `${facts.fileName}、${facts.size}。App ログ ${facts.appLogLineCount} 行、Debug ログ ${facts.debugLogLineCount} 行、タスクログ ${facts.taskLogLineCount} 行、監査イベント ${facts.auditEventCount} 件。`,
      failedTitle: "診断エクスポート失敗",
      failedMessage: "診断バンドルを一時的に利用できません。しばらくしてから再試行し、現在のリカバリーセンター状態を保持してください。",
    },
  },
} satisfies LocaleDictionary<RecoveryCenterCopy>;
