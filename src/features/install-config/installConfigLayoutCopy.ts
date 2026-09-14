import type { LocaleDictionary } from "../../shared/i18n";

type LayoutCopy = {
  rootSettings: string; closeRoot: string; rootImmediate: string; title: string;
  search: string; all: string; excluded: string; compact: string; original: string;
  noMatches: string; filterHint: string; clearSearch: string; planDetails: string;
  files: (count: number) => string; attachments: (count: number) => string;
  tools: (count: number) => string; blocked: string;
  workspace: { selection: string; preview: string; collapsePreview: string; showPreview: string };
};

export const installConfigLayoutCopy = {
  zh_cn: {
    rootSettings: "目录设置", closeRoot: "收起目录设置", rootImmediate: "目录更改会立即保存；文件勾选通过底部按钮保存。", title: "安装配置",
    search: "搜索文件名或路径", all: "全部文件", excluded: "已排除", compact: "合并单目录", original: "完整层级",
    noMatches: "没有符合筛选条件的文件。", filterHint: "筛选只影响显示；勾选目录仍作用于该目录的全部可选文件。", clearSearch: "清除搜索", planDetails: "安装位置",
    files: (count) => `包内 ${count} 个文件`, attachments: (count) => `插件与附件 ${count}`, tools: (count) => `随包工具 ${count}（不安装）`, blocked: "安装变更暂不能应用，请查看预览中的原因。",
    workspace: { selection: "安装文件", preview: "安装变更预览", collapsePreview: "收起预览", showPreview: "查看预览" },
  },
  en: {
    rootSettings: "Directory settings", closeRoot: "Hide directory settings", rootImmediate: "Directory changes save immediately. Save file choices using the buttons below.", title: "Install configuration",
    search: "Search file names or paths", all: "All files", excluded: "Excluded", compact: "Compact folders", original: "Full hierarchy",
    noMatches: "No files match these filters.", filterHint: "Filters only change the view. Selecting a folder still affects all its selectable files.", clearSearch: "Clear search", planDetails: "Install locations",
    files: (count) => `${count} files in package`, attachments: (count) => `Plugins and attachments ${count}`, tools: (count) => `Bundled tools ${count} (not installed)`, blocked: "Installation changes are blocked. See the preview for details.",
    workspace: { selection: "Install files", preview: "Installation changes", collapsePreview: "Hide preview", showPreview: "Show preview" },
  },
  ja: {
    rootSettings: "ディレクトリ設定", closeRoot: "設定を閉じる", rootImmediate: "ディレクトリの変更は即時保存されます。ファイルの選択は下部のボタンで保存します。", title: "インストール設定",
    search: "ファイル名・パスを検索", all: "すべて", excluded: "除外済み", compact: "単一階層をまとめる", original: "全階層を表示",
    noMatches: "条件に一致するファイルがありません。", filterHint: "絞り込みは表示のみを変更します。フォルダーの選択は、その配下の選択可能な全ファイルに適用されます。", clearSearch: "検索をクリア", planDetails: "インストール先",
    files: (count) => `パッケージ内 ${count} ファイル`, attachments: (count) => `プラグイン・付属ファイル ${count}`, tools: (count) => `付属ツール ${count}（インストール対象外）`, blocked: "変更を適用できません。プレビューで理由を確認してください。",
    workspace: { selection: "インストールファイル", preview: "インストール変更の確認", collapsePreview: "プレビューを閉じる", showPreview: "プレビューを表示" },
  },
} satisfies LocaleDictionary<LayoutCopy>;
