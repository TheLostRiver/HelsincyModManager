---
name: hmm-rust-crate-boundary
description: Use when Helsincy Mod Manager work touches Rust crates, module placement, dependency direction, domain/app/ports/infra boundaries, game adapters, Rust DTO/domain mapping, or workspace verification.
---

# HMM Rust Crate Boundary

## 概览

按依赖方向拆清 Rust 职责。Domain rules 保持纯净，app services 依赖 ports，infra 实现 ports，游戏专属规则留在 game adapters，Tauri 保持薄壳。

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
- `.codex/skills/hmm-project-guardrails/references/architecture-map.md`

如果改动触及 Tauri commands、DTOs、command errors、task events、custom protocols 或 frontend/backend contract shape，同时读 `docs/FRONTEND_BACKEND_CONTRACT.md` 并使用 `hmm-tauri-command`。

如果触及 `TaskManager`、long-running tasks、task events、cancellation、progress phases、game/profile locks、queues、并发 scan/hash/extract/analyze work 或 database/write serialization，同时使用 `hmm-task-and-concurrency`。

如果触及真实 file operations、install、backup、rollback、staging、saves、audit logging 或 data safety，同时使用 `hmm-install-safety`。

## Crate 放置

| 边界 | 放这里 | 不要放这里 |
| --- | --- | --- |
| `hmm-core` | 纯 domain types/rules：Game、Profile、InstallPlan、Manifest、Conflict、ReplacementTarget | Tauri、SQLite、真实 FS、Steam、MHW path parsing |
| `hmm-ports` | app services 使用的 traits/interfaces | 具体 infra、UI DTOs、game-specific parsing |
| `hmm-app` | 通过 ports 编排 use-case 和 workflows | 具体 file/database/platform APIs |
| `hmm-infra` | 真实 filesystem、config/SQLite、archive/hash、Steam/platform integration | 应属于 core/app 的 domain decisions |
| `hmm-games-mhw` | MHW:I directory rules、`nativePC`、loader/DLL、catalog、resource parsing | Rise/Wilds branches 或通用 install flow |
| `src-tauri/src` | Tauri command registration、state wiring、DTOs、event/protocol boundary | Business rules、adapter parsing、direct install execution |

只有真实 adapter 工作落地时才创建未来游戏 crates；避免空抽象。

## 边界规则

1. 只有 game-independent 且稳定的 shared concepts 才向内移动。
2. Game-specific parsing/catalog/rules 留在对应 `hmm-games-*` crate。
3. App services 依赖 `hmm-ports` 的 traits，不依赖具体 infra structs。
4. 在 Tauri/app 边界映射 DTOs 和 domain types；不要让 DTOs 取代 domain models。
5. 测试使用 fake ports、temp dirs 和人工 fixtures；不要要求真实游戏安装。
6. 跨层保留可恢复错误信息，同时不泄漏敏感路径。

## 硬性停止条件

- 不要把 MHW slot/resource/path parsing 加到 `hmm-core`、`hmm-app` 或通用 frontend code。
- 不要让 `hmm-app` 依赖 `hmm-infra` concrete types。
- 不要在 Tauri commands 或 core domain logic 中实现 file copies/deletes。
- 不要把 Rise/Wilds 逻辑加到 `hmm-games-mhw`。
- 不要为了快而绕过 `InstallPlan`、manifest、backup、rollback 或 adapter registration。

## 验证

Review 时使用 `references/rust-crate-boundary-checklist.md`。最小检查：

| 改动 | 最小检查 |
| --- | --- |
| Rust core/app/ports/infra/game adapter | `cargo test --workspace` 和 `cargo clippy --workspace --all-targets -- -D warnings`。 |
| Tauri shell/DTO bridge | 读取 `docs/FRONTEND_BACKEND_CONTRACT.md`；运行 `cargo test --workspace`、`cargo check --workspace`，并满足 `hmm-tauri-command` 检查。 |
| Task/concurrency boundary | 满足 `hmm-task-and-concurrency` 检查，并按触及范围聚焦测试 task identity、phase codes、cancellation、locks/queues 或 database write serialization。 |
| Game adapter | 使用 fake/temp paths 的聚焦 adapter tests；不要求真实 game install。 |
| File/install/save/audit | 满足 `hmm-install-safety` 检查，并使用 temp/fake fixtures。 |
| Dependency boundary refactor | 检查 crate imports；如果 architecture boundary 改变则更新文档。 |

Crate 工作跨 frontend/Tauri/Rust 或治理边界时，最终交付前优先完整运行 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`。

## 常见错误

- 在 core 中加入“很小的 helper”，但偷偷编码了 MHW path rules。
- 因为测试方便，把 concrete infra 传进 app services。
- 把 DTO enum 当成 domain model。
- 在没有真实代码前创建未来游戏 crates。
- 改 crate boundaries 却不更新 architecture docs 或 tests。
