import type { LocaleDictionary } from "../../shared/i18n";

// 前置环境面板的全部用户可见文案（I18N-01）。tone/icon 是语义不是文案，留在
// GamePrerequisitePanel 的语义映射里。后端 DTO 携带的 message 原样透传，不进字典；
// 这里的 fallbackMessage 只在后端未给 message 时使用。

export type GamePrerequisiteSummaryText = { label: string; description: string };

export type GamePrerequisiteCopy = {
  panelTitle: string;
  recheck: string;
  checking: string;
  configureFirst: string;
  itemVerifiedNote: string;
  summary: {
    loading: GamePrerequisiteSummaryText;
    rulesUnavailable: GamePrerequisiteSummaryText;
    directoryInvalid: GamePrerequisiteSummaryText;
    directoryNotWritable: GamePrerequisiteSummaryText;
    notConfigured: GamePrerequisiteSummaryText;
    verified: GamePrerequisiteSummaryText;
    warning: GamePrerequisiteSummaryText;
    error: GamePrerequisiteSummaryText;
  };
  noteHeading: {
    rulesUnavailable: string;
    directoryNotWritable: string;
    directoryInvalid: string;
  };
  fallbackMessage: {
    rulesUnavailable: string;
    directoryInvalid: string;
    directoryNotWritable: string;
  };
  itemStatus: {
    missing: string;
    misconfigured: string;
    installedVerified: string;
    installedUnverified: string;
  };
  issue: {
    missingRequiredFile: string;
    signatureUnverified: string;
    configReadFailed: string;
    configInvalidJson: string;
    configFieldMismatch: string;
    rulesUnavailable: string;
    rulesCorrupted: string;
  };
};

