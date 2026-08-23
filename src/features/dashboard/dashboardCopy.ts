import type { LocaleDictionary } from "../../shared/i18n";

// 工作台首页（页头、Hero 卡、模块预览、设置状态栏、设置步骤/日志、安装健康面板）的
// 全部用户可见文案。安装健康的 issue 标签复用 recoveryCenterCopy.issues，避免双表漂移。
// 支持卡的值「Monster Hunter: World - Iceborne」「Windows」为专有名词保持原文。

export type DashboardCopy = {
  page: {
    title: string;
    subtitle: string;
  };
  hero: {
    launchGroupAria: string;
    launching: string;
    launchButton: string;
    supportAria: string;
    launchStates: {
      readyStatus: string;
      readyDescription: string;
      validatingStatus: string;
      validatingDescription: string;
      invalidStatus: string;
      notConfiguredStatus: string;
      blockedDescription: string;
    };
    setupStates: {
      configuredBadge: string;
      configuredDescription: (pathLabel: string) => string;
      validatingBadge: string;
      validatingTitle: string;
      validatingDescription: string;
      invalidBadge: string;
      invalidTitle: string;
      invalidFallbackDescription: string;
      notConfiguredBadge: string;
      notConfiguredTitle: string;
      notConfiguredDescription: string;
    };
  };
  supportCards: {
    currentGame: string;
    currentPlatform: string;
    experimentalReserved: string;
  };
  modulePreview: {
    title: string;
    description: string;
    heading: string;
    cards: {
      modOverview: string;
      conflictStatus: string;
      prerequisiteCheck: string;
      recentBackup: string;
    };
  };
  setupPanel: {
    railAria: string;
    eyebrow: string;
    title: string;
    description: string;
    nextStepTitle: string;
    summaryTitle: string;
    statusLabel: string;
    riskLabel: string;
    logTitle: string;
    states: {
      configured: {
        title: string;
        description: (displayName: string, pathLabel: string) => string;
        badge: string;
        stepLabel: string;
        summaryStatus: string;
        summaryRisk: string;
        noteTitle: string;
        noteBody: string;
      };
      validating: {
        title: string;
        description: string;
        badge: string;
        stepLabel: string;
        summaryStatus: string;
        summaryRisk: string;
        noteTitle: string;
        noteBody: string;
      };
      invalid: {
        title: string;
        fallbackDescription: string;
        badge: string;
        stepLabel: string;
        summaryStatus: string;
        summaryRisk: string;
        noteTitle: string;
        noteBody: string;
      };
      notConfigured: {
        title: string;
        defaultDescription: string;
        badge: string;
        stepLabel: string;
        summaryStatus: string;
        summaryRisk: string;
        noteTitle: string;
        noteBody: string;
      };
    };
  };
  steps: Array<{ title: string; meta: string }>;
  logs: Array<{ time: string; message: string; muted?: boolean }>;
  recoveryHealth: {
    title: string;
    loadingBadge: string;
    loadingBody: string;
    unknownBadge: string;
    unavailableBody: string;
    metricsAria: string;
    metricScanned: string;
    metricAttention: string;
    metricUnknown: string;
    metricIssues: string;
    issuesAria: string;
    emptyBadge: string;
    emptyDescription: string;
    attentionBadge: string;
    attentionDescriptionUnknown: string;
    attentionDescriptionRepair: string;
    healthyBadge: string;
    healthyDescription: (count: number) => string;
  };
};

