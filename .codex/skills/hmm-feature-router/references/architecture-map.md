# 架构边界

当任务涉及 crate 布局、模块放置、领域建模、应用服务、adapter 或功能边界时读取本文件。

主要源文档：

- `docs/ARCHITECTURE.md`
- `CONTRIBUTING.md`
- `docs/ROADMAP.md`

## 分层

```text
React UI -> Tauri shell -> hmm-runtime <- CLI
hmm-runtime -> hmm-app / hmm-infra / hmm-games-mhw / hmm-ports / hmm-core
hmm-app -> hmm-ports -> hmm-core
hmm-app -> hmm-core
hmm-infra -> hmm-ports -> hmm-core
hmm-infra -> hmm-core
hmm-games-mhw -> hmm-ports -> hmm-core
hmm-games-mhw -> hmm-core
```

箭头从依赖方指向被依赖方。`hmm-app` 通过 ports 编排用例，infra 和 game adapter 实现 ports，
`hmm-runtime` 负责 GUI/CLI 共用 composition。UI 和 command 不接触 infra 细节；core 不知道 Tauri、
SQLite、真实文件系统 API、Steam 或具体 MHW 路径。

## Rust Crate 职责

- `src-tauri/`：Tauri app crate，command 注册、app state、event 入口。
- `hmm-core`：`Game`、`Profile`、`InstallPlan`、`Manifest`、`ReplacementTarget`、conflict 等领域类型。
- `hmm-ports`：repository、filesystem、game adapter、task、discovery、staging、manifest、backend traits。
- `hmm-app`：导入、分析、计划、安装、卸载、备份、启动、retarget 等用例编排。
- `hmm-infra`：JSON/SQLite、真实文件系统、压缩包工具、hash、Steam discovery、平台 API。
- `hmm-games-mhw`：Monster Hunter: World - Iceborne 规则、路径、catalog、资源编号解析。
- `hmm-runtime`：为 Tauri 和 CLI 组合 app services、ports 实现、game adapter 和共享运行时状态。

只有在真实 adapter 工作落地时，才创建未来游戏 crate（如 `hmm-games-rise`、`hmm-games-wilds` 或 common helper）。

## 放置规则

- 游戏专属 catalog 和路径格式放在游戏 adapter。
- 通用安装、备份、冲突、profile、任务流程放在 core/app/ports。
- 真实 I/O 属于 infra，并通过 ports 隔离。
- Tauri command 是薄包装；如果 command 里出现业务规则，应移动到 `hmm-app`。
- 前端 feature 默认保持游戏无关；只有 capabilities/catalog 无法表达时才建游戏专属 UI。

## 常见错误

- 把 MHW slot 解析写进 `hmm-core`。
- 让 React 组件拼安装路径。
- 把 `internal_id` 当 replacement 的全局主键。
- 在 command 中直接复制文件，而不是经过 app use case。
- 把 UI shell 关注点和领域状态混在一起。
