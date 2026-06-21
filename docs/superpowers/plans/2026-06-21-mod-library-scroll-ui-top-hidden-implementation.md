# Mod Library Scroll UI Top Hidden Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Mod list page hide both the back-to-top floating control and the list scrollbar visual track when the list is at the very top, then show both while the inner Mod list is scrolled away from the top.

**Architecture:** Use `.mod-library__content` as the single source of truth for scroll UI state. Keep it as the real scroll container, hide the browser-native scrollbar visuals, and render a page-owned custom scrollbar overlay plus the existing back-to-top button only when `isScrollable && !isAtTop`. Put all new behavior inside `src/features/mods/` and leave Tauri/Rust/install/backup/rollback boundaries untouched.

**Tech Stack:** React 19, TypeScript, CSS Grid, DOM scroll events, `ResizeObserver`, CSS custom properties, Node built-in test runner, Vite dev server, Codex in-app browser.

---

## File Structure

- Create `src/features/mods/modLibraryScrollUi.ts`: Pure helper functions for deriving scroll UI state and thumb geometry from scroll measurements.
- Create `src/features/mods/modLibraryScrollUi.test.mjs`: Node tests for top/away-from-top, non-scrollable content, threshold behavior, and thumb geometry.
- Modify `src/features/mods/ModLibraryPage.tsx`: Add a ref for `.mod-library__content`, subscribe to scroll/resize/list changes, conditionally render `BackToTopButton`, and render the custom scrollbar overlay.
- Modify `src/features/mods/ModLibraryPage.css`: Hide native scrollbar visuals for `.mod-library__content`; add `.mod-library__content-shell`, `.mod-library__scrollbar`, and `.mod-library__scrollbar-thumb`; hide scroll UI at top through state-driven rendering.
- Modify `src/features/mods/modLibraryBackToTop.test.mjs`: Replace old “always visible/fixed” assertions with “button is conditionally rendered from scroll UI state” and “native scrollbar visuals are hidden in favor of custom scroll UI.”
- Inspect `src/features/mods/BackToTopButton.tsx`: Keep component API unchanged unless implementation needs an optional class or disabled state. Prefer no change.
- Inspect `src/features/mods/modLibraryStickyControls.test.mjs`: Update only if wrapping `.mod-library__content` in a shell breaks structure assertions.
- Do not modify `src-tauri/`, Rust crates, Tauri commands, InstallPlan, manifest, backup, rollback, game adapters, mock data generation, or player data paths.

## Important Context

Current behavior verified in the in-app browser at `http://localhost:1420/` after navigating through the app sidebar to `Mod 管理`:

- `.mod-library__content` is the real scroll container on the Mod page.
- At top: `scrollTop = 0`, `scrollHeight = 3518`, `clientHeight = 867`, and `canScrollY = true`.
- At top: `.mod-library__back-to-top` is still visible because `ModLibraryPage.tsx` renders it unconditionally.
- At top: native scrollbar visual space is still visible because `.mod-library__content` uses `overflow-y: auto`, `scrollbar-gutter: stable`, and `scrollbar-width: thin`.
- Direct URL navigation to `/mods` may still show the dashboard because route state initializes from `/`; browser smoke tests must navigate via the sidebar.

Target behavior:

- Initial top state: no back-to-top button; no visible scrollbar track/thumb in the list area.
- Scroll down: back-to-top button appears; custom scrollbar overlay appears with a thumb proportional to the visible fraction.
- Scroll upward but not yet top: both remain visible.
- Reach top: both disappear together.
- Search/filter changes that make the list shorter or reset the scroll position must recompute state and hide both when at top or non-scrollable.
- Keyboard, wheel, touchpad, and touch scrolling must continue to work on `.mod-library__content`.

---

### Task 1: Add Pure Scroll UI State Tests

**Files:**
- Create: `src/features/mods/modLibraryScrollUi.test.mjs`
- Create later in Task 2: `src/features/mods/modLibraryScrollUi.ts`

- [ ] **Step 1: Write the failing helper tests**

