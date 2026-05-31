# Dashboard v2 Sidebar Modes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将现有首启工作台迁移为 Dashboard v2 共享视觉基线，并在同一套业务页面下支持普通侧边栏和悬浮侧边栏两种导航模式。

**Architecture:** 先拆边界，再换视觉，最后加模式切换。`AppFrame` 负责整体槽位，`AppHeader` 负责顶部状态栏，`ClassicSidebar` 与 `FloatingSidebar` 只负责导航呈现，`DashboardPage` 和 `SetupStatusPanel` 不读取侧边栏模式。

**Tech Stack:** Tauri 2、React 19、TypeScript、Vite、lucide-react、CSS Modules 风格的按目录 CSS 文件、前端 localStorage MVP 持久化。

---

## 背景与边界

本计划落地 [Dashboard v2 与侧边栏模式设计](../../DASHBOARD_V2_SIDEBAR_MODES.md)，并遵守 [前端外观系统设计](../../APPEARANCE_SYSTEM.md) 与 [前端外观系统扩展指南](../../APPEARANCE_EXTENSION_GUIDE.md)。

当前代码状态：

- `src/app/AppShell.tsx` 同时包含导航定义、普通侧边栏、顶部状态栏和应用壳。
- `src/features/dashboard/FirstLaunchDashboard.tsx` 同时包含主工作区、右侧设置状态面板和静态演示数据。
- `src/app/AppShell.css` 承载应用壳、导航、工作台、右侧面板和响应式样式。
- `src/main.tsx` 只导入 `./app/AppShell.css`，全局样式入口过于集中。

本计划的硬边界：

- 不复制 `DashboardPage`。
- 不新增 `FloatingDashboardPage` 或 `ClassicDashboardPage`。
- 不让页面级组件读取 `sidebarMode`。
- 不为普通侧边栏和悬浮侧边栏维护两份导航定义。
- 不把颜色主题、密度或游戏规则塞进侧边栏模式。
- 不接入真实 Mod、游戏目录扫描、Tauri 文件写入或存档逻辑。

## 目标文件结构

完成后前端结构应接近：

```text
src/
  App.tsx
  main.tsx
  app/
    frame/
      AppFrame.tsx
      AppFrame.css
      AppHeader.tsx
    shell/
      sidebarTypes.ts
      SidebarModeProvider.tsx
      useSidebarMode.ts
      navigation/
        navItems.ts
        NavButtonBase.tsx
      layouts/
        classic-sidebar/
          ClassicSidebar.tsx
          ClassicSidebar.css
        floating-sidebar/
          FloatingSidebar.tsx
          FloatingSidebar.css
  features/
    dashboard/
      DashboardPage.tsx
      DashboardHeroCard.tsx
      DashboardModulePreview.tsx
      SetupStatusPanel.tsx
      dashboardData.ts
      Dashboard.css
  shared/
    styles/
      reset.css
      tokens.css
```

职责锁定：

- `src/app/shell/navigation/navItems.ts`：唯一导航定义。
- `src/app/frame/AppFrame.tsx`：组合 Header、Sidebar、Main，不包含业务页面内容。
- `src/app/frame/AppHeader.tsx`：顶部状态栏，后续可接入真实状态摘要。
- `src/app/shell/layouts/*`：只渲染导航，不判断业务流程。
- `src/features/dashboard/*`：只渲染工作台内容，不知道当前侧边栏形态。
- `src/shared/styles/*`：全局 reset 与语义 token，不放页面专属样式。

## Task 1: 抽离导航定义

**Files:**

- Create: `src/app/shell/navigation/navItems.ts`
- Modify: `src/app/AppShell.tsx`
- Test: `cmd /c corepack pnpm run typecheck`

- [ ] **Step 1: 创建唯一导航定义**

在 `src/app/shell/navigation/navItems.ts` 写入：

```ts
import {
  Archive,
  Crosshair,
  FileSearch,
  Gamepad2,
  LayoutDashboard,
  ListChecks,
  Puzzle,
  Settings,
  Tags,
  User,
} from "lucide-react";
import type { ComponentType } from "react";

export type NavItemState = "active" | "disabled";

export type NavItem = {
  id: string;
  label: string;
  icon: ComponentType<{ size?: number; strokeWidth?: number }>;
  route: string;
  state?: NavItemState;
  disabledReason?: string;
};

export const navItems: NavItem[] = [
  { id: "dashboard", label: "工作台", icon: LayoutDashboard, route: "/", state: "active" },
  {
    id: "mods",
    label: "Mod 管理",
    icon: Puzzle,
    route: "/mods",
    state: "disabled",
    disabledReason: "完成游戏目录设置后启用",
  },
  {
    id: "categories",
    label: "分类 / 标签",
    icon: Tags,
    route: "/categories",
    state: "disabled",
    disabledReason: "导入 Mod 后启用",
  },
  {
    id: "profiles",
    label: "配置档",
    icon: User,
    route: "/profiles",
    state: "disabled",
    disabledReason: "创建默认配置档后启用",
  },
  {
    id: "replacements",
    label: "替换目标",
    icon: Crosshair,
    route: "/replacements",
    state: "disabled",
    disabledReason: "替换目标 catalog 接入后启用",
  },
  {
    id: "backups",
    label: "存档备份",
    icon: Archive,
    route: "/backups",
    state: "disabled",
    disabledReason: "存档路径规则接入后启用",
  },
  { id: "games", label: "游戏管理", icon: Gamepad2, route: "/games" },
  { id: "tasks", label: "任务队列", icon: ListChecks, route: "/tasks" },
  { id: "diagnostics", label: "日志 / 诊断", icon: FileSearch, route: "/diagnostics" },
  { id: "settings", label: "设置", icon: Settings, route: "/settings" },
];
```

- [ ] **Step 2: 让 `AppShell.tsx` 使用新导航定义**

