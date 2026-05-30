# Scaffold Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 创建 Helsincy Mod Manager 的 Tauri 2 + React + TypeScript + Rust workspace 脚手架基线，并让统一验证脚本覆盖前端、Rust、文档和治理检查。

**Architecture:** 保留 Tauri 默认的 `src-tauri` 应用入口，同时把可复用 Rust 业务能力拆到 `src-tauri/crates/*`。前端只放应用壳、导航和空状态，不承载文件系统规则。真实 Mod 安装、游戏目录写入、存档备份和日志审计实现留到后续计划。

**Tech Stack:** Tauri 2、React、TypeScript、Vite、pnpm via Corepack、Rust workspace、PowerShell 验证脚本、GitHub Actions。

---

## 执行前提

- 当前分支：`codex/scaffold-baseline`。
- 不直接修改 `main`。
- 标准 Tauri CLI 入口使用 `@tauri-apps/cli` devDependency，不要求全局安装 `cargo-tauri`。
- Windows PowerShell 5.1 下不要直接调用 `npm.ps1` / `pnpm.ps1`，统一通过 `cmd /c corepack pnpm ...` 或 npm package scripts 运行。
- 所有文档默认使用简体中文，代码命名使用英文。

## 文件结构边界

新增或修改的主要文件：

```text
package.json
pnpm-lock.yaml
pnpm-workspace.yaml
index.html
vite.config.ts
tsconfig.json
tsconfig.node.json
eslint.config.js
src/
  App.tsx
  main.tsx
  app/AppShell.tsx
  app/AppShell.css
  features/dashboard/DashboardPage.tsx
  shared/api/tauri.ts
  shared/types/app.ts
src-tauri/
  Cargo.toml
  tauri.conf.json
  build.rs
  capabilities/default.json
  src/lib.rs
  src/main.rs
  crates/hmm-core/
  crates/hmm-ports/
  crates/hmm-app/
  crates/hmm-infra/
  crates/hmm-games-mhw/
Cargo.toml
.github/workflows/verify.yml
scripts/verify.ps1
docs/ARCHITECTURE.md
docs/TESTING.md
README.md
CHANGELOG.md
```

重要结构决策：

- 项目根目录 `Cargo.toml` 作为 Rust workspace 根，方便从仓库根目录运行 `cargo test --workspace`。
- `src-tauri/Cargo.toml` 仍然是 Tauri 应用 crate，包名使用 `hmm-tauri`。
- `src-tauri/crates/hmm-*` 放领域、接口、应用、基础设施和 MHW:I 适配器骨架。
- 暂不创建 `src-tauri/crates/hmm-tauri/`，避免和 Tauri 默认 CLI 约定冲突。架构文档需要说明这个实现取舍。

## Task 1: 生成并落地前端与 Tauri 壳

**Files:**
- Create: `package.json`
- Create: `pnpm-workspace.yaml`
- Create: `index.html`
- Create: `vite.config.ts`
- Create: `tsconfig.json`
- Create: `tsconfig.node.json`
- Create: `src/main.tsx`
- Create: `src/App.tsx`
- Create: `src/app/AppShell.tsx`
- Create: `src/app/AppShell.css`
- Create: `src/features/dashboard/DashboardPage.tsx`
- Create: `src/shared/api/tauri.ts`
- Create: `src/shared/types/app.ts`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`
- Create: `src-tauri/capabilities/default.json`

- [ ] **Step 1: 生成官方模板到临时目录用于对照**

Run:

```powershell
$scratch = Join-Path $env:TEMP "helsincy-tauri-react-ts-template"
if (Test-Path -LiteralPath $scratch) {
  Remove-Item -LiteralPath $scratch -Recurse -Force
}
cmd /c corepack pnpm dlx create-tauri-app@4.6.2 "$scratch" --template react-ts --manager pnpm --tauri-version 2 --identifier dev.helsincy.modmanager --yes
```

Expected:

```text
临时目录中出现 Tauri 2 + React + TypeScript 模板。
仓库工作区没有新增文件。
```

- [ ] **Step 2: 在仓库根目录创建前端配置**

Create `package.json`:

```json
{
  "name": "helsincy-mod-manager",
  "version": "0.1.0-alpha.0",
  "private": true,
  "type": "module",
  "packageManager": "pnpm@11.1.3",
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit && vite build",
    "preview": "vite preview",
    "typecheck": "tsc --noEmit",
    "lint": "eslint .",
    "tauri": "tauri",
    "tauri:dev": "tauri dev",
    "tauri:build": "tauri build"
  },
  "dependencies": {
    "@tauri-apps/api": "2.9.0",
    "react": "19.2.0",
    "react-dom": "19.2.0"
  },
  "devDependencies": {
    "@eslint/js": "9.39.1",
    "@tauri-apps/cli": "2.11.2",
    "@types/react": "19.2.7",
    "@types/react-dom": "19.2.3",
    "@vitejs/plugin-react": "5.1.1",
    "eslint": "9.39.1",
    "globals": "16.5.0",
    "typescript": "5.9.3",
    "typescript-eslint": "8.48.0",
    "vite": "7.2.4"
  }
}
```

Create `pnpm-workspace.yaml`:

```yaml
packages:
  - "."
