import type { LocaleDictionary } from "../../shared/i18n";
import type { ProfileSaveDirectoryNoticeKind } from "./ProfileSaveDirectoryDiscoveryProvider";
import type { ProfileDirectorySelectionDto } from "./profileSaveSettingsTypes";

// 存档目录（目录面板、Steam 账户候选列表、自动发现通知）的全部用户可见文案。
// 发现流程的 notice 只存语义 kind，文本在 toast 组装时经本字典取。

export type SaveDirectoryNoticeCopy = {
  title: string;
  message: string;
  detail: string;
};

export type SaveDirectoryCopy = {
  directoryStatus: Record<ProfileDirectorySelectionDto["status"], string>;
  panel: {
    title: string;
    subtitle: string;
    saveRowLabel: string;
    backupRowLabel: string;
    validating: string;
    choose: string;
    detecting: string;
    autoDetect: string;
    errorFallback: string;
  };
  candidates: {
    title: string;
    hint: string;
    accountUnavailable: string;
    recommended: string;
    choose: string;
    modifiedUnavailable: string;
    modifiedMinutesAgo: (minutes: number) => string;
    modifiedHoursAgo: (hours: number) => string;
    modifiedDaysAgo: (days: number) => string;
  };
  notices: Record<ProfileSaveDirectoryNoticeKind, SaveDirectoryNoticeCopy>;
  noticeActions: {
    reviewCandidates: string;
    retryDetection: string;
  };
};

