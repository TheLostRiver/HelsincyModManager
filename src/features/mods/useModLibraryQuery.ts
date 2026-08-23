import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { ModLibraryFilter } from "./modLibraryFilters";
import {
  consumeOneShotQueryKey,
  createLatestRequestSequenceGate,
  normalizeModLibraryQueryErrorCode,
  type NormalizedModLibraryQueryErrorCode,
  isCommittedModLibraryQueryResponse,
  mapModLibraryFilterToQueryFilter,
  readModLibraryPageSize,
  resolveProfileQueryPage,
  writeModLibraryPageSize,
  type ModLibraryPageSize,
  type ModLibraryQueryFilterBlockReason,
} from "./modLibraryQueryState";
import type {
  ModLibraryItem,
  ModLibraryPage,
  ModLibraryProfileContext,
  QueryModLibraryInput,
} from "./modLibraryTypes";

const MOD_LIBRARY_SEARCH_DEBOUNCE_MS = 250;

type ModLibraryQueryPhase = "idle" | "initial-loading" | "refreshing" | "error";

type ModLibraryQueryRecord = {
  profileKey: string;
  page: ModLibraryPage;
};

type ModLibraryQueryExecutionState = {
  record: ModLibraryQueryRecord | null;
  phase: ModLibraryQueryPhase;
  phaseProfileKey: string;
  errorCode: NormalizedModLibraryQueryErrorCode | null;
};

type ModLibraryQueryRequest = {
  input: QueryModLibraryInput;
  profileKey: string;
  queryKey: string;
};

type UseModLibraryQueryInput = {
  rawSearch: string;
  filter: ModLibraryFilter;
  profileContext: ModLibraryProfileContext | null;
  loadPage: (input: QueryModLibraryInput) => Promise<ModLibraryPage>;
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
}: UseModLibraryQueryInput) {
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

    setExecutionState((current) => {
      const hasCurrentProfilePage = current.record?.profileKey === profileKey;
      return {
        ...current,
        phase: hasCurrentProfilePage ? "refreshing" : "initial-loading",
        phaseProfileKey: profileKey,
        errorCode: null,
      };
    });
  }, [profileKey, queryInput, queryKey]);

  const executeQuery = useCallback(
    async (request: ModLibraryQueryRequest) => {
      const requestId = requestGateRef.current.beginRequest();

      const isCurrentResponse = () => isCommittedModLibraryQueryResponse(
        requestGateRef.current.isLatest(requestId),
        latestCommittedQueryKeyRef.current,
        request.queryKey,
      );

      setExecutionState((current) => {
        const hasCurrentProfilePage = current.record?.profileKey === request.profileKey;
        return {
          ...current,
          phase: hasCurrentProfilePage ? "refreshing" : "initial-loading",
          phaseProfileKey: request.profileKey,
          errorCode: null,
        };
      });

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
          const clampedInput = { ...request.input, page: page.page };
          skippedClampQueryKeyRef.current = getQueryKey(clampedInput);
          setRequestedPage(page.page);
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