```

Create `index.html`:

```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Helsincy Mod Manager</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

Create `vite.config.ts`:

```ts
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
```

Create `tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "allowJs": false,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "strict": true,
    "forceConsistentCasingInFileNames": true,
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx"
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

Create `tsconfig.node.json`:

```json
{
  "compilerOptions": {
    "composite": true,
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "allowSyntheticDefaultImports": true
  },
  "include": ["vite.config.ts", "eslint.config.js"]
}
```

- [ ] **Step 3: 创建最小前端应用壳**

Create `src/main.tsx`:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./app/AppShell.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

Create `src/App.tsx`:

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

Create `src/app/AppShell.tsx`:

```tsx
import type { ReactNode } from "react";

type AppShellProps = {
  children: ReactNode;
};

const navItems = ["Mods", "Profiles", "Backups", "Games", "Settings"];

export function AppShell({ children }: AppShellProps) {
  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="主导航">
        <div className="brand-block">
          <span className="brand-mark">H</span>
          <div>
            <h1>Helsincy</h1>
            <p>Mod Manager</p>
          </div>
        </div>
        <nav className="nav-list">
          {navItems.map((item) => (
            <button key={item} type="button" className="nav-item">
              {item}
            </button>
          ))}
        </nav>
      </aside>
      <main className="workspace">{children}</main>
    </div>
  );
}
```

Create `src/features/dashboard/DashboardPage.tsx`:

```tsx
const cards = [
  { label: "已导入 Mod", value: "0" },
  { label: "当前 Profile", value: "Default" },
  { label: "待处理任务", value: "0" },
];

export function DashboardPage() {
  return (
    <section className="dashboard-page" aria-labelledby="dashboard-title">
      <header className="page-header">
        <div>
          <p className="eyebrow">Monster Hunter: World - Iceborne</p>
          <h2 id="dashboard-title">管理工作台</h2>
        </div>
        <button type="button" className="primary-action">
          导入 Mod
        </button>
      </header>

      <div className="metric-grid" aria-label="概览">
        {cards.map((card) => (
          <article key={card.label} className="metric-card">
            <span>{card.label}</span>
            <strong>{card.value}</strong>
          </article>
        ))}
      </div>

      <section className="empty-panel" aria-label="Mod 列表">
        <h3>尚未导入 Mod</h3>
        <p>后续任务会接入安全导入、预览图、分类、前置检查和安装计划。</p>
      </section>
    </section>
  );
}
```

Create `src/shared/api/tauri.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";

export async function getAppHealth(): Promise<string> {
  return invoke<string>("app_health");
}
```

Create `src/shared/types/app.ts`:

```ts
export type AppHealth = "ok";
```

- [ ] **Step 4: 创建样式文件**

Create `src/app/AppShell.css`:

```css
:root {
  color: #202124;
  background: #f4f6f8;
  font-family:
    Inter, "Segoe UI", "Microsoft YaHei", system-ui, -apple-system, BlinkMacSystemFont, sans-serif;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  min-width: 320px;
  min-height: 100vh;
}

button {
  font: inherit;
}

