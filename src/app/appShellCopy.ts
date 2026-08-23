import type { LocaleDictionary } from "../shared/i18n";
import type { AppExitBlockReason, SaveBackupExitGuardReason } from "./window-lifecycle/windowLifecycleApi";

// 应用外壳（侧边栏导航、顶部状态栏、主题菜单入口、窗口关闭对话框与生命周期错误）的
// 全部用户可见文案。主题选项文案复用 settingsPageCopy.appearance.theme，避免双表漂移；
// 品牌名 Helsincy / Mod Manager 与 MHW:I 缩写为设计元素保持原文。

export type NavItemId =
  | "dashboard"
  | "mods"
  | "recovery"
  | "categories"
  | "profiles"
  | "replacements"
  | "backups"
  | "games"
  | "tasks"
  | "diagnostics"
  | "settings"
  | "about";

export type AppShellCopy = {
  nav: {
    labels: Record<NavItemId, string>;
    disabledReasons: Partial<Record<NavItemId, string>>;
  };
  sidebar: {
    primaryAria: string;
    utilityAria: string;
    footnote: string;
    switchToClassic: string;
    switchToFloating: string;
    floatingModeLabel: string;
  };
  header: {
    currentGame: string;
    statusAria: string;
    tourAria: string;
    tourLabel: string;
    profilePill: string;
    profileLoading: string;
    profileUnavailable: string;
    directoryConfigured: string;
    directoryValidating: string;
    directoryInvalid: string;
    directoryNotConfigured: string;
    taskIdle: string;
    windowToolsAria: string;
    openSettingsAria: string;
  };
  themeMenu: {
    triggerAria: string;
    triggerLabel: string;
    menuAria: string;
  };
  windowClose: {
    unsafeReasons: Record<SaveBackupExitGuardReason, string>;
    blockedReasons: Record<AppExitBlockReason, string>;
    cancelCloseAria: string;
    unsafeTitle: string;
    blockedTitle: string;
    normalTitle: string;
    normalDescription: string;
    trayStay: string;
    trayCollapse: string;
    trayUnsafeHint: string;
    trayBlockedHint: string;
    trayNormalHint: string;
    exitStill: string;
    exitFull: string;
    exitUnsafeHint: string;
    exitNormalHint: string;
    remember: string;
    cancelUnsafe: string;
    cancelBlocked: string;
    cancelNormal: string;
    successTray: string;
    successExit: string;
  };
  windowLifecycle: {
    errorFallback: string;
    errors: {
      exit_confirmation_required: string;
      exit_authorization_unavailable: string;
      window_hide_failed: string;
    };
    preferenceSaveError: string;
  };
};

