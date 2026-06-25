---
name: hmm-project-guardrails
description: Use when working in the Helsincy Mod Manager repository, especially before code changes, architecture decisions, frontend/backend boundary work, Tauri commands, Rust crates, mod installation, retargeting, file I/O, safety-sensitive logic, tests, governance files, or multi-agent handoffs.
---

# HMM Project Guardrails

这是 Helsincy Mod Manager 仓库的项目入口 skill。它用于让 agent 在不一次性加载所有长文档的情况下，先对齐项目架构、开发边界和安全红线。

## 开工动作

1. 默认使用简体中文回复，除非用户另有要求。
2. 修改前先判断模块边界，再读取最小相关文档。
3. 搜索文件和文本优先使用 `rg` / `rg --files`。
4. `.planning/`、`.plan-attestation`、`__pycache__/`、`*.pyc`、构建产物、备份、真实存档、真实 Mod 包、token、cookie、API key 都不能提交。
5. 不回退用户或其他 agent 的改动，除非用户明确要求。
6. 创建 PR、推送 review 修复、将草稿 PR 标记为 ready 或最终交付前，必须触发 `hmm-review-gate`，并在最后一次变更后完成本地自审；不能只依赖 CodeRabbit、CI 或外部 reviewer。

任何文件修改前必须先查看这些基础文档。为控制上下文，可以先用标题扫描或关键词定位，但不能用 reference 摘要替代它们：

- `AGENTS.md`
- `README.md`
- `docs/ARCHITECTURE.md`
- `CONTRIBUTING.md`
- `docs/TESTING.md`
- `docs/GOVERNANCE.md`
- `SECURITY.md`

小范围任务在完成基础文档检查后，再读下面对应 reference，并按需打开其指向的源文档。

## 架构速览

- 前端 `src/`：展示、交互、局部 UI 状态、typed API wrappers。
- Tauri 壳 `src-tauri/src/`：薄 command 边界，只做参数校验、DTO 映射和调用应用服务。
- `hmm-core`：纯领域模型和规则，不接触真实文件系统、数据库、Tauri 或 MHW 路径解析。
- `hmm-ports`：应用服务依赖的 traits/interfaces。
- `hmm-app`：用例编排，依赖 trait，不依赖具体 infra。
- `hmm-infra`：真实文件系统、配置存储、Steam discovery、hash、staging、压缩包等 I/O。
- `hmm-games-mhw`：MHW:I adapter、游戏规则和 catalog。

前端不能承担安装规则、游戏路径解析、retarget slot 改写、备份/回滚策略或文件系统安全规则。

## 硬边界

- 游戏目录写入不能绕过 `InstallPlan`、manifest、backup、rollback 设计。
- MHW 专属路径、资源编号、`nativePC`、`plNNN_VVVV`、loader 细节不能写进通用 core 或通用前端。
- Tauri command 不能暴露宽泛文件系统能力；command 应是窄用例入口。
- 持有游戏写锁时不能执行长时间解压、hash、分析。
- 同一游戏实例的写入串行；同一 profile 的启用/禁用串行；扫描、hash、解压、分析可以并行。
- 长任务进度事件必须携带 task id。
- 原始导入 Mod 包只读；转换后的变体放在 staging。

## 高风险工作

如果任务涉及压缩包解压、路径校验、游戏目录写入、覆盖/删除、存档、安装/卸载、manifest、rollback、并发、Steam 路径、loader/DLL、Tauri 文件命令、日志、诊断或 retarget，先读：

- `references/safety-boundary.md`
- `docs/TESTING.md`
- `SECURITY.md`
- `docs/LOGGING.md`

必须补充聚焦测试，或说明为什么无法测试。

## Reference 导航

- 项目和 crate 边界：`references/architecture-map.md`
- 前后端分工和 Tauri DTO：`references/frontend-backend-boundary.md`
- 安全、日志、安装、回滚、并发：`references/safety-boundary.md`
- 按改动类型选择验证命令：`references/testing-map.md`
- 功能对应应读哪些设计文档：`references/feature-doc-index.md`
- 多 agent 任务包和 review 门禁：`references/multi-agent-workflow.md`

reference 是导航和规则摘要，不能替代当前源码和源文档。

## 治理变更

修改 `.codex/`、`.agents/`、`policy/`、`scripts/`、`.githooks/`、`.github/workflows/`、`AGENTS.md`、`CONTRIBUTING.md`、`SECURITY.md` 或核心 docs 都属于治理相关变更。保持范围小，说明 review 影响，并预期需要人工 review。

不要向 `.codex/` 或 `.agents/` 写入 token、会话日志、玩家数据、本地私有路径、真实 Mod 内容或 IDE scratch 路径。

## 验证

最终交付前优先运行统一验证脚本：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

如果改动很小，按 `docs/TESTING.md` 和 `references/testing-map.md` 执行最小有意义检查。未在当前回合实际运行且成功退出的命令，不能声称已通过。
