import type { LocaleDictionary } from "../../shared/i18n";
import type { InstallRecoveryIssueSummary, UnsafeInstallStatus } from "./modInstallPlanTypes";
import type { GamePrerequisiteDecisionCode } from "./modInstallPlanTypes";
import type { ManagedInstallTaskPhase } from "./modInstallTaskState";

// 单 Mod 安装/卸载生命周期（任务进度、计划详情面板、卸载确认、终态 toast）
// 与安装前置检查的全部用户可见文案。语义推进留在 modInstallTaskState /
// modLifecycleFeedbackState，文本在渲染或 toast 组装时经 copy 取。

export type ModLifecycleCopy = {
  prerequisite: {
    codes: Record<GamePrerequisiteDecisionCode, string>;
    ready: string;
    warning: string;
    blocked: string;
  };
  installTask: {
    phases: Record<ManagedInstallTaskPhase, string>;
    startingInstall: string;
    startingUninstall: string;
    installFailedDefault: string;
    uninstallFailedDefault: string;
    installFailures: Record<string, string>;
    uninstallFailures: Record<string, string>;
  };
  terminalToasts: {
    installCompleted: string;
    uninstallCompleted: string;
    installCancelled: string;
    installFailed: string;
    uninstallFailed: string;
  };
  planSheet: {
    recoveryTitles: Record<UnsafeInstallStatus, string>;
    prerequisiteBlockedTitle: string;
    conflictsTitle: string;
    defaultTitle: string;
    closeAria: string;
    generating: string;
    recoveryMessages: Record<UnsafeInstallStatus, string>;
    recoveryIssueLabels: Record<InstallRecoveryIssueSummary["issue"], string>;
    recoverySummaryAria: string;
    recoveryIssuesAria: string;
    metricManagedFiles: string;
    metricBackups: string;
    metricChecks: string;
    planDetailsAria: string;
    prerequisiteResultsAria: string;
    metricActions: string;
    metricConflicts: string;
    pathPreviewAria: string;
    conflictPreviewAria: string;
    noActions: string;
  };
  uninstallDialog: {
    title: string;
    closeAria: string;
    cancel: string;
    confirm: string;
    body: string;
  };
  taskFeedback: {
    noticeViewportAria: string;
    installingTitle: string;
    uninstallingTitle: string;
    toastViewportAria: string;
    dismissAria: string;
  };
};

