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

/*
 * Mod 库样式分布在页面骨架与卡片两个文件中。断言按合并后的样式表检查，
 * 不绑定规则落在哪个文件，避免后续在两者间搬迁规则时产生假失败。
 * 拼接顺序与实际加载顺序一致：ModPosterCard.css 由卡片组件先加载。
 */
function readModLibraryCss() {
  return [
    readSource("src/features/mods/ModPosterCard.css"),
    readSource("src/features/mods/ModLibraryPage.css"),
  ].join("\n");
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

test("mod poster card displays the same generated version in grid, list, and tech views", () => {
  const source = readSource("src/features/mods/ModPosterCard.tsx");

  assert.match(source, /const versionLabel = item\.versionLabel \?\? "v1\.0\.0"/);
  assert.equal(source.match(/\{versionLabel\}/g)?.length, 3);
  assert.doesNotMatch(source, />v1\.0\.0<\/div>/);
  assert.doesNotMatch(source, /版本:\s*v1\.0\.0/);
});

test("mod poster card retries when thumbnail url changes", () => {
  const source = readSource("src/features/mods/ModPosterCard.tsx");

  assert.match(source, /useEffect/);
  assert.match(source, /setPosterFailed\(false\)/);
  assert.match(source, /\[\s*previewThumbnail\?\.thumbnailUrl\s*\]/);
});

test("mod poster card prefers unsafe recovery issue count over managed file count", () => {
  const source = readSource("src/features/mods/ModPosterCard.tsx");
  const css = readSource("src/features/mods/ModPosterCard.css");

  assert.match(source, /isUnsafeInstallStatus\(item\.status\) && summary/);
  assert.match(source, /committed_cleanup_pending:\s*"重装待收尾"/);
  assert.match(source, /cleanup_pending:\s*"恢复待清理"/);
  assert.match(source, /className="mod-card__status-label"/);
  assert.match(source, /summary\.issueCount && summary\.issueCount > 0/);
  assert.match(source, /summary\.managedFileCount > 0/);
  assert.match(css, /is-committed_cleanup_pending/);
  assert.match(css, /is-cleanup_pending/);
  assert.match(css, /\.mod-card__status-pill\s*{[\s\S]*?max-width:\s*calc\(100% - 24px\);/);
  assert.match(css, /\.mod-card__status-label\s*{[\s\S]*?text-overflow:\s*ellipsis;/);
});

test("mod poster card renders compact category labels with overflow", () => {
  const source = readSource("src/features/mods/ModPosterCard.tsx");
  const css = readModLibraryCss();

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
  const css = readModLibraryCss();

  assert.match(source, /showCategoryLabels\?:\s*boolean/);
  assert.match(source, /showCategoryLabels = true/);
  assert.match(source, /const categorySummary/);
  assert.match(source, /item\.categoryLabels\.map\(\(label\) => label\.name\)\.join\("、"\)/);
  assert.match(source, /categoryLabels\.visible\.length > 0/);
  assert.match(source, /data-visible=\{showCategoryLabels \? "true" : "false"\}/);
  assert.match(source, /aria-hidden="true"/);
  assert.match(source, /aria-label=\{`选择 \$\{item\.name\}\$\{categorySummary\}`\}/);

  assert.match(css, /\.mod-card__categories\s*{[\s\S]*?max-height:\s*22px;/);
  assert.match(css, /\.mod-card__categories\s*{[\s\S]*?transition:[\s\S]*?opacity[\s\S]*?max-height[\s\S]*?transform/);
  assert.match(css, /\.mod-card__categories\[data-visible="false"\]\s*{[\s\S]*?opacity:\s*0;/);
  assert.match(css, /\.mod-card__categories\[data-visible="false"\]\s*{[\s\S]*?max-height:\s*0;/);
  assert.match(css, /\.mod-card__categories\[data-visible="false"\]\s*{[\s\S]*?transform:\s*translateY\(-4px\);/);
});

test("mod poster cards stay focusable but expose and guard busy interaction", () => {
  const source = readSource("src/features/mods/ModPosterCard.tsx");

  assert.match(source, /interactionDisabled\?:\s*boolean/);
  assert.match(source, /interactionDisabled = false/);
  assert.match(source, /tabIndex=\{0\}/);
  assert.match(source, /aria-disabled=\{interactionDisabled \|\| undefined\}/);
  assert.match(source, /onClick=\{\(\) => \{\s*if \(!interactionDisabled\) \{\s*onSelect\(item\.id\);/);
  assert.match(source, /onContextMenu=\{\(e\) => \{[\s\S]*?if \(interactionDisabled\) \{\s*return;/);
  assert.match(source, /onKeyDown=\{\(e\) => \{[\s\S]*?if \(interactionDisabled\) \{\s*return;/);
});

test("mod poster image fills stable poster frame", () => {
  const css = readModLibraryCss();
  const body = getRuleBody(css, ".mod-card__poster-img");

  // .mod-card__status-pill 有两条规则（定位一条、文本截断一条），getRuleBody 只取首个匹配，
  // 因此这里直接按"定位规则内含 z-index"匹配，不依赖两条规则的先后顺序。
  assert.match(css, /\.mod-card__status-pill\s*\{[^}]*position:\s*absolute;[^}]*z-index:\s*2;/);

  assert.match(body, /position:\s*absolute;/);
  assert.match(body, /inset:\s*0;/);
  assert.match(body, /width:\s*100%;/);
  assert.match(body, /height:\s*100%;/);
  assert.match(body, /object-fit:\s*cover;/);
  assert.match(body, /object-position:\s*center top;/);
});
