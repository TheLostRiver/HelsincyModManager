import type { LocaleDictionary } from "../../shared/i18n";

type RetargetDialogCopy = {
  title: string;
  selection: string;
  preview: string;
  showPreview: string;
  collapsePreview: string;
  closeDetails: string;
  currentAndDefault: string;
  current: string;
  original: string;
  notInstalled: string;
  selected: string;
  unavailableTarget: string;
  attachments: (selected: number, excluded: number) => string;
  sources: (count: number) => string;
};

export const retargetDialogCopy = {
  zh_cn: {
    title: "MOD 文件重定向",
    selection: "选择目标",
    preview: "文件变更预览",
    showPreview: "查看预览",
    collapsePreview: "收起预览",
    closeDetails: "关闭详情",
    currentAndDefault: "当前 · 作者默认",
    current: "当前",
    original: "作者默认",
    notInstalled: "尚未安装",
    selected: "已选目标",
    unavailableTarget: "所选目标暂不可确认，请重新选择",
    attachments: (selected, excluded) => `附件 · 已选 ${selected} / 排除 ${excluded}`,
    sources: (count) => `包内 ${count} 件装备`,
  },
  en: {
    title: "MOD file retargeting",
    selection: "Choose targets",
    preview: "File changes",
    showPreview: "Show preview",
    collapsePreview: "Hide preview",
    closeDetails: "Close details",
    currentAndDefault: "Current · Original",
    current: "Current",
    original: "Original",
    notInstalled: "Not installed",
    selected: "Selected target",
    unavailableTarget: "Selected target unavailable. Choose again.",
    attachments: (selected, excluded) => `Files · ${selected} selected / ${excluded} excluded`,
    sources: (count) => `${count} items in this package`,
  },
  ja: {
    title: "MOD ファイルのリターゲット",
    selection: "対象を選択",
    preview: "ファイル変更のプレビュー",
    showPreview: "プレビューを表示",
    collapsePreview: "プレビューを閉じる",
    closeDetails: "詳細を閉じる",
    currentAndDefault: "現在 · 作者の設定",
    current: "現在",
    original: "作者の設定",
    notInstalled: "未インストール",
    selected: "選択した対象",
    unavailableTarget: "選択した対象を確認できません。再選択してください。",
    attachments: (selected, excluded) => `付属ファイル · 選択 ${selected} / 除外 ${excluded}`,
    sources: (count) => `パッケージ内の装備 ${count} 件`,
  },
} satisfies LocaleDictionary<RetargetDialogCopy>;
