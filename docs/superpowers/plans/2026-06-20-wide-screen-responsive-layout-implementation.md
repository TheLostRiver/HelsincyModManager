# 宽屏响应式布局 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落地宽屏响应式布局方案，让 App Shell、路由层和 Mod 管理页在 2K、4K、带鱼屏、超宽屏以及 4K 低浏览器缩放工作流下保持结构稳定并提升信息密度。

**Architecture:** 这次只在前端样式层解决问题。先在 `tokens.css` 增加全局布局 token，再让 AppFrame、RouterOutlet 和 Mod 管理页消费这些 token；宽屏通过 `min-width` 断点逐级增强，小屏继续沿用现有 `max-width` 响应式语义。不修改 Tauri command、Rust crate、InstallPlan、manifest、backup、rollback、游戏适配器、路由状态机或 Mod 数据模型。

**Tech Stack:** React 19、TypeScript、Vite、CSS custom properties、CSS media query、Node 内置测试运行器、浏览器手动 smoke 验证。

---

## 文件结构

- 新建 `src/shared/styles/layoutTokens.test.mjs`：CSS 合约测试，防止宽屏规则回退成硬编码宽度。
- 修改 `src/shared/styles/tokens.css`：定义布局 token 和宽屏 `min-width` 断点。
- 修改 `src/app/frame/AppFrame.css`：消费 shell 最大宽度、页面内边距和内容间距 token。
- 修改 `src/app/routing/RouterOutlet.css`：消费 route aside 宽度 token，同时保持现有路由过渡行为。
- 修改 `src/features/mods/ModLibraryPage.css`：消费 Mod 操作面板、卡片最小宽度、海报高度和小屏局部密度覆盖 token。
- 不修改 `src/features/mods/ModLibraryPage.tsx`、`src/features/mods/ModPosterCard.tsx`、`src/features/mods/modsLibraryData.ts`，除非实现中发现编译期问题。本任务是布局行为，不是 Mod 数据、筛选或选择行为。
- 不为了验收而给 Mod 页新增假的 loading skeleton。当前 Mod 管理页没有 loading state。本次只让真实卡片网格 token 化；未来如果新增 Mod skeleton，必须复用同一套 `.mod-grid` 布局契约。

## 重要上下文

当前行为：

- `src/app/frame/AppFrame.css` 中 `.app-shell { max-width: 1920px; }` 会让 3840px 宽屏出现明显左右空白。
- `src/app/routing/RouterOutlet.css` 中 `.route-transition__layer { grid-template-columns: minmax(0, 1fr) 360px; }` 仍是硬编码 route aside 宽度。
- `src/features/mods/ModLibraryPage.css` 中 `.mod-library__body { grid-template-columns: minmax(0, 1fr) 168px; }`、`.mod-grid { grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); }`、`.mod-card__poster { height: 268px; }` 都是硬编码密度值。
- Mod 页小屏已经有 `max-width: 1280px`、`960px`、`640px` 规则。保留这些语义，但把卡片尺寸改成局部 custom property 覆盖。

响应式目标：

- `<= 1920px`：保留现有桌面基线。
- `1921px - 2560px`：shell 可增长到 `2400px`。
- `2561px - 3200px`：shell 可增长到 `2880px`。
- `> 3200px`：即使在 4K 低缩放或超宽 CSS 视口下，shell 也封顶到 `3200px`。
- `3840x2160` 下浏览器缩放 `50%`、`33%`、`25%` 都是必测档。不能只测其中一个后推断另外两个也可用。

---

### Task 1: 添加 Shell 和 Route CSS 合约测试

**Files:**
- Create: `src/shared/styles/layoutTokens.test.mjs`

- [ ] **Step 1: 写出失败的布局 token 测试**

创建 `src/shared/styles/layoutTokens.test.mjs`：

