import type { LocaleDictionary } from "../../shared/i18n";
import type { BackupCenterStatus, BackupCenterTrigger, SaveBackupRetentionReportDto } from "./backupCenterTypes";

// 备份整理中心（页头、概览、筛选、配置档摘要、备份历史、整理对话框、备注编辑、toast）的
// 全部用户可见文案。BACKUP CENTER / PROFILES / HISTORY 大写 kicker 为设计元素保持英文；
// 触发来源与备份状态表按 tsc 穷尽锁定。

export type BackupCenterCopy = {
  triggers: Record<BackupCenterTrigger, string>;
  statuses: Record<BackupCenterStatus, string>;
  report: {
    outcomes: Record<SaveBackupRetentionReportDto["outcome"], string>;
    evidenceDegradedSuffix: string;
  };
  errors: {
    unavailableFallback: string;
  };
  page: {
    title: string;
    subtitle: string;
    reloadAria: string;
    overviewAria: string;
    metricBackups: string;
    metricSpace: string;
    metricProtected: string;
    metricAttention: string;
    filtersAria: string;
    filterLabel: string;
    filterProfile: string;
    filterAllProfiles: string;
    filterTrigger: string;
    filterAllTriggers: string;
    filterStatus: string;
    filterAllStatuses: string;
    filterSearch: string;
    filterSearchPlaceholder: string;
    profilesAria: string;
    profilesTitle: string;
    profilesCount: (count: number) => string;
    historyAria: string;
    historyTitle: string;
    historyCount: (count: number) => string;
    pagination: (current: number, total: number) => string;
    prevPage: string;
    nextPage: string;
    unavailableTitle: string;
    emptyTitle: string;
    emptyHint: string;
    retry: string;
    loadingPage: string;
  };
  profileCard: {
    activeIdentity: string;
    unboundIdentity: string;
    activeDotAria: string;
    factRecords: string;
    factSpace: string;
    factPolicy: string;
    policyOk: string;
    policyOverBudget: string;
    maintainFailed: string;
    maintaining: string;
    maintainNow: string;
  };
  historyRow: {
    fileCount: (count: number) => string;
    noteAria: string;
    saveNoteAria: string;
    cancelEditAria: string;
    editNoteAria: string;
    restore: string;
    notRestorable: string;
    notRestorableHint: string;
  };
  maintenanceDialog: {
    title: string;
    description: string;
    cancel: string;
    confirm: string;
  };
  toasts: {
    maintenanceMessage: (facts: { scannedCount: number; deletedCount: number; elapsed: string }) => string;
    maintenanceEvidenceSuffix: string;
    maintenanceFailedTitle: string;
    maintenanceFailedMessage: (message: string, elapsed: string) => string;
    noteSavedTitle: string;
    noteSavedMessage: string;
    noteClearedMessage: string;
    noteFailedTitle: string;
  };
};

