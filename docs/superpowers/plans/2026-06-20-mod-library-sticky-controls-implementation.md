# Mod 列表吸顶控制区 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 Mod 列表页的搜索、筛选和快捷操作在滚动列表时始终停留在可视区域内，减少用户回到顶部的操作成本。

**Architecture:** 最终目标已从原始「双列 sticky slots」修订为「全局状态栏吸顶 + Mod 页面单列两行」。`.top-status-bar` 在全局 App Frame 中吸顶；`.mod-library__sticky-controls` 是 `.mod-library` 第一行的实体不透明控制条，搜索栏在第一行，快捷操作在第二行并通过 `flex-wrap` 换行；`.mod-library__content` 是唯一真实列表滚动容器。返回顶部浮动控件是后续范围修订中的 `position: fixed` 例外，由列表滚动状态控制显隐。

**Tech Stack:** React 19, TypeScript, CSS Grid, `position: sticky`, CSS custom properties, Node built-in test runner.

---

## Prerequisites

These global CSS dependencies are part of the final architecture and must be in place before executing the Mod page tasks:

- Modify `src/shared/styles/tokens.css`: add `--app-header-height: 64px;` to both light and dark theme token blocks so the app header has a stable layout token.
- Modify `src/app/frame/AppFrame.css`: keep `.app-surface` as the app-level surface, and make `.top-status-bar` globally sticky with `position: sticky; top: 0; z-index: 40;`.
- Keep the Mod route-specific scroll ownership in `src/features/mods/ModLibraryPage.css`: for the mods route, `.app-surface` hides its own scrolling and `.mod-library__content` becomes the constrained inner scroller.

---

## File Structure

- Modify `src/shared/styles/tokens.css`: Add `--app-header-height` for the global sticky status bar.
- Modify `src/app/frame/AppFrame.css`: Make `.top-status-bar` sticky and keep `.app-surface` overflow split behavior.
- Create `src/features/mods/modLibraryStickyControls.test.mjs`: 用源码结构测试保护 sticky controls 的 DOM 和 CSS 合约。
- Modify `src/features/mods/ModLibraryPage.tsx`: 重排 Mod 列表页结构，把 `LibraryToolbar` 和 `CompactActionPanel` 放入统一控制区。
- Modify `src/features/mods/ModLibraryPage.css`: 增加实体不透明控制条、路由级内层滚动容器、隐藏原生列表滚动条视觉，并保留返回顶部按钮和卡片网格职责。
- Inspect `src/features/mods/LibraryToolbar.tsx`: 不改组件 API，只确认它继续由父页面传入 query/filter 状态。
- Inspect `src/features/mods/CompactActionPanel.tsx`: 不改业务动作 API，只让父页面控制布局位置。
- Inspect `src/features/mods/modLibraryBackToTop.test.mjs`: 若结构测试和旧断言冲突，只更新与页面 wrapper 名称相关的断言，不改变 back-to-top 行为。
- Do not modify `src-tauri/`, Rust crates, Tauri commands, InstallPlan, manifest, backup, rollback, game adapters, mock data generation, or mod selection behavior.

## Important Context

当前结构：

- `src/app/frame/AppFrame.css` 中 `.app-surface` 是真实滚动容器，设置了 `overflow: auto`。
- `src/features/mods/ModLibraryPage.tsx` 中 `LibraryToolbar` 位于主列表列顶部，`CompactActionPanel` 位于右侧列外层 `.anim-stagger-item` 中。
- `src/features/mods/ModLibraryPage.css` 中 `.compact-panel` 写了 `position: sticky; top: 0;`，但 sticky 在内部面板上，外层动画 wrapper 高度基本等于面板自身，可活动空间不足。
- `@media (max-width: 1280px)` 中 `.compact-panel` 被改成 `position: static`，所以中窄屏下快捷操作一定会随列表滚走。
- `.library-toolbar` 目前没有 sticky 逻辑，滚动后必然离开视野。

目标行为：