export const modLifecycleCopy = {
  zh_cn: {
    prerequisite: {
      codes: {
        game_not_configured: "尚未配置游戏目录",
        game_directory_invalid: "游戏目录校验失败",
        game_directory_not_writable: "游戏目录当前不可写，请关闭游戏与相关程序，或用管理员身份运行后重试",
        rules_unavailable: "前置规则不可用",
        rules_corrupted: "前置规则已损坏",
        storage_unavailable: "前置状态存储不可用",
        storage_corrupted: "前置状态存储已损坏",
        unsupported_game: "当前游戏不支持前置检查",
        missing_required_file: "缺少必要前置文件",
        signature_unverified: "前置文件签名无法验证",
        config_read_failed: "前置配置读取失败",
        config_invalid_json: "前置配置格式无效",
        config_field_mismatch: "前置配置未启用必要选项",
        prerequisite_decision_invalid: "前置检查结果无效",
      },
      ready: "前置检查通过。",
      warning: "前置文件存在未验证项，确认来源可信后仍可继续。",
      blocked: "前置检查未通过，后端已阻止写入。",
    },
    installTask: {
      phases: {
        "install.queued": "等待安装",
        "install.plan.building": "生成安装计划",
        "install.commit.processing": "写入中",
        "install.completed": "安装完成",
        "install.failed": "安装失败",
        "install.cancelled": "已取消",
        "install.uninstall.queued": "等待卸载",
        "install.uninstall.processing": "卸载中",
        "install.uninstall.completed": "卸载完成",
        "install.uninstall.failed": "卸载失败",
      },
      startingInstall: "启动安装任务",
      startingUninstall: "启动卸载任务",
      installFailedDefault: "安装失败",
      uninstallFailedDefault: "卸载失败",
      installFailures: {
        planning: "无法生成安装计划",
        lock: "安装任务暂时无法开始",
        commit: "安装未完成，已重新检查安装状态",
        complete: "安装收尾未完成，已重新检查安装状态",
        recovery_pending: "安装被待处理的恢复状态阻断",
        recovery_unavailable: "安装状态暂时无法确认",
        empty_plan: "包内没有找到可安装的文件，未做任何修改",
      },
      uninstallFailures: {
        lock: "卸载任务暂时无法开始",
        uninstall: "卸载未完成，已重新检查安装状态",
        complete: "卸载收尾未完成，已重新检查安装状态",
        recovery_pending: "卸载被待处理的恢复状态阻断",
        recovery_unavailable: "卸载状态暂时无法确认",
      },
    },
    terminalToasts: {
      installCompleted: "安装完成",
      uninstallCompleted: "卸载完成",
      installCancelled: "安装已取消",
      installFailed: "安装失败",
      uninstallFailed: "卸载失败",
    },
    planSheet: {
      recoveryTitles: {
        rollback_required: "需要回滚",
        committed_cleanup_pending: "重装待收尾",
        cleanup_pending: "恢复待清理",
        unknown: "安装状态未知",
        repair_required: "需要人工处理",
      },
      prerequisiteBlockedTitle: "安装前置未就绪",
      conflictsTitle: "安装计划存在冲突",
      defaultTitle: "安装计划预览",
      closeAria: "关闭安装计划",
      generating: "正在生成安装计划",
      recoveryMessages: {
        rollback_required: "恢复记录显示上次写入未确认完成。请保留现场，前往恢复中心执行受控处理。",
        committed_cleanup_pending: "新版本已提交，但完成记录尚未收敛。状态收敛前不要安装、卸载或重装。",
        cleanup_pending: "重装事务已完成，但恢复数据尚待清理。清理完成前不要继续写入操作。",
        unknown: "恢复扫描无法确认当前安装状态。请保留现场并重新扫描。",
        repair_required: "当前安装状态不能安全自动处理。请先在恢复中心确认。",
      },
      recoveryIssueLabels: {
        missing_installed_file_summary: "缺少安装摘要",
        target_missing: "目标缺失",
        target_changed: "目标已变化",
        target_read_failed: "目标读取失败",
        backup_missing: "备份缺失",
        backup_read_failed: "备份读取失败",
      },
      recoverySummaryAria: "恢复扫描摘要",
      recoveryIssuesAria: "恢复扫描问题",
      metricManagedFiles: "托管文件",
      metricBackups: "备份恢复点",
      metricChecks: "检查项",
      planDetailsAria: "安装计划详情",
      prerequisiteResultsAria: "安装前置检查结果",
      metricActions: "可执行动作",
      metricConflicts: "阻断冲突",
      pathPreviewAria: "目标路径预览",
      conflictPreviewAria: "冲突路径预览",
      noActions: "没有可执行动作",
    },
    uninstallDialog: {
      title: "确认卸载",
      closeAria: "取消卸载",
      cancel: "取消",
      confirm: "确认卸载",
      body: "将删除本工具新增的托管文件，并从受控备份恢复被覆盖文件。",
    },
    taskFeedback: {
      noticeViewportAria: "Mod 任务进度",
      installingTitle: "正在安装 Mod",
      uninstallingTitle: "正在卸载 Mod",
      toastViewportAria: "Mod 操作通知",
      dismissAria: "关闭通知",
    },
  },
  en: {
    prerequisite: {
      codes: {
        game_not_configured: "Game directory not configured yet",
        game_directory_invalid: "Game directory validation failed",
        game_directory_not_writable: "The game directory is not writable. Close the game and related programs, or run as administrator and retry",
        rules_unavailable: "Prerequisite rules unavailable",
        rules_corrupted: "Prerequisite rules corrupted",
        storage_unavailable: "Prerequisite state storage unavailable",
        storage_corrupted: "Prerequisite state storage corrupted",
        unsupported_game: "The current game does not support prerequisite checks",
        missing_required_file: "A required prerequisite file is missing",
        signature_unverified: "A prerequisite file signature could not be verified",
        config_read_failed: "Failed to read prerequisite configuration",
        config_invalid_json: "Prerequisite configuration format is invalid",
        config_field_mismatch: "Prerequisite configuration does not enable a required option",
        prerequisite_decision_invalid: "Prerequisite check result is invalid",
      },
      ready: "Prerequisite checks passed.",
      warning: "Some prerequisite files are unverified. You may continue if you trust their source.",
      blocked: "Prerequisite checks failed; the backend blocked the write.",
    },
    installTask: {
      phases: {
        "install.queued": "Waiting to install",
        "install.plan.building": "Building install plan",
        "install.commit.processing": "Writing",
        "install.completed": "Install completed",
        "install.failed": "Install failed",
        "install.cancelled": "Cancelled",
        "install.uninstall.queued": "Waiting to uninstall",
        "install.uninstall.processing": "Uninstalling",
        "install.uninstall.completed": "Uninstall completed",
        "install.uninstall.failed": "Uninstall failed",
      },
      startingInstall: "Starting install task",
      startingUninstall: "Starting uninstall task",
      installFailedDefault: "Install failed",
      uninstallFailedDefault: "Uninstall failed",
      installFailures: {
        planning: "Failed to build the install plan",
        lock: "The install task cannot start right now",
        commit: "Install did not complete; install status was re-checked",
        complete: "Install finalization did not complete; install status was re-checked",
        recovery_pending: "Install blocked by a pending recovery state",
        recovery_unavailable: "Install status temporarily unconfirmable",
        empty_plan: "No installable files were found in the package; nothing was changed",
      },
      uninstallFailures: {
        lock: "The uninstall task cannot start right now",
        uninstall: "Uninstall did not complete; install status was re-checked",
        complete: "Uninstall finalization did not complete; install status was re-checked",
        recovery_pending: "Uninstall blocked by a pending recovery state",
        recovery_unavailable: "Uninstall status temporarily unconfirmable",
      },
    },
    terminalToasts: {
      installCompleted: "Install completed",
      uninstallCompleted: "Uninstall completed",
      installCancelled: "Install cancelled",
      installFailed: "Install failed",
      uninstallFailed: "Uninstall failed",
    },
    planSheet: {
      recoveryTitles: {
        rollback_required: "Rollback required",
        committed_cleanup_pending: "Reinstall cleanup pending",
        cleanup_pending: "Recovery cleanup pending",
        unknown: "Install status unknown",
        repair_required: "Manual handling required",
      },
      prerequisiteBlockedTitle: "Install prerequisites not ready",
      conflictsTitle: "Install plan has conflicts",
      defaultTitle: "Install Plan Preview",
      closeAria: "Close install plan",
      generating: "Generating install plan",
      recoveryMessages: {
        rollback_required: "Recovery records show the last write was not confirmed complete. Preserve the current state and run controlled handling in the Recovery Center.",
        committed_cleanup_pending: "The new revision was committed, but completion records have not converged. Do not install, uninstall, or reinstall before convergence.",
        cleanup_pending: "The reinstall transaction completed, but recovery data awaits cleanup. Do not continue write operations before cleanup finishes.",
        unknown: "The recovery scan could not confirm the current install status. Preserve the current state and rescan.",
        repair_required: "The current install status cannot be handled automatically and safely. Confirm in the Recovery Center first.",
      },
      recoveryIssueLabels: {
        missing_installed_file_summary: "Install summary missing",
        target_missing: "Target missing",
        target_changed: "Target changed",
        target_read_failed: "Target read failed",
        backup_missing: "Backup missing",
        backup_read_failed: "Backup read failed",
      },
      recoverySummaryAria: "Recovery scan summary",
      recoveryIssuesAria: "Recovery scan issues",
      metricManagedFiles: "Managed files",
      metricBackups: "Backup restore points",
      metricChecks: "Checks",
      planDetailsAria: "Install plan details",
      prerequisiteResultsAria: "Install prerequisite check results",
      metricActions: "Executable actions",
      metricConflicts: "Blocking conflicts",
      pathPreviewAria: "Target path preview",
      conflictPreviewAria: "Conflict path preview",
      noActions: "No executable actions",
    },
    uninstallDialog: {
      title: "Confirm Uninstall",
      closeAria: "Cancel uninstall",
      cancel: "Cancel",
      confirm: "Confirm uninstall",
      body: "Managed files added by this tool will be deleted, and overwritten files will be restored from controlled backups.",
    },
    taskFeedback: {
      noticeViewportAria: "Mod task progress",
      installingTitle: "Installing mod",
      uninstallingTitle: "Uninstalling mod",
      toastViewportAria: "Mod operation notifications",
      dismissAria: "Dismiss notification",
    },
  },
  ja: {
    prerequisite: {
      codes: {
        game_not_configured: "ゲームディレクトリが未設定です",
        game_directory_invalid: "ゲームディレクトリの検証に失敗しました",
        game_directory_not_writable: "ゲームディレクトリに書き込めません。ゲームと関連プログラムを終了するか、管理者として実行して再試行してください",
        rules_unavailable: "前提ルールを利用できません",
        rules_corrupted: "前提ルールが破損しています",
        storage_unavailable: "前提状態ストレージを利用できません",
        storage_corrupted: "前提状態ストレージが破損しています",
        unsupported_game: "現在のゲームは前提チェックに対応していません",
        missing_required_file: "必要な前提ファイルが不足しています",
        signature_unverified: "前提ファイルの署名を検証できません",
        config_read_failed: "前提設定の読み込みに失敗しました",
        config_invalid_json: "前提設定の形式が無効です",
        config_field_mismatch: "前提設定で必要なオプションが有効になっていません",
        prerequisite_decision_invalid: "前提チェック結果が無効です",
      },
      ready: "前提チェックを通過しました。",
      warning: "未検証の前提ファイルがあります。提供元を信頼できる場合は続行できます。",
      blocked: "前提チェックを通過しなかったため、バックエンドが書き込みをブロックしました。",
    },
    installTask: {
      phases: {
        "install.queued": "インストール待機中",
        "install.plan.building": "インストールプランを生成中",
        "install.commit.processing": "書き込み中",
        "install.completed": "インストール完了",
        "install.failed": "インストール失敗",
        "install.cancelled": "キャンセル済み",
        "install.uninstall.queued": "アンインストール待機中",
        "install.uninstall.processing": "アンインストール中",
        "install.uninstall.completed": "アンインストール完了",
        "install.uninstall.failed": "アンインストール失敗",
      },
      startingInstall: "インストールタスクを起動",
      startingUninstall: "アンインストールタスクを起動",
      installFailedDefault: "インストール失敗",
      uninstallFailedDefault: "アンインストール失敗",
      installFailures: {
        planning: "インストールプランを生成できません",
        lock: "インストールタスクを今は開始できません",
        commit: "インストールが完了しなかったため、インストール状態を再確認しました",
        complete: "インストールの後処理が完了しなかったため、インストール状態を再確認しました",
        recovery_pending: "処理待ちの復旧状態によりインストールがブロックされました",
        recovery_unavailable: "インストール状態を一時的に確認できません",
        empty_plan:
          "パッケージ内にインストール可能なファイルが見つからなかったため、変更は行っていません",
      },
      uninstallFailures: {
        lock: "アンインストールタスクを今は開始できません",
        uninstall: "アンインストールが完了しなかったため、インストール状態を再確認しました",
        complete: "アンインストールの後処理が完了しなかったため、インストール状態を再確認しました",
        recovery_pending: "処理待ちの復旧状態によりアンインストールがブロックされました",
        recovery_unavailable: "アンインストール状態を一時的に確認できません",
      },
    },
    terminalToasts: {
      installCompleted: "インストール完了",
      uninstallCompleted: "アンインストール完了",
      installCancelled: "インストールをキャンセルしました",
      installFailed: "インストール失敗",
      uninstallFailed: "アンインストール失敗",
    },
    planSheet: {
      recoveryTitles: {
        rollback_required: "ロールバックが必要",
        committed_cleanup_pending: "再インストールの後処理待ち",
        cleanup_pending: "復旧データの整理待ち",
        unknown: "インストール状態が不明",
        repair_required: "手動対応が必要",
      },
      prerequisiteBlockedTitle: "インストール前提が未整備",
      conflictsTitle: "インストールプランに競合あり",
      defaultTitle: "インストールプランのプレビュー",
      closeAria: "インストールプランを閉じる",
      generating: "インストールプランを生成中",
      recoveryMessages: {
        rollback_required: "復旧記録によると、前回の書き込みは完了が確認されていません。現状を保持したまま、復旧センターで管理された処理を実行してください。",
        committed_cleanup_pending: "新バージョンはコミットされましたが、完了記録がまだ収束していません。収束前にインストール・アンインストール・再インストールを行わないでください。",
        cleanup_pending: "再インストールトランザクションは完了しましたが、復旧データの整理が残っています。整理完了前に書き込み操作を続けないでください。",
        unknown: "復旧スキャンで現在のインストール状態を確認できません。現状を保持して再スキャンしてください。",
        repair_required: "現在のインストール状態は安全に自動処理できません。先に復旧センターで確認してください。",
      },
      recoveryIssueLabels: {
        missing_installed_file_summary: "インストール概要の欠落",
        target_missing: "ターゲットの欠落",
        target_changed: "ターゲットの変更",
        target_read_failed: "ターゲットの読み取り失敗",
        backup_missing: "バックアップの欠落",
        backup_read_failed: "バックアップの読み取り失敗",
      },
      recoverySummaryAria: "復旧スキャンの概要",
      recoveryIssuesAria: "復旧スキャンの問題",
      metricManagedFiles: "管理ファイル",
      metricBackups: "バックアップ復元ポイント",
      metricChecks: "チェック項目",
      planDetailsAria: "インストールプランの詳細",
      prerequisiteResultsAria: "インストール前提チェック結果",
      metricActions: "実行可能アクション",
      metricConflicts: "ブロッキング競合",
      pathPreviewAria: "ターゲットパスのプレビュー",
      conflictPreviewAria: "競合パスのプレビュー",
      noActions: "実行可能なアクションはありません",
    },
    uninstallDialog: {
      title: "アンインストールの確認",
      closeAria: "アンインストールを中止",
      cancel: "キャンセル",
      confirm: "アンインストールを確定",
      body: "本ツールが追加した管理ファイルを削除し、上書きされたファイルを管理バックアップから復元します。",
    },
    taskFeedback: {
      noticeViewportAria: "Mod タスクの進行状況",
      installingTitle: "Mod をインストール中",
      uninstallingTitle: "Mod をアンインストール中",
      toastViewportAria: "Mod 操作の通知",
      dismissAria: "通知を閉じる",
    },
  },
} satisfies LocaleDictionary<ModLifecycleCopy>;
