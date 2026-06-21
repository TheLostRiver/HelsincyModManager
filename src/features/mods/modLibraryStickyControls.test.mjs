import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "../../..");

function readProjectFile(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

function getRuleBody(css, selector) {
  const start = css.indexOf(`${selector} {`);
  assert.ok(start >= 0, `missing CSS rule: ${selector}`);

  const openBraceIndex = css.indexOf("{", start);
  const closeBraceIndex = css.indexOf("}", openBraceIndex);
  assert.ok(openBraceIndex >= 0 && closeBraceIndex > openBraceIndex, `invalid CSS rule: ${selector}`);
  return css.slice(openBraceIndex + 1, closeBraceIndex);
}

test("ModLibraryPage groups toolbar and quick actions into one sticky controls area", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");

  // sticky-controls 整体承载入场动画；内部 slot 不再各自动画。
  assert.match(source, /className="mod-library__sticky-controls anim-stagger-item"/);
  assert.match(source, /className="mod-library__toolbar-slot"/);
  assert.match(source, /className="mod-library__actions-slot"/);
  assert.match(source, /<LibraryToolbar[\s\S]*?query={query}[\s\S]*?activeFilter={activeFilter}/);
  assert.match(source, /<CompactActionPanel selectedCount={selectedCount} onAction={handleAction} \/>/);

  const toolbarIndex = source.indexOf("mod-library__toolbar-slot");
  const actionsIndex = source.indexOf("mod-library__actions-slot");
  const gridIndex = source.indexOf("mod-library__content");

  assert.ok(toolbarIndex > -1, "toolbar slot should exist");
  assert.ok(actionsIndex > -1, "actions slot should exist");
  assert.ok(gridIndex > -1, "content area should exist");
  assert.ok(toolbarIndex < gridIndex, "toolbar slot should render before content");
  assert.ok(actionsIndex < gridIndex, "actions slot should render before content");
});

test("global status bar stays pinned so the sticky controls can sit beneath it", () => {
  const css = readProjectFile("src/app/frame/AppFrame.css");

  assert.match(css, /\.top-status-bar\s*{[\s\S]*?position:\s*sticky;/);
  assert.match(css, /\.top-status-bar\s*{[\s\S]*?top:\s*0;/);
  // header 高度 token 是状态栏与吸顶条的对齐基准，必须在 tokens 中存在。
  const tokens = readProjectFile("src/shared/styles/tokens.css");
  assert.match(tokens, /--app-header-height:\s*64px;/);
});

test("sticky controls are an opaque single-column bar that sits beneath the status bar", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  // 吸顶条 top 紧贴状态栏下方，不再依赖 page-padding。
  assert.match(
    css,
    /\.mod-library\s*{[\s\S]*?--mod-library-sticky-top:\s*calc\(var\(--app-header-height\)\s*\+\s*4px\);/,
  );
  // 单列垂直堆叠：搜索栏独占上行，操作区在下行，杜绝操作按钮贴在搜索框右侧被误当成搜索按钮。
  assert.match(
    css,
    /\.mod-library__sticky-controls\s*{[\s\S]*?grid-template-columns:\s*minmax\(0,\s*1fr\);/,
  );
  // 实体不透明条：sticky + 自带背景/边框，杜绝卡片从缝隙透出。
  assert.match(css, /\.mod-library__sticky-controls\s*{[\s\S]*?position:\s*sticky;/);
  assert.match(css, /\.mod-library__sticky-controls\s*{[\s\S]*?background:\s*var\(--color-surface\);/);
  assert.match(css, /\.mod-library__sticky-controls\s*{[\s\S]*?border:\s*1px\s+solid\s+var\(--color-border-muted\);/);
  assert.doesNotMatch(css, /\.mod-library__sticky-controls\s*{[\s\S]*?display:\s*contents;/);
  // sticky 归属上移到吸顶条，内部 slot 退为 static。
  assert.match(
    css,
    /\.mod-library__toolbar-slot,[\s\S]*?\.mod-library__actions-slot\s*{[\s\S]*?position:\s*static;/,
  );
  // 操作区独占第二行且带分隔线，与搜索栏视觉区隔。
  assert.match(
    css,
    /\.mod-library__actions-slot\s*{[\s\S]*?border-top:\s*1px\s+solid\s+var\(--color-border-muted\);/,
  );
  assert.doesNotMatch(css, /\.library-toolbar[\s\S]*?position:\s*fixed;/);
  assert.doesNotMatch(css, /\.compact-panel[\s\S]*?position:\s*fixed;/);
});

test("quick actions wrap instead of horizontally scrolling, so no ugly scrollbar or layout overflow", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  // 按钮容器换行，不再强制单行横排；移除横向滚动条，杜绝水平溢出截断返回顶部按钮。
  assert.match(css, /\.compact-panel__stack\s*{[\s\S]*?flex-wrap:\s*wrap;/);
  assert.doesNotMatch(css, /\.compact-panel__stack\s*{[\s\S]*?overflow-x:\s*auto;/);
  assert.doesNotMatch(css, /@media\s*\(max-width:\s*1280px\)\s*{[\s\S]*?\.compact-panel__stack\s*{[\s\S]*?overflow-x:\s*auto;/);
});

test("quick action panel no longer owns sticky positioning directly", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");
  const compactPanelBody = getRuleBody(css, ".compact-panel");

  assert.doesNotMatch(compactPanelBody, /position:\s*sticky;/);
  assert.match(compactPanelBody, /min-width:\s*0;/);
});
