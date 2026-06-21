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

test("returns app-surface as the preferred back-to-top scroll target", () => {
  const appSurface = { scrollTo() {} };
  const fallbackTarget = { scrollTo() {} };
  const documentLike = {
    querySelector(selector) {
      return selector === ".app-surface" ? appSurface : null;
    },
  };

  assert.equal(getModLibraryBackToTopTarget(documentLike, fallbackTarget), appSurface);
});

test("falls back when app-surface scroll target is unavailable", () => {
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

test("back-to-top control stays inside the mod library main column instead of pinning to the viewport edge", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(source, /mod-library__main-floating-actions/);
  assert.match(css, /\.mod-library__main-floating-actions[\s\S]*?position:\s*sticky;/);
  assert.match(css, /\.mod-library__main-floating-actions[\s\S]*?justify-content:\s*end;/);
  assert.match(
    css,
    /\.mod-library__main-floating-actions[\s\S]*?top:\s*calc\(100dvh\s*-\s*var\(--layout-page-padding\)\s*-\s*var\(--mod-library-back-to-top-size\)\s*-\s*var\(--mod-library-back-to-top-block-offset\)\);/,
  );
  assert.doesNotMatch(css, /\.mod-library__back-to-top[\s\S]*?position:\s*fixed;/);
  assert.match(css, /\.mod-library__back-to-top[\s\S]*?pointer-events:\s*auto;/);
});

test("back-to-top button stays inside its container so it never triggers horizontal scrolling", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  // 上下偏移变量保留（控制距视口底部的距离）。
  assert.match(css, /\.mod-library\s*{[\s\S]*?--mod-library-back-to-top-block-offset:\s*12px;/);
  // 关键根因保护：浮动层不得再用 translateX 向外平移——那会把按钮推出容器右边界，
  // 触发 .app-surface 水平滚动条，并截断按钮自身。inline-offset 变量已彻底移除。
  assert.doesNotMatch(css, /\.mod-library__main-floating-actions[\s\S]*?transform:\s*translateX/);
  assert.doesNotMatch(css, /--mod-library-back-to-top-inline-offset/);
  // 640px 仍保留更紧凑的底部偏移。
  assert.match(
    css,
    /@media\s*\(max-width:\s*640px\)\s*{[\s\S]*?\.mod-library\s*{[\s\S]*?--mod-library-back-to-top-block-offset:\s*8px;/,
  );
});

test("compact action panel does not absorb the back-to-top action", () => {
  const source = readProjectFile("src/features/mods/CompactActionPanel.tsx");

  assert.doesNotMatch(source, /返回顶部|back-to-top/i);
});
