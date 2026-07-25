import type { CategoryItem } from "./modCategoryApi";
import type { InstallManifestStatus } from "./modInstallPlanTypes";
import type { CategoryLabel, ModLibraryItem } from "./modLibraryTypes";

export type ModLibraryFilter =
  | { kind: "all" }
  | { kind: "status"; status: InstallManifestStatus }
  | { kind: "category"; categoryId: string; categoryName: string };

export type LibraryFilterChip = {
  key: string;
  label: string;
  filter: ModLibraryFilter;
  kind: ModLibraryFilter["kind"];
  color?: string;
  disabled?: boolean;
  disabledReason?: string;
};

const statusFilterChips: Array<{ label: string; status: InstallManifestStatus }> = [
  { label: "已安装", status: "installed" },
  { label: "未安装", status: "not_installed" },
];

type BuildLibraryFilterChipsOptions = {
  statusFiltersEnabled?: boolean;
  statusDisabledReason?: string;
};

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

export function buildLibraryFilterChips(
  categories: CategoryItem[],
  {
    statusFiltersEnabled = true,
    statusDisabledReason = "选择配置档后可用",
  }: BuildLibraryFilterChipsOptions = {},
): LibraryFilterChip[] {
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
        disabled: !statusFiltersEnabled,
        disabledReason: statusFiltersEnabled ? undefined : statusDisabledReason,
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
        color: category.color ?? undefined,
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
  const matchedFilter = chips.find((chip) => isSameLibraryFilter(chip.filter, filter))?.filter;
  if (!matchedFilter) {
    return allLibraryFilter;
  }
  if (
    filter.kind === "category"
    && matchedFilter.kind === "category"
    && filter.categoryName !== matchedFilter.categoryName
  ) {
    return matchedFilter;
  }
  return filter;
}

export function visibleCategoryLabelsForCard(labels: CategoryLabel[], maxVisible = 3) {
  const visible = labels.slice(0, maxVisible);
  return {
    visible,
    overflowCount: Math.max(0, labels.length - visible.length),
  };
}