- 全局状态栏：`.top-status-bar` 吸顶，滚动 Mod 列表时顶部不留透明空洞。
- Mod 控制区：搜索栏和快捷操作合并为一个不透明实体条，常驻 `.mod-library` 第一行；快捷操作另起第二行并换行，不使用横向滚动条。
- Mod 列表区：`.mod-library__content` 是真实滚动容器，滚动条视觉从控制条下方开始；原生滚动条视觉隐藏，自绘滚动条由滚动状态控制。
- `<=640px`：控制区仍可读、可点击，不遮挡 Mod 卡片标题和返回顶部按钮。
- 返回顶部按钮继续独立于快捷操作面板，由 `.mod-library__content` 的滚动状态控制显隐，当前最终偏移为桌面 `100px`、小屏 `80px`。

---

### Task 1: Add Sticky Controls Contract Tests

**Files:**
- Create: `src/features/mods/modLibraryStickyControls.test.mjs`

- [ ] **Step 1: Write the failing structure and CSS tests**

Create `src/features/mods/modLibraryStickyControls.test.mjs`:

```js
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

test("ModLibraryPage groups toolbar and quick actions into one sticky controls area", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /className="mod-library__sticky-controls"/);
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

  const tokens = readProjectFile("src/shared/styles/tokens.css");
  assert.match(tokens, /--app-header-height:\s*64px;/);
});

test("sticky controls are an opaque single-column bar fixed above the scroll container", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(css, /\.mod-library__content\s*{[\s\S]*?overflow-y:\s*auto;/);
  assert.match(css, /\.mod-library__sticky-controls\s*{[\s\S]*?display:\s*grid;/);
  assert.match(css, /\.mod-library__sticky-controls\s*{[\s\S]*?grid-template-columns:\s*minmax\(0,\s*1fr\);/);
  assert.match(css, /\.mod-library__sticky-controls\s*{[\s\S]*?background:\s*var\(--color-surface\);/);
  assert.match(css, /\.mod-library__sticky-controls\s*{[\s\S]*?border:\s*1px\s+solid\s+var\(--color-border-muted\);/);
  assert.doesNotMatch(css, /\.mod-library__sticky-controls\s*{[\s\S]*?display:\s*contents;/);
  assert.match(css, /\.mod-library__toolbar-slot,[\s\S]*?\.mod-library__actions-slot\s*{[\s\S]*?position:\s*static;/);
  assert.match(css, /\.mod-library__actions-slot\s*{[\s\S]*?border-top:\s*1px\s+solid\s+var\(--color-border-muted\);/);
  assert.doesNotMatch(css, /\.library-toolbar[\s\S]*?position:\s*fixed;/);
  assert.doesNotMatch(css, /\.compact-panel[\s\S]*?position:\s*fixed;/);
});

test("quick actions wrap instead of horizontally scrolling", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(css, /\.compact-panel__stack\s*{[\s\S]*?flex-wrap:\s*wrap;/);
  assert.doesNotMatch(css, /\.compact-panel__stack\s*{[\s\S]*?overflow-x:\s*auto;/);
});

test("quick action panel no longer owns sticky positioning directly", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.doesNotMatch(css, /\.compact-panel\s*{[\s\S]*?position:\s*sticky;/);
  assert.match(css, /\.compact-panel\s*{[\s\S]*?min-width:\s*0;/);
});
```

- [ ] **Step 2: Run the focused tests and confirm they fail for the current layout**

Run:

```powershell
cmd /c corepack pnpm run test
```

Expected: FAIL. The new test should report missing `mod-library__sticky-controls`, missing opaque single-column control bar CSS, missing global sticky status bar support, or direct sticky ownership on `.compact-panel`.

- [ ] **Step 3: Keep the failing test uncommitted until Task 3 passes**

Do not commit the failing test alone. It should be committed with the implementation that makes it pass.

---

### Task 2: Restructure ModLibraryPage Around Sticky Controls

**Files:**
- Modify: `src/features/mods/ModLibraryPage.tsx`
- Inspect: `src/features/mods/LibraryToolbar.tsx`
- Inspect: `src/features/mods/CompactActionPanel.tsx`

- [ ] **Step 1: Replace the page return structure**

