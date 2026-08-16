# Helsincy Mod Manager

Helsincy Mod Manager 是一个面向《怪物猎人》系列 PC 版的跨平台桌面 Mod 管理器。项目会先支持《怪物猎人：世界 冰原》，后续再考虑《怪物猎人：崛起》《怪物猎人：荒野》以及 Linux / Steam Deck 的实验性支持。

项目目前处于架构设计与脚手架基线阶段，已开始落地 Tauri 2、React、TypeScript 与 Rust workspace。

## 计划技术栈

- 桌面框架：Tauri 2
- 前端：React + TypeScript
- 后端核心：Rust
- 本地存储：SQLite
- 首个支持游戏：《怪物猎人：世界 冰原》
- 后续目标：《怪物猎人：崛起》《怪物猎人：荒野》、Linux / Steam Deck

## 设计目标

- Mod 安装必须安全、可回滚、可追踪。
- 游戏支持通过适配器扩展，而不是把逻辑写死在核心代码里。
- 分类、依赖、替换目标、备份策略、平台路径等规则尽量数据驱动。
- Mod 安装前必须经过压缩包校验、文件结构分析和依赖检查。
- 支持预览图、分类/标签、前置检查、存档备份、一键启动游戏、外观/武器/语音替换目标映射。
- 扫描、解压、hash、分析等重任务必须在后台执行，并通过受控并发保证性能和安全性。

## 文档

- [项目任务状态快照](docs/PROJECT_TASK_STATUS.md)
- [HMM CLI 与自动化测试设计](docs/HMM_CLI_AUTOMATION_DESIGN.md)
- [批量 Mod 生命周期领域设计](docs/BATCH_MOD_LIFECYCLE_DESIGN.md)
- [架构设计](docs/ARCHITECTURE.md)
- [InstallPlan 模块现状](docs/INSTALL_PLAN_STATUS.md)
- [InstallPlan MVP 待办](docs/INSTALL_PLAN_MVP_TODO.md)
- [核心 Mod 生命周期优先级计划](docs/CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md)
- [核心 Mod 生命周期产品化加固实施计划](docs/CORE_MOD_LIFECYCLE_PRODUCTIZATION_PLAN.md)
- [Core Mod Lifecycle CL0 验收基线](docs/CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md)
- [Core Mod Lifecycle CL1 实施计划](docs/superpowers/plans/2026-07-12-core-mod-lifecycle-cl1-implementation.md)
- [Core Mod Lifecycle CL3 真正重装设计](docs/superpowers/specs/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-design.md)
- [Core Mod Lifecycle CL3 真正重装实施计划](docs/superpowers/plans/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-implementation.md)
- [安装恢复受控动作实施计划](docs/INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md)
- [Mod 库分页设计](docs/MOD_LIBRARY_PAGINATION_DESIGN.md)
- [第三方 Mod 管理器批量迁移设计（狩技盒子兼容）](docs/EXTERNAL_MOD_MANAGER_BATCH_IMPORT_DESIGN.md)
- [存档备份系统设计](docs/SAVE_BACKUP_DESIGN.md)
- [自动备份后台保障设计](docs/SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md)
- [后台自动备份调度内核实现计划](docs/SAVE_BACKUP_BACKGROUND_SCHEDULER_CORE_PLAN.md)
- [存档目录自动发现设计](docs/SAVE_DIRECTORY_AUTO_DISCOVERY_DESIGN.md)
- [存档目录自动发现实现计划](docs/superpowers/plans/2026-07-05-save-directory-auto-discovery-implementation.md)
- [MHW:I 外观套装重定向设计](docs/ARMOR_RETARGET_DESIGN.md)
- [MHW:I 外观套装重定向实现计划](docs/ARMOR_RETARGET_IMPLEMENTATION.md)
- [MHW:I 武器重定向设计](docs/WEAPON_RETARGET_DESIGN.md)
- [装备 Catalog 候选数据治理](docs/EQUIPMENT_CATALOG_GOVERNANCE.md)
- [Mod 预览图安全处理设计](docs/MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md)
- [Mod 预览图安全处理实现计划](docs/MOD_PREVIEW_IMAGE_IMPLEMENTATION_PLAN.md)
- [前端外观系统设计](docs/APPEARANCE_SYSTEM.md)
- [前端外观系统扩展指南](docs/APPEARANCE_EXTENSION_GUIDE.md)
- [Dashboard v2 与侧边栏模式设计](docs/DASHBOARD_V2_SIDEBAR_MODES.md)
- [Dashboard v2 侧边栏模式实现计划](docs/superpowers/plans/2026-05-31-dashboard-v2-sidebar-modes-implementation.md)
- [精准锚定式新手引导设计](docs/ONBOARDING_TOUR_DESIGN.md)
- [路线图](docs/ROADMAP.md)
- [自主迭代路线图](docs/AUTONOMOUS_ITERATION_ROADMAP.md)
- [Codex 目标模式提示词](docs/CODEX_GOAL_MODE_PROMPTS.md)
- [发布与产物说明](docs/release/发布与产物说明.md)
- [构建发布与脚本说明](docs/release/构建发布与脚本说明.md)
- [更新日志](CHANGELOG.md)
- [工程治理与强制约束](docs/GOVERNANCE.md)
- [日志与审计设计](docs/LOGGING.md)
- [多 Agent 协作手册](docs/MULTI_AGENT_COLLABORATION.md)
- [贡献指南](CONTRIBUTING.md)
- [安全策略](SECURITY.md)
- [测试指南](docs/TESTING.md)
- [AI 协作约束](AGENTS.md)

