# 按功能选择源文档

用本文件决定开工前应读哪些文档。打开长文档前，优先用 `rg -n` 或标题扫描定位章节。

## 基础入口

- 始终先读 `AGENTS.md`，再检查当前任务、源码、测试和 `git status`。
- 按 `AGENTS.md` 要求检查基础文档，但不要因为打开本索引就重复加载所有长文档。
- 架构、贡献、测试、治理和安全文档优先用标题或关键词定位当前 boundary 的章节。

## 前端 Shell / 外观系统

- `docs/APPEARANCE_SYSTEM.md`
- `docs/APPEARANCE_EXTENSION_GUIDE.md`
- `docs/DASHBOARD_V2_SIDEBAR_MODES.md`
- `docs/superpowers/specs/2026-06-01-appearance-color-scheme-design.md`
- `docs/superpowers/plans/2026-06-01-appearance-color-scheme-implementation.md`

## 游戏目录 / 发现

- `docs/superpowers/specs/2026-06-04-game-directory-settings-design.md`
- `docs/superpowers/plans/2026-06-04-game-directory-settings-implementation.md`
- `docs/superpowers/specs/2026-06-06-steam-library-discovery-design.md`
- `docs/superpowers/plans/2026-06-06-steam-library-discovery-implementation.md`

## Mod 安装

- `docs/mod_installation_strategy.md`
- `docs/superpowers/plans/2026-06-19-mod-installation-mvp-implementation.md`
- `docs/ARCHITECTURE.md` 中关于导入、InstallPlan、执行器、manifest、rollback、并发的章节。

## Armor Retarget / Replacement Targets

- `docs/ARMOR_RETARGET_DESIGN.md`
- `docs/ARMOR_RETARGET_IMPLEMENTATION.md`
- `docs/ARMOR_RETARGET_REVIEW.md`
- 接 staging、InstallPlan、manifest 或冲突 UI 前，同时读取安装策略。

## 第三方 Mod 管理器批量迁移 / 导入记录

- `docs/EXTERNAL_MOD_MANAGER_BATCH_IMPORT_DESIGN.md`(权威边界:命名治理、脱敏口径、
  audit 不扩张、来源契约与 Slice 5 导入记录/保留期)
- `docs/FRONTEND_BACKEND_CONTRACT.md` 的 T17 章节(command 表、错误码全集、cursor 约定)
- `docs/EXTERNAL_MOD_IMPORT_REAL_SOURCE_SMOKE.md`(真实来源人工 smoke 清单与脱敏记录纪律;
  **尚未执行**——真实来源兼容性目前只有合成 fixture 证据)
- 涉及 SQLite 批次表或保留期时同时读 `docs/PERSISTENCE_DECISION.md`
- 涉及不可信来源目录、解压或 staging 时改用 `hmm-install-safety`

## 日志 / 诊断 / 审计

- `docs/LOGGING.md`
- `SECURITY.md`
- `docs/TESTING.md` 日志相关章节。

## 发布 / 打包

- `docs/release/发布与产物说明.md`
- `docs/release/构建发布与脚本说明.md`
- `docs/TESTING.md` 发布相关章节。

## 多 Agent 协作

- `docs/MULTI_AGENT_COLLABORATION.md`
- `.agents/rules/multiagent.md`

## 路线图上下文

- `docs/ROADMAP.md`

只在需要判断优先级、前置或是否误做未来范围时读取路线图。