In `src/features/mods/ModLibraryPage.tsx`, replace only the JSX returned inside:

```tsx
return (
  <section className="mod-library" aria-label="模组库">
```

with this structure:

```tsx
  return (
    <section className="mod-library" aria-label="模组库">
      <div className="mod-library__sticky-controls anim-stagger-item" style={staggerStyle(0)}>
        <div className="mod-library__toolbar-slot">
          <LibraryToolbar
            query={query}
            activeFilter={activeFilter}
            onQueryChange={setQuery}
            onFilterChange={setActiveFilter}
          />
        </div>

        <div className="mod-library__actions-slot">
          <CompactActionPanel selectedCount={selectedCount} onAction={handleAction} />
        </div>
      </div>

      <div className="mod-library__content-shell" data-scroll-ui={showScrollUi ? "visible" : "hidden"}>
        <div ref={contentRef} className="mod-library__content">
          {showScrollUi ? (
            <div className="mod-library__main-floating-actions">
              <BackToTopButton onClick={handleBackToTop} />
            </div>
          ) : null}

          {visibleItems.length === 0 ? (
            <div className="mod-library__empty anim-stagger-item" style={staggerStyle(1)} role="status">
              <strong>没有匹配的 Mod</strong>
              <p>试试调整搜索关键词或筛选条件。</p>
            </div>
          ) : (
            <div className="mod-grid" role="list">
              {visibleItems.map((item, index) => (
                <ModPosterCard
                  key={item.id}
                  item={item}
                  selected={selectedIds.has(item.id)}
                  onSelect={selectCard}
                  index={index}
                />
              ))}
            </div>
          )}
        </div>

        {showScrollUi ? (
          <div className="mod-library__scrollbar" aria-hidden="true">
            <div
              className="mod-library__scrollbar-thumb"
              style={thumbStyle}
              onPointerDown={handleScrollbarPointerDown}
            />
          </div>
        ) : null}
      </div>
    </section>
  );
```

The exact display text must match the current decoded Simplified Chinese strings in the file. If the file still shows mojibake in the editor, preserve the existing string literals instead of retyping user-facing copy.

- [ ] **Step 2: Remove obsolete wrapper classes from JSX**

After Step 1, confirm `ModLibraryPage.tsx` no longer contains these layout wrappers:

```tsx
className="mod-library__body"
className="mod-library__main"
```

It should still contain:

```tsx
className="mod-library__main-floating-actions"
```

Reason: back-to-top keeps its own floating layer inside content; the two-column page body is replaced by sticky controls + content.

- [ ] **Step 3: Run typecheck**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: PASS. `LibraryToolbar` and `CompactActionPanel` props remain unchanged.

---

### Task 3: Implement Final Sticky Controls CSS

**Files:**
- Modify: `src/features/mods/ModLibraryPage.css`
- Test: `src/features/mods/modLibraryStickyControls.test.mjs`
- Possible test update: `src/features/mods/modLibraryBackToTop.test.mjs`

- [ ] **Step 1: Replace the body/main layout with opaque controls plus inner content scroller**

In `src/features/mods/ModLibraryPage.css`, replace the existing `.mod-library__body` and `.mod-library__main` rule blocks with the final single-column layout:

```css
.mod-library {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: auto minmax(0, 1fr);
  gap: var(--layout-content-gap);
  min-width: 0;
  min-height: 0;
  --mod-library-back-to-top-size: 52px;
  --mod-library-back-to-top-block-offset: 100px;
}

.mod-library__sticky-controls {
  grid-column: 1;
  grid-row: 1;
  position: relative;
  z-index: 30;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  align-items: center;
  gap: 10px;
  min-width: 0;
  padding: 10px 14px;
  background: var(--color-surface);
  border: 1px solid var(--color-border-muted);
  border-radius: 20px;
  box-shadow: var(--shadow-soft);
}

.mod-library__toolbar-slot,
.mod-library__actions-slot {
  position: static;
  min-width: 0;
}

.mod-library__toolbar-slot {
  grid-column: 1;
  grid-row: 1;
}

.mod-library__actions-slot {
  grid-column: 1;
  grid-row: 2;
  padding-top: 10px;
  border-top: 1px solid var(--color-border-muted);
}

.mod-library__content-shell {
  grid-column: 1;
  grid-row: 2;
  position: relative;
  display: grid;
  grid-template-rows: minmax(0, 1fr);
  min-width: 0;
  min-height: 0;
}

.mod-library__content {
  position: relative;
  display: grid;
  grid-template-rows: minmax(0, 1fr);
  min-width: 0;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  scrollbar-gutter: auto;
  scrollbar-width: none;
}

.mod-library__content::-webkit-scrollbar {
  width: 0;
  height: 0;
}
```

