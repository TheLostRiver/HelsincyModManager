# 更新日志

本项目遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 的基本结构，并计划采用语义化版本。

当前项目仍处于规划和脚手架前阶段，因此尚未发布正式版本。

## [Unreleased]

### Added

- 初始化项目规划文档。
- 添加架构设计、路线图、贡献指南、安全策略、测试指南和 AI 协作约束。
- 添加发布与产物规划文档。
- 添加机器可读策略文件、本地验证脚本、Git hooks 和 GitHub Actions 验证工作流。
- 添加 Codex 上下文管理工具目录 `.codex/`，用于项目内 planning hooks、skills、脚本和模板。
- 添加 CODEOWNERS 和工程治理文档，规划分支保护、治理文件 review 和强制约束层级。
- 添加日志与审计设计文档，明确日志类型、脱敏规则、Audit Log、诊断导出和测试要求。
- 添加 Tauri 2、React、TypeScript、Vite 与 Rust workspace 脚手架基线。
- 根据 Pencil 设计稿落地首启工作台前端页面。
- 添加安全的 `app_health` Tauri command，用于验证前后端桥接。
- 添加 React Hooks ESLint 规则，用于提前发现 hooks 使用问题。

### Changed

- 将项目文档默认语言确认为简体中文。
- 放宽文件大小治理提醒线，并增加大文件滥用的硬性干预线。
- 更新忽略规则，排除 Python 缓存和上下文管理工具运行时状态。
- 扩展文件大小检查覆盖范围，纳入 JavaScript、样式、HTML、Vue、Svelte、Python、Shell、TOML 和配置文件。
- 更新统一验证脚本和 GitHub Actions，使其覆盖前端类型检查、lint、构建以及 Rust workspace 检查。
- 固定 GitHub Actions 第三方 action 到 commit SHA，降低供应链风险。
- 为 Tauri 配置生产 CSP，并保留开发环境 CSP 放宽入口。
- 改进首启工作台导航的可访问性和中文文案一致性。
- 补充多游戏扩展边界，明确通用流程、游戏适配器和前端专属 UI 的职责划分。

### Fixed

- 补齐 Tauri Linux CI 需要的 PNG 图标资源，并在统一验证脚本中检查 Tauri 图标资产是否存在。
- 避免 Windows release 构建打开额外控制台窗口。

### Security

- 添加 Mod 包、存档、敏感信息和文件写入相关的安全策略。
- 添加禁止文件和敏感信息扫描的基础验证脚本。

## 版本记录原则

每次面向用户、贡献者、发布流程或安全边界的变化，都应记录到 `[Unreleased]`。

推荐分类：

- `Added`：新增功能、文档、脚本、工作流。
- `Changed`：行为变化、架构调整、流程调整。
- `Deprecated`：计划废弃但仍可用的内容。
- `Removed`：已移除内容。
- `Fixed`：问题修复。
- `Security`：安全相关变化。

发布新版本时，将 `[Unreleased]` 中的内容移动到对应版本号下，例如：

```text
## [0.1.0-alpha.1] - 2026-xx-xx
```
