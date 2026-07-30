---
name: hmm-feature-router
description: Primary HMM repository entry point. Use when starting or scoping Helsincy Mod Manager feature, bug-fix, architecture, frontend, Tauri, Rust, task/concurrency, game-adapter, governance, or verification work to identify touched boundaries, load only relevant project references, and choose risk-proportionate checks. Use hmm-install-safety additionally for untrusted Mod packages, archive extraction, staging or retarget staging, player-data paths, or real filesystem writes, and hmm-review-gate only for review or PR readiness.
---

# HMM 功能路由

在不把全部项目规则加载进上下文的前提下，为 HMM 工作选择正确边界。

## 工作流

1. 阅读 `AGENTS.md`，检查 `git status --short`，并查看当前源码、测试和任务文档。
2. 定义一个用户可见的纵向切片或 release blocker，列出它触及的边界。
3. 只加载下表匹配的 reference；当前源码和正式文档始终更权威。
4. 涉及不可信 Mod 包、压缩包、解压、staging、路径 containment、retarget staging、玩家数据、
   game/save 路径或真实写入时，使用 `hmm-install-safety`。
5. 开发期间运行聚焦检查，按下方风险规则决定何时需要完整验证。
6. Review、准备 PR、处理 review 修复或判断可合并性时，使用 `hmm-review-gate`。

不要仅因为 reference 存在就加载它。不要重建独立 frontend、Rust、Tauri 或 concurrency skill；
这些边界的 checklist 通过本 skill 按需加载。

## 边界路由

| 信号 | 相关时读取 |
| --- | --- |
| 产品范围或功能专属文档 | `references/feature-doc-index.md` |
| React UI、CSS、state、accessibility、responsive behavior | `references/frontend-workflow-checklist.md` |
| 前后端职责或 typed API 形状 | `references/frontend-backend-boundary.md` 和 `docs/FRONTEND_BACKEND_CONTRACT.md` |
| Tauri command、DTO、event、AppState wiring | `references/tauri-command-checklist.md` |
| Rust crate 放置或依赖方向 | `references/architecture-map.md` 和 `references/rust-crate-boundary-checklist.md` |
| TaskManager、cancellation、progress、locks、queues | `references/task-concurrency-checklist.md`；涉及 event 或证据时再读 `docs/LOGGING.md` |
| MHW:I adapter、catalog、路径语法、retarget | `references/architecture-map.md`、匹配的功能文档和当前 adapter 源码 |
| 不可信压缩包、解压、staging、retarget staging、安装、卸载、回滚、存档备份/恢复 | `hmm-install-safety` 和 `references/safety-boundary.md` |
| 治理或多 agent 工作 | `docs/GOVERNANCE.md`；仅在 agent 协作时再读 `references/multi-agent-workflow.md` |
| 选择聚焦检查 | `references/testing-map.md` 和 `docs/TESTING.md` 的相关章节 |

## 不可破坏的边界

- 不要把文件系统、安装、备份、回滚或游戏 adapter 规则放进 React。
- Tauri command 保持窄边界，只校验 DTO、映射错误并调用应用服务。
- `hmm-core` 不依赖 Tauri、具体 infra、数据库或 MHW:I 路径解析。
- 游戏专属 catalog 和路径语法留在 game adapter。
- 长时间 scan/hash/extract/analyze 工作放在 game write lock 外；同 game/profile 写入串行。
- 保留 `InstallPlan`、backup、manifest、rollback/recovery、staging containment 和 task/audit identity。
- 自动测试默认不得使用真实游戏目录、真实存档或第三方 Mod 包。

## 风险分级验证

| 风险 | 开发期间 | 首次 PR ready 前 |
| --- | --- | --- |
| Low：文档、局部重构、隔离 UI | 运行最小相关 policy、link、unit、type 或 browser 检查。 | 聚焦检查通常足够；required CI 仍为必需。 |
| Medium：跨层行为、public DTO、task/event contract | 为每个触及边界运行聚焦测试。 | 对 PR candidate 运行一次完整 `scripts/verify.ps1`。 |
| High：安装/存档写入、回滚、并发、安全、治理/CI | 使用 temp/fake fixture 运行正负聚焦测试。 | 运行一次完整 `scripts/verify.ps1` 和 findings-first 自审。 |

Review 小修只重跑受影响的聚焦检查。仅当修改扩大高风险边界、改变 public contract 或治理规则、
改变依赖/基线，或使旧完整结果失效时，才重复本地完整验证。最终 commit 的 required CI 仍必须成功。
