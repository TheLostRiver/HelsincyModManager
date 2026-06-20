# Route Transition Animation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make sidebar route changes animate symmetrically when entering and leaving the Mod management page, while keeping page-specific stagger animations as content-level detail.

**Architecture:** Route changes should be animated at the routing outlet layer, not inside individual feature pages. The outlet will keep the previous route mounted briefly as an exiting layer, mount the next route as an entering layer, and remove the old layer after the exit duration. Existing Mod page stagger animation remains an entry detail for Mod content.

**Tech Stack:** React 19, TypeScript, Vite, CSS animations, Node built-in test runner.

---

## File Structure

- Modify `src/app/routing/RouterOutlet.tsx`: replace immediate route rendering with an animated route stack.
- Create `src/app/routing/routeTransition.ts`: pure transition state reducer for route layer state.
- Create `src/app/routing/routeTransition.test.mjs`: regression tests for route enter/exit state.
- Create `src/app/routing/RouterOutlet.css`: route-layer layout and enter/exit animation classes.
- Modify `src/main.tsx`: import the new routing CSS.
- Leave `src/features/mods/ModLibraryPage.css` stagger animation intact; do not move feature-specific card/tool stagger in this task.
- Do not modify sidebar button components unless implementation reveals a compile-time type issue; navigation should still call `navigate(path)`.

## Important Context

Current behavior:

- `src/app/routing/AppRouteProvider.tsx` immediately calls `setCurrentPath(targetRoute.path)` inside `navigate`.
- `src/app/routing/RouterOutlet.tsx` immediately renders `<RouteElement />` for `currentRoute`.
- `src/features/mods/ModLibraryPage.css` defines `.anim-stagger-item` with only an entrance animation.
- Therefore, switching into `/mods` animates because Mod page elements mount; switching away from `/mods` has no exit animation because the Mod page unmounts immediately.

The fix is not to add more exit CSS inside `ModLibraryPage`; it has no lifecycle window to run after route replacement. The lifecycle window belongs in `RouterOutlet`.

---

### Task 1: Add Pure Route Transition State Tests

**Files:**
- Create: `src/app/routing/routeTransition.test.mjs`
- Create: `src/app/routing/routeTransition.ts`

- [x] **Step 1: Write the failing test**

Create `src/app/routing/routeTransition.test.mjs`:

```js
import assert from "node:assert/strict";
import { test } from "node:test";
import { beginRouteTransition, completeRouteExit, createInitialRouteLayer } from "./routeTransition.ts";

function serializeLayers(layers) {
  return layers.map((layer) => ({
    key: layer.key,
    routeId: layer.route.id,
    phase: layer.phase,
  }));
}

const dashboardRoute = {
  id: "dashboard",
  path: "/",
  element: function DashboardRouteElement() {
    return null;
  },
};

const modsRoute = {
  id: "mods",
  path: "/mods",
  element: function ModsRouteElement() {
    return null;
  },
};

test("creates a stable initial route layer", () => {
  const layer = createInitialRouteLayer(dashboardRoute);

  assert.equal(layer.key, "dashboard:/");
  assert.equal(layer.route, dashboardRoute);
  assert.equal(layer.phase, "active");
});

test("beginRouteTransition keeps the old route exiting and the target route entering", () => {
  const currentLayers = [createInitialRouteLayer(dashboardRoute)];

  const nextLayers = beginRouteTransition(currentLayers, modsRoute);

  assert.deepEqual(serializeLayers(nextLayers), [
    { key: "dashboard:/", routeId: "dashboard", phase: "exiting" },
    { key: "mods:/mods", routeId: "mods", phase: "entering" },
  ]);
});

test("beginRouteTransition ignores navigation to the already visible route", () => {
  const currentLayers = [createInitialRouteLayer(dashboardRoute)];

  const nextLayers = beginRouteTransition(currentLayers, dashboardRoute);

  assert.deepEqual(serializeLayers(nextLayers), [{ key: "dashboard:/", routeId: "dashboard", phase: "active" }]);
});

test("beginRouteTransition replaces an in-flight entering route with the newest target", () => {
  const inFlightLayers = beginRouteTransition([createInitialRouteLayer(dashboardRoute)], modsRoute);

  const nextLayers = beginRouteTransition(inFlightLayers, dashboardRoute);

  assert.deepEqual(serializeLayers(nextLayers), [{ key: "dashboard:/", routeId: "dashboard", phase: "active" }]);
});

test("completeRouteExit removes exiting layers and promotes entering route to active", () => {
  const inFlightLayers = beginRouteTransition([createInitialRouteLayer(dashboardRoute)], modsRoute);

  const nextLayers = completeRouteExit(inFlightLayers);

  assert.deepEqual(serializeLayers(nextLayers), [{ key: "mods:/mods", routeId: "mods", phase: "active" }]);
});
```

