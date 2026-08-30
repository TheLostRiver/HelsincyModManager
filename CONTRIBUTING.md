# 贡献指南

本文档用于约束 Helsincy Mod Manager 的日常协作方式。目标不是制造流程负担，而是让项目在功能增加后仍然容易理解、测试和扩展。

## 默认语言

项目文档、Issue、PR 描述和提交说明默认使用简体中文。

代码中的命名、公开 API、Rust crate 名、TypeScript 类型名使用英文。

## 项目边界

Helsincy Mod Manager 是一个基于 Tauri 的本地桌面应用，计划由以下部分组成：

- 前端 UI：`src/`
- Tauri 壳与命令：`src-tauri/`
- Rust 领域模型：`src-tauri/crates/hmm-core/`
- Rust 接口层：`src-tauri/crates/hmm-ports/`
- Rust 应用用例：`src-tauri/crates/hmm-app/`
- Rust 基础设施：`src-tauri/crates/hmm-infra/`
- MHW:I 游戏适配器：`src-tauri/crates/hmm-games-mhw/`
- Tauri-free runtime 策略与共享装配边界：`src-tauri/crates/hmm-runtime/`
- CLI transport 与机器契约：`src-tauri/crates/hmm-cli/`
- 项目治理文档：`README.md`、`CONTRIBUTING.md`、`SECURITY.md`、`AGENTS.md`、`docs/`

提交前必须先判断改动属于哪个边界，避免把多个职责堆进同一个文件或同一个模块。

## 架构原则

- 前端只负责展示、交互和状态呈现。
- Tauri command 只做参数校验、DTO 转换和用例转发。
- 应用层负责编排用例，不直接依赖具体文件系统、数据库或平台 API。
- 领域层不接触真实系统 API。
- 基础设施实现接口，不把实现细节反向塞进应用层。
- 游戏差异通过游戏适配器处理，不能散落在通用核心逻辑中。
- 安装 Mod 必须先生成 `InstallPlan`，再执行安装。
- 所有真实文件写入必须有安装清单、备份策略和回滚路径。

## 提交边界

一次提交应只解决一类问题。

推荐拆分方式：

- 文档治理改动单独提交。
- 工程脚手架单独提交。
- 前端 UI 改动和 Rust 核心改动尽量分开提交。
- 重构和行为修改尽量分开提交。
- 新功能先补必要接口和模型，再补具体实现。

不建议的提交方式：

- 同时改架构、UI、安装逻辑、发布脚本。
- 顺手重命名大量文件但没有说明目的。
- 在没有测试或说明的情况下修改文件写入、删除、备份、回滚逻辑。
- 把多个无关功能塞进一个大提交。

## PR 粒度

一个 PR 应交付一条可演示的纵向产品能力，或关闭一个明确的 release blocker。服务于同一能力的设计、
Rust 领域/app、CLI/Tauri、React、测试和文档可以放在同一 PR，用多个单一职责 commit 保持可 review。

文档同步、测试搬迁、dead-code 清理、文件拆分和内部前置默认并入相邻产品 PR，不为这些工作单独制造
没有用户价值的 PR。只有改动彼此无关、需要独立回滚、安全风险明显扩大，或 diff 已无法连贯 review
时才拆分 PR。

## 文件大小治理

达到以下阈值时，应主动评估是否需要拆分：

- TypeScript / TSX：超过 900 行开始关注，超过 1400 行应在 PR 中说明为什么暂不拆分。
- Rust：超过 800 行开始关注，超过 1200 行应在 PR 中说明为什么暂不拆分。
- YAML / Workflow：超过 400 行开始关注，超过 700 行应在 PR 中说明为什么暂不拆分。
- Markdown：超过 700 行开始关注，超过 1200 行优先考虑拆到 `docs/` 下的专门文档。

这些阈值不是硬性失败条件，也不是强行追求小文件。它们只用于提醒协作者检查文件是否已经承担过多职责。只要文件仍然高内聚、结构清晰、测试覆盖合理，可以保留较大的文件。

机器门禁把原硬上限作为非阻断 review warning。达到以下情况时，PR 需要检查职责是否过多，但
warning 本身不要求为了过 CI 立即拆分：

- 单个 TypeScript / TSX 文件超过 2500 行。
- 单个 Rust 文件超过 2200 行。
- 单个 Markdown 文档超过 2500 行。
- 单个文件同时承担 UI、状态管理、业务规则、I/O、平台适配等多种职责。
- 新增代码明显只是为了图方便继续往总控文件里堆逻辑。

