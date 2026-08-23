import type { LocaleDictionary } from "../../shared/i18n";

export type DebugLogSettingsCopy = {
  loading: string;
  recheck: string;
  toggleTitle: string;
  toggleDescription: string;
  saving: string;
  enabled: string;
  disabled: string;
  errors: {
    unavailableRetry: string;
    saveFailed: string;
    unavailableRecheck: string;
  };
};

export const debugLogSettingsCopy = {
  zh_cn: {
    loading: "正在读取调试日志设置…",
    recheck: "重新检查",
    toggleTitle: "启用调试日志",
    toggleDescription:
      "仅在开启后写入受控的 Debug 事件；不会记录原始路径、错误正文、Manifest、Hash 或 Mod 内容。",
    saving: "正在保存…",
    enabled: "已启用",
    disabled: "已关闭",
    errors: {
      unavailableRetry: "调试日志设置暂时不可用，请稍后重试。",
      saveFailed: "调试日志设置保存失败，当前运行状态未改变。",
      unavailableRecheck: "调试日志设置暂时不可用，请重新检查。",
    },
  },
  en: {
    loading: "Reading debug log settings…",
    recheck: "Check again",
    toggleTitle: "Enable debug logging",
    toggleDescription:
      "Controlled debug events are written only while enabled; raw paths, error bodies, manifests, hashes, and mod contents are never recorded.",
    saving: "Saving…",
    enabled: "Enabled",
    disabled: "Disabled",
    errors: {
      unavailableRetry: "Debug log settings are temporarily unavailable. Please try again later.",
      saveFailed: "Failed to save debug log settings; the current runtime state is unchanged.",
      unavailableRecheck: "Debug log settings are temporarily unavailable. Please check again.",
    },
  },
  ja: {
    loading: "デバッグログ設定を読み込んでいます…",
    recheck: "再確認",
    toggleTitle: "デバッグログを有効化",
    toggleDescription:
      "有効時のみ管理されたデバッグイベントを記録します。元のパス、エラー本文、Manifest、ハッシュ、Mod の内容は記録しません。",
    saving: "保存中…",
    enabled: "有効",
    disabled: "無効",
    errors: {
      unavailableRetry:
        "デバッグログ設定が一時的に利用できません。しばらくしてから再試行してください。",
      saveFailed: "デバッグログ設定の保存に失敗しました。現在の実行状態は変わっていません。",
      unavailableRecheck: "デバッグログ設定が一時的に利用できません。再確認してください。",
    },
  },
} satisfies LocaleDictionary<DebugLogSettingsCopy>;
