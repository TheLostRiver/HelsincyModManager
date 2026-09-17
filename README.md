# Helsincy Mod Manager

Helsincy Mod Manager（HMM）是一个 Windows 优先的桌面 Mod 管理器，当前面向《怪物猎人：世界
冰原》（Monster Hunter: World - Iceborne）提供 Mod 导入、安装、卸载、存档备份与恢复等能力。
当前版本为 `0.1.0-alpha.0`，仍处于 Alpha 阶段。

HMM 提供**精准锚定式新手引导**，帮助玩家从游戏目录配置开始，逐步认识 Mod 管理和存档备份。
完整的功能说明、使用场景与多 Steam 账号示例见 **[项目功能介绍](docs/PROJECT_FEATURES.md)**。

## 非官方项目声明

Helsincy Mod Manager 是由社区开发者独立维护的个人开源项目，与 CAPCOM CO., LTD.（卡普空）及其
关联公司不存在任何隶属、授权、赞助、合作或官方认可关系。本项目中提及的游戏名称、商标、角色、
图像及其他相关权利均归其各自权利人所有。

## 当前能力

- **精准锚定式新手引导**：高亮真实按钮与功能区，按步骤介绍工作台、Mod 管理、存档备份和设置；
  可从顶部入口重新开启，也支持部分页面的局部引导。
- **Steam 游戏自动扫描**：发现 Steam 库中的 MHW:I 安装目录，支持用户确认候选或手动选择目录。
- **前置文件检测**：检查游戏与 Mod 所需的前置环境，展示缺失项，方便用户补齐后重新检测。
- **Mod 导入与拖拽导入**：支持 ZIP、RAR、7z 压缩包，支持批量导入；拖入一个或多个包后先加入待导入清单。
- **安装配置与文件选择**：自行勾选或排除 Mod 包内的一个或多个文件，也可按目录批量选择；
  保存选择后查看安装预览，再确认安装。
- **批量 Mod 管理**：支持批量安装、批量卸载、批量删除，也提供单个 Mod 的安装、卸载与重装。
  已安装的 Mod 需先卸载再从库中删除；安装操作保留备份、清单及回滚和恢复记录。
- **武器、防具与猎虫重定向**：为可识别的装备 Mod 选择替换目标，预览变更后确认安装或重新应用。
- **分类、标签、筛选与排序**：整理 Mod 分类和标签，按名称、作者或标签搜索，按分类和安装状态筛选；
  按名称、首次导入时间或当前版本内容大小排序，默认最新导入并记住选择。
- **Mod 卡片预览图**：自动读取包内可用图片生成封面，支持打开预览图查看；也可开启卡片悬浮信息，
  查看 Mod 信息与装备替换目标。
- **从狩技盒子迁移 Mod**：扫描来源目录、选择候选并批量导入 HMM，保留可识别的名称、作者等信息。
- **更改 Mod 存储目录**：支持选择其他目录或磁盘，迁移已有 Mod 包，重启 HMM 后生效。
- **存档目录自动扫描与多 Steam 账号选择**：发现各账号的可用存档，由用户确认要关联的账号；
  用独立的存档备份配置档保存各账号的目录与备份设置。
- **手动、自动与系统级后台存档备份**：支持按配置档设置备份计划；开启设置中的
  **“退出后继续保护自动备份”**并确认已受保护后，即使 HMM 完全关闭，Windows 系统计划任务仍可执行自动备份。
- **备份历史与恢复**：查看备份记录、恢复存档，默认先创建恢复前安全备份；按数量、时间和空间整理普通备份，
  保留值为 `0` 时表示对应维度不限制。
- **中文、日文、英文三语**：支持简体中文、日本語、English，可在首次引导或设置中切换，也可跟随系统语言。
- 一键启动游戏，并提供日志、诊断和“关于 HMM”页面。
- 设置中可选择启动时打开工作台、Mod 管理或上次页面；选择自动保存，完整退出并重启后生效。

**第三方前置文件需自行准备。** HMM 不提供、打包或分发这些文件；相关版权归原作者所有，项目未获得
其再分发授权。请从原作者提供的渠道获取所需前置文件，按其说明安装后再检测。

**配置档专用于存档备份，与 Mod 功能独立。** 同一游戏的不同 Steam 账号使用各自的存档目录。
例如，可将 `default` 配置档关联账号 A，再新建“账号 B”配置档关联另一个账号；完成自动扫描与账号确认后，
激活相应配置档即可手动备份或设置自动备份，无需反复查找存档路径。切换、重命名或删除存档配置档，
不会改变 Mod 库、已安装 Mod 或装备重定向设置；Mod 安装状态按游戏安装目录管理。

退出后的后台备份需要配置档已启用自动备份、系统任务正常，以及电脑开机、用户已登录且相关目录可用。
完整的开关说明与使用步骤见 [项目功能介绍](docs/PROJECT_FEATURES.md)。

所有涉及玩家文件的操作都经过后端服务和受控任务流程处理，前端不会直接复制、覆盖或删除游戏
文件、Mod 文件或存档文件。

## Mod 预览图处理

