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

/*
 * Mod 库样式分布在页面骨架与卡片两个文件中。断言按合并后的样式表检查，
 * 不绑定规则落在哪个文件，避免后续在两者间搬迁规则时产生假失败。
 * 拼接顺序与实际加载顺序一致：ModPosterCard.css 由卡片组件先加载。
 */
function readModLibraryCss() {
  const cardCss = readProjectFile("src/features/mods/ModPosterCard.css");
  const pageCss = readProjectFile("src/features/mods/ModLibraryPage.css");
  return `${cardCss}\n${pageCss}`;
}

function getRuleBody(css, selector) {
  const start = css.indexOf(`${selector} {`);
  assert.ok(start >= 0, `missing CSS rule: ${selector}`);

  const openBraceIndex = css.indexOf("{", start);
  const closeBraceIndex = css.indexOf("}", openBraceIndex);
  assert.ok(openBraceIndex >= 0 && closeBraceIndex > openBraceIndex, `invalid CSS rule: ${selector}`);
  return css.slice(openBraceIndex + 1, closeBraceIndex);
}

test("ModLibraryPage groups controls while separating global and page-local selection counts", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");

  // sticky-controls 整体承载入场动画；内部 slot 不再各自动画。
  assert.match(source, /className="mod-library__sticky-controls anim-stagger-item"/);
  assert.match(source, /className="mod-library__toolbar-slot"/);
  assert.match(source, /className="mod-library__actions-slot"/);
  assert.match(source, /<LibraryToolbar[\s\S]*?query={query}[\s\S]*?activeFilter={activeFilter}/);
  assert.match(source, /<CompactActionPanel[\s\S]*?selectedCount={selectedCount}/);
  assert.match(source, /<CompactActionPanel[\s\S]*?selectionMode={selectionMode}/);
  assert.match(source, /<CompactActionPanel[\s\S]*?selectedPageCount={selectedPageCount}/);
  assert.match(source, /<CompactActionPanel[\s\S]*?pageCount={libraryItems\.length}/);
  assert.match(source, /<CompactActionPanel[\s\S]*?installTaskActive={installTaskActive}/);
  assert.match(source, /<CompactActionPanel[\s\S]*?libraryQueryBusy={libraryQueryBusy}/);
  assert.match(
    source,
    /<CompactActionPanel[\s\S]*?profileReady=\{activeProfile\.status === "ready" && activeProfileId !== null\}/,
  );
  assert.match(source, /<CompactActionPanel[\s\S]*?onAction={handleAction}/);
  assert.match(
    source,
    /const selectAll = \(\) => \{\s*if \(selectionInteractionLocked\) \{\s*return;\s*\}\s*if \(selectionMode === "single"\) \{\s*dispatchSelection\(\{ type: "enter-batch" \}\);\s*\}[\s\S]*?dispatchSelection\(\{ type: "select-page"/,
  );
  assert.match(source, /const invertSelection = \(\) => \{\s*if \(selectionInteractionLocked\) \{\s*return;\s*\}\s*if \(selectionMode === "single"\) \{\s*dispatchSelection\(\{ type: "enter-batch" \}\);\s*\}[\s\S]*?dispatchSelection\(\{ type: "invert-page"/);
  assert.match(source, /const handlePageChange = \(nextPage: number\) => \{[\s\S]*?resetContentScroll\(\);/);
  assert.match(source, /const handlePageSizeChange = \(nextPageSize:[\s\S]*?resetContentScroll\(\);/);

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

test("query refresh fails closed for stale page interactions and clears landed-page selection", () => {
  const page = readProjectFile("src/features/mods/ModLibraryPage.tsx");
  const panel = readProjectFile("src/features/mods/CompactActionPanel.tsx");

  assert.match(page, /const selectCard = \(intent: ModCardSelectionIntent\) => \{\s*if \(selectionInteractionLocked\) \{\s*return;/);
  assert.match(page, /const handleContextMenu = \(modId: string, x: number, y: number\) => \{\s*if \(libraryQueryBusy\) \{\s*return;/);
  assert.match(page, /selectionMode !== "single"\s*\|\| selectedIds\.size !== 1/);
  assert.match(page, /const promptSelectedUninstallTask = \(\) => \{\s*if \(\s*libraryQueryBusy \|\|/);
  assert.match(page, /case "reinstall":\s*if \(libraryQueryBusy\) \{\s*break;/);
  assert.match(page, /const handleContextMenuAction = \(actionId: string, modId: string\) => \{\s*if \(libraryQueryBusy\) \{\s*return;/);
  // Cross-page selection: the library-page effect no longer clears selections; refresh and
  // query/filter changes own that responsibility (refreshModLibrary + resetPageInteraction).
  assert.match(page, /useEffect\(\(\) => \{\s*setContextMenuState\(null\);\s*\}, \[libraryPage\]\);/);
  assert.match(page, /const refreshModLibrary = useCallback\(async \(\) => \{[\s\S]*?dispatchSelection\(\{ type: "reset-context", reason: "Mod 库已刷新" \}\);/);
  assert.ok((panel.match(/<ModLibraryControlTooltip/g) ?? []).length >= 3);
  assert.ok((panel.match(/aria-describedby=\{descriptionId\}/g) ?? []).length >= 3);
  assert.doesNotMatch(panel, /aria-label=\{disabledReason/);
  assert.doesNotMatch(panel, /\sdisabled=\{disabledReason/);
  assert.match(page, /<ModPosterCard[\s\S]*?interactionDisabled=\{selectionInteractionLocked\}/);
  assert.match(
    panel,
    /const revisionImportDisabledReason =\s*libraryQueryBusy\s*\? MOD_LIBRARY_QUERY_BUSY_MESSAGE/,
  );
});

test("automatic filter reconciliation resets page interaction only when the filter key changes", () => {
  const page = readProjectFile("src/features/mods/ModLibraryPage.tsx");

  assert.match(page, /const normalizedFilter = normalizeLibraryFilter\(activeFilter, filterChips\);/);
  assert.match(page, /if \(normalizedFilter === activeFilter\) \{\s*return;/);
  assert.match(page, /if \(!isSameLibraryFilter\(activeFilter, normalizedFilter\)\) \{[\s\S]*?resetLibraryPage\(\);[\s\S]*?resetPageInteraction\("筛选条件已变化"\);/);
  assert.match(page, /setActiveFilter\(normalizedFilter\);/);
});

test("ModLibraryPage persists card category label visibility as a local UI preference", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /CARD_CATEGORY_LABELS_STORAGE_KEY/);
  assert.match(source, /readInitialCardCategoryLabelsVisibility/);
  assert.match(source, /localStorage\.getItem\(CARD_CATEGORY_LABELS_STORAGE_KEY\)/);
  assert.match(source, /localStorage\.setItem\(CARD_CATEGORY_LABELS_STORAGE_KEY,\s*String\(nextValue\)\)/);
  assert.match(source, /showCardCategoryLabels=\{showCardCategoryLabels\}/);
  assert.match(source, /onToggleCardCategoryLabels=\{toggleCardCategoryLabels\}/);
  assert.match(source, /showCategoryLabels=\{showCardCategoryLabels\}/);
});

test("library search submits on Enter without interrupting IME composition", () => {
  const source = readProjectFile("src/features/mods/LibraryToolbar.tsx");

  assert.match(source, /event\.key === "Enter" && !event\.nativeEvent\.isComposing/);
  assert.match(source, /event\.preventDefault\(\);\s*onQuerySubmit\(\);/);
});

test("disabled status chips stay focusable and expose a custom accessible reason", () => {
  const source = readProjectFile("src/features/mods/LibraryToolbar.tsx");
  const css = readModLibraryCss();

  assert.match(source, /<ModLibraryControlTooltip key=\{chip\.key\} content=\{chip\.disabledReason\}>/);
  assert.match(source, /aria-disabled=\{chip\.disabled \|\| undefined\}/);
  assert.match(source, /aria-describedby=\{descriptionId\}/);
  assert.match(source, /if \(chip\.disabled\) \{[\s\S]*?event\.preventDefault\(\);[\s\S]*?return;/);
  assert.doesNotMatch(source, /disabled=\{chip\.disabled\}/);
  assert.doesNotMatch(source, /title=\{chip\.disabledReason\}/);
  assert.match(css, /\.library-chip\[aria-disabled="true"\]/);
  assert.match(css, /\.library-chip:hover:not\(\[aria-disabled="true"\]\)/);
});

test("lifecycle and future batch actions fail closed with focusable custom reasons", () => {
  const source = readProjectFile("src/features/mods/CompactActionPanel.tsx");

  assert.match(source, /getCompactActionDisabledReason\(\{/);
  assert.match(source, /<ModLibraryControlTooltip key=\{action\.id\} content=\{disabledReason\}>/);
  assert.match(source, /aria-disabled=\{disabledReason \? true : undefined\}/);
  assert.match(source, /aria-describedby=\{descriptionId\}/);
  assert.match(source, /if \(disabledReason\) \{[\s\S]*?event\.preventDefault\(\);[\s\S]*?return;/);
  assert.doesNotMatch(source, /disabled=\{isDisabled\}/);
  assert.match(source, /role="status"[\s\S]*?aria-live="polite"[\s\S]*?aria-atomic="true"/);
});

test("compact page-selection tooltips escape the segmented group without losing separators", () => {
  const css = readModLibraryCss();

  assert.match(css, /\.compact-action-group\s*{[\s\S]*?overflow:\s*visible;/);
  assert.match(
    css,
    /\.compact-action-group\s*>\s*\.mod-library-control-tooltip:last-child\s+\.compact-action\s*{[\s\S]*?border-right:\s*none;/,
  );
  assert.match(
    css,
    /\.compact-action-group\s*>\s*\.mod-library-control-tooltip:first-child\s+\.compact-action\s*{[\s\S]*?border-start-start-radius:\s*9999px;/,
  );
  assert.doesNotMatch(css, /\.compact-action-group\s+\.compact-action:last-child/);
});

test("global status bar stays pinned so the sticky controls can sit beneath it", () => {
  const css = readProjectFile("src/app/frame/AppFrame.css");
  const dockBody = getRuleBody(css, ".app-surface__header-dock");
  const statusBarBody = getRuleBody(css, ".top-status-bar");

  // 吸顶职责在满幅背板上，不在状态栏卡片自身：卡片背景只覆盖圆角矩形，挡不住身后滚过的内容。
  assert.match(dockBody, /position:\s*sticky;/);
  // top 必须是 0：粘滞是绘制期偏移，top 大于自然偏移量会把绘制位置下推，
  // 背板随之越过状态栏底边压住下方内容。
  assert.match(dockBody, /top:\s*0;/);
  assert.match(dockBody, /z-index:\s*40;/);
  assert.doesNotMatch(statusBarBody, /position:\s*sticky;/);
  assert.doesNotMatch(statusBarBody, /z-index:/);
  // header 高度 token 是状态栏与吸顶条的对齐基准，必须在 tokens 中存在。
  const tokens = readProjectFile("src/shared/styles/tokens.css");
  assert.match(tokens, /--app-header-height:\s*64px;/);
});

test("sticky header dock paints an opaque full-bleed backdrop so content cannot stack under the status bar", () => {
  const css = readProjectFile("src/app/frame/AppFrame.css");
  const dockBody = getRuleBody(css, ".app-surface__header-dock");
  const backdropBody = getRuleBody(css, ".app-surface__header-dock::before");

  // 背板必须不透明，否则滚动内容仍会从状态栏四周和圆角缺口透出来。
  assert.match(backdropBody, /background:\s*var\(--color-bg\);/);
  // 负偏移向上盖住页面内边距、向两侧盖到 padding 外沿。
  assert.match(backdropBody, /top:\s*calc\(-1 \* var\(--app-surface-inset\)\);/);
  assert.match(backdropBody, /left:\s*calc\(-1 \* var\(--app-surface-inset\)\);/);
  assert.match(backdropBody, /right:\s*calc\(-1 \* var\(--app-surface-inset\)\);/);
  // 高度必须是"页面内边距 + 状态栏高度"的显式值。用 bottom 去贴 dock 盒底是不安全的：
  // dock 的盒高不保证等于状态栏高度，越界就会裁掉内容区首行。
  assert.match(
    backdropBody,
    /height:\s*calc\(var\(--app-surface-inset\) \+ var\(--app-header-height\)\);/,
  );
  assert.doesNotMatch(backdropBody, /(^|[;{])\s*bottom:/);
  // 位于 .top-status-bar 之下、dock 层叠上下文之外的页面内容之上，只遮内容不遮状态栏。
  assert.match(backdropBody, /z-index:\s*-1;/);
  // 背板必须画在伪元素上：dock 是 grid item，sticky 包含块就是它的 grid area，
  // 自身盒模型一旦被负 margin 撑大就会改变粘滞行为，不再等价于改动前的状态栏。
  assert.doesNotMatch(dockBody, /margin:/);
  assert.doesNotMatch(dockBody, /background:/);

  // 背板尺寸依赖 --app-surface-inset，因此每个改写 .app-surface padding 的断点都必须同步该变量，
  // 否则背板会与实际内边距错位，重新露出被遮挡的内容。
  const surfaceRules = css.match(/\.app-surface\s*{[^}]*}/g) ?? [];
  const paddingRules = surfaceRules.filter((rule) => /(^|[;{])\s*padding:/.test(rule));
  assert.ok(paddingRules.length > 0, "expected at least one .app-surface padding rule");
  for (const rule of paddingRules) {
    assert.match(
      rule,
      /--app-surface-inset:/,
      `.app-surface rule declares padding without syncing --app-surface-inset: ${rule}`,
    );
  }
});

test("sticky controls are an opaque single-column bar fixed above the scroll container", () => {
  const css = readModLibraryCss();
  const paginationLayoutCss = readProjectFile("src/features/mods/ModLibraryPaginationLayout.css");

  // 滚动容器已下沉到 .mod-library__content：它是 overflow-y:auto 的滚动容器，
  // 高度由路由作用域闭合的高度链约束，滚动条只出现在卡片区域，不达到状态栏高度。
  assert.match(css, /\.mod-library__content\s*{[\s\S]*?overflow-y:\s*auto;/);
  assert.match(
    css,
    /\.route-transition__layer\[data-route-id="mods"\][\s\S]*?grid-template-rows:\s*minmax\(0,\s*1fr\);/,
  );
  // 吸顶条处于滚动容器(content)之外的 mod-library 第1行，浮层反馈不占页面网格；
  // 祖先链都不滚，故无需 sticky。滚动条(content 的)从内容区顶部开始，绝不达搜索栏。
  const stickyControlsBody = getRuleBody(css, ".mod-library__sticky-controls");
  assert.doesNotMatch(stickyControlsBody, /position:\s*sticky;/);
  assert.match(stickyControlsBody, /grid-row:\s*1;/);
  assert.match(getRuleBody(css, ".mod-library__content-shell"), /grid-row:\s*2;/);
  assert.match(getRuleBody(paginationLayoutCss, ".mod-library > .mod-library-pagination"), /grid-row:\s*3;/);
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
  const css = readModLibraryCss();

  // 按钮容器换行，不再强制单行横排；移除横向滚动条，杜绝水平溢出截断返回顶部按钮。
  assert.match(css, /\.compact-panel__stack\s*{[\s\S]*?flex-wrap:\s*wrap;/);
  assert.doesNotMatch(css, /\.compact-panel__stack\s*{[\s\S]*?overflow-x:\s*auto;/);
  assert.doesNotMatch(css, /@media\s*\(max-width:\s*1280px\)\s*{[\s\S]*?\.compact-panel__stack\s*{[\s\S]*?overflow-x:\s*auto;/);
});

test("narrow screens keep the mod list usable beneath tall controls", () => {
  const css = readModLibraryCss();

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
    /@media\s*\(max-width:\s*860px\)\s*{[\s\S]*?\.mod-library\s*{[\s\S]*?grid-template-rows:\s*auto\s+minmax\(320px,\s*1fr\)\s+auto;/,
  );
});

test("short desktop windows allow outer scrolling and reserve a usable content track", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPaginationLayout.css");

  assert.match(
    css,
    /@media\s*\(max-width:\s*1280px\)\s*and\s*\(max-height:\s*720px\)\s*{[\s\S]*?\.app-surface:has\(\.route-transition__layer\[data-route-id="mods"\]\)\s*{[\s\S]*?overflow-y:\s*auto;/,
  );
  assert.match(
    css,
    /@media\s*\(max-width:\s*1280px\)\s*and\s*\(max-height:\s*720px\)\s*{[\s\S]*?\.mod-library\s*{[\s\S]*?grid-template-rows:\s*auto\s+minmax\(460px,\s*1fr\)\s+auto;/,
  );

  // 顶部遮挡不再由本断点的局部 ::before 承担，改由 AppFrame 的满幅背板全局负责，
  // 因此这里不得再出现只覆盖单一断点/单一路由的补丁式遮挡。
  assert.doesNotMatch(css, /\.top-status-bar::before\s*\{/);
});

test("quick action panel no longer owns sticky positioning directly", () => {
  const css = readModLibraryCss();
  const compactPanelBody = getRuleBody(css, ".compact-panel");

  assert.doesNotMatch(compactPanelBody, /position:\s*sticky;/);
  assert.match(compactPanelBody, /min-width:\s*0;/);
});

test("primary add action keeps contrast-safe blue gradients for white text", () => {
  const css = readModLibraryCss();
  const primaryBody = getRuleBody(css, ".compact-action.is-primary");
  const primaryHoverBody = getRuleBody(
    css,
    '.compact-action.is-primary:not(:disabled):not([aria-disabled="true"]):hover',
  );
  const darkPrimaryBody = getRuleBody(css, ':root[data-color-scheme="dark"] .compact-action.is-primary');
  const darkPrimaryHoverBody = getRuleBody(
    css,
    ':root[data-color-scheme="dark"]\n  .compact-action.is-primary:not(:disabled):not([aria-disabled="true"]):hover',
  );

  for (const body of [primaryBody, primaryHoverBody, darkPrimaryBody, darkPrimaryHoverBody]) {
    assert.doesNotMatch(body, /#3b82f6|#60a5fa|#93c5fd/i);
  }

  assert.match(primaryBody, /#2563eb/);
  assert.match(primaryBody, /#1d4ed8/);
  assert.match(primaryHoverBody, /#1e40af/);
  assert.match(darkPrimaryBody, /#2563eb/);
  assert.match(darkPrimaryHoverBody, /#1e40af/);
});

test("tech view mod cards are allowed to grow to their full data panel height", () => {
  const css = readModLibraryCss();
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
  const css = readModLibraryCss();
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
  const selection = readProjectFile("src/features/mods/modSelection.ts");

  assert.match(source, /const \[selectionState, dispatchSelection\] = useReducer\(/);
  assert.match(source, /createInitialModSelectionState\(\)/);
  assert.match(selection, /mode: "single",\s*selectedIds: new Set<string>\(\)/);
  assert.doesNotMatch(selection, /selectedIds: new Set<string>\(\[/);
});
