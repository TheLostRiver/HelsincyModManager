import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { test } from "node:test";

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (
      specifier === "./modLibraryPaginationModel"
      && context.parentURL?.endsWith("/modLibraryQueryState.ts")
    ) {
      return nextResolve("./modLibraryPaginationModel.ts", context);
    }
    return nextResolve(specifier, context);
  },
});

const {
  BrowserMockModLibraryQueryError,
  DEFAULT_MOD_LIBRARY_PAGE_SIZE,
  MOD_LIBRARY_PAGE_SIZE_STORAGE_KEY,
  consumeOneShotQueryKey,
  createLatestRequestSequenceGate,
  normalizeModLibraryQueryErrorCode,
  isCommittedModLibraryQueryResponse,
  isPlainBrowserDevRuntime,
  mapModLibraryFilterToQueryFilter,
  normalizeBrowserMockInstallStatus,
  parseModLibraryPageSize,
  queryBrowserMockModLibrary,
  readModLibraryPageSize,
  resolveProfileQueryPage,
  writeModLibraryPageSize,
} = await import("./modLibraryQueryState.ts");

function createStorage(initialValue = null) {
  const values = new Map();
  if (initialValue !== null) {
    values.set(MOD_LIBRARY_PAGE_SIZE_STORAGE_KEY, initialValue);
  }

  return {
    values,
    getItem(key) {
      return values.get(key) ?? null;
    },
    setItem(key, value) {
      values.set(key, value);
    },
  };
}

test("page size parsing accepts only the supported values", () => {
  for (const pageSize of [12, 24, 48, 96]) {
    assert.equal(parseModLibraryPageSize(pageSize), pageSize);
    assert.equal(parseModLibraryPageSize(String(pageSize)), pageSize);
  }

  for (const invalidValue of [null, undefined, "", "25", 25, 24.5, Number.NaN, Infinity, {}]) {
    assert.equal(parseModLibraryPageSize(invalidValue), DEFAULT_MOD_LIBRARY_PAGE_SIZE);
  }
});

test("page size storage reads, writes and falls back without surfacing storage failures", () => {
  const storage = createStorage("48");
  assert.equal(readModLibraryPageSize(storage), 48);
  assert.equal(writeModLibraryPageSize(storage, 96), 96);
  assert.equal(storage.values.get(MOD_LIBRARY_PAGE_SIZE_STORAGE_KEY), "96");

  assert.equal(writeModLibraryPageSize(storage, 999), DEFAULT_MOD_LIBRARY_PAGE_SIZE);
  assert.equal(storage.values.get(MOD_LIBRARY_PAGE_SIZE_STORAGE_KEY), "24");
  assert.equal(readModLibraryPageSize(null), DEFAULT_MOD_LIBRARY_PAGE_SIZE);

  const failingStorage = {
    getItem() {
      throw new Error("storage blocked");
    },
    setItem() {
      throw new Error("storage blocked");
    },
  };
  assert.equal(readModLibraryPageSize(failingStorage), DEFAULT_MOD_LIBRARY_PAGE_SIZE);
  assert.equal(writeModLibraryPageSize(failingStorage, 48), 48);
});

test("library filters map to the backend query shape without display-only fields", () => {
  assert.deepEqual(mapModLibraryFilterToQueryFilter({ kind: "all" }), {
    kind: "ready",
    filter: { kind: "all" },
  });
  assert.deepEqual(
    mapModLibraryFilterToQueryFilter({
      kind: "category",
      categoryId: "cat-armor",
      categoryName: "Armor",
    }),
    {
      kind: "ready",
      filter: { kind: "category", categoryId: "cat-armor" },
    },
  );
  assert.deepEqual(
    mapModLibraryFilterToQueryFilter(
      { kind: "status", status: "installed" },
      { gameId: "mhw", profileId: "default" },
    ),
    {
      kind: "ready",
      filter: { kind: "status", status: "installed" },
    },
  );
});

test("status filters are explicitly blocked without a usable profile context", () => {
  for (const profileContext of [undefined, null, { gameId: "mhw", profileId: "" }]) {
    assert.deepEqual(
      mapModLibraryFilterToQueryFilter({ kind: "status", status: "installed" }, profileContext),
      {
        kind: "blocked",
        filter: null,
        reason: "profile_context_required",
      },
    );
  }
});

