---
name: hmm-feature-router
description: Use when starting or scoping Helsincy Mod Manager work, especially feature requests, bug fixes, architecture decisions, frontend/backend boundary changes, Rust crate edits, Tauri commands, task/concurrency work, safety-sensitive flows, review gates, or choosing verification.
---

# HMM Feature Router

## 概览

编辑前先路由 HMM 工作。先识别模块边界，只加载相关项目文档，再选择最窄且安全的实现与验证路径。

项目专属 skill 必须放在本仓库 `.codex/skills/` 下。不要在全局 skill 目录中创建或更新 HMM skill。

## 开始步骤

1. 任何编辑前，先读或扫描基础文件：`AGENTS.md`、`README.md`、`docs/ARCHITECTURE.md`、`docs/ROADMAP.md`、`CONTRIBUTING.md`、`docs/TESTING.md`、`docs/GOVERNANCE.md`、`SECURITY.md`。
2. 检查 `git status --short`；不要回退用户或其他 agent 的改动。
3. 使用下方路由表给任务分类。
4. 打开对应类别要求的文档或 skill。reference 只负责导航，源文档和当前代码始终更权威。
5. 非平凡编辑前，先说明模块边界和验证计划。

## 路由表

| 任务信号 | 主要边界 | 下一步读取 | 硬性停止条件 |
| --- | --- | --- | --- |
| UI、页面、组件、CSS、route、本地 UI 状态、typed API wrapper | `src/` | 如果存在则读 `.codex/skills/hmm-frontend-workflow`，再读 `docs/TESTING.md`、`.codex/skills/hmm-project-guardrails/references/frontend-backend-boundary.md` | 不要把文件系统安全、安装规则、MHW 路径、retarget、backup 或 rollback 逻辑放进前端。 |
| Tauri command、DTO、app state、event bridge | `src-tauri/src/` | `docs/FRONTEND_BACKEND_CONTRACT.md`，如果存在则读 `.codex/skills/hmm-tauri-command` | 不要暴露宽泛文件系统 command；command 必须是窄用例入口。 |
| Domain model、install plan、manifest、conflict、replacement target | `src-tauri/crates/hmm-core/` | 如果存在则读 `.codex/skills/hmm-rust-crate-boundary`，再读 `docs/ARCHITECTURE.md`、`.codex/skills/hmm-project-guardrails/references/architecture-map.md` | 不要依赖 Tauri、真实文件系统、数据库或 MHW 专属路径解析。 |
| 应用用例、编排、task manager | `src-tauri/crates/hmm-app/` | 如果存在则读 `.codex/skills/hmm-rust-crate-boundary`；触及 task/lock/event 时读 `.codex/skills/hmm-task-and-concurrency`；再读 `docs/ARCHITECTURE.md`、`docs/TESTING.md` | 不要直接绑定具体 infra；使用 ports/traits。 |
| 真实 file I/O、config、Steam discovery、hash、staging、archive handling | `src-tauri/crates/hmm-infra/` | 如果存在则读 `.codex/skills/hmm-rust-crate-boundary`，再读 `SECURITY.md`、`docs/TESTING.md`、`docs/LOGGING.md`、`.codex/skills/hmm-project-guardrails/references/safety-boundary.md` | 测试默认不得操作真实游戏/存档路径。 |
| MHW:I adapter、`nativePC`、`plNNN_VVVV`、游戏专属 catalog/rules | `src-tauri/crates/hmm-games-mhw/` | 如果存在则读 `.codex/skills/hmm-rust-crate-boundary`，再读 `docs/ARCHITECTURE.md`、`docs/TESTING.md` 的 game adapter 小节，以及相关 game adapter 文档 | 不要让 MHW 规则泄漏到通用 core 或通用前端；自动测试不得要求真实游戏安装。 |
| install、uninstall、backup、rollback、overwrite、delete、path validation | 跨 crate 高风险流 | 如果存在则读 `.codex/skills/hmm-install-safety`，再读 `SECURITY.md`、`docs/TESTING.md`、`docs/LOGGING.md` | 不要绕过 `InstallPlan`、manifest、backup、rollback、staging 或路径 containment。 |
| long task、progress event、cancellation、lock、concurrency | `hmm-app`、`src-tauri/src/task_events.rs` | 如果存在则读 `.codex/skills/hmm-task-and-concurrency`，再读 `docs/FRONTEND_BACKEND_CONTRACT.md`、`docs/TESTING.md` 并发小节；触及 events/logs/audit 时读 `docs/LOGGING.md` | 不要在长时间 hash/extract/analyze 期间持有 game write lock；progress event 必须带 task id。 |
| review、final handoff、PR readiness、artifact audit | 跨边界 review | 如果存在则读 `.codex/skills/hmm-review-gate`，再读 `docs/TESTING.md`、`docs/GOVERNANCE.md` | 没有当前验证证据或明确遗漏说明时，不要声称完成。 |
| Governance、`.codex/`、`.agents/`、policy、scripts、hooks、CI | 治理文件 | 如果存在则读 `.codex/skills/hmm-review-gate`，再读 `docs/GOVERNANCE.md`、`docs/MULTI_AGENT_COLLABORATION.md` | 视为需要人工 review 的工作；不要写入 secret、session log、私有本地路径、玩家数据或真实 Mod 内容。 |

## 验证映射

优先从 `docs/TESTING.md` 选择最小有意义检查；跨边界或风险不清时运行完整验证。

| 改动类型 | 最小有用检查 |
| --- | --- |
| 仅 docs/governance/skills | 可行时运行 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`；否则运行相关 policy/link/secret 检查并说明遗漏。 |
| 仅 frontend | `hmm-frontend-workflow` 检查：通过项目脚本运行 frontend typecheck/lint/build；相关时补 boundary/browser smoke。 |
| Tauri bridge | Rust tests/checks 加 frontend typed wrapper 检查；可行时 smoke command shape。 |
| Rust core/app/infra/game adapter | `hmm-rust-crate-boundary` 检查：`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`，以及触及 crate 的聚焦测试。 |
| 高风险 file/install/save/concurrency | 使用 temp fixtures 的聚焦 safety/task 测试，不使用真实 game/save 目录；相关时补 Audit Log/redaction 检查；可行时完整 verify。 |
| Review/final handoff | `hmm-review-gate` 检查：review 时 findings first、当前验证证据、artifact hygiene 和明确遗漏说明。 |

## 常见错误

- 把 `.codex/skills/` 当成运行时缓存。它是可版本管理的项目治理内容。
- 为 HMM 专属规则更新全局 skills。HMM 规则必须留在本仓库。
- 在源文档或当前代码相关时，只读 reference 摘要。
- 让前端代码决定文件系统、install、backup、rollback 或 game-adapter 规则。
- 没有在当前回合成功运行测试，却声称测试已通过。
