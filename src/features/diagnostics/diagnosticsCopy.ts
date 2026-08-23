import type { LocaleDictionary } from "../../shared/i18n";

// 日志与诊断页（页头、健康卡、日志面板、审计事件、导出对话框、toast）的
// 全部用户可见文案。App Log / Debug Log / Task Log / Audit Log 与后端状态码
// 原样展示为设计元素/DTO 透传。

export type DiagnosticsCopy = {
  page: {
    eyebrow: string;
    title: string;
    subtitle: string;
    refresh: string;
    exportBundle: string;
    loading: string;
    failedTitle: string;
    failedHint: string;
    retry: string;
  };
  dialog: {
    title: string;
    description: string;
    cancel: string;
    exporting: string;
    confirm: string;
  };
  content: {
    healthAria: string;
    platformLabel: string;
    logStorageLabel: string;
    healthOk: string;
    auditTitle: string;
    auditEmpty: string;
    logEmpty: string;
    copyStableIdTitle: string;
  };
  toasts: {
    copiedTitle: string;
    copyFailedTitle: string;
    copyFailedMessage: string;
    exportedTitle: string;
    exportedMessage: (facts: {
      fileName: string;
      size: string;
      appLogLineCount: number;
      debugLogLineCount: number;
      taskLogLineCount: number;
      auditEventCount: number;
    }) => string;
    exportFailedTitle: string;
    exportFailedMessage: string;
  };
};

export const diagnosticsCopy = {
  zh_cn: {
    page: {
      eyebrow: "只读支持工具",
      title: "日志与诊断",
      subtitle: "这里只显示后端已校验和脱敏的信息，不展示本地路径或原始错误。",
      refresh: "刷新",
      exportBundle: "导出诊断包",
      loading: "正在读取安全诊断摘要…",
      failedTitle: "诊断摘要不可用",
      failedHint: "读取失败未暴露原始错误；可重试或直接使用受控导出。",
      retry: "重试读取",
    },
    dialog: {
      title: "确认导出诊断包",
      description: "导出包将包含平台摘要、已脱敏 App/Task 日志、已校验审计事件和健康聚合，不包含完整路径与原始错误。",
      cancel: "取消",
      exporting: "导出中…",
      confirm: "确认导出",
    },
    content: {
      healthAria: "诊断健康摘要",
      platformLabel: "平台",
      logStorageLabel: "日志空间",
      healthOk: "正常",
      auditTitle: "最近审计事件",
      auditEmpty: "没有可显示的已校验事件。",
      logEmpty: "没有可显示的安全日志。",
      copyStableIdTitle: "复制稳定标识",
    },
    toasts: {
      copiedTitle: "已复制诊断标识",
      copyFailedTitle: "复制失败",
      copyFailedMessage: "无法写入剪贴板，请手动记录稳定诊断标识。",
      exportedTitle: "诊断包已导出",
      exportedMessage: (facts) =>
        `${facts.fileName}，${facts.size}；App 日志 ${facts.appLogLineCount} 行，Debug 日志 ${facts.debugLogLineCount} 行，任务日志 ${facts.taskLogLineCount} 行，审计事件 ${facts.auditEventCount} 条。`,
      exportFailedTitle: "诊断导出失败",
      exportFailedMessage: "未生成诊断包，请稍后重试。",
    },
  },
  en: {
    page: {
      eyebrow: "Read-only support tools",
      title: "Logs & diagnostics",
      subtitle: "Only backend-verified and redacted information is shown here — no local paths or raw errors.",
      refresh: "Refresh",
      exportBundle: "Export diagnostics",
      loading: "Reading the safe diagnostics summary…",
      failedTitle: "Diagnostics summary unavailable",
      failedHint: "The read failure does not expose raw errors; retry, or use the controlled export directly.",
      retry: "Retry read",
    },
    dialog: {
      title: "Confirm diagnostics export",
      description: "The bundle includes the platform summary, redacted App/Task logs, verified audit events, and health aggregates — no full paths or raw errors.",
      cancel: "Cancel",
      exporting: "Exporting…",
      confirm: "Confirm export",
    },
    content: {
      healthAria: "Diagnostics health summary",
      platformLabel: "Platform",
      logStorageLabel: "Log storage",
      healthOk: "OK",
      auditTitle: "Recent audit events",
      auditEmpty: "No verified events to show.",
      logEmpty: "No safe logs to show.",
      copyStableIdTitle: "Copy stable identifier",
    },
    toasts: {
      copiedTitle: "Diagnostic identifier copied",
      copyFailedTitle: "Copy failed",
      copyFailedMessage: "Could not write to the clipboard. Record the stable diagnostic identifier manually.",
      exportedTitle: "Diagnostics bundle exported",
      exportedMessage: (facts) =>
        `${facts.fileName}, ${facts.size}; app log ${facts.appLogLineCount} lines, debug log ${facts.debugLogLineCount} lines, task log ${facts.taskLogLineCount} lines, audit events ${facts.auditEventCount}.`,
      exportFailedTitle: "Diagnostics export failed",
      exportFailedMessage: "No diagnostics bundle was generated. Please try again later.",
    },
  },
  ja: {
    page: {
      eyebrow: "読み取り専用サポートツール",
      title: "ログと診断",
      subtitle: "ここにはバックエンドで検証・マスキング済みの情報のみ表示され、ローカルパスや生のエラーは表示されません。",
      refresh: "更新",
      exportBundle: "診断バンドルをエクスポート",
      loading: "安全な診断サマリーを読み込み中…",
      failedTitle: "診断サマリーを利用できません",
      failedHint: "読み取り失敗時も生のエラーは表示しません。再試行するか、管理されたエクスポートを直接使用してください。",
      retry: "読み込みを再試行",
    },
    dialog: {
      title: "診断バンドルのエクスポート確認",
      description: "エクスポートにはプラットフォームサマリー、マスキング済み App/Task ログ、検証済み監査イベント、健全性集計が含まれます。完全なパスや生のエラーは含まれません。",
      cancel: "キャンセル",
      exporting: "エクスポート中…",
      confirm: "エクスポートを確定",
    },
    content: {
      healthAria: "診断健全性サマリー",
      platformLabel: "プラットフォーム",
      logStorageLabel: "ログ容量",
      healthOk: "正常",
      auditTitle: "最近の監査イベント",
      auditEmpty: "表示できる検証済みイベントはありません。",
      logEmpty: "表示できる安全なログはありません。",
      copyStableIdTitle: "安定識別子をコピー",
    },
    toasts: {
      copiedTitle: "診断識別子をコピーしました",
      copyFailedTitle: "コピーに失敗",
      copyFailedMessage: "クリップボードへ書き込めません。安定診断識別子を手動で記録してください。",
      exportedTitle: "診断バンドルをエクスポート済み",
      exportedMessage: (facts) =>
        `${facts.fileName}、${facts.size}。App ログ ${facts.appLogLineCount} 行、Debug ログ ${facts.debugLogLineCount} 行、タスクログ ${facts.taskLogLineCount} 行、監査イベント ${facts.auditEventCount} 件。`,
      exportFailedTitle: "診断エクスポートに失敗",
      exportFailedMessage: "診断バンドルは生成されませんでした。しばらくしてから再試行してください。",
    },
  },
} satisfies LocaleDictionary<DiagnosticsCopy>;