test("legacy display statuses never degrade into an all query", () => {
  for (const status of ["disabled", "conflict"]) {
    assert.deepEqual(
      mapModLibraryFilterToQueryFilter(
        { kind: "status", status },
        { gameId: "mhw", profileId: "default" },
      ),
      {
        kind: "blocked",
        filter: null,
        reason: "status_unsupported",
      },
    );
  }
});

test("stable query error codes normalize without leaking backend messages", () => {
  // I18N-02 起文本在渲染层取词；本层只保证 code 归一化 + 未知错误不透传原文。
  assert.equal(
    normalizeModLibraryQueryErrorCode({ code: "mod_library_search_too_long" }),
    "mod_library_search_too_long",
  );
  assert.equal(
    normalizeModLibraryQueryErrorCode({
      code: "mod_library_profile_context_required",
      message: "C:/Users/private/profile.json",
    }),
    "mod_library_profile_context_required",
  );
  assert.equal(
    normalizeModLibraryQueryErrorCode("mod_library_status_unavailable"),
    "mod_library_status_unavailable",
  );
  assert.equal(normalizeModLibraryQueryErrorCode({ code: "future_error", message: "secret" }), "unknown");
  assert.equal(normalizeModLibraryQueryErrorCode(new Error("raw backend error")), "unknown");
});

test("latest request gate rejects responses from superseded and invalidated requests", () => {
  const gate = createLatestRequestSequenceGate();
  const first = gate.beginRequest();
  assert.equal(gate.isLatest(first), true);

  const second = gate.beginRequest();
  assert.equal(gate.isLatest(first), false);
  assert.equal(gate.isLatest(second), true);

  gate.invalidate();
  assert.equal(gate.isLatest(second), false);
});

test("query responses require both the latest request id and committed query key", () => {
  assert.equal(isCommittedModLibraryQueryResponse(true, "query:new", "query:new"), true);
  assert.equal(isCommittedModLibraryQueryResponse(false, "query:new", "query:new"), false);
  assert.equal(isCommittedModLibraryQueryResponse(true, "query:new", "query:old"), false);
  assert.equal(isCommittedModLibraryQueryResponse(true, null, "query:old"), false);
});

test("one-shot query keys match at most once and clear on any consumption", () => {
  assert.deepEqual(consumeOneShotQueryKey("query:2", "query:2"), {
    matches: true,
    remainingKey: null,
  });
  assert.deepEqual(consumeOneShotQueryKey("query:2", "query:status-filter"), {
    matches: false,
    remainingKey: null,
  });
  assert.deepEqual(consumeOneShotQueryKey(null, "query:2"), {
    matches: false,
    remainingKey: null,
  });
});

test("profile changes query page one before synchronizing stored pagination state", () => {
  assert.equal(resolveProfileQueryPage("profile:old", "profile:new", 7), 1);
  assert.equal(resolveProfileQueryPage("profile:same", "profile:same", 7), 7);
  assert.equal(resolveProfileQueryPage("profile:none", "profile:none", 1), 1);
});

test("plain-browser mocks are enabled only for a windowed development runtime without Tauri", () => {
  assert.equal(
    isPlainBrowserDevRuntime({ isDev: true, hasWindow: true, hasTauriRuntime: false }),
    true,
  );
  assert.equal(
    isPlainBrowserDevRuntime({ isDev: false, hasWindow: true, hasTauriRuntime: false }),
    false,
  );
  assert.equal(
    isPlainBrowserDevRuntime({ isDev: true, hasWindow: false, hasTauriRuntime: false }),
    false,
  );
  assert.equal(
    isPlainBrowserDevRuntime({ isDev: true, hasWindow: true, hasTauriRuntime: true }),
    false,
  );
});

const mockItems = [
  {
    id: "zeta-2",
    name: "Zeta",
    author: "Hunter",
    sizeLabel: "2 MB",
    status: "disabled",
    categoryLabels: [{ name: "Armor" }, { name: "Classic" }],
  },
  {
    id: "alpha-2",
    name: "Alpha",
    author: "Smith",
    sizeLabel: "3 MB",
    status: "conflict",
    categoryLabels: [{ name: "Weapon" }],
  },
  {
    id: "alpha-1",
    name: "Alpha",
    author: "Maker",
    sizeLabel: "4 MB",
    status: "installed",
    categoryLabels: [{ name: "Armor" }],
  },
];

