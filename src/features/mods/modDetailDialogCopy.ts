import type { LocaleDictionary } from "../../shared/i18n";

// Mod 详情对话框（右键菜单「Mod 信息 / Mod 修改」入口）的全部用户可见文案。
// 保存/加载的结果语义在 modDetailDialogWorkflow 里，文本一律在渲染时取。

export type ModDetailDialogCopy = {
  eyebrow: string;
  closeAria: string;
  tablistAria: string;
  tabDetails: string;
  tabReplacement: string;
  previewAria: string;
  noPreview: string;
  packageIdLabel: string;
  originLabel: string;
  originImported: string;
  originExternalImport: (sourceLabel: string, importedAt: string) => string;
  originUnknownSource: string;
  originMigrated: string;
  selectedCategoriesLabel: string;
  noCategoriesSelected: string;
  sectionInfo: string;
  fieldName: string;
  fieldAuthor: string;
  fieldVersion: string;
  fieldNexusId: string;
  fieldNotes: string;
  sectionCategories: string;
  noCategoriesAvailable: string;
  cancel: string;
  save: string;
  saving: string;
  closeButton: string;
  messages: {
    detailLoadFailed: string;
    categoryLoadFailed: string;
    nexusIdInvalid: string;
    metadataFailure: string;
    partialCategoryFailure: string;
    refreshFailure: string;
  };
};

export const modDetailDialogCopy = {
  zh_cn: {
    eyebrow: "Mod 详情",
    closeAria: "关闭",
    tablistAria: "Mod 详情视图",
    tabDetails: "基本信息",
    tabReplacement: "替换目标",
    previewAria: "Mod 预览图",
    noPreview: "暂无预览图",
    packageIdLabel: "Package ID",
    originLabel: "来源",
    originImported: "手动导入",
    originExternalImport: (sourceLabel: string, importedAt: string) =>
      `第三方导入 · ${sourceLabel} · ${importedAt}`,
    originUnknownSource: "第三方来源",
    originMigrated: "旧版本迁移",
    selectedCategoriesLabel: "已选分类",
    noCategoriesSelected: "未关联",
    sectionInfo: "信息编辑",
    fieldName: "名称",
    fieldAuthor: "作者",
    fieldVersion: "版本",
    fieldNexusId: "NexusMods ID",
    fieldNotes: "备注",
    sectionCategories: "分类关联",
    noCategoriesAvailable: "还没有可关联的分类。",
    cancel: "取消",
    save: "保存",
    saving: "保存中",
    closeButton: "关闭",
    messages: {
      detailLoadFailed: "详情读取失败，已使用列表中的基础信息。",
      categoryLoadFailed: "分类读取失败，本次保存不会改动分类关联。",
      nexusIdInvalid: "NexusMods ID 只能填写正整数。",
      metadataFailure: "信息保存失败，请稍后重试。",
      partialCategoryFailure: "信息已保存，但分类关联保存失败，请稍后重试。",
      refreshFailure: "保存成功，但列表刷新失败，请稍后手动刷新。",
    },
  },
  en: {
    eyebrow: "Mod Details",
    closeAria: "Close",
    tablistAria: "Mod detail views",
    tabDetails: "Details",
    tabReplacement: "Replacement Target",
    previewAria: "Mod preview image",
    noPreview: "No preview image",
    packageIdLabel: "Package ID",
    originLabel: "Origin",
    originImported: "Manual import",
    originExternalImport: (sourceLabel: string, importedAt: string) =>
      `Third-party import · ${sourceLabel} · ${importedAt}`,
    originUnknownSource: "Third-party source",
    originMigrated: "Legacy migration",
    selectedCategoriesLabel: "Categories",
    noCategoriesSelected: "None",
    sectionInfo: "Edit Info",
    fieldName: "Name",
    fieldAuthor: "Author",
    fieldVersion: "Version",
    fieldNexusId: "NexusMods ID",
    fieldNotes: "Notes",
    sectionCategories: "Category Assignment",
    noCategoriesAvailable: "No categories available yet.",
    cancel: "Cancel",
    save: "Save",
    saving: "Saving",
    closeButton: "Close",
    messages: {
      detailLoadFailed: "Failed to load details; showing basic info from the list instead.",
      categoryLoadFailed: "Failed to load categories; this save will not change category assignments.",
      nexusIdInvalid: "NexusMods ID must be a positive integer.",
      metadataFailure: "Failed to save info. Please try again later.",
      partialCategoryFailure: "Info saved, but category assignments failed to save. Please try again later.",
      refreshFailure: "Saved successfully, but the list failed to refresh. Please refresh manually.",
    },
  },
  ja: {
    eyebrow: "Mod 詳細",
    closeAria: "閉じる",
    tablistAria: "Mod 詳細ビュー",
    tabDetails: "基本情報",
    tabReplacement: "置換ターゲット",
    previewAria: "Mod プレビュー画像",
    noPreview: "プレビュー画像なし",
    packageIdLabel: "Package ID",
    originLabel: "由来",
    originImported: "手動インポート",
    originExternalImport: (sourceLabel: string, importedAt: string) =>
      `サードパーティインポート · ${sourceLabel} · ${importedAt}`,
    originUnknownSource: "サードパーティソース",
    originMigrated: "旧バージョンからの移行",
    selectedCategoriesLabel: "選択中のカテゴリ",
    noCategoriesSelected: "未設定",
    sectionInfo: "情報を編集",
    fieldName: "名前",
    fieldAuthor: "作者",
    fieldVersion: "バージョン",
    fieldNexusId: "NexusMods ID",
    fieldNotes: "メモ",
    sectionCategories: "カテゴリ割り当て",
    noCategoriesAvailable: "割り当て可能なカテゴリがまだありません。",
    cancel: "キャンセル",
    save: "保存",
    saving: "保存中",
    closeButton: "閉じる",
    messages: {
      detailLoadFailed: "詳細の読み込みに失敗したため、リストの基本情報を表示しています。",
      categoryLoadFailed: "カテゴリの読み込みに失敗しました。今回の保存ではカテゴリ割り当ては変更されません。",
      nexusIdInvalid: "NexusMods ID には正の整数のみ入力できます。",
      metadataFailure: "情報の保存に失敗しました。しばらくしてから再試行してください。",
      partialCategoryFailure: "情報は保存されましたが、カテゴリ割り当ての保存に失敗しました。しばらくしてから再試行してください。",
      refreshFailure: "保存は成功しましたが、リストの更新に失敗しました。手動で更新してください。",
    },
  },
} satisfies LocaleDictionary<ModDetailDialogCopy>;