- [x] **Step 2: Run the test to verify it fails**

Run:

```powershell
cmd /c corepack pnpm run test
```

Expected: FAIL because `src/app/routing/routeTransition.ts` does not export the tested functions yet.

- [x] **Step 3: Add the minimal transition state module**

Create `src/app/routing/routeTransition.ts`:

```ts
import type { AppRoute } from "./routeTypes";

export type RouteLayerPhase = "active" | "entering" | "exiting";

export type RouteLayer = {
  key: string;
  route: AppRoute;
  phase: RouteLayerPhase;
};

export function createRouteLayerKey(route: AppRoute) {
  return `${route.id}:${route.path}`;
}

export function createInitialRouteLayer(route: AppRoute): RouteLayer {
  return {
    key: createRouteLayerKey(route),
    route,
    phase: "active",
  };
}

export function beginRouteTransition(currentLayers: readonly RouteLayer[], targetRoute: AppRoute): RouteLayer[] {
  const targetKey = createRouteLayerKey(targetRoute);
  const visibleLayer = currentLayers.find((layer) => layer.phase !== "exiting") ?? currentLayers.at(-1);

  if (!visibleLayer || visibleLayer.key === targetKey) {
    return [createInitialRouteLayer(targetRoute)];
  }

  return [
    {
      ...visibleLayer,
      phase: "exiting",
    },
    {
      key: targetKey,
      route: targetRoute,
      phase: "entering",
    },
  ];
}

export function completeRouteExit(currentLayers: readonly RouteLayer[]): RouteLayer[] {
  const visibleLayer = currentLayers.findLast((layer) => layer.phase !== "exiting");

  if (!visibleLayer) {
    return [];
  }

  return [
    {
      ...visibleLayer,
      phase: "active",
    },
  ];
}
```

- [x] **Step 4: Run the test to verify it passes**

Run:

```powershell
cmd /c corepack pnpm run test
```

Expected: PASS with the existing Mod selection tests and the new route transition tests.

- [x] **Step 5: Commit**

Run:

```powershell
git add src/app/routing/routeTransition.ts src/app/routing/routeTransition.test.mjs
git commit -m "test: 覆盖路由切换状态"
```

---

### Task 2: Implement Animated Router Outlet

**Files:**
- Modify: `src/app/routing/RouterOutlet.tsx`
- Modify: `src/app/routing/routeTransition.ts`

- [x] **Step 1: Write the failing behavior test for reduced edge cases**

Append this test to `src/app/routing/routeTransition.test.mjs`:

```js
test("completeRouteExit keeps only the newest non-exiting layer", () => {
  const profileRoute = {
    id: "profiles",
    path: "/profiles",
    element: function ProfilesRouteElement() {
      return null;
    },
  };
  const layers = [
    { key: "dashboard:/", route: dashboardRoute, phase: "exiting" },
    { key: "mods:/mods", route: modsRoute, phase: "exiting" },
    { key: "profiles:/profiles", route: profileRoute, phase: "entering" },
  ];

  const nextLayers = completeRouteExit(layers);

  assert.deepEqual(serializeLayers(nextLayers), [{ key: "profiles:/profiles", routeId: "profiles", phase: "active" }]);
});
```

Run:

```powershell
cmd /c corepack pnpm run test
```

Expected: PASS if Task 1 implementation already uses `findLast`. If this fails, update `completeRouteExit` exactly as shown in Task 1.

- [x] **Step 2: Replace `RouterOutlet` with route layer rendering**

Update `src/app/routing/RouterOutlet.tsx`:

```tsx
import { useEffect, useMemo, useState } from "react";
import {
  beginRouteTransition,
  completeRouteExit,
  createInitialRouteLayer,
  createRouteLayerKey,
  type RouteLayer,
} from "./routeTransition";
import { useAppRoute } from "./useAppRoute";

const routeExitDurationMs = 240;

export function RouterOutlet() {
  const { currentRoute } = useAppRoute();
  const currentRouteKey = createRouteLayerKey(currentRoute);
  const [routeLayers, setRouteLayers] = useState<RouteLayer[]>(() => [createInitialRouteLayer(currentRoute)]);

  const visibleRouteKey = useMemo(() => {
    return routeLayers.findLast((layer) => layer.phase !== "exiting")?.key;
  }, [routeLayers]);

  useEffect(() => {
    if (visibleRouteKey === currentRouteKey) {
      return;
    }

    setRouteLayers((previousLayers) => beginRouteTransition(previousLayers, currentRoute));
  }, [currentRoute, currentRouteKey, visibleRouteKey]);

  useEffect(() => {
    if (!routeLayers.some((layer) => layer.phase === "exiting")) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      setRouteLayers((previousLayers) => completeRouteExit(previousLayers));
    }, routeExitDurationMs);

    return () => window.clearTimeout(timeoutId);
  }, [routeLayers]);

  return (
    <div className="route-transition" aria-live="polite">
      {routeLayers.map((layer) => {
        const RouteElement = layer.route.element;
        const isHiddenFromA11y = layer.phase === "exiting";

        return (
          <div
            key={layer.key}
            className={`route-transition__layer is-${layer.phase}`}
            aria-hidden={isHiddenFromA11y || undefined}
            data-route-id={layer.route.id}
          >
            <RouteElement />
          </div>
        );
      })}
    </div>
  );
}
```

- [x] **Step 3: Run typecheck**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: PASS. If TypeScript rejects `findLast` on the configured lib, replace `findLast` with this helper inside `routeTransition.ts` and `RouterOutlet.tsx`:

```ts
function findLastVisibleLayer(layers: readonly RouteLayer[]) {
  for (let index = layers.length - 1; index >= 0; index -= 1) {
    const layer = layers[index];
    if (layer.phase !== "exiting") {
      return layer;
    }
  }

  return undefined;
}
```

- [x] **Step 4: Commit**

Run:

```powershell
git add src/app/routing/RouterOutlet.tsx src/app/routing/routeTransition.ts src/app/routing/routeTransition.test.mjs
git commit -m "feat: 添加路由层过渡状态"
```

---

### Task 3: Add Route Transition CSS

**Files:**
- Create: `src/app/routing/RouterOutlet.css`
- Modify: `src/main.tsx`

- [x] **Step 1: Create route transition stylesheet**

Create `src/app/routing/RouterOutlet.css`:

```css
.route-transition {
  position: relative;
  display: grid;
  min-width: 0;
  min-height: 0;
}

.route-transition__layer {
  grid-area: 1 / 1;
  min-width: 0;
  min-height: 0;
}

.route-transition__layer.is-entering {
  animation: route-layer-enter 240ms cubic-bezier(0.2, 0.8, 0.2, 1) both;
}

.route-transition__layer.is-exiting {
  z-index: 1;
  pointer-events: none;
  animation: route-layer-exit 180ms cubic-bezier(0.4, 0, 1, 1) both;
}

.route-transition__layer.is-active {
  z-index: 0;
}

@keyframes route-layer-enter {
  from {
    opacity: 0;
    transform: translateY(10px);
  }

  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes route-layer-exit {
  from {
    opacity: 1;
    transform: translateY(0);
  }

  to {
    opacity: 0;
    transform: translateY(-6px);
  }
}

@media (prefers-reduced-motion: reduce) {
  .route-transition__layer.is-entering,
  .route-transition__layer.is-exiting {
    animation-duration: 0.01ms;
    transform: none;
  }
}
```

- [x] **Step 2: Import the stylesheet**

Update `src/main.tsx` imports so routing CSS loads with app shell CSS:

```tsx
import "./app/frame/AppFrame.css";
import "./app/frame/ThemeMenu.css";
import "./app/routing/RouterOutlet.css";
import "./app/shell/sidebar-mode-control/SidebarModeControl.css";
```

- [x] **Step 3: Run build**

Run:

```powershell
cmd /c corepack pnpm run build
```

