import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { getModLibraryBackToTopTarget, scrollModLibraryBackToTop } from "./modLibraryBackToTop.ts";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "../../..");

function readProjectFile(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

test("returns mod-library__content as the preferred back-to-top scroll target", () => {
  const modLibraryContent = { scrollTo() {} };
  const fallbackTarget = { scrollTo() {} };
  const documentLike = {
    querySelector(selector) {
      return selector === ".mod-library__content" ? modLibraryContent : null;
    },
  };

  assert.equal(getModLibraryBackToTopTarget(documentLike, fallbackTarget), modLibraryContent);
});

test("falls back when mod-library__content scroll target is unavailable", () => {
  const fallbackTarget = { scrollTo() {} };
  const documentLike = {
    querySelector() {
      return null;
    },
  };

  assert.equal(getModLibraryBackToTopTarget(documentLike, fallbackTarget), fallbackTarget);
});

test("scroll helper always requests smooth scroll-to-top", () => {
  let receivedOptions = null;
  const target = {
    scrollTo(options) {
      receivedOptions = options;
    },
  };

  scrollModLibraryBackToTop(target);

  assert.deepEqual(receivedOptions, {
    top: 0,
    behavior: "smooth",
  });
});

test("mod library page renders a dedicated back-to-top button", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /BackToTopButton/);
  assert.match(source, /mod-library__main-floating-actions/);
});

test("back-to-top control is fixed to the viewport bottom-right so it stays visible while scrolling", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(source, /mod-library__main-floating-actions/);
  // 返回顶部用 position:fixed 相对视口固定：滚动卡片时不消失、不随卡片移动，
  // 始终悬浮在右下角。这是浮动按钮的标准实现。
  assert.match(css, /\.mod-library__main-floating-actions[\s\S]*?position:\s*fixed;/);
  assert.match(css, /\.mod-library__main-floating-actions[\s\S]*?justify-content:\s*end;/);
  // 距右边缘对齐 page-padding，距底部由 block-offset 控制（默认 100px，不贴太近方便点击）。
  assert.match(css, /\.mod-library__main-floating-actions[\s\S]*?right:\s*var\(--layout-page-padding\);/);
  assert.match(
    css,
    /\.mod-library__main-floating-actions[\s\S]*?bottom:\s*var\(--mod-library-back-to-top-block-offset\);/,
  );
  assert.match(css, /\.mod-library__back-to-top[\s\S]*?pointer-events:\s*auto;/);
});

test("back-to-top button offset keeps it clear of the corner for easy clicking", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  // 距底部 100px：不贴太近右下角，方便点击（用户明确要求）。
  assert.match(css, /\.mod-library\s*{[\s\S]*?--mod-library-back-to-top-block-offset:\s*100px;/);
  // 关键根因保护：浮动层不得用 translateX 向外平移（会触发水平滚动并截断按钮）。
  assert.doesNotMatch(css, /\.mod-library__main-floating-actions[\s\S]*?transform:\s*translateX/);
  assert.doesNotMatch(css, /--mod-library-back-to-top-inline-offset/);
  // 640px 小屏缩小底部偏移但仍保持点击舒适距离。
  assert.match(
    css,
    /@media\s*\(max-width:\s*640px\)\s*{[\s\S]*?\.mod-library\s*{[\s\S]*?--mod-library-back-to-top-block-offset:\s*80px;/,
  );
});

test("compact action panel does not absorb the back-to-top action", () => {
  const source = readProjectFile("src/features/mods/CompactActionPanel.tsx");

  assert.doesNotMatch(source, /返回顶部|back-to-top/i);
});