## Agent 技能与本地工具

- 仓库只分发 `.codex/skills/hmm*` 下的 HMM 项目技能，用于功能路由、安装安全和 review 门禁。
- 通用 Codex hooks、skills、脚本、模板和个人配置不进入版本库。需要文件级任务规划时，可在本地单独安装
  **[HelsincyPlanWithFiles](https://github.com/TheLostRiver/HelsincyPlanWithFiles)**（MIT 协议）。

## 支持项目

如果 Helsincy Mod Manager 对你有帮助，欢迎通过以下方式支持项目的持续开发与维护。支持完全自愿，不影响项目功能、更新或开源协作。

### [通过爱发电支持 Helsincy](https://afdian.com/a/Helsincy)

### [通过 Ko-fi 支持 Helsincy](https://ko-fi.com/helsincy)

### 微信赞赏码

<img src="docs/assets/support/wechat-reward-code.jpg" alt="微信赞赏码" width="320">

## 本地开发

首次运行前安装前端依赖：

```powershell
cmd /c corepack pnpm install --frozen-lockfile
```

常用开发命令：

```powershell
cmd /c corepack pnpm run dev
cmd /c corepack pnpm run build
cmd /c corepack pnpm run tauri:dev
cargo test --workspace
cargo run -p hmm-cli -- --format json runtime status
cargo run -p hmm-cli -- --format json game status --game mhw
cargo run -p hmm-cli -- --environment sandbox --data-dir C:\temp\hmm-fixture --format json install plan --mod mod-a
cargo run -p hmm-cli -- --environment sandbox --data-dir C:\temp\hmm-fixture --format json install status --profile default --mod mod-a
cargo run -p hmm-cli -- --environment sandbox --data-dir C:\temp\hmm-fixture --format json backup list --profile default
cargo run -p hmm-cli -- --environment sandbox --data-dir C:\temp\hmm-fixture --format json diagnostics snapshot
```

Tauri CLI 通过 `@tauri-apps/cli` 作为项目内 devDependency 提供，不要求全局安装 `cargo-tauri`。
当前已完成 CLI-2C Sandbox 单项生命周期自动化：桌面端、CLI 与固定 `--once` 存档 worker 复用
Tauri-free runtime composition；`hmm` 提供 `runtime status`、
`game status|scan|validate|prerequisites` 和
`install plan|status|recovery scan|recovery preview`、`backup list|background status` 与
`diagnostics snapshot` 等只读命令。Sandbox 另外开放单项
`install apply|uninstall|reinstall|recovery apply`：ready preview 签发 5 分钟 opaque token，
提交要求 `--commit --yes`；安装/重装在获取共享写锁前最终重验前置 decision，锁内再重验封存计划、
manifest/recovery 状态和 containment。安装与重装
preview 只返回 `prerequisiteDecision` 的 status、stable codes 和 rules version；required missing
或 rules unavailable 阻断且不签 token，未知签名以 warning 显式继续。

CLI-4 Slice B/C 另外在 Sandbox 开放 `install batch plan|apply|result|retry`，通过批次级
`--operation install|uninstall|reinstall` 复用同一 sealed batch service。Preview 严格只读；apply
要求 `--commit --yes --preview-token`，每项继续走既有 InstallPlan/manifest/backup/rollback/recovery、
Task/Audit 和 game/profile 写锁。Production 在 CLI policy 与 runtime composition 两层继续拒绝写入。

backup 查询只读取已 checkpoint 且不存在 `hmm.db-wal`/`hmm.db-shm`
sidecar 的 SQLite；发现任一 sidecar 时以 `backup_database_unavailable` fail closed，不执行
checkpoint、修复、创建或修改。为满足 immutable snapshot 的一致性前提，backup 查询应在桌面端
关闭、数据库静止时运行；当前没有跨进程只读快照锁。backup/diagnostics 只输出稳定状态、受控平台
摘要和聚合计数，不返回存档/备份路径、Steam ID、日志正文、manifest/hash 列表或 Scheduled Task
原始信息。备份创建/恢复、后台启停与诊断导出仍未开放；Production 没有 CLI 写能力。

## 当前验证入口

仓库提供统一验证脚本：

```powershell
./scripts/verify.ps1
```

Linux / Steam Deck 开发环境可以使用原生 Bash 入口：

```bash
bash scripts/verify.sh
```

如果 Windows PowerShell 执行策略阻止脚本运行，可以使用一次性绕过方式：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

该脚本会检查验证入口契约、必需文档、文件大小硬性线、禁止提交的文件类型、Markdown 内链、
明显敏感信息、前端类型检查/lint/tests/build，以及 Rust workspace tests/check/clippy。
PowerShell 与 Bash 入口执行同一质量序列；任一命令失败都会使统一验证非零退出。

可选安装本地 Git hooks：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install-hooks.ps1
```

## 仓库状态

本仓库已经完成初始化，并沉淀项目设计文档、治理脚本、首启工作台前端页面和 Rust workspace 骨架。当前前端已拆分 App Frame、顶部状态栏、普通侧边栏、悬浮侧边栏和 Dashboard v2 组件，并通过前端边界检查防止 Dashboard 按侧边栏模式分叉。当前脚手架不会读写真实 Mod 包、真实游戏目录或真实玩家存档。