从 `src/app/AppShell.tsx` 删除导航图标 import、本地 `NavItem` 类型和本地 `navItems` 常量，改为：

```ts
import { Moon, Settings, Sun } from "lucide-react";
import type { ReactNode } from "react";
import { navItems, type NavItem } from "./shell/navigation/navItems";
```

`NavButton` 保持同名，接收导入的 `NavItem` 类型。

- [ ] **Step 3: 验证类型检查通过**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: TypeScript completes with exit code `0`.

- [ ] **Step 4: Commit**

```powershell
git add src/app/AppShell.tsx src/app/shell/navigation/navItems.ts
git commit -m "refactor: 抽离前端导航定义"
```

## Task 2: 抽离顶部状态栏

**Files:**

- Create: `src/app/frame/AppHeader.tsx`
- Modify: `src/app/AppShell.tsx`
- Test: `cmd /c corepack pnpm run typecheck`

- [ ] **Step 1: 创建 `AppHeader`**

在 `src/app/frame/AppHeader.tsx` 写入：

```tsx
import { Moon, Settings, Sun } from "lucide-react";

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
        <div className="theme-toggle" aria-label="主题模式">
          <button type="button" className="theme-button is-selected" aria-label="浅色主题">
            <Sun size={14} />
          </button>
          <button type="button" className="theme-button" aria-label="深色主题">
            <Moon size={14} />
          </button>
        </div>
        <button type="button" className="icon-button" aria-label="打开设置">
          <Settings size={16} />
        </button>
      </div>
    </header>
  );
}
```

- [ ] **Step 2: 替换 `AppShell.tsx` 内联顶部栏**

在 `src/app/AppShell.tsx` 引入：

```ts
import { AppHeader } from "./frame/AppHeader";
```

把 `<TopStatusBar />` 替换为 `<AppHeader />`，删除本文件中的 `TopStatusBar` 函数和 `Moon`、`Sun`、`Settings` import。

- [ ] **Step 3: 验证类型检查通过**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: TypeScript completes with exit code `0`.

- [ ] **Step 4: Commit**

```powershell
git add src/app/AppShell.tsx src/app/frame/AppHeader.tsx
git commit -m "refactor: 抽离应用顶部状态栏"
```

## Task 3: 抽离普通侧边栏

**Files:**

- Create: `src/app/shell/layouts/classic-sidebar/ClassicSidebar.tsx`
- Modify: `src/app/AppShell.tsx`
- Test: `cmd /c corepack pnpm run typecheck`

- [ ] **Step 1: 创建 `ClassicSidebar`**

在 `src/app/shell/layouts/classic-sidebar/ClassicSidebar.tsx` 写入：

```tsx
import { navItems, type NavItem } from "../../navigation/navItems";

export function ClassicSidebar() {
  return (
    <aside className="sidebar" aria-label="主导航">
      <div className="brand-block">
        <h1>Helsincy</h1>
        <p>Mod Manager</p>
      </div>

      <nav className="nav-list">
        {navItems.map((item) => (
          <ClassicNavButton key={item.id} item={item} />
        ))}
      </nav>

      <div className="nav-footnote">
        <span aria-hidden="true" />
        <p>MHW:I&nbsp;&nbsp;首次启动</p>
      </div>
    </aside>
  );
}

function ClassicNavButton({ item }: { item: NavItem }) {
  const Icon = item.icon;
  const isActive = item.state === "active";
  const isDisabled = item.state === "disabled";

  return (
    <button
      type="button"
      className={`nav-item ${isActive ? "is-active" : ""}`}
      disabled={isDisabled}
      aria-current={isActive ? "page" : undefined}
      title={isDisabled ? item.disabledReason : undefined}
    >
      {isActive && <span className="active-mark" aria-hidden="true" />}
      <Icon size={16} strokeWidth={2.1} />
      <span>{item.label}</span>
    </button>
  );
}
```

- [ ] **Step 2: 精简 `AppShell.tsx`**

`src/app/AppShell.tsx` 应只保留应用壳组合：

```tsx
import type { ReactNode } from "react";
import { AppHeader } from "./frame/AppHeader";
import { ClassicSidebar } from "./shell/layouts/classic-sidebar/ClassicSidebar";

type AppShellProps = {
  children: ReactNode;
};

export function AppShell({ children }: AppShellProps) {
  return (
    <div className="app-shell">
      <ClassicSidebar />
      <div className="app-surface">
        <AppHeader />
        <main className="workbench-body">{children}</main>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: 验证类型检查通过**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: TypeScript completes with exit code `0`.

- [ ] **Step 4: Commit**

```powershell
git add src/app/AppShell.tsx src/app/shell/layouts/classic-sidebar/ClassicSidebar.tsx
git commit -m "refactor: 抽离经典侧边栏"
```

## Task 4: 建立 AppFrame 槽位

**Files:**

- Create: `src/app/frame/AppFrame.tsx`
- Modify: `src/app/AppShell.tsx`
- Test: `cmd /c corepack pnpm run typecheck`

- [ ] **Step 1: 创建 `AppFrame`**

在 `src/app/frame/AppFrame.tsx` 写入：

```tsx
import type { ReactNode } from "react";
import { AppHeader } from "./AppHeader";
import { ClassicSidebar } from "../shell/layouts/classic-sidebar/ClassicSidebar";

type AppFrameProps = {
  children: ReactNode;
};

