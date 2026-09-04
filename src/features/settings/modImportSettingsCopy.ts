import type { LocaleDictionary } from "../../shared/i18n/locales";

// 「移动导入」开关（#275 切片④）的全部用户可见文案：开关行、开启前的 alertdialog 警告、
// 开启后的常驻提醒。导入完成后「压缩包未删除」的降级提示在 modImportCopy.archiveKept。

export type ModImportSettingsCopy = {
  loading: string;
  recheck: string;
  toggleTitle: string;
  toggleDescription: string;
  saving: string;
  enabled: string;
  disabled: string;
  enabledNote: string;
  confirm: {
    title: string;
    body: string;
    pointConsumed: string;
    pointCrossVolume: string;
    pointProtected: string;
    cancel: string;
    confirm: string;
    closeAria: string;
  };
  errors: {
    unavailableRetry: string;
    saveFailed: string;
  };
};

export const modImportSettingsCopy = {
  zh_cn: {
    loading: "正在读取导入设置…",
    recheck: "重新检查",
    toggleTitle: "导入后删除原始压缩包",
    toggleDescription:
      "解包并写入 Mod 库成功后，删除你选择的那个 ZIP 文件，省下一份磁盘占用。默认关闭。",
    saving: "正在保存…",
    enabled: "已启用",
    disabled: "已关闭",
    enabledNote: "已开启：每次导入成功后原始压缩包都会被删除，无法撤销。外部导入（狩技盒子目录）不受影响。",
    confirm: {
      title: "导入后删除原始压缩包？",
      body: "开启后，每次 ZIP 导入成功都会删除你选择的原始文件。",
      pointConsumed: "原始文件会被消耗，无法撤销；请确认你不再需要它，或另有备份。",
      pointCrossVolume: "先解包到 Mod 存储目录、再删除源文件——跨磁盘时等同于「复制 + 删除」，过程中会短暂占用两份空间。",
      pointProtected: "位于游戏目录、Mod 存储目录或应用数据目录内的压缩包，以及导入失败或取消时，一律保留。",
      cancel: "取消",
      confirm: "开启并删除原始文件",
      closeAria: "关闭确认",
    },
    errors: {
      unavailableRetry: "导入设置暂时不可用，请稍后重试。",
      saveFailed: "导入设置保存失败，当前选项未改变。",
    },
  },
  en: {
    loading: "Reading import settings…",
    recheck: "Check again",
    toggleTitle: "Delete the original archive after import",
    toggleDescription:
      "After a mod is unpacked and written to the library, delete the ZIP you picked to save one copy's worth of disk space. Off by default.",
    saving: "Saving…",
    enabled: "Enabled",
    disabled: "Disabled",
    enabledNote: "Enabled: the original archive is deleted after every successful import and cannot be restored. External imports (Hunting Box directory) are unaffected.",
    confirm: {
      title: "Delete the original archive after import?",
      body: "When enabled, every successful ZIP import deletes the original file you picked.",
      pointConsumed: "The original file is consumed and cannot be undone; make sure you no longer need it or have a backup.",
      pointCrossVolume: "The mod is unpacked into the storage directory first, then the source is deleted — across drives this equals copy + delete and briefly uses space in both places.",
      pointProtected: "Archives inside the game directory, the mod storage directory or the app data directory are always kept, as is the archive of a failed or cancelled import.",
      cancel: "Cancel",
      confirm: "Enable and delete originals",
      closeAria: "Close confirmation",
    },
    errors: {
      unavailableRetry: "Import settings are temporarily unavailable. Please try again later.",
      saveFailed: "Failed to save the import settings; the current option is unchanged.",
    },
  },
  ja: {
    loading: "インポート設定を読み込んでいます…",
    recheck: "再確認",
    toggleTitle: "インポート後に元のアーカイブを削除",
    toggleDescription:
      "Mod を展開してライブラリに書き込んだ後、選択した ZIP ファイルを削除してディスク容量を節約します。既定では無効です。",
    saving: "保存中…",
    enabled: "有効",
    disabled: "無効",
    enabledNote: "有効：インポートが成功するたびに元のアーカイブが削除され、元に戻せません。外部インポート（狩技ボックスのフォルダー）には影響しません。",
    confirm: {
      title: "インポート後に元のアーカイブを削除しますか？",
      body: "有効にすると、ZIP のインポートが成功するたびに選択した元のファイルが削除されます。",
      pointConsumed: "元のファイルは消費され、元に戻せません。不要であること、または別にバックアップがあることを確認してください。",
      pointCrossVolume: "先に Mod 保存フォルダーへ展開し、その後に元のファイルを削除します。ドライブをまたぐ場合は「コピー + 削除」と同じで、一時的に両方の場所で容量を使います。",
      pointProtected: "ゲームフォルダー、Mod 保存フォルダー、アプリデータフォルダー内のアーカイブと、インポートが失敗またはキャンセルされた場合のアーカイブは常に保持されます。",
      cancel: "キャンセル",
      confirm: "有効にして元のファイルを削除",
      closeAria: "確認を閉じる",
    },
    errors: {
      unavailableRetry: "インポート設定が一時的に利用できません。しばらくしてから再試行してください。",
      saveFailed: "インポート設定の保存に失敗しました。現在の設定は変わっていません。",
    },
  },
} satisfies LocaleDictionary<ModImportSettingsCopy>;
