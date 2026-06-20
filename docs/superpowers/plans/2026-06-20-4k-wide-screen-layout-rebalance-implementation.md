# 4K 宽屏留白修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不破坏上一轮全视口响应式稳定性的前提下，按 `B2：受控扩展型 + mods 密度微增` 修复 4K 宽屏下的明显大留白，让 `App Shell` 在 4K 下更像桌面工具，同时让 `/mods` 页把额外宽度更多转化为有效信息密度，并保持 `dashboard` 的扫视稳定性。

**Architecture:** 纯前端样式层调整。继续沿用现有 layout token 体系，不增删 `.tsx` 结构，不引入新布局容器。核心工作分成三层：第一层调整 `tokens.css` 中 `2561px+` 与 `3201px+` 的超宽数值；第二层确认 `AppFrame`、`RouterOutlet`、`Dashboard`、`ModLibraryPage` 继续通过 token 消费这些变化；第三层补强 `layoutTokens.test.mjs`，把新超宽档位数值和 `/mods` 增密意图锁成回归护栏，并通过真实浏览器在 `2560/3440/3840` 档位验收。

**Tech Stack:** React 19、TypeScript、Vite、CSS custom properties、Node `--test`、Codex 内置浏览器、本地 `localhost` 预览与真实 dev server。

---

## 设计与执行约束

### 最小 patch 原则

本轮仍遵循上一轮约束：

- 只改需要变化的 token 或声明行。
- 不重写整段 CSS 规则。
- 不把 `/mods` 的宽屏策略扩散到 `dashboard`。
- 不改 `.tsx`，除非验证证明 CSS 无法完成目标。

### 本轮只处理超宽增强

这不是重新做一轮完整响应式体系。上一轮已经通过的小屏和低高度契约必须视为锁定：

- `1360px`
- `1280px`
- `960px`
- `860px`
- `640px`
- `560px`

任何改动若影响这些断点，应视为回归。

### 页面影响边界

- `App Shell`：允许继续放宽超宽上限。
- `/mods`：允许做超宽档位的轻微增密。
- `dashboard`：只共享更宽 shell，不新增自身 4K 增密规则。

---

## 文件结构

### 修改文件

- `src/shared/styles/tokens.css`
- `src/shared/styles/layoutTokens.test.mjs`

### 重点验证但默认不改的文件

- `src/app/frame/AppFrame.css`
- `src/app/routing/RouterOutlet.css`
- `src/features/dashboard/Dashboard.css`
- `src/features/mods/ModLibraryPage.css`

### 文档

- `docs/superpowers/specs/2026-06-20-4k-wide-screen-layout-rebalance-design.md`

---

## Task 1: 先把新的 4K 宽屏目标写进测试护栏

**Files:**
- Modify: `src/shared/styles/layoutTokens.test.mjs`

- [ ] **Step 1: 读现有测试并确认当前锁的是旧超宽数值**

Run:

```powershell
Get-Content src/shared/styles/layoutTokens.test.mjs
```

Expected:

- 现有断言仍锁定 `2880px` 与 `3200px`
- 还没有表达 `B2` 的新目标数值

- [ ] **Step 2: 写失败测试，锁定新的超宽 token 数值**

在 `src/shared/styles/layoutTokens.test.mjs` 中，把超宽数值相关断言改成：

- `2561px+` 档位断言 `--layout-shell-max-width: 3040px`
- `2561px+` 档位断言 `--layout-page-padding: 32px`
- `2561px+` 档位断言 `--layout-content-gap: 20px`
- `2561px+` 档位断言 `--layout-mod-action-panel-width: 200px`
- `2561px+` 档位断言 `--layout-mod-card-min-width: 208px`
- `3201px+` 档位断言 `--layout-shell-max-width: min(100vw, 3440px)`
- `3201px+` 档位断言 `--layout-page-padding: 36px`
- `3201px+` 档位断言 `--layout-content-gap: 22px`
- `3201px+` 档位断言 `--layout-mod-action-panel-width: 212px`
- `3201px+` 档位断言 `--layout-mod-card-min-width: 212px`

