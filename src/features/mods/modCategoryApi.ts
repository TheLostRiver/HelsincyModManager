import { invoke } from "@tauri-apps/api/core";

export type CategoryItem = {
  id: string;
  name: string;
  color?: string;
  sortOrder: number;
  modCount: number;
};

export type CategoryRef = {
  id: string;
  name: string;
  color?: string;
  sortOrder: number;
};

export type CreateCategoryInput = {
  name: string;
  color?: string;
  sortOrder?: number;
};

export type UpdateCategoryInput = {
  categoryId: string;
  name?: string;
  color?: string | null;
  sortOrder?: number;
};

export function createCategory(input: CreateCategoryInput): Promise<string> {
  return invoke("create_category", {
    name: input.name,
    color: input.color,
    sortOrder: input.sortOrder,
  });
}

export function updateCategory(input: UpdateCategoryInput): Promise<void> {
  return invoke("update_category", {
    categoryId: input.categoryId,
    name: input.name,
    color: input.color,
    sortOrder: input.sortOrder,
  });
}

export function deleteCategory(categoryId: string): Promise<void> {
  return invoke("delete_category", { categoryId });
}

export function listCategories(): Promise<CategoryItem[]> {
  return invoke("list_categories");
}

export function setModCategories(modId: string, categoryIds: string[]): Promise<void> {
  return invoke("set_mod_categories", { modId, categoryIds });
}

export function getModCategories(modId: string): Promise<CategoryRef[]> {
  return invoke("get_mod_categories", { modId });
}
