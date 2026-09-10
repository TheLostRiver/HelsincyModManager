import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
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
const noCacheGeneration = () => 0;
const noCacheSubscription = () => () => {};

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
  writePage: (profileKey: string, queryKey: string, page: ModLibraryPage, generation: number) => void;
  invalidatePage: (profileKey: string, queryKey: string) => void;
  getGeneration: () => number;
  subscribe: (notify: () => void) => () => void;
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
  //
  // 同步走 layout effect 而不是 render 期间直接赋值：render 期间写 ref 违反 React 的
  // 约定（唯一被许可的例外是惰性初始化），因为 render 可能被丢弃或重跑。今天本仓库
  // 没有并发特性、且这里写的是 Provider 持有的恒定 store 对象，所以还看不出
  // 差别——但那是**碰巧**无害，不是设计上无害。
  //
  // 声明顺序有意义：本 effect 必须排在下面读缓存的那个 layout effect 之前，同一次提交里
  // 才会先同步后读取。挂载那一次由 `useRef(cache)` 的初值兜住。
  const cacheRef = useRef(cache);
  useLayoutEffect(() => {
    cacheRef.current = cache;
  }, [cache]);
  const cacheGeneration = useSyncExternalStore(
    cache?.subscribe ?? noCacheSubscription,
    cache?.getGeneration ?? noCacheGeneration,
    noCacheGeneration,
  );
  const mountedRef = useRef(false);
  useLayoutEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);
  const [submittedSearch, setSubmittedSearch] = useState(rawSearch);
  const [requestedPage, setRequestedPage] = useState(1);
  const [pageSize, setPageSizeState] = useState<ModLibraryPageSize>(() =>
    readModLibraryPageSize(getBrowserStorage()),
  );
  const profileKey = getProfileKey(profileContext);
  const previousProfileKeyRef = useRef(profileKey);
  const [executionState, setExecutionState] = useState<ModLibraryQueryExecutionState & { generation: number }>({
    record: null,
    phase: "idle",
    phaseProfileKey: profileKey,
    errorCode: null,
    generation: cacheGeneration,
  });
  const requestGateRef = useRef(createLatestRequestSequenceGate());
  const debounceTimerRef = useRef<number | null>(null);
  const skippedClampQueryKeyRef = useRef<string | null>(null);
  const skippedCommittedQueryEffectKeyRef = useRef<string | null>(null);
  const latestCommittedQueryKeyRef = useRef<string | null>(null);
  const latestCommittedGenerationRef = useRef(cacheGeneration);
  const lastExecutionRef = useRef<{ queryKey: string; generation: number; loadPage: typeof loadPage } | null>(null);
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
    const generationChanged = latestCommittedGenerationRef.current !== cacheGeneration;
    if (latestCommittedQueryKeyRef.current === queryKey && !generationChanged) {
      return;
    }

    latestCommittedQueryKeyRef.current = queryKey;
    latestCommittedGenerationRef.current = cacheGeneration;
    latestRequestRef.current = queryInput === null || queryKey === null
      ? null
      : { input: queryInput, profileKey, queryKey };

    const clampConsumption = queryKey === null || generationChanged
      ? { matches: false, remainingKey: null }
      : consumeOneShotQueryKey(skippedClampQueryKeyRef.current, queryKey);
    skippedClampQueryKeyRef.current = clampConsumption.remainingKey;
    skippedCommittedQueryEffectKeyRef.current = clampConsumption.matches ? queryKey : null;
    if (clampConsumption.matches) {
      return;
    }

    if (lastExecutionRef.current?.queryKey !== queryKey
      || lastExecutionRef.current?.generation !== cacheGeneration) {
      requestGateRef.current.invalidate();
    }
    if (queryKey === null) {
      return;
    }

    // 命中就先摆出上次的结果。注意这里**不** return——下面的请求照发，
    // 缓存只负责填住「请求在路上」这段空窗，不负责代替事实。
    const cachedPage = cacheRef.current?.readPage(profileKey, queryKey) ?? null;

    setExecutionState((current) => ({
      ...resolveQueryStartExecutionState(
        current.generation === cacheGeneration ? current : { ...current, record: null },
        profileKey,
        cachedPage,
      ),
      generation: cacheGeneration,
    }));
  }, [cacheGeneration, profileKey, queryInput, queryKey]);

  const executeQuery = useCallback(
    async (request: ModLibraryQueryRequest) => {
      if (!mountedRef.current) return null;
      const requestId = requestGateRef.current.beginRequest();
      const generation = cacheRef.current?.getGeneration() ?? 0;
      lastExecutionRef.current = { queryKey: request.queryKey, generation, loadPage };

      const isCurrentResponse = () => isCommittedModLibraryQueryResponse(
        mountedRef.current && requestGateRef.current.isLatest(requestId)
          && generation === (cacheRef.current?.getGeneration() ?? 0),
        latestCommittedQueryKeyRef.current,
        request.queryKey,
      );

      // 这里不再查缓存：空窗期显示什么已经在 useLayoutEffect 里定过了，
      // 再查一遍只会把可能更新的在途结果按回到同一份旧快照。
      setExecutionState((current) => ({
        ...resolveQueryStartExecutionState(
          current.generation === generation ? current : { ...current, record: null },
          request.profileKey,
          null,
        ),
        generation,
      }));

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
          generation,
        });

        if (page.page !== request.input.page) {
          // 后端把页码夹紧了，这份结果对应的其实是夹紧后的查询。按请求时的 key 存会让
          // 下次请求越界页码时先闪一下别的页，所以按**结果实际对应**的 key 存。
          const clampedInput = { ...request.input, page: page.page };
          skippedClampQueryKeyRef.current = getQueryKey(clampedInput);
          cacheRef.current?.writePage(request.profileKey, skippedClampQueryKeyRef.current, page, generation);
          setRequestedPage(page.page);
        } else {
          cacheRef.current?.writePage(request.profileKey, request.queryKey, page, generation);
        }

        return page;
      } catch (error: unknown) {
        if (!isCurrentResponse()) {
          return null;
        }

        cacheRef.current?.invalidatePage(request.profileKey, request.queryKey);
        setExecutionState((current) => ({
          ...current,
          record: null,
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
    const previous = lastExecutionRef.current;
    if (previous?.queryKey === queryKey && previous.generation === cacheGeneration
      && previous.loadPage === loadPage) return;
    if (request?.queryKey === queryKey) {
      void executeQuery(request).catch(() => undefined);
    }
  }, [cacheGeneration, executeQuery, loadPage, queryKey]);

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
      lastExecutionRef.current = null;
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
      if (!mountedRef.current || latestRequestRef.current?.profileKey !== profileKey) return;
      // 这条路径只在页面已经判定手里这份不可信时走（安装终态校验失败的 fail-closed 标记）。
      // 缓存里那份是标记**之前**的快照，留着复用等于下次进页面把已知有问题的状态又摆回来，
      // 所以直接丢掉槽位，让下次重新查——缓存失效必须倒向重新取数。
      //
      // 故意不看下面那个「记录属不属于当前配置档」的判据：判据不成立时多丢一个槽位的代价
      // 只是下次多查一遍，而漏丢一个槽位的代价是留下一份已知不可信的快照。
      const committedQueryKey = latestCommittedQueryKeyRef.current;
      const generation = cacheRef.current?.getGeneration() ?? 0;
      requestGateRef.current.invalidate();
      if (committedQueryKey !== null) {
        cacheRef.current?.invalidatePage(profileKey, committedQueryKey);
      }

      setExecutionState((current) => {
        if (current.record?.profileKey !== profileKey || current.generation !== generation) {
          return {
            ...current,
            record: null,
            phase: "error",
            phaseProfileKey: profileKey,
            errorCode: "mod_library_status_unavailable",
            generation,
          };
        }

        return {
          ...current,
          phase: "idle",
          errorCode: null,
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

  const generationIsCurrent = executionState.generation === cacheGeneration;
  const page = generationIsCurrent && executionState.record?.profileKey === profileKey ? executionState.record.page : null;
  const phaseIsCurrent = generationIsCurrent && executionState.phaseProfileKey === profileKey;
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
