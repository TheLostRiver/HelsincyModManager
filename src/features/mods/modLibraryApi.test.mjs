import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("mod library API invokes controlled query commands", () => {
  const source = readSource("src/features/mods/modLibraryApi.ts");

  assert.match(source, /invoke<ModLibraryItem\[]>\("get_mod_library"/);
  assert.match(source, /invoke<ModLibraryPage>\("query_mod_library"/);
  assert.match(source, /request:\s*\{/);
  assert.match(source, /profileContext:/);
  assert.match(source, /gameId:\s*input\.profileContext\.gameId/);
  assert.match(source, /profileId:\s*input\.profileContext\.profileId/);
  assert.match(source, /search:\s*input\.search/);
  assert.match(source, /filter:\s*input\.filter/);
  assert.match(source, /sort:\s*input\.sort/);
  assert.match(source, /page:\s*input\.page/);
  assert.match(source, /pageSize:\s*input\.pageSize/);
  assert.match(source, /invoke<ModDetail\s*\|\s*null>\("get_mod_detail"/);
  assert.match(source, /modId:\s*input\.modId/);
  assert.doesNotMatch(source, /convertFileSrc|asset:|archivePath|sandbox|cache|rawPath/i);
});

test("mod library types expose preview image without local paths", () => {
  const source = readSource("src/features/mods/modLibraryTypes.ts");

  assert.match(source, /previewImage\??:\s*PreviewImage/);
  assert.match(source, /type QueryModLibraryInput/);
  assert.match(source, /pageSize:\s*12\s*\|\s*24\s*\|\s*48\s*\|\s*96/);
  assert.match(source, /type ModLibraryPage/);
  assert.match(source, /matchingTotal:\s*number/);
  assert.match(source, /from "\.\/modPreviewImageTypes"/);
  assert.doesNotMatch(source, /cachePath|diskPath|localPath|convertFileSrc|asset:/i);
});

test("mod library page consumes paged queries and limits mock data to plain-browser development", () => {
  const source = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /isPlainBrowserDevRuntime/);
  assert.match(source, /const browserPreviewEnabled = useMemo/);
  assert.match(source, /isDev:\s*\(import\.meta[\s\S]*?\.env\?\.DEV\s*===\s*true/);
  assert.match(source, /hasTauriRuntime:\s*hasTauriRuntime\(\)/);
  assert.match(
    source,
    /const page = browserPreviewEnabled\s*\?\s*queryBrowserMockModLibrary\(input,\s*fallbackModLibraryItems,\s*categoriesRef\.current\)\s*:\s*await queryModLibrary\(input\);/,
  );
  assert.match(source, /if \(browserPreviewEnabled \|\| input\.profileContext === undefined\) \{\s*return page;/);
  assert.match(source, /const libraryQuery = useModLibraryQuery\(\{[\s\S]*?loadPage:\s*loadModLibraryPage/);
  assert.match(source, /const libraryPage = libraryQuery\.page/);
  assert.doesNotMatch(source, /\bgetModLibrary\b|setLibraryItems/);
  assert.doesNotMatch(
    source,
    /const visibleItems = useMemo\(\(\) => \{\s*const keyword[\s\S]*?return modLibraryItems\.filter/,
  );
});

test("mod library write completions use a dedicated refresh that clears selection and returns to the top", () => {
  const source = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(
    source,
    /const refreshModLibraryAfterWrite = useCallback\(async \(\) => \{\s*resetContentScroll\(\);\s*await refreshModLibrary\(\);/,
  );
  assert.match(source, /refreshLibrary:\s*refreshModLibraryAfterWrite/);
  assert.match(source, /onImportCompleted=\{refreshModLibraryAfterWrite\}/);
  assert.match(source, /onSaved=\{refreshModLibraryAfterWrite\}/);
  assert.match(source, /Promise\.allSettled\(\[\s*refreshModLibraryAfterWrite\(\)/);
  assert.match(source, /case "refresh":[\s\S]*?refreshModLibrary\(\)/);
});
