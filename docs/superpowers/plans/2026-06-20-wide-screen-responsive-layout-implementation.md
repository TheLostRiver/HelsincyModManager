# 全视口响应式布局 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落地全视口响应式布局方案，让 App Shell、路由层、Dashboard、Mod 管理页在 `375px` 到 `15360px` 的全视口区间（含 4K 低缩放）保持结构稳定、无横向滚动、无内容裁切，并在放大方向逐级提升信息密度。

**Architecture:** 纯前端样式层。先在 `tokens.css` 增加全局布局 token 和宽屏 `min-width` 断点，再让 AppFrame、RouterOutlet、Dashboard、Mod 管理页消费这些 token；所有改动**采用最小 patch（精确替换目标行）**，不重写整个选择器块。测试分三层：CSS 合约（token 存在 + 小屏契约负向保护）、CSS 结构解析（断点/选择器关系）、浏览器 DOM 行为（手动 smoke，分层 fixture 辅助）。不修改 Tauri command、Rust crate、InstallPlan、manifest、backup、rollback、游戏适配器、路由状态机或 Mod 数据模型。

**Tech Stack:** React 19、TypeScript、Vite、CSS custom properties、CSS media query、Node 内置测试运行器（`node --test`）、可选 Playwright（手动浏览器 smoke）。

---

## 设计与实现约束（务必先读）

### 最小 patch 原则

所有 CSS 改动**只替换需要变化的声明行**，不重写整个规则块。原因：

1. 避免悄悄回退他人后续新增的属性。
2. 让 diff 可读、可 review。
3. 符合 `AGENTS.md` 的"手工编辑优先使用 patch、不做无关重构"。

**示例**：要 token 化 `.app-shell` 的 `max-width`，只改一行：

```diff
 .app-shell {
   display: grid;
   grid-template-columns: 240px minmax(0, 1fr);
   width: 100%;
-  max-width: 1920px;
+  max-width: var(--layout-shell-max-width);
   margin: 0 auto;
   ...
 }
```

**禁止**把整个 `.app-shell { ... }` 块全量替换。

### 测试分层策略

原方案用纯字符串正则断言，**测不出 cascade、测不出溢出、测不出断点触发**。本计划用三层：

| 层级 | 工具 | 能验证什么 | 不能验证什么 |
| --- | --- | --- | --- |
| L1 token 合约 | `node --test` + 正则 | token 名存在、宽屏断点数值、硬编码已消除 | cascade 实际值 |
| L2 CSS 结构解析 | `node --test` + 轻量 CSS 解析（正则切分规则块） | 选择器与 media query 的包含关系、小屏契约规则未被删除 | 真实渲染 |
| L3 浏览器 DOM 行为 | 手动 smoke + 可选 Playwright | shell 实际宽度、横向溢出、列数、裁切、真实页面/模式差异 | — |

L1/L2 跑在 `pnpm run test` 里，是**回归护栏**；L3 是**验收门**，不可跳过。三者互补：L1/L2 守住"源码没回退"，L3 证明"当前验收范围内运行时稳定"。不要把 L3 的抽样结果表述成"任意视口都已被证明"。

### 文件结构

- 新建 `src/shared/styles/layoutTokens.test.mjs`：L1 + L2 测试。
- 新建 `src/shared/styles/layout.fixture.html`：最小 DOM 骨架，供 L3 手动加载测量；**只作辅助，不替代真实页面验收**。
- 修改 `src/shared/styles/tokens.css`：布局 token + 宽屏断点。
- 修改 `src/app/frame/AppFrame.css`：shell/padding/gap token。
- 修改 `src/app/routing/RouterOutlet.css`：route aside token。
- 修改 `src/features/dashboard/Dashboard.css`：workbench + setup-rail token。
- 修改 `src/features/mods/ModLibraryPage.css`：mod 密度 token + 小屏局部覆盖。
- 不修改任何 `.tsx`，除非编译期发现真问题。
- 不新增 Mod loading skeleton；建立 token 化 `.mod-grid` 契约，未来 skeleton 必须复用。

## 重要上下文

当前行为（已核对源码，含行号）：

- `AppFrame.css:5` `.app-shell { max-width: 1920px }`（硬编码）。
- `AppFrame.css:9` `.app-shell { overflow: hidden }`（裁切溢出，缩小方向最大风险，但保留）。
- `RouterOutlet.css:12` `.route-transition__layer { grid-template-columns: minmax(0, 1fr) 360px }`（硬编码）。
- `Dashboard.css:3` `.workbench-body { grid-template-columns: minmax(0, 1fr) 360px }`（硬编码，**Dashboard 真正消费的双列容器**）。
- `Dashboard.css:374` `.setup-rail { width: 360px }`（硬编码）。
- `ModLibraryPage.css:28` `.mod-library__body { grid-template-columns: minmax(0, 1fr) 168px }`。
- `ModLibraryPage.css:157` `.mod-grid { grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)) }`。
- `ModLibraryPage.css:172` `.mod-card__poster { height: 268px }`。
- DOM 层级：`.app-shell > .app-surface > .workbench-body > .route-transition > .route-transition__layer > 页面`。

