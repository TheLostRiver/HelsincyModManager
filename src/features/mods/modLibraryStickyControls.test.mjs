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
  assert.match(
    source,
    /<CompactActionPanel selectedCount={selectedCount} totalCount={visibleItems\.length} onAction={handleAction} \/>/,
  );

  const toolbarIndex = source.indexOf("mod-library__toolbar-slot");
  const actionsIndex = source.indexOf("mod-library__actions-slot");
  const gridIndex = source.indexOf("mod-library__content");

  assert.ok(toolbarIndex > -1, "toolbar slot should exist");
  assert.ok(actionsIndex > -1, "actions slot should exist");
  assert.ok(gridIndex > -1, "content area should exist");
  // sticky-controls 在 content 之外（mod-library 第1行），故 slot 先于 content 出现。
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

test("sticky controls are an opaque single-column bar fixed above the scroll container", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  // 滚动容器已下沉到 .mod-library__content：它是 overflow-y:auto 的滚动容器，
  // 高度由路由作用域闭合的高度链约束，滚动条只出现在卡片区域，不达到状态栏高度。
  assert.match(css, /\.mod-library__content\s*{[\s\S]*?overflow-y:\s*auto;/);
  assert.match(
    css,
    /\.route-transition__layer\[data-route-id="mods"\][\s\S]*?grid-template-rows:\s*minmax\(0,\s*1fr\);/,
  );
  // 吸顶条处于滚动容器(content)之外的 mod-library 第1行，祖先链都不滚，故无需 sticky——
  // 物理固定常驻视野，滚动条(content 的)从其正下方开始，绝不达搜索栏。
  const stickyControlsBody = getRuleBody(css, ".mod-library__sticky-controls");
  assert.doesNotMatch(stickyControlsBody, /position:\s*sticky;/);
  assert.match(stickyControlsBody, /grid-row:\s*1;/);
  assert.match(getRuleBody(css, ".mod-library__content-shell"), /grid-row:\s*2;/);
  assert.match(getRuleBody(css, ".mod-library__content"), /overflow-y:\s*auto;/);
  // 单列垂直堆叠：搜索栏独占上行，操作区在下行，杜绝操作按钮贴在搜索框右侧被误当成搜索按钮。
  assert.match(
    css,
    /\.mod-library__sticky-controls\s*{[\s\S]*?grid-template-columns:\s*minmax\(0,\s*1fr\);/,
  );
  // 实体不透明条：自带背景/边框，杜绝卡片从缝隙透出。
  assert.match(css, /\.mod-library__sticky-controls\s*{[\s\S]*?background:\s*var\(--color-surface\);/);
  assert.match(css, /\.mod-library__sticky-controls\s*{[\s\S]*?border:\s*1px\s+solid\s+var\(--color-border-muted\);/);
  assert.doesNotMatch(css, /\.mod-library__sticky-controls\s*{[\s\S]*?display:\s*contents;/);
  // 内部 slot 退为 static。
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

test("narrow screens keep the mod list usable beneath tall controls", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(
    css,
    /@media\s*\(max-width:\s*860px\)\s*{[\s\S]*?\.app-surface:has\(\.route-transition__layer\[data-route-id="mods"\]\)\s*{[\s\S]*?overflow-y:\s*auto;/,
  );
  assert.match(
    css,
    /@media\s*\(max-width:\s*860px\)\s*{[\s\S]*?\.app-surface:has\(\.route-transition__layer\[data-route-id="mods"\]\)\s*{[\s\S]*?scrollbar-width:\s*none;/,
  );
  assert.match(
    css,
    /@media\s*\(max-width:\s*860px\)\s*{[\s\S]*?\.mod-library\s*{[\s\S]*?grid-template-rows:\s*auto\s+minmax\(320px,\s*1fr\);/,
  );
});

test("quick action panel no longer owns sticky positioning directly", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");
  const compactPanelBody = getRuleBody(css, ".compact-panel");

  assert.doesNotMatch(compactPanelBody, /position:\s*sticky;/);
  assert.match(compactPanelBody, /min-width:\s*0;/);
});

test("tech view mod cards are allowed to grow to their full data panel height", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");
  const contentBody = getRuleBody(css, ".mod-library__content");
  const techCardBody = getRuleBody(css, ".mod-grid.view-tech .mod-card");

  // The scroll container must not be a one-row grid. Otherwise the nested
  // mod-grid is stretched to the viewport and many tech rows are compressed.
  assert.doesNotMatch(contentBody, /display:\s*grid;/);
  assert.doesNotMatch(contentBody, /grid-template-rows:\s*minmax\(0,\s*1fr\);/);

  // The tech card keeps the demo's hard-edged shell, but its block size must
  // come from the data panel content instead of being clipped to a thin row.
  assert.match(techCardBody, /min-height:\s*max-content;/);
  assert.match(techCardBody, /overflow:\s*hidden;/);
});

test("tech view selection styling overrides the generic blue filled card state", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");
  const techHoverBody = getRuleBody(css, ".mod-grid.view-tech .mod-card:hover:not(.is-selected)");
  const techSelectedBody = getRuleBody(css, ".mod-grid.view-tech .mod-card.is-selected");

  assert.match(techHoverBody, /border-color:\s*var\(--color-accent-alpha-40\);/);
  assert.match(techHoverBody, /box-shadow:\s*5px\s+5px\s+0px\s+var\(--color-accent-alpha-20\);/);
  assert.doesNotMatch(techHoverBody, /box-shadow:\s*6px\s+6px\s+0px\s+var\(--color-accent\);/);
  assert.match(techSelectedBody, /background:\s*var\(--color-surface\);/);
  assert.match(techSelectedBody, /border-color:\s*var\(--color-accent\);/);
  assert.match(techSelectedBody, /box-shadow:\s*6px\s+6px\s+0px\s+var\(--color-accent\);/);
  assert.doesNotMatch(css, /--color-primary\)/);
});

test("mod library starts with no selected mod cards", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /const \[selectedIds,\s*setSelectedIds\] = useState<Set<string>>\(new Set\(\)\);/);
  assert.doesNotMatch(source, /useState<Set<string>>\(new Set\(\[/);
});
