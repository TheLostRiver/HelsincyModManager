import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { ModLibraryFilter } from "./modLibraryFilters";
import {
  consumeOneShotQueryKey,
  createLatestRequestSequenceGate,
  normalizeModLibraryQueryErrorCode,
  isCommittedModLibraryQueryResponse,
  mapModLibraryFilterToQueryFilter,
  readModLibraryPageSize,
  resolveProfileQueryPage,
  resolveQueryStartExecutionState,
  writeModLibraryPageSize,
  type ModLibraryPageSize,
  type ModLibraryQueryExecutionState,
  type ModLibraryQueryFilterBlockReason,
} from "./modLibraryQueryState";
import type {
  ModLibraryItem,
  ModLibraryPage,
  ModLibraryProfileContext,
  QueryModLibraryInput,
} from "./modLibraryTypes";

const MOD_LIBRARY_SEARCH_DEBOUNCE_MS = 250;

type ModLibraryQueryRequest = {
  input: QueryModLibraryInput;
  profileKey: string;
  queryKey: string;
};

/**
 * 会话级缓存的最小接口。传进来就启用 stale-while-revalidate：切回本页时先摆出上次的
 * 结果，请求照发，回来再无声替换。不传就是原来的行为（每次挂载先出骨架屏）。
 *
 * 实现挂在 RouterOutlet 之上（`ModLibrarySessionCacheProvider`），因为页面本身
 * 活不过一次路由切换。
 */
export type ModLibraryQueryCache = {
  readPage: (profileKey: string, queryKey: string) => ModLibraryPage | null;
  writePage: (profileKey: string, queryKey: string, page: ModLibraryPage) => void;
  invalidatePage: (profileKey: string, queryKey: string) => void;
};

type UseModLibraryQueryInput = {
  rawSearch: string;
  filter: ModLibraryFilter;
  profileContext: ModLibraryProfileContext | null;
  loadPage: (input: QueryModLibraryInput) => Promise<ModLibraryPage>;
  cache?: ModLibraryQueryCache | null;
};

function getProfileKey(profileContext: ModLibraryProfileContext | null) {
  return profileContext === null
    ? "profile:none"
    : `profile:${profileContext.gameId}\u0000${profileContext.profileId}`;
}