If `.mod-library` already exists, merge the new custom properties into that existing block instead of creating a duplicate `.mod-library` rule.

- [ ] **Step 2: Update back-to-top positioning to the final floating behavior**

Keep `.mod-library__main-floating-actions` as the back-to-top wrapper, but make it a state-rendered fixed floating control. This is the explicit exception to the original "no fixed" preference because the later scroll UI requirement needs a viewport-floating control that does not move with cards:

```css
.mod-library__main-floating-actions {
  position: fixed;
  right: var(--layout-page-padding);
  bottom: var(--mod-library-back-to-top-block-offset);
  z-index: 50;
  display: flex;
  justify-content: end;
  pointer-events: none;
}
```

The button itself remains `pointer-events: auto`; the wrapper is rendered only when `showScrollUi` is true.

- [ ] **Step 3: Move sticky ownership away from `.compact-panel`**

Replace the top of `.compact-panel` so the panel itself no longer owns sticky positioning:

```diff
 .compact-panel {
-  position: sticky;
-  top: 0;
   display: grid;
   grid-template-rows: auto auto;
   gap: 6px;
   min-width: 0;
```

If `min-width: 0;` is already present, keep one copy only.

- [ ] **Step 4: Remove independent card chrome from toolbar and action panel**

The unified controls bar owns the surface, border, radius, and shadow. Remove nested card chrome from the toolbar and compact panel:

```css
.library-toolbar,
.compact-panel {
  padding: 0;
  background: none;
  border: none;
  border-radius: 0;
  box-shadow: none;
}
```

Keep component APIs unchanged; this is only a layout and chrome change.

- [ ] **Step 5: Keep quick actions wrapping instead of horizontally scrolling**

The action panel is part of the controls bar second row. Buttons should wrap naturally and must not force a horizontal scrollbar:

```css
.compact-panel {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.compact-panel__header {
  display: none;
}

.compact-panel__stack {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

@media (max-width: 1280px) {
  .mod-library__sticky-controls {
    z-index: 35;
  }

  .compact-panel__stack {
    flex-wrap: wrap;
    overflow-x: visible;
    padding-bottom: 0;
  }
}
```

Reason: horizontal action scrolling was an intermediate idea; the final design uses wrap to avoid an extra horizontal scrollbar and overflow at narrow widths.

- [ ] **Step 6: Update the 640px block to reference `.mod-library`**

Keep the existing card density behavior, but ensure back-to-top variables now live on `.mod-library`:

```css
@media (max-width: 640px) {
  .mod-library {
    --layout-mod-card-min-width: 150px;
    --layout-mod-card-poster-height: 220px;
    --mod-library-back-to-top-size: 48px;
    --mod-library-back-to-top-block-offset: 80px;
  }

  .mod-grid {
    gap: 12px;
  }

  .compact-panel__stack {
    grid-auto-columns: auto;
    grid-auto-flow: row;
    overflow-x: visible;
    padding-bottom: 0;
  }

  .compact-action {
    min-width: 0;
  }

  .mod-library__back-to-top {
    width: 48px;
    height: 48px;
  }
}
```

- [ ] **Step 7: Update back-to-top tests only if wrapper assertions fail**

Run:

```powershell
cmd /c corepack pnpm run test
```

Expected: the new sticky controls tests should PASS. If `modLibraryBackToTop.test.mjs` fails because it expected `.mod-library__main` to own variables, update only those assertions to expect variables under `.mod-library`; keep these invariants unchanged:

