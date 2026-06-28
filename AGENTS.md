# AI 协作约束

本文档用于约束 AI coding agent 在本仓库中的工作方式。目标是让 AI 协作更稳定、可追踪，并避免破坏玩家数据安全相关的设计边界。

## 默认沟通方式

- 默认使用简体中文回复。
- 文档默认使用简体中文。
- 代码命名使用英文。
- 解释设计时优先说明取舍和影响边界，不写空泛结论。

## 工作前检查

开始修改前应先查看：

- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/ROADMAP.md`
- `CONTRIBUTING.md`
- `docs/TESTING.md`
- `docs/GOVERNANCE.md`
- `SECURITY.md`

涉及安全、文件写入、存档、安装回滚、并发、游戏适配器时，必须先确认相关文档中的约束。

## 上下文管理工具

仓库内的 `.codex/` 目录用于存放本项目的 Codex 上下文管理 hooks、skills、脚本和模板。

仓库内的 `.agents/` 目录用于存放 Antigravity IDE、Gemini 或其他外部 agent 可读取的协作规则适配文件。它不是运行时缓存，而是项目治理文档的一部分。

约束：

- `.codex/` 下的源码、模板和 skill 文档可以纳入版本管理。
- `.agents/` 下的规则文档可以纳入版本管理，但内容应与 `docs/MULTI_AGENT_COLLABORATION.md` 保持一致。
- `.planning/`、`.plan-attestation`、`__pycache__/`、`*.pyc` 是运行时状态或缓存，不能提交。
- 修改 `.codex/` 视为治理相关变更，应触发人工 review。
- 修改 `.agents/` 视为治理相关变更，应触发人工 review。
- 不要在 `.codex/` 中写入真实 token、会话日志、玩家数据或本地私有路径。
- 不要在 `.agents/` 中写入真实 token、会话日志、玩家数据、本地私有路径或 IDE scratch 路径。

## 修改原则

- 优先小步提交。
- 不做无关重构。
- 不回退用户或其他协作者的改动。
- 不把多个职责堆进一个文件。
- 不让前端直接承担文件系统规则。
- 不让游戏适配规则散落在通用核心逻辑里。
- 不绕过 `InstallPlan` / manifest / backup / rollback 设计。

## 文件编辑约束

- 新增或修改文件前，先确认它属于哪个模块边界。
- 手工编辑优先使用 patch。
- 不用脚本批量改写大量文件，除非改动明确、可验证。
- 不提交真实 Mod 包、真实存档、token、cookie、API key。
- 不把生成产物、缓存、备份目录提交到仓库。

## 高风险区域

以下区域需要格外谨慎：

- 压缩包解压和路径校验。
- 游戏目录写入、覆盖、删除。
- 存档备份和恢复。
- 安装清单和回滚逻辑。
- Tauri command 暴露的文件操作。
- 并发任务、锁和取消逻辑。
- 平台路径识别和 Steam library 扫描。
- 外部工具、loader、DLL 相关检测。

涉及这些区域时，必须补充测试或说明无法测试的原因。

## 并发约束

- 扫描、hash、解压、分析可以并行。
- 同一个游戏实例的写入必须串行。
- 同一个 profile 的启用/禁用操作必须串行。
- 不要在持有游戏写锁时做长时间任务。
- 进度事件必须携带 task id，避免 UI 串任务。

## 测试要求

根据改动范围参考 `docs/TESTING.md`。

提交或最终回复前应优先执行：

```powershell
./scripts/verify.ps1
```

如果 Windows PowerShell 执行策略阻止脚本运行，使用：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

如果暂时没有脚手架或测试命令不可用，最终回复必须明确：

- 已执行什么检查。
- 未执行什么检查。
- 未执行的原因。

不要声称未实际执行的测试已经通过。

## 文档维护

当改动影响以下内容时，应同步更新文档：

- 架构边界。
- 新模块或新 crate。
- 安装、卸载、备份、回滚流程。
- 安全策略。
- 测试命令。
- 游戏适配规则。
- 用户可配置项。

## 提交说明

提交说明默认使用中文或简洁英文均可，但同一批提交风格应保持一致。

推荐格式：

```text
docs: 补充协作与测试文档
feat: 添加 MHW 游戏目录识别接口
test: 覆盖安装计划冲突检测
```

提交前应确认工作区只包含本次任务相关文件。

## Agent skills

### Issue tracker

GitHub Issues（使用 `gh` CLI）。外部 PR 不纳入 triage 分流。详见 `docs/agents/issue-tracker.md`。

### Triage labels

使用默认标签词汇：`needs-triage`、`needs-info`、`ready-for-agent`、`ready-for-human`、`wontfix`。详见 `docs/agents/triage-labels.md`。

### Domain docs

单上下文布局：根目录 `CONTEXT.md` 和 `docs/adr/`。详见 `docs/agents/domain.md`。