HMM 不会把第三方 Mod 包中的原始图片直接交给前端。压缩包完成安全解包后，后端只从受控 sandbox
中发现候选图：对于包含 `nativePC` 的 Mod，只读取与 `nativePC` 文件夹同级的直接图片，不扫描
`nativePC` 内部资源，也不递归读取其他图片目录；带外层包装目录的压缩包同样以实际 `nativePC`
所在目录为准。没有 `nativePC` 的根目录 Mod 只检查压缩包根级图片。

预览图文件名可以使用中文、英文、数字或符号，当前支持 `.png`、`.jpg`、`.jpeg` 和 `.webp`。存在
多张图片时，常见的 `preview`、`cover`、`poster`、`thumbnail`、`image` 名称优先，其余候选按稳定
路径顺序排列；每个包最多处理 8 张，导入流程按候选顺序逐个校验并自动使用第一张通过安全校验的
图片。前端手动切换多张候选图的界面尚未接入。

每张候选图都会经过文件大小、magic bytes、解码和像素数检查。默认拒绝超过 `20 MiB` 或解码后超过
`16 MP` 的图片，通过校验后生成最长边 `768 px` 的受控 JPEG 缩略图并写入可重建缓存；这组默认值
来自导入缩略图使用的 `PreviewImagePolicy::default()`。详情页复用同一条受控流水线，但使用最长边
`1024 px` 的策略。图片缺失、损坏或处理失败只会回退到默认封面，不会阻断 Mod 导入；原始图片路径、
缓存路径和图片字节不会暴露给前端。

## 当前限制

- Alpha 版本不应视为稳定版；真实 Windows 安装态仍应按发布说明和验收清单验证。
- Windows x64 是当前主要支持和验收平台。Linux / Steam Deck 暂不属于本轮正式支持范围。
- 当前实际支持的游戏适配器为 MHW:I；《怪物猎人：崛起》和《怪物猎人：荒野》属于后续适配方向。
- 完整 Armor / Weapon catalog 仍受数据授权门禁限制。未取得可再分发数据前，只允许使用人工构造的
  最小 developer / Sandbox seed 进行相关流程验证。
- CLI 提供只读查询、诊断和受控自动化；部分 Mod 生命周期写入命令已开放，并受预览确认与安全门禁约束。
  备份创建/恢复、后台保护启停和诊断导出继续由桌面端负责。
- 左侧导航暂不显示游戏管理、任务队列和全局替换目标页；装备重定向通过 Mod 的相关入口打开。
- 多张 Mod 预览图目前只会自动选择第一张可安全处理的候选，前端候选切换界面尚未接入。

## 安全原则

Mod 安装操作遵循以下基本链路：

```text
分析 -> 构建 InstallPlan -> 前置/冲突检查 -> 备份 -> 提交 -> manifest -> 回滚/恢复
```

存档恢复必须经过二次确认；默认先写入独立的恢复前安全备份，再提交恢复。备份、manifest、回滚、
恢复证据、路径 containment、任务审计，以及按游戏安装目录或存档配置档区分的写入锁共同构成安全边界。详细约束见
[安全策略](SECURITY.md) 和相关专题文档。

## 技术栈

- 桌面框架：Tauri 2
- 前端：React、TypeScript、Vite
- 后端：Rust workspace
- 本地数据：SQLite 与受控事实仓储
- 包管理：pnpm（通过 Corepack 调用）

## 文档入口

### 项目与产品

- [项目功能介绍](docs/PROJECT_FEATURES.md)：面向玩家的功能说明、新手引导、装备重定向、多账号配置档与后台备份。
- [项目任务状态快照](docs/PROJECT_TASK_STATUS.md)：当前已完成能力、限制和验收状态。
- [路线图](docs/ROADMAP.md)：产品 backlog 与后续方向。
- [存档备份系统设计](docs/SAVE_BACKUP_DESIGN.md)：备份、恢复和整理规则。
- [精准锚定式新手引导设计](docs/ONBOARDING_TOUR_DESIGN.md)：新手引导行为与锚点约束。
- [HMM CLI 与自动化测试设计](docs/HMM_CLI_AUTOMATION_DESIGN.md)：CLI 边界和 Sandbox 自动化契约。

### 架构与安全

- [架构设计](docs/ARCHITECTURE.md)
- [Mod 库多选与批量操作交互设计](docs/MOD_LIBRARY_MULTI_SELECTION_DESIGN.md)
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

赞助用于支持持续开发、测试环境、文档维护和发布相关成本，完全自愿，不影响软件功能、更新、
问题处理顺序或开源协作。

支持方式包括 **爱发电**、**Ko-fi** 与 **微信赞赏码**（可直接扫码）：

<img src="docs/assets/support/wechat-reward-code.jpg" alt="微信赞赏码" width="240">

完整的赞助入口、用途说明与其他支持项目的方式见 [赞助与支持](docs/SPONSOR.md)。问题反馈和功能建议请
使用 GitHub [Issues](https://github.com/TheLostRiver/HelsincyModManager/issues)，提交内容请遵守
[安全策略](SECURITY.md) 中的脱敏要求。
