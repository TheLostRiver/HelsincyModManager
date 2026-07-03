import type { CategoryItem } from "./categoryApi";

export type CategorySortMode = "custom" | "name" | "modCount";

export type CategoryMetrics = {
  total: number;
  linkedModCount: number;
  emptyCategoryCount: number;
  coloredCategoryCount: number;
};

export type CategorySortOrderUpdate = {
  categoryId: string;
  sortOrder: number;
};

export type CategoryBatchSummary = {
  count: number;
  linkedModCount: number;
};

const CATEGORY_NAME_LOCALE = "zh-Hans-CN";

export function getCategoryMetrics(categories: readonly CategoryItem[]): CategoryMetrics {
  return categories.reduce<CategoryMetrics>(
    (metrics, category) => ({
      total: metrics.total + 1,
      linkedModCount: metrics.linkedModCount + category.modCount,
      emptyCategoryCount: metrics.emptyCategoryCount + (category.modCount === 0 ? 1 : 0),
      coloredCategoryCount: metrics.coloredCategoryCount + (category.color ? 1 : 0),
    }),
    { total: 0, linkedModCount: 0, emptyCategoryCount: 0, coloredCategoryCount: 0 },
  );
}

export function normalizeCategoryQuery(query: string): string {
  return query.trim().toLowerCase();
}

export function filterCategories(
  categories: readonly CategoryItem[],
  query: string,
): CategoryItem[] {
  const normalized = normalizeCategoryQuery(query);
  if (!normalized) {
    return [...categories];
  }

  return categories.filter((category) => category.name.toLowerCase().includes(normalized));
}

function compareByCustomOrder(a: CategoryItem, b: CategoryItem): number {
  return a.sortOrder - b.sortOrder || a.name.localeCompare(b.name, CATEGORY_NAME_LOCALE);
}

export function sortCategoriesForView(
  categories: readonly CategoryItem[],
  mode: CategorySortMode,
): CategoryItem[] {
  const sorted = [...categories];

  switch (mode) {
    case "name":
      sorted.sort((a, b) => a.name.localeCompare(b.name, CATEGORY_NAME_LOCALE));
      break;
    case "modCount":
      sorted.sort((a, b) => b.modCount - a.modCount || compareByCustomOrder(a, b));
      break;
    case "custom":
      sorted.sort(compareByCustomOrder);
      break;
  }

  return sorted;
}

export function canReorderCategories(
  mode: CategorySortMode,
  query: string,
  batchMode: boolean,
): boolean {
  return mode === "custom" && normalizeCategoryQuery(query) === "" && !batchMode;
}

/**
 * 把 fromIndex 上的分类移动到 insertIndex 插入位（0..length）。
 * 返回新数组；无效或无变化时返回 null。
 */
export function reorderCategoryList(
  categories: readonly CategoryItem[],
  fromIndex: number,
  insertIndex: number,
): CategoryItem[] | null {
  if (fromIndex < 0 || fromIndex >= categories.length) {
    return null;
  }
  if (insertIndex < 0 || insertIndex > categories.length) {
    return null;
  }
  // 移除自身后的落点与原位相同则视为无变化。
  if (insertIndex === fromIndex || insertIndex === fromIndex + 1) {
    return null;
  }

  const next = [...categories];
  const [moved] = next.splice(fromIndex, 1);
  const target = insertIndex > fromIndex ? insertIndex - 1 : insertIndex;
  next.splice(target, 0, moved);
  return next;
}

export function moveCategoryByOffset(
  categories: readonly CategoryItem[],
  categoryId: string,
  offset: -1 | 1,
): CategoryItem[] | null {
  const fromIndex = categories.findIndex((category) => category.id === categoryId);
  if (fromIndex < 0) {
    return null;
  }

  const insertIndex = offset === -1 ? fromIndex - 1 : fromIndex + 2;
  return reorderCategoryList(categories, fromIndex, insertIndex);
}

/**
 * 按展示顺序重编号 sortOrder（0..n-1），只返回发生变化的更新项。
 */
export function buildSortOrderUpdates(
  categories: readonly CategoryItem[],
): CategorySortOrderUpdate[] {
  return categories
    .map((category, index) => ({ categoryId: category.id, sortOrder: index, category }))
    .filter((entry) => entry.category.sortOrder !== entry.sortOrder)
    .map(({ categoryId, sortOrder }) => ({ categoryId, sortOrder }));
}

export function nextAppendSortOrder(categories: readonly CategoryItem[]): number {
  if (categories.length === 0) {
    return 0;
  }
  return Math.max(...categories.map((category) => category.sortOrder)) + 1;
}

export function findDuplicateCategoryName(
  categories: readonly CategoryItem[],
  name: string,
  excludeId?: string,
): CategoryItem | undefined {
  const normalized = name.trim().toLowerCase();
  if (!normalized) {
    return undefined;
  }

  return categories.find(
    (category) => category.id !== excludeId && category.name.trim().toLowerCase() === normalized,
  );
}

export function toggleCategorySelection(
  selectedIds: ReadonlySet<string>,
  categoryId: string,
): Set<string> {
  const next = new Set(selectedIds);
  if (next.has(categoryId)) {
    next.delete(categoryId);
  } else {
    next.add(categoryId);
  }
  return next;
}

/** 选择集合可能包含已被删除的分类 id，统计时只保留仍存在的。 */
export function pruneCategorySelection(
  selectedIds: ReadonlySet<string>,
  categories: readonly CategoryItem[],
): Set<string> {
  const known = new Set(categories.map((category) => category.id));
  return new Set([...selectedIds].filter((id) => known.has(id)));
}

export function summarizeBatchTargets(
  categories: readonly CategoryItem[],
  selectedIds: ReadonlySet<string>,
): CategoryBatchSummary {
  return categories.reduce<CategoryBatchSummary>(
    (summary, category) =>
      selectedIds.has(category.id)
        ? {
            count: summary.count + 1,
            linkedModCount: summary.linkedModCount + category.modCount,
          }
        : summary,
    { count: 0, linkedModCount: 0 },
  );
}

export function formatCategoryMutationError(error: unknown, fallback: string): string {
  if (typeof error === "string" && error.trim()) {
    return error;
  }

  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  if (typeof error === "object" && error !== null && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) {
      return message;
    }
  }

  return fallback;
}

export function getCategoryMutationErrorMessage(error: unknown, fallback: string): string {
  // 后端 command error 不直接透出 raw message，统一使用前端可读文案。
  if (isCategoryCommandError(error)) {
    return fallback;
  }

  return formatCategoryMutationError(error, fallback);
}

export function isCategoryCommandError(error: unknown): boolean {
  return (
    typeof error === "object"
    && error !== null
    && "code" in error
    && (error as { code?: unknown }).code === "category_error"
  );
}
