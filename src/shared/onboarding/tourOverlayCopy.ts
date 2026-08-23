import type { LocaleDictionary } from "../i18n";

// 引导浮层的界面文案（导航按钮、目标定位状态、退出入口）。
// 引导步骤内容本身在 onboardingTourCopy（构建时按当次语言生成）。

export type TourOverlayCopy = {
  emptyStepPrimary: string;
  exitAria: string;
  locatingTarget: string;
  targetUnavailableTitle: string;
  targetUnavailableRouteHint: string;
  targetUnavailableSkipHint: string;
  relocate: string;
  navigationAria: string;
  previous: string;
  next: string;
  exit: string;
  skipStep: string;
};

export const tourOverlayCopy = {
  zh_cn: {
    emptyStepPrimary: "关闭引导",
    exitAria: "退出新手引导",
    locatingTarget: "正在定位界面目标...",
    targetUnavailableTitle: "当前页面没有可高亮的对应区域",
    targetUnavailableRouteHint: "可以重新定位；若页面入口尚未开放，请退出引导后先完成前置设置。",
    targetUnavailableSkipHint: "可以重新定位，或跳过此项继续查看后续功能。",
    relocate: "重新定位",
    navigationAria: "引导步骤",
    previous: "上一步",
    next: "下一步",
    exit: "退出引导",
    skipStep: "跳过此项",
  },
  en: {
    emptyStepPrimary: "Close tour",
    exitAria: "Exit the onboarding tour",
    locatingTarget: "Locating the UI target...",
    targetUnavailableTitle: "This page has no matching area to highlight",
    targetUnavailableRouteHint: "You can relocate; if the page entry is not open yet, exit the tour and finish the prerequisite setup first.",
    targetUnavailableSkipHint: "You can relocate, or skip this item and continue with the remaining features.",
    relocate: "Relocate",
    navigationAria: "Tour steps",
    previous: "Previous",
    next: "Next",
    exit: "Exit tour",
    skipStep: "Skip this item",
  },
  ja: {
    emptyStepPrimary: "ガイドを閉じる",
    exitAria: "チュートリアルを終了",
    locatingTarget: "画面上の対象を特定中...",
    targetUnavailableTitle: "このページにはハイライトできる対応領域がありません",
    targetUnavailableRouteHint: "再特定できます。ページの入口がまだ開放されていない場合は、ガイドを終了して先に前提設定を完了してください。",
    targetUnavailableSkipHint: "再特定するか、この項目をスキップして残りの機能を確認できます。",
    relocate: "再特定",
    navigationAria: "ガイドステップ",
    previous: "前へ",
    next: "次へ",
    exit: "ガイドを終了",
    skipStep: "この項目をスキップ",
  },
} satisfies LocaleDictionary<TourOverlayCopy>;