Create `src/features/mods/modLibraryScrollUi.test.mjs`:

```js
import assert from "node:assert/strict";
import { test } from "node:test";
import { getModLibraryScrollUiState } from "./modLibraryScrollUi.ts";

test("hides scroll UI when content is at the very top", () => {
  const state = getModLibraryScrollUiState({
    scrollTop: 0,
    scrollHeight: 3518,
    clientHeight: 867,
  });

  assert.equal(state.isScrollable, true);
  assert.equal(state.isAtTop, true);
  assert.equal(state.showScrollUi, false);
  assert.deepEqual(state.thumbStyle, {
    height: "213.7px",
    transform: "translateY(0px)",
  });
});

test("shows scroll UI when content has moved away from the top", () => {
  const state = getModLibraryScrollUiState({
    scrollTop: 520,
    scrollHeight: 3518,
    clientHeight: 867,
  });

  assert.equal(state.isScrollable, true);
  assert.equal(state.isAtTop, false);
  assert.equal(state.showScrollUi, true);
  assert.deepEqual(state.thumbStyle, {
    height: "213.7px",
    transform: "translateY(128.2px)",
  });
});

test("keeps scroll UI visible while scrolling upward before reaching the top", () => {
  const state = getModLibraryScrollUiState({
    scrollTop: 280,
    scrollHeight: 3518,
    clientHeight: 867,
  });

  assert.equal(state.isScrollable, true);
  assert.equal(state.isAtTop, false);
  assert.equal(state.showScrollUi, true);
  assert.deepEqual(state.thumbStyle, {
    height: "213.7px",
    transform: "translateY(69px)",
  });
});

test("hides scroll UI when content is not scrollable", () => {
  const state = getModLibraryScrollUiState({
    scrollTop: 0,
    scrollHeight: 640,
    clientHeight: 640,
  });

  assert.equal(state.isScrollable, false);
  assert.equal(state.isAtTop, true);
  assert.equal(state.showScrollUi, false);
  assert.deepEqual(state.thumbStyle, {
    height: "0px",
    transform: "translateY(0px)",
  });
});

test("treats subpixel scrollTop near zero as top to avoid flicker", () => {
  const state = getModLibraryScrollUiState({
    scrollTop: 0.5,
    scrollHeight: 3518,
    clientHeight: 867,
  });

  assert.equal(state.isScrollable, true);
  assert.equal(state.isAtTop, true);
  assert.equal(state.showScrollUi, false);
});

test("clamps thumb position when scrollTop exceeds the maximum scroll range", () => {
  const state = getModLibraryScrollUiState({
    scrollTop: 9999,
    scrollHeight: 3518,
    clientHeight: 867,
  });

  assert.equal(state.isScrollable, true);
  assert.equal(state.showScrollUi, true);
  assert.deepEqual(state.thumbStyle, {
    height: "213.7px",
    transform: "translateY(653.3px)",
  });
});
```

- [ ] **Step 2: Run the focused test and confirm it fails**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/features/mods/modLibraryScrollUi.test.mjs
```

Expected: FAIL with a module-not-found error for `./modLibraryScrollUi.ts`.

---

### Task 2: Implement Pure Scroll UI State Helper

**Files:**
- Create: `src/features/mods/modLibraryScrollUi.ts`
- Test: `src/features/mods/modLibraryScrollUi.test.mjs`

- [ ] **Step 1: Add the helper implementation**

Create `src/features/mods/modLibraryScrollUi.ts`:

```ts
type ScrollMetrics = {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
};

type ScrollUiState = {
  isScrollable: boolean;
  isAtTop: boolean;
  showScrollUi: boolean;
  thumbStyle: {
    height: string;
    transform: string;
  };
};

const SCROLL_TOP_EPSILON = 1;
const MIN_THUMB_HEIGHT = 36;

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function formatPixels(value: number) {
  const rounded = Math.round(value * 10) / 10;
  return `${Number.isInteger(rounded) ? rounded.toFixed(0) : rounded.toFixed(1)}px`;
}

