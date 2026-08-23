import type { ModLibraryFilter } from "./modLibraryFilters";
import type { InstallManifestStatus } from "./modInstallPlanTypes";
import {
  DEFAULT_MOD_LIBRARY_PAGE_SIZE,
  isModLibraryPageSize,
} from "./modLibraryPaginationModel";
import type { ModLibraryPageSize } from "./modLibraryPaginationModel";
import type {
  ModInstallStatus,
  ModLibraryItem,
  ModLibraryPage,
  ModLibraryProfileContext,
  ModLibraryQueryFilter,
  QueryModLibraryInput,
} from "./modLibraryTypes";

export {
  DEFAULT_MOD_LIBRARY_PAGE_SIZE,
  MOD_LIBRARY_PAGE_SIZES,
} from "./modLibraryPaginationModel";
export type { ModLibraryPageSize } from "./modLibraryPaginationModel";

export const MOD_LIBRARY_PAGE_SIZE_STORAGE_KEY = "hmm.modLibrary.pageSize.v1";

type PageSizeStorage = {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
};

export function parseModLibraryPageSize(value: unknown): ModLibraryPageSize {
  const candidate = typeof value === "string" && value.trim() !== "" ? Number(value) : value;

  return typeof candidate === "number" && isModLibraryPageSize(candidate)
    ? candidate
    : DEFAULT_MOD_LIBRARY_PAGE_SIZE;
}

export function readModLibraryPageSize(storage: PageSizeStorage | null | undefined): ModLibraryPageSize {
  if (!storage) {
    return DEFAULT_MOD_LIBRARY_PAGE_SIZE;
  }

  try {
    return parseModLibraryPageSize(storage.getItem(MOD_LIBRARY_PAGE_SIZE_STORAGE_KEY));
  } catch {
    return DEFAULT_MOD_LIBRARY_PAGE_SIZE;
  }
}

export function writeModLibraryPageSize(
  storage: PageSizeStorage | null | undefined,
  value: unknown,
): ModLibraryPageSize {
  const pageSize = parseModLibraryPageSize(value);

  try {
    storage?.setItem(MOD_LIBRARY_PAGE_SIZE_STORAGE_KEY, String(pageSize));
  } catch {
    // Persisting a UI preference is best-effort; the in-memory selection remains usable.
  }

  return pageSize;
}

const installManifestStatuses = new Set<ModInstallStatus>([
  "not_installed",
  "installed",
  "committed_cleanup_pending",
  "cleanup_pending",
  "rollback_required",
  "repair_required",
  "unknown",
]);

export type ModLibraryQueryFilterBlockReason =
  | "profile_context_required"
  | "status_unsupported";

export type ModLibraryQueryFilterMapping =
  | { kind: "ready"; filter: ModLibraryQueryFilter }
  | { kind: "blocked"; filter: null; reason: ModLibraryQueryFilterBlockReason };

function hasUsableProfileContext(
  profileContext: ModLibraryProfileContext | null | undefined,
): profileContext is ModLibraryProfileContext {
  return Boolean(profileContext?.gameId.trim() && profileContext.profileId.trim());
}

export function isInstallManifestStatus(status: ModInstallStatus): status is InstallManifestStatus {
  return installManifestStatuses.has(status);
}

export function mapModLibraryFilterToQueryFilter(
  filter: ModLibraryFilter,
  profileContext?: ModLibraryProfileContext | null,
): ModLibraryQueryFilterMapping {
  switch (filter.kind) {
    case "all":
      return { kind: "ready", filter: { kind: "all" } };
    case "category":
      return {
        kind: "ready",
        filter: { kind: "category", categoryId: filter.categoryId },
      };
    case "status":
      if (!hasUsableProfileContext(profileContext)) {
        return {
          kind: "blocked",
          filter: null,
          reason: "profile_context_required",
        };
      }
      if (!isInstallManifestStatus(filter.status)) {
        return {
          kind: "blocked",
          filter: null,
          reason: "status_unsupported",
        };
      }
      return {
        kind: "ready",
        filter: { kind: "status", status: filter.status },
      };
  }
}

export type ModLibraryQueryErrorCode =
  | "game_id_invalid"
  | "profile_id_empty"
  | "mod_library_filter_invalid"
  | "mod_library_sort_invalid"
  | "mod_library_page_invalid"
  | "mod_library_page_size_unsupported"
  | "mod_library_search_too_long"
  | "mod_library_category_not_found"
  | "mod_library_profile_context_required"
  | "mod_library_unavailable"
  | "mod_library_status_unavailable";

// I18N-02 起本模块不再携带任何语言文案：错误只归一化为稳定 code，
// 文本由页面按当前界面语言从 modLibraryCopy.page.queryErrors 取。
const modLibraryQueryErrorCodes: ReadonlySet<string> = new Set([
  "game_id_invalid",
  "profile_id_empty",
  "mod_library_filter_invalid",
  "mod_library_sort_invalid",
  "mod_library_page_invalid",
  "mod_library_page_size_unsupported",
  "mod_library_search_too_long",
  "mod_library_category_not_found",
  "mod_library_profile_context_required",
  "mod_library_unavailable",
  "mod_library_status_unavailable",
]);

export type NormalizedModLibraryQueryErrorCode = ModLibraryQueryErrorCode | "unknown";

function getModLibraryQueryErrorCode(error: unknown): string | null {
  if (typeof error === "string") {
    return error;
  }
  if (typeof error !== "object" || error === null || !("code" in error)) {
    return null;
  }

  const code = (error as { code?: unknown }).code;
  return typeof code === "string" ? code : null;
}