function createQuery(overrides = {}) {
  return {
    profileContext: { gameId: "mhw", profileId: "default" },
    search: "",
    filter: { kind: "all" },
    sort: "name_asc",
    page: 1,
    pageSize: 12,
    ...overrides,
  };
}

test("browser mock query searches names, authors and category tags", () => {
  for (const search of ["zeta", "hunter", "classic"]) {
    const page = queryBrowserMockModLibrary(createQuery({ search }), mockItems);
    assert.deepEqual(page.items.map((item) => item.id), ["zeta-2"]);
  }

  const trimmedCaseInsensitive = queryBrowserMockModLibrary(
    createQuery({ search: "  SMITH  " }),
    mockItems,
  );
  assert.deepEqual(trimmedCaseInsensitive.items.map((item) => item.id), ["alpha-2"]);
});

test("browser mock category query resolves category ids and rejects stale ids", () => {
  const categories = [
    { id: "cat-armor", name: "Armor" },
    { id: "cat-weapon", name: "Weapon" },
  ];
  const page = queryBrowserMockModLibrary(
    createQuery({ filter: { kind: "category", categoryId: "cat-armor" } }),
    mockItems,
    categories,
  );

  assert.deepEqual(page.items.map((item) => item.id), ["alpha-1", "zeta-2"]);
  assert.throws(
    () =>
      queryBrowserMockModLibrary(
        createQuery({ filter: { kind: "category", categoryId: "stale" } }),
        mockItems,
        categories,
      ),
    (error) =>
      error instanceof BrowserMockModLibraryQueryError
      && error.code === "mod_library_category_not_found",
  );
});

test("browser mock query applies stable name/id sorting and clamps oversized pages", () => {
  const firstPage = queryBrowserMockModLibrary(createQuery(), mockItems);
  assert.deepEqual(firstPage.items.map((item) => item.id), ["alpha-1", "alpha-2", "zeta-2"]);

  const manyItems = Array.from({ length: 13 }, (_, index) => ({
    id: `mod-${String(index + 1).padStart(2, "0")}`,
    name: `Mod ${String(index + 1).padStart(2, "0")}`,
    sizeLabel: "1 MB",
    status: "not_installed",
    categoryLabels: [],
  }));
  const clamped = queryBrowserMockModLibrary(createQuery({ page: 99 }), manyItems);
  assert.equal(clamped.page, 2);
  assert.equal(clamped.pageSize, 12);
  assert.equal(clamped.libraryTotal, 13);
  assert.equal(clamped.matchingTotal, 13);
  assert.deepEqual(clamped.items.map((item) => item.id), ["mod-13"]);

  const empty = queryBrowserMockModLibrary(createQuery({ page: 99, search: "missing" }), manyItems);
  assert.equal(empty.page, 1);
  assert.equal(empty.matchingTotal, 0);
  assert.deepEqual(empty.items, []);
});

test("browser mock query normalizes legacy statuses without mutating source items", () => {
  assert.equal(normalizeBrowserMockInstallStatus("disabled"), "not_installed");
  assert.equal(normalizeBrowserMockInstallStatus("conflict"), "unknown");
  assert.equal(normalizeBrowserMockInstallStatus("installed"), "installed");

  const notInstalled = queryBrowserMockModLibrary(
    createQuery({ filter: { kind: "status", status: "not_installed" } }),
    mockItems,
  );
  const unknown = queryBrowserMockModLibrary(
    createQuery({ filter: { kind: "status", status: "unknown" } }),
    mockItems,
  );

  assert.deepEqual(notInstalled.items.map((item) => item.id), ["zeta-2"]);
  assert.deepEqual(unknown.items.map((item) => item.id), ["alpha-2"]);
  assert.equal(mockItems[0].status, "disabled");
  assert.equal(mockItems[1].status, "conflict");
});

test("browser mock status queries require profile context", () => {
  assert.throws(
    () =>
      queryBrowserMockModLibrary(
        createQuery({
          profileContext: undefined,
          filter: { kind: "status", status: "installed" },
        }),
        mockItems,
      ),
    (error) =>
      error instanceof BrowserMockModLibraryQueryError
      && error.code === "mod_library_profile_context_required",
  );
});
