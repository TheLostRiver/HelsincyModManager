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
  assert.match(provider, /publishModLibraryTaskProgress\(payload\)/);
  assert.doesNotMatch(provider, /useState/);
  assert.match(provider, /unlisten\?\.\(\)/);
});

test("the library consumes the shared cache and versioned category snapshot", () => {
  assert.match(page, /cache: librarySessionCache/);
  assert.match(page, /librarySessionCache\.readCategories\(\)/);
  assert.match(page, /librarySessionCache\.writeCategories\(loadedCategories, cacheGeneration\)/);
});

test("write ownership is registered by API starts rather than page render effects", () => {
  assert.doesNotMatch(page, /const libraryWriteInFlight/);
  const api = readFileSync("src/features/mods/modInstallPlanApi.ts", "utf8");
  assert.match(api, /trackModLibraryTaskStart\(input, \(\) => invoke<TaskStartedDto>\("start_install_task"/);
  assert.match(api, /trackModLibraryTaskStart\(input, \(\) => invoke<TaskStartedDto>\("start_uninstall_task"/);
  assert.match(provider, /attachModLibraryWriteTracking\(value\)/);
});

test("only invalidation subscriptions drive cache-related query effects", () => {
  assert.match(hook, /useSyncExternalStore\(/);
  assert.match(hook, /useLayoutEffect\(\(\) => \{\s*cacheRef\.current = cache;\s*\}, \[cache\]\)/);
  assert.doesNotMatch(hook, /\n {2}cacheRef\.current = cache;/);
  assert.match(hook, /\[cacheGeneration, executeQuery, loadPage, queryKey\]/);
});