```js
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const repoRoot = process.cwd();

function readProjectFile(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

test("tokens.css defines wide-screen layout tokens and breakpoints", () => {
  const tokensCss = readProjectFile("src/shared/styles/tokens.css");

  for (const tokenName of [
    "--layout-shell-max-width",
    "--layout-page-padding",
    "--layout-content-gap",
    "--layout-route-aside-width",
    "--layout-mod-action-panel-width",
    "--layout-mod-card-min-width",
    "--layout-mod-card-poster-height",
  ]) {
    assert.match(tokensCss, new RegExp(`${tokenName}:`));
  }

  assert.match(tokensCss, /--layout-shell-max-width:\s*1920px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*1921px\)\s*{[\s\S]*--layout-shell-max-width:\s*2400px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*2561px\)\s*{[\s\S]*--layout-shell-max-width:\s*2880px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*3201px\)\s*{[\s\S]*--layout-shell-max-width:\s*min\(100vw,\s*3200px\);/);
});

test("AppFrame consumes shell layout tokens instead of fixed wide-screen values", () => {
  const appFrameCss = readProjectFile("src/app/frame/AppFrame.css");

  assert.doesNotMatch(appFrameCss, /max-width:\s*1920px;/);
  assert.match(appFrameCss, /max-width:\s*var\(--layout-shell-max-width\);/);
  assert.match(appFrameCss, /gap:\s*var\(--layout-content-gap\);/);
  assert.match(appFrameCss, /padding:\s*var\(--layout-page-padding\);/);
});

test("RouterOutlet consumes the route aside layout token", () => {
  const routerOutletCss = readProjectFile("src/app/routing/RouterOutlet.css");

  assert.doesNotMatch(routerOutletCss, /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+360px;/);
  assert.match(routerOutletCss, /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+var\(--layout-route-aside-width\);/);
});
```

- [ ] **Step 2: 运行测试并确认失败**

运行：

```powershell
cmd /c corepack pnpm run test
```

Expected: FAIL。第一处失败应指向 `tokens.css` 缺少 `--layout-shell-max-width` 或对应宽屏 media rule。

- [ ] **Step 3: 不单独提交失败测试**

保持测试文件未提交，直到 Task 2 让它通过。这样不会把刻意失败的中间态写入分支历史。

---

### Task 2: 实现全局布局 Token 和 Shell 消费

**Files:**
- Modify: `src/shared/styles/tokens.css`
- Modify: `src/app/frame/AppFrame.css`
- Modify: `src/app/routing/RouterOutlet.css`
- Test: `src/shared/styles/layoutTokens.test.mjs`

- [ ] **Step 1: 增加基础布局 token**

在 `src/shared/styles/tokens.css` 的第一个 `:root, :root[data-color-scheme="light"]` 块内，紧跟 `--space-content-gap: 20px;` 插入：

```css
  --layout-shell-max-width: 1920px;
  --layout-page-padding: var(--space-page);
  --layout-content-gap: var(--space-content-gap);
  --layout-route-aside-width: 360px;
  --layout-mod-action-panel-width: 168px;
  --layout-mod-card-min-width: 200px;
  --layout-mod-card-poster-height: 268px;
```

- [ ] **Step 2: 增加宽屏 token 断点**

在 `src/shared/styles/tokens.css` 文件末尾，也就是 dark theme block 之后，添加：

```css
@media (min-width: 1921px) {
  :root {
    --layout-shell-max-width: 2400px;
    --layout-mod-action-panel-width: 192px;
    --layout-mod-card-min-width: 210px;
  }
}

@media (min-width: 2561px) {
  :root {
    --layout-shell-max-width: 2880px;
    --layout-page-padding: 36px;
    --layout-content-gap: 24px;
    --layout-mod-action-panel-width: 208px;
    --layout-mod-card-min-width: 220px;
  }
}

@media (min-width: 3201px) {
  :root {
    --layout-shell-max-width: min(100vw, 3200px);
    --layout-page-padding: 40px;
    --layout-content-gap: 28px;
    --layout-mod-action-panel-width: 220px;
    --layout-mod-card-min-width: 230px;
  }
}
```

- [ ] **Step 3: 让 AppFrame 消费 shell token**

在 `src/app/frame/AppFrame.css` 中更新 `.app-shell` 和 `.app-surface`，相关声明应变为：

```css
.app-shell {
  display: grid;
  grid-template-columns: 240px minmax(0, 1fr);
  width: 100%;
  max-width: var(--layout-shell-max-width);
  margin: 0 auto;
  height: 100vh;
  overflow-anchor: none;
  overflow: hidden;
  color: var(--color-text);
  background: var(--color-bg);
}

.app-surface {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  gap: var(--layout-content-gap);
  min-width: 0;
  min-height: 0;
  padding: var(--layout-page-padding);
  overflow: auto;
  view-transition-name: app-surface;
}
```

保留现有 `@media (max-width: 860px)` 中的 `padding: 16px;`，因为这是当前小屏覆盖规则。

- [ ] **Step 4: 让 RouterOutlet 消费 aside 宽度 token**

在 `src/app/routing/RouterOutlet.css` 中只更新 route layer 的列声明：