export const backupCenterCopy = {
  zh_cn: {
    triggers: {
      manual: "手动",
      auto: "自动",
      pre_install: "安装前",
      pre_restore: "恢复前保护",
    },
    statuses: {
      completed: "可恢复",
      retention_pending: "整理中断",
      retention_partial: "清理未完成",
      deleted_by_retention: "已整理",
      missing: "文件缺失",
      invalid: "记录异常",
    },
    report: {
      outcomes: {
        completed: "整理完成",
        within_policy: "已符合保留策略",
        partial: "整理部分完成，下次会继续重试",
        blocked: "整理被保护点阻断",
        failed: "整理失败，未删除备份",
      },
      evidenceDegradedSuffix: "，但审计记录不可用",
    },
    errors: {
      unavailableFallback: "备份中心暂时无法读取，请稍后重试。",
    },
    page: {
      title: "备份整理",
      subtitle: "跨配置档查看备份历史、保护点与整理状态。",
      reloadAria: "重新加载",
      overviewAria: "备份摘要",
      metricBackups: "备份记录",
      metricSpace: "已知空间",
      metricProtected: "保护点",
      metricAttention: "需处理",
      filtersAria: "筛选备份",
      filterLabel: "筛选",
      filterProfile: "配置档",
      filterAllProfiles: "全部配置档",
      filterTrigger: "来源",
      filterAllTriggers: "全部来源",
      filterStatus: "状态",
      filterAllStatuses: "全部状态",
      filterSearch: "搜索备注或配置档",
      filterSearchPlaceholder: "输入关键词",
      profilesAria: "配置档摘要",
      profilesTitle: "配置档摘要",
      profilesCount: (count: number) => `${count} 个`,
      historyAria: "备份历史",
      historyTitle: "备份历史",
      historyCount: (count: number) => `${count} 条`,
      pagination: (current: number, total: number) => `第 ${current} / ${total} 页`,
      prevPage: "上一页",
      nextPage: "下一页",
      unavailableTitle: "备份中心暂时不可用",
      emptyTitle: "暂无符合条件的备份",
      emptyHint: "调整筛选条件后再试。",
      retry: "重试",
      loadingPage: "正在读取备份中心",
    },
    profileCard: {
      activeIdentity: "当前活动配置档",
      unboundIdentity: "未绑定账号摘要",
      activeDotAria: "活动配置档",
      factRecords: "记录",
      factSpace: "空间",
      factPolicy: "策略",
      policyOk: "正常",
      policyOverBudget: "超预算",
      maintainFailed: "整理失败",
      maintaining: "整理中",
      maintainNow: "立即整理",
    },
    historyRow: {
      fileCount: (count: number) => `${count} 个文件`,
      noteAria: "备份备注",
      saveNoteAria: "保存备注",
      cancelEditAria: "取消编辑",
      editNoteAria: "编辑备注",
      restore: "恢复存档",
      notRestorable: "不可恢复",
      notRestorableHint: "只有可恢复的备份点才能恢复",
    },
    maintenanceDialog: {
      title: "确认立即整理备份",
      description: "将按该配置档已保存的保留策略整理普通备份。最新普通备份和恢复前保护点不会被删除，符合数量、年龄或空间规则的普通备份可能被永久删除。此操作不可撤销。",
      cancel: "取消",
      confirm: "确认整理",
    },
    toasts: {
      maintenanceMessage: (facts) =>
        `已扫描 ${facts.scannedCount} 条，删除 ${facts.deletedCount} 条，耗时 ${facts.elapsed}。`,
      maintenanceEvidenceSuffix: " 清理结果已生效，但本次审计证据不完整。",
      maintenanceFailedTitle: "整理失败",
      maintenanceFailedMessage: (message: string, elapsed: string) => `${message} 已耗时 ${elapsed}。`,
      noteSavedTitle: "备注已保存",
      noteSavedMessage: "备份记录已更新。",
      noteClearedMessage: "备份备注已清空。",
      noteFailedTitle: "备注保存失败",
    },
  },
  en: {
    triggers: {
      manual: "Manual",
      auto: "Auto",
      pre_install: "Pre-install",
      pre_restore: "Pre-restore protection",
    },
    statuses: {
      completed: "Restorable",
      retention_pending: "Pruning interrupted",
      retention_partial: "Cleanup incomplete",
      deleted_by_retention: "Pruned",
      missing: "File missing",
      invalid: "Record abnormal",
    },
    report: {
      outcomes: {
        completed: "Pruning completed",
        within_policy: "Already within the retention policy",
        partial: "Pruning partially completed; it retries next time",
        blocked: "Pruning blocked by protection points",
        failed: "Pruning failed; no backups deleted",
      },
      evidenceDegradedSuffix: ", but the audit record is unavailable",
    },
    errors: {
      unavailableFallback: "The backup center is temporarily unreadable. Please try again later.",
    },
    page: {
      title: "Backup maintenance",
      subtitle: "Review backup history, protection points, and pruning state across profiles.",
      reloadAria: "Reload",
      overviewAria: "Backup summary",
      metricBackups: "Backups",
      metricSpace: "Known space",
      metricProtected: "Protected",
      metricAttention: "Action needed",
      filtersAria: "Filter backups",
      filterLabel: "Filter",
      filterProfile: "Profile",
      filterAllProfiles: "All profiles",
      filterTrigger: "Source",
      filterAllTriggers: "All sources",
      filterStatus: "Status",
      filterAllStatuses: "All statuses",
      filterSearch: "Search notes or profiles",
      filterSearchPlaceholder: "Type a keyword",
      profilesAria: "Profile summaries",
      profilesTitle: "Profile summaries",
      profilesCount: (count: number) => `${count}`,
      historyAria: "Backup history",
      historyTitle: "Backup history",
      historyCount: (count: number) => `${count} record(s)`,
      pagination: (current: number, total: number) => `Page ${current} / ${total}`,
      prevPage: "Previous page",
      nextPage: "Next page",
      unavailableTitle: "Backup center temporarily unavailable",
      emptyTitle: "No backups match the filters",
      emptyHint: "Adjust the filters and try again.",
      retry: "Retry",
      loadingPage: "Loading the backup center",
    },
    profileCard: {
      activeIdentity: "Current active profile",
      unboundIdentity: "No account summary bound",
      activeDotAria: "Active profile",
      factRecords: "Records",
      factSpace: "Space",
      factPolicy: "Policy",
      policyOk: "OK",
      policyOverBudget: "Over budget",
      maintainFailed: "Pruning failed",
      maintaining: "Pruning",
      maintainNow: "Prune now",
    },
    historyRow: {
      fileCount: (count: number) => `${count} file(s)`,
      noteAria: "Backup note",
      saveNoteAria: "Save note",
      cancelEditAria: "Cancel editing",
      editNoteAria: "Edit note",
      restore: "Restore save data",
      notRestorable: "Not restorable",
      notRestorableHint: "Only restorable backup points can be restored",
    },
    maintenanceDialog: {
      title: "Confirm immediate backup pruning",
      description: "Regular backups are pruned by this profile's saved retention policy. The latest regular backup and pre-restore protection points are never deleted; regular backups matching the count, age, or space rules may be permanently deleted. This cannot be undone.",
      cancel: "Cancel",
      confirm: "Confirm pruning",
    },
    toasts: {
      maintenanceMessage: (facts) =>
        `Scanned ${facts.scannedCount}, deleted ${facts.deletedCount}, took ${facts.elapsed}.`,
      maintenanceEvidenceSuffix: " The cleanup took effect, but this run's audit evidence is incomplete.",
      maintenanceFailedTitle: "Pruning failed",
      maintenanceFailedMessage: (message: string, elapsed: string) => `${message} Elapsed ${elapsed}.`,
      noteSavedTitle: "Note saved",
      noteSavedMessage: "The backup record was updated.",
      noteClearedMessage: "The backup note was cleared.",
      noteFailedTitle: "Saving the note failed",
    },
  },
  ja: {
    triggers: {
      manual: "手動",
      auto: "自動",
      pre_install: "インストール前",
      pre_restore: "復元前保護",
    },
    statuses: {
      completed: "復元可能",
      retention_pending: "整理が中断",
      retention_partial: "クリーンアップ未完了",
      deleted_by_retention: "整理済み",
      missing: "ファイル欠落",
      invalid: "記録異常",
    },
    report: {
      outcomes: {
        completed: "整理完了",
        within_policy: "既に保持ポリシーの範囲内",
        partial: "整理は一部完了。次回に再試行します",
        blocked: "整理は保護ポイントにより遮断",
        failed: "整理に失敗。バックアップは削除していません",
      },
      evidenceDegradedSuffix: "。ただし監査記録は利用できません",
    },
    errors: {
      unavailableFallback: "バックアップセンターを一時的に読み取れません。しばらくしてから再試行してください。",
    },
    page: {
      title: "バックアップ整理",
      subtitle: "プロファイル横断でバックアップ履歴・保護ポイント・整理状態を確認します。",
      reloadAria: "再読み込み",
      overviewAria: "バックアップ概要",
      metricBackups: "バックアップ記録",
      metricSpace: "既知の容量",
      metricProtected: "保護ポイント",
      metricAttention: "要対応",
      filtersAria: "バックアップを絞り込み",
      filterLabel: "絞り込み",
      filterProfile: "プロファイル",
      filterAllProfiles: "すべてのプロファイル",
      filterTrigger: "ソース",
      filterAllTriggers: "すべてのソース",
      filterStatus: "状態",
      filterAllStatuses: "すべての状態",
      filterSearch: "メモまたはプロファイルを検索",
      filterSearchPlaceholder: "キーワードを入力",
      profilesAria: "プロファイル概要",
      profilesTitle: "プロファイル概要",
      profilesCount: (count: number) => `${count} 件`,
      historyAria: "バックアップ履歴",
      historyTitle: "バックアップ履歴",
      historyCount: (count: number) => `${count} 件`,
      pagination: (current: number, total: number) => `${current} / ${total} ページ`,
      prevPage: "前のページ",
      nextPage: "次のページ",
      unavailableTitle: "バックアップセンターを一時的に利用できません",
      emptyTitle: "条件に一致するバックアップはありません",
      emptyHint: "絞り込み条件を調整して再試行してください。",
      retry: "再試行",
      loadingPage: "バックアップセンターを読み込み中",
    },
    profileCard: {
      activeIdentity: "現在アクティブなプロファイル",
      unboundIdentity: "アカウント概要は未紐付け",
      activeDotAria: "アクティブなプロファイル",
      factRecords: "記録",
      factSpace: "容量",
      factPolicy: "ポリシー",
      policyOk: "正常",
      policyOverBudget: "予算超過",
      maintainFailed: "整理に失敗",
      maintaining: "整理中",
      maintainNow: "今すぐ整理",
    },
    historyRow: {
      fileCount: (count: number) => `${count} ファイル`,
      noteAria: "バックアップメモ",
      saveNoteAria: "メモを保存",
      cancelEditAria: "編集をキャンセル",
      editNoteAria: "メモを編集",
      restore: "セーブデータを復元",
      notRestorable: "復元不可",
      notRestorableHint: "復元可能なバックアップポイントのみ復元できます",
    },
    maintenanceDialog: {
      title: "今すぐバックアップ整理を確認",
      description: "このプロファイルの保存済み保持ポリシーに従って通常バックアップを整理します。最新の通常バックアップと復元前保護ポイントは削除されません。数・経過日数・容量ルールに該当する通常バックアップは完全に削除される可能性があります。この操作は取り消せません。",
      cancel: "キャンセル",
      confirm: "整理を確定",
    },
    toasts: {
      maintenanceMessage: (facts) =>
        `${facts.scannedCount} 件をスキャンし、${facts.deletedCount} 件を削除しました。所要 ${facts.elapsed}。`,
      maintenanceEvidenceSuffix: " クリーンアップは反映済みですが、今回の監査証跡は不完全です。",
      maintenanceFailedTitle: "整理に失敗",
      maintenanceFailedMessage: (message: string, elapsed: string) => `${message} 経過時間 ${elapsed}。`,
      noteSavedTitle: "メモを保存しました",
      noteSavedMessage: "バックアップ記録を更新しました。",
      noteClearedMessage: "バックアップメモをクリアしました。",
      noteFailedTitle: "メモの保存に失敗",
    },
  },
} satisfies LocaleDictionary<BackupCenterCopy>;