export function getModLibraryScrollUiState({
  scrollTop,
  scrollHeight,
  clientHeight,
}: ScrollMetrics): ScrollUiState {
  const maxScrollTop = Math.max(0, scrollHeight - clientHeight);
  const isScrollable = maxScrollTop > SCROLL_TOP_EPSILON;
  const normalizedScrollTop = clamp(scrollTop, 0, maxScrollTop);
  const isAtTop = normalizedScrollTop <= SCROLL_TOP_EPSILON;

  if (!isScrollable || clientHeight <= 0 || scrollHeight <= 0) {
    return {
      isScrollable: false,
      isAtTop: true,
      showScrollUi: false,
      thumbStyle: {
        height: "0px",
        transform: "translateY(0px)",
      },
    };
  }

  const thumbHeight = clamp((clientHeight / scrollHeight) * clientHeight, MIN_THUMB_HEIGHT, clientHeight);
  const maxThumbTop = Math.max(0, clientHeight - thumbHeight);
  const thumbTop = maxScrollTop === 0 ? 0 : (normalizedScrollTop / maxScrollTop) * maxThumbTop;

  return {
    isScrollable,
    isAtTop,
    showScrollUi: !isAtTop,
    thumbStyle: {
      height: formatPixels(thumbHeight),
      transform: `translateY(${formatPixels(thumbTop)})`,
    },
  };
}
```

- [ ] **Step 2: Run the focused helper test**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/features/mods/modLibraryScrollUi.test.mjs
```

Expected: PASS.

- [ ] **Step 3: Run typecheck**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: PASS.

---

### Task 3: Update Contract Tests for Conditional Scroll UI

**Files:**
- Modify: `src/features/mods/modLibraryBackToTop.test.mjs`
- Inspect: `src/features/mods/modLibraryStickyControls.test.mjs`

- [ ] **Step 1: Replace always-visible button assertions**

In `src/features/mods/modLibraryBackToTop.test.mjs`, replace the test named:

```js
test("mod library page renders a dedicated back-to-top button", () => {
```

with:

```js
test("mod library page renders back-to-top from scroll UI state instead of unconditionally", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");

  assert.match(source, /BackToTopButton/);
  assert.match(source, /showScrollUi\s*\?/);
  assert.match(source, /mod-library__main-floating-actions/);
  assert.doesNotMatch(source, /<div className="mod-library__main-floating-actions">\s*<BackToTopButton/);
});
```

- [ ] **Step 2: Replace fixed-position button contract with state-driven scroll UI contract**

In the same file, replace the test named:

```js
test("back-to-top control is fixed to the viewport bottom-right so it stays visible while scrolling", () => {
```

with:

```js
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
```

- [ ] **Step 3: Update offset test to keep only positioning invariants that still apply**

Replace the test named:

```js
test("back-to-top button offset keeps it clear of the corner for easy clicking", () => {
```

with:

```js
test("back-to-top button keeps the requested comfortable bottom offset when visible", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  // 100px / 80px are the final comfortable bottom offsets requested after the
  // original sticky-controls plan. Do not revert these to the early 12px / 8px
  // draft values from the first pass.
  assert.match(css, /\.mod-library\s*{[\s\S]*?--mod-library-back-to-top-block-offset:\s*100px;/);
  assert.match(
    css,
    /\.mod-library__main-floating-actions[\s\S]*?bottom:\s*var\(--mod-library-back-to-top-block-offset\);/,
  );
  assert.doesNotMatch(css, /\.mod-library__main-floating-actions[\s\S]*?transform:\s*translateX/);
  assert.doesNotMatch(css, /--mod-library-back-to-top-inline-offset/);
  assert.match(
    css,
    /@media\s*\(max-width:\s*640px\)\s*{[\s\S]*?\.mod-library\s*{[\s\S]*?--mod-library-back-to-top-block-offset:\s*80px;/,
  );
});
```