同时补一条负向护栏：

- 不允许继续出现旧的 `3200px` 超宽上限断言

- [ ] **Step 3: 写一条 `dashboard` 稳定性测试**

在同一测试文件里增加一条 L1/L2 断言：

- `tokens.css` 的超宽档位不允许覆盖 `--layout-route-aside-width`

这条测试的意义是防止后续有人顺手把 `dashboard` 右 rail 也跟着放大。

- [ ] **Step 4: 跑测试确认当前是红灯**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/shared/styles/layoutTokens.test.mjs
```

Expected:

- FAIL
- 失败点应指向 `tokens.css` 当前仍是 `2880px` / `3200px` 那套旧数值

---

## Task 2: 调整全局超宽 token 到 B2 数值

**Files:**
- Modify: `src/shared/styles/tokens.css`
- Test: `src/shared/styles/layoutTokens.test.mjs`

- [ ] **Step 1: 修改 `2561px+` 档位**

在 `src/shared/styles/tokens.css` 的 `@media (min-width: 2561px)` 中，仅替换以下声明行：

```diff
-    --layout-shell-max-width: 2880px;
-    --layout-page-padding: 36px;
-    --layout-content-gap: 24px;
-    --layout-mod-action-panel-width: 208px;
-    --layout-mod-card-min-width: 220px;
+    --layout-shell-max-width: 3040px;
+    --layout-page-padding: 32px;
+    --layout-content-gap: 20px;
+    --layout-mod-action-panel-width: 200px;
+    --layout-mod-card-min-width: 208px;
```

- [ ] **Step 2: 修改 `3201px+` 档位**

在 `@media (min-width: 3201px)` 中，仅替换以下声明行：

```diff
-    --layout-shell-max-width: min(100vw, 3200px);
-    --layout-page-padding: 40px;
-    --layout-content-gap: 28px;
-    --layout-mod-action-panel-width: 220px;
-    --layout-mod-card-min-width: 230px;
+    --layout-shell-max-width: min(100vw, 3440px);
+    --layout-page-padding: 36px;
+    --layout-content-gap: 22px;
+    --layout-mod-action-panel-width: 212px;
+    --layout-mod-card-min-width: 212px;
```

不要修改：

- `1921px+` 档位
- `--layout-route-aside-width`
- 任何小屏 `max-width` 断点

- [ ] **Step 3: 跑聚焦测试确认转绿**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/shared/styles/layoutTokens.test.mjs
```

Expected:

- PASS

- [ ] **Step 4: 跑类型检查**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected:

- PASS

---

## Task 3: 代码侧复核页面消费关系，确认不需要结构改动

**Files:**
- Inspect: `src/app/frame/AppFrame.css`
- Inspect: `src/app/routing/RouterOutlet.css`
- Inspect: `src/features/dashboard/Dashboard.css`
- Inspect: `src/features/mods/ModLibraryPage.css`

- [ ] **Step 1: 复核 App Shell 仍然消费 token**

确认：

- `.app-shell` 继续使用 `var(--layout-shell-max-width)`
- `.app-surface` 继续使用 `var(--layout-page-padding)` 与 `var(--layout-content-gap)`

如果不是，停下并补回；如果是，不修改。

- [ ] **Step 2: 复核 `dashboard` 仍固定消费 `--layout-route-aside-width`**

确认：

- `.route-transition__layer`
- `.workbench-body`
- `.setup-rail`

都仍然走 `--layout-route-aside-width`

如果是，不修改；本轮设计明确不让 `dashboard` 宽屏增密。

- [ ] **Step 3: 复核 `/mods` 仍然消费 token**

确认：

- `.mod-library__body` 继续走 `--layout-mod-action-panel-width`
- `.mod-grid` 继续走 `--layout-mod-card-min-width`
- `.mod-card__poster` 继续走 `--layout-mod-card-poster-height`

