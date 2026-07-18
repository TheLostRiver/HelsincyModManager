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

  const normalizedPage = Math.min(Math.max(1, Math.floor(page)), totalPages);
  return {
    start: (normalizedPage - 1) * pageSize + 1,
    end: Math.min(normalizedPage * pageSize, normalizedTotal),
  };
}

export function getModLibraryPageSlots(currentPage: number, totalPages: number): ModLibraryPageSlot[] {
  const normalizedTotalPages = normalizeNonNegativeInteger(totalPages);

  if (normalizedTotalPages === 0) {
    return [];
  }

  if (normalizedTotalPages <= 7) {
    return Array.from({ length: normalizedTotalPages }, (_, index) => index + 1);
  }

  const normalizedCurrentPage = Math.min(
    Math.max(1, Math.floor(currentPage)),
    normalizedTotalPages,
  );

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
