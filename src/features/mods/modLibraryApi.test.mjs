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

test("mod library page loads backend data with mock fallback", () => {
  const source = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /getModLibrary/);
  assert.match(source, /fallbackModLibraryItems/);
  assert.match(source, /setLibraryItems/);
  assert.doesNotMatch(source, /const visibleItems = useMemo\(\(\) => \{\s*const keyword[\s\S]*?return modLibraryItems\.filter/);
});