export const saveDirectoryCopy = {
  zh_cn: {
    directoryStatus: {
      valid: "已配置",
      defaulted: "默认目录",
      invalid: "目录不可用",
      unset: "未选择",
    },
    panel: {
      title: "存档路径",
      subtitle: "源目录 / 备份目录",
      saveRowLabel: "游戏存档",
      backupRowLabel: "备份目录",
      validating: "校验中",
      choose: "选择",
      detecting: "检测中",
      autoDetect: "自动检测",
      errorFallback: "目录不可用",
    },
    candidates: {
      title: "选择 Steam 存档账户",
      hint: "按最近修改时间推荐，确认后写入当前配置档",
      accountUnavailable: "Steam 资料不可用",
      recommended: "推荐",
      choose: "选择此账户",
      modifiedUnavailable: "最近修改时间不可用",
      modifiedMinutesAgo: (minutes: number) => `${minutes} 分钟前修改`,
      modifiedHoursAgo: (hours: number) => `${hours} 小时前修改`,
      modifiedDaysAgo: (days: number) => `${days} 天前修改`,
    },
    notices: {
      preview_manual_only: {
        title: "自动检测仅在桌面端可用",
        message: "当前预览环境不会访问本地 Steam 存档目录。",
        detail: "可以继续使用手动选择入口调整界面状态。",
      },
      detect_failed: {
        title: "存档目录检测失败",
        message: "没有完成本次自动检测。",
        detail: "可以稍后重试，或继续手动选择存档目录。",
      },
      confirm_failed: {
        title: "候选确认失败",
        message: "所选 Steam 存档目录未能通过重新验证。",
        detail: "请重新检测，或使用手动选择入口。",
      },
      auto_saved_startup: {
        title: "已自动关联存档目录",
        message: "启动自检已完成，当前配置档可直接备份。",
        detail: "备份前仍会再次验证目录状态。",
      },
      auto_saved_manual: {
        title: "已自动关联存档目录",
        message: "存档目录已写入当前配置档。",
        detail: "备份前仍会再次验证目录状态。",
      },
      confirmation_required: {
        title: "发现多个 Steam 存档账户",
        message: "请选择要绑定到当前配置档的账户。",
        detail: "已按最近修改时间推荐候选，但仍需要你确认。",
      },
      not_found: {
        title: "未发现可用存档目录",
        message: "没有发现可用的 MHW:I Steam 存档目录。",
        detail: "可以重新检测，或继续使用手动选择入口。",
      },
      scan_failed: {
        title: "存档目录检测失败",
        message: "检测过程中遇到系统或读取问题。",
        detail: "可以稍后重试；如果 Steam 或游戏正在更新，请等待完成后再检测。",
      },
      existing_invalid: {
        title: "当前存档目录需要重新确认",
        message: "已保存的存档目录未能通过结构校验。",
        detail: "可以重新检测，或使用手动选择入口重新绑定。",
      },
      reconfirm_required: {
        title: "当前存档目录需要重新确认",
        message: "当前存档目录需要重新确认。",
        detail: "可以重新检测，或使用手动选择入口重新绑定。",
      },
    },
    noticeActions: {
      reviewCandidates: "查看候选",
      retryDetection: "重新检测",
    },
  },
  en: {
    directoryStatus: {
      valid: "Configured",
      defaulted: "Default directory",
      invalid: "Directory unavailable",
      unset: "Not selected",
    },
    panel: {
      title: "Save data paths",
      subtitle: "Source / backup directories",
      saveRowLabel: "Game save data",
      backupRowLabel: "Backup directory",
      validating: "Validating",
      choose: "Choose",
      detecting: "Detecting",
      autoDetect: "Auto detect",
      errorFallback: "Directory unavailable",
    },
    candidates: {
      title: "Choose a Steam save data account",
      hint: "Recommended by last modified time; the confirmed choice is written to the current profile",
      accountUnavailable: "Steam profile unavailable",
      recommended: "Recommended",
      choose: "Use this account",
      modifiedUnavailable: "Last modified time unavailable",
      modifiedMinutesAgo: (minutes: number) => `Modified ${minutes} min ago`,
      modifiedHoursAgo: (hours: number) => `Modified ${hours} h ago`,
      modifiedDaysAgo: (days: number) => `Modified ${days} d ago`,
    },
    notices: {
      preview_manual_only: {
        title: "Auto detection is desktop-only",
        message: "This preview environment does not access local Steam save data directories.",
        detail: "You can keep using the manual selection entry to adjust the UI state.",
      },
      detect_failed: {
        title: "Save data directory detection failed",
        message: "This auto detection did not finish.",
        detail: "Try again later, or keep selecting the save data directory manually.",
      },
      confirm_failed: {
        title: "Candidate confirmation failed",
        message: "The selected Steam save data directory failed re-validation.",
        detail: "Detect again, or use the manual selection entry.",
      },
      auto_saved_startup: {
        title: "Save data directory linked automatically",
        message: "The startup self-check finished; the current profile can back up right away.",
        detail: "The directory state is validated again before each backup.",
      },
      auto_saved_manual: {
        title: "Save data directory linked automatically",
        message: "The save data directory was written to the current profile.",
        detail: "The directory state is validated again before each backup.",
      },
      confirmation_required: {
        title: "Multiple Steam save data accounts found",
        message: "Choose the account to bind to the current profile.",
        detail: "Candidates are recommended by last modified time, but your confirmation is still required.",
      },
      not_found: {
        title: "No usable save data directory found",
        message: "No usable MHW:I Steam save data directory was found.",
        detail: "Detect again, or keep using the manual selection entry.",
      },
      scan_failed: {
        title: "Save data directory detection failed",
        message: "The detection ran into a system or read problem.",
        detail: "Try again later; if Steam or the game is updating, wait for it to finish first.",
      },
      existing_invalid: {
        title: "Current save data directory needs re-confirmation",
        message: "The saved save data directory failed the structure validation.",
        detail: "Detect again, or re-bind via the manual selection entry.",
      },
      reconfirm_required: {
        title: "Current save data directory needs re-confirmation",
        message: "The current save data directory needs to be re-confirmed.",
        detail: "Detect again, or re-bind via the manual selection entry.",
      },
    },
    noticeActions: {
      reviewCandidates: "View candidates",
      retryDetection: "Detect again",
    },
  },
  ja: {
    directoryStatus: {
      valid: "設定済み",
      defaulted: "既定ディレクトリ",
      invalid: "ディレクトリ利用不可",
      unset: "未選択",
    },
    panel: {
      title: "セーブデータパス",
      subtitle: "ソース / バックアップディレクトリ",
      saveRowLabel: "ゲームセーブデータ",
      backupRowLabel: "バックアップディレクトリ",
      validating: "検証中",
      choose: "選択",
      detecting: "検出中",
      autoDetect: "自動検出",
      errorFallback: "ディレクトリ利用不可",
    },
    candidates: {
      title: "Steam セーブデータアカウントを選択",
      hint: "最終更新時刻順に推奨。確認後に現在のプロファイルへ書き込みます",
      accountUnavailable: "Steam プロフィール利用不可",
      recommended: "推奨",
      choose: "このアカウントを使用",
      modifiedUnavailable: "最終更新時刻は不明",
      modifiedMinutesAgo: (minutes: number) => `${minutes} 分前に更新`,
      modifiedHoursAgo: (hours: number) => `${hours} 時間前に更新`,
      modifiedDaysAgo: (days: number) => `${days} 日前に更新`,
    },
    notices: {
      preview_manual_only: {
        title: "自動検出はデスクトップ専用",
        message: "このプレビュー環境はローカルの Steam セーブデータディレクトリへアクセスしません。",
        detail: "手動選択の入口で UI の状態を調整できます。",
      },
      detect_failed: {
        title: "セーブデータディレクトリの検出に失敗",
        message: "今回の自動検出は完了しませんでした。",
        detail: "後で再試行するか、手動でセーブデータディレクトリを選択してください。",
      },
      confirm_failed: {
        title: "候補の確認に失敗",
        message: "選択した Steam セーブデータディレクトリが再検証を通過しませんでした。",
        detail: "再検出するか、手動選択の入口を使用してください。",
      },
      auto_saved_startup: {
        title: "セーブデータディレクトリを自動関連付け",
        message: "起動時セルフチェックが完了し、現在のプロファイルはすぐにバックアップできます。",
        detail: "バックアップ前にもディレクトリ状態を再検証します。",
      },
      auto_saved_manual: {
        title: "セーブデータディレクトリを自動関連付け",
        message: "セーブデータディレクトリを現在のプロファイルへ書き込みました。",
        detail: "バックアップ前にもディレクトリ状態を再検証します。",
      },
      confirmation_required: {
        title: "複数の Steam セーブデータアカウントを検出",
        message: "現在のプロファイルに紐付けるアカウントを選択してください。",
        detail: "最終更新時刻順に候補を推奨していますが、確認が必要です。",
      },
      not_found: {
        title: "利用可能なセーブデータディレクトリなし",
        message: "利用可能な MHW:I の Steam セーブデータディレクトリが見つかりませんでした。",
        detail: "再検出するか、手動選択の入口を引き続き使用してください。",
      },
      scan_failed: {
        title: "セーブデータディレクトリの検出に失敗",
        message: "検出中にシステムまたは読み取りの問題が発生しました。",
        detail: "後で再試行してください。Steam やゲームが更新中の場合は完了を待ってください。",
      },
      existing_invalid: {
        title: "現在のセーブデータディレクトリは再確認が必要",
        message: "保存済みのセーブデータディレクトリが構造検証を通過しませんでした。",
        detail: "再検出するか、手動選択の入口で再度紐付けてください。",
      },
      reconfirm_required: {
        title: "現在のセーブデータディレクトリは再確認が必要",
        message: "現在のセーブデータディレクトリは再確認が必要です。",
        detail: "再検出するか、手動選択の入口で再度紐付けてください。",
      },
    },
    noticeActions: {
      reviewCandidates: "候補を表示",
      retryDetection: "再検出",
    },
  },
} satisfies LocaleDictionary<SaveDirectoryCopy>;