export function normalizeModLibraryQueryErrorCode(
  error: unknown,
): NormalizedModLibraryQueryErrorCode {
  const code = getModLibraryQueryErrorCode(error);

  // 未知 code 一律归一化为 "unknown"，绝不透传后端原文（可能含路径等敏感内容）。
  return code !== null && modLibraryQueryErrorCodes.has(code)
    ? (code as ModLibraryQueryErrorCode)
    : "unknown";
}

export type LatestRequestSequenceGate = {
  beginRequest(): number;
  isLatest(requestId: number): boolean;
  invalidate(): number;
};

export function createLatestRequestSequenceGate(): LatestRequestSequenceGate {
  let latestRequestId = 0;

  const advance = () => {
    latestRequestId += 1;
    return latestRequestId;
  };

  return {
    beginRequest: advance,
    isLatest: (requestId) => requestId === latestRequestId,
    invalidate: advance,
  };
}

export function isCommittedModLibraryQueryResponse(
  requestIsLatest: boolean,
  committedQueryKey: string | null,
  requestQueryKey: string,
): boolean {
  return requestIsLatest && committedQueryKey === requestQueryKey;
}

export type OneShotQueryKeyConsumption = {
  matches: boolean;
  remainingKey: null;
};

export function consumeOneShotQueryKey(
  pendingKey: string | null,
  currentKey: string,
): OneShotQueryKeyConsumption {
  return {
    matches: pendingKey !== null && pendingKey === currentKey,
    remainingKey: null,
  };
}

export function resolveProfileQueryPage(
  previousProfileKey: string,
  profileKey: string,
  requestedPage: number,
) {
  return previousProfileKey === profileKey ? requestedPage : 1;
}

export type PlainBrowserDevRuntimeFlags = {
  isDev: boolean;
  hasWindow: boolean;
  hasTauriRuntime: boolean;
};

export function isPlainBrowserDevRuntime(flags: PlainBrowserDevRuntimeFlags): boolean {
  return flags.isDev && flags.hasWindow && !flags.hasTauriRuntime;
}

export type BrowserMockCategory = {
  id: string;
  name: string;
};

export class BrowserMockModLibraryQueryError extends Error {
  readonly code: ModLibraryQueryErrorCode;

  constructor(code: ModLibraryQueryErrorCode) {
    super(code);
    this.name = "BrowserMockModLibraryQueryError";
    this.code = code;
  }
}

export function normalizeBrowserMockInstallStatus(status: ModInstallStatus): InstallManifestStatus {
  if (status === "disabled") {
    return "not_installed";
  }
  if (status === "conflict") {
    return "unknown";
  }
  return status;
}

function normalizeBrowserMockItem(item: ModLibraryItem): ModLibraryItem {
  const normalizedSummary = item.installSummary
    ? {
        ...item.installSummary,
        status: normalizeBrowserMockInstallStatus(item.installSummary.status),
        issues: item.installSummary.issues?.map((issue) => ({ ...issue })),
      }
    : undefined;

  return {
    ...item,
    status: normalizeBrowserMockInstallStatus(normalizedSummary?.status ?? item.status),
    installSummary: normalizedSummary,
    categoryLabels: item.categoryLabels.map((label) => ({ ...label })),
  };
}

function includesSearch(item: ModLibraryItem, normalizedSearch: string): boolean {
  if (normalizedSearch === "") {
    return true;
  }

  return [item.name, item.author ?? "", ...item.categoryLabels.map((label) => label.name)].some(
    (value) => value.toLocaleLowerCase().includes(normalizedSearch),
  );
}

function compareText(left: string, right: string): number {
  return left.localeCompare(right, "zh-Hans-CN", { sensitivity: "base", numeric: true });
}

function clampRequestedPage(page: number, pageCount: number): number {
  if (!Number.isFinite(page)) {
    return 1;
  }
  return Math.min(Math.max(1, Math.trunc(page)), pageCount);
}

export function queryBrowserMockModLibrary(
  input: QueryModLibraryInput,
  sourceItems: readonly ModLibraryItem[],
  categories: readonly BrowserMockCategory[] = [],
): ModLibraryPage {
  if (input.sort !== "name_asc") {
    throw new BrowserMockModLibraryQueryError("mod_library_sort_invalid");
  }
  if (input.filter.kind === "status" && !hasUsableProfileContext(input.profileContext)) {
    throw new BrowserMockModLibraryQueryError("mod_library_profile_context_required");
  }

  const categoryId = input.filter.kind === "category" ? input.filter.categoryId : null;
  const categoryName =
    categoryId === null
      ? undefined
      : categories.find((category) => category.id === categoryId)?.name;
  if (input.filter.kind === "category" && categoryName === undefined) {
    throw new BrowserMockModLibraryQueryError("mod_library_category_not_found");
  }

  const normalizedSearch = input.search.trim().toLocaleLowerCase();
  const normalizedItems = sourceItems.map(normalizeBrowserMockItem);
  const matchingItems = normalizedItems
    .filter((item) => includesSearch(item, normalizedSearch))
    .filter((item) => {
      switch (input.filter.kind) {
        case "all":
          return true;
        case "status":
          return item.status === input.filter.status;
        case "category":
          return item.categoryLabels.some((label) => label.name === categoryName);
      }
    })
    .sort((left, right) => compareText(left.name, right.name) || compareText(left.id, right.id));

  const pageSize = parseModLibraryPageSize(input.pageSize);
  const pageCount = Math.max(1, Math.ceil(matchingItems.length / pageSize));
  const page = clampRequestedPage(input.page, pageCount);
  const pageStart = (page - 1) * pageSize;

  return {
    items: matchingItems.slice(pageStart, pageStart + pageSize),
    page,
    pageSize,
    libraryTotal: sourceItems.length,
    matchingTotal: matchingItems.length,
  };
}
