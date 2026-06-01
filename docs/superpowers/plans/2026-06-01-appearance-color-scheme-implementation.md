# 黑白主题切换 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落地真实可用的 `light` / `dark` / `system` 颜色方案切换，并用已确认的 v9 顶部下拉菜单替换当前静态 Sun / Moon 按钮。

**Architecture:** 将颜色方案状态、持久化、系统媒体查询监听和 DOM `data-color-scheme` 写入集中在 `src/app/appearance/`。顶部菜单只消费 `useColorScheme()`，不直接读写 `localStorage`，也不接触 sidebar、Dashboard 或游戏适配规则。颜色变化通过 `tokens.css` 语义变量驱动，业务页面不按主题分支。

**Tech Stack:** React 19, TypeScript strict, Vite, lucide-react, CSS variables, localStorage, `window.matchMedia`.

---

## File Structure

Create:

```text
src/app/appearance/colorSchemeTypes.ts
src/app/appearance/colorSchemeStorage.ts
src/app/appearance/ColorSchemeProvider.tsx
src/app/appearance/useColorScheme.ts
src/app/frame/ThemeMenu.tsx
src/app/frame/ThemeMenu.css
```

Modify:

```text
src/App.tsx
src/main.tsx
src/app/frame/AppHeader.tsx
src/app/frame/AppFrame.css
src/shared/styles/tokens.css
```

Responsibilities:

- `colorSchemeTypes.ts`: 类型、默认值、合法值判断。
- `colorSchemeStorage.ts`: 本地存储读写、旧裸字符串兼容、异常回退。
- `ColorSchemeProvider.tsx`: React context、系统偏好监听、`document.documentElement.dataset.colorScheme` 写入。
- `useColorScheme.ts`: hook 出口，保证组件不直接依赖 context 实现细节。
- `ThemeMenu.tsx`: 顶部主题下拉 UI，采用 v9 动效基线。
- `ThemeMenu.css`: 主题菜单局部样式，不污染业务页面。
- `tokens.css`: 补齐浅色 / 深色语义 token。
- `AppHeader.tsx`: 删除静态 `theme-toggle`，组合 `ThemeMenu`。
- `AppFrame.css`: 移除旧 `.theme-toggle` / `.theme-button` 样式，保留通用 `.icon-button`。

---

### Task 1: Add Color Scheme Types And Storage

**Files:**

- Create: `src/app/appearance/colorSchemeTypes.ts`
- Create: `src/app/appearance/colorSchemeStorage.ts`

- [ ] **Step 1: Create `colorSchemeTypes.ts`**

Create `src/app/appearance/colorSchemeTypes.ts`:

```ts
export const colorSchemePreferences = ["light", "dark", "system"] as const;

export type ColorSchemePreference = (typeof colorSchemePreferences)[number];

export type EffectiveColorScheme = "light" | "dark";

export type PersistedColorSchemeSettings = {
  version: 1;
  preference: ColorSchemePreference;
};

export const defaultColorSchemePreference: ColorSchemePreference = "system";

export function isColorSchemePreference(value: unknown): value is ColorSchemePreference {
  return value === "light" || value === "dark" || value === "system";
}
```

- [ ] **Step 2: Create `colorSchemeStorage.ts`**

Create `src/app/appearance/colorSchemeStorage.ts`:

```ts
import {
  defaultColorSchemePreference,
  isColorSchemePreference,
  type ColorSchemePreference,
  type PersistedColorSchemeSettings,
} from "./colorSchemeTypes";

const storageKey = "helsincy.colorSchemePreference";

export function readPersistedColorSchemePreference(): ColorSchemePreference {
  try {
    const rawValue = window.localStorage.getItem(storageKey);
    if (rawValue === null) {
      return defaultColorSchemePreference;
    }

    if (isColorSchemePreference(rawValue)) {
      return rawValue;
    }

    const parsedValue = JSON.parse(rawValue) as Partial<PersistedColorSchemeSettings>;
    return parsedValue.version === 1 && isColorSchemePreference(parsedValue.preference)
      ? parsedValue.preference
      : defaultColorSchemePreference;
  } catch {
    return defaultColorSchemePreference;
  }
}

export function writePersistedColorSchemePreference(preference: ColorSchemePreference) {
  try {
    const value: PersistedColorSchemeSettings = { version: 1, preference };
    window.localStorage.setItem(storageKey, JSON.stringify(value));
  } catch {
    return;
  }
}
```