响应式目标：

- `<= 1920px`：保留现有桌面基线（含全部 `max-width` 小屏契约）。
- `1921px – 2560px`：shell 2400px。
- `2561px – 3200px`：shell 2880px。
- `> 3200px`：shell 封顶 `min(100vw, 3200px)`。
- `375px` 及更窄：保证无横向滚动、无裁切，不追求美观。
- 低高度窗口：至少补测 `1280x720` / `1280x640`，避免只盯宽度不盯可达性。

---

### Task 1: 添加布局 token 合约与结构测试（L1 + L2）

**Files:**
- Create: `src/shared/styles/layoutTokens.test.mjs`

- [ ] **Step 1: 写出失败的 L1 + L2 测试**

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

/**
 * L2 辅助：把 CSS 切成顶层规则块，返回 [{selector, body, media}]。
 * 仅处理本计划需要的结构（:root、媒体查询、普通选择器），不做完整 CSS 解析。
 */
function parseCssRules(css) {
  const rules = [];
  let currentMedia = null;
  const mediaRe = /@media\s*([^{]+)\s*{([\s\S]*?)}\s*(?=@media|:root|\.|$)/g;
  let mediaMatch;
  while ((mediaMatch = mediaRe.exec(css)) !== null) {
    currentMedia = mediaMatch[1].trim();
    const inner = mediaMatch[2];
    const innerRe = /([^{]+)\s*{([^}]*)}/g;
    let innerMatch;
    while ((innerMatch = innerRe.exec(inner)) !== null) {
      rules.push({ selector: innerMatch[1].trim(), body: innerMatch[2], media: currentMedia });
    }
  }
  // 顶层非媒体规则
  const topRe = /(^|})\s*([^{@][^{]*?)\s*{([^}]*)}/g;
  let topMatch;
  while ((topMatch = topRe.exec(css)) !== null) {
    rules.push({ selector: topMatch[2].trim(), body: topMatch[3], media: null });
  }
  return rules;
}

// ===== L1: token 存在与硬编码消除 =====

test("tokens.css 定义全部布局 token", () => {
  const tokensCss = readProjectFile("src/shared/styles/tokens.css");
  for (const tokenName of [
    "--layout-shell-max-width",
    "--layout-page-padding",
    "--layout-content-gap",
    "--layout-route-aside-width",
    "--layout-mod-action-panel-width",
    "--layout-mod-card-min-width",
    "--layout-mod-card-poster-height",
    "--layout-text-overflow",
  ]) {
    assert.match(tokensCss, new RegExp(`${tokenName}:`), `缺少 token: ${tokenName}`);
  }
});

test("tokens.css 宽屏断点逐级覆盖 shell max-width", () => {
  const tokensCss = readProjectFile("src/shared/styles/tokens.css");
  assert.match(tokensCss, /--layout-shell-max-width:\s*1920px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*1921px\)\s*{[\s\S]*--layout-shell-max-width:\s*2400px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*2561px\)\s*{[\s\S]*--layout-shell-max-width:\s*2880px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*3201px\)\s*{[\s\S]*--layout-shell-max-width:\s*min\(100vw,\s*3200px\);/);
});

test("AppFrame 不再硬编码 1920px，改为 token", () => {
  const css = readProjectFile("src/app/frame/AppFrame.css");
  assert.doesNotMatch(css, /\.app-shell[\s\S]*?max-width:\s*1920px;/);
  assert.match(css, /\.app-shell[\s\S]*?max-width:\s*var\(--layout-shell-max-width\);/);
  assert.match(css, /\.app-surface[\s\S]*?gap:\s*var\(--layout-content-gap\);/);
  assert.match(css, /\.app-surface[\s\S]*?padding:\s*var\(--layout-page-padding\);/);
});

test("RouterOutlet 与 Dashboard 都消费 route aside token，且无残留 360px 硬编码", () => {
  for (const file of ["src/app/routing/RouterOutlet.css", "src/features/dashboard/Dashboard.css"]) {
    const css = readProjectFile(file);
    assert.match(
      css,
      /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+var\(--layout-route-aside-width\);/,
      `${file} 未 token 化双列`,
    );
  }
  const routerCss = readProjectFile("src/app/routing/RouterOutlet.css");
  assert.doesNotMatch(routerCss, /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+360px;/);
  const dashCss = readProjectFile("src/features/dashboard/Dashboard.css");
  assert.doesNotMatch(dashCss, /\.workbench-body[\s\S]*?grid-template-columns:[^;]*360px;/);
  assert.doesNotMatch(dashCss, /\.setup-rail[\s\S]*?width:\s*360px;/);
});

test("Mod 管理页消费密度 token，无残留硬编码", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");
  assert.match(css, /minmax\(0,\s*1fr\)\s+var\(--layout-mod-action-panel-width\);/);
  assert.match(css, /repeat\(auto-fill,\s*minmax\(var\(--layout-mod-card-min-width\),\s*1fr\)\)/);
  assert.match(css, /\.mod-card__poster[\s\S]*?height:\s*var\(--layout-mod-card-poster-height\);/);
  assert.doesNotMatch(css, /\.mod-library__body[\s\S]*?minmax\(0,\s*1fr\)\s+168px;/);
  assert.doesNotMatch(css, /\.mod-grid[\s\S]*?minmax\(200px,\s*1fr\)/);
  assert.doesNotMatch(css, /\.mod-card__poster[\s\S]*?height:\s*268px;/);
});

