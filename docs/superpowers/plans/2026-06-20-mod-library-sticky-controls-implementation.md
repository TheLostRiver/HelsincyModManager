# Mod 列表吸顶控制区 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 Mod 列表页的搜索、筛选和快捷操作在滚动列表时始终停留在可视区域内，减少用户回到顶部的操作成本。

**Architecture:** 只调整 `src/features/mods/` 前端页面边界。把搜索栏和快捷操作收束为页面级 sticky controls：桌面宽屏仍呈现左侧工具栏 + 右侧操作面板，中窄屏折叠为单列顶部吸顶控制区。继续使用 `.app-surface` 作为滚动容器，不使用 `position: fixed`，避免脱离 App Shell、侧边栏模式和路由过渡层。

**Tech Stack:** React 19, TypeScript, CSS Grid, `position: sticky`, CSS custom properties, Node built-in test runner.

---

## File Structure

- Create `src/features/mods/modLibraryStickyControls.test.mjs`: 用源码结构测试保护 sticky controls 的 DOM 和 CSS 合约。
- Modify `src/features/mods/ModLibraryPage.tsx`: 重排 Mod 列表页结构，把 `LibraryToolbar` 和 `CompactActionPanel` 放入统一控制区。
- Modify `src/features/mods/ModLibraryPage.css`: 增加 sticky controls 布局，修正快捷操作 sticky 生效位置，保留返回顶部按钮和卡片网格现有职责。
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

- 宽屏：搜索栏/筛选在左列吸顶，快捷操作在右列吸顶，二者顶部对齐。
- `<=1280px`：搜索栏和快捷操作变成单列顶部吸顶控制区，快捷操作仍为横向滚动按钮条。
- `<=640px`：控制区仍吸顶，按钮可读、可点击，不遮挡 Mod 卡片标题和返回顶部按钮。
- 返回顶部按钮继续停留在主列表列的右下视觉位置，不并入快捷操作面板。

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

