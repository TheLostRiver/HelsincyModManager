import type { LocaleDictionary } from "../../shared/i18n";

type RetargetDialogCopy = {
  title: string;
  selection: string;
  preview: string;
  emptyTitle: string;
  emptyDescription: string;
};

export const retargetDialogCopy = {
  zh_cn: {
    title: "MOD 文件重定向",
    selection: "选择目标",
    preview: "文件变更预览",
    emptyTitle: "先选择目标，再预览文件变更",
    emptyDescription: "预览会在这里列出安装、保留与移除的文件，确认后才会应用。",
  },
  en: {
    title: "MOD file retargeting",
    selection: "Choose targets",
    preview: "File changes",
    emptyTitle: "Choose a target to preview file changes",
    emptyDescription: "Review files to install, keep and remove here before confirming the changes.",
  },
  ja: {
    title: "MOD ファイルのリターゲット",
    selection: "対象を選択",
    preview: "ファイル変更のプレビュー",
    emptyTitle: "対象を選んでファイル変更を確認",
    emptyDescription: "インストール・保持・削除するファイルをここで確認できます。変更は確定後に適用されます。",
  },
} satisfies LocaleDictionary<RetargetDialogCopy>;