export const appShellCopy = {
  zh_cn: {
    nav: {
      labels: {
        dashboard: "工作台",
        mods: "Mod 管理",
        recovery: "恢复中心",
        categories: "分类 / 标签",
        profiles: "存档备份",
        replacements: "替换目标",
        backups: "备份整理",
        games: "游戏管理",
        tasks: "任务队列",
        diagnostics: "日志 / 诊断",
        settings: "设置",
        about: "关于",
      },
      disabledReasons: {
        categories: "导入 Mod 后启用",
        replacements: "替换目标 catalog 接入后启用",
        games: "游戏管理页面尚未接入",
        tasks: "任务队列页面尚未接入",
      },
    },
    sidebar: {
      primaryAria: "主导航",
      utilityAria: "辅助导航",
      footnote: "首次启动",
      switchToClassic: "切换为普通侧边栏",
      switchToFloating: "切换为悬浮侧边栏",
      floatingModeLabel: "悬浮侧边栏",
    },
    header: {
      currentGame: "当前游戏",
      statusAria: "当前状态",
      tourAria: "打开新手引导",
      tourLabel: "新手引导",
      profilePill: "配置档",
      profileLoading: "读取中",
      profileUnavailable: "不可用",
      directoryConfigured: "目录已配置",
      directoryValidating: "校验目录中",
      directoryInvalid: "目录不可用",
      directoryNotConfigured: "目录未配置",
      taskIdle: "任务空闲",
      windowToolsAria: "窗口工具",
      openSettingsAria: "打开设置",
    },
    themeMenu: {
      triggerAria: "选择主题模式",
      triggerLabel: "主题",
      menuAria: "主题模式",
    },
    windowClose: {
      unsafeReasons: {
        background_starting:
          "后台任务已注册，但尚未完成首次运行验证。Windows 仍会在约 1 分钟后尝试运行；若失败，应用退出后无法立即提醒你。",
        background_not_enabled: "后台保护尚未启用。完全退出后，自动备份不会继续按计划检查。",
        registration_failed: "后台任务注册或校验失败。完全退出后，自动备份可能不会按计划运行。",
        worker_unhealthy: "后台任务最近没有按预期运行。完全退出后，自动备份可能失去保护。",
        permission_required: "当前账户权限不足，后台任务无法完成注册或校验。",
        unsupported_platform: "当前平台不支持退出后的后台自动备份保护。",
        status_unavailable: "暂时无法确认后台保护状态。为避免静默失去保护，建议先留在托盘。",
      },
      blockedReasons: {
        save_restore_in_progress:
          "存档恢复正在进行。为保护当前存档和自动创建的恢复前备份，此时不能完全退出应用程序。",
        save_restore_status_unavailable:
          "暂时无法确认存档恢复任务状态。为避免中断存档写入，此时不能完全退出应用程序。",
      },
      cancelCloseAria: "取消关闭",
      unsafeTitle: "后台保护尚未就绪",
      blockedTitle: "存档恢复正在保护中",
      normalTitle: "准备退出 Helsincy？",
      normalDescription: "请选择关闭主窗口时的操作。你也可以在设置里随时改回每次询问。",
      trayStay: "留在托盘",
      trayCollapse: "收起至系统托盘",
      trayUnsafeHint: "保留客户端运行，让自动备份继续在本次会话内检查。",
      trayBlockedHint: "让存档恢复在后台继续完成；完成后可正常完全退出。",
      trayNormalHint: "应用将在后台持续运行，自动备份仍会在客户端运行期间检查。",
      exitStill: "仍然退出",
      exitFull: "完全退出应用程序",
      exitUnsafeHint: "忽略本次后台保护警告并完全退出。此确认只对本次有效。",
      exitNormalHint: "关闭主客户端。若后台保护尚未就绪，退出前会再次向你确认。",
      remember: "记住我的选择，下次直接执行",
      cancelUnsafe: "取消退出",
      cancelBlocked: "返回应用",
      cancelNormal: "暂不退出",
      successTray: "已收起至系统托盘",
      successExit: "正在退出应用",
    },
    windowLifecycle: {
      errorFallback: "窗口关闭操作失败",
      errors: {
        exit_confirmation_required: "退出前需要确认后台保护状态。",
        exit_authorization_unavailable: "退出确认状态不可用，请暂时留在托盘或重启应用后再试。",
        window_hide_failed: "窗口隐藏失败，请重试。",
      },
      preferenceSaveError: "关闭行为偏好保存失败，请检查应用存储权限后重试。",
    },
  },
  en: {
    nav: {
      labels: {
        dashboard: "Workbench",
        mods: "Mod management",
        recovery: "Recovery Center",
        categories: "Categories / Tags",
        profiles: "Save backups",
        replacements: "Replacement targets",
        backups: "Backup maintenance",
        games: "Game management",
        tasks: "Task queue",
        diagnostics: "Logs / Diagnostics",
        settings: "Settings",
        about: "About",
      },
      disabledReasons: {
        categories: "Enabled after importing mods",
        replacements: "Enabled once the replacement target catalog is wired",
        games: "The game management page is not wired yet",
        tasks: "The task queue page is not wired yet",
      },
    },
    sidebar: {
      primaryAria: "Primary navigation",
      utilityAria: "Utility navigation",
      footnote: "First launch",
      switchToClassic: "Switch to the classic sidebar",
      switchToFloating: "Switch to the floating sidebar",
      floatingModeLabel: "Floating sidebar",
    },
    header: {
      currentGame: "Current game",
      statusAria: "Current status",
      tourAria: "Open the onboarding tour",
      tourLabel: "Onboarding",
      profilePill: "Profile",
      profileLoading: "Loading",
      profileUnavailable: "Unavailable",
      directoryConfigured: "Directory configured",
      directoryValidating: "Validating directory",
      directoryInvalid: "Directory unavailable",
      directoryNotConfigured: "Directory not configured",
      taskIdle: "Tasks idle",
      windowToolsAria: "Window tools",
      openSettingsAria: "Open settings",
    },
    themeMenu: {
      triggerAria: "Choose theme mode",
      triggerLabel: "Theme",
      menuAria: "Theme mode",
    },
    windowClose: {
      unsafeReasons: {
        background_starting:
          "The background task is registered but has not passed its first verified run. Windows still tries to run it in about a minute; if that fails after the app exits, you cannot be alerted immediately.",
        background_not_enabled: "Background protection is not enabled yet. After a full exit, auto backups stop checking on schedule.",
        registration_failed: "Background task registration or verification failed. After a full exit, auto backups may not run on schedule.",
        worker_unhealthy: "The background task has not run as expected recently. After a full exit, auto backups may lose protection.",
        permission_required: "The current account lacks the permissions to register or verify the background task.",
        unsupported_platform: "This platform does not support background auto backup protection after exit.",
        status_unavailable: "The background protection state cannot be confirmed right now. To avoid silently losing protection, staying in the tray is recommended.",
      },
      blockedReasons: {
        save_restore_in_progress:
          "A save data restore is in progress. To protect the current save data and the automatically created pre-restore backup, the application cannot fully exit right now.",
        save_restore_status_unavailable:
          "The save data restore task state cannot be confirmed right now. To avoid interrupting save data writes, the application cannot fully exit right now.",
      },
      cancelCloseAria: "Cancel closing",
      unsafeTitle: "Background protection not ready",
      blockedTitle: "Save data restore is being protected",
      normalTitle: "Exit Helsincy?",
      normalDescription: "Choose what closing the main window should do. You can switch back to asking every time in Settings.",
      trayStay: "Stay in the tray",
      trayCollapse: "Minimize to system tray",
      trayUnsafeHint: "Keep the client running so auto backups keep checking within this session.",
      trayBlockedHint: "Let the save data restore finish in the background; a normal full exit is possible afterwards.",
      trayNormalHint: "The app keeps running in the background; auto backups still check while the client is running.",
      exitStill: "Exit anyway",
      exitFull: "Exit the application completely",
      exitUnsafeHint: "Ignore this background protection warning and exit completely. This confirmation applies to this time only.",
      exitNormalHint: "Close the main client. If background protection is not ready, you are asked again before exiting.",
      remember: "Remember my choice and apply it next time",
      cancelUnsafe: "Cancel exit",
      cancelBlocked: "Back to the app",
      cancelNormal: "Not now",
      successTray: "Minimized to the system tray",
      successExit: "Exiting the application",
    },
    windowLifecycle: {
      errorFallback: "The window close operation failed",
      errors: {
        exit_confirmation_required: "The background protection state must be confirmed before exiting.",
        exit_authorization_unavailable: "The exit confirmation state is unavailable. Stay in the tray for now or restart the app and try again.",
        window_hide_failed: "Hiding the window failed. Please try again.",
      },
      preferenceSaveError: "Saving the close behavior preference failed. Check the app storage permissions and try again.",
    },
  },
  ja: {
    nav: {
      labels: {
        dashboard: "ワークベンチ",
        mods: "Mod 管理",
        recovery: "リカバリーセンター",
        categories: "カテゴリ / タグ",
        profiles: "セーブバックアップ",
        replacements: "置き換え対象",
        backups: "バックアップ整理",
        games: "ゲーム管理",
        tasks: "タスクキュー",
        diagnostics: "ログ / 診断",
        settings: "設定",
        about: "情報",
      },
      disabledReasons: {
        categories: "Mod のインポート後に有効化",
        replacements: "置き換え対象カタログの接続後に有効化",
        games: "ゲーム管理ページは未接続です",
        tasks: "タスクキューページは未接続です",
      },
    },
    sidebar: {
      primaryAria: "メインナビゲーション",
      utilityAria: "補助ナビゲーション",
      footnote: "初回起動",
      switchToClassic: "通常サイドバーに切り替え",
      switchToFloating: "フローティングサイドバーに切り替え",
      floatingModeLabel: "フローティングサイドバー",
    },
    header: {
      currentGame: "現在のゲーム",
      statusAria: "現在の状態",
      tourAria: "チュートリアルを開く",
      tourLabel: "チュートリアル",
      profilePill: "プロファイル",
      profileLoading: "読み込み中",
      profileUnavailable: "利用不可",
      directoryConfigured: "ディレクトリ設定済み",
      directoryValidating: "ディレクトリ検証中",
      directoryInvalid: "ディレクトリ利用不可",
      directoryNotConfigured: "ディレクトリ未設定",
      taskIdle: "タスクなし",
      windowToolsAria: "ウィンドウツール",
      openSettingsAria: "設定を開く",
    },
    themeMenu: {
      triggerAria: "テーマモードを選択",
      triggerLabel: "テーマ",
      menuAria: "テーマモード",
    },
    windowClose: {
      unsafeReasons: {
        background_starting:
          "バックグラウンドタスクは登録済みですが、初回実行の検証が未完了です。Windows は約 1 分後に実行を試みます。失敗した場合、アプリ終了後はすぐに通知できません。",
        background_not_enabled: "バックグラウンド保護はまだ有効ではありません。完全終了後、自動バックアップは計画どおりの確認を停止します。",
        registration_failed: "バックグラウンドタスクの登録または検証に失敗しました。完全終了後、自動バックアップは計画どおり実行されない可能性があります。",
        worker_unhealthy: "バックグラウンドタスクが最近期待どおり実行されていません。完全終了後、自動バックアップの保護が失われる可能性があります。",
        permission_required: "現在のアカウントの権限が不足しており、バックグラウンドタスクの登録・検証を完了できません。",
        unsupported_platform: "現在のプラットフォームは終了後のバックグラウンド自動バックアップ保護に未対応です。",
        status_unavailable: "バックグラウンド保護の状態を現在確認できません。保護を静かに失わないため、まずトレイに残ることを推奨します。",
      },
      blockedReasons: {
        save_restore_in_progress:
          "セーブデータの復元が進行中です。現在のセーブデータと自動作成された復元前バックアップを保護するため、今はアプリを完全終了できません。",
        save_restore_status_unavailable:
          "セーブデータ復元タスクの状態を現在確認できません。セーブデータ書き込みの中断を避けるため、今はアプリを完全終了できません。",
      },
      cancelCloseAria: "閉じる操作をキャンセル",
      unsafeTitle: "バックグラウンド保護が未準備",
      blockedTitle: "セーブデータ復元を保護中",
      normalTitle: "Helsincy を終了しますか？",
      normalDescription: "メインウィンドウを閉じるときの動作を選択してください。設定でいつでも毎回確認に戻せます。",
      trayStay: "トレイに残る",
      trayCollapse: "システムトレイへ最小化",
      trayUnsafeHint: "クライアントを実行したまま、自動バックアップが本セッション内で確認を続けられるようにします。",
      trayBlockedHint: "セーブデータの復元をバックグラウンドで完了させます。完了後は通常どおり完全終了できます。",
      trayNormalHint: "アプリはバックグラウンドで動作し続け、クライアント実行中は自動バックアップの確認が続きます。",
      exitStill: "それでも終了",
      exitFull: "アプリを完全終了",
      exitUnsafeHint: "今回のバックグラウンド保護警告を無視して完全終了します。この確認は今回のみ有効です。",
      exitNormalHint: "メインクライアントを閉じます。バックグラウンド保護が未準備の場合、終了前に再度確認します。",
      remember: "選択を記憶し、次回から直接実行",
      cancelUnsafe: "終了をキャンセル",
      cancelBlocked: "アプリに戻る",
      cancelNormal: "今は終了しない",
      successTray: "システムトレイへ最小化しました",
      successExit: "アプリを終了しています",
    },
    windowLifecycle: {
      errorFallback: "ウィンドウを閉じる操作に失敗しました",
      errors: {
        exit_confirmation_required: "終了前にバックグラウンド保護の状態確認が必要です。",
        exit_authorization_unavailable: "終了確認の状態を利用できません。しばらくトレイに残るか、アプリを再起動してから再試行してください。",
        window_hide_failed: "ウィンドウの非表示に失敗しました。再試行してください。",
      },
      preferenceSaveError: "閉じる動作の設定保存に失敗しました。アプリストレージの権限を確認してから再試行してください。",
    },
  },
} satisfies LocaleDictionary<AppShellCopy>;