test("sticky controls use app-surface friendly sticky positioning instead of fixed positioning", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(css, /\.mod-library\s*{[\s\S]*?--mod-library-sticky-top:\s*var\(--layout-page-padding\);/);
  assert.match(css, /\.mod-library__sticky-controls\s*{[\s\S]*?display:\s*grid;/);
  assert.match(css, /\.mod-library__sticky-controls\s*{[\s\S]*?grid-template-columns:\s*minmax\(0,\s*1fr\)\s+var\(--layout-mod-action-panel-width\);/);
  assert.match(css, /\.mod-library__toolbar-slot,[\s\S]*?\.mod-library__actions-slot\s*{[\s\S]*?position:\s*sticky;/);
  assert.match(css, /\.mod-library__toolbar-slot,[\s\S]*?\.mod-library__actions-slot\s*{[\s\S]*?top:\s*var\(--mod-library-sticky-top\);/);
  assert.doesNotMatch(css, /\.library-toolbar[\s\S]*?position:\s*fixed;/);
  assert.doesNotMatch(css, /\.compact-panel[\s\S]*?position:\s*fixed;/);
});

test("narrow layouts collapse controls into a single sticky top area and keep quick actions horizontally scrollable", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(
    css,
    /@media\s*\(max-width:\s*1280px\)\s*{[\s\S]*?\.mod-library__sticky-controls\s*{[\s\S]*?position:\s*sticky;[\s\S]*?grid-template-columns:\s*minmax\(0,\s*1fr\);/,
  );
  assert.match(
    css,
    /@media\s*\(max-width:\s*1280px\)\s*{[\s\S]*?\.mod-library__toolbar-slot,[\s\S]*?\.mod-library__actions-slot\s*{[\s\S]*?position:\s*static;/,
  );
  assert.match(
    css,
    /@media\s*\(max-width:\s*1280px\)\s*{[\s\S]*?\.compact-panel__stack\s*{[\s\S]*?overflow-x:\s*auto;/,
  );
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

Expected: FAIL. The new test should report missing `mod-library__sticky-controls`, missing sticky slot CSS, and direct sticky ownership on `.compact-panel`.

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

      <div className="mod-library__content">
        <div className="mod-library__main-floating-actions">
          <BackToTopButton onClick={handleBackToTop} />
        </div>

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

### Task 3: Implement Sticky Controls CSS

**Files:**
- Modify: `src/features/mods/ModLibraryPage.css`
- Test: `src/features/mods/modLibraryStickyControls.test.mjs`
- Possible test update: `src/features/mods/modLibraryBackToTop.test.mjs`

- [ ] **Step 1: Replace the body/main layout with sticky controls/content layout**

In `src/features/mods/ModLibraryPage.css`, replace the existing `.mod-library__body` and `.mod-library__main` rule blocks with:

```css
.mod-library {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  gap: var(--layout-content-gap);
  min-width: 0;
  min-height: 0;
  --mod-library-sticky-top: var(--layout-page-padding);
  --mod-library-back-to-top-size: 52px;
  --mod-library-back-to-top-inline-offset: calc(var(--mod-library-back-to-top-size) + 12px);
  --mod-library-back-to-top-block-offset: 12px;
}

.mod-library__sticky-controls {
  display: grid;
  grid-template-columns: minmax(0, 1fr) var(--layout-mod-action-panel-width);
  align-items: start;
  gap: var(--layout-content-gap);
  min-width: 0;
  z-index: 30;
}

.mod-library__toolbar-slot,
.mod-library__actions-slot {
  position: sticky;
  top: var(--mod-library-sticky-top);
  z-index: 30;
  min-width: 0;
}

.mod-library__content {
  position: relative;
  display: grid;
  gap: 12px;
  min-width: 0;
  min-height: 0;
}
```

If `.mod-library` already exists, merge the new custom properties into that existing block instead of creating a duplicate `.mod-library` rule.

- [ ] **Step 2: Update back-to-top positioning to target content**

Keep `.mod-library__main-floating-actions` as the back-to-top wrapper, but ensure it can live under `.mod-library__content`:

```css
.mod-library__main-floating-actions {
  grid-column: 1;
  grid-row: 1 / -1;
  position: sticky;
  top: calc(100dvh - var(--layout-page-padding) - var(--mod-library-back-to-top-size) - var(--mod-library-back-to-top-block-offset));
  z-index: 20;
  display: flex;
  justify-content: end;
  align-self: start;
  height: 0;
  transform: translateX(var(--mod-library-back-to-top-inline-offset));
  pointer-events: none;
}
```

This is the current behavior. Only keep it after removing `.mod-library__main`; do not convert the button to fixed positioning.

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

- [ ] **Step 4: Add visual separation for sticky toolbar state**

Extend `.library-toolbar` and `.compact-panel` with stable backgrounds that remain readable over scrolled cards. Keep the existing colors and shadows; add only backdrop support:

```css
.library-toolbar,
.compact-panel {
  backdrop-filter: blur(12px);
}
```

If visual QA shows `backdrop-filter` creates poor contrast in dark mode, remove this addition and rely on the existing solid surface colors and shadows.

- [ ] **Step 5: Replace the 1280px responsive block**

In `@media (max-width: 1280px)`, replace the old `.mod-library__body`, `.mod-library__main`, and `.compact-panel { position: static; ... }` layout parts with:

```css
@media (max-width: 1280px) {
  .mod-library {
    --mod-library-sticky-top: 12px;
    --mod-library-back-to-top-inline-offset: 0px;
  }

  .mod-library__sticky-controls {
    position: sticky;
    top: var(--mod-library-sticky-top);
    grid-template-columns: minmax(0, 1fr);
    gap: 10px;
    z-index: 35;
  }

  .mod-library__toolbar-slot,
  .mod-library__actions-slot {
    position: static;
  }

  .compact-panel {
    grid-auto-flow: column;
    grid-template-rows: none;
    min-width: 0;
    overflow: hidden;
  }

  .compact-panel__stack {
    grid-auto-flow: column;
    grid-auto-columns: minmax(140px, max-content);
    min-width: 0;
    overflow-x: auto;
    overscroll-behavior-x: contain;
    padding-bottom: 4px;
  }

  .compact-action {
    min-width: 140px;
  }
}
```

Reason: on narrow layouts the whole controls group is sticky, while its children become static inside the group.

- [ ] **Step 6: Update the 640px block to reference `.mod-library`**

Keep the existing card density behavior, but ensure back-to-top variables now live on `.mod-library`:

```css
@media (max-width: 640px) {
  .mod-library {
    --layout-mod-card-min-width: 150px;
    --layout-mod-card-poster-height: 220px;
    --mod-library-back-to-top-size: 48px;
    --mod-library-back-to-top-block-offset: 8px;
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

  .mod-library__main-floating-actions {
    top: calc(100dvh - 20px - var(--mod-library-back-to-top-size) - var(--mod-library-back-to-top-block-offset));
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
assert.match(css, /\.mod-library__main-floating-actions[\s\S]*?position:\s*sticky;/);
assert.doesNotMatch(css, /\.mod-library__back-to-top[\s\S]*?position:\s*fixed;/);
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

- [ ] **Step 3: Verify sticky behavior at desktop width**

At `1440x900`, scroll `.app-surface` until the first row of Mod cards has left the viewport.

Run this snippet in the browser console:

```js
(() => {
  const surface = document.querySelector(".app-surface");
  const controls = document.querySelector(".mod-library__sticky-controls");
  const toolbar = document.querySelector(".library-toolbar");
  const actions = document.querySelector(".compact-panel");
  if (!surface || !controls || !toolbar || !actions) {
    return { error: "missing required elements" };
  }

  const surfaceRect = surface.getBoundingClientRect();
  const controlsRect = controls.getBoundingClientRect();
  const toolbarRect = toolbar.getBoundingClientRect();
  const actionsRect = actions.getBoundingClientRect();

  return {
    surfaceTop: Math.round(surfaceRect.top),
    controlsTop: Math.round(controlsRect.top),
    toolbarVisible: toolbarRect.bottom > surfaceRect.top && toolbarRect.top < surfaceRect.bottom,
    actionsVisible: actionsRect.bottom > surfaceRect.top && actionsRect.top < surfaceRect.bottom,
    toolbarAboveActionsDelta: Math.abs(Math.round(toolbarRect.top - actionsRect.top)),
  };
})();
```

Expected:

```js
{
  toolbarVisible: true,
  actionsVisible: true,
  toolbarAboveActionsDelta: 0
}
```

`controlsTop` may be slightly below `surfaceTop` because of page padding.

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
- Quick action buttons remain visible or reachable through horizontal scrolling.
- No horizontal page overflow appears.
- Back-to-top button remains visible and does not cover the search input or action buttons.

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

Do not move controls to `position: fixed`.

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

- **Spec coverage:** 覆盖搜索栏、筛选 chips、右侧快捷操作、宽屏双列、中窄屏单列、返回顶部按钮不并入快捷操作、`.app-surface` sticky 模型、不使用 fixed。
- **Scope check:** 单一前端布局任务。没有触碰 Tauri command、Rust crates、InstallPlan、manifest、backup、rollback、真实文件写入、游戏适配器或玩家数据路径。
- **Existing patterns:** 使用当前项目已有 Node 源码扫描测试风格，延续 `src/features/mods/modLibraryBackToTop.test.mjs` 的测试方式。
- **Placeholder scan:** 每个任务都有明确文件、代码片段、命令和期望结果；没有未定义的实现步骤。
- **Type consistency:** 新 wrapper 名称统一为 `mod-library__sticky-controls`、`mod-library__toolbar-slot`、`mod-library__actions-slot`、`mod-library__content`。
- **Risk note:** sticky 生效依赖 `.app-surface` 作为滚动容器；最终必须做浏览器滚动 smoke test，源码测试只能保护结构合约，不能替代真实滚动验证。
