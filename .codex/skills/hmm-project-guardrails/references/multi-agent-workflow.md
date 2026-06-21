# 多 Agent 工作流

分发任务给其他 agent、审查其他 agent、拆分任务或接收外部 agent patch 时读取本文件。

主要源文档：

- `docs/MULTI_AGENT_COLLABORATION.md`
- `.agents/rules/multiagent.md`

## 角色

- 主控 agent：负责上下文、任务边界、允许文件、review、集成和最终验证。
- 前端 worker：只处理 React/CSS/UI 交互；不处理 Rust、Tauri command 设计、文件系统、安装、存档、loader、policy、hooks、治理脚本。
- 后端 worker：处理 Rust crates、traits、app services、校验和测试；不能修改 UI 来规避接口问题。
- 审查 agent：默认只读，除非明确分配修复任务。

## 任务包最小内容

每个 worker 任务应包含：

- 目标。
- 允许修改文件。
- 禁止修改文件。
- 必读文档。
- 架构边界。
- 预期行为。
- 验证命令。
- 完成汇报格式。

允许文件是硬边界。如果任务需要边界外文件，worker 应停止并返回 `NEEDS_CONTEXT` 或 `BLOCKED`。

## Review 门禁

需要两类 review：

- 规格审查：符合任务包，没有漏做或多做。
- 质量审查：架构边界、测试、安全、可维护性、文件大小、无无关改动。

未通过 review 的改动不能进入下一任务。必须独立验证，不能只相信 worker 汇报。

## 常见反模式

- 只给模糊任务，不给允许/禁止文件。
- 让前端 worker 决定文件系统或安装行为。
- 让后端 worker 修改 UI 来掩盖 API 缺口。
- 不检查 `git status --short --branch` 就合并。
- 把“应该能过”当成验证结果。
