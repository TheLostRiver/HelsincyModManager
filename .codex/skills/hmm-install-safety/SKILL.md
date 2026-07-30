---
name: hmm-install-safety
description: Use for HMM work that can affect player data or real filesystem state, including Mod import, archive extraction, staging, path validation, install plans, game directory writes, overwrite/delete, manifest, backup, uninstall, rollback/recovery, save backup/restore, Steam save-account selection, audit logging, and their safety tests. Do not trigger for ordinary UI, read-only queries, or unrelated repository work.
---

# HMM 安装安全

保护玩家数据，同时只加载当前风险需要的规则。

## 加载上下文

1. 阅读 `AGENTS.md`、当前实现和测试、`SECURITY.md`，以及 `docs/TESTING.md` 的相关章节。
2. 流程会写入、删除、备份、恢复或改变 evidence health 时，读取 `docs/LOGGING.md`。
3. 读取 `references/install-safety-checklist.md`。
4. 需要完整项目安全边界时，读取 `../hmm-feature-router/references/safety-boundary.md`。
5. 仅在触及对应边界时加载 router 的 Tauri、Rust、task/concurrency 或 frontend checklist。

## 保留安全事实链

```text
sealed input
  -> analyze / preflight
  -> InstallPlan
  -> 持久化 Planned recovery intent
  -> 读取 source/target 并建立 backup
  -> 持久化 Committing rollback facts
  -> commit 玩家文件
  -> 原子保存最终 manifest
success -> 标记 Completed 并清理 recovery
failure -> rollback；rollback 失败则保留 RollbackRequired
```

- 原始导入包保持只读；派生 variant 只写入可丢弃 staging。
- 拒绝父级穿越、绝对路径、link/junction 逃逸、大小写不敏感碰撞、可疑类型、archive bomb，以及任何
  超出批准 root 的 target。
- 覆盖前备份已有 target。卸载以 manifest/recovery 事实为准，不根据 package 猜测。
- Commit 保持短小并按 game/profile 串行；scan/hash/extract/analyze 放在 write lock 外。
- Task/audit 证据失败必须成为显式 degraded result，不能伪造玩家文件 rollback。
- 存档备份放在游戏安装目录外，写入 backup manifest；restore 前验证 source、target、用户选择的
  Steam account/profile、游戏状态和确认。
- 日志与输出必须脱敏本地路径、用户名、Steam ID、token、存档内容和第三方 Mod 内容。

## 硬性停止条件

- 不要在 install executor 外新增直接复制、删除或覆盖路径。
- 不要暴露宽泛文件系统 Tauri 或 CLI command。
- Logging/redaction、task identity、manifest、backup 和 recovery 完成前，不要实现真实游戏写入。
- 没有 containment 验证、明确确认和 backup manifest 时，不要恢复存档。
- 自动测试不得使用真实游戏/存档目录或第三方 Mod 包。
- 不要把 MHW:I 路径语法或 retarget 规则放进 generic core 或 frontend。

## 聚焦测试

使用 temp/fake fixture 覆盖每个变更风险：

- archive traversal、绝对路径、link/junction、碰撞、大小和 containment 拒绝；
- 最终 target normalization、冲突、stale plan/binding 拒绝和 preflight 决策；
- 覆盖前备份、manifest 内容、rollback/restart recovery 和未知文件保留；
- 卸载和真正重装的 retained/replaced/added/stale 行为；
- 存档账号选择、backup manifest、不可写目标、retention、restore 验证和确认；
- cancellation safe point、同 game/profile 串行和 task id 传播；
- Audit Log 覆盖、脱敏和显式 evidence-health degradation。

开发期间运行聚焦测试。由于这些路径是高风险，PR candidate 运行一次完整 `scripts/verify.ps1`，发布前
使用 `hmm-review-gate`。Review 小修仅在改变安全边界或使旧结果失效时重复完整验证；最终 commit 仍由
required CI 验证。