如果这些消费关系仍在，则说明本轮只改 token 即可驱动 `/mods` 轻微增密。

- [ ] **Step 4: 如确实无需 CSS 文件改动，保持这些文件不动**

本任务的重要点之一就是证明：

- `B2` 的这轮修复可以由 token 数值完成
- 不需要再扩散改动面

---

## Task 4: 跑自动化验证

**Files:**
- No planned source edits

- [ ] **Step 1: 跑布局测试**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/shared/styles/layoutTokens.test.mjs
```

- [ ] **Step 2: 跑类型检查**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

- [ ] **Step 3: 跑生产构建**

Run:

```powershell
cmd /c corepack pnpm run build
```

- [ ] **Step 4: 跑统一仓库验证**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected:

- 全部 PASS

---

## Task 5: 做真实浏览器验收

**Files:**
- Inspect only
- Use existing local app / dev server

- [ ] **Step 1: 启动或复用本地前端**

优先复用已有 `http://localhost:1420/`。
若未运行，则启动：

```powershell
cmd /c corepack pnpm run dev -- --host 127.0.0.1 --port 1420
```

- [ ] **Step 2: 在 `/mods` 做宽屏验收**

至少检查：

- `2560x1440`
- `3440x1440`
- `3840x1600`
- `3840x2160`

每个视口记录：

- `window.innerWidth`
- `.app-shell` 实际宽度
- 横向溢出是否为 `0`
- 卡片列数相对当前基线是否增加

目标：

- 4K 下 shell 上限靠近 `3440px`
- 比旧版本明显减少留白
- `/mods` 不是单纯卡片变大，而是更容易多出有效列数

- [ ] **Step 3: 在 `dashboard` 做稳定性验收**

至少检查：

- `2560x1440`
- `3840x2160`

目标：

- 外层更宽
- 右侧 rail 仍维持稳定，不出现明显空面板化
- 无横向滚动

- [ ] **Step 4: 回归小屏与低高度**

至少回归：

- `1366x768`
- `1280x720`
- `1280x640`
- `800x600`
- `375x812`

目标：

- 无横向滚动
- 无明显裁切
- 之前通过的下降契约不回退

- [ ] **Step 5: classic / floating 都抽样**

对 `/mods` 至少抽样：

- classic sidebar
- floating sidebar

目标：

- 两种模式都不因更宽 shell 引入异常

---

## Task 6: 收尾与提交

**Files:**
- Modify only if verification reveals a real defect

- [ ] **Step 1: 如果浏览器验收发现问题，只做责任内最小 patch**

优先修复顺序：

1. token 数值误差
2. `/mods` 密度过松或过紧
3. 宽屏下出现的个别容器溢出

禁止：

- 顺手扩大到 `dashboard` 重设计
- 顺手改 `.tsx`
- 顺手做无关视觉重构

- [ ] **Step 2: 再次跑全量验证**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/shared/styles/layoutTokens.test.mjs
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run build
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected:

- 全部 PASS

- [ ] **Step 3: 提交 4K 修复实现**

```powershell
git add src/shared/styles/tokens.css src/shared/styles/layoutTokens.test.mjs
git commit -m "style: 优化 4K 宽屏布局利用率"
```

如果浏览器验收引出额外责任内修复，则把对应文件一并 add。

---

## Self-Review Notes

- **Spec coverage:** 已覆盖 4K 大留白修复、B2 方案、`3440px` 目标上限、`/mods` 微增密、`dashboard` 稳定、真实浏览器验收与小屏回归。
- **Scope control:** 本计划刻意把改动收缩到 token 与测试，不重开结构层工程。
- **No placeholders:** 每个任务都给出了明确文件、命令和预期。
- **Risk focus:** 主要风险集中在 `/mods` 的 4K 观感与 `dashboard` 的稳定性，均通过浏览器验收锁定。