```css
.route-transition__layer {
  display: grid;
  grid-area: 1 / 1;
  grid-template-columns: minmax(0, 1fr) var(--layout-route-aside-width);
  justify-content: start;
  gap: 28px;
  width: 100%;
  min-width: 0;
  min-height: 0;
  margin: 0;
  background: var(--color-bg);
}
```

本任务先保留 `gap: 28px;`，避免影响 Dashboard 现有基线。Mod 页面会在 Task 4 消费内容间距 token。

- [ ] **Step 5: 运行聚焦测试**

运行：

```powershell
cmd /c corepack pnpm run test
```

Expected: PASS。新的布局 token 测试和现有 Node 测试都应通过。

- [ ] **Step 6: 运行类型检查**

运行：

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: PASS。本任务只改 CSS 和 `.mjs` 测试，TypeScript 不应出现新错误。

- [ ] **Step 7: 提交 shell 和 route 布局 token**

运行：

```powershell
git add src/shared/styles/layoutTokens.test.mjs src/shared/styles/tokens.css src/app/frame/AppFrame.css src/app/routing/RouterOutlet.css
git commit -m "style: 添加宽屏布局 token"
```

---

### Task 3: 添加 Mod 管理页密度合约测试

**Files:**
- Modify: `src/shared/styles/layoutTokens.test.mjs`

- [ ] **Step 1: 追加 Mod 管理页 CSS 合约测试**

在 `src/shared/styles/layoutTokens.test.mjs` 末尾追加：

```js
test("Mod library consumes responsive density tokens", () => {
  const modLibraryCss = readProjectFile("src/features/mods/ModLibraryPage.css");

  assert.doesNotMatch(modLibraryCss, /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+168px;/);
  assert.match(
    modLibraryCss,
    /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+var\(--layout-mod-action-panel-width\);/,
  );

  assert.doesNotMatch(modLibraryCss, /repeat\(auto-fill,\s*minmax\(200px,\s*1fr\)\)/);
  assert.match(
    modLibraryCss,
    /grid-template-columns:\s*repeat\(auto-fill,\s*minmax\(var\(--layout-mod-card-min-width\),\s*1fr\)\);/,
  );

  assert.doesNotMatch(modLibraryCss, /height:\s*268px;/);
  assert.match(modLibraryCss, /height:\s*var\(--layout-mod-card-poster-height\);/);

  assert.match(modLibraryCss, /@media\s*\(max-width:\s*960px\)\s*{[\s\S]*--layout-mod-card-min-width:\s*170px;/);
  assert.match(
    modLibraryCss,
    /@media\s*\(max-width:\s*640px\)\s*{[\s\S]*--layout-mod-card-min-width:\s*150px;[\s\S]*--layout-mod-card-poster-height:\s*220px;/,
  );
});
```

- [ ] **Step 2: 运行测试并确认失败**

运行：

```powershell
cmd /c corepack pnpm run test
```

Expected: FAIL。失败点应指向当前 Mod 管理页仍存在硬编码面板宽度、卡片宽度或海报高度。

- [ ] **Step 3: 不单独提交失败测试**

保持测试文件未提交，直到 Task 4 让它通过。

---

### Task 4: 实现 Mod 管理页响应式密度

**Files:**
- Modify: `src/features/mods/ModLibraryPage.css`
- Test: `src/shared/styles/layoutTokens.test.mjs`

- [ ] **Step 1: 让 Mod 页消费内容间距 token**

在 `src/features/mods/ModLibraryPage.css` 中更新 `.mod-library`：

```css
.mod-library {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  gap: var(--layout-content-gap);
  min-width: 0;
  min-height: 0;
}
```

- [ ] **Step 2: token 化 Mod body 右侧操作面板**

更新 `.mod-library__body`：

```css
.mod-library__body {
  display: grid;
  grid-template-columns: minmax(0, 1fr) var(--layout-mod-action-panel-width);
  align-items: start;
  gap: var(--layout-content-gap);
  min-width: 0;
  min-height: 0;
}
```

- [ ] **Step 3: token 化 Mod 卡片网格**

更新 `.mod-grid`：

```css
.mod-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--layout-mod-card-min-width), 1fr));
  gap: 16px;
  align-content: start;
}
```

- [ ] **Step 4: token 化 Mod 卡片海报高度**

更新 `.mod-card__poster`，其中高度声明应为：

```css
  height: var(--layout-mod-card-poster-height);
```

