# Helsincy Mod Manager

Helsincy Mod Manager 是一个面向《怪物猎人》系列 PC 版的跨平台桌面 Mod 管理器。项目会先支持《怪物猎人：世界 冰原》，后续再考虑《怪物猎人：崛起》《怪物猎人：荒野》以及 Linux / Steam Deck 的实验性支持。

项目目前处于架构设计与规划阶段。

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
- [路线图](docs/ROADMAP.md)
- [发布与产物说明](docs/release/发布与产物说明.md)
- [构建发布与脚本说明](docs/release/构建发布与脚本说明.md)
- [贡献指南](CONTRIBUTING.md)
- [安全策略](SECURITY.md)
- [测试指南](docs/TESTING.md)
- [AI 协作约束](AGENTS.md)

## 当前验证入口

脚手架创建前，仓库提供了基础策略验证脚本：

```powershell
./scripts/verify.ps1
```

如果 Windows PowerShell 执行策略阻止脚本运行，可以使用一次性绕过方式：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

该脚本会检查必需文档、文件大小硬性线、禁止提交的文件类型、Markdown 内链和明显敏感信息。

## 仓库状态

本仓库已经完成初始化，并先沉淀项目设计文档。应用脚手架会在架构基线确认后创建。