- [ ] **Step 4: Run the focused tests and confirm they fail before implementation**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/features/mods/modLibraryBackToTop.test.mjs src/features/mods/modLibraryScrollUi.test.mjs
```

Expected: `modLibraryScrollUi.test.mjs` should PASS from Task 2, while `modLibraryBackToTop.test.mjs` should FAIL because `ModLibraryPage.tsx` and CSS have not yet been updated.

---

### Task 4: Wire Scroll UI State Into ModLibraryPage

**Files:**
- Modify: `src/features/mods/ModLibraryPage.tsx`
- Inspect: `src/features/mods/BackToTopButton.tsx`
- Test: `src/features/mods/modLibraryBackToTop.test.mjs`

- [ ] **Step 1: Update imports**

In `src/features/mods/ModLibraryPage.tsx`, change the React import:

```ts
import { useMemo, useState, type CSSProperties } from "react";
```

to:

```ts
import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
```

Add the helper import:

```ts
import { getModLibraryScrollUiState } from "./modLibraryScrollUi";
```

- [ ] **Step 2: Add scroll UI state type and initial state**

Below `staggerStyle`, add:

```ts
const initialScrollUiState = getModLibraryScrollUiState({
  scrollTop: 0,
  scrollHeight: 0,
  clientHeight: 0,
});
```

- [ ] **Step 3: Add ref, state, and updater inside `ModLibraryPage`**

Inside `ModLibraryPage`, after the existing `useState` declarations, add:

```ts
  const contentRef = useRef<HTMLDivElement | null>(null);
  const [scrollUiState, setScrollUiState] = useState(initialScrollUiState);

  const updateScrollUiState = useCallback(() => {
    const content = contentRef.current;

    if (!content) {
      setScrollUiState(initialScrollUiState);
      return;
    }

    setScrollUiState(
      getModLibraryScrollUiState({
        scrollTop: content.scrollTop,
        scrollHeight: content.scrollHeight,
        clientHeight: content.clientHeight,
      }),
    );
  }, []);
```

- [ ] **Step 4: Add scroll and resize subscriptions**

Still inside `ModLibraryPage`, below `updateScrollUiState`, add:

```ts
  useEffect(() => {
    const content = contentRef.current;

    if (!content) {
      return undefined;
    }

    let frameId = 0;
    const requestUpdate = () => {
      if (frameId !== 0) {
        return;
      }

      frameId = window.requestAnimationFrame(() => {
        frameId = 0;
        updateScrollUiState();
      });
    };

    const resizeObserver = new ResizeObserver(requestUpdate);
    resizeObserver.observe(content);
    if (content.firstElementChild) {
      resizeObserver.observe(content.firstElementChild);
    }

    content.addEventListener("scroll", requestUpdate, { passive: true });
    requestUpdate();

    return () => {
      content.removeEventListener("scroll", requestUpdate);
      resizeObserver.disconnect();
      if (frameId !== 0) {
        window.cancelAnimationFrame(frameId);
      }
    };
  }, [updateScrollUiState, visibleItems.length]);
```

Reason: `ResizeObserver` catches viewport and content height changes; `visibleItems.length` reconnects observation when the list switches between empty state and grid.

- [ ] **Step 5: Update `handleBackToTop` to use the ref first**

Replace the current target selection inside `handleBackToTop`:

```ts
    const fallbackTarget = document.scrollingElement ?? document.documentElement;
    const target = getModLibraryBackToTopTarget(document, fallbackTarget);
    scrollModLibraryBackToTop(target);
```

with:

```ts
    const fallbackTarget = document.scrollingElement ?? document.documentElement;
    const target = contentRef.current ?? getModLibraryBackToTopTarget(document, fallbackTarget);
    scrollModLibraryBackToTop(target);
```

- [ ] **Step 6: Derive render state before JSX**

Above the `return`, add:

```ts
  const { showScrollUi, thumbStyle } = scrollUiState;
```

- [ ] **Step 7: Wrap the content and conditionally render scroll UI**

Replace:

```tsx
      <div className="mod-library__content">
        <div className="mod-library__main-floating-actions">
          <BackToTopButton onClick={handleBackToTop} />
        </div>