本任务不修改 poster 圆角、hover 动画、选中环或装饰剪影。

- [ ] **Step 5: 把小屏卡片宽度覆盖改为局部 token 覆盖**

替换现有 `@media (max-width: 960px)` 块：

```css
@media (max-width: 960px) {
  .mod-library {
    --layout-mod-card-min-width: 170px;
  }
}
```

更新现有 `@media (max-width: 640px)` 块，让它以局部 token 覆盖开头，并保留已有 compact panel 行为：

```css
@media (max-width: 640px) {
  .mod-library {
    --layout-mod-card-min-width: 150px;
    --layout-mod-card-poster-height: 220px;
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
}
```

保留现有 `@media (max-width: 1280px)` 行为：右侧快捷操作面板在空间不足时折叠为横向操作区。

- [ ] **Step 6: 运行聚焦测试**

运行：

```powershell
cmd /c corepack pnpm run test
```

Expected: PASS。Shell、Route、Mod 密度 CSS 合约测试都应通过。

- [ ] **Step 7: 运行前端构建**

运行：

```powershell
cmd /c corepack pnpm run build
```

Expected: PASS，Vite 生产构建输出正常且没有 TypeScript 错误。

- [ ] **Step 8: 提交 Mod 密度改动**

运行：

```powershell
git add src/shared/styles/layoutTokens.test.mjs src/features/mods/ModLibraryPage.css
git commit -m "style: 调整 Mod 管理页宽屏密度"
```

---

### Task 5: 浏览器布局 Smoke 验证

**Files:**
- Inspect: `src/shared/styles/tokens.css`
- Inspect: `src/app/frame/AppFrame.css`
- Inspect: `src/app/routing/RouterOutlet.css`
- Inspect: `src/features/mods/ModLibraryPage.css`
- Modify only if verification finds a layout bug in those files.

- [ ] **Step 1: 启动固定端口 dev server**

运行：

```powershell
cmd /c corepack pnpm run dev -- --host 127.0.0.1 --port 1420
```

Expected: Vite 输出包含 `http://127.0.0.1:1420/` 的本地 URL。Task 5 期间保持服务运行，完成后停止。

- [ ] **Step 2: 打开 Mod 管理页**

打开：

```text
http://127.0.0.1:1420/
```

通过左侧侧边栏进入 `Mod 管理`。如果需要直接访问路由，使用：

```text
http://127.0.0.1:1420/mods
```

Expected: Mod 管理页正常显示工具栏、卡片网格和右侧快捷操作面板。

- [ ] **Step 3: 在每个视口运行 DOM 测量片段**

在下方矩阵中的每个视口下，等 Mod 管理页可见后，在浏览器 console 运行：

```js
(() => {
  const shell = document.querySelector(".app-shell");
  const grid = document.querySelector(".mod-grid");
  const actionPanel = document.querySelector(".compact-panel");

  if (!shell || !grid || !actionPanel) {
    return { error: "required elements missing" };
  }

  const shellRect = shell.getBoundingClientRect();
  const gridColumns = getComputedStyle(grid)
    .gridTemplateColumns
    .split(" ")
    .filter(Boolean).length;
  const hasHorizontalOverflow = document.documentElement.scrollWidth > document.documentElement.clientWidth;

  return {
    viewport: `${window.innerWidth}x${window.innerHeight}`,
    shellWidth: Math.round(shellRect.width),
    shellLeft: Math.round(shellRect.left),
    shellRight: Math.round(window.innerWidth - shellRect.right),
    gridColumns,
    actionPanelWidth: Math.round(actionPanel.getBoundingClientRect().width),
    hasHorizontalOverflow,
  };
})();
```

验证矩阵：

| Viewport | Expected shell width | Expected layout result |
| --- | --- | --- |
| `1366x768` | `<= 1366` | 无横向滚动，侧边栏和状态栏仍可用 |
| `1440x900` | `<= 1440` | 桌面基线视觉基本保持 |
| `1920x1080` | `<= 1920` | 不比当前 Full HD 布局更松散 |
| `2560x1440` | `> 1920` 且 `<= 2400` | Mod 网格比 Full HD 有更多可用列 |
| `3440x1440` | `> 2400` 且 `<= 3200` | 左右空白减少，但内容不横向失控 |
| `3840x1600` | `> 2400` 且 `<= 3200` | 操作面板仍可读，不变成宽空白面板 |
| `3840x2160` | `> 2400` 且 `<= 3200` | 不再出现 `1920px` 硬上限导致的大块空白 |
| `7680x4320` | `<= 3200` | 等效 4K 浏览器 50% 缩放，shell 居中且受上限约束 |
| `11636x6545` | `<= 3200` | 等效 4K 浏览器 33% 缩放，shell 居中且受上限约束 |
| `15360x8640` | `<= 3200` | 等效 4K 浏览器 25% 缩放，shell 居中且受上限约束 |

