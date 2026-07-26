export const MOD_LIBRARY_PAGE_SIZES = [12, 24, 48, 96] as const;

export type ModLibraryPageSize = (typeof MOD_LIBRARY_PAGE_SIZES)[number];

export const DEFAULT_MOD_LIBRARY_PAGE_SIZE: ModLibraryPageSize = 24;

export type ModLibraryItemRange = {
  start: number;
  end: number;
};

export type ModLibraryPageSlot = number | "ellipsis";

function normalizeNonNegativeInteger(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }

  return Math.max(0, Math.floor(value));
}

function normalizePage(value: number, totalPages: number): number {
  if (!Number.isFinite(value)) {
    return 1;
  }

  return Math.min(Math.max(1, Math.floor(value)), totalPages);
}

export function isModLibraryPageSize(value: number): value is ModLibraryPageSize {
  return MOD_LIBRARY_PAGE_SIZES.some((pageSize) => pageSize === value);
}

export function getModLibraryTotalPages(matchingTotal: number, pageSize: ModLibraryPageSize) {
  const normalizedTotal = normalizeNonNegativeInteger(matchingTotal);
  return normalizedTotal === 0 ? 0 : Math.ceil(normalizedTotal / pageSize);
}

export function getModLibraryItemRange(
  page: number,
  pageSize: ModLibraryPageSize,
  matchingTotal: number,
): ModLibraryItemRange {
  const normalizedTotal = normalizeNonNegativeInteger(matchingTotal);
  const totalPages = getModLibraryTotalPages(normalizedTotal, pageSize);

  if (totalPages === 0) {
    return { start: 0, end: 0 };
  }

  const normalizedPage = normalizePage(page, totalPages);
  return {
    start: (normalizedPage - 1) * pageSize + 1,
    end: Math.min(normalizedPage * pageSize, normalizedTotal),
  };
}

/*
 * 省略号代表一段被折叠的页码区间。点击它跳到该区间的中点：
 * 目标完全由相邻的两个页码推导，不引入"每次跳 5 页"这类魔法常量，
 * 也让用户可以通过反复点击二分逼近任意页，而不必逐页翻。
 * 相邻槽位不是数字、或两者之间没有真正的空隙时返回 null，调用方据此判定不可跳转。
 */
export function getModLibraryEllipsisTarget(
  slots: readonly ModLibraryPageSlot[],
  ellipsisIndex: number,
): number | null {
  const before = slots[ellipsisIndex - 1];
  const after = slots[ellipsisIndex + 1];

  if (typeof before !== "number" || typeof after !== "number") {
    return null;
  }

  if (after - before <= 1) {
    return null;
  }

  return Math.floor((before + after) / 2);
}

export function getModLibraryPageSlots(currentPage: number, totalPages: number): ModLibraryPageSlot[] {
  const normalizedTotalPages = normalizeNonNegativeInteger(totalPages);

  if (normalizedTotalPages === 0) {
    return [];
  }

  if (normalizedTotalPages <= 7) {
    return Array.from({ length: normalizedTotalPages }, (_, index) => index + 1);
  }

  const normalizedCurrentPage = normalizePage(currentPage, normalizedTotalPages);

  if (normalizedCurrentPage <= 4) {
    return [1, 2, 3, 4, 5, "ellipsis", normalizedTotalPages];
  }

  if (normalizedCurrentPage >= normalizedTotalPages - 3) {
    return [
      1,
      "ellipsis",
      normalizedTotalPages - 4,
      normalizedTotalPages - 3,
      normalizedTotalPages - 2,
      normalizedTotalPages - 1,
      normalizedTotalPages,
    ];
  }

  return [
    1,
    "ellipsis",
    normalizedCurrentPage - 1,
    normalizedCurrentPage,
    normalizedCurrentPage + 1,
    "ellipsis",
    normalizedTotalPages,
  ];
}