```js
assert.match(source, /mod-library__main-floating-actions/);
assert.match(source, /showScrollUi\s*\?\s*\([\s\S]*?mod-library__main-floating-actions[\s\S]*?<BackToTopButton/);
assert.match(css, /\.mod-library__main-floating-actions[\s\S]*?position:\s*fixed;/);
assert.match(css, /\.mod-library\s*{[\s\S]*?--mod-library-back-to-top-block-offset:\s*100px;/);
```

- [ ] **Step 8: Run build**

Run:

```powershell
cmd /c corepack pnpm run build
```

Expected: PASS.

- [ ] **Step 9: Commit the page structure and sticky CSS**

Run:

```powershell
git add src/features/mods/ModLibraryPage.tsx src/features/mods/ModLibraryPage.css src/features/mods/modLibraryStickyControls.test.mjs src/features/mods/modLibraryBackToTop.test.mjs
git commit -m "style: 固定 Mod 列表搜索与快捷操作"
```

If `modLibraryBackToTop.test.mjs` did not change, omit it from `git add`.

---

### Task 4: Browser Smoke Verification

**Files:**
- Inspect: `src/features/mods/ModLibraryPage.tsx`
- Inspect: `src/features/mods/ModLibraryPage.css`
- Modify only if smoke test reveals overlap or inaccessible controls.

- [ ] **Step 1: Start the dev server**

Run:

```powershell
cmd /c corepack pnpm run dev -- --host 127.0.0.1 --port 1420
```

Expected: Vite serves the app at `http://127.0.0.1:1420/`.

- [ ] **Step 2: Open the Mod list page**

Open:

```text
http://127.0.0.1:1420/
```

Navigate to the Mod list route through the app sidebar.

- [ ] **Step 3: Verify desktop controls and scroll UI behavior**

At `1440x900`, scroll `.mod-library__content` until the first row of Mod cards has left the visible list area.

Run this snippet in the browser console:

```js
(() => {
  const content = document.querySelector(".mod-library__content");
  const controls = document.querySelector(".mod-library__sticky-controls");
  const toolbar = document.querySelector(".library-toolbar");
  const actions = document.querySelector(".compact-panel");
  if (!content || !controls || !toolbar || !actions) {
    return { error: "missing required elements" };
  }

  content.scrollTop = 520;
  const contentRect = content.getBoundingClientRect();
  const controlsRect = controls.getBoundingClientRect();
  const toolbarRect = toolbar.getBoundingClientRect();
  const actionsRect = actions.getBoundingClientRect();

  return {
    contentScrollTop: Math.round(content.scrollTop),
    controlsAboveContent: controlsRect.bottom <= contentRect.top,
    toolbarVisible: toolbarRect.bottom > 0,
    actionsVisible: actionsRect.bottom > 0,
    actionsBelowToolbar: actionsRect.top >= toolbarRect.bottom,
    backToTopVisible: Boolean(document.querySelector(".mod-library__back-to-top")),
    customScrollbarVisible: Boolean(document.querySelector(".mod-library__scrollbar-thumb")),
  };
})();
```

Expected:

```js
{
  contentScrollTop: 520,
  controlsAboveContent: true,
  toolbarVisible: true,
  actionsVisible: true,
  actionsBelowToolbar: true,
  backToTopVisible: true,
  customScrollbarVisible: true
}
```

Then set `content.scrollTop = 0` and verify both `.mod-library__back-to-top` and `.mod-library__scrollbar-thumb` disappear.

- [ ] **Step 4: Verify sticky behavior at common narrow widths**

Repeat the scroll and console check at these viewports:

```text
1366x768
1280x800
1024x768
800x600
640x812
375x812
```

Expected at every size:

- Search input remains visible after scrolling down.
- Filter chips remain reachable.
- Quick action buttons remain visible through wrapping; no horizontal action scrollbar is introduced.
- No horizontal page overflow appears.
- Back-to-top button and custom scrollbar are hidden at top and visible after scrolling down.

- [ ] **Step 5: Verify click and keyboard access**