- [ ] **Step 3: Run TypeScript check**

Run:

```powershell
pnpm typecheck
```

Expected:

```text
tsc --noEmit
```

Exit code must be `0`.

- [ ] **Step 4: Commit Task 1**

Run:

```powershell
git add src/app/appearance/colorSchemeTypes.ts src/app/appearance/colorSchemeStorage.ts
git commit -m "feat: add color scheme preference storage"
```

Expected: commit succeeds and only the two new appearance files are included.

---

### Task 2: Add ColorSchemeProvider And Hook

**Files:**

- Create: `src/app/appearance/ColorSchemeProvider.tsx`
- Create: `src/app/appearance/useColorScheme.ts`
- Modify: `src/App.tsx`

- [ ] **Step 1: Create `ColorSchemeProvider.tsx`**

Create `src/app/appearance/ColorSchemeProvider.tsx`:

```tsx
import {
  createContext,
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  readPersistedColorSchemePreference,
  writePersistedColorSchemePreference,
} from "./colorSchemeStorage";
import type { ColorSchemePreference, EffectiveColorScheme } from "./colorSchemeTypes";

type ColorSchemeContextValue = {
  preference: ColorSchemePreference;
  effective: EffectiveColorScheme;
  setPreference: (preference: ColorSchemePreference) => void;
};

export const ColorSchemeContext = createContext<ColorSchemeContextValue | null>(null);

type ColorSchemeProviderProps = {
  children: ReactNode;
};

export function ColorSchemeProvider({ children }: ColorSchemeProviderProps) {
  const [preference, setPreferenceState] = useState<ColorSchemePreference>(
    readPersistedColorSchemePreference,
  );
  const [systemScheme, setSystemScheme] = useState<EffectiveColorScheme>(readSystemColorScheme);

  useEffect(() => {
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = (event: MediaQueryListEvent) => {
      setSystemScheme(event.matches ? "dark" : "light");
    };

    setSystemScheme(query.matches ? "dark" : "light");
    query.addEventListener("change", handleChange);
    return () => query.removeEventListener("change", handleChange);
  }, []);

  const effective = preference === "system" ? systemScheme : preference;

  useEffect(() => {
    document.documentElement.dataset.colorScheme = effective;
  }, [effective]);

  const setPreference = useCallback((nextPreference: ColorSchemePreference) => {
    setPreferenceState(nextPreference);
    writePersistedColorSchemePreference(nextPreference);
  }, []);

  const value = useMemo(
    () => ({ effective, preference, setPreference }),
    [effective, preference, setPreference],
  );

  return <ColorSchemeContext.Provider value={value}>{children}</ColorSchemeContext.Provider>;
}

function readSystemColorScheme(): EffectiveColorScheme {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return "light";
  }

  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}
```

- [ ] **Step 2: Create `useColorScheme.ts`**

Create `src/app/appearance/useColorScheme.ts`:

```ts
import { useContext } from "react";
import { ColorSchemeContext } from "./ColorSchemeProvider";

export function useColorScheme() {
  const value = useContext(ColorSchemeContext);

  if (value === null) {
    throw new Error("useColorScheme must be used within ColorSchemeProvider");
  }

  return value;
}
```

- [ ] **Step 3: Wrap App With ColorSchemeProvider**

Modify `src/App.tsx` to:

```tsx
import { AppShell } from "./app/AppShell";
import { ColorSchemeProvider } from "./app/appearance/ColorSchemeProvider";
import { SidebarModeProvider } from "./app/shell/SidebarModeProvider";
import { DashboardPage } from "./features/dashboard/DashboardPage";

export function App() {
  return (
    <ColorSchemeProvider>
      <SidebarModeProvider>
        <AppShell>
          <DashboardPage />
        </AppShell>
      </SidebarModeProvider>
    </ColorSchemeProvider>
  );
}
```

- [ ] **Step 4: Run TypeScript check**

Run:

```powershell
pnpm typecheck
```

Expected:

```text
tsc --noEmit
```

Exit code must be `0`.

- [ ] **Step 5: Commit Task 2**

Run:

```powershell
git add src/App.tsx src/app/appearance/ColorSchemeProvider.tsx src/app/appearance/useColorScheme.ts
git commit -m "feat: add color scheme provider"
```

Expected: commit succeeds and `App.tsx` is the only modified existing source file.