.app-shell {
  display: grid;
  grid-template-columns: 248px 1fr;
  min-height: 100vh;
}

.sidebar {
  display: flex;
  flex-direction: column;
  gap: 28px;
  padding: 24px 18px;
  color: #f8fafc;
  background: #18202a;
}

.brand-block {
  display: flex;
  align-items: center;
  gap: 12px;
}

.brand-mark {
  display: grid;
  width: 42px;
  height: 42px;
  place-items: center;
  border-radius: 8px;
  color: #17202a;
  background: #7dd3c7;
  font-weight: 800;
}

.brand-block h1,
.brand-block p,
.page-header h2,
.empty-panel h3 {
  margin: 0;
}

.brand-block h1 {
  font-size: 18px;
}

.brand-block p,
.eyebrow,
.metric-card span,
.empty-panel p {
  color: #64748b;
}

.nav-list {
  display: grid;
  gap: 8px;
}

.nav-item {
  width: 100%;
  padding: 10px 12px;
  border: 0;
  border-radius: 6px;
  color: #dbeafe;
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.nav-item:hover,
.nav-item:focus-visible {
  background: #243244;
  outline: none;
}

.workspace {
  padding: 32px;
}

.dashboard-page {
  display: grid;
  gap: 24px;
  max-width: 1120px;
}

.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.eyebrow {
  margin: 0 0 6px;
  font-size: 13px;
  font-weight: 700;
  text-transform: uppercase;
}

.page-header h2 {
  font-size: 28px;
}

.primary-action {
  min-height: 40px;
  padding: 0 16px;
  border: 0;
  border-radius: 6px;
  color: #082f49;
  background: #7dd3c7;
  font-weight: 700;
  cursor: pointer;
}

.metric-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 14px;
}

.metric-card,
.empty-panel {
  border: 1px solid #dbe3ea;
  border-radius: 8px;
  background: #ffffff;
}

.metric-card {
  display: grid;
  gap: 10px;
  padding: 18px;
}

.metric-card strong {
  font-size: 24px;
}

.empty-panel {
  padding: 28px;
}

.empty-panel p {
  max-width: 620px;
  margin: 8px 0 0;
  line-height: 1.7;
}

@media (max-width: 760px) {
  .app-shell {
    grid-template-columns: 1fr;
  }

  .sidebar {
    position: static;
  }

  .metric-grid {
    grid-template-columns: 1fr;
  }

  .page-header {
    align-items: flex-start;
    flex-direction: column;
  }
}
```

- [ ] **Step 5: 创建 Tauri 应用壳**

Create `src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Helsincy Mod Manager",
  "version": "0.1.0-alpha.0",
  "identifier": "dev.helsincy.modmanager",
  "build": {
    "beforeDevCommand": "cmd /c corepack pnpm dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "cmd /c corepack pnpm build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Helsincy Mod Manager",
        "width": 1200,
        "height": 780,
        "minWidth": 960,
        "minHeight": 640
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": []
  }
}
```

Create `src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build();
}
```

Create `src-tauri/src/main.rs`:

```rust
fn main() {
    hmm_tauri::run();
}
```

Create `src-tauri/src/lib.rs`:

```rust
#[tauri::command]
fn app_health() -> &'static str {
    "ok"
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_health])
        .run(tauri::generate_context!())
        .expect("failed to run Helsincy Mod Manager");
}

#[cfg(test)]
mod tests {
    use super::app_health;

    #[test]
    fn app_health_returns_ok() {
        assert_eq!(app_health(), "ok");
    }
}
```

Create `src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default desktop capability for Helsincy Mod Manager.",
  "windows": ["main"],
  "permissions": ["core:default"]
}
```

## Task 2: 创建 Rust workspace 与 crate 骨架

**Files:**
- Create: `Cargo.toml`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/crates/hmm-core/Cargo.toml`
- Create: `src-tauri/crates/hmm-core/src/lib.rs`
- Create: `src-tauri/crates/hmm-ports/Cargo.toml`
- Create: `src-tauri/crates/hmm-ports/src/lib.rs`
- Create: `src-tauri/crates/hmm-app/Cargo.toml`
- Create: `src-tauri/crates/hmm-app/src/lib.rs`
- Create: `src-tauri/crates/hmm-infra/Cargo.toml`
- Create: `src-tauri/crates/hmm-infra/src/lib.rs`
- Create: `src-tauri/crates/hmm-games-mhw/Cargo.toml`
- Create: `src-tauri/crates/hmm-games-mhw/src/lib.rs`