```

with:

```tsx
      <div className="mod-library__content-shell" data-scroll-ui={showScrollUi ? "visible" : "hidden"}>
        <div ref={contentRef} className="mod-library__content">
          {showScrollUi ? (
            <div className="mod-library__main-floating-actions">
              <BackToTopButton onClick={handleBackToTop} />
            </div>
          ) : null}
```

Replace the closing `</div>` for `.mod-library__content` with:

```tsx
        </div>

        {showScrollUi ? (
          <div className="mod-library__scrollbar" aria-hidden="true">
            <div className="mod-library__scrollbar-thumb" style={thumbStyle} />
          </div>
        ) : null}
      </div>
```

The final structure inside `.mod-library__content-shell` must be:

```tsx
<div ref={contentRef} className="mod-library__content">
  {showScrollUi ? <div className="mod-library__main-floating-actions">...</div> : null}
  {visibleItems.length === 0 ? ... : ...}
</div>
{showScrollUi ? <div className="mod-library__scrollbar" aria-hidden="true">...</div> : null}
```

- [ ] **Step 8: Run typecheck**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: PASS. If TypeScript complains about `ResizeObserver` types, confirm `lib.dom` is available in `tsconfig`; do not add custom ambient types unless necessary.

---

### Task 5: Hide Native Scrollbar Visuals and Style Custom Scrollbar

**Files:**
- Modify: `src/features/mods/ModLibraryPage.css`
- Test: `src/features/mods/modLibraryBackToTop.test.mjs`
- Test: `src/features/mods/modLibraryStickyControls.test.mjs`

- [ ] **Step 1: Add the content shell**

In `src/features/mods/ModLibraryPage.css`, add this block before `.mod-library__content`:

```css
.mod-library__content-shell {
  grid-column: 1;
  grid-row: 2;
  position: relative;
  min-width: 0;
  min-height: 0;
}
```

- [ ] **Step 2: Move grid row ownership to the shell**

In `.mod-library__content`, remove:

```css
  grid-column: 1;
  grid-row: 2;
```

Keep:

```css
  position: relative;
  display: grid;
  grid-template-rows: minmax(0, 1fr);
  min-width: 0;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
```

- [ ] **Step 3: Hide native scrollbar visuals**

Replace the `.mod-library__content` scrollbar style block:

```css
.mod-library__content {
  scrollbar-width: thin;
  scrollbar-color: var(--color-border) transparent;
}
```

with:

```css
.mod-library__content {
  scrollbar-width: none;
  scrollbar-gutter: auto;
}
```

Replace:

```css
.mod-library__content::-webkit-scrollbar {
  width: 10px;
  height: 10px;
}
```

with:

```css
.mod-library__content::-webkit-scrollbar {
  width: 0;
  height: 0;
}
```

Delete the remaining `.mod-library__content::-webkit-scrollbar-track`, `.mod-library__content::-webkit-scrollbar-thumb`, `.mod-library__content::-webkit-scrollbar-thumb:hover`, `.mod-library__content::-webkit-scrollbar-corner`, and dark-mode scrollbar-thumb rules for `.mod-library__content`. They no longer apply to the hidden native scrollbar.

- [ ] **Step 4: Add custom scrollbar overlay styles**

Add below the `.mod-library__content::-webkit-scrollbar` block:

```css
.mod-library__scrollbar {
  position: absolute;
  top: 0;
  right: 4px;
  bottom: 0;
  z-index: 15;
  width: 10px;
  pointer-events: none;
}

.mod-library__scrollbar-thumb {
  width: 10px;
  min-height: 36px;
  background: var(--color-border);
  border: 2px solid transparent;
  border-radius: 9999px;
  background-clip: padding-box;
  opacity: 0.9;
  transition:
    background-color 0.18s ease,
    opacity 0.18s ease;
}