At `1280x800` and `375x812`:

- Click the search input, type a query that filters the list.
- Click at least two filter chips.
- Click `select-all` and `invert` quick actions.
- Tab through search input, filter chips, quick actions, and the first visible Mod card.

Expected:

- Focus ring is visible.
- Focused controls are not hidden behind the sticky controls area.
- Quick action disabled states still work when no item is selected.

- [ ] **Step 6: Fix only concrete smoke failures**

If controls overlap cards or hide focus, adjust only `src/features/mods/ModLibraryPage.css`.

Acceptable targeted fixes:

```css
.mod-library__sticky-controls {
  margin-bottom: 2px;
}
```

or:

```css
.mod-library__sticky-controls {
  z-index: 40;
}
```

or, if narrow sticky area consumes too much height:

```css
@media (max-width: 640px) {
  .library-toolbar {
    padding: 8px 10px;
  }

  .library-filters {
    gap: 6px;
  }
}
```

Do not make `.mod-library__sticky-controls` fixed. The back-to-top wrapper may remain `position: fixed` because it is the intentional floating-control exception.

- [ ] **Step 7: Commit smoke-test fixes if needed**

If Step 6 changed CSS, run:

```powershell
git add src/features/mods/ModLibraryPage.css
git commit -m "fix: 完善 Mod 列表吸顶控制区视口表现"
```

If no source files changed, do not create an empty commit.

---

### Task 5: Final Verification

**Files:**
- No planned edits. Modify only if verification reveals a regression.

- [ ] **Step 1: Run focused frontend tests**

Run:

```powershell
cmd /c corepack pnpm run test
```

Expected: PASS. This includes Mod selection tests, back-to-top tests, and sticky controls contract tests.

- [ ] **Step 2: Run frontend typecheck**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: PASS.

- [ ] **Step 3: Run frontend lint**

Run:

```powershell
cmd /c corepack pnpm run lint
```

Expected: PASS.

- [ ] **Step 4: Run frontend build**

Run:

```powershell
cmd /c corepack pnpm run build
```

Expected: PASS.

- [ ] **Step 5: Run unified project verification**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected: PASS. This covers policy checks, frontend checks, Rust tests, and Rust check.

- [ ] **Step 6: Record manual verification evidence**

In the final response or PR description, record:

- Automatic commands actually run.
- Browser viewports checked.
- Whether search, filters, and quick actions stayed visible after scrolling.
- Whether keyboard focus was visible and reachable.
- Whether any viewport still has known limitations.
- Confirmation that no Tauri/Rust/install/backup/rollback files were modified.

- [ ] **Step 7: Commit final fixes only if needed**

If final verification required changes, commit them:

```powershell
git add src/features/mods/ModLibraryPage.tsx src/features/mods/ModLibraryPage.css src/features/mods/modLibraryStickyControls.test.mjs src/features/mods/modLibraryBackToTop.test.mjs
git commit -m "fix: 修正 Mod 列表吸顶控制区回归问题"
```

If no files changed, do not create an empty commit.

---

## Self-Review Notes

- **Spec coverage:** 覆盖搜索栏、筛选 chips、快捷操作换行、全局状态栏吸顶、`.mod-library__content` 内层滚动、顶部隐藏滚动 UI、返回顶部按钮不并入快捷操作。
- **Scope check:** 单一前端布局任务。没有触碰 Tauri command、Rust crates、InstallPlan、manifest、backup、rollback、真实文件写入、游戏适配器或玩家数据路径。
- **Existing patterns:** 使用当前项目已有 Node 源码扫描测试风格，延续 `src/features/mods/modLibraryBackToTop.test.mjs` 的测试方式。
- **Placeholder scan:** 每个任务都有明确文件、代码片段、命令和期望结果；没有未定义的实现步骤。
- **Type consistency:** 新 wrapper 名称统一为 `mod-library__sticky-controls`、`mod-library__toolbar-slot`、`mod-library__actions-slot`、`mod-library__content`。
- **Risk note:** 运行态依赖 mods 路由把滚动容器下沉到 `.mod-library__content`；最终必须做浏览器滚动 smoke test，源码测试只能保护结构合约，不能替代真实滚动验证。

