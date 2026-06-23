---
name: hmm-frontend-workflow
description: Use when Helsincy Mod Manager work touches React UI, feature pages, components, CSS, appearance system, frontend state, typed API wrappers, accessibility, responsive behavior, or browser smoke tests.
---

# HMM Frontend Workflow

## 概览

前端只负责展示、交互、view models 和 typed command calls。文件系统安全、游戏规则、install logic、retargeting、backup 和 rollback 留在 Tauri/app/Rust 边界之后。

HMM 专属 skills 属于本仓库 `.codex/skills/`，不属于全局 skill 目录。

## 必读上下文

编辑前，读或扫描：

- `AGENTS.md`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/ROADMAP.md`
- `CONTRIBUTING.md`
- `docs/TESTING.md`
- `docs/GOVERNANCE.md`
- `SECURITY.md`
- `.codex/skills/hmm-project-guardrails/references/frontend-backend-boundary.md`

如果涉及 appearance、shell、Dashboard 或 sidebar，还要读：

- `docs/APPEARANCE_SYSTEM.md`
- `docs/APPEARANCE_EXTENSION_GUIDE.md`
- 触及 Dashboard/sidebar behavior 时读 `docs/DASHBOARD_V2_SIDEBAR_MODES.md`。

如果涉及 typed API wrappers、command results、command errors、task events、thumbnail URLs、custom protocols 或 DTO shape 变化，还要读 `docs/FRONTEND_BACKEND_CONTRACT.md` 并使用 `hmm-tauri-command`。

## 前端边界

1. Feature UI 放在 `src/features/<feature>/`；共享可复用 UI 放在 `src/shared/`；app shell 和导航放在 `src/app/`。
2. 业务页面默认保持 game-agnostic。使用后端提供的 capabilities、ids、labels 和 catalogs；不要在通用 UI 中根据 game paths 或 MHW resource names 分支。
3. 使用 feature-local typed API wrappers。`src/shared/api/tauri.ts` 只做通用 invoke helper/re-export，不做每个 feature call 的 dumping ground。
4. DTO 只映射为展示用 view models。不要在 TypeScript 中重建 install paths、retarget paths、backup paths、conflict rules 或 adapter decisions。
5. 使用 `src/shared/styles/tokens.css` 的 semantic tokens 和本地 component CSS namespace。避免大范围硬编码 color/spacing system。
6. Navigation definitions 保持单一来源。Shell variants 决定 layout；feature pages 不要按 shell/sidebar mode 复制整页。
7. 构建可访问状态：keyboard focus、icon-only controls 的 labels/tooltips、可见 disabled states，以及在桌面和移动宽度下都能容纳的文本。

## 硬性停止条件

- 不要让前端代码读取任意本地路径、调用宽泛 filesystem APIs，或使用 `convertFileSrc`/asset protocol/raw cache paths，除非 contract 明确允许。
- 不要把 Mod install、archive extraction、staging、path validation、backup、rollback、manifest、dependency 或 game adapter rules 放进 UI components。
- 不要让 Dashboard 或业务页面依赖 `sidebarMode` / `useSidebarMode`，也不要通过 `[data-sidebar-mode]` 分支 CSS。
- 不要为了 shell variants、themes、games 或 density modes 复制整页。
- 没有明确理由和用户可见 scope 时，不要引入新的 frontend dependencies 或修改 lockfiles。

## 验证

Review 时使用 `references/frontend-workflow-checklist.md`。最小检查：

| 改动 | 最小检查 |
| --- | --- |
| Component/CSS/page | `cmd /c corepack pnpm run typecheck`、`cmd /c corepack pnpm run lint`、`cmd /c corepack pnpm run build`。 |
| UI workflow/state behavior | 前端检查，加上 `cmd /c corepack pnpm run test`（当 behavior、state、routing 或 helper tests 相关时）；省略时记录原因。 |
| App shell/sidebar/Dashboard | 前端检查，加 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1`。 |
| Typed API wrapper | 读取 `docs/FRONTEND_BACKEND_CONTRACT.md`；运行 frontend typecheck，并聚焦检查 command name、DTO shape、稳定 error/kind/status/phase 值、contract docs 和禁止 path APIs。 |
| Visual workflow | 在相关状态和宽度做 Browser 或 manual smoke；shell/sidebar/Dashboard visual baseline 包含 `1440x900`、`1366x768`、`1280x800`；记录已检查和未检查内容。 |
| 跨入 Tauri/Rust | 同时使用 `hmm-tauri-command`，并运行 `docs/TESTING.md` 中 bridge checks。 |

当前端改动跨边界或风险不清时，最终交付前优先完整运行 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`。

## 常见错误

- 通过复制 layout 来“美化”页面，而不是扩展 shell/appearance primitives。
- 让 TypeScript 从 display label 或 `metadata` 推断 install behavior。
- 把 feature-specific API calls 加进 shared barrel。
- 没检查相关 viewport/state 就声称视觉正确。
- 通过削弱 governance、lint、boundary 或 contract checks 来修前端症状。
