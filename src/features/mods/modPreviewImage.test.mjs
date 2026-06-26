import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "../../..");

function readSource(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

function getRuleBody(css, selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = css.match(new RegExp(`(?:^|\\n)${escaped}\\s*\\{([\\s\\S]*?)\\n\\}`));
  assert.ok(match, `Expected CSS rule for ${selector}`);
  return match[1];
}

test("mod preview image type matches backend DTO shape", () => {
  const types = readSource("src/features/mods/modPreviewImageTypes.ts");

  assert.match(types, /kind:\s*"thumbnail"/);
  assert.match(types, /thumbnailUrl:\s*string/);
  assert.match(types, /contentHash:\s*string/);
  assert.match(types, /"pixel_limit_exceeded"/);
  assert.match(types, /"cache_write_failed"/);
});

test("mod poster card renders controlled lazy thumbnails with fallback", () => {
  const source = readSource("src/features/mods/ModPosterCard.tsx");

  assert.match(source, /className="mod-card__poster-img"/);
  assert.match(source, /loading="lazy"/);
  assert.match(source, /decoding="async"/);
  assert.match(source, /onError=\{\(\)\s*=>\s*setPosterFailed\(true\)\}/);
  assert.match(source, /item\.previewImage\?\.kind === "thumbnail"/);
});

test("mod poster card retries when thumbnail url changes", () => {
  const source = readSource("src/features/mods/ModPosterCard.tsx");

  assert.match(source, /useEffect/);
  assert.match(source, /setPosterFailed\(false\)/);
  assert.match(source, /\[\s*previewThumbnail\?\.thumbnailUrl\s*\]/);
});

test("mod poster card prefers repair issue count over managed file count", () => {
  const source = readSource("src/features/mods/ModPosterCard.tsx");

  assert.match(source, /item\.status === "repair_required" && summary\)/);
  assert.match(source, /summary\.issueCount && summary\.issueCount > 0/);
  assert.match(source, /summary\.managedFileCount > 0/);
  assert.doesNotMatch(source, /item\.status === "repair_required" && summary && summary\.managedFileCount > 0/);
});

test("mod poster image fills stable poster frame", () => {
  const css = readSource("src/features/mods/ModLibraryPage.css");
  const body = getRuleBody(css, ".mod-card__poster-img");
  const statusBody = getRuleBody(css, ".mod-card__status-pill");

  assert.match(body, /position:\s*absolute;/);
  assert.match(body, /inset:\s*0;/);
  assert.match(body, /width:\s*100%;/);
  assert.match(body, /height:\s*100%;/);
  assert.match(body, /object-fit:\s*cover;/);
  assert.match(body, /object-position:\s*center top;/);
  assert.match(statusBody, /z-index:\s*2;/);
});