function getBrowserStorage(): Storage | null {
  if (typeof window === "undefined") {
    return null;
  }

  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

function getQueryKey(input: QueryModLibraryInput) {
  return JSON.stringify(input);
}

export function useModLibraryQuery({
  rawSearch,
  filter,
  profileContext,
  loadPage,
  cache = null,
}: UseModLibraryQueryInput) {
  // 经 ref 取用：调用方若传了个每次渲染都新建的对象，直接进 effect 依赖会变成
  // 「每渲染一次重发一次请求」。缓存是可选优化，不该有能力把主查询拖成请求风暴。
  const cacheRef = useRef(cache);
  cacheRef.current = cache;
  const [submittedSearch, setSubmittedSearch] = useState(rawSearch);
  const [requestedPage, setRequestedPage] = useState(1);
  const [pageSize, setPageSizeState] = useState<ModLibraryPageSize>(() =>
    readModLibraryPageSize(getBrowserStorage()),
  );
  const profileKey = getProfileKey(profileContext);
  const previousProfileKeyRef = useRef(profileKey);
  const [executionState, setExecutionState] = useState<ModLibraryQueryExecutionState>({
    record: null,
    phase: "idle",
    phaseProfileKey: profileKey,
    errorCode: null,
  });
  const requestGateRef = useRef(createLatestRequestSequenceGate());
  const debounceTimerRef = useRef<number | null>(null);
  const skippedClampQueryKeyRef = useRef<string | null>(null);
  const skippedCommittedQueryEffectKeyRef = useRef<string | null>(null);
  const latestCommittedQueryKeyRef = useRef<string | null>(null);
  const latestRequestRef = useRef<ModLibraryQueryRequest | null>(null);
  const profileQueryPage = resolveProfileQueryPage(
    previousProfileKeyRef.current,
    profileKey,
    requestedPage,
  );

  useEffect(() => {
    if (previousProfileKeyRef.current === profileKey) {
      return;
    }
    previousProfileKeyRef.current = profileKey;
    skippedClampQueryKeyRef.current = null;
    setRequestedPage(1);
  }, [profileKey]);

  const filterMapping = useMemo(
    () => mapModLibraryFilterToQueryFilter(filter, profileContext),
    [filter, profileContext],
  );

  const queryInput = useMemo<QueryModLibraryInput | null>(() => {
    if (filterMapping.kind === "blocked") {
      return null;
    }

    return {
      ...(profileContext === null ? {} : { profileContext }),
      search: submittedSearch,
      filter: filterMapping.filter,
      sort: "name_asc",
      page: profileQueryPage,
      pageSize,
    };
  }, [filterMapping, pageSize, profileContext, profileQueryPage, submittedSearch]);

  const queryKey = queryInput === null ? null : getQueryKey(queryInput);

  useLayoutEffect(() => {
    if (latestCommittedQueryKeyRef.current === queryKey) {
      return;
    }

    latestCommittedQueryKeyRef.current = queryKey;
    latestRequestRef.current = queryInput === null || queryKey === null
      ? null
      : { input: queryInput, profileKey, queryKey };

    const clampConsumption = queryKey === null
      ? { matches: false, remainingKey: null }
      : consumeOneShotQueryKey(skippedClampQueryKeyRef.current, queryKey);
    skippedClampQueryKeyRef.current = clampConsumption.remainingKey;
    skippedCommittedQueryEffectKeyRef.current = clampConsumption.matches ? queryKey : null;
    if (clampConsumption.matches) {
      return;
    }

    requestGateRef.current.invalidate();
    if (queryKey === null) {
      return;
    }

    // 命中就先摆出上次的结果。注意这里**不** return——下面的请求照发，
    // 缓存只负责填住「请求在路上」这段空窗，不负责代替事实。
    const cachedPage = cacheRef.current?.readPage(profileKey, queryKey) ?? null;

    setExecutionState((current) => resolveQueryStartExecutionState(current, profileKey, cachedPage));
  }, [profileKey, queryInput, queryKey]);

  const executeQuery = useCallback(
    async (request: ModLibraryQueryRequest) => {
      const requestId = requestGateRef.current.beginRequest();

      const isCurrentResponse = () => isCommittedModLibraryQueryResponse(
        requestGateRef.current.isLatest(requestId),
        latestCommittedQueryKeyRef.current,
        request.queryKey,
      );

      // 这里不再查缓存：空窗期显示什么已经在 useLayoutEffect 里定过了，
      // 再查一遍只会把可能更新的在途结果按回到同一份旧快照。
      setExecutionState((current) =>
        resolveQueryStartExecutionState(current, request.profileKey, null));

      try {
        const page = await loadPage(request.input);
        if (!isCurrentResponse()) {
          return null;
        }

        setExecutionState({
          record: { profileKey: request.profileKey, page },
          phase: "idle",
          phaseProfileKey: request.profileKey,
          errorCode: null,
        });

        if (page.page !== request.input.page) {
          // 后端把页码夹紧了，这份结果对应的其实是夹紧后的查询。按请求时的 key 存会让
          // 下次请求越界页码时先闪一下别的页，所以按**结果实际对应**的 key 存。
          const clampedInput = { ...request.input, page: page.page };
          skippedClampQueryKeyRef.current = getQueryKey(clampedInput);
          cacheRef.current?.writePage(request.profileKey, skippedClampQueryKeyRef.current, page);
          setRequestedPage(page.page);
        } else {
          cacheRef.current?.writePage(request.profileKey, request.queryKey, page);
        }

        return page;
      } catch (error: unknown) {
        if (!isCurrentResponse()) {
          return null;
        }

        setExecutionState((current) => ({
          ...current,
          phase: "error",
          phaseProfileKey: request.profileKey,
          errorCode: normalizeModLibraryQueryErrorCode(error),
        }));
        throw error;
      }
    },
    [loadPage],
  );

  useEffect(() => {
    if (queryKey === null) {
      return;
    }

    if (skippedCommittedQueryEffectKeyRef.current === queryKey) {
      skippedCommittedQueryEffectKeyRef.current = null;
      return;
    }

    const request = latestRequestRef.current;
    if (request?.queryKey === queryKey) {
      void executeQuery(request).catch(() => undefined);
    }
  }, [executeQuery, queryKey]);

  useEffect(() => {
    if (debounceTimerRef.current !== null) {
      window.clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = null;
    }
    if (rawSearch === submittedSearch) {
      return undefined;
    }

    debounceTimerRef.current = window.setTimeout(() => {
      debounceTimerRef.current = null;
      setRequestedPage(1);
      setSubmittedSearch(rawSearch);
    }, MOD_LIBRARY_SEARCH_DEBOUNCE_MS);

    return () => {
      if (debounceTimerRef.current !== null) {
        window.clearTimeout(debounceTimerRef.current);
        debounceTimerRef.current = null;
      }
    };
  }, [rawSearch, submittedSearch]);

  useEffect(
    () => () => {
      requestGateRef.current.invalidate();
      if (debounceTimerRef.current !== null) {
        window.clearTimeout(debounceTimerRef.current);
      }
    },
    [],
  );

  const setPage = useCallback((nextPage: number) => {
    setRequestedPage(Math.max(1, Math.trunc(nextPage)));
  }, []);

  const setPageSize = useCallback((nextPageSize: ModLibraryPageSize) => {
    const persistedPageSize = writeModLibraryPageSize(getBrowserStorage(), nextPageSize);
    setRequestedPage(1);
    setPageSizeState(persistedPageSize);
  }, []);

  const resetPage = useCallback(() => {
    setRequestedPage(1);
  }, []);

  const flushSearch = useCallback(() => {
    if (debounceTimerRef.current !== null) {
      window.clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = null;
    }
    setRequestedPage(1);
    setSubmittedSearch(rawSearch);
  }, [rawSearch]);

  const refresh = useCallback(async () => {
    const request = latestRequestRef.current;
    if (request === null) {
      return null;
    }
    return executeQuery(request);
  }, [executeQuery]);

  const updateCurrentPageItems = useCallback(
    (update: (items: ModLibraryItem[]) => ModLibraryItem[]) => {
      // 这条路径只在页面已经判定手里这份不可信时走（安装终态校验失败的 fail-closed 标记）。
      // 缓存里那份是标记**之前**的快照，留着复用等于下次进页面把已知有问题的状态又摆回来，
      // 所以直接丢掉槽位，让下次重新查——缓存失效必须倒向重新取数。
      //
      // 故意不看下面那个「记录属不属于当前配置档」的判据：判据不成立时多丢一个槽位的代价
      // 只是下次多查一遍，而漏丢一个槽位的代价是留下一份已知不可信的快照。
      const committedQueryKey = latestCommittedQueryKeyRef.current;
      if (committedQueryKey !== null) {
        cacheRef.current?.invalidatePage(profileKey, committedQueryKey);
      }

      setExecutionState((current) => {
        if (current.record?.profileKey !== profileKey) {
          return current;
        }

        return {
          ...current,
          record: {
            ...current.record,
            page: {
              ...current.record.page,
              items: update(current.record.page.items),
            },
          },
        };
      });
    },
    [profileKey],
  );

  const page = executionState.record?.profileKey === profileKey ? executionState.record.page : null;
  const phaseIsCurrent = executionState.phaseProfileKey === profileKey;
  const phase = phaseIsCurrent ? executionState.phase : "initial-loading";
  const errorCode = phaseIsCurrent ? executionState.errorCode : null;
  const blockedReason: ModLibraryQueryFilterBlockReason | null =
    filterMapping.kind === "blocked" ? filterMapping.reason : null;

  return {
    page,
    pageSize,
    submittedSearch,
    initialLoading: blockedReason === null && page === null && phase !== "error",
    refreshing: blockedReason === null && page !== null && phase === "refreshing",
    errorCode,
    blockedReason,
    setPage,
    setPageSize,
    resetPage,
    flushSearch,
    refresh,
    updateCurrentPageItems,
  };
}
