import { invoke } from "@tauri-apps/api/core";

export type CategoryItem = {
  id: string;
  name: string;
  color?: string | null;
  sortOrder: number;
  modCount: number;
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

const CATEGORY_DEV_STORAGE_KEY = "hmm.category.devStore.v1";

// 浏览器预览环境的种子分类（mock 内容不翻译），见 categoriesPreviewData.ts。
import { CATEGORY_DEV_SEED } from "./categoriesPreviewData";

let categoryDevStore: CategoryItem[] | null = null;
let nextCategoryDevId = 1;

function hasTauriRuntime(): boolean {
  return (
    typeof window !== "undefined"
    && "__TAURI_INTERNALS__" in window
    && typeof (window as { __TAURI_INTERNALS__?: { invoke?: unknown } }).__TAURI_INTERNALS__
      ?.invoke === "function"
  );
}

function cloneCategories(categories: readonly CategoryItem[]): CategoryItem[] {
  return categories.map((category) => ({ ...category }));
}

function readStoredDevCategories(): CategoryItem[] | null {
  if (typeof window === "undefined" || !window.localStorage) {
    return null;
  }

  try {
    const raw = window.localStorage.getItem(CATEGORY_DEV_STORAGE_KEY);
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) {
      return null;
    }

    const categories = parsed.filter(isCategoryItem);
    if (categories.length !== parsed.length) {
      return null;
    }
    return categories;
  } catch {
    return null;
  }
}

function writeStoredDevCategories(categories: readonly CategoryItem[]): void {
  if (typeof window === "undefined" || !window.localStorage) {
    return;
  }

  try {
    window.localStorage.setItem(CATEGORY_DEV_STORAGE_KEY, JSON.stringify(categories));
  } catch {
    // Browser preview storage is best-effort only; keep the in-memory store usable.
  }
}

function isCategoryItem(value: unknown): value is CategoryItem {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const category = value as Partial<CategoryItem>;
  return (
    typeof category.id === "string"
    && typeof category.name === "string"
    && typeof category.sortOrder === "number"
    && Number.isFinite(category.sortOrder)
    && typeof category.modCount === "number"
    && Number.isFinite(category.modCount)
    && (
      category.color === undefined
      || category.color === null
      || typeof category.color === "string"
    )
  );
}

function getCategoryDevStore(): CategoryItem[] {
  if (categoryDevStore === null) {
    categoryDevStore = readStoredDevCategories() ?? cloneCategories(CATEGORY_DEV_SEED);
    nextCategoryDevId = categoryDevStore.length + 1;
  }

  return categoryDevStore;
}

function saveCategoryDevStore(categories: CategoryItem[]): void {
  categoryDevStore = normalizeCategoryOrder(categories);
  writeStoredDevCategories(categoryDevStore);
}

function normalizeCategoryOrder(categories: readonly CategoryItem[]): CategoryItem[] {
  return cloneCategories(categories).sort(
    (a, b) => a.sortOrder - b.sortOrder || a.name.localeCompare(b.name, "zh-Hans-CN"),
  );
}

function createCategoryDevId(name: string): string {
  const slug = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9\u4e00-\u9fa5]+/gi, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 24);
  const suffix = String(nextCategoryDevId++).padStart(2, "0");
  return `local-${slug || "category"}-${suffix}`;
}

export function createCategory(input: CreateCategoryInput): Promise<string> {
  if (!hasTauriRuntime()) {
    const store = getCategoryDevStore();
    const id = createCategoryDevId(input.name);
    saveCategoryDevStore([
      ...store,
      {
        id,
        name: input.name.trim(),
        color: input.color?.trim() || undefined,
        sortOrder: input.sortOrder ?? store.length,
        modCount: 0,
      },
    ]);
    return Promise.resolve(id);
  }

  return invoke("create_category", {
    name: input.name,
    color: input.color,
    sortOrder: input.sortOrder,
  });
}

export function updateCategory(input: UpdateCategoryInput): Promise<void> {
  if (!hasTauriRuntime()) {
    const store = getCategoryDevStore();
    const next = store.map((category) =>
      category.id === input.categoryId
        ? {
            ...category,
            name: input.name?.trim() || category.name,
            color: input.color === null ? undefined : input.color?.trim() || category.color,
            sortOrder: input.sortOrder ?? category.sortOrder,
          }
        : category,
    );
    saveCategoryDevStore(next);
    return Promise.resolve();
  }

  return invoke("update_category", {
    categoryId: input.categoryId,
    name: input.name,
    color: input.color,
    sortOrder: input.sortOrder,
  });
}

export function deleteCategory(categoryId: string): Promise<void> {
  if (!hasTauriRuntime()) {
    saveCategoryDevStore(getCategoryDevStore().filter((category) => category.id !== categoryId));
    return Promise.resolve();
  }

  return invoke("delete_category", { categoryId });
}

export function listCategories(): Promise<CategoryItem[]> {
  if (!hasTauriRuntime()) {
    return Promise.resolve(cloneCategories(getCategoryDevStore()));
  }

  return invoke("list_categories");
}
