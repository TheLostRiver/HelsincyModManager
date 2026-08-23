import type { CategoryItem } from "./categoryApi";

// 浏览器预览环境的种子分类（mock 内容不翻译，不进入 i18n sweep 的无中文清单）。

export const CATEGORY_DEV_SEED: CategoryItem[] = [
  { id: "cat-appearance", name: "外观", color: "#DB2777", sortOrder: 0, modCount: 18 },
  { id: "cat-armor", name: "防具替换", color: "#2563EB", sortOrder: 1, modCount: 11 },
  { id: "cat-weapon", name: "武器替换", color: "#7C3AED", sortOrder: 2, modCount: 9 },
  { id: "cat-voice", name: "语音替换", color: "#0891B2", sortOrder: 3, modCount: 4 },
  { id: "cat-utility", name: "工具 / 前置", color: "#D97706", sortOrder: 4, modCount: 6 },
  { id: "cat-effects", name: "特效", color: "#16A34A", sortOrder: 5, modCount: 0 },
];
