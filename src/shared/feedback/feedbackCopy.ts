import type { LocaleDictionary } from "../i18n";

// 共享反馈基元（toast/modal/任务通知视口）的默认文案。
// 调用方传入的已本地化标签优先；这里只提供未传参时的兜底。

export type FeedbackCopy = {
  toastDismissAria: string;
  toastMerged: (occurrences: number) => string;
  modalCloseLabel: string;
  taskNoticeViewportLabel: string;
  toastViewportLabel: string;
};

export const feedbackCopy = {
  zh_cn: {
    toastDismissAria: "关闭通知",
    toastMerged: (occurrences: number) => `已合并 ${occurrences} 次相同通知`,
    modalCloseLabel: "关闭",
    taskNoticeViewportLabel: "任务进度",
    toastViewportLabel: "通知",
  },
  en: {
    toastDismissAria: "Dismiss notification",
    toastMerged: (occurrences: number) => `${occurrences} identical notifications merged`,
    modalCloseLabel: "Close",
    taskNoticeViewportLabel: "Task progress",
    toastViewportLabel: "Notifications",
  },
  ja: {
    toastDismissAria: "通知を閉じる",
    toastMerged: (occurrences: number) => `同一の通知を ${occurrences} 件統合しました`,
    modalCloseLabel: "閉じる",
    taskNoticeViewportLabel: "タスク進捗",
    toastViewportLabel: "通知",
  },
} satisfies LocaleDictionary<FeedbackCopy>;
