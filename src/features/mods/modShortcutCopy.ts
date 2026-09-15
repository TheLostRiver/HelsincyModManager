const zh = {
  folderTitle: "无法打开 Mod 文件夹",
  nexusTitle: "无法打开 NexusMods",
  unavailable: "无法读取此 Mod 的信息，请刷新 Mod 列表后重试。",
  folderUnavailable: "Mod 文件夹不存在或无法访问。请检查 Mod 存储目录。",
  nexusIdMissing: "请先在“MOD 信息设置”中填写有效的 NexusMods ID。",
  nexusOpenFailed: "无法启动系统浏览器，请稍后重试。",
};
export const modShortcutCopy = {
  zh_cn: zh,
  en: {
    folderTitle: "Could not open mod folder",
    nexusTitle: "Could not open NexusMods",
    unavailable: "Mod information is unavailable. Refresh the library and try again.",
    folderUnavailable: "The mod folder is missing or inaccessible. Check the mod storage directory.",
    nexusIdMissing: "Enter a valid NexusMods ID in Mod info settings first.",
    nexusOpenFailed: "The system browser could not be opened. Please try again.",
  },
  ja: {
    folderTitle: "Mod フォルダーを開けません",
    nexusTitle: "NexusMods を開けません",
    unavailable: "Mod 情報を取得できません。一覧を更新して再試行してください。",
    folderUnavailable: "Mod フォルダーが存在しないか、アクセスできません。Mod 保存先を確認してください。",
    nexusIdMissing: "先に「Mod 情報設定」で有効な NexusMods ID を入力してください。",
    nexusOpenFailed: "システムブラウザーを起動できません。再試行してください。",
  },
};

export function modShortcutErrorMessage(error: unknown, copy: typeof zh): string {
  const code = typeof error === "object" && error !== null && "code" in error ? error.code : null;
  switch (code) {
    case "mod_nexus_id_missing": return copy.nexusIdMissing;
    case "mod_nexus_open_failed": return copy.nexusOpenFailed;
    case "mod_folder_unavailable": return copy.folderUnavailable;
    default: return copy.unavailable;
  }
}