---

## 范围修订（实现期发现）

> 实施过程中发现原始目标漏掉了两项视觉缺陷，需求随之扩展。本节记录改动超出原计划边界的部分与根因。

### 新发现的问题

1. **全局状态栏（`AppHeader` / `.top-status-bar`）不吸顶**：它是 `.app-surface` 滚动容器第一个 grid 行，原本不是 sticky，向下滚动时整条滚出视野，顶部留下一片透明区域。原计划只考虑了搜索栏/快捷操作的吸顶，未覆盖状态栏。
2. **透卡缝隙**：搜索栏 `.library-toolbar` 与操作面板 `.compact-panel` 是两个独立的圆角卡片，各自 `sticky`，且 `.mod-library__sticky-controls` 用 `display: contents` 把它们直接暴露到 grid。卡片之间的列 gap、卡片与已滚走状态栏之间的区域都是透明的，滚动的 Mod 卡片就从这些缝隙透出，视觉很突兀。

### 范围扩展

由此改动从「只改 `src/features/mods/`」扩展到全局布局层：

- **`src/shared/styles/tokens.css`**：新增 `--app-header-height: 64px;`（light + dark 两块），作为状态栏与 Mod 吸顶条的对齐基准。
- **`src/app/frame/AppFrame.css`**：`.top-status-bar` 追加 `position: sticky; top: 0; z-index: 40;`（全局行为，所有路由共用）。既有颜色/border/高度/小屏契约（`1360px` / `860px`）不回退，仅在原规则上叠加 sticky。

### Mod 控制区改为统一不透明条（与原计划「双列常驻」不同）

- `.mod-library__sticky-controls` 从 `display: contents` 改为**实体不透明容器**（`background` + `border` + `box-shadow`），位于 `.mod-library` 第一行、`.mod-library__content` 滚动容器之外，因此自身无需 `position: sticky` 也会常驻视野。
- 内部 `.library-toolbar` / `.compact-panel` 褪去独立卡片外观（去背景/边框/圆角/阴影），融入吸顶条，杜绝嵌套卡片与透卡缝隙。
- 桌面端操作面板由「右侧竖排常驻」改为**控制条第二行的 pill 按钮组**：`.compact-action` 收紧为 pill 状按钮，右侧装饰 dot 收起，`.compact-panel__stack` 使用 `flex-wrap` 换行，不引入横向滚动条。
- `.mod-library` 主体由「双列」改为「单列两行」（控制条 + 内容），原双列职责并入控制条；`--layout-mod-action-panel-width` 仍作为全局 token 保留，但本页最终不再消费它生成右侧列。
- `.mod-library__content` 成为真实列表滚动容器，原生滚动条视觉隐藏；返回顶部按钮和自绘滚动条由 `.mod-library__content` 的滚动状态统一控制，在顶部隐藏、下滚显示。

### 取舍

- **桌面操作面板从竖排常驻变控制条内换行按钮组**：这是彻底消除水平缝隙和额外水平滚动条的必要取舍。滚动时所有按钮仍可见可点。
- **状态栏吸顶是全局行为**：影响所有页面，但 Dashboard 等无 sticky 依赖，安全。
- `.mod-library__sticky-controls` 不使用 fixed；返回顶部按钮作为浮动控件使用 `position: fixed`，这是后续“顶部隐藏滚动 UI”需求下的明确例外。

### 测试同步

- `src/features/mods/modLibraryStickyControls.test.mjs`：断言改为校验「实体不透明控制条」「状态栏全局吸顶」「slot 为 static」「快捷操作换行」「内层滚动容器」。
- `src/shared/styles/layoutTokens.test.mjs`：min-width 护栏清单补齐 `.mod-library__sticky-controls` 与 `.compact-panel__stack`。
- `src/features/mods/modLibraryBackToTop.test.mjs`：断言改为校验按钮条件渲染、原生滚动条视觉隐藏、自绘滚动条存在，以及 `100px` / `80px` 的舒适底部偏移。