export const gamePrerequisiteCopy = {
  zh_cn: {
    panelTitle: "前置环境",
    recheck: "重新检查",
    checking: "正在检查前置环境…",
    configureFirst: "配置游戏目录后即可检查前置环境。",
    itemVerifiedNote: "关键文件、配置和已知签名都已通过检查。",
    summary: {
      loading: { label: "检查中", description: "只读检查当前已配置游戏目录中的已知前置文件。" },
      rulesUnavailable: { label: "规则不可用", description: "无法完成签名校验，但不会写入游戏目录。" },
      directoryInvalid: {
        label: "目录失效",
        description: "请先修正当前保存的游戏目录，再重新检查前置环境。",
      },
      directoryNotWritable: {
        label: "目录不可写",
        description: "游戏目录存在但当前写不进去，安装会被阻止。请先关闭游戏再重试。",
      },
      notConfigured: {
        label: "等待配置",
        description: "配置游戏目录后即可检查 Stracker's Loader 和 CRCBypass。",
      },
      verified: { label: "已验证", description: "两个已知前置都通过了文件、配置和签名检查。" },
      warning: {
        label: "存在警告",
        description: "已检测到前置文件，但至少有一个签名不在当前已知集合内。",
      },
      error: { label: "需要处理", description: "至少有一个前置缺失，或关键配置不正确。" },
    },
    noteHeading: {
      rulesUnavailable: "暂时无法读取前置规则。",
      directoryNotWritable: "游戏目录当前不可写。",
      directoryInvalid: "游戏目录当前不可用。",
    },
    fallbackMessage: {
      rulesUnavailable: "前置规则暂不可用。",
      directoryInvalid: "当前保存的游戏目录已失效，请重新选择。",
      directoryNotWritable:
        "游戏目录当前不可写。请先完全退出游戏与 Steam，确认目录未被设为只读或被安全软件占用后重试。",
    },
    itemStatus: {
      missing: "缺少必需文件",
      misconfigured: "配置不正确",
      installedVerified: "已安装，版本已验证",
      installedUnverified: "已安装，但版本未验证",
    },
    issue: {
      missingRequiredFile: "缺少必需文件",
      signatureUnverified: "签名未命中当前已知集合",
      configReadFailed: "配置文件无法读取",
      configInvalidJson: "配置文件不是有效 JSON",
      configFieldMismatch: "关键字段未满足 enablePluginLoader = true",
      rulesUnavailable: "规则暂不可用",
      rulesCorrupted: "规则文件已损坏",
    },
  },
  en: {
    panelTitle: "Prerequisites",
    recheck: "Check again",
    checking: "Checking prerequisites…",
    configureFirst: "Configure the game directory to check prerequisites.",
    itemVerifiedNote: "Key files, configuration, and known signatures all passed.",
    summary: {
      loading: {
        label: "Checking",
        description: "Read-only checks of known prerequisite files in the configured game directory.",
      },
      rulesUnavailable: {
        label: "Rules unavailable",
        description: "Signature verification cannot finish; the game directory is never written.",
      },
      directoryInvalid: {
        label: "Directory invalid",
        description: "Fix the saved game directory first, then check prerequisites again.",
      },
      directoryNotWritable: {
        label: "Directory not writable",
        description:
          "The game directory exists but cannot be written right now, so installs are blocked. Close the game and retry.",
      },
      notConfigured: {
        label: "Awaiting setup",
        description: "Configure the game directory to check Stracker's Loader and CRCBypass.",
      },
      verified: {
        label: "Verified",
        description: "Both known prerequisites passed file, configuration, and signature checks.",
      },
      warning: {
        label: "Warnings present",
        description:
          "Prerequisite files were detected, but at least one signature is outside the known set.",
      },
      error: {
        label: "Action needed",
        description: "At least one prerequisite is missing or a key configuration is incorrect.",
      },
    },
    noteHeading: {
      rulesUnavailable: "Prerequisite rules are temporarily unreadable.",
      directoryNotWritable: "The game directory is not writable right now.",
      directoryInvalid: "The game directory is currently unavailable.",
    },
    fallbackMessage: {
      rulesUnavailable: "Prerequisite rules are temporarily unavailable.",
      directoryInvalid: "The saved game directory is no longer valid. Please choose it again.",
      directoryNotWritable:
        "The game directory is not writable right now. Fully exit the game and Steam, make sure the directory is not read-only or locked by security software, then retry.",
    },
    itemStatus: {
      missing: "Required file missing",
      misconfigured: "Misconfigured",
      installedVerified: "Installed, version verified",
      installedUnverified: "Installed, version unverified",
    },
    issue: {
      missingRequiredFile: "Required file missing",
      signatureUnverified: "Signature not in the known set",
      configReadFailed: "Configuration file unreadable",
      configInvalidJson: "Configuration file is not valid JSON",
      configFieldMismatch: "Key field does not satisfy enablePluginLoader = true",
      rulesUnavailable: "Rules temporarily unavailable",
      rulesCorrupted: "Rules file corrupted",
    },
  },
  ja: {
    panelTitle: "前提環境",
    recheck: "再確認",
    checking: "前提環境を確認しています…",
    configureFirst: "ゲームディレクトリを設定すると前提環境を確認できます。",
    itemVerifiedNote: "主要ファイル、設定、既知の署名はすべてチェックを通過しました。",
    summary: {
      loading: {
        label: "確認中",
        description: "設定済みゲームディレクトリ内の既知の前提ファイルを読み取り専用で確認します。",
      },
      rulesUnavailable: {
        label: "ルールを利用できません",
        description: "署名検証を完了できませんが、ゲームディレクトリへの書き込みは行いません。",
      },
      directoryInvalid: {
        label: "ディレクトリが無効",
        description: "保存されているゲームディレクトリを修正してから、前提環境を再確認してください。",
      },
      directoryNotWritable: {
        label: "ディレクトリに書き込めません",
        description:
          "ゲームディレクトリは存在しますが現在書き込めないため、インストールはブロックされます。ゲームを終了してから再試行してください。",
      },
      notConfigured: {
        label: "設定待ち",
        description:
          "ゲームディレクトリを設定すると Stracker's Loader と CRCBypass を確認できます。",
      },
      verified: {
        label: "検証済み",
        description: "既知の前提 2 つがファイル・設定・署名のチェックを通過しました。",
      },
      warning: {
        label: "警告あり",
        description: "前提ファイルは検出されましたが、既知セットに含まれない署名が少なくとも 1 つあります。",
      },
      error: {
        label: "要対応",
        description: "少なくとも 1 つの前提が欠落しているか、重要な設定が正しくありません。",
      },
    },
    noteHeading: {
      rulesUnavailable: "前提ルールを一時的に読み取れません。",
      directoryNotWritable: "ゲームディレクトリに現在書き込めません。",
      directoryInvalid: "ゲームディレクトリが現在利用できません。",
    },
    fallbackMessage: {
      rulesUnavailable: "前提ルールは一時的に利用できません。",
      directoryInvalid: "保存されているゲームディレクトリが無効になりました。選び直してください。",
      directoryNotWritable:
        "ゲームディレクトリに現在書き込めません。ゲームと Steam を完全に終了し、ディレクトリが読み取り専用やセキュリティソフトの占有になっていないことを確認してから再試行してください。",
    },
    itemStatus: {
      missing: "必須ファイルが欠落",
      misconfigured: "設定が不正",
      installedVerified: "インストール済み・バージョン検証済み",
      installedUnverified: "インストール済み・バージョン未検証",
    },
    issue: {
      missingRequiredFile: "必須ファイルが欠落",
      signatureUnverified: "署名が既知セットに一致しません",
      configReadFailed: "設定ファイルを読み取れません",
      configInvalidJson: "設定ファイルが有効な JSON ではありません",
      configFieldMismatch: "重要フィールドが enablePluginLoader = true を満たしていません",
      rulesUnavailable: "ルールを一時的に利用できません",
      rulesCorrupted: "ルールファイルが破損しています",
    },
  },
} satisfies LocaleDictionary<GamePrerequisiteCopy>;
