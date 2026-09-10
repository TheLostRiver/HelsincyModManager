// These assertions cover structure only. Runtime behavior is exercised by modLibrarySessionBehavior.test.mjs.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const app = readFileSync("src/App.tsx", "utf8");
const hook = readFileSync("src/features/mods/useModLibraryQuery.ts", "utf8");
const page = readFileSync("src/features/mods/ModLibraryPage.tsx", "utf8");
const provider = readFileSync("src/features/mods/ModLibrarySessionCacheProvider.tsx", "utf8");

test("cache provider encloses both the router and window-wide import queue", () => {
  const start = app.indexOf("<ModLibrarySessionCacheProvider>");
  const end = app.indexOf("</ModLibrarySessionCacheProvider>");
  for (const marker of ["<RouterOutlet />", "<ModImportDropProvider>"]) {
    const child = app.indexOf(marker);
    assert.ok(start >= 0 && child > start && end > child);
  }
});

test("the stable cache store and task subscription belong to the long-lived provider", () => {
  assert.match(provider, /useRef<ModLibrarySessionStore \| null>/);
  assert.match(provider, /value\.observeTask\(payload\)/);
  assert.doesNotMatch(provider, /useState/);
  assert.match(provider, /unlisten\?\.\(\)/);
});

test("the library consumes the shared cache and versioned category snapshot", () => {
  assert.match(page, /cache: librarySessionCache/);
  assert.match(page, /librarySessionCache\.readCategories\(\)/);
  assert.match(page, /librarySessionCache\.writeCategories\(loadedCategories, cacheGeneration\)/);
});

test("known write starts invalidate pages without treating read previews as writes", () => {
  const start = page.indexOf("const libraryWriteInFlight =");
  const end = page.indexOf("if (!libraryWriteInFlight) return;");
  assert.ok(start >= 0 && end > start);
  const predicate = page.slice(start, end);
  for (const fact of ["managedInstallTaskActive", "reinstallWorkflow.taskActive", "deletionBusy", 'batchWorkflow.state.status === "starting"']) {
    assert.ok(predicate.includes(fact));
  }
  assert.doesNotMatch(predicate, /workflowActive|status !== "idle"/);
});

test("only invalidation subscriptions drive cache-related query effects", () => {
  assert.match(hook, /useSyncExternalStore\(/);
  assert.match(hook, /useLayoutEffect\(\(\) => \{\s*cacheRef\.current = cache;\s*\}, \[cache\]\)/);
  assert.doesNotMatch(hook, /\n {2}cacheRef\.current = cache;/);
  assert.match(hook, /\[cacheGeneration, executeQuery, loadPage, queryKey\]/);
});