export const dashboardCopy = {
  zh_cn: {
    page: {
      title: "工作台",
      subtitle: "首次启动需要先完成游戏目录识别。",
    },
    hero: {
      launchGroupAria: "游戏启动",
      launching: "正在启动",
      launchButton: "启动游戏",
      supportAria: "支持信息",
      launchStates: {
        readyStatus: "已准备就绪",
        readyDescription: "当前配置档可用，游戏目录已通过校验。",
        validatingStatus: "等待目录校验",
        validatingDescription: "目录校验完成后即可启动。",
        invalidStatus: "需要重新选择目录",
        notConfiguredStatus: "等待目录配置",
        blockedDescription: "配置游戏目录后即可启动。",
      },
      setupStates: {
        configuredBadge: "目录已配置",
        configuredDescription: (pathLabel: string) => `当前目录：${pathLabel}`,
        validatingBadge: "正在校验",
        validatingTitle: "正在验证游戏目录",
        validatingDescription: "Helsincy 正在确认所选目录是否包含 MHW:I 可执行文件。",
        invalidBadge: "校验失败",
        invalidTitle: "目录校验未通过",
        invalidFallbackDescription: "请选择正确的游戏安装目录。",
        notConfiguredBadge: "目录未配置",
        notConfiguredTitle: "未找到游戏目录",
        notConfiguredDescription: "需要先识别《怪物猎人：世界 冰原》的安装目录，才能导入和安装 Mod。",
      },
    },
    supportCards: {
      currentGame: "当前支持",
      currentPlatform: "当前平台",
      experimentalReserved: "实验性支持预留",
    },
    modulePreview: {
      title: "完成设置后将显示",
      description: "以下模块会在目录识别、权限校验和默认配置档案创建后启用。",
      heading: "设置完成后启用",
      cards: {
        modOverview: "Mod 概览",
        conflictStatus: "冲突状态",
        prerequisiteCheck: "前置检查",
        recentBackup: "最近备份",
      },
    },
    setupPanel: {
      railAria: "首次启动设置状态",
      eyebrow: "首次启动",
      title: "设置状态",
      description: "Helsincy 需要先完成几项检查，才能启用模组管理。",
      nextStepTitle: "下一步",
      summaryTitle: "设置摘要",
      statusLabel: "状态",
      riskLabel: "风险",
      logTitle: "设置日志",
      states: {
        configured: {
          title: "游戏目录已保存",
          description: (displayName: string, pathLabel: string) =>
            `已识别 ${displayName}，目录摘要：${pathLabel}。`,
          badge: "配置完成",
          stepLabel: "第 4 / 4 步",
          summaryStatus: "已配置",
          summaryRisk: "低：等待 Mod 导入",
          noteTitle: "可以继续",
          noteBody: "游戏目录配置已经保存，后续导入、安装和备份功能会基于该配置继续启用。",
        },
        validating: {
          title: "正在验证目录",
          description: "正在检查所选目录是否包含 MHW:I 可执行文件。",
          badge: "校验中",
          stepLabel: "第 2 / 4 步",
          summaryStatus: "校验中",
          summaryRisk: "中：等待结果",
          noteTitle: "正在检查",
          noteBody: "当前只读取玩家主动选择的目录，不会写入游戏目录或读取存档。",
        },
        invalid: {
          title: "目录校验失败",
          fallbackDescription: "未知错误",
          badge: "需要重新选择",
          stepLabel: "第 2 / 4 步",
          summaryStatus: "未通过",
          summaryRisk: "高：目录不可用",
          noteTitle: "检查未通过",
          noteBody: "请选择包含 MonsterHunterWorld.exe 的游戏安装目录。当前失败不会保存为有效配置。",
        },
        notConfigured: {
          title: "等待选择游戏目录",
          defaultDescription: "尚未选择游戏目录。自动扫描暂未启用时，请先手动选择 MHW:I 安装目录。",
          badge: "等待主区操作",
          stepLabel: "第 1 / 4 步",
          summaryStatus: "未配置",
          summaryRisk: "风险：等待检查",
          noteTitle: "检查等待中",
          noteBody: "将在设置过程中检查游戏可执行文件和配置存储，但不会写入真实游戏目录。",
        },
      },
    },
    steps: [
      { title: "扫描 Steam 游戏库", meta: "检测已安装游戏和可用候选项。" },
      { title: "验证游戏目录", meta: "确认可执行文件、数据目录和写入权限。" },
      { title: "创建默认配置档案", meta: "在导入前准备一份干净的基线。" },
      { title: "开始导入模组", meta: "仅在目录和配置检查通过后启用。" },
    ],
    logs: [
      { time: "09:42", message: "首次启动设置已打开" },
      { time: "09:42", message: "等待扫描 Steam 游戏库" },
      { time: "--:--", message: "尚未选择游戏目录", muted: true },
    ],
    recoveryHealth: {
      title: "安装健康",
      loadingBadge: "检查中",
      loadingBody: "正在读取当前配置档的托管安装摘要。",
      unknownBadge: "状态未知",
      unavailableBody: "无法读取当前配置档的恢复摘要。",
      metricsAria: "安装恢复摘要",
      metricScanned: "扫描",
      metricAttention: "需处理",
      metricUnknown: "未知",
      metricIssues: "问题",
      issuesAria: "恢复问题聚合",
      emptyBadge: "无托管记录",
      emptyDescription: "当前配置档没有托管安装记录。",
      attentionBadge: "需要处理",
      attentionDescriptionUnknown: "存在无法确认的托管安装状态。",
      attentionDescriptionRepair: "存在需要修复的托管安装状态。",
      healthyBadge: "正常",
      healthyDescription: (count: number) => `${count} 个托管 Mod 状态一致。`,
    },
  },
  en: {
    page: {
      title: "Workbench",
      subtitle: "First launch requires identifying the game directory first.",
    },
    hero: {
      launchGroupAria: "Game launch",
      launching: "Launching",
      launchButton: "Launch game",
      supportAria: "Support info",
      launchStates: {
        readyStatus: "Ready",
        readyDescription: "The current profile is available and the game directory passed validation.",
        validatingStatus: "Waiting for directory validation",
        validatingDescription: "Launch becomes available once the directory validation finishes.",
        invalidStatus: "Directory needs reselection",
        notConfiguredStatus: "Waiting for directory setup",
        blockedDescription: "Launch becomes available once the game directory is configured.",
      },
      setupStates: {
        configuredBadge: "Directory configured",
        configuredDescription: (pathLabel: string) => `Current directory: ${pathLabel}`,
        validatingBadge: "Validating",
        validatingTitle: "Validating game directory",
        validatingDescription: "Helsincy is confirming whether the selected directory contains the MHW:I executable.",
        invalidBadge: "Validation failed",
        invalidTitle: "Directory validation failed",
        invalidFallbackDescription: "Select the correct game installation directory.",
        notConfiguredBadge: "Directory not configured",
        notConfiguredTitle: "Game directory not found",
        notConfiguredDescription: "The Monster Hunter World: Iceborne installation directory must be identified before importing and installing mods.",
      },
    },
    supportCards: {
      currentGame: "Supported game",
      currentPlatform: "Current platform",
      experimentalReserved: "Reserved for experimental support",
    },
    modulePreview: {
      title: "Shown after setup completes",
      description: "These modules are enabled after directory identification, permission validation, and default profile creation.",
      heading: "Enabled after setup",
      cards: {
        modOverview: "Mod overview",
        conflictStatus: "Conflict status",
        prerequisiteCheck: "Prerequisite check",
        recentBackup: "Recent backups",
      },
    },
    setupPanel: {
      railAria: "First-launch setup status",
      eyebrow: "First launch",
      title: "Setup status",
      description: "Helsincy needs to finish a few checks before mod management can be enabled.",
      nextStepTitle: "Next step",
      summaryTitle: "Setup summary",
      statusLabel: "Status",
      riskLabel: "Risk",
      logTitle: "Setup log",
      states: {
        configured: {
          title: "Game directory saved",
          description: (displayName: string, pathLabel: string) =>
            `Identified ${displayName}; directory summary: ${pathLabel}.`,
          badge: "Setup complete",
          stepLabel: "Step 4 / 4",
          summaryStatus: "Configured",
          summaryRisk: "Low: waiting for mod import",
          noteTitle: "Ready to continue",
          noteBody: "The game directory configuration is saved. Import, install, and backup features build on it from here.",
        },
        validating: {
          title: "Validating directory",
          description: "Checking whether the selected directory contains the MHW:I executable.",
          badge: "Validating",
          stepLabel: "Step 2 / 4",
          summaryStatus: "Validating",
          summaryRisk: "Medium: waiting for result",
          noteTitle: "Checking",
          noteBody: "Only the directory the player actively selected is read; no game directory writes or save data reads happen.",
        },
        invalid: {
          title: "Directory validation failed",
          fallbackDescription: "Unknown error",
          badge: "Reselection needed",
          stepLabel: "Step 2 / 4",
          summaryStatus: "Failed",
          summaryRisk: "High: directory unavailable",
          noteTitle: "Check failed",
          noteBody: "Select the game installation directory containing MonsterHunterWorld.exe. This failure is not saved as a valid configuration.",
        },
        notConfigured: {
          title: "Waiting for game directory selection",
          defaultDescription: "No game directory selected yet. While auto-scan is unavailable, select the MHW:I installation directory manually first.",
          badge: "Waiting for main-area action",
          stepLabel: "Step 1 / 4",
          summaryStatus: "Not configured",
          summaryRisk: "Risk: waiting for checks",
          noteTitle: "Checks pending",
          noteBody: "The game executable and configuration storage are checked during setup, without writing to the real game directory.",
        },
      },
    },
    steps: [
      { title: "Scan the Steam library", meta: "Detect installed games and available candidates." },
      { title: "Validate the game directory", meta: "Confirm the executable, data directories, and write permissions." },
      { title: "Create the default profile", meta: "Prepare a clean baseline before importing." },
      { title: "Start importing mods", meta: "Enabled only after the directory and configuration checks pass." },
    ],
    logs: [
      { time: "09:42", message: "First-launch setup opened" },
      { time: "09:42", message: "Waiting to scan the Steam library" },
      { time: "--:--", message: "No game directory selected yet", muted: true },
    ],
    recoveryHealth: {
      title: "Install health",
      loadingBadge: "Checking",
      loadingBody: "Reading the managed install summary of the current profile.",
      unknownBadge: "State unknown",
      unavailableBody: "The recovery summary of the current profile cannot be read.",
      metricsAria: "Install recovery summary",
      metricScanned: "Scanned",
      metricAttention: "Action needed",
      metricUnknown: "Unknown",
      metricIssues: "Issues",
      issuesAria: "Aggregated recovery issues",
      emptyBadge: "No managed records",
      emptyDescription: "The current profile has no managed install records.",
      attentionBadge: "Action needed",
      attentionDescriptionUnknown: "Some managed install states cannot be confirmed.",
      attentionDescriptionRepair: "Some managed install states need repair.",
      healthyBadge: "Healthy",
      healthyDescription: (count: number) => `${count} managed mod(s) are consistent.`,
    },
  },
  ja: {
    page: {
      title: "ワークベンチ",
      subtitle: "初回起動ではまずゲームディレクトリの識別が必要です。",
    },
    hero: {
      launchGroupAria: "ゲーム起動",
      launching: "起動中",
      launchButton: "ゲームを起動",
      supportAria: "サポート情報",
      launchStates: {
        readyStatus: "準備完了",
        readyDescription: "現在のプロファイルは利用可能で、ゲームディレクトリは検証を通過しました。",
        validatingStatus: "ディレクトリ検証待ち",
        validatingDescription: "ディレクトリの検証が完了すると起動できます。",
        invalidStatus: "ディレクトリの再選択が必要",
        notConfiguredStatus: "ディレクトリ設定待ち",
        blockedDescription: "ゲームディレクトリを設定すると起動できます。",
      },
      setupStates: {
        configuredBadge: "ディレクトリ設定済み",
        configuredDescription: (pathLabel: string) => `現在のディレクトリ：${pathLabel}`,
        validatingBadge: "検証中",
        validatingTitle: "ゲームディレクトリを検証中",
        validatingDescription: "Helsincy は選択したディレクトリに MHW:I の実行ファイルが含まれるか確認しています。",
        invalidBadge: "検証失敗",
        invalidTitle: "ディレクトリ検証を通過せず",
        invalidFallbackDescription: "正しいゲームインストールディレクトリを選択してください。",
        notConfiguredBadge: "ディレクトリ未設定",
        notConfiguredTitle: "ゲームディレクトリが見つかりません",
        notConfiguredDescription: "Mod のインポートとインストールには、先に『モンスターハンターワールド：アイスボーン』のインストールディレクトリの識別が必要です。",
      },
    },
    supportCards: {
      currentGame: "対応ゲーム",
      currentPlatform: "現在のプラットフォーム",
      experimentalReserved: "実験的サポート予約枠",
    },
    modulePreview: {
      title: "設定完了後に表示",
      description: "以下のモジュールは、ディレクトリ識別・権限検証・既定プロファイル作成の後に有効になります。",
      heading: "設定完了後に有効化",
      cards: {
        modOverview: "Mod 概要",
        conflictStatus: "競合状態",
        prerequisiteCheck: "前提チェック",
        recentBackup: "最近のバックアップ",
      },
    },
    setupPanel: {
      railAria: "初回起動セットアップ状態",
      eyebrow: "初回起動",
      title: "セットアップ状態",
      description: "Mod 管理を有効にする前に、Helsincy はいくつかのチェックを完了する必要があります。",
      nextStepTitle: "次のステップ",
      summaryTitle: "セットアップ概要",
      statusLabel: "状態",
      riskLabel: "リスク",
      logTitle: "セットアップログ",
      states: {
        configured: {
          title: "ゲームディレクトリを保存済み",
          description: (displayName: string, pathLabel: string) =>
            `${displayName} を識別しました。ディレクトリ概要：${pathLabel}。`,
          badge: "設定完了",
          stepLabel: "ステップ 4 / 4",
          summaryStatus: "設定済み",
          summaryRisk: "低：Mod インポート待ち",
          noteTitle: "続行できます",
          noteBody: "ゲームディレクトリの設定は保存済みです。以降のインポート・インストール・バックアップ機能はこの設定を基に有効になります。",
        },
        validating: {
          title: "ディレクトリを検証中",
          description: "選択したディレクトリに MHW:I の実行ファイルが含まれるか確認しています。",
          badge: "検証中",
          stepLabel: "ステップ 2 / 4",
          summaryStatus: "検証中",
          summaryRisk: "中：結果待ち",
          noteTitle: "確認中",
          noteBody: "現在はプレイヤーが選択したディレクトリの読み取りのみで、ゲームディレクトリへの書き込みやセーブデータの読み取りは行いません。",
        },
        invalid: {
          title: "ディレクトリ検証に失敗",
          fallbackDescription: "不明なエラー",
          badge: "再選択が必要",
          stepLabel: "ステップ 2 / 4",
          summaryStatus: "未通過",
          summaryRisk: "高：ディレクトリ利用不可",
          noteTitle: "チェック未通過",
          noteBody: "MonsterHunterWorld.exe を含むゲームインストールディレクトリを選択してください。今回の失敗は有効な設定として保存されません。",
        },
        notConfigured: {
          title: "ゲームディレクトリの選択待ち",
          defaultDescription: "ゲームディレクトリは未選択です。自動スキャンが無効な間は、先に MHW:I のインストールディレクトリを手動で選択してください。",
          badge: "メインエリアの操作待ち",
          stepLabel: "ステップ 1 / 4",
          summaryStatus: "未設定",
          summaryRisk: "リスク：チェック待ち",
          noteTitle: "チェック待機中",
          noteBody: "セットアップ中にゲーム実行ファイルと設定ストレージを確認しますが、実際のゲームディレクトリへは書き込みません。",
        },
      },
    },
    steps: [
      { title: "Steam ライブラリをスキャン", meta: "インストール済みゲームと利用可能な候補を検出します。" },
      { title: "ゲームディレクトリを検証", meta: "実行ファイル・データディレクトリ・書き込み権限を確認します。" },
      { title: "既定プロファイルを作成", meta: "インポート前にクリーンなベースラインを準備します。" },
      { title: "Mod のインポートを開始", meta: "ディレクトリと設定のチェック通過後にのみ有効になります。" },
    ],
    logs: [
      { time: "09:42", message: "初回起動セットアップを開始" },
      { time: "09:42", message: "Steam ライブラリのスキャン待ち" },
      { time: "--:--", message: "ゲームディレクトリは未選択", muted: true },
    ],
    recoveryHealth: {
      title: "インストール健全性",
      loadingBadge: "確認中",
      loadingBody: "現在のプロファイルの管理対象インストールサマリーを読み込み中です。",
      unknownBadge: "状態不明",
      unavailableBody: "現在のプロファイルの復旧サマリーを読み取れません。",
      metricsAria: "インストール復旧サマリー",
      metricScanned: "スキャン",
      metricAttention: "要対応",
      metricUnknown: "不明",
      metricIssues: "問題",
      issuesAria: "復旧問題の集計",
      emptyBadge: "管理対象記録なし",
      emptyDescription: "現在のプロファイルには管理対象インストール記録がありません。",
      attentionBadge: "要対応",
      attentionDescriptionUnknown: "確認できない管理対象インストール状態があります。",
      attentionDescriptionRepair: "修復が必要な管理対象インストール状態があります。",
      healthyBadge: "正常",
      healthyDescription: (count: number) => `${count} 件の管理対象 Mod の状態が一致しています。`,
    },
  },
} satisfies LocaleDictionary<DashboardCopy>;
