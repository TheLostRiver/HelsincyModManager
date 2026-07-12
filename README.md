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

- [架构设计](docs/ARCHITECTURE.md)
- [InstallPlan 模块现状](docs/INSTALL_PLAN_STATUS.md)
- [InstallPlan MVP 待办](docs/INSTALL_PLAN_MVP_TODO.md)
- [核心 Mod 生命周期优先级计划](docs/CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md)
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
- [Mod 预览图安全处理设计](docs/MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md)
- [Mod 预览图安全处理实现计划](docs/MOD_PREVIEW_IMAGE_IMPLEMENTATION_PLAN.md)
- [前端外观系统设计](docs/APPEARANCE_SYSTEM.md)
- [前端外观系统扩展指南](docs/APPEARANCE_EXTENSION_GUIDE.md)
- [Dashboard v2 与侧边栏模式设计](docs/DASHBOARD_V2_SIDEBAR_MODES.md)
- [Dashboard v2 侧边栏模式实现计划](docs/superpowers/plans/2026-05-31-dashboard-v2-sidebar-modes-implementation.md)
- [路线图](docs/ROADMAP.md)
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

## 内置工具

- **[HelsincyPlanWithFiles](https://github.com/TheLostRiver/HelsincyPlanWithFiles)** — 基于 `.codex/` 的上下文管理与任务规划工具（MIT 协议），为 AI Agent 提供文件级规划、进度追踪与上下文切换能力。

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
```

Tauri CLI 通过 `@tauri-apps/cli` 作为项目内 devDependency 提供，不要求全局安装 `cargo-tauri`。

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

该脚本会检查必需文档、文件大小硬性线、禁止提交的文件类型、Markdown 内链、明显敏感信息、前端类型检查、前端 lint、前端构建以及 Rust workspace 测试和编译检查。

可选安装本地 Git hooks：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install-hooks.ps1
```

## 仓库状态

本仓库已经完成初始化，并沉淀项目设计文档、治理脚本、首启工作台前端页面和 Rust workspace 骨架。当前前端已拆分 App Frame、顶部状态栏、普通侧边栏、悬浮侧边栏和 Dashboard v2 组件，并通过前端边界检查防止 Dashboard 按侧边栏模式分叉。当前脚手架不会读写真实 Mod 包、真实游戏目录或真实玩家存档。
