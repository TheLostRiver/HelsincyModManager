import type { LocaleDictionary } from "../../shared/i18n";

// Preview image viewer (#283). Card thumbnails are intentionally cropped with
// `object-fit: cover` + `object-position: center top`, so this dialog is the only
// place the full image is shown. All user-visible strings live here.

export type PreviewImageViewCopy = {
  menu: {
    viewPreview: string;
  };
  dialog: {
    title: string;
    closeAria: string;
    loading: string;
    unavailable: string;
    fallbackNotice: string;
    loadFailed: string;
    zoomIn: string;
    zoomOut: string;
    reset: string;
    zoomLevel: (percent: number) => string;
    dragHint: string;
  };
};

export const previewImageViewCopy = {
  zh_cn: {
    menu: {
      viewPreview: "查看预览图",
    },
    dialog: {
      title: "预览图",
      closeAria: "关闭预览图",
      loading: "正在加载预览图",
      unavailable: "该 MOD 没有可用的预览图",
      fallbackNotice: "高清预览图不可用，当前显示卡片缩略图",
      loadFailed: "预览图加载失败",
      zoomIn: "放大",
      zoomOut: "缩小",
      reset: "重置",
      zoomLevel: (percent: number) => `缩放 ${percent}%`,
      dragHint: "拖动可平移，双击重置",
    },
  },
  en: {
    menu: {
      viewPreview: "View preview image",
    },
    dialog: {
      title: "Preview image",
      closeAria: "Close preview image",
      loading: "Loading preview image",
      unavailable: "No preview image available for this mod",
      fallbackNotice: "High resolution preview unavailable, showing the card thumbnail",
      loadFailed: "Failed to load preview image",
      zoomIn: "Zoom in",
      zoomOut: "Zoom out",
      reset: "Reset",
      zoomLevel: (percent: number) => `Zoom ${percent}%`,
      dragHint: "Drag to pan, double click to reset",
    },
  },
  ja: {
    menu: {
      viewPreview: "プレビュー画像を表示",
    },
    dialog: {
      title: "プレビュー画像",
      closeAria: "プレビュー画像を閉じる",
      loading: "プレビュー画像を読み込み中",
      unavailable: "この MOD には利用可能なプレビュー画像がありません",
      fallbackNotice:
        "高解像度プレビューを利用できないため、カードのサムネイルを表示しています",
      loadFailed: "プレビュー画像の読み込みに失敗しました",
      zoomIn: "拡大",
      zoomOut: "縮小",
      reset: "リセット",
      zoomLevel: (percent: number) => `ズーム ${percent}%`,
      dragHint: "ドラッグで移動、ダブルクリックでリセット",
    },
  },
} satisfies LocaleDictionary<PreviewImageViewCopy>;
