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

test("back-to-top control stays inside the mod library main column instead of pinning to the viewport edge", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(source, /mod-library__main-floating-actions/);
  assert.match(css, /\.mod-library__main-floating-actions[\s\S]*?position:\s*sticky;/);
  assert.match(css, /\.mod-library__main-floating-actions[\s\S]*?justify-content:\s*end;/);
  // 滚动容器下沉到 .mod-library__content 后，按钮用 sticky bottom 贴 content 底部，
  // 不再用基于 100dvh 的 top 计算（那会把按钮推到 content 可视区外，因 content 顶部在状态栏下方）。
  assert.match(
    css,
    /\.mod-library__main-floating-actions[\s\S]*?bottom:\s*var\(--mod-library-back-to-top-block-offset\);/,
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