每个视口都应满足：`hasHorizontalOverflow` 为 `false`，`gridColumns` 不为 `0`，`shellLeft` 和 `shellRight` 接近，说明 shell 仍居中。

- [ ] **Step 4: 在可用的 4K 显示器上做真实浏览器缩放检查**

如果当前机器可用 3840x2160 显示器，分别设置浏览器缩放为 `50%`、`33%`、`25%`。

每个缩放值都要确认：

- App shell 仍居中且受上限约束。
- Mod 卡片网格、工具栏、顶部状态栏、右侧操作面板保持相对结构。
- Mod 卡片标题、状态 pill、筛选 chip、操作按钮不出现文本重叠。
- 极低缩放下允许 shell 外侧保留空白，不为了铺满屏幕破坏模块比例。

Expected: 三个缩放值分别检查。不能只用其中一个通过来替代另外两个。

- [ ] **Step 5: 只在发现视觉缺陷时提交修复**

如果 Task 5 发现 CSS 缺陷，只修改责任 CSS 文件并提交：

```powershell
git add src/shared/styles/tokens.css src/app/frame/AppFrame.css src/app/routing/RouterOutlet.css src/features/mods/ModLibraryPage.css
git commit -m "fix: 稳定宽屏响应式布局"
```

如果没有 CSS 改动，不创建空提交。

---

### Task 6: 最终验证

**Files:**
- No planned source edits. Modify only if verification reveals a regression.

- [ ] **Step 1: 运行前端测试**

运行：

```powershell
cmd /c corepack pnpm run test
```

Expected: PASS。现有 Mod selection 测试、route transition 测试和新的 layout token 测试都通过。

- [ ] **Step 2: 运行前端类型检查**

运行：

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: PASS。

- [ ] **Step 3: 运行前端 lint**

运行：

```powershell
cmd /c corepack pnpm run lint
```

Expected: PASS。

- [ ] **Step 4: 运行前端构建**

运行：

```powershell
cmd /c corepack pnpm run build
```

Expected: PASS。

- [ ] **Step 5: 运行统一仓库验证**

运行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected: PASS。该命令覆盖 policy、whitespace、doc links、frontend boundary、secret scan、前端 typecheck/lint/build、Rust tests 和 Rust check。

- [ ] **Step 6: 记录手动验证证据**

在最终实现回复或 PR 描述中记录：

- 实际通过的自动化命令。
- 已检查的浏览器视口。
- 是否能在真实 4K 显示器上验证 `50%`、`33%`、`25%` 缩放。
- 未执行的低缩放场景及原因。

- [ ] **Step 7: 只在最终验证要求改动时提交修复**

如果最终验证导致修复，提交：

```powershell
git add src/shared/styles/layoutTokens.test.mjs src/shared/styles/tokens.css src/app/frame/AppFrame.css src/app/routing/RouterOutlet.css src/features/mods/ModLibraryPage.css
git commit -m "fix: 完善宽屏响应式验证问题"
```

如果没有改动，不创建空提交。

---

## Self-Review Notes

- Spec coverage: 本计划覆盖 shell 扩展、最大宽度保留、布局 token 化、route aside 宽度、Mod 操作面板密度、Mod 卡片网格密度、小屏不回退、带鱼屏/超宽屏行为，以及 `50%` / `33%` / `25%` 低缩放必测要求。
- Skeleton boundary: 设计文档提到卡片和 skeleton 的栅格节奏。当前 Mod 管理代码没有 loading skeleton。本计划不新增未使用的 loading state，而是建立 token 化 `.mod-grid` 契约，后续 Mod skeleton 必须复用该规则。
- Scope check: 这是单一前端布局计划。不修改 Tauri command、Rust crate、游戏适配器、InstallPlan、manifest、backup、rollback、文件写入或玩家数据逻辑。
- Placeholder scan: 计划包含具体文件、代码片段、命令和期望结果，没有依赖未说明的实现工作。
- Type consistency: 新测试中只定义 `readProjectFile(relativePath)` 一个辅助函数，后续测试均一致使用它。
