// Mod 库展示层数据。
// 现阶段使用本地 mock 数据还原设计稿，后续由 Mod 仓储或视图模型提供真实数据。
// 业务规则（安装、冲突、依赖判定）不在此处推断，仅承载展示字段。

export type ModInstallStatus = "installed" | "disabled" | "conflict";

export type ModLibraryItem = {
  id: string;
  name: string;
  sizeLabel: string;
  status: ModInstallStatus;
  categoryLabels: string[];
  // 海报背景的渐变色，用于无预览图时的占位。
  // 设计稿中每张卡片有独立色调，这里保留为语义化的色卡描述。
  posterFrom: string;
  posterTo: string;
};

export const modLibraryItems: ModLibraryItem[] = [
  {
    id: "mod-ceremony-gown",
    name: "非官方仪式礼服",
    sizeLabel: "2.1 MB",
    status: "installed",
    categoryLabels: ["外观"],
    posterFrom: "#d7e7ff",
    posterTo: "#77a8ff",
  },
  {
    id: "mod-summer-bunny",
    name: "盛夏兔女郎",
    sizeLabel: "默认封面",
    status: "installed",
    categoryLabels: ["外观"],
    posterFrom: "#eeeff3",
    posterTo: "#b5c2d6",
  },
  {
    id: "mod-pencil-skirt",
    name: "包臀裙",
    sizeLabel: "3.8 MB",
    status: "installed",
    categoryLabels: ["外观"],
    posterFrom: "#e0f0dc",
    posterTo: "#7cc47c",
  },
  {
    id: "mod-noble-dress",
    name: "贵妇装",
    sizeLabel: "1.9 MB",
    status: "installed",
    categoryLabels: ["外观"],
    posterFrom: "#fbe9cb",
    posterTo: "#e8b15a",
  },
  {
    id: "mod-gauze-gown",
    name: "薄纱礼服",
    sizeLabel: "2.4 MB",
    status: "installed",
    categoryLabels: ["外观"],
    posterFrom: "#f5e6f2",
    posterTo: "#d98bc7",
  },
  {
    id: "mod-night-banquet",
    name: "夜宴礼裙",
    sizeLabel: "2.6 MB",
    status: "installed",
    categoryLabels: ["外观"],
    posterFrom: "#e5e7ff",
    posterTo: "#9ca3ff",
  },
  {
    id: "mod-snow-fox",
    name: "雪狐披肩",
    sizeLabel: "1.7 MB",
    status: "installed",
    categoryLabels: ["外观"],
    posterFrom: "#eef2ff",
    posterTo: "#a5b4fc",
  },
  {
    id: "mod-concierge-suit",
    name: "礼宾套装",
    sizeLabel: "2.9 MB",
    status: "installed",
    categoryLabels: ["外观"],
    posterFrom: "#e2f7ec",
    posterTo: "#86efac",
  },
  {
    id: "mod-moon-white",
    name: "月白长裙",
    sizeLabel: "2.2 MB",
    status: "installed",
    categoryLabels: ["外观"],
    posterFrom: "#fff1de",
    posterTo: "#fdba74",
  },
  {
    id: "mod-festival-dress",
    name: "祭典洋装",
    sizeLabel: "2.5 MB",
    status: "installed",
    categoryLabels: ["外观"],
    posterFrom: "#fce7f3",
    posterTo: "#f9a8d4",
  },
];

// 快捷操作面板的动作项。
// 仅承载展示语义，点击行为由页面层透传，不在此处实现业务。
export type CompactActionVariant = "primary" | "neutral" | "success" | "warning" | "danger" | "info";

export type CompactAction = {
  id: string;
  label: string;
  variant: CompactActionVariant;
};

export const compactActions: CompactAction[] = [
  { id: "add", label: "添加 MOD", variant: "primary" },
  { id: "select-all", label: "全选", variant: "neutral" },
  { id: "invert", label: "反选", variant: "neutral" },
  { id: "refresh", label: "刷新", variant: "neutral" },
  { id: "enable-all", label: "启用全部 MOD", variant: "success" },
  { id: "disable-all", label: "禁用全部 MOD", variant: "warning" },
  { id: "reinstall", label: "重装选中 MOD", variant: "info" },
  { id: "uninstall", label: "卸载选中 MOD", variant: "danger" },
];

export const libraryFilterChips = ["全部", "已安装", "已禁用", "存在冲突", "外观", "武器", "语音"] as const;