:root[data-color-scheme="dark"] .mod-library__scrollbar-thumb {
  background: var(--color-border-muted);
  background-clip: padding-box;
}
```

Do not add track background; the user explicitly does not want the track visible at top, and the overlay itself should stay visually quiet when shown.

- [ ] **Step 5: Preserve button offset behavior**

Keep the current `.mod-library__main-floating-actions` rule using `position: fixed`, `right: var(--layout-page-padding)`, and:

```css
bottom: var(--mod-library-back-to-top-block-offset);
```

Reason: the current branch just committed a fixed bottom-right button positioning change. This task changes visibility, not its approved screen position.

- [ ] **Step 6: Run focused tests**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/features/mods/modLibraryBackToTop.test.mjs src/features/mods/modLibraryScrollUi.test.mjs src/features/mods/modLibraryStickyControls.test.mjs
```

Expected: PASS. If `modLibraryStickyControls.test.mjs` fails because it expects `.mod-library__content` to own `grid-row: 2`, update it to expect `.mod-library__content-shell` to own `grid-row: 2` and keep `.mod-library__content` as the scroll container.

---

### Task 6: Add Drag Support for the Custom Scrollbar Thumb

**Files:**
- Modify: `src/features/mods/ModLibraryPage.tsx`
- Test: `src/features/mods/modLibraryBackToTop.test.mjs` or create `src/features/mods/modLibraryCustomScrollbar.test.mjs`

- [ ] **Step 1: Add a pointer drag handler**

Inside `ModLibraryPage`, below `handleBackToTop`, add:

```ts
  const handleScrollbarPointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    const content = contentRef.current;

    if (!content) {
      return;
    }

    event.preventDefault();
    const startY = event.clientY;
    const startScrollTop = content.scrollTop;
    const maxScrollTop = Math.max(0, content.scrollHeight - content.clientHeight);
    const thumbHeight = Number.parseFloat(scrollUiState.thumbStyle.height);
    const maxThumbTop = Math.max(1, content.clientHeight - thumbHeight);

    const handlePointerMove = (moveEvent: PointerEvent) => {
      const deltaY = moveEvent.clientY - startY;
      content.scrollTop = startScrollTop + (deltaY / maxThumbTop) * maxScrollTop;
    };

    const stopDragging = () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", stopDragging);
      window.removeEventListener("pointercancel", stopDragging);
    };

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", stopDragging);
    window.addEventListener("pointercancel", stopDragging);
  };
```

Add `React` namespace type support by changing the import to include `type PointerEvent` only if needed. Prefer:

```ts
import { ..., type CSSProperties, type PointerEvent } from "react";
```

and use `PointerEvent<HTMLDivElement>` in the handler if the namespace form is not already available.

- [ ] **Step 2: Make the custom scrollbar thumb interactive**

Change:

```tsx
<div className="mod-library__scrollbar" aria-hidden="true">
  <div className="mod-library__scrollbar-thumb" style={thumbStyle} />
</div>
```

to:

```tsx
<div className="mod-library__scrollbar" aria-hidden="true">
  <div
    className="mod-library__scrollbar-thumb"
    style={thumbStyle}
    onPointerDown={handleScrollbarPointerDown}
  />
</div>
```

- [ ] **Step 3: Enable pointer events on the thumb only**

In `ModLibraryPage.css`, update:

```css
.mod-library__scrollbar {
  pointer-events: none;
}
```

Keep it on the track, then add:

```css
.mod-library__scrollbar-thumb {
  pointer-events: auto;
  cursor: grab;
}

.mod-library__scrollbar-thumb:active {
  cursor: grabbing;
  background: var(--color-text-muted);
}
```

- [ ] **Step 4: Add a source contract test for drag support**

Append to `src/features/mods/modLibraryBackToTop.test.mjs`:

```js
test("custom scrollbar thumb supports pointer dragging without exposing a native track", () => {
  const source = readProjectFile("src/features/mods/ModLibraryPage.tsx");
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.match(source, /handleScrollbarPointerDown/);
  assert.match(source, /onPointerDown={handleScrollbarPointerDown}/);
  assert.match(source, /window\.addEventListener\("pointermove"/);
  assert.match(source, /content\.scrollTop\s*=/);
  assert.match(css, /\.mod-library__scrollbar\s*{[\s\S]*?pointer-events:\s*none;/);
  assert.match(css, /\.mod-library__scrollbar-thumb\s*{[\s\S]*?pointer-events:\s*auto;/);
  assert.match(css, /\.mod-library__scrollbar-thumb\s*{[\s\S]*?cursor:\s*grab;/);
});
```

- [ ] **Step 5: Run focused tests and typecheck**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/features/mods/modLibraryBackToTop.test.mjs src/features/mods/modLibraryScrollUi.test.mjs
cmd /c corepack pnpm run typecheck
```

Expected: PASS.

---

### Task 7: Browser Smoke Verification on Port 1420

**Files:**
- No planned source edits. Modify only if smoke test reveals a concrete bug.

- [ ] **Step 1: Use the existing 1420 dev server or start it**

If `http://localhost:1420/` is already serving the current app, reuse it.

If not running, start:

```powershell
cmd /c corepack pnpm run dev -- --host 127.0.0.1 --port 1420
```

Expected: Vite serves the app at `http://127.0.0.1:1420/`.

- [ ] **Step 2: Navigate through the app sidebar**

Open:

```text
http://localhost:1420/
```

Click the sidebar button named `Mod 管理`. Do not rely on direct `/mods` URL loading.

- [ ] **Step 3: Verify initial top state**

At the top of the Mod list, run this in the browser page context:

```js
(() => {
  const content = document.querySelector(".mod-library__content");
  const button = document.querySelector(".mod-library__back-to-top");
  const customScrollbar = document.querySelector(".mod-library__scrollbar");
  const styles = content ? getComputedStyle(content) : null;
  return {
    scrollTop: content?.scrollTop ?? null,
    canScrollY: content ? content.scrollHeight > content.clientHeight : null,
    buttonVisible: !!button,
    customScrollbarVisible: !!customScrollbar,
    scrollbarWidth: styles?.scrollbarWidth ?? null,
    scrollbarGutter: styles?.scrollbarGutter ?? null,
  };
})();
```

Expected:

```js
{
  scrollTop: 0,
  canScrollY: true,
  buttonVisible: false,
  customScrollbarVisible: false,
  scrollbarWidth: "none",
  scrollbarGutter: "auto"
}
```

- [ ] **Step 4: Verify after scrolling down**

Scroll inside the card list area with the wheel or touchpad, then run:

```js
(() => {
  const content = document.querySelector(".mod-library__content");
  const button = document.querySelector(".mod-library__back-to-top");
  const thumb = document.querySelector(".mod-library__scrollbar-thumb");
  return {
    scrollTop: content?.scrollTop ?? null,
    buttonVisible: !!button,
    thumbVisible: !!thumb,
    thumbHeight: thumb ? getComputedStyle(thumb).height : null,
    thumbTransform: thumb ? getComputedStyle(thumb).transform : null,
  };
})();
```

Expected:

- `scrollTop > 1`
- `buttonVisible === true`
- `thumbVisible === true`
- `thumbHeight` is at least `36px`
- `thumbTransform` is not `none`

- [ ] **Step 5: Verify upward scroll before top**

Scroll upward but stop before the top. Run the same snippet from Step 4.

Expected:

- `scrollTop > 1`
- `buttonVisible === true`
- `thumbVisible === true`

- [ ] **Step 6: Verify return to top**

Click the back-to-top button or scroll to the top. After smooth scroll completes, run the Step 3 snippet.

Expected:

- `scrollTop <= 1`
- `buttonVisible === false`
- `customScrollbarVisible === false`

- [ ] **Step 7: Verify filtering and non-scrollable states**

Use the search box to filter the list to zero or very few items.

Run the Step 3 snippet.

Expected:

- `canScrollY === false` when content is shorter than the viewport.
- `buttonVisible === false`
- `customScrollbarVisible === false`

