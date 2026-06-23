---
name: hmm-task-and-concurrency
description: Use when Helsincy Mod Manager work touches TaskManager, long-running tasks, task events, cancellation, progress phases, game/profile locks, queues, concurrent scanning/hash/extract/analyze work, or database/write serialization.
---

# HMM Task And Concurrency

## 概览

重任务必须显式、可取消、可追踪。准备工作可以并行，但同一 game instance/profile 的写入必须串行，progress 必须始终携带 task identity。

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
- `docs/FRONTEND_BACKEND_CONTRACT.md`
- `docs/LOGGING.md`
- `.codex/skills/hmm-project-guardrails/references/safety-boundary.md`

如果任务暴露或修改 command/event DTOs，同时使用 `hmm-tauri-command`。如果修改 Rust crate placement、app/ports/infra dependency direction、AppState services、repositories 或 DTO/domain mapping，同时使用 `hmm-rust-crate-boundary`。如果修改 React task UI、frontend listeners、typed API wrappers、task state 或浏览器可见 workflow，同时使用 `hmm-frontend-workflow`。如果会写入、删除、备份、恢复、install、uninstall 或 rollback 文件，同时使用 `hmm-install-safety`。

## Task 模式

1. 通过返回 `TaskStartedDto` 或等价 identity 的窄 command 启动长任务。
2. 通过文档化的 `hmm://task-progress` contract 发 progress，包含 `taskId`、kind、status、phase、progress、error 和可选 result reference。
3. Phase codes 保持稳定，并注册在 `docs/FRONTEND_BACKEND_CONTRACT.md`。
4. 大型最终结果通过 `resultRef` 或 query command 获取，不塞进 events。
5. 在明确安全点支持 cancellation，并保持 cancellation state 一致。
6. 使用带同一 task id 的 structured logs；不要把原始敏感路径放进 messages。

## 并发规则

| 工作 | 允许形态 |
| --- | --- |
| Scan、hash、archive inspect、sandbox extract、package analyze、dependency check、plan preview | 资源允许时可并行；可取消；不写游戏目录。 |
| 同一 game instance write | 通过 game write queue/lock 串行。 |
| 同一 profile enable/disable/install/uninstall | 通过 profile 或 game/profile coordination 串行。 |
| Database write transaction | 短、明确，不跨长 I/O 持有。 |
| Commit/install/backup/restore | 写入前 revalidate，lock 保持短，写 audit data，保留 recovery path。 |

extract、hash、scan、analyze 或构建长时间 plan 时，不要持有 game write locks。

## 硬性停止条件

- 不要用“只有一个任务活跃”来匹配 events；task identity 必须显式。
- `docs/LOGGING.md` 要求的 logging/redaction/task/audit 基础存在前，不要启动真实 game writes。
- cancellation/failure 后，不要留下没有 recovery/audit handling 的 partial manifest 或 half-committed state。
- 不要使用没有清晰 locking 和 tests 的 global mutable task state。
- 不要让 frontend 只从 page state 推断 task ownership。

## 验证

Review 时使用 `references/task-concurrency-checklist.md`。最小检查：

| 改动 | 最小检查 |
| --- | --- |
| Rust task manager/app/infra logic | `cargo test --workspace`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`，加聚焦 task/concurrency tests。 |
| Task identity/event DTO | `cargo test --workspace`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`，serialization/phase tests；bridge 变化时补 `hmm-tauri-command` 检查。 |
| Cancellation | 覆盖 queued/running/unknown/already-finished task behavior，以及 cancel 后 stable state。 |
| Locks/queues | 测试证明同 game/profile writes 串行，长 prepare work 在 write lock 外执行。 |
| Install/save/write task | 使用 temp/fake fixtures 满足 `hmm-install-safety` 检查；相关时补 Audit Log/redaction tests。 |
| Frontend listener/task UI | 满足 `hmm-frontend-workflow` 检查、frontend typecheck，并确认 listener 按 `taskId` 匹配，而不是 page-local assumptions。 |

Task/concurrency 改动最终交付前，优先完整运行 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`。

## 常见错误

- 新增 progress event 却没有稳定 phase codes。
- cancellation 只翻 UI state，不改变 backend state。
- 做 archive 或 hash 工作时持有 write lock。
- 把 task logs 当成 manifest 或 rollback state。
- 忘记 failed install tasks 不能留下 half-written manifests。
