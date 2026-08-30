import type { LocaleDictionary } from "../../shared/i18n";

// Mod 删除（#276）的全部用户可见文案。后端只出稳定错误码
// （mod_delete_*），文案在此按码表三语言兜底；不得在组件里硬编码。

export type ModDeleteErrorCopy = {
  message: string;
  hint?: string;
};

export type ModDeleteCopy = {
  dialog: {
    singleTitle: string;
    batchTitle: string;
    closeAria: string;
    cancel: string;
    confirm: string;
    confirmBusy: string;
    body: string;
    metricRevisions: string;
    metricCategories: string;
    affectedProfiles: string;
    affectedProfilesEmpty: string;
    batchBody: string;
    skipInstalled: string;
    skipPreviewUnavailable: string;
    retainedAudit: string;
  };
  menu: {
    delete: string;
    deleteBlockedInstalled: string;
  };
  errors: {
    fallback: ModDeleteErrorCopy;
    codes: Record<string, ModDeleteErrorCopy>;
  };
  toasts: {
    deletedTitle: (count: number) => string;
    deletedMessage: string;
    deleteFailedTitle: (count: number) => string;
    deleteFailedMessage: string;
  };
};

export const modDeleteCopy = {
  zh_cn: {
    dialog: {
      singleTitle: "删除 Mod",
      batchTitle: "删除选中的 Mod",
      closeAria: "关闭删除确认",
      cancel: "取消",
      confirm: "删除",
      confirmBusy: "删除中…",
      body: "将从库中移除该 Mod 及其全部版本，并回收已提取的包内容与预览图。游戏目录不受影响；审计记录按保留策略留存。",
      metricRevisions: "版本数",
      metricCategories: "分类数",
      affectedProfiles: "有安装记录的配置档",
      affectedProfilesEmpty: "无（各配置档均未安装）",
      batchBody: "将从库中移除以下选中的 Mod（含全部版本与已提取的包内容）：",
      skipInstalled: "已安装——将跳过，请先卸载",
      skipPreviewUnavailable: "无法读取该 Mod 的删除预览，将跳过",
      retainedAudit: "操作会记入审计日志（保留 90 天）。",
    },
    menu: {
      delete: "删除 Mod",
      deleteBlockedInstalled: "已安装——先卸载后才能删除",
    },
    errors: {
      fallback: { message: "删除失败，请稍后重试。" },
      codes: {
        mod_delete_blocked_installed: {
          message: "该 Mod 仍有安装记录，不能直接删除。",
          hint: "请先在库中卸载该 Mod，再执行删除。",
        },
        mod_delete_blocked_recovery: {
          message: "该 Mod 存在未完成的安装/恢复状态，暂时不能删除。",
          hint: "请先在恢复中心处理对应的恢复项。",
        },
        mod_delete_target_not_found: {
          message: "该 Mod 已不在库中，无需重复删除。",
        },
        mod_delete_store_unavailable: {
          message: "删除所需的存储暂时不可用。",
          hint: "请稍后重试；若持续出现，请连同诊断信息反馈。",
        },
      },
    },
    toasts: {
      deletedTitle: (count: number) => `已删除 ${count} 个 Mod`,
      deletedMessage: "库条目、版本与已提取的包内容均已回收。",
      deleteFailedTitle: (count: number) => `${count} 个 Mod 删除失败`,
      deleteFailedMessage: "其余选中的 Mod 未受影响；失败原因见逐项提示。",
    },
  },
  en: {
    dialog: {
      singleTitle: "Delete Mod",
      batchTitle: "Delete selected Mods",
      closeAria: "Close delete confirmation",
      cancel: "Cancel",
      confirm: "Delete",
      confirmBusy: "Deleting…",
      body: "Removes the Mod and all of its revisions from the library, and reclaims the extracted package content and preview images. The game directory is not touched; audit records are retained per policy.",
      metricRevisions: "Revisions",
      metricCategories: "Categories",
      affectedProfiles: "Profiles with install records",
      affectedProfilesEmpty: "None (not installed in any profile)",
      batchBody: "The following selected Mods will be removed from the library (including all revisions and extracted package content):",
      skipInstalled: "Installed — will be skipped; uninstall first",
      skipPreviewUnavailable: "Deletion preview unavailable — will be skipped",
      retainedAudit: "The operation is recorded in the audit log (90-day retention).",
    },
    menu: {
      delete: "Delete Mod",
      deleteBlockedInstalled: "Installed — uninstall before deleting",
    },
    errors: {
      fallback: { message: "Deletion failed. Please try again later." },
      codes: {
        mod_delete_blocked_installed: {
          message: "This Mod still has install records and cannot be deleted directly.",
          hint: "Uninstall the Mod in the library first, then delete it.",
        },
        mod_delete_blocked_recovery: {
          message: "This Mod has a pending install/recovery state and cannot be deleted yet.",
          hint: "Resolve the matching recovery item in the recovery center first.",
        },
        mod_delete_target_not_found: {
          message: "This Mod is no longer in the library; nothing to delete.",
        },
        mod_delete_store_unavailable: {
          message: "The storage required for deletion is temporarily unavailable.",
          hint: "Retry later; if it persists, report it with diagnostics.",
        },
      },
    },
    toasts: {
      deletedTitle: (count: number) => `Deleted ${count} ${count === 1 ? "Mod" : "Mods"}`,
      deletedMessage: "Library entries, revisions and extracted package content were reclaimed.",
      deleteFailedTitle: (count: number) => `${count} deletion${count === 1 ? "" : "s"} failed`,
      deleteFailedMessage: "The other selected Mods were not affected; see per-item reasons.",
    },
  },
  ja: {
    dialog: {
      singleTitle: "Mod を削除",
      batchTitle: "選択した Mod を削除",
      closeAria: "削除確認を閉じる",
      cancel: "キャンセル",
      confirm: "削除",
      confirmBusy: "削除中…",
      body: "この Mod とその全リビジョンをライブラリから削除し、展開済みパッケージ内容とプレビュー画像を回収します。ゲームディレクトリには影響しません。監査記録は保持ポリシーに従い残ります。",
      metricRevisions: "リビジョン数",
      metricCategories: "カテゴリ数",
      affectedProfiles: "インストール記録のあるプロファイル",
      affectedProfilesEmpty: "なし（全プロファイルで未インストール）",
      batchBody: "以下の選択した Mod をライブラリから削除します（全リビジョンと展開済みパッケージ内容を含む）：",
      skipInstalled: "インストール済み — スキップします。先にアンインストールしてください",
      skipPreviewUnavailable: "削除プレビューを取得できないためスキップします",
      retainedAudit: "この操作は監査ログに記録されます（90 日保持）。",
    },
    menu: {
      delete: "Mod を削除",
      deleteBlockedInstalled: "インストール済み — 先にアンインストールしてください",
    },
    errors: {
      fallback: { message: "削除に失敗しました。しばらくしてから再試行してください。" },
      codes: {
        mod_delete_blocked_installed: {
          message: "この Mod にはインストール記録があり、直接削除できません。",
          hint: "先にライブラリでアンインストールしてから削除してください。",
        },
        mod_delete_blocked_recovery: {
          message: "この Mod には未完了のインストール/リカバリー状態があり、削除できません。",
          hint: "先にリカバリーセンターで該当する復旧項目を処理してください。",
        },
        mod_delete_target_not_found: {
          message: "この Mod は既にライブラリに存在しません。",
        },
        mod_delete_store_unavailable: {
          message: "削除に必要なストレージが一時的に利用できません。",
          hint: "しばらくしてから再試行してください。継続する場合は診断情報と併せて報告してください。",
        },
      },
    },
    toasts: {
      deletedTitle: (count: number) => `${count} 件の Mod を削除しました`,
      deletedMessage: "ライブラリ項目・リビジョン・展開済みパッケージ内容を回収しました。",
      deleteFailedTitle: (count: number) => `${count} 件の削除に失敗しました`,
      deleteFailedMessage: "他の選択した Mod には影響ありません。失敗理由は各項目を参照してください。",
    },
  },
} satisfies LocaleDictionary<ModDeleteCopy>;
