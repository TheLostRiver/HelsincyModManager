# 功能文档索引

用本文件决定开工前应读哪些文档。打开长文档前，优先用 `rg -n` 或标题扫描定位章节。

## 总是相关

- 项目概览：`README.md`
- Agent 规则：`AGENTS.md`
- 架构：`docs/ARCHITECTURE.md`
- 贡献与模块边界：`CONTRIBUTING.md`
- 测试：`docs/TESTING.md`
- 治理：`docs/GOVERNANCE.md`
- 安全：`SECURITY.md`

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

用路线图阶段判断是否误做了未来阶段范围。
