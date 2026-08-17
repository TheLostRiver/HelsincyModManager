# Helsincy Mod Manager

Helsincy Mod Manager（HMM）是一个 Windows 优先的桌面 Mod 管理器，当前面向《怪物猎人：世界
冰原》（Monster Hunter: World - Iceborne）提供 Mod 导入、安装、卸载、存档备份与恢复等能力。
当前版本为 `0.1.0-alpha.0`，仍处于 Alpha 阶段。

## 非官方项目声明

Helsincy Mod Manager 是由社区开发者独立维护的个人开源项目，与 CAPCOM CO., LTD.（卡普空）及其
关联公司不存在任何隶属、授权、赞助、合作或官方认可关系。本项目中提及的游戏名称、商标、角色、
图像及其他相关权利均归其各自权利人所有。

## 当前能力

- 识别 MHW:I 游戏目录并检查前置环境。
- 导入 Mod 包，查看 Mod 信息、分类、标签和预览图。
- 通过 `InstallPlan` 执行安装、卸载、真正重装，并保留 manifest、备份、回滚和恢复证据。
- 管理存档备份配置档，支持激活配置档、存档路径和备份路径设置。
- 创建手动或自动存档备份，提供后台保护、备份历史、恢复前安全备份和受控存档恢复。
- 按数量、时间和空间整理普通备份；保留值为 `0` 时表示对应维度不限制。
- 执行批量 Mod 生命周期操作和第三方 Mod 管理器（狩技盒子）批量迁移。
- 一键启动游戏，并提供日志、诊断、新手引导和“关于 HMM”页面。

所有涉及玩家文件的操作都经过后端服务和受控任务流程处理，前端不会直接复制、覆盖或删除游戏
文件、Mod 文件或存档文件。

## Mod 预览图处理

HMM 不会把第三方 Mod 包中的原始图片直接交给前端。压缩包完成安全解包后，后端只从受控 sandbox
中发现候选图：对于包含 `nativePC` 的 Mod，只读取与 `nativePC` 文件夹同级的直接图片，不扫描
`nativePC` 内部资源，也不递归读取其他图片目录；带外层包装目录的压缩包同样以实际 `nativePC`
所在目录为准。没有 `nativePC` 的根目录 Mod 只检查压缩包根级图片。

预览图文件名可以使用中文、英文、数字或符号，当前支持 `.png`、`.jpg`、`.jpeg` 和 `.webp`。存在
多张图片时，常见的 `preview`、`cover`、`poster`、`thumbnail`、`image` 名称优先，其余候选按稳定
路径顺序排列；每个包最多处理 8 张，并自动使用第一张通过安全校验的图片。前端手动切换多张候选图
的界面尚未接入。

每张候选图都会经过文件大小、magic bytes、解码和像素数检查。默认拒绝超过 `20 MiB` 或解码后超过
`16 MP` 的图片，通过校验后生成最长边 `768 px` 的受控 JPEG 缩略图并写入可重建缓存。图片缺失、
损坏或处理失败只会回退到默认封面，不会阻断 Mod 导入；原始图片路径、缓存路径和图片字节不会暴露
给前端。

## 当前限制

- Alpha 版本不应视为稳定版；真实 Windows 安装态仍应按发布说明和验收清单验证。
- Windows x64 是当前主要支持和验收平台。Linux / Steam Deck 暂不属于本轮正式支持范围。
- 当前实际支持的游戏适配器为 MHW:I；《怪物猎人：崛起》和《怪物猎人：荒野》属于后续适配方向。
- 完整 Armor / Weapon catalog 仍受数据授权门禁限制。未取得可再分发数据前，只允许使用人工构造的
  最小 developer / Sandbox seed 进行相关流程验证。
- CLI 主要用于只读查询、诊断和受控 Sandbox 自动化。Production 写入命令仍未开放；备份创建/恢复、
  后台保护启停和诊断导出继续由桌面端负责。
- 游戏管理、任务队列和独立替换目标页等扩展界面尚未完整接入。
- 多张 Mod 预览图目前只会自动选择第一张可安全处理的候选，前端候选切换界面尚未接入。

## 安全原则

Mod 和存档操作遵循以下基本链路：

```text
分析 -> 构建 InstallPlan -> 前置/冲突检查 -> 备份 -> 提交 -> manifest -> 回滚/恢复
```

存档恢复必须经过二次确认；默认先写入独立的恢复前安全备份，再提交恢复。备份、manifest、回滚、
恢复证据、路径 containment、任务审计和同一游戏/配置档的写入锁共同构成安全边界。详细约束见
[安全策略](SECURITY.md) 和相关专题文档。

## 技术栈

- 桌面框架：Tauri 2
- 前端：React、TypeScript、Vite
- 后端：Rust workspace
- 本地数据：SQLite 与受控事实仓储
- 包管理：pnpm（通过 Corepack 调用）

## 文档入口

### 项目与产品

- [项目任务状态快照](docs/PROJECT_TASK_STATUS.md)：当前已完成能力、限制和验收状态。
- [路线图](docs/ROADMAP.md)：产品 backlog 与后续方向。
- [存档备份系统设计](docs/SAVE_BACKUP_DESIGN.md)：备份、恢复和整理规则。
- [精准锚定式新手引导设计](docs/ONBOARDING_TOUR_DESIGN.md)：新手引导行为与锚点约束。
- [HMM CLI 与自动化测试设计](docs/HMM_CLI_AUTOMATION_DESIGN.md)：CLI 边界和 Sandbox 自动化契约。

### 架构与安全

- [架构设计](docs/ARCHITECTURE.md)
- [日志与审计设计](docs/LOGGING.md)
- [安全策略](SECURITY.md)

### 开发与贡献

- [贡献指南](CONTRIBUTING.md)
- [测试指南](docs/TESTING.md)
- [工程治理与强制约束](docs/GOVERNANCE.md)
- [多 Agent 协作手册](docs/MULTI_AGENT_COLLABORATION.md)
- [AI 协作约束](AGENTS.md)

### 发布与支持

- [发布与产物说明](docs/release/发布与产物说明.md)
- [构建发布与脚本说明](docs/release/构建发布与脚本说明.md)
- [更新日志](CHANGELOG.md)
- [赞助与支持](docs/SPONSOR.md)

## 本地开发

首次运行前安装依赖：

```powershell
cmd /c corepack pnpm install --frozen-lockfile
```

常用命令：

```powershell
cmd /c corepack pnpm run dev
cmd /c corepack pnpm run build
cmd /c corepack pnpm run tauri:dev
cmd /c corepack pnpm test
cargo test --workspace
```

统一验证入口：

```powershell
./scripts/verify.ps1
```

如果 PowerShell 执行策略阻止脚本运行，可使用一次性绕过方式：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Linux 环境可使用等价的 Bash 入口：

```bash
bash scripts/verify.sh
```

Tauri CLI 通过项目内的 `@tauri-apps/cli` devDependency 提供，不要求全局安装 `cargo-tauri`。CLI
的只读命令和 Sandbox 自动化示例见 [CLI 与自动化测试设计](docs/HMM_CLI_AUTOMATION_DESIGN.md)；
不要将 Production CLI 当作绕过桌面端安全流程的写入接口。

## 支持项目

赞助方式、用途说明和其他支持项目的方法见 [赞助与支持](docs/SPONSOR.md)。问题反馈和功能建议请
使用 GitHub [Issues](https://github.com/TheLostRiver/HelsincyModManager/issues)，提交内容请遵守
[安全策略](SECURITY.md) 中的脱敏要求。