---

### Task 3: Add Light And Dark Semantic Tokens

**Files:**

- Modify: `src/shared/styles/tokens.css`

- [ ] **Step 1: Replace root token block with explicit light and dark blocks**

Modify `src/shared/styles/tokens.css` so it keeps the current light values under both `:root` and `:root[data-color-scheme="light"]`, then adds a dark block:

```css
:root,
:root[data-color-scheme="light"] {
  --color-bg: #f8fafc;
  --color-surface: #ffffff;
  --color-surface-raised: #ffffffd9;
  --color-surface-muted: #f6f8fa;
  --color-surface-subtle: #f1f5f9;
  --color-border: #d8dee8;
  --color-border-muted: #e8edf4;
  --color-text: #0f172a;
  --color-text-soft: #334155;
  --color-text-muted: #64748b;
  --color-accent: #0062ff;
  --color-accent-strong: #0969ff;
  --color-accent-weak: #eaf3ff;
  --color-warning-text: #92400e;
  --color-warning-bg: #fff4cf;
  --color-warning-dot: #f59e0b;
  --color-neutral-text: #475569;
  --color-neutral-bg: #f1f5f9;
  --color-neutral-dot: #64748b;
  --radius-nav: 8px;
  --radius-card: 28px;
  --radius-panel: 24px;
  --radius-pill: 999px;
  --radius-inner: 8px;
  --space-page: 28px;
  --space-content-gap: 20px;
  --shadow-soft: 0 18px 45px #0f172a12;
  --shadow-panel: 0 24px 70px #0f172a18;
  --shadow-card: 0 22px 60px #0f172a14;
  --shadow-control: 0 8px 20px #0062ff26;
  --focus-ring: #93c5fd;
}

:root[data-color-scheme="dark"] {
  --color-bg: #0f172a;
  --color-surface: #1e293b;
  --color-surface-raised: #1e293bd9;
  --color-surface-muted: #172033;
  --color-surface-subtle: #111827;
  --color-border: #334155;
  --color-border-muted: #2b3a50;
  --color-text: #e2e8f0;
  --color-text-soft: #cbd5e1;
  --color-text-muted: #94a3b8;
  --color-accent: #60a5fa;
  --color-accent-strong: #93c5fd;
  --color-accent-weak: #1d4ed833;
  --color-warning-text: #fcd34d;
  --color-warning-bg: #78350f66;
  --color-warning-dot: #f59e0b;
  --color-neutral-text: #cbd5e1;
  --color-neutral-bg: #334155;
  --color-neutral-dot: #94a3b8;
  --radius-nav: 8px;
  --radius-card: 28px;
  --radius-panel: 24px;
  --radius-pill: 999px;
  --radius-inner: 8px;
  --space-page: 28px;
  --space-content-gap: 20px;
  --shadow-soft: 0 18px 45px #02061766;
  --shadow-panel: 0 24px 70px #02061780;
  --shadow-card: 0 22px 60px #02061770;
  --shadow-control: 0 8px 20px #60a5fa26;
  --focus-ring: #60a5fa;
}
```

- [ ] **Step 2: Run frontend build**

Run:

```powershell
pnpm build
```

Expected:

```text
tsc --noEmit && vite build
```

Exit code must be `0`.

- [ ] **Step 3: Commit Task 3**

Run:

```powershell
git add src/shared/styles/tokens.css
git commit -m "style: add dark color scheme tokens"
```

Expected: commit succeeds and only `tokens.css` is staged.

---

### Task 4: Build ThemeMenu UI

**Files:**

- Create: `src/app/frame/ThemeMenu.tsx`
- Create: `src/app/frame/ThemeMenu.css`
- Modify: `src/main.tsx`

- [ ] **Step 1: Create `ThemeMenu.tsx`**

Create `src/app/frame/ThemeMenu.tsx`:

```tsx
import { Check, ChevronDown, Moon, Sun } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useColorScheme } from "../appearance/useColorScheme";
import type { ColorSchemePreference } from "../appearance/colorSchemeTypes";

type ThemeOption = {
  preference: ColorSchemePreference;
  label: string;
  icon: "sun" | "moon" | "system";
};

const themeOptions: ThemeOption[] = [
  { preference: "light", label: "浅色模式", icon: "sun" },
  { preference: "dark", label: "深色模式", icon: "moon" },
  { preference: "system", label: "跟随系统", icon: "system" },
];

export function ThemeMenu() {
  const { effective, preference, setPreference } = useColorScheme();
  const [isOpen, setIsOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const handlePointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };

    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [isOpen]);

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape") {
      setIsOpen(false);
    }
  };

  const selectPreference = (nextPreference: ColorSchemePreference) => {
    setPreference(nextPreference);
    setIsOpen(false);
  };

  return (
    <div className="theme-menu" ref={rootRef} onKeyDown={handleKeyDown}>
      <button
        type="button"
        className="theme-menu__trigger"
        aria-label="选择主题模式"
        aria-haspopup="menu"
        aria-expanded={isOpen}
        onClick={() => setIsOpen((current) => !current)}
      >
        <span className="theme-menu__trigger-icon" aria-hidden="true">
          {effective === "dark" ? <Moon size={14} /> : <Sun size={14} />}
        </span>
        <ChevronDown
          size={14}
          className={isOpen ? "theme-menu__chevron is-open" : "theme-menu__chevron"}
          aria-hidden="true"
        />
      </button>

      {isOpen ? (
        <div className="theme-menu__panel" role="menu" aria-label="主题模式">
          {themeOptions.map((option) => {
            const isSelected = preference === option.preference;

            return (
              <button
                key={option.preference}
                type="button"
                className={isSelected ? "theme-menu__item is-selected" : "theme-menu__item"}
                role="menuitemradio"
                aria-checked={isSelected}
                onClick={() => selectPreference(option.preference)}
              >
                <ThemeOptionIcon icon={option.icon} />
                <span className="theme-menu__item-label">{option.label}</span>
                {isSelected ? <Check size={14} className="theme-menu__check" aria-hidden="true" /> : null}
              </button>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}

function ThemeOptionIcon({ icon }: { icon: ThemeOption["icon"] }) {
  if (icon === "system") {
    return (
      <span className="theme-menu__system-icon" aria-hidden="true">
        <span className="theme-menu__system-icon-half is-light">
          <Sun size={10} />
        </span>
        <span className="theme-menu__system-icon-half is-dark">
          <Moon size={10} />
        </span>
      </span>
    );
  }

  return (
    <span className={`theme-menu__option-icon is-${icon}`} aria-hidden="true">
      {icon === "moon" ? <Moon size={14} /> : <Sun size={14} />}
    </span>
  );
}
```

- [ ] **Step 2: Create `ThemeMenu.css`**

Create `src/app/frame/ThemeMenu.css`:

```css
.theme-menu {
  position: relative;
  display: inline-flex;
}

.theme-menu__trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 36px;
  padding: 6px 6px 6px 6px;
  color: var(--color-text-muted);
  background: var(--color-surface);
  border: 1px solid var(--color-border-muted);
  border-radius: var(--radius-pill);
  box-shadow: 0 6px 16px #0f172a12;
  cursor: pointer;
  transition:
    background-color 120ms ease,
    border-color 120ms ease,
    box-shadow 120ms ease;
}

.theme-menu__trigger:hover {
  background: var(--color-surface-muted);
}

.theme-menu__trigger:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 2px;
}

.theme-menu__trigger-icon {
  display: inline-grid;
  place-items: center;
  width: 24px;
  height: 24px;
  color: #f59e0b;
  background: var(--color-surface-subtle);
  border-radius: var(--radius-pill);
}

:root[data-color-scheme="dark"] .theme-menu__trigger-icon {
  color: #c7d2fe;
}

.theme-menu__chevron {
  margin-right: 2px;
  color: var(--color-text-muted);
  transition: transform 120ms ease;
}

.theme-menu__chevron.is-open {
  transform: rotate(180deg);
}

.theme-menu__panel {
  position: absolute;
  top: calc(100% + 12px);
  right: 0;
  z-index: 20;
  display: grid;
  width: 160px;
  gap: 2px;
  padding: 6px;
  color: var(--color-text-soft);
  background: var(--color-surface);
  border: 1px solid var(--color-border-muted);
  border-radius: 16px;
  box-shadow: 0 18px 46px #0f172a18;
  animation: theme-menu-enter 100ms ease-out;
  transform-origin: top right;
}

.theme-menu__item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  height: 40px;
  padding: 0 8px;
  color: inherit;
  font-size: 14px;
  font-weight: 700;
  text-align: left;
  background: transparent;
  border: 0;
  border-radius: 12px;
  cursor: pointer;
}

.theme-menu__item:hover {
  background: var(--color-surface-muted);
}

.theme-menu__item.is-selected {
  color: var(--color-accent);
  background: var(--color-accent-weak);
}

.theme-menu__item:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 2px;
}

.theme-menu__item-label {
  flex: 1;
  white-space: nowrap;
}

.theme-menu__option-icon,
.theme-menu__system-icon {
  flex: 0 0 auto;
  width: 24px;
  height: 24px;
  border-radius: var(--radius-pill);
}

.theme-menu__option-icon {
  display: inline-grid;
  place-items: center;
}

.theme-menu__option-icon.is-sun {
  color: #f59e0b;
  background: var(--color-surface-subtle);
  border: 1px solid #e2e8f080;
}

.theme-menu__option-icon.is-moon {
  color: #c7d2fe;
  background: #0f172a;
  border: 1px solid #1e293b;
}

.theme-menu__system-icon {
  display: flex;
  overflow: hidden;
  border: 1px solid var(--color-border-muted);
  box-shadow: inset 0 1px 2px #0f172a10;
}

.theme-menu__system-icon-half {
  display: flex;
  align-items: center;
  width: 50%;
  height: 100%;
}

.theme-menu__system-icon-half.is-light {
  justify-content: flex-end;
  padding-right: 1px;
  color: #f59e0b;
  background: #f1f5f9;
}

.theme-menu__system-icon-half.is-dark {
  justify-content: flex-start;
  padding-left: 1px;
  color: #c7d2fe;
  background: #0f172a;
}

.theme-menu__check {
  flex: 0 0 auto;
  color: var(--color-accent);
}

@keyframes theme-menu-enter {
  from {
    opacity: 0;
    transform: scale(0.95);
  }

  to {
    opacity: 1;
    transform: scale(1);
  }
}
```

- [ ] **Step 3: Import `ThemeMenu.css` in `main.tsx`**

Modify `src/main.tsx`:

```tsx
import "./app/frame/AppFrame.css";
import "./app/frame/ThemeMenu.css";
```

Keep the existing import order otherwise unchanged.

- [ ] **Step 4: Run lint and typecheck**

Run:

```powershell
pnpm typecheck
pnpm lint
```

Expected:

```text
tsc --noEmit
eslint .
```

Both commands must exit `0`.

- [ ] **Step 5: Commit Task 4**

Run:

```powershell
git add src/main.tsx src/app/frame/ThemeMenu.tsx src/app/frame/ThemeMenu.css
git commit -m "feat: add theme menu component"
```

Expected: commit succeeds with the new component and CSS only.

---

### Task 5: Replace Header Theme Buttons

**Files:**

- Modify: `src/app/frame/AppHeader.tsx`
- Modify: `src/app/frame/AppFrame.css`

- [ ] **Step 1: Replace static buttons in `AppHeader.tsx`**

Modify `src/app/frame/AppHeader.tsx`:

```tsx
import { Settings } from "lucide-react";
import { ThemeMenu } from "./ThemeMenu";

export function AppHeader() {
  return (
    <header className="top-status-bar">
      <div className="current-game">
        <span>当前游戏</span>
        <strong>Monster Hunter: World - Iceborne</strong>
      </div>

      <div className="status-actions" aria-label="当前状态">
        <span className="status-pill warning">
          <span>配置档</span>
          <strong>待初始化</strong>
        </span>
        <span className="status-pill warning compact">
          <span className="dot warning-dot" aria-hidden="true" />
          <strong>目录未配置</strong>
        </span>
        <span className="status-pill neutral compact">
          <span className="dot neutral-dot" aria-hidden="true" />
          <span>任务空闲</span>
        </span>
      </div>

      <div className="window-tools" aria-label="窗口工具">
        <ThemeMenu />
        <button type="button" className="icon-button" aria-label="打开设置">
          <Settings size={16} />
        </button>
      </div>
    </header>
  );
}
```

- [ ] **Step 2: Remove old theme toggle CSS**

In `src/app/frame/AppFrame.css`, remove `.theme-toggle`, `.theme-button`, `.theme-button.is-selected`, and `.theme-button:focus-visible` rules. Keep `.window-tools` and `.icon-button`.

The remaining tool styles should look like:

```css
.status-actions,
.window-tools {
  display: flex;
  align-items: center;
}

.window-tools {
  gap: 8px;
}

.icon-button {
  display: inline-grid;
  place-items: center;
  width: 38px;
  height: 34px;
  color: var(--color-text-soft);
  background: var(--color-surface-subtle);
  border: 1px solid var(--color-border-muted);
  border-radius: var(--radius-pill);
  cursor: pointer;
}

.icon-button:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 2px;
}
```

- [ ] **Step 3: Run frontend build**

Run:

```powershell
pnpm build
```

Expected:

```text
tsc --noEmit && vite build
```

Exit code must be `0`.

- [ ] **Step 4: Commit Task 5**

Run:

```powershell
git add src/app/frame/AppHeader.tsx src/app/frame/AppFrame.css
git commit -m "feat: replace header theme toggle"
```

Expected: commit succeeds and the old static two-button theme toggle is gone.

---

### Task 6: Browser Smoke Test And Full Verification

**Files:**

- No source file changes expected.
- Update PWF `progress.md` after verification.

- [ ] **Step 1: Start or reuse local dev server**

If no dev server is running, run:

```powershell
pnpm dev -- --host 127.0.0.1
```

Expected: Vite prints a local URL such as `http://127.0.0.1:5173/`.

- [ ] **Step 2: Verify theme menu in browser**

Using the in-app browser, verify:

```text
1. Page loads without console errors.
2. Top theme trigger is collapsed and shows only icon + chevron.
3. Clicking trigger opens menu.
4. Chevron rotates upward while menu is open.
5. Menu shows 浅色模式 / 深色模式 / 跟随系统.
6. Selecting 深色模式 sets document.documentElement.dataset.colorScheme to dark.
7. Selecting 浅色模式 sets document.documentElement.dataset.colorScheme to light.
8. Selecting 跟随系统 stores system preference and trigger icon follows effective scheme.
9. Refresh keeps the last preference.
10. Pressing Escape closes the menu.
```

Use this browser-side check for the dataset:

```js
document.documentElement.dataset.colorScheme
```

Expected after dark selection:

```text
dark
```

Expected after light selection:

```text
light
```

- [ ] **Step 3: Verify corrupted localStorage fallback**

In the browser console or Playwright evaluate, set:

```js
localStorage.setItem("helsincy.colorSchemePreference", "broken-value");
location.reload();
```

Expected:

```text
Application reloads without white screen.
Theme preference falls back to system.
document.documentElement.dataset.colorScheme is light or dark depending on system preference.
```

- [ ] **Step 4: Run full repository verification**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected final line:

```text
Verification passed.
```

- [ ] **Step 5: Update PWF progress**

Update `.planning/2026-06-01-plan-61a4bab1/progress.md` with:

```markdown
- 已完成黑白主题切换实现。
- 已验证浅色 / 深色 / 跟随系统切换、持久化、非法存储回退、菜单开合和 chevron 动效。
- 已执行 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`，结果通过。
```

Do not stage `.planning/`.

- [ ] **Step 6: Commit verification notes if source changed after previous commits**

If verification required source fixes, commit them:

```powershell
git add <changed-source-files>
git commit -m "fix: polish color scheme toggle"
```

Expected: no `.planning/`, `tmp/`, `dist/`, `target/`, or `node_modules/` files are staged.

---

## Self-Review

Spec coverage:

- `light` / `dark` / `system` state: Task 1 and Task 2.
- `data-color-scheme` DOM application: Task 2.
- CSS semantic tokens: Task 3.
- v9 theme dropdown UI: Task 4 and Task 5.
- Persistence and invalid value fallback: Task 1 and Task 6.
- System preference listening: Task 2 and Task 6.
- Separation from `sidebarMode` and business pages: Task 2 wraps independently from `SidebarModeProvider`, Task 5 only touches header composition.
- Verification boundaries: Task 6.

Placeholder scan:

- The plan has no placeholder markers or incomplete-action markers.
- Every task lists exact files and commands.
- Code snippets define the names used later in the plan.

Type consistency:

- `ColorSchemePreference`, `EffectiveColorScheme`, `PersistedColorSchemeSettings`, `ColorSchemeProvider`, and `useColorScheme` names are consistent across tasks.
- `ThemeMenu` imports `useColorScheme` from `../appearance/useColorScheme`.
- `AppHeader.tsx` imports `ThemeMenu` from `./ThemeMenu`.
