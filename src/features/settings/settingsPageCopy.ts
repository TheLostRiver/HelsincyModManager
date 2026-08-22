import type { LocaleDictionary } from "../../shared/i18n";

// SettingsPage 自有文案（I18N-01 试点）。内嵌面板（后台保护、Debug Log、前置环境）的
// 文案归属各自 feature，随 I18N-04~06 迁移，不在本模块。

export type SettingsPageCopy = {
  hero: {
    eyebrow: string;
    title: string;
    description: string;
    statusLabel: string;
    dirty: string;
    pristine: string;
    reset: string;
  };
  appearance: {
    title: string;
    description: string;
    theme: {
      label: string;
      hint: string;
      light: string;
      dark: string;
      system: string;
    };
    language: {
      label: string;
      hint: string;
      followSystem: (systemNativeName: string) => string;
      toastTitle: string;
      toastMessage: (nativeName: string) => string;
    };
    compactPanels: { title: string; description: string };
    reduceMotion: { title: string; description: string };
    startPage: {
      label: string;
      dashboard: string;
      mods: string;
      last: string;
    };
  };
  windowBehavior: {
    title: string;
    description: string;
    closeLabel: string;
    ask: string;
    tray: string;
    exit: string;
    saveError: string;
    note: string;
  };
  modImport: {
    title: string;
    description: string;
    previewAfterImport: { title: string; description: string };
    confirmBeforeConflict: { title: string; description: string };
  };
  prerequisites: { title: string; description: string };
  saveBackup: {
    title: string;
    description: string;
    backupReminder: { title: string; description: string };
  };
  logs: { title: string; description: string; exportNote: string };
};

