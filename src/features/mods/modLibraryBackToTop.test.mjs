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

test("mod library page renders back-to-top from scroll UI state instead of unconditionally", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /BackToTopButton/);
  assert.match(source, /showScrollUi\s*\?/);
  assert.match(source, /mod-library__main-floating-actions/);
  assert.match(source, /showScrollUi\s*\?\s*\([\s\S]*?mod-library__main-floating-actions[\s\S]*?<BackToTopButton/);
});

test("scroll UI hides native scrollbar visuals and uses a custom state-driven scrollbar", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(source, /mod-library__content-shell/);
  assert.match(source, /mod-library__scrollbar/);
  assert.match(source, /mod-library__scrollbar-thumb/);
  assert.match(source, /thumbStyle/);
  assert.match(css, /\.mod-library__content[\s\S]*?scrollbar-width:\s*none;/);
  assert.match(css, /\.mod-library__content::-webkit-scrollbar\s*{[\s\S]*?width:\s*0;/);
  assert.match(css, /\.mod-library__scrollbar\s*{[\s\S]*?position:\s*absolute;/);
  assert.match(css, /\.mod-library__scrollbar-thumb\s*{[\s\S]*?transform:\s*translateY/);
});

test("back-to-top button keeps the requested comfortable bottom offset when visible", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  // 距底部 100px：不贴太近右下角，方便点击（用户明确要求）。
  assert.match(css, /\.mod-library\s*{[\s\S]*?--mod-library-back-to-top-block-offset:\s*100px;/);
  assert.match(
    css,
    /\.mod-library__main-floating-actions[\s\S]*?bottom:\s*var\(--mod-library-back-to-top-block-offset\);/,
  );
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
