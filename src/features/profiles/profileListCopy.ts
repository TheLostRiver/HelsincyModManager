import type { LocaleDictionary } from "../../shared/i18n";

// 配置档列表面板（卡包列表、浮层表单、删除确认）的全部用户可见文案。
// SLOT 编号为设计元素保持英文，不进字典。

export type ProfileListCopy = {
  title: string;
  countReady: (count: number) => string;
  loadingMeta: string;
  refreshAria: string;
  createCard: string;
  workspaceEyebrow: string;
  createTitle: string;
  editTitle: string;
  closeFormAria: string;
  loadingList: string;
  loadFailed: string;
  retry: string;
  listAria: string;
  activeBadge: string;
  standbyBadge: string;
  noDescription: string;
  activating: string;
  activate: string;
  editAria: string;
  deleteAria: string;
  cannotDeleteActive: string;
  deleteConfirm: string;
  deleteConfirmAction: string;
  cancel: string;
  nameField: string;
  descriptionField: string;
  creating: string;
  createSubmit: string;
  saving: string;
  saveSubmit: string;
  errorFallback: string;
};

export const profileListCopy = {
  zh_cn: {
    title: "配置卡包",
    countReady: (count: number) => `${count} 个本地配置`,
    loadingMeta: "读取配置中",
    refreshAria: "刷新配置档",
    createCard: "新建独立游戏配置档",
    workspaceEyebrow: "配置工作空间",
    createTitle: "新建配置档",
    editTitle: "编辑配置档",
    closeFormAria: "关闭配置档信息",
    loadingList: "正在加载名片盒...",
    loadFailed: "配置数据加载失败",
    retry: "重试",
    listAria: "配置档列表",
    activeBadge: "活动中",
    standbyBadge: "备用档",
    noDescription: "暂无备注描述",
    activating: "激活中",
    activate: "激活",
    editAria: "编辑配置档",
    deleteAria: "删除配置档",
    cannotDeleteActive: "当前配置档不能删除",
    deleteConfirm: "确认注销并删除该配置卡？",
    deleteConfirmAction: "注销",
    cancel: "取消",
    nameField: "配置卡名称",
    descriptionField: "描述与备注",
    creating: "正在创建...",
    createSubmit: "确认创建",
    saving: "正在同步...",
    saveSubmit: "保存更改",
    errorFallback: "操作失败",
  },
  en: {
    title: "Profile deck",
    countReady: (count: number) => `${count} local profile${count === 1 ? "" : "s"}`,
    loadingMeta: "Loading profiles",
    refreshAria: "Refresh profiles",
    createCard: "Create standalone game profile",
    workspaceEyebrow: "Profile workspace",
    createTitle: "New profile",
    editTitle: "Edit profile",
    closeFormAria: "Close profile form",
    loadingList: "Loading the profile deck...",
    loadFailed: "Failed to load profile data",
    retry: "Retry",
    listAria: "Profile list",
    activeBadge: "Active",
    standbyBadge: "Standby",
    noDescription: "No description yet",
    activating: "Activating",
    activate: "Activate",
    editAria: "Edit profile",
    deleteAria: "Delete profile",
    cannotDeleteActive: "The active profile cannot be deleted",
    deleteConfirm: "Deactivate and delete this profile card?",
    deleteConfirmAction: "Delete",
    cancel: "Cancel",
    nameField: "Profile card name",
    descriptionField: "Description and notes",
    creating: "Creating...",
    createSubmit: "Create",
    saving: "Saving...",
    saveSubmit: "Save changes",
    errorFallback: "Operation failed",
  },
  ja: {
    title: "プロファイルデッキ",
    countReady: (count: number) => `ローカルプロファイル ${count} 件`,
    loadingMeta: "プロファイルを読み込み中",
    refreshAria: "プロファイルを更新",
    createCard: "独立したゲームプロファイルを作成",
    workspaceEyebrow: "プロファイルワークスペース",
    createTitle: "プロファイルを新規作成",
    editTitle: "プロファイルを編集",
    closeFormAria: "プロファイルフォームを閉じる",
    loadingList: "プロファイルデッキを読み込み中...",
    loadFailed: "プロファイルデータの読み込みに失敗",
    retry: "再試行",
    listAria: "プロファイル一覧",
    activeBadge: "使用中",
    standbyBadge: "待機",
    noDescription: "説明はまだありません",
    activating: "有効化中",
    activate: "有効化",
    editAria: "プロファイルを編集",
    deleteAria: "プロファイルを削除",
    cannotDeleteActive: "使用中のプロファイルは削除できません",
    deleteConfirm: "このプロファイルカードを登録解除して削除しますか？",
    deleteConfirmAction: "削除",
    cancel: "キャンセル",
    nameField: "プロファイルカード名",
    descriptionField: "説明・メモ",
    creating: "作成中...",
    createSubmit: "作成を確定",
    saving: "同期中...",
    saveSubmit: "変更を保存",
    errorFallback: "操作に失敗しました",
  },
} satisfies LocaleDictionary<ProfileListCopy>;