export function AppFrame({ children }: AppFrameProps) {
  return (
    <div className="app-shell" data-sidebar-mode="classic">
      <ClassicSidebar />
      <div className="app-surface">
        <AppHeader />
        <main className="workbench-body">{children}</main>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: 保留 `AppShell` 作为兼容出口**

将 `src/app/AppShell.tsx` 改成：

```tsx
import type { ReactNode } from "react";
import { AppFrame } from "./frame/AppFrame";

type AppShellProps = {
  children: ReactNode;
};

export function AppShell({ children }: AppShellProps) {
  return <AppFrame>{children}</AppFrame>;
}
```

- [ ] **Step 3: 验证类型检查通过**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: TypeScript completes with exit code `0`.

- [ ] **Step 4: Commit**

```powershell
git add src/app/AppShell.tsx src/app/frame/AppFrame.tsx
git commit -m "refactor: 建立应用框架槽位"
```

## Task 5: 拆分 Dashboard 数据和组件

**Files:**

- Create: `src/features/dashboard/dashboardData.ts`
- Create: `src/features/dashboard/DashboardHeroCard.tsx`
- Create: `src/features/dashboard/DashboardModulePreview.tsx`
- Create: `src/features/dashboard/SetupStatusPanel.tsx`
- Create: `src/features/dashboard/DashboardPage.tsx`
- Modify: `src/features/dashboard/FirstLaunchDashboard.tsx`
- Modify: `src/App.tsx`
- Test: `cmd /c corepack pnpm run typecheck`

- [ ] **Step 1: 创建 Dashboard 静态数据模块**

在 `src/features/dashboard/dashboardData.ts` 写入：

```ts
export const supportCards = [
  { label: "当前支持", value: "Monster Hunter: World - Iceborne" },
  { label: "当前平台", value: "Windows" },
  { label: "Linux / Steam Deck", value: "实验性支持预留" },
] as const;

export const previewCards = [
  { label: "Mod 概览", shortWidth: "80px" },
  { label: "冲突状态", shortWidth: "72px" },
  { label: "前置检查", shortWidth: "76px" },
  { label: "最近备份", shortWidth: "70px" },
] as const;

export const setupSteps = [
  { title: "扫描 Steam 游戏库", meta: "检测已安装游戏和可用候选项。", active: true },
  { title: "验证游戏目录", meta: "确认可执行文件、数据目录和写入权限。" },
  { title: "创建默认配置档案", meta: "在导入前准备一份干净的基线。" },
  { title: "开始导入模组", meta: "仅在目录和配置检查通过后启用。" },
] as const;

export const setupLogs = [
  { time: "09:42", message: "首次启动设置已打开" },
  { time: "09:42", message: "等待扫描 Steam 游戏库" },
  { time: "--:--", message: "尚未选择游戏目录", muted: true },
] as const;
```

- [ ] **Step 2: 创建主目录识别卡片**

在 `src/features/dashboard/DashboardHeroCard.tsx` 写入：

```tsx
import { FolderOpen, Search } from "lucide-react";
import { supportCards } from "./dashboardData";

export function DashboardHeroCard() {
  return (
    <section className="setup-panel" aria-labelledby="setup-title">
      <div className="setup-message">
        <span className="badge warning">
          <span className="dot warning-dot" aria-hidden="true" />
          目录未配置
        </span>
        <h3 id="setup-title">未找到游戏目录</h3>
        <p>需要先识别《怪物猎人：世界 冰原》的安装目录，才能导入和安装 Mod。</p>
      </div>

      <div className="setup-actions">
        <button type="button" className="primary-action">
          <Search size={16} />
          自动扫描 Steam
        </button>
        <button type="button" className="secondary-action">
          <FolderOpen size={16} />
          手动选择游戏目录
        </button>
      </div>

      <div className="support-grid" aria-label="支持信息">
        {supportCards.map((card) => (
          <article className="support-card" key={card.label}>
            <span>{card.label}</span>
            <strong>{card.value}</strong>
          </article>
        ))}
      </div>
    </section>
  );
}
```

- [ ] **Step 3: 创建模块预览组件**

在 `src/features/dashboard/DashboardModulePreview.tsx` 写入：

```tsx
import { LayoutDashboard } from "lucide-react";
import { previewCards } from "./dashboardData";

export function DashboardModulePreview() {
  return (
    <section className="preview-panel" aria-labelledby="preview-title">
      <h3 id="preview-title">完成设置后将显示</h3>
      <p>以下模块会在目录识别、权限校验和默认配置档案创建后启用。</p>

      <div className="preview-heading">
        <LayoutDashboard size={16} />
        <strong>设置完成后启用</strong>
      </div>

      <div className="preview-grid">
        {previewCards.map((card) => (
          <article className="preview-card" key={card.label}>
            <strong>{card.label}</strong>
            <span className="skeleton-line" />
            <span className="skeleton-line short" style={{ width: card.shortWidth }} />
          </article>
        ))}
      </div>
    </section>
  );
}
```

- [ ] **Step 4: 创建右侧设置状态面板**

在 `src/features/dashboard/SetupStatusPanel.tsx` 写入：

```tsx
import { setupLogs, setupSteps } from "./dashboardData";

export function SetupStatusPanel() {
  return (
    <aside className="setup-rail" aria-label="首次启动设置状态">
      <header className="rail-header">
        <span>首次启动</span>
        <h2>设置状态</h2>
        <p>Helsincy 需要先完成几项检查，才能启用模组管理。</p>
      </header>

      <section className="rail-card current-state" aria-labelledby="current-state-title">
        <div className="state-title-row">
          <span className="dot neutral-dot" aria-hidden="true" />
          <h3 id="current-state-title">等待扫描游戏库</h3>
        </div>
        <p>尚未选择游戏目录。请先在主区域自动扫描 Steam 安装。</p>
        <span className="soft-badge">等待主区扫描</span>
      </section>

      <section className="rail-section" aria-labelledby="next-step-title">
        <div className="section-title-row">
          <h3 id="next-step-title">下一步</h3>
          <span>第 1 / 4 步</span>
        </div>
        <div className="step-list">
          {setupSteps.map((step, index) => (
            <StepItem key={step.title} index={index + 1} step={step} isLast={index === setupSteps.length - 1} />
          ))}
        </div>
      </section>

      <section className="rail-section" aria-labelledby="summary-title">
        <h3 id="summary-title">设置摘要</h3>
        <div className="summary-grid">
          <SummaryBox label="状态" value="未扫描" />
          <SummaryBox label="风险" value="风险：等待检查" />
        </div>
        <article className="summary-note">
          <strong>检查等待中</strong>
          <p>将在设置过程中检查 Steam 访问、游戏文件夹写入权限和配置存储。</p>
        </article>
      </section>

      <section className="rail-section" aria-labelledby="setup-log-title">
        <h3 id="setup-log-title">设置日志</h3>
        <div className="log-card">
          {setupLogs.map((log) => (
            <p key={`${log.time}-${log.message}`} className={log.muted ? "is-muted" : ""}>
              <time>{log.time}</time>
              {log.message}
            </p>
          ))}
        </div>
      </section>
    </aside>
  );
}

function StepItem({
  index,
  step,
  isLast,
}: {
  index: number;
  step: { title: string; meta: string; active?: boolean };
  isLast: boolean;
}) {
  return (
    <article className={`step-item ${step.active ? "is-active" : ""}`}>
      <div className="step-rail" aria-hidden="true">
        <span>{index}</span>
        {!isLast && <i />}
      </div>
      <div className="step-body">
        <strong>{step.title}</strong>
        <p>{step.meta}</p>
      </div>
    </article>
  );
}

function SummaryBox({ label, value }: { label: string; value: string }) {
  return (
    <article className="summary-box">
      <span>{label}</span>
      <strong>{value}</strong>
    </article>
  );
}
```

- [ ] **Step 5: 创建 Dashboard 页面组合组件**

在 `src/features/dashboard/DashboardPage.tsx` 写入：

```tsx
import { DashboardHeroCard } from "./DashboardHeroCard";
import { DashboardModulePreview } from "./DashboardModulePreview";
import { SetupStatusPanel } from "./SetupStatusPanel";

export function DashboardPage() {
  return (
    <>
      <section className="main-workspace" aria-labelledby="workbench-title">
        <header className="main-header">
          <h2 id="workbench-title">工作台</h2>
          <p>首次启动需要先完成游戏目录识别。</p>
        </header>

        <DashboardHeroCard />
        <DashboardModulePreview />
      </section>

      <SetupStatusPanel />
    </>
  );
}
```

- [ ] **Step 6: 保留旧组件名作为过渡出口**

将 `src/features/dashboard/FirstLaunchDashboard.tsx` 改成：

```tsx
import { DashboardPage } from "./DashboardPage";

export function FirstLaunchDashboard() {
  return <DashboardPage />;
}
```

- [ ] **Step 7: 更新 `App.tsx` 使用新页面名**

将 `src/App.tsx` 改成：

```tsx
import { AppShell } from "./app/AppShell";
import { DashboardPage } from "./features/dashboard/DashboardPage";

export function App() {
  return (
    <AppShell>
      <DashboardPage />
    </AppShell>
  );
}
```

- [ ] **Step 8: 验证类型检查通过**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: TypeScript completes with exit code `0`.

- [ ] **Step 9: Commit**

```powershell
git add src/App.tsx src/features/dashboard
git commit -m "refactor: 拆分工作台页面组件"
```

## Task 6: 拆分全局样式入口

**Files:**

- Create: `src/shared/styles/reset.css`
- Create: `src/shared/styles/tokens.css`
- Create: `src/app/frame/AppFrame.css`
- Create: `src/app/shell/layouts/classic-sidebar/ClassicSidebar.css`
- Create: `src/features/dashboard/Dashboard.css`
- Modify: `src/app/AppShell.css`
- Modify: `src/main.tsx`
- Test: `cmd /c corepack pnpm run build`

- [ ] **Step 1: 创建 reset 样式**

从 `src/app/AppShell.css` 移出 `:root` 字体基础、`*`、`body`、`button`、`#root`，写入 `src/shared/styles/reset.css`。

- [ ] **Step 2: 创建 token 样式**

在 `src/shared/styles/tokens.css` 写入第一批语义变量：

```css
:root {
  --color-bg: #f8fafc;
  --color-surface: #ffffff;
  --color-surface-muted: #f3f5f8;
  --color-border: #d5dae2;
  --color-border-muted: #e5e7eb;
  --color-text: #1f2933;
  --color-text-muted: #697386;
  --color-accent: #2563eb;
  --color-accent-weak: #f0f7ff;
  --color-warning-text: #92400e;
  --color-warning-bg: #fef3c7;
  --color-neutral-text: #475569;
  --color-neutral-bg: #f1f5f9;
  --radius-nav: 6px;
  --radius-card: 18px;
  --radius-panel: 24px;
  --space-page: 28px;
  --shadow-panel: 0 18px 50px #1f29371a;
}
```

- [ ] **Step 3: 按职责迁移 CSS**

移动规则，不改视觉：

- `.app-shell`、`.app-surface`、`.top-status-bar`、`.current-game`、`.status-actions`、`.window-tools`、`.theme-toggle`、`.status-pill`、`.icon-button`、`.theme-button` 移到 `src/app/frame/AppFrame.css`。
- `.sidebar`、`.brand-block`、`.nav-list`、`.nav-item`、`.active-mark`、`.nav-footnote` 移到 `src/app/shell/layouts/classic-sidebar/ClassicSidebar.css`。
- `.workbench-body`、`.main-workspace`、`.main-header`、`.setup-panel`、`.preview-panel`、`.setup-rail`、`.rail-*`、`.step-*`、`.summary-*`、`.log-card`、`.support-*`、`.preview-*`、`.badge`、`.soft-badge`、`.dot` 移到 `src/features/dashboard/Dashboard.css`。

`src/app/AppShell.css` 在本任务结束后可以删除；如果暂时保留，文件内只允许写导入语句。

- [ ] **Step 4: 更新 CSS 入口**

将 `src/main.tsx` 改成：

```ts
import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./shared/styles/reset.css";
import "./shared/styles/tokens.css";
import "./app/frame/AppFrame.css";
import "./app/shell/layouts/classic-sidebar/ClassicSidebar.css";
import "./features/dashboard/Dashboard.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

- [ ] **Step 5: 删除空壳旧样式文件**

如果 `src/app/AppShell.css` 已无规则，删除该文件，并确认 `src/main.tsx` 没有继续导入它。

- [ ] **Step 6: 验证构建通过**

Run:

```powershell
cmd /c corepack pnpm run build
```

Expected: TypeScript and Vite build complete with exit code `0`.

- [ ] **Step 7: Commit**

```powershell
git add src/main.tsx src/shared/styles src/app/frame src/app/shell/layouts/classic-sidebar src/features/dashboard
git add -u src/app/AppShell.css
git commit -m "refactor: 拆分前端样式边界"
```

## Task 7: 引入侧边栏模式状态

**Files:**

- Create: `src/app/shell/sidebarTypes.ts`
- Create: `src/app/shell/SidebarModeProvider.tsx`
- Create: `src/app/shell/useSidebarMode.ts`
- Modify: `src/App.tsx`
- Modify: `src/app/frame/AppFrame.tsx`
- Test: `cmd /c corepack pnpm run typecheck`

- [ ] **Step 1: 创建类型文件**

在 `src/app/shell/sidebarTypes.ts` 写入：

```ts
export type SidebarMode = "classic" | "floating";

export type PersistedSidebarModeSettings = {
  version: 1;
  sidebarMode: SidebarMode;
};

export const defaultSidebarMode: SidebarMode = "classic";
```

- [ ] **Step 2: 创建 Provider**

在 `src/app/shell/SidebarModeProvider.tsx` 写入：

```tsx
import { createContext, useCallback, useMemo, useState, type ReactNode } from "react";
import { defaultSidebarMode, type PersistedSidebarModeSettings, type SidebarMode } from "./sidebarTypes";

const storageKey = "helsincy.sidebar-mode";

type SidebarModeContextValue = {
  sidebarMode: SidebarMode;
  setSidebarMode: (mode: SidebarMode) => void;
  toggleSidebarMode: () => void;
};

export const SidebarModeContext = createContext<SidebarModeContextValue | null>(null);

type SidebarModeProviderProps = {
  children: ReactNode;
};

export function SidebarModeProvider({ children }: SidebarModeProviderProps) {
  const [sidebarMode, setSidebarModeState] = useState<SidebarMode>(readPersistedSidebarMode);

  const setSidebarMode = useCallback((mode: SidebarMode) => {
    setSidebarModeState(mode);
    writePersistedSidebarMode(mode);
  }, []);

  const toggleSidebarMode = useCallback(() => {
    setSidebarModeState((currentMode) => {
      const nextMode: SidebarMode = currentMode === "classic" ? "floating" : "classic";
      writePersistedSidebarMode(nextMode);
      return nextMode;
    });
  }, []);

  const value = useMemo(
    () => ({ sidebarMode, setSidebarMode, toggleSidebarMode }),
    [setSidebarMode, sidebarMode, toggleSidebarMode],
  );

  return <SidebarModeContext.Provider value={value}>{children}</SidebarModeContext.Provider>;
}

function readPersistedSidebarMode(): SidebarMode {
  try {
    const rawValue = window.localStorage.getItem(storageKey);
    if (rawValue === null) {
      return defaultSidebarMode;
    }

    const parsedValue = JSON.parse(rawValue) as Partial<PersistedSidebarModeSettings>;
    return parsedValue.version === 1 && isSidebarMode(parsedValue.sidebarMode)
      ? parsedValue.sidebarMode
      : defaultSidebarMode;
  } catch {
    return defaultSidebarMode;
  }
}

function writePersistedSidebarMode(sidebarMode: SidebarMode) {
  const value: PersistedSidebarModeSettings = { version: 1, sidebarMode };
  window.localStorage.setItem(storageKey, JSON.stringify(value));
}

function isSidebarMode(value: unknown): value is SidebarMode {
  return value === "classic" || value === "floating";
}
```

- [ ] **Step 3: 创建 hook**

在 `src/app/shell/useSidebarMode.ts` 写入：

```ts
import { useContext } from "react";
import { SidebarModeContext } from "./SidebarModeProvider";

export function useSidebarMode() {
  const value = useContext(SidebarModeContext);

  if (value === null) {
    throw new Error("useSidebarMode must be used within SidebarModeProvider");
  }

  return value;
}
```

- [ ] **Step 4: 在应用入口包裹 Provider**

将 `src/App.tsx` 改成：

```tsx
import { AppShell } from "./app/AppShell";
import { SidebarModeProvider } from "./app/shell/SidebarModeProvider";
import { DashboardPage } from "./features/dashboard/DashboardPage";

export function App() {
  return (
    <SidebarModeProvider>
      <AppShell>
        <DashboardPage />
      </AppShell>
    </SidebarModeProvider>
  );
}
```

- [ ] **Step 5: 让 `AppFrame` 读取模式但仍只渲染经典侧边栏**

先在 `src/app/frame/AppFrame.tsx` 引入 hook，并写入 data attribute：

```tsx
import type { ReactNode } from "react";
import { useSidebarMode } from "../shell/useSidebarMode";
import { AppHeader } from "./AppHeader";
import { ClassicSidebar } from "../shell/layouts/classic-sidebar/ClassicSidebar";

type AppFrameProps = {
  children: ReactNode;
};

export function AppFrame({ children }: AppFrameProps) {
  const { sidebarMode } = useSidebarMode();

  return (
    <div className="app-shell" data-sidebar-mode={sidebarMode}>
      <ClassicSidebar />
      <div className="app-surface">
        <AppHeader />
        <main className="workbench-body">{children}</main>
      </div>
    </div>
  );
}
```

- [ ] **Step 6: 验证类型检查通过**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: TypeScript completes with exit code `0`.

- [ ] **Step 7: Commit**

```powershell
git add src/App.tsx src/app/frame/AppFrame.tsx src/app/shell/sidebarTypes.ts src/app/shell/SidebarModeProvider.tsx src/app/shell/useSidebarMode.ts
git commit -m "feat: 添加侧边栏模式状态"
```

## Task 8: 添加悬浮侧边栏骨架

**Files:**

- Create: `src/app/shell/layouts/floating-sidebar/FloatingSidebar.tsx`
- Create: `src/app/shell/layouts/floating-sidebar/FloatingSidebar.css`
- Modify: `src/app/frame/AppFrame.tsx`
- Modify: `src/app/frame/AppFrame.css`
- Modify: `src/main.tsx`
- Test: `cmd /c corepack pnpm run build`

- [ ] **Step 1: 创建悬浮侧边栏组件**

在 `src/app/shell/layouts/floating-sidebar/FloatingSidebar.tsx` 写入：

```tsx
import { PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { navItems, type NavItem } from "../../navigation/navItems";
import { useSidebarMode } from "../../useSidebarMode";

export function FloatingSidebar() {
  const { toggleSidebarMode } = useSidebarMode();

  return (
    <aside className="floating-sidebar" aria-label="主导航">
      <div className="floating-sidebar__brand" aria-label="Helsincy">
        H
      </div>

      <nav className="floating-sidebar__nav">
        {navItems.map((item) => (
          <FloatingNavButton key={item.id} item={item} />
        ))}
      </nav>

      <button
        type="button"
        className="floating-sidebar__mode-button"
        aria-label="切换为普通侧边栏"
        onClick={toggleSidebarMode}
      >
        <PanelLeftOpen size={18} />
      </button>
    </aside>
  );
}

function FloatingNavButton({ item }: { item: NavItem }) {
  const Icon = item.icon;
  const isActive = item.state === "active";
  const isDisabled = item.state === "disabled";

  return (
    <button
      type="button"
      className={`floating-sidebar__item ${isActive ? "is-active" : ""}`}
      disabled={isDisabled}
      aria-label={item.label}
      aria-current={isActive ? "page" : undefined}
      title={isDisabled ? item.disabledReason : item.label}
    >
      <Icon size={18} strokeWidth={2.15} />
    </button>
  );
}

export function ClassicSidebarModeButton() {
  const { toggleSidebarMode } = useSidebarMode();

  return (
    <button type="button" className="sidebar-mode-button" aria-label="切换为悬浮侧边栏" onClick={toggleSidebarMode}>
      <PanelLeftClose size={16} />
      <span>悬浮侧栏</span>
    </button>
  );
}
```

- [ ] **Step 2: 在经典侧边栏底部添加模式切换入口**

在 `ClassicSidebar.tsx` 引入：

```ts
import { ClassicSidebarModeButton } from "../floating-sidebar/FloatingSidebar";
```

并放入 `.nav-footnote` 上方：

```tsx
<ClassicSidebarModeButton />
```

- [ ] **Step 3: 创建悬浮侧边栏 CSS**

在 `src/app/shell/layouts/floating-sidebar/FloatingSidebar.css` 写入：

```css
.floating-sidebar {
  position: fixed;
  z-index: 20;
  top: 18px;
  bottom: 18px;
  left: 18px;
  display: grid;
  grid-template-rows: auto 1fr auto;
  gap: 14px;
  width: 64px;
  padding: 10px;
  background: var(--color-surface);
  border: 1px solid var(--color-border-muted);
  border-radius: 24px;
  box-shadow: var(--shadow-panel);
}

.floating-sidebar__brand,
.floating-sidebar__item,
.floating-sidebar__mode-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 42px;
  height: 42px;
  border: 0;
  border-radius: 16px;
}

.floating-sidebar__brand {
  color: var(--color-surface);
  font-weight: 700;
  background: var(--color-accent);
}

.floating-sidebar__nav {
  display: grid;
  align-content: start;
  gap: 8px;
}

.floating-sidebar__item,
.floating-sidebar__mode-button {
  color: var(--color-text-muted);
  background: transparent;
  cursor: pointer;
}

.floating-sidebar__item.is-active {
  color: var(--color-accent);
  background: var(--color-accent-weak);
}

.floating-sidebar__item:disabled {
  cursor: default;
  opacity: 0.45;
}

.floating-sidebar__item:not(:disabled):hover,
.floating-sidebar__item:not(:disabled):focus-visible,
.floating-sidebar__mode-button:hover,
.floating-sidebar__mode-button:focus-visible {
  color: var(--color-accent);
  background: var(--color-accent-weak);
  outline: none;
}

.sidebar-mode-button {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 34px;
  padding: 0 10px;
  color: var(--color-text-muted);
  background: transparent;
  border: 0;
  border-radius: var(--radius-nav);
  cursor: pointer;
}

.sidebar-mode-button:hover,
.sidebar-mode-button:focus-visible {
  color: var(--color-accent);
  background: var(--color-accent-weak);
  outline: none;
}
```

- [ ] **Step 4: 让 `AppFrame` 根据模式选择侧边栏**

将 `src/app/frame/AppFrame.tsx` 改成：

```tsx
import type { ReactNode } from "react";
import { ClassicSidebar } from "../shell/layouts/classic-sidebar/ClassicSidebar";
import { FloatingSidebar } from "../shell/layouts/floating-sidebar/FloatingSidebar";
import { useSidebarMode } from "../shell/useSidebarMode";
import { AppHeader } from "./AppHeader";

type AppFrameProps = {
  children: ReactNode;
};

export function AppFrame({ children }: AppFrameProps) {
  const { sidebarMode } = useSidebarMode();
  const Sidebar = sidebarMode === "floating" ? FloatingSidebar : ClassicSidebar;

  return (
    <div className="app-shell" data-sidebar-mode={sidebarMode}>
      <Sidebar />
      <div className="app-surface">
        <AppHeader />
        <main className="workbench-body">{children}</main>
      </div>
    </div>
  );
}
```

这个分支只能存在于 App Frame，不允许移动到 Dashboard 页面。

- [ ] **Step 5: 在框架 CSS 中处理悬浮模式安全边距**

在 `src/app/frame/AppFrame.css` 增加：

```css
.app-shell[data-sidebar-mode="floating"] {
  grid-template-columns: minmax(0, 1fr);
}

.app-shell[data-sidebar-mode="floating"] .app-surface {
  padding-left: 96px;
}
```

- [ ] **Step 6: 导入悬浮侧边栏 CSS**

在 `src/main.tsx` 增加：

```ts
import "./app/shell/layouts/floating-sidebar/FloatingSidebar.css";
```

- [ ] **Step 7: 验证构建通过**

Run:

```powershell
cmd /c corepack pnpm run build
```

Expected: TypeScript and Vite build complete with exit code `0`.

- [ ] **Step 8: Commit**

```powershell
git add src/main.tsx src/app/frame/AppFrame.tsx src/app/frame/AppFrame.css src/app/shell/layouts/classic-sidebar/ClassicSidebar.tsx src/app/shell/layouts/floating-sidebar
git commit -m "feat: 添加悬浮侧边栏模式"
```

## Task 9: 落地 Dashboard v2 视觉基线

**Files:**

- Modify: `src/shared/styles/tokens.css`
- Modify: `src/app/frame/AppFrame.css`
- Modify: `src/app/shell/layouts/classic-sidebar/ClassicSidebar.css`
- Modify: `src/app/shell/layouts/floating-sidebar/FloatingSidebar.css`
- Modify: `src/features/dashboard/Dashboard.css`
- Test: `cmd /c corepack pnpm run build`

- [ ] **Step 1: 统一 token**

确认 `tokens.css` 覆盖 Dashboard v2 需要的背景、表面、边框、文本、强调色、圆角、阴影、页边距变量。不得新增 `--floating-dashboard-*` 或 `--classic-dashboard-*` 变量。

- [ ] **Step 2: 更新顶部状态栏和工作台视觉**

在 `AppFrame.css` 和 `Dashboard.css` 中按 Pencil 基线调整：

- 顶部状态栏更轻，仍横跨主内容区域。
- 主目录识别卡片成为 Dashboard v2 共享主卡片。
- 模块预览和右侧状态面板使用同一套卡片、按钮、pill 和日志样式。
- 响应式断点仍只根据空间变化，不根据 `sidebarMode` 改页面结构。

- [ ] **Step 3: 保证普通侧边栏也使用 Dashboard v2 主体**

手动检查 `ClassicSidebar` 模式下主内容、顶部栏和右侧面板已经升级到同一套 v2 样式。不要创建经典专属 Dashboard。

- [ ] **Step 4: 验证构建通过**

Run:

```powershell
cmd /c corepack pnpm run build
```

Expected: TypeScript and Vite build complete with exit code `0`.

- [ ] **Step 5: Commit**

```powershell
git add src/shared/styles/tokens.css src/app/frame/AppFrame.css src/app/shell/layouts/classic-sidebar/ClassicSidebar.css src/app/shell/layouts/floating-sidebar/FloatingSidebar.css src/features/dashboard/Dashboard.css
git commit -m "style: 落地 Dashboard v2 视觉基线"
```

## Task 10: 增加结构防回归测试

**Files:**

- Modify: `package.json`
- Create: `scripts/check-frontend-boundaries.ps1`
- Modify: `scripts/verify.ps1`
- Modify: `docs/TESTING.md`
- Test: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`

- [ ] **Step 1: 创建前端边界检查脚本**

在 `scripts/check-frontend-boundaries.ps1` 写入：

```powershell
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = git rev-parse --show-toplevel 2>$null
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($repoRoot)) {
    Write-Host "Current directory is not inside a Git repository." -ForegroundColor Red
    exit 1
}

$repoRoot = $repoRoot.Trim()
$errors = New-Object System.Collections.Generic.List[string]

$dashboardFiles = Get-ChildItem -LiteralPath (Join-Path $repoRoot "src/features/dashboard") -Recurse -File -Include *.ts,*.tsx
foreach ($file in $dashboardFiles) {
    $content = Get-Content -LiteralPath $file.FullName -Raw -Encoding UTF8
    if ($content -match "sidebarMode" -or $content -match "useSidebarMode") {
        $relative = [System.IO.Path]::GetRelativePath($repoRoot, $file.FullName)
        $errors.Add("Dashboard file must not read sidebar mode: $relative")
    }
}

$forbiddenDashboardFiles = @(
    "src/features/dashboard/FloatingDashboardPage.tsx",
    "src/features/dashboard/ClassicDashboardPage.tsx"
)

foreach ($relativePath in $forbiddenDashboardFiles) {
    if (Test-Path -LiteralPath (Join-Path $repoRoot $relativePath) -PathType Leaf) {
        $errors.Add("Do not duplicate dashboard page by sidebar mode: $relativePath")
    }
}

$navDefinitionFiles = Get-ChildItem -LiteralPath (Join-Path $repoRoot "src") -Recurse -File -Include *navItems.ts,*NavItems.ts
if (@($navDefinitionFiles).Count -ne 1) {
    $errors.Add("Expected exactly one navItems file, found $(@($navDefinitionFiles).Count).")
}

if ($errors.Count -gt 0) {
    Write-Host "Frontend boundary check failed:" -ForegroundColor Red
    foreach ($errorMessage in $errors) {
        Write-Host " - $errorMessage" -ForegroundColor Red
    }
    exit 1
}

Write-Host "Frontend boundary check passed."
```

- [ ] **Step 2: 接入统一验证脚本**

在 `scripts/verify.ps1` 的 `$checks` 列表中加入：

```powershell
"scripts/check-frontend-boundaries.ps1",
```

位置建议放在 `scripts/check-doc-links.ps1` 之后、`scripts/check-secrets.ps1` 之前。

- [ ] **Step 3: 更新测试指南**

在 `docs/TESTING.md` 的“前端改动”部分补充：

```markdown
涉及 App Shell、侧边栏模式、Dashboard 页面拆分时，还必须确认 `scripts/check-frontend-boundaries.ps1` 通过。该脚本会阻止 Dashboard 页面读取 `sidebarMode`、阻止按侧边栏模式复制 Dashboard 页面，并确认导航定义只有一份。
```

- [ ] **Step 4: 执行统一验证**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected: verification completes with `Verification passed.`

- [ ] **Step 5: Commit**

```powershell
git add scripts/check-frontend-boundaries.ps1 scripts/verify.ps1 docs/TESTING.md
git commit -m "test: 添加前端边界检查"
```

## Task 11: 浏览器和截图验收

**Files:**

- Modify: `docs/TESTING.md`
- Test: Browser smoke test with Vite dev server

- [ ] **Step 1: 启动前端开发服务器**

Run:

```powershell
cmd /c corepack pnpm run dev -- --host 127.0.0.1
```

Expected: Vite serves the app at `http://127.0.0.1:1420/`.

- [ ] **Step 2: 桌面宽屏验证**

Open `http://127.0.0.1:1420/` at `1440x900` and verify:

- 普通侧边栏模式下顶部状态栏、主卡片、模块预览、右侧状态面板正常。
- 切换悬浮侧边栏后，顶部状态栏、主卡片、模块预览、右侧状态面板文案不变。
- 悬浮侧边栏没有遮挡主操作按钮。

- [ ] **Step 3: 常见窗口验证**

At `1366x768`, verify:

- 普通侧边栏可用。
- 悬浮侧边栏可用。
- 顶部状态栏文本不重叠。
- 主按钮文字完整可读。

- [ ] **Step 4: Steam Deck 近似窗口验证**

At `1280x800`, verify:

- 触控目标没有小到难点。
- 悬浮侧边栏不遮挡主操作和右侧面板。
- 右侧状态面板在空间不足时按 CSS 响应式策略下移或收窄。

- [ ] **Step 5: 记录验证要求**

在 `docs/TESTING.md` 的前端改动部分补充上述三个手动 smoke test 场景，说明这是 UI Shell 改动的建议验收。

- [ ] **Step 6: Commit**

```powershell
git add docs/TESTING.md
git commit -m "docs: 补充侧边栏模式视觉验收"
```

## Task 12: 文档和变更记录收尾

**Files:**

- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/DASHBOARD_V2_SIDEBAR_MODES.md`
- Test: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`

- [ ] **Step 1: 更新文档入口**

在 `README.md` 文档列表中确认包含：

```markdown
- Dashboard v2 与侧边栏模式设计：`docs/DASHBOARD_V2_SIDEBAR_MODES.md`
```

如果实现后新增用户可见行为说明，再补充新的实现说明文档链接。

- [ ] **Step 2: 更新设计文档实现状态**

在 `docs/DASHBOARD_V2_SIDEBAR_MODES.md` 增加“实现状态”小节，记录：

```markdown
## 实现状态

- 已拆分 App Frame、顶部状态栏、普通侧边栏、悬浮侧边栏和 Dashboard 页面组件。
- 已添加侧边栏模式本地持久化，未知配置会回退普通侧边栏。
- Dashboard 页面不读取侧边栏模式，普通侧边栏和悬浮侧边栏共享同一套 Dashboard v2 主体。
- 前端边界检查已接入统一验证脚本。
```

- [ ] **Step 3: 更新 CHANGELOG**

在 `CHANGELOG.md` 的 `[Unreleased]` 下记录：

```markdown
- 拆分 Dashboard v2 前端结构，添加普通 / 悬浮侧边栏模式与前端边界检查。
```

- [ ] **Step 4: 执行统一验证**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected: verification completes with `Verification passed.`

- [ ] **Step 5: Commit**

```powershell
git add README.md CHANGELOG.md docs/DASHBOARD_V2_SIDEBAR_MODES.md
git commit -m "docs: 更新侧边栏模式实现状态"
```

## 最终验证

实现分支合并前必须执行：

```powershell
git status --short --branch
git diff --check
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

如果启动了 Vite dev server，完成浏览器验证后必须停止该进程。

## 自检清单

- [ ] `src/app/shell/navigation/navItems.ts` 是唯一导航定义。
- [ ] `src/features/dashboard/` 下没有 `sidebarMode` 或 `useSidebarMode`。
- [ ] `src/features/dashboard/` 下没有 `FloatingDashboardPage.tsx` 或 `ClassicDashboardPage.tsx`。
- [ ] `AppFrame` 是唯一根据 `SidebarMode` 选择侧边栏组件的位置。
- [ ] `DashboardPage`、`DashboardHeroCard`、`DashboardModulePreview`、`SetupStatusPanel` 在两种侧边栏下复用。
- [ ] 悬浮侧边栏 icon-only 按钮有 `aria-label`。
- [ ] 侧边栏模式持久化不记录本地路径、Steam ID、Mod 包信息或玩家隐私数据。
- [ ] CSS 文件按职责拆分，没有继续把所有规则堆进单个 `AppShell.css`。
- [ ] 统一验证脚本和浏览器 smoke test 结果已记录在 PR 描述中。