- [ ] **Step 1: 创建 workspace 根**

Create `Cargo.toml`:

```toml
[workspace]
members = [
    "src-tauri",
    "src-tauri/crates/hmm-core",
    "src-tauri/crates/hmm-ports",
    "src-tauri/crates/hmm-app",
    "src-tauri/crates/hmm-infra",
    "src-tauri/crates/hmm-games-mhw",
]
resolver = "2"

[workspace.package]
edition = "2021"
license = "MIT"
repository = "https://github.com/TheLostRiver/HelsincyModManager"

[workspace.dependencies]
anyhow = "1.0.100"
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.145"
tauri = "2.9.5"
tauri-build = "2.5.3"
thiserror = "2.0.17"
tracing = "0.1.43"
```

- [ ] **Step 2: 创建 Tauri crate manifest**

Create `src-tauri/Cargo.toml`:

```toml
[package]
name = "hmm-tauri"
version = "0.1.0-alpha.0"
description = "Tauri shell for Helsincy Mod Manager"
edition.workspace = true
license.workspace = true
repository.workspace = true

[lib]
name = "hmm_tauri"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build.workspace = true

[dependencies]
hmm-app = { path = "crates/hmm-app" }
hmm-core = { path = "crates/hmm-core" }
serde.workspace = true
serde_json.workspace = true
tauri = { workspace = true, features = [] }
tracing.workspace = true
```

- [ ] **Step 3: 创建领域层 crate**

Create `src-tauri/crates/hmm-core/Cargo.toml`:

```toml
[package]
name = "hmm-core"
version = "0.1.0-alpha.0"
description = "Domain model and rules for Helsincy Mod Manager"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
serde.workspace = true
thiserror.workspace = true
```

Create `src-tauri/crates/hmm-core/src/lib.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameId(String);

impl GameId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::GameId;

    #[test]
    fn game_id_keeps_value() {
        let id = GameId::new("mhw");
        assert_eq!(id.as_str(), "mhw");
    }
}
```

- [ ] **Step 4: 创建 ports crate**

Create `src-tauri/crates/hmm-ports/Cargo.toml`:

```toml
[package]
name = "hmm-ports"
version = "0.1.0-alpha.0"
description = "Application ports for Helsincy Mod Manager"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
anyhow.workspace = true
hmm-core = { path = "../hmm-core" }
```

Create `src-tauri/crates/hmm-ports/src/lib.rs`:

```rust
use anyhow::Result;
use hmm_core::GameId;

pub trait GameAdapter {
    fn game_id(&self) -> GameId;
    fn display_name(&self) -> &'static str;
}

pub trait AppClock {
    fn now_unix_millis(&self) -> Result<u128>;
}
```

- [ ] **Step 5: 创建 app crate**

Create `src-tauri/crates/hmm-app/Cargo.toml`:

```toml
[package]
name = "hmm-app"
version = "0.1.0-alpha.0"
description = "Application use cases for Helsincy Mod Manager"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
hmm-core = { path = "../hmm-core" }
hmm-ports = { path = "../hmm-ports" }
```

Create `src-tauri/crates/hmm-app/src/lib.rs`:

```rust
pub fn app_name() -> &'static str {
    "Helsincy Mod Manager"
}

#[cfg(test)]
mod tests {
    use super::app_name;

    #[test]
    fn app_name_is_stable() {
        assert_eq!(app_name(), "Helsincy Mod Manager");
    }
}
```

- [ ] **Step 6: 创建 infra crate**

Create `src-tauri/crates/hmm-infra/Cargo.toml`:

```toml
[package]
name = "hmm-infra"
version = "0.1.0-alpha.0"
description = "Infrastructure adapters for Helsincy Mod Manager"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
anyhow.workspace = true
hmm-ports = { path = "../hmm-ports" }
```

Create `src-tauri/crates/hmm-infra/src/lib.rs`:

```rust
use anyhow::Result;
use hmm_ports::AppClock;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SystemClock;

impl AppClock for SystemClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
    }
}
```

- [ ] **Step 7: 创建 MHW:I adapter crate**

Create `src-tauri/crates/hmm-games-mhw/Cargo.toml`:

```toml
[package]
name = "hmm-games-mhw"
version = "0.1.0-alpha.0"
description = "Monster Hunter: World - Iceborne adapter for Helsincy Mod Manager"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
hmm-core = { path = "../hmm-core" }
hmm-ports = { path = "../hmm-ports" }
```

Create `src-tauri/crates/hmm-games-mhw/src/lib.rs`:

```rust
use hmm_core::GameId;
use hmm_ports::GameAdapter;

pub struct MonsterHunterWorldAdapter;

impl GameAdapter for MonsterHunterWorldAdapter {
    fn game_id(&self) -> GameId {
        GameId::new("mhw")
    }

    fn display_name(&self) -> &'static str {
        "Monster Hunter: World - Iceborne"
    }
}

#[cfg(test)]
mod tests {
    use super::MonsterHunterWorldAdapter;
    use hmm_ports::GameAdapter;

    #[test]
    fn adapter_reports_game_id() {
        let adapter = MonsterHunterWorldAdapter;
        assert_eq!(adapter.game_id().as_str(), "mhw");
    }
}
```

## Task 3: 接入 lint、类型检查和 CI 依赖安装

**Files:**
- Create: `eslint.config.js`
- Modify: `.github/workflows/verify.yml`
- Modify: `scripts/verify.ps1`

- [ ] **Step 1: 创建 ESLint flat config**

Create `eslint.config.js`:

```js
import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      ecmaVersion: 2022,
      globals: {
        ...globals.browser,
        ...globals.es2022,
      },
    },
  },
  {
    ignores: ["dist", "src-tauri/target", "target"],
  },
);
```

- [ ] **Step 2: 更新统一验证脚本**

Modify `scripts/verify.ps1` so the frontend and Rust blocks become:

```powershell
if (Test-Path -LiteralPath (Join-Path $repoRoot "package.json")) {
    if (-not (Test-Path -LiteralPath (Join-Path $repoRoot "node_modules"))) {
        Write-Host "node_modules is missing. Run: cmd /c corepack pnpm install --frozen-lockfile" -ForegroundColor Red
        exit 1
    }

    Write-Host "Running frontend typecheck..."
    cmd /c corepack pnpm run typecheck
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    Write-Host "Running frontend lint..."
    cmd /c corepack pnpm run lint
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    Write-Host "Running frontend build..."
    cmd /c corepack pnpm run build
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}
else {
    Write-Host "Skipping frontend checks: package.json does not exist yet."
}

if (Test-Path -LiteralPath (Join-Path $repoRoot "Cargo.toml")) {
    Write-Host "Running Rust tests..."
    cargo test --workspace
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    Write-Host "Running Rust check..."
    cargo check --workspace
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}
else {
    Write-Host "Skipping Rust checks: Cargo.toml does not exist yet."
}
```

- [ ] **Step 3: 更新 GitHub Actions**

Modify `.github/workflows/verify.yml`:

```yaml
name: Verify

on:
  pull_request:
  push:
    branches:
      - main

permissions:
  contents: read

jobs:
  policy:
    name: Policy and docs
    runs-on: ubuntu-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 24
          cache: pnpm

      - name: Enable Corepack
        shell: bash
        run: corepack enable

      - name: Install frontend dependencies
        shell: bash
        run: corepack pnpm install --frozen-lockfile

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Run verification
        shell: pwsh
        run: ./scripts/verify.ps1
```

## Task 4: 更新项目文档

**Files:**
- Modify: `README.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/TESTING.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: 更新 README 当前验证入口**

Add commands:

```markdown
脚手架创建后，首次运行需要安装前端依赖：

```powershell
cmd /c corepack pnpm install --frozen-lockfile
```

常用开发命令：

