---
name: hmm-install-safety
description: Use when Helsincy Mod Manager work touches Mod import, archive extraction, staging, path validation, install plans, game directory writes, overwrite/delete, manifest, backup, uninstall, rollback, save backup, audit logging, or data-safety tests.
---

# HMM Install Safety

## 概览

玩家数据安全优先。任何会写入、覆盖、删除、备份、恢复或推断 game/save 路径的流程，都必须保留 `InstallPlan -> backup -> commit -> manifest -> rollback/recover` 链路。

HMM 专属 skill 和安全规则必须放在本仓库 `.codex/skills/` 下，绝不放进全局 skill 目录。

## 必读上下文

编辑前，读或扫描：

- `AGENTS.md`
- `README.md`
- `SECURITY.md`
- `docs/ARCHITECTURE.md`
- `docs/ROADMAP.md`
- `CONTRIBUTING.md`
- `docs/TESTING.md`
- `docs/GOVERNANCE.md`
- `docs/LOGGING.md`
- `.codex/skills/hmm-project-guardrails/references/safety-boundary.md`

如果任务包含 retarget 或 MHW 路径，动代码前还要读取当前 retarget 设计文档。

## 边界路由

- 如果安全工作修改或暴露 Tauri commands、DTOs、AppState wiring、task events、custom protocols、command errors 或 frontend/backend contract shape，同时使用 `hmm-tauri-command`。
- 如果修改 Rust crate placement、app/ports/infra dependency direction、repositories、game adapters、domain/app mapping 或 DTO/domain conversion，同时使用 `hmm-rust-crate-boundary`。
- 如果修改 TaskManager、long-running tasks、cancellation、progress phases、game/profile locks、queues、并发 scan/hash/extract/analyze 或 database/write serialization，同时使用 `hmm-task-and-concurrency`。
- 如果修改 React UI、frontend state、task listeners、typed API wrappers、thumbnail/resource display、accessibility、responsive behavior 或浏览器可见 workflow，同时使用 `hmm-frontend-workflow`。

## 安全流程

1. 判断改动影响 import-only、staging-only、plan-building、真实 game writes、uninstall、save backup、logging 还是 concurrency。
2. 原始导入 Mod 包保持只读。派生 variant 放在 sandbox/cache/staging，且必须可丢弃。
3. 解压前拒绝不安全 archive entry：父级穿越、绝对路径、link/junction 陷阱、可疑文件类型、archive bomb、大小写不敏感碰撞。
4. 任何真实游戏目录写入前，必须生成或消费 `InstallPlan`。
5. 覆盖前先备份已有文件；commit 后写 manifest；uninstall 基于 manifest，不基于 package 猜测。
6. 失败时尽可能 rollback，并留下可恢复状态和 Audit Log 事件。
7. save backup 或 restore 要保证默认备份目录在游戏安装目录外，写 backup manifest，restore 前验证并要求确认，同时保留可配置 interval/retention 行为。
8. 测试使用 temp fixtures、fake file systems 或人工最小 package。不要要求真实 MHW 安装、真实 save 目录或第三方 Mod 包。

## 硬性停止条件

- 不要在 install executor 路径之外复制、删除、重命名或覆盖游戏目录文件。
- 在 logging/telemetry、`task_id` 传播、redaction helpers、log directory resolution、Audit Log writer 和相关测试存在前，不要实现真实游戏目录写入。
- 没有预先验证、明确确认和 backup manifest 时，不要 restore saves。
- 不要让 frontend 或 Tauri command 代码计算最终 install paths 或 replacement targets。
- extract、hash、scan、analyze 或构建长时间 plan 时，不要持有 game write lock。
- 不要记录完整本地路径、用户名、Steam IDs、tokens、cookies、真实 save 内容或第三方 Mod 内容。
- 不要把 staging 当成事实来源。事实来源是导入包 metadata、bindings/configuration 和 manifest。

## 最小测试

使用 `references/install-safety-checklist.md` 作为聚焦 checklist。至少覆盖被改动的风险：

| 风险 | 必要测试形态 |
| --- | --- |
| Archive path safety | 用人工 archive 或 entry fixture 覆盖父级穿越、绝对路径、大小写碰撞和 sandbox containment。 |
| Staging | 覆盖相对 target normalization 和 escape rejection；断言输出留在 staging root 下。 |
| Install plan | 按最终 target path 做 conflict detection；plan 存在前不得直接写入。 |
| Overwrite/delete | 覆盖前备份；manifest 记录变更；rollback 恢复 temp game directory。 |
| Uninstall | manifest 驱动删除；未知文件保留。 |
| Save backup/restore | 默认备份目录在游戏安装目录外、自定义备份目录、backup manifest、restore validation 和 confirmation、retention limits、不可写备份目录。 |
| Concurrency | 同 game/profile 写入串行；长分析工作在 write lock 外执行；progress 携带 task id。 |
| Logging | write/overwrite/delete/backup/restore/manifest/rollback 写 Audit Log；敏感路径脱敏。 |

运行 `docs/TESTING.md` 中最小有意义命令；最终交付前可行时优先完整运行 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`。

## 常见错误

- 因为 install MVP 未完成，就临时做方便的 direct-copy 路径。
- 为了快，用本地真实游戏目录测试。
- 安装后仍把当前 package contents 当成 uninstall 事实来源。
- 让 `nativePC` 或 MHW slot parsing 泄漏到通用 core 或 frontend 代码。
- 没有聚焦测试，也没有说明为何无法测试，就报告“safe”。
