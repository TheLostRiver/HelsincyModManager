import type { CategoryItem } from "./modCategoryApi";
import type { CategoryLabel, ModInstallStatus, ModLibraryItem } from "./modLibraryTypes";

export type ModLibraryFilter =
  | { kind: "all" }
  | { kind: "status"; status: ModInstallStatus }
  | { kind: "category"; categoryId: string; categoryName: string };

export type LibraryFilterChip = {
  key: string;
  label: string;
  filter: ModLibraryFilter;
  kind: ModLibraryFilter["kind"];
  color?: string;
};

const statusFilterChips: Array<{ label: string; status: ModInstallStatus }> = [
  { label: "已安装", status: "installed" },
  { label: "已禁用", status: "disabled" },
  { label: "存在冲突", status: "conflict" },
];

export const allLibraryFilter: ModLibraryFilter = { kind: "all" };

export function libraryFilterKey(filter: ModLibraryFilter) {
  switch (filter.kind) {
    case "all":
      return "all";
    case "status":
      return `status:${filter.status}`;
    case "category":
      return `category:${filter.categoryId}`;
  }
}

export function isSameLibraryFilter(a: ModLibraryFilter, b: ModLibraryFilter) {
  return libraryFilterKey(a) === libraryFilterKey(b);
}

export function buildLibraryFilterChips(categories: CategoryItem[]): LibraryFilterChip[] {
  const sortedCategories = categories
    .filter((category) => category.modCount > 0)
    .sort((a, b) => a.sortOrder - b.sortOrder || a.name.localeCompare(b.name, "zh-Hans-CN"));

  return [
    {
      key: libraryFilterKey(allLibraryFilter),
      label: "全部",
      kind: "all",
      filter: allLibraryFilter,
    },
    ...statusFilterChips.map((chip) => {
      const filter: ModLibraryFilter = { kind: "status", status: chip.status };
      return {
        key: libraryFilterKey(filter),
        label: chip.label,
        kind: "status" as const,
        filter,
      };
    }),
    ...sortedCategories.map((category) => {
      const filter: ModLibraryFilter = {
        kind: "category",
        categoryId: category.id,
        categoryName: category.name,
      };

      return {
        key: libraryFilterKey(filter),
        label: category.name,
        kind: "category" as const,
        color: category.color,
        filter,
      };
    }),
  ];
}

export function matchesLibraryFilter(item: ModLibraryItem, filter: ModLibraryFilter) {
  switch (filter.kind) {
    case "all":
      return true;
    case "status":
      return item.status === filter.status;
    case "category":
      return item.categoryLabels.some((category) => category.name === filter.categoryName);
  }
}

export function normalizeLibraryFilter(filter: ModLibraryFilter, chips: LibraryFilterChip[]) {
  return chips.some((chip) => isSameLibraryFilter(chip.filter, filter)) ? filter : allLibraryFilter;
}

export function visibleCategoryLabelsForCard(labels: CategoryLabel[], maxVisible = 3) {
  const visible = labels.slice(0, maxVisible);
  return {
    visible,
    overflowCount: Math.max(0, labels.length - visible.length),
  };
}
