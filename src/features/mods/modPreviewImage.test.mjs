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

test("mod poster card prefers unsafe recovery issue count over managed file count", () => {
  const source = readSource("src/features/mods/ModPosterCard.tsx");

  assert.match(source, /\(item\.status === "rollback_required" \|\| item\.status === "repair_required"\) && summary\)/);
  assert.match(source, /summary\.issueCount && summary\.issueCount > 0/);
  assert.match(source, /summary\.managedFileCount > 0/);
  assert.doesNotMatch(source, /item\.status === "repair_required" && summary && summary\.managedFileCount > 0/);
});

test("mod poster card renders compact category labels with overflow", () => {
  const source = readSource("src/features/mods/ModPosterCard.tsx");
  const css = readSource("src/features/mods/ModLibraryPage.css");

  assert.match(source, /visibleCategoryLabelsForCard/);
  assert.match(source, /categoryLabelLimit = isList \|\| isTech \? 3 : 2/);
  assert.match(source, /className="mod-card__categories"/);
  assert.match(source, /className="mod-card__category-dot"/);
  assert.match(source, /className="mod-card__category-overflow"/);
  assert.match(css, /\.mod-card__categories\s*{[\s\S]*?overflow:\s*hidden;/);
  assert.match(css, /\.mod-card__category-name\s*{[\s\S]*?text-overflow:\s*ellipsis;/);
});

test("mod poster card can hide category labels without affecting card rendering", () => {
  const source = readSource("src/features/mods/ModPosterCard.tsx");
  const css = readSource("src/features/mods/ModLibraryPage.css");

  assert.match(source, /showCategoryLabels\?:\s*boolean/);
  assert.match(source, /showCategoryLabels = true/);
  assert.match(source, /categoryLabels\.visible\.length > 0/);
  assert.match(source, /data-visible=\{showCategoryLabels \? "true" : "false"\}/);
  assert.match(source, /aria-hidden=\{!showCategoryLabels\}/);

  assert.match(css, /\.mod-card__categories\s*{[\s\S]*?max-height:\s*22px;/);
  assert.match(css, /\.mod-card__categories\s*{[\s\S]*?transition:[\s\S]*?opacity[\s\S]*?max-height[\s\S]*?transform/);
  assert.match(css, /\.mod-card__categories\[data-visible="false"\]\s*{[\s\S]*?opacity:\s*0;/);
  assert.match(css, /\.mod-card__categories\[data-visible="false"\]\s*{[\s\S]*?max-height:\s*0;/);
  assert.match(css, /\.mod-card__categories\[data-visible="false"\]\s*{[\s\S]*?transform:\s*translateY\(-4px\);/);
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