// ===== L2: 小屏契约负向保护（不得删除/破坏）=====

test("AppFrame 小屏契约保留：1360px 状态栏降级 + 860px shell 单列", () => {
  const css = readProjectFile("src/app/frame/AppFrame.css");
  assert.match(css, /@media\s*\(max-width:\s*1360px\)\s*{[\s\S]*\.window-tools\s*{[\s\S]*display:\s*none/);
  assert.match(css, /@media\s*\(max-width:\s*1360px\)\s*{[\s\S]*\.status-pill:not\(\.compact\)\s*{[\s\S]*display:\s*none/);
  assert.match(css, /@media\s*\(max-width:\s*860px\)\s*{[\s\S]*\.app-shell:not\(\[data-sidebar-mode="floating"\]\)[\s\S]*grid-template-columns:\s*1fr/);
  assert.match(css, /@media\s*\(max-width:\s*860px\)\s*{[\s\S]*\.app-surface[\s\S]*padding:\s*16px/);
});

test("RouterOutlet 与 Dashboard 小屏契约保留：1360px 单列化", () => {
  for (const file of ["src/app/routing/RouterOutlet.css", "src/features/dashboard/Dashboard.css"]) {
    const css = readProjectFile(file);
    assert.match(
      css,
      /@media\s*\(max-width:\s*1360px\)\s*{[\s\S]*grid-template-columns:\s*1fr;/,
      `${file} 缺少 1360px 单列化`,
    );
  }
});

test("关键承压容器保留 min-width: 0 护栏", () => {
  const checks = [
    ["src/app/frame/AppFrame.css", [".app-surface", ".top-status-bar", ".current-game"]],
    ["src/app/routing/RouterOutlet.css", [".route-transition", ".route-transition__layer"]],
    ["src/features/dashboard/Dashboard.css", [".main-workspace", ".setup-rail"]],
    ["src/features/mods/ModLibraryPage.css", [".mod-library", ".mod-library__body", ".mod-library__main", ".compact-panel__stack", ".compact-action__left"]],
  ];

  for (const [file, selectors] of checks) {
    const css = readProjectFile(file);
    for (const selector of selectors) {
      const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      assert.match(
        css,
        new RegExp(`${escaped}[\\s\\S]*?min-width:\\s*0`),
        `${file} 缺少关键容器护栏: ${selector}`,
      );
    }
  }
});

test("Mod 管理页小屏契约保留：1280/960/640 断点", () => {
  const css = readProjectFile("src/features/mods/ModLibraryPage.css");
  assert.match(css, /@media\s*\(max-width:\s*1280px\)/);
  assert.match(css, /@media\s*\(max-width:\s*960px\)\s*{[\s\S]*--layout-mod-card-min-width:\s*170px/);
  assert.match(
    css,
    /@media\s*\(max-width:\s*640px\)\s*{[\s\S]*--layout-mod-card-min-width:\s*150px;[\s\S]*--layout-mod-card-poster-height:\s*220px/,
  );
});

// ===== L2: 断点方向不冲突 =====

test("宽屏断点全部为 min-width，不与 max-width 小屏断点方向冲突", () => {
  const tokensCss = readProjectFile("src/shared/styles/tokens.css");
  const wideBlocks = tokensCss.match(/@media\s*\(min-width:[^)]+\)\s*{[\s\S]*?--layout-/g) ?? [];
  assert.ok(wideBlocks.length >= 3, "宽屏断点至少应有 3 个（1921/2561/3201）");
  for (const block of wideBlocks) {
    assert.match(block, /min-width:\s*(1921|2561|3201)px/, `意外断点值: ${block}`);
  }
});
```

- [ ] **Step 2: 运行测试并确认失败**

```powershell
cmd /c corepack pnpm run test
```

Expected: FAIL。失败应指向 `tokens.css` 缺少 token、AppFrame/RouterOutlet/Dashboard/Mod 仍有硬编码。

- [ ] **Step 3: 不单独提交失败测试**

保持测试文件未提交，直到 Task 3 让它通过。

---

### Task 2: 实现全局布局 Token

**Files:**
- Modify: `src/shared/styles/tokens.css`

- [ ] **Step 1: 在 light :root 块插入基础布局 token**

在 `src/shared/styles/tokens.css` 的 `:root, :root[data-color-scheme="light"]` 块内，紧跟 `--space-content-gap: 20px;` 之后插入（**只插入，不动其它行**）：

```css
  --layout-shell-max-width: 1920px;
  --layout-page-padding: var(--space-page);
  --layout-content-gap: var(--space-content-gap);
  --layout-route-aside-width: 360px;
  --layout-mod-action-panel-width: 168px;
  --layout-mod-card-min-width: 200px;
  --layout-mod-card-poster-height: 268px;
  --layout-text-overflow: ellipsis;
```

- [ ] **Step 2: 在文件末尾追加宽屏断点**

在 `src/shared/styles/tokens.css` 文件末尾（dark theme block 之后）追加：

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

> 注意：宽屏断点**不覆盖** `--layout-route-aside-width`。这是刻意取舍——Dashboard 右侧状态栏在宽屏保持 360px，避免空面板。

- [ ] **Step 3: 运行类型检查**

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: PASS（仅改 CSS，TS 无影响）。

---

### Task 3: AppFrame 与 RouterOutlet 消费 token（最小 patch）

**Files:**
- Modify: `src/app/frame/AppFrame.css`
- Modify: `src/app/routing/RouterOutlet.css`
- Test: `src/shared/styles/layoutTokens.test.mjs`

- [ ] **Step 1: AppFrame — 仅替换 4 个声明行**

在 `src/app/frame/AppFrame.css` 中做 4 处精确替换（**不要重写整个规则块**）：

```diff
-  max-width: 1920px;
+  max-width: var(--layout-shell-max-width);
```
（`.app-shell` 内）

```diff
-  gap: var(--space-content-gap);
+  gap: var(--layout-content-gap);
```
（`.app-surface` 内）

```diff
-  padding: var(--space-page);
+  padding: var(--layout-page-padding);
```
（`.app-surface` 内）

**保留** `@media (max-width: 860px)` 中的 `padding: 16px;`（小屏契约，不动）。

- [ ] **Step 2: RouterOutlet — 仅替换 1 个声明行**

在 `src/app/routing/RouterOutlet.css` 的 `.route-transition__layer` 内：

```diff
-  grid-template-columns: minmax(0, 1fr) 360px;
+  grid-template-columns: minmax(0, 1fr) var(--layout-route-aside-width);
```

**保留** `gap: 28px;`（Dashboard 基线，本任务不动）。**保留** `@media (max-width: 1360px)`（小屏契约）。

- [ ] **Step 3: 运行聚焦测试**

```powershell
cmd /c corepack pnpm run test
```

Expected: token 存在类测试 PASS；硬编码消除类测试对 Dashboard/Mod 仍 FAIL（后续 Task 处理）。

- [ ] **Step 4: 运行类型检查**

```powershell
cmd /c corepack pnpm run typecheck
```

Expected: PASS。

- [ ] **Step 5: 提交 shell 与 route 布局 token**

```powershell
git add src/shared/styles/tokens.css src/app/frame/AppFrame.css src/app/routing/RouterOutlet.css
git commit -m "style: 添加宽屏布局 token 并让 AppFrame/RouterOutlet 消费"
```

---

### Task 4: Dashboard 消费 route aside token（最小 patch）

**Files:**
- Modify: `src/features/dashboard/Dashboard.css`
- Test: `src/shared/styles/layoutTokens.test.mjs`

- [ ] **Step 1: workbench-body — 替换 1 个声明行**

在 `.workbench-body` 内：

```diff
-  grid-template-columns: minmax(0, 1fr) 360px;
+  grid-template-columns: minmax(0, 1fr) var(--layout-route-aside-width);
```

- [ ] **Step 2: setup-rail — 替换 1 个声明行**

在 `.setup-rail` 内：

```diff
-  width: 360px;
+  width: var(--layout-route-aside-width);
```

**保留** `@media (max-width: 1360px)` 的 `.setup-rail { width: auto; }`（小屏契约）。**保留** `.setup-panel { min-height: 360px }`（内容高度，非布局）。

- [ ] **Step 3: 运行聚焦测试**

```powershell
cmd /c corepack pnpm run test
```

Expected: Dashboard 硬编码消除测试 PASS。

- [ ] **Step 4: 提交 Dashboard token 化**

```powershell
git add src/features/dashboard/Dashboard.css
git commit -m "style: Dashboard 双列容器消费 route aside token"
```

---

### Task 5: Mod 管理页响应式密度（最小 patch）

**Files:**
- Modify: `src/features/mods/ModLibraryPage.css`
- Test: `src/shared/styles/layoutTokens.test.mjs`

- [ ] **Step 1: .mod-library — 替换 gap**

```diff
-  gap: var(--space-content-gap);
+  gap: var(--layout-content-gap);
```

- [ ] **Step 2: .mod-library__body — 替换列宽与 gap**

```diff
-  grid-template-columns: minmax(0, 1fr) 168px;
+  grid-template-columns: minmax(0, 1fr) var(--layout-mod-action-panel-width);
```
```diff
-  gap: 20px;
+  gap: var(--layout-content-gap);
```

- [ ] **Step 3: .mod-grid — 替换列宽**

```diff
-  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
+  grid-template-columns: repeat(auto-fill, minmax(var(--layout-mod-card-min-width), 1fr));
```

- [ ] **Step 4: .mod-card__poster — 替换高度**

```diff
-  height: 268px;
+  height: var(--layout-mod-card-poster-height);
```

- [ ] **Step 5: 小屏覆盖改为局部 token 覆盖**

替换 `@media (max-width: 960px)` 块内的 `.mod-grid` 规则为局部 token 覆盖：

```diff
 @media (max-width: 960px) {
-  .mod-grid {
-    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
+  .mod-library {
+    --layout-mod-card-min-width: 170px;
   }
 }
```

更新 `@media (max-width: 640px)` 块，把卡片宽度/高度覆盖改为局部 token 覆盖，**保留** `.compact-panel__stack` 和 `.compact-action` 的现有规则不动：

```diff
 @media (max-width: 640px) {
-  .mod-grid {
-    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
-    gap: 12px;
-  }
-
-  .mod-card__poster {
-    height: 220px;
-  }
+  .mod-library {
+    --layout-mod-card-min-width: 150px;
+    --layout-mod-card-poster-height: 220px;
+  }
+
+  .mod-grid {
+    gap: 12px;
+  }

   .compact-panel__stack {
     grid-auto-columns: auto;
     ...
   }
   ...
 }
```

**保留** `@media (max-width: 1280px)` 块整体不动（小屏契约）。

- [ ] **Step 6: 运行聚焦测试**

```powershell
cmd /c corepack pnpm run test
```

Expected: 全部 token 合约与硬编码消除测试 PASS。

- [ ] **Step 7: 运行构建**

```powershell
cmd /c corepack pnpm run build
```

Expected: PASS，Vite 生产构建无 TS 错误。

- [ ] **Step 8: 提交 Mod 密度改动**

```powershell
git add src/shared/styles/layoutTokens.test.mjs src/features/mods/ModLibraryPage.css
git commit -m "style: Mod 管理页消费密度 token 并保留小屏契约"
```

---

### Task 6: 创建布局 DOM 骨架 fixture（L3 辅助）

**Files:**
- Create: `src/shared/styles/layout.fixture.html`

- [ ] **Step 1: 写出最小 DOM 骨架**

创建 `src/shared/styles/layout.fixture.html`，包含 `.app-shell > .app-surface > .workbench-body > .route-transition__layer` 的最小结构，并 `<link>` 到 tokens.css、AppFrame.css、RouterOutlet.css、Dashboard.css、ModLibraryPage.css。在 `.app-shell` 内放一个 `.mod-grid` 占位（含若干 `.mod-card`）和一个 `.setup-rail` 占位。

目的：在不启动完整 Vite dev server 时，也能用浏览器直接打开此文件快速测量 shell 宽度、横向溢出、列数。fixture **不参与构建产物**（Vite 只处理被 import 的资源）。

限制：fixture 不能完整复现真实路由切换、classic/floating 侧边栏差异、Dashboard 右侧 rail 的真实内容高度、sticky 面板行为。因此它只能作为 L3 的辅助载体，不能单独替代真实页面验收。

```html
<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Layout Fixture</title>
  <link rel="stylesheet" href="./tokens.css" />
  <link rel="stylesheet" href="../../app/frame/AppFrame.css" />
  <link rel="stylesheet" href="../../app/routing/RouterOutlet.css" />
  <link rel="stylesheet" href="../../features/dashboard/Dashboard.css" />
  <link rel="stylesheet" href="../../features/mods/ModLibraryPage.css" />
  <style>
    /* 仅 fixture 用：模拟 reset 的最小盒模型 */
    *, *::before, *::after { box-sizing: border-box; }
    body { margin: 0; }
  </style>
</head>
<body data-color-scheme="light">
  <div class="app-shell" data-sidebar-mode="classic">
    <aside class="sidebar" style="display:flex;flex-direction:column;gap:14px;padding:20px 16px;background:var(--color-surface-muted);min-height:0;">
      <div class="brand-block"><h1>Fixture</h1></div>
    </aside>
    <div class="app-surface">
      <header class="top-status-bar">
        <div class="current-game"><strong>Fixture Game With A Very Long Title That Might Overflow</strong><span>已就绪</span></div>
        <div class="status-actions"><span class="status-pill"><span class="dot neutral-dot"></span><strong>状态</strong></span></div>
        <div class="window-tools"><span>window</span></div>
      </header>
      <main class="workbench-body">
        <div class="route-transition__layer">
          <section class="mod-library">
            <div class="mod-library__body">
              <div class="mod-grid">
                <!-- 重复 12 张占位卡 -->
                <article class="mod-card"><div class="mod-card__poster"></div><div class="mod-card__meta"><strong class="mod-card__title">Mod Title</strong><span class="mod-card__size">42 MB</span></div></article>
              </div>
              <aside class="compact-panel"><div class="compact-panel__header"><h3 class="compact-panel__title">操作</h3></div><div class="compact-panel__stack"><button class="compact-action is-primary"><span class="compact-action__label">添加</span></button></div></aside>
            </div>
          </section>
        </div>
      </main>
    </div>
  </div>
</body>
</html>
```

（实际文件里把 `.mod-card` 重复 12 次。）

- [ ] **Step 2: 不提交单独的 fixture 步骤**

fixture 与 Task 7 的验证一起提交。

---

### Task 7: 浏览器 DOM 行为验证（L3 验收门）

**Files:**
- Inspect: `src/shared/styles/layout.fixture.html` 或 dev server
- Modify only if verification finds a layout bug in 4 个 CSS 文件。

- [ ] **Step 1: 选择验证载体**

先用 Task 6 的 `layout.fixture.html` 做快速测量，再**必须**补跑真实页面。若需验证真实路由与组件，起 dev server：

```powershell
cmd /c corepack pnpm run dev -- --host 127.0.0.1 --port 1420
```

- [ ] **Step 2: 在每个视口运行 DOM 测量片段**

在下方矩阵的每个视口下（用 DevTools Responsive 模拟），先打开 fixture 做壳体测量，再对真实 `/mods` 路由复测；Dashboard 则至少运行一次同类测量或做等价目视检查。等渲染完成后在 console 运行：

```js
(() => {
  const shell = document.querySelector(".app-shell");
  const grid = document.querySelector(".mod-grid");
  if (!shell || !grid) return { error: "required elements missing" };
  const shellRect = shell.getBoundingClientRect();
  const gridColumns = getComputedStyle(grid).gridTemplateColumns.split(" ").filter(Boolean).length;
  const overflowX = document.documentElement.scrollWidth - document.documentElement.clientWidth;
  // 检测可交互元素是否被 .app-shell 裁切（粗略：检查是否有元素 right 超出 shell right）
  const shellRight = shellRect.right;
  const interactive = Array.from(document.querySelectorAll("button, a, input"));
  const clipped = interactive.filter(el => {
    const r = el.getBoundingClientRect();
    return r.right > shellRight + 1 || r.left < shellRect.left - 1;
  }).map(el => el.className || el.tagName);
  return {
    viewport: `${window.innerWidth}x${window.innerHeight}`,
    shellWidth: Math.round(shellRect.width),
    shellLeft: Math.round(shellRect.left),
    shellRightGap: Math.round(window.innerWidth - shellRect.right),
    gridColumns,
    overflowX,
    clippedCount: clipped.length,
    clippedSample: clipped.slice(0, 3),
  };
})();
```

- [ ] **Step 2.5: 运行焦点可达性片段**

在真实 `/mods` 与 Dashboard 页面各至少运行一次。目标：补齐 `overflowX` / `clippedCount` 之外的键盘交互可达性证据。

```js
(() => {
  const shell = document.querySelector(".app-shell");
  if (!shell) return { error: "shell missing" };
  const shellRect = shell.getBoundingClientRect();
  const focusables = Array.from(
    document.querySelectorAll('button, a[href], input, select, textarea, [tabindex]:not([tabindex="-1"])'),
  ).filter(el => !el.hasAttribute("disabled"));

  const sample = focusables.slice(0, 12).map(el => {
    el.focus();
    const r = el.getBoundingClientRect();
    return {
      label: el.textContent?.trim() || el.getAttribute("aria-label") || el.tagName,
      hiddenByAttr: !!el.closest("[hidden],[aria-hidden='true']"),
      zeroSized: r.width <= 0 || r.height <= 0,
      outsideShell:
        r.right > shellRect.right + 1 ||
        r.left < shellRect.left - 1 ||
        r.bottom > shellRect.bottom + 1 ||
        r.top < shellRect.top - 1,
    };
  });

  return {
    sampled: sample.length,
    failures: sample.filter(item => item.hiddenByAttr || item.zeroSized || item.outsideShell),
    sample,
  };
})();
```

期望：`failures.length === 0`。若某控件使用父层 focus ring 而非自身 outline，可在记录中注明，但仍需证明焦点位置可见、未被裁切。

- [ ] **Step 3: 全视口验收矩阵（放大 + 缩小双向）**

| Viewport | 区间 | shellWidth 预期 | gridColumns | overflowX | clipped |
| --- | --- | --- | --- | --- | --- |
| `375x812` | 缩小·手机 | `<= 375` | `>= 1` | `0` | `[]` |
| `800x600` | 缩小·窄窗 | `<= 800` | `>= 1` | `0` | `[]` |
| `1024x768` | 缩小·小本 | `<= 1024` | `>= 1` | `0` | `[]` |
| `1366x768` | 基线·小本 | `<= 1366` | `>= 1` | `0` | `[]` |
| `1440x900` | 基线 | `<= 1440` | `>= 1` | `0` | `[]` |
| `1920x1080` | 基线·FHD | `<= 1920` | `>= 1` | `0` | `[]` |
| `2560x1440` | 放大·2K | `> 1920 && <= 2400` | `>` FHD 列数 | `0` | `[]` |
| `3440x1440` | 放大·21:9 | `<= 2880` | 更多 | `0` | `[]` |
| `3840x1600` | 放大·超宽 | `<= 3200` | 更多 | `0` | `[]` |
| `3840x2160` | 放大·4K | `<= 3200` | 更多 | `0` | `[]` |
| `7680x4320` | 放大·4K@50% | `<= 3200` | 更多 | `0` | `[]` |
| `11636x6545` | 放大·4K@33% | `<= 3200` | 更多 | `0` | `[]` |
| `15360x8640` | 放大·4K@25% | `<= 3200` | 更多 | `0` | `[]` |

**每个视口都必须**：`overflowX === 0`、`clippedCount === 0`、`gridColumns >= 1`、`shellLeft ≈ shellRightGap`（居中）。

另外补充两组低高度窗口：

| Viewport | 区间 | 重点 |
| --- | --- | --- |
| `1280x720` | 低高度·桌面 | sticky 面板、顶部状态栏、主操作可达 |
| `1280x640` | 低高度·极限 | floating 侧边栏高度断点、内容滚动区可用 |

- [ ] **Step 4: 小屏契约与真实模式回归目视检查**

在 `375x812`、`800x600`、`1366x768` 三个视口目视确认，并分别抽样 classic / floating 两种侧边栏模式：

- `<= 1360px`：`.window-tools` 已隐藏、非 compact `.status-pill` 已隐藏。
- `<= 860px`：shell 已单列（非浮动模式）、surface padding 为 16px。
- `<= 640px`：Mod 卡片更小、compact-panel 已单列。
- 超长游戏名（fixture 里故意放的长标题）被 `text-overflow: ellipsis` 截断，未撑破状态栏。
- floating 模式下，浮动侧边栏不会遮住主操作按钮或右侧关键内容。
- Dashboard 真实页面下，右侧 setup/status rail 在 `<= 1360px` 单列化后没有出现裁切或顺序错乱。
- 键盘 Tab 导航下，至少一轮主操作按钮、搜索输入、紧凑操作按钮的焦点移动可见且未被 shell 裁切。

- [ ] **Step 5: 连续拖拽与真实 4K 缩放检查（若硬件可用）**

先做一轮连续拖拽观察：从 `1366px` 持续拖到 `375px`，确认没有在**非断点位置**突然出现横向滚动、按钮消失、文本重叠或 rail/面板闪断。

若有 3840x2160 显示器，分别设浏览器缩放 `50%`、`33%`、`25%`，每个都确认：shell 居中且 `<= 3200px`、无横向滚动、无文本重叠。**三个缩放值分别测，不可互相替代**。

> 若无硬件，用 DevTools Responsive 模拟 `7680/11636/15360` 宽度替代，并在证据中注明"用设备模拟而非真实缩放"。

- [ ] **Step 6: 只在发现缺陷时提交修复**

若发现 CSS 缺陷，**只改责任文件的对应行**（最小 patch），提交：

```powershell
git add src/shared/styles/layout.fixture.html src/shared/styles/tokens.css src/app/frame/AppFrame.css src/app/routing/RouterOutlet.css src/features/dashboard/Dashboard.css src/features/mods/ModLibraryPage.css
git commit -m "fix: 修复全视口响应式布局缺陷"
```

若无缺陷，仅提交 fixture：

```powershell
git add src/shared/styles/layout.fixture.html
git commit -m "test: 添加全视口响应式布局 DOM fixture"
```

---

### Task 8: 最终验证

**Files:**
- No planned source edits. Modify only if verification reveals a regression.

- [ ] **Step 1: 前端测试**

```powershell
cmd /c corepack pnpm run test
```

Expected: PASS（含 L1 token 合约、L2 小屏契约负向、断点方向）。

- [ ] **Step 2: 类型检查**

```powershell
cmd /c corepack pnpm run typecheck
```

- [ ] **Step 3: lint**

```powershell
cmd /c corepack pnpm run lint
```

- [ ] **Step 4: 构建**

```powershell
cmd /c corepack pnpm run build
```

- [ ] **Step 5: 统一仓库验证**

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected: PASS（policy、whitespace、doc links、frontend boundary、secret scan、前端 typecheck/lint/build、Rust tests、Rust check）。

- [ ] **Step 6: 记录手动验证证据**

在最终回复或 PR 描述中记录：

- 实际通过的自动化命令。
- 已检查的浏览器视口（含缩小方向 375/800/1024、低高度 1280x720/640 与放大方向各档）。
- `overflowX === 0` 与 `clippedCount === 0` 的实测结果。
- 焦点可达性片段的抽样结果，及任何需要人工解释的焦点呈现差异。
- fixture 与真实页面各自检查了哪些路由、哪些侧边栏模式。
- 是否在真实 4K 显示器验证 50%/33%/25%；若用设备模拟替代，注明。
- 是否执行了连续拖拽检查；若未执行，注明。
- 未执行的场景及原因。

- [ ] **Step 7: 只在最终验证要求改动时提交修复**

```powershell
git add src/shared/styles/layoutTokens.test.mjs src/shared/styles/layout.fixture.html src/shared/styles/tokens.css src/app/frame/AppFrame.css src/app/routing/RouterOutlet.css src/features/dashboard/Dashboard.css src/features/mods/ModLibraryPage.css
git commit -m "fix: 完善全视口响应式验证问题"
```

若无改动，不创建空提交。

---

## Self-Review Notes

- **Spec coverage**：覆盖 shell 扩展、上限保留、双列容器同步 token 化（route layer + workbench + setup-rail）、Mod 密度、小屏契约负向保护、横向溢出三道防线、长文本兜底、放大（2K/4K/21:9/32:9/低缩放）与缩小（375–1366）全区间。
- **Dashboard 收口**：原方案漏掉的 `.workbench-body` 与 `.setup-rail` 的 `360px` 已纳入 Task 4，两处都消费 `--layout-route-aside-width`，且澄清了与 route layer 的关系。
- **最小 patch**：所有 CSS 改动明确标注"只替换目标行"，禁止全量重写规则块，杜绝悄悄回退。
- **测试分层**：L1 正则（token 存在 + 硬编码消除）+ L2 结构（小屏契约负向 + 断点方向）+ L3 浏览器 DOM（宽度/溢出/裁切）。L3 是验收门，不可跳过。
- **关键容器护栏**：L2 新增关键承压容器的 `min-width: 0` 回归断言，避免以后有人无感知删掉这层保护。
- **缩小方向**：新增 `375/800/1024` 视口验收与连续拖拽观察，`overflowX === 0` 与 `clippedCount === 0` 为硬约束。
- **真实覆盖**：fixture 只辅助测量；真实 `/mods`、Dashboard、classic/floating 模式与低高度窗口必须补测，否则不能宣称"全视口"完成验收。
- **焦点可达性**：L3 补充焦点抽样脚本；若未来需要更强保证，可进一步演进为 Playwright 键盘 smoke。
- **放大方向**：保留 50%/33%/25% 必测，并提示优先用 DevTools 设备模拟（确定性高于浏览器缩放快捷键）。
- **Scope check**：单一前端布局计划。不修改 Tauri command、Rust crate、游戏适配器、InstallPlan、manifest、backup、rollback、文件写入或玩家数据逻辑。
- **Placeholder scan**：每个 Task 含具体文件、精确 diff、命令与期望结果，无未说明的实现工作。
- **Type consistency**：测试中只定义 `readProjectFile` 与 `parseCssRules` 两个辅助，后续测试一致使用。