- [ ] **Step 8: Verify responsive viewports**

Repeat Steps 3 through 6 at:

```text
1440x900
1366x768
1280x800
640x812
375x812
```

Expected:

- Top state hides both scroll UI pieces.
- Scrolled state shows both scroll UI pieces.
- Back-to-top button does not overlap search input or quick action buttons.
- Custom scrollbar does not create horizontal overflow.

- [ ] **Step 9: Verify thumb drag**

At desktop width, scroll down until the custom thumb is visible. Drag the custom thumb downward and upward.

Expected:

- Dragging downward increases `.mod-library__content.scrollTop`.
- Dragging upward decreases `.mod-library__content.scrollTop`.
- Reaching the top hides both the thumb and button.

---

### Task 8: Final Verification and Commit

**Files:**
- Stage only files changed for this task.

- [ ] **Step 1: Run focused Mod tests**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/features/mods/modLibraryBackToTop.test.mjs src/features/mods/modLibraryScrollUi.test.mjs src/features/mods/modLibraryStickyControls.test.mjs
```

Expected: PASS.

- [ ] **Step 2: Run all frontend tests**

Run:

```powershell
cmd /c corepack pnpm run test
```

Expected: PASS.

- [ ] **Step 3: Run frontend typecheck**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: PASS.

- [ ] **Step 4: Run frontend lint**

Run:

```powershell
cmd /c corepack pnpm run lint
```

Expected: PASS.

- [ ] **Step 5: Run frontend build**

Run:

```powershell
cmd /c corepack pnpm run build
```

Expected: PASS.

- [ ] **Step 6: Run unified project verification**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected: PASS.

- [ ] **Step 7: Review changed files**

Run:

```powershell
git status --short --branch
git diff --stat
git diff -- src/features/mods/ModLibraryPage.tsx src/features/mods/ModLibraryPage.css src/features/mods/modLibraryBackToTop.test.mjs src/features/mods/modLibraryScrollUi.ts src/features/mods/modLibraryScrollUi.test.mjs
```

Expected:

- Changes are limited to the planned frontend files plus planning documentation if this plan was updated during execution.
- No `.planning/`, `.plan-attestation`, cache, generated logs, `dist/`, `target/`, real mod packages, real saves, token files, or local private paths are staged.

- [ ] **Step 8: Commit implementation**

Run:

```powershell
git add src/features/mods/ModLibraryPage.tsx src/features/mods/ModLibraryPage.css src/features/mods/modLibraryBackToTop.test.mjs src/features/mods/modLibraryScrollUi.ts src/features/mods/modLibraryScrollUi.test.mjs
git commit -m "fix: 顶部隐藏 Mod 列表滚动 UI"
```

If planning docs are intentionally committed in this branch, stage them in a separate governance/docs commit and mention that `.planning/` remains untracked runtime state.

---

## Self-Review Notes

- **Spec coverage:** Covers both reported defects: back-to-top floating control and right-side native scrollbar visual track. Both are now controlled by the same `showScrollUi` state.
- **Root cause coverage:** Removes unconditional button rendering and avoids relying on native scrollbar track behavior that CSS cannot reliably bind to `scrollTop`.
- **Scope check:** Single frontend page task. No Rust, Tauri command, InstallPlan, manifest, backup, rollback, game adapter, real file write, real save, or player data path changes.
- **Accessibility:** Native scrolling remains on `.mod-library__content`; custom scrollbar overlay is `aria-hidden`; back-to-top button keeps its existing accessible label.
- **Pointer support:** Plan includes thumb dragging so hiding the native scrollbar does not remove mouse drag capability.
- **Testing:** Pure helper tests cover the state math; source contract tests protect wiring and CSS; browser smoke tests verify actual visual behavior at port 1420.
- **Placeholder scan:** No placeholder markers remain. Every task has exact file paths, code snippets, commands, and expected outcomes.
- **Known risk:** Source regex tests cannot prove visual scrollbar absence by themselves; browser smoke verification is mandatory before claiming the bug is fixed.
