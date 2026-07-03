import { invoke } from "@tauri-apps/api/core";

// 分类 CRUD typed API 的所有权在 categories feature；这里 re-export 供 mods 页面消费。
export {
  createCategory,
  deleteCategory,
  listCategories,
  updateCategory,
  type CategoryItem,
  type CreateCategoryInput,
  type UpdateCategoryInput,
} from "../categories/categoryApi";

export type CategoryRef = {
  id: string;
  name: string;
  color?: string;
  sortOrder: number;
};

export function setModCategories(modId: string, categoryIds: string[]): Promise<void> {
  return invoke("set_mod_categories", { modId, categoryIds });
}

export function getModCategories(modId: string): Promise<CategoryRef[]> {
  return invoke("get_mod_categories", { modId });
}