export const settingsPageCopy = {
  zh_cn: {
    hero: {
      eyebrow: "应用设置",
      title: "调整管理器的工作方式",
      description:
        "后台保护与窗口关闭偏好会正式保存；其余标记为预览的选项只在当前会话中生效。",
      statusLabel: "设置保存状态",
      dirty: "存在本次会话改动",
      pristine: "使用默认预览值",
      reset: "重置预览",
    },
    appearance: {
      title: "界面偏好",
      description:
        "主题模式与界面语言会立即保存并长期生效；其余显示密度类选项只是本次会话的预览，正式保存前不写入配置文件。",
      theme: {
        label: "主题模式",
        hint: "立即生效并长期保存，不受下方预览选项的重置影响。",
        light: "浅色模式",
        dark: "深色模式",
        system: "跟随系统",
      },
      language: {
        label: "界面语言",
        hint: "立即生效并长期保存；语言名称始终以其自身语言显示。",
        followSystem: (systemNativeName) => `跟随系统（${systemNativeName}）`,
        toastTitle: "界面语言已更新",
        toastMessage: (nativeName) => `当前界面语言：${nativeName}。`,
      },
      compactPanels: {
        title: "紧凑面板",
        description: "减少卡片内边距，适合小窗口或 Steam Deck 桌面模式。",
      },
      reduceMotion: {
        title: "减少动效",
        description: "降低页面过渡和 hover 动画强度。未来应与系统无障碍偏好合并。",
      },
      startPage: {
        label: "启动后打开",
        dashboard: "工作台",
        mods: "Mod 管理",
        last: "上次页面",
      },
    },
    windowBehavior: {
      title: "窗口行为",
      description: "控制点击窗口关闭按钮时的默认动作；这不会改变后台守护是否已启用。",
      closeLabel: "关闭主窗口时",
      ask: "每次询问",
      tray: "收起至托盘",
      exit: "退出应用",
      saveError: "关闭行为偏好保存失败，请检查应用存储权限后重试。",
      note: "关闭行为偏好与后台保护是独立设置；退出后的保护状态以“存档备份”区域为准。",
    },
    modImport: {
      title: "Mod 导入",
      description: "这些选项只影响未来导入流程的前端意图表达，不在前端判断文件安全。",
      previewAfterImport: {
        title: "导入后显示预览",
        description: "导入完成后优先展示预览图和结构摘要。预览图校验仍应由后端完成。",
      },
      confirmBeforeConflict: {
        title: "冲突前二次确认",
        description: "当安装计划存在冲突时，在继续前显示确认步骤。",
      },
    },
    prerequisites: {
      title: "前置环境",
      description:
        "只读检查当前已配置游戏目录中的 Stracker's Loader 与 CRCBypass，不访问测试目录。",
    },
    saveBackup: {
      title: "存档备份",
      description: "后台保护会正式保存；安装前提醒仍是当前会话预览，不读取真实存档。",
      backupReminder: {
        title: "安装前提醒备份",
        description: "在执行会写入游戏目录的任务前提示检查存档备份状态。",
      },
    },
    logs: {
      title: "日志与诊断",
      description: "诊断包导出需要后端脱敏能力，本页不会生成或写入任何日志文件。",
      exportNote: "正式导出前必须经过统一脱敏，并由用户主动触发。",
    },
  },
  en: {
    hero: {
      eyebrow: "App Settings",
      title: "Tune how the manager works",
      description:
        "Background protection and the window-close preference are saved for real; options marked as previews only apply to the current session.",
      statusLabel: "Settings save status",
      dirty: "Session changes pending",
      pristine: "Using default preview values",
      reset: "Reset previews",
    },
    appearance: {
      title: "Interface preferences",
      description:
        "Theme and interface language are saved immediately and persist; the remaining display-density options are session-only previews and are not written to the config file.",
      theme: {
        label: "Theme mode",
        hint: "Takes effect immediately and persists; unaffected by resetting the preview options below.",
        light: "Light",
        dark: "Dark",
        system: "Follow system",
      },
      language: {
        label: "Interface language",
        hint: "Takes effect immediately and persists; language names are always shown in their own language.",
        followSystem: (systemNativeName) => `Follow system (${systemNativeName})`,
        toastTitle: "Interface language updated",
        toastMessage: (nativeName) => `Interface language is now ${nativeName}.`,
      },
      compactPanels: {
        title: "Compact panels",
        description: "Reduce card padding for small windows or Steam Deck desktop mode.",
      },
      reduceMotion: {
        title: "Reduce motion",
        description:
          "Tone down page transitions and hover animations. Should eventually merge with the system accessibility preference.",
      },
      startPage: {
        label: "Open on launch",
        dashboard: "Dashboard",
        mods: "Mod library",
        last: "Last page",
      },
    },
    windowBehavior: {
      title: "Window behavior",
      description:
        "Controls the default action when the window close button is clicked; it does not change whether background protection is enabled.",
      closeLabel: "When closing the main window",
      ask: "Ask every time",
      tray: "Minimize to tray",
      exit: "Exit the app",
      saveError:
        "Failed to save the window-close preference. Check the app's storage permissions and try again.",
      note: "The window-close preference and background protection are independent settings; after exit, protection status is shown in the “Save backups” section.",
    },
    modImport: {
      title: "Mod import",
      description:
        "These options only express frontend intent for future import flows; file safety is never judged in the frontend.",
      previewAfterImport: {
        title: "Show preview after import",
        description:
          "Prefer showing preview images and a structure summary after an import completes. Preview validation still belongs to the backend.",
      },
      confirmBeforeConflict: {
        title: "Confirm before conflicts",
        description:
          "When an install plan contains conflicts, show a confirmation step before continuing.",
      },
    },
    prerequisites: {
      title: "Prerequisites",
      description:
        "Read-only checks for Stracker's Loader and CRCBypass in the configured game directory; test directories are never accessed.",
    },
    saveBackup: {
      title: "Save backups",
      description:
        "Background protection is saved for real; the pre-install reminder is still a session preview and never reads real saves.",
      backupReminder: {
        title: "Remind to back up before installs",
        description:
          "Prompt to check the save-backup status before tasks that write to the game directory.",
      },
    },
    logs: {
      title: "Logs & diagnostics",
      description:
        "Exporting a diagnostic bundle requires backend redaction; this page never creates or writes log files.",
      exportNote: "Exports must pass unified redaction and be explicitly triggered by the user.",
    },
  },
  ja: {
    hero: {
      eyebrow: "アプリ設定",
      title: "マネージャーの動作を調整",
      description:
        "バックグラウンド保護とウィンドウを閉じる際の設定は正式に保存されます。プレビューと記載された項目は現在のセッションのみ有効です。",
      statusLabel: "設定の保存状態",
      dirty: "このセッションでの変更あり",
      pristine: "既定のプレビュー値を使用中",
      reset: "プレビューをリセット",
    },
    appearance: {
      title: "インターフェース設定",
      description:
        "テーマと表示言語は即時保存され長期的に有効です。その他の表示密度系オプションはセッション内プレビューで、正式保存まで設定ファイルには書き込まれません。",
      theme: {
        label: "テーマモード",
        hint: "即時に反映され長期保存されます。下のプレビュー項目のリセットの影響を受けません。",
        light: "ライトモード",
        dark: "ダークモード",
        system: "システムに従う",
      },
      language: {
        label: "表示言語",
        hint: "即時に反映され長期保存されます。言語名は常にその言語自身で表示されます。",
        followSystem: (systemNativeName) => `システムに従う（${systemNativeName}）`,
        toastTitle: "表示言語を更新しました",
        toastMessage: (nativeName) => `現在の表示言語：${nativeName}。`,
      },
      compactPanels: {
        title: "コンパクトパネル",
        description: "カードの余白を減らし、小さいウィンドウや Steam Deck デスクトップモードに適します。",
      },
      reduceMotion: {
        title: "アニメーションを減らす",
        description:
          "ページ遷移やホバーアニメーションを抑えます。将来的にはシステムのアクセシビリティ設定との統合を予定しています。",
      },
      startPage: {
        label: "起動後に開くページ",
        dashboard: "ダッシュボード",
        mods: "Mod 管理",
        last: "前回のページ",
      },
    },
    windowBehavior: {
      title: "ウィンドウ動作",
      description:
        "閉じるボタンを押したときの既定動作を設定します。バックグラウンド保護の有効状態は変わりません。",
      closeLabel: "メインウィンドウを閉じるとき",
      ask: "毎回確認",
      tray: "トレイに格納",
      exit: "アプリを終了",
      saveError:
        "ウィンドウを閉じる設定の保存に失敗しました。アプリのストレージ権限を確認して再試行してください。",
      note: "ウィンドウを閉じる設定とバックグラウンド保護は独立した設定です。終了後の保護状態は「セーブバックアップ」セクションをご確認ください。",
    },
    modImport: {
      title: "Mod インポート",
      description:
        "これらのオプションは今後のインポートにおけるフロントエンドの意図表明のみで、ファイルの安全性はフロントエンドでは判断しません。",
      previewAfterImport: {
        title: "インポート後にプレビューを表示",
        description:
          "インポート完了後にプレビュー画像と構成サマリーを優先表示します。プレビューの検証はバックエンドが担当します。",
      },
      confirmBeforeConflict: {
        title: "競合時に再確認",
        description: "インストール計画に競合がある場合、続行前に確認ステップを表示します。",
      },
    },
    prerequisites: {
      title: "前提環境",
      description:
        "設定済みゲームディレクトリ内の Stracker's Loader と CRCBypass を読み取り専用で確認します。テストディレクトリにはアクセスしません。",
    },
    saveBackup: {
      title: "セーブバックアップ",
      description:
        "バックグラウンド保護は正式に保存されます。インストール前のリマインダーはセッション内プレビューで、実際のセーブデータは読み取りません。",
      backupReminder: {
        title: "インストール前にバックアップを確認",
        description: "ゲームディレクトリへ書き込むタスクの前に、バックアップ状態の確認を促します。",
      },
    },
    logs: {
      title: "ログと診断",
      description:
        "診断バンドルのエクスポートにはバックエンドの秘匿化処理が必要です。このページがログファイルを生成・書き込みすることはありません。",
      exportNote: "正式なエクスポートは統一秘匿化を経て、ユーザーが明示的に実行する必要があります。",
    },
  },
} satisfies LocaleDictionary<SettingsPageCopy>;