`policy/project-policy.json` 另设只防止灾难性膨胀的硬上限：Rust、TypeScript 和 JavaScript
为 5000 行，样式类为 4000 行，Markdown 为 6000 行，JSON 为 10000 行，其他脚本与配置按
类别限制。超过硬上限才会使本地验证和 CI 失败。软提醒应在相关产品 PR 中顺手处理或记录理由，
不应单独制造无用户价值的重构 PR。

如果确实存在生成代码、第三方协议定义、静态数据 catalog 或与主应用无关的独立工具目录等特殊场景，可以例外，但必须在 PR 中说明原因，并尽量把生成物、静态数据、独立工具和手写业务逻辑隔离开。

## 安全和数据保护要求

本项目会修改玩家本地游戏文件和存档备份，因此必须把安全性放在便利性之前。

涉及以下行为时，必须特别谨慎：

- 解压第三方 Mod 压缩包。
- 写入、覆盖或删除游戏目录文件。
- 备份或恢复玩家存档。
- 扫描正在运行的游戏进程。
- 读取 Steam library 或平台相关路径。
- 执行外部工具或 DLL/loader 相关检测。

基本要求：

- 不信任 Mod 压缩包中的路径和文件名。
- 禁止路径穿越和绝对路径写入。
- 覆盖文件前必须备份。
- 安装结果必须写入 manifest。
- 卸载必须基于 manifest，而不是猜测文件。
- 测试不得默认操作真实游戏目录或真实存档目录。

## 并发要求

允许并行的工作：

- 压缩包检查
- 沙盒解压
- 文件扫描
- hash 计算
- 冲突分析
- 安装计划生成

必须串行或加资源锁的工作：

- 写入同一个游戏实例目录
- 卸载同一个 profile 下的 Mod
- 修改同一份安装清单
- 备份或恢复同一个存档目录
- 数据库写事务

原则：

- Prepare 阶段可以并行、可取消。
- Commit 阶段必须短、串行、可恢复。
- 不要在持有游戏写锁时做长时间解压或 hash。

## 数据驱动要求

以下内容不应直接写死在业务逻辑里：

- 默认分类
- 前置依赖规则
- 官方外观、武器、语音替换目标 catalog
- 自动备份时间间隔
- 备份保留策略
- 预览图大小限制
- 压缩包大小限制
- 平台路径规则

可以使用 SQLite 存储用户状态，使用 JSON / TOML 存储游戏规则和默认 catalog。

## 提交前验证

根据改动范围执行最小验证，具体命令以 [测试指南](docs/TESTING.md) 为准。

验证失败或环境表现异常时，先查 [排障手册](docs/TROUBLESHOOTING.md)。里面收录的都是
**症状与根因相距很远**的问题：cargo 锁死、测试成片失败实为环境自伤、自定义协议在
WebView2 下不生效、校验脚本的隐性扫描范围等。按报错字面去查容易误改没问题的代码。

- 开发期间运行 touched boundary 的聚焦测试，不在每个 commit 后重复跑完整统一入口。
- 跨层/public contract、高风险文件写入、安全、并发或治理 PR 在首次 ready 前运行一次完整统一入口。
- 低风险 docs、内部重构或隔离 UI 改动可以只保留聚焦本地证据，但 required CI 仍必须成功。
- review 小修只重跑受影响的聚焦验证；风险边界扩大、公共契约/治理规则变化、依赖或基线变化，或旧
  完整结果已不适用时，才重新运行完整本地验证。

当前统一验证入口：

```powershell
./scripts/verify.ps1
```

Linux / Steam Deck 开发环境：

```bash
bash scripts/verify.sh
```

Windows PowerShell 执行策略阻止脚本时，可以使用：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

建议安装本地 Git hooks：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install-hooks.ps1
```

hooks 会在提交和推送前运行基础检查。它们可以被绕过，因此最终门禁仍以 GitHub Actions 为准。

提交说明中应写明：

- 已执行哪些验证。
- 哪些验证因为环境限制未执行。
- 是否涉及真实文件写入、存档备份、安装回滚等高风险路径。

不要把“理论上应该能过”当成“已验证”。

## PR 描述最低要求

PR 至少说明：

- 改了什么。
- 为什么改。
- 涉及哪些模块。
- 影响哪些平台。
- 执行了哪些验证。
- 还有哪些风险或未覆盖场景。

## 文档维护

- `README.md`：项目介绍和文档入口。
- `docs/ARCHITECTURE.md`：架构、模块边界和核心模型。
- `docs/ROADMAP.md`：阶段计划。
- `docs/TESTING.md`：测试和验证基线。
- `docs/GOVERNANCE.md`：工程治理与强制约束。
- `CONTRIBUTING.md`：协作规则。
- `SECURITY.md`：安全报告和敏感信息处理。
- `AGENTS.md`：AI 协作约束。

不要把架构、路线图、测试规则和安全规则都塞进 README。