```powershell
cmd /c corepack pnpm tauri dev
cmd /c corepack pnpm run build
cargo test --workspace
```
```

- [ ] **Step 2: 更新架构文档的 Tauri crate 取舍**

In `docs/ARCHITECTURE.md`, adjust Rust workspace section to state:

```markdown
`src-tauri/` 本身作为 Tauri 应用 crate，包名为 `hmm-tauri`。这保留 Tauri CLI 默认约定，避免额外配置成本；可复用业务 crate 放在 `src-tauri/crates/` 下。
```

- [ ] **Step 3: 更新测试指南**

In `docs/TESTING.md`, update frontend and Rust commands:

```powershell
cmd /c corepack pnpm install --frozen-lockfile
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
cargo test --workspace
cargo check --workspace
```

- [ ] **Step 4: 更新 CHANGELOG**

Add under `[Unreleased] / Added`:

```markdown
- 规划并落地 Tauri 2、React、TypeScript 与 Rust workspace 脚手架基线。
```

## Task 5: 安装依赖、生成锁文件并验证

**Files:**
- Create: `pnpm-lock.yaml`
- Create or update: `Cargo.lock`
- Potentially generated: `dist/` and `src-tauri/target/`, both ignored

- [ ] **Step 1: 安装前端依赖**

Run:

```powershell
cmd /c corepack pnpm install
```

Expected:

```text
pnpm-lock.yaml created.
node_modules/ created but ignored by Git.
```

- [ ] **Step 2: 运行前端检查**

Run:

```powershell
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

Expected:

```text
typecheck, lint and Vite build all exit 0.
dist/ is generated but ignored by Git.
```

- [ ] **Step 3: 运行 Rust 检查**

Run:

```powershell
cargo test --workspace
cargo check --workspace
```

Expected:

```text
All Rust tests pass.
All workspace crates compile.
Cargo.lock is created or updated.
```

- [ ] **Step 4: 运行统一验证**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
git diff --check
git status --short --branch
```

Expected:

```text
verify.ps1 exits 0.
git diff --check has no output.
Only intended scaffold files are modified or untracked.
```

## Task 6: 提交与 PR

**Files:**
- All files from previous tasks

- [ ] **Step 1: 检查提交范围**

Run:

```powershell
git status --short --branch
git diff --stat
```

Expected:

```text
No .planning files, node_modules, dist, target, cache, real Mod packages or real save files are staged.
```

- [ ] **Step 2: 提交**

Run:

```powershell
git add package.json pnpm-lock.yaml pnpm-workspace.yaml index.html vite.config.ts tsconfig.json tsconfig.node.json eslint.config.js src src-tauri Cargo.toml Cargo.lock README.md docs/ARCHITECTURE.md docs/TESTING.md CHANGELOG.md .github/workflows/verify.yml scripts/verify.ps1
git commit -m "chore: scaffold tauri workspace baseline"
```

Expected:

```text
pre-commit checks pass.
One commit is created on codex/scaffold-baseline.
```

- [ ] **Step 3: 推送并创建 PR**

Run:

```powershell
git push -u origin codex/scaffold-baseline
gh pr create --title "chore: scaffold tauri workspace baseline" --body "## Summary`n- Add Tauri 2 + React + TypeScript scaffold.`n- Add Rust workspace crates for core, ports, app, infra, and MHW adapter.`n- Wire frontend and Rust checks into verification.`n`n## Test Plan`n- powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`n- cmd /c corepack pnpm run typecheck`n- cmd /c corepack pnpm run lint`n- cmd /c corepack pnpm run build`n- cargo test --workspace`n- cargo check --workspace"
```

Expected:

```text
Remote branch is created.
Draft or ready PR is opened against main.
GitHub Actions runs Policy and docs.
```

## 自检清单

- [ ] 没有真实 Mod 包、真实存档、token、cookie、API key 进入仓库。
- [ ] 前端没有直接承担文件系统规则。
- [ ] Tauri command 只有 `app_health`，没有暴露危险文件操作。
- [ ] Rust crate 依赖方向符合 `core -> ports -> app/infra/adapters -> tauri` 的边界。
- [ ] `scripts/verify.ps1` 在脚手架后实际运行前端和 Rust 检查。
- [ ] 文档同步说明 `src-tauri` 作为 Tauri app crate 的取舍。
- [ ] `cargo-tauri` 不作为必需依赖。