Expected: PASS with Vite build output and no TypeScript errors.

- [x] **Step 4: Commit**

Run:

```powershell
git add src/app/routing/RouterOutlet.css src/main.tsx
git commit -m "style: 添加路由切换动画"
```

---

### Task 4: Verify Mod Page Stagger Does Not Fight Route Exit

**Files:**
- Inspect: `src/features/mods/ModLibraryPage.css`
- Modify only if necessary: `src/features/mods/ModLibraryPage.css`

- [x] **Step 1: Check whether Mod page stagger reruns during exit**

Run the app:

```powershell
cmd /c corepack pnpm run dev
```

Open `http://localhost:1420`.

Manual check:

- Start at Dashboard.
- Click `Mod 管理`.
- Confirm route layer enters and Mod toolbar/cards still stagger in.
- Click `工作台`.
- Confirm Mod page fades/slides out instead of disappearing immediately.

Expected: Mod content should not restart the stagger while exiting because the exiting layer keeps already-mounted DOM nodes.

- [x] **Step 2: If layout overlap causes scrollbars, constrain exiting layer**

If the exiting route layer creates temporary scrollbars or changes layout height, update `src/app/routing/RouterOutlet.css`:

```css
.route-transition:has(.route-transition__layer.is-exiting) {
  overflow: hidden;
}
```

If `:has()` support is a concern in the Tauri WebView target, use this instead by adding a `data-transitioning` attribute in `RouterOutlet.tsx`:

```tsx
<div className="route-transition" data-transitioning={routeLayers.some((layer) => layer.phase === "exiting") || undefined} aria-live="polite">
```

And add CSS:

```css
.route-transition[data-transitioning="true"] {
  overflow: hidden;
}
```

- [x] **Step 3: Do not add exit CSS to `.anim-stagger-item`**

Keep `src/features/mods/ModLibraryPage.css` page-content animation as-is:

```css
.anim-stagger-item {
  opacity: 0;
  animation: libraryStaggerFadeIn 0.35s cubic-bezier(0.2, 0.8, 0.2, 1) forwards;
  animation-delay: calc(var(--stagger-idx, 0) * 0.03s);
}
```

Reason: the exit animation is now owned by route layers. Adding an exit animation here would duplicate responsibilities and risk Mod-only behavior drift.

- [x] **Step 4: Commit only if CSS or TS changed**

If Step 2 required a change, run:

```powershell
git add src/app/routing/RouterOutlet.css src/app/routing/RouterOutlet.tsx
git commit -m "fix: 稳定路由过渡布局"
```

If no changes were required, do not create an empty commit.

---

### Task 5: Final Verification

**Files:**
- No new code files unless fixes are discovered during verification.

- [ ] **Step 1: Run focused frontend tests**

Run:

```powershell
cmd /c corepack pnpm run test
```

Expected: PASS. The route transition tests and Mod selection tests should all pass.

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

- [ ] **Step 6: Manual browser smoke test**

Run:

```powershell
cmd /c corepack pnpm run dev
```

Manual checks:

- `工作台` -> `Mod 管理`: page-level enter animation runs; Mod content stagger still runs.
- `Mod 管理` -> `工作台`: Mod page exits visibly instead of disappearing.
- Rapidly click `工作台` and `Mod 管理`: no duplicate permanent layers remain.
- With reduced motion enabled at OS/browser level: route transitions become effectively instant.

- [ ] **Step 7: Commit any final fixes**

If verification required changes, commit them:

```powershell
git add src/app/routing src/main.tsx
git commit -m "fix: 完善路由过渡验证问题"
```

If no changes were required, do not create an empty commit.

---

## Self-Review Notes

- Spec coverage: The plan addresses the root cause by moving enter/exit ownership to the route outlet. It preserves the existing Mod stagger animation and avoids scattering route exit logic into feature pages.
- Placeholder scan: No implementation step uses placeholder instructions; code snippets and commands are concrete.
- Type consistency: `RouteLayer`, `RouteLayerPhase`, `createInitialRouteLayer`, `beginRouteTransition`, and `completeRouteExit` are defined before use and reused consistently.
- Scope check: This is one frontend routing animation task. It does not touch Tauri commands, file writes, install plans, game adapters, backups, or other high-risk areas.
