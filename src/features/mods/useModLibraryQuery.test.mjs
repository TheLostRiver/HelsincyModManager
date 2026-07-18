import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync("src/features/mods/useModLibraryQuery.ts", "utf8");

test("the query hook debounces search for 250ms and supports an immediate flush", () => {
  assert.match(source, /MOD_LIBRARY_SEARCH_DEBOUNCE_MS = 250/);
  assert.match(source, /window\.setTimeout[\s\S]*MOD_LIBRARY_SEARCH_DEBOUNCE_MS/);
  assert.match(source, /const flushSearch = useCallback/);
  assert.match(source, /setSubmittedSearch\(rawSearch\)/);
  assert.match(source, /window\.clearTimeout/);
});

test("every request shares one latest-response gate for success and failure", () => {
  assert.match(source, /requestGateRef = useRef\(createLatestRequestSequenceGate\(\)\)/);
  assert.match(source, /const requestId = requestGateRef\.current\.beginRequest\(\)/);
  assert.match(source, /const isCurrentResponse = \(\) => isCommittedModLibraryQueryResponse\(/);
  assert.match(source, /requestGateRef\.current\.isLatest\(requestId\)/);
  assert.match(source, /latestCommittedQueryKeyRef\.current/);
  assert.match(source, /request\.queryKey/);
  assert.equal(source.match(/if \(!isCurrentResponse\(\)\)/g)?.length, 2);
  assert.match(source, /requestGateRef\.current\.invalidate\(\)/);
});

test("committed query changes invalidate stale responses before paint", () => {
  assert.match(source, /useLayoutEffect\(\(\) => \{/);
  assert.match(source, /latestCommittedQueryKeyRef\.current === queryKey/);
  assert.match(source, /latestCommittedQueryKeyRef\.current = queryKey/);
  assert.match(source, /requestGateRef\.current\.invalidate\(\)/);
  assert.match(source, /phase: hasCurrentProfilePage \? "refreshing" : "initial-loading"/);
  assert.match(source, /skippedCommittedQueryEffectKeyRef\.current = clampConsumption\.matches \? queryKey : null/);
});

test("backend page clamp updates the requested page without issuing a duplicate effect query", () => {
  assert.match(source, /page\.page !== request\.input\.page/);
  assert.match(source, /skippedClampQueryKeyRef\.current = getQueryKey\(clampedInput\)/);
  assert.match(source, /consumeOneShotQueryKey\(skippedClampQueryKeyRef\.current, queryKey\)/);
  assert.match(source, /skippedClampQueryKeyRef\.current = clampConsumption\.remainingKey/);
  assert.match(source, /if \(clampConsumption\.matches\)/);
  assert.match(source, /skippedCommittedQueryEffectKeyRef\.current === queryKey/);
  assert.match(source, /setRequestedPage\(page\.page\)/);
});

test("refresh reuses the latest committed query and current-profile data only", () => {
  assert.match(source, /latestRequestRef\.current/);
  assert.match(source, /return executeQuery\(request\)/);
  assert.match(source, /current\.record\?\.profileKey !== profileKey/);
  assert.doesNotMatch(source, /getModLibrary\(|visibleItems|\.slice\(/);
});

test("blocked status filters stop loading instead of degrading to an all query", () => {
  assert.match(source, /filterMapping\.kind === "blocked"[\s\S]*?return null/);
  assert.match(source, /blockedReason === null && page === null && phase !== "error"/);
  assert.match(source, /blockedReason === null && page !== null && phase === "refreshing"/);
});

test("semantic query keys suppress duplicate effects and profile changes query page one", () => {
  assert.match(source, /previousProfileKeyRef = useRef\(profileKey\)/);
  assert.match(source, /resolveProfileQueryPage\([\s\S]*previousProfileKeyRef\.current,[\s\S]*profileKey,[\s\S]*requestedPage/);
  assert.match(source, /const queryKey = queryInput === null \? null : getQueryKey\(queryInput\)/);
  assert.match(source, /latestRequestRef\.current/);
  assert.match(source, /\[executeQuery, queryKey\]/);
  assert.doesNotMatch(source, /\[executeQuery, profileKey, queryInput\]/);
});
