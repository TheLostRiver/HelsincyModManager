---
name: hmm-review-gate
description: Use when reviewing Helsincy Mod Manager changes, preparing final handoff or PR readiness, checking governance edits, auditing tests, scanning for forbidden artifacts, or validating safety and architecture boundaries before completion.
---

# HMM Review Gate

## 概览

交付前 review HMM 变更，必须 findings first、基于已验证证据，并套用项目专属安全门禁。`.codex/`、`.agents/`、policy、scripts、hooks、workflows 和核心 docs 都视为需要人工 review 的治理变更。

HMM 专属 skills 属于本仓库 `.codex/skills/`，不属于全局 skill 目录。

## PR 前自审门禁

创建 PR、将草稿 PR 标记为 ready、推送 review 修复、或做最终交付前，必须在最后一次代码/文档变更后执行至少一次本地自审。不能只依赖 CodeRabbit、CI、GitHub review 或其他外部 reviewer；外部 review 是补充，不是替代。

本地自审至少检查：

- `git status --short --branch`，确认工作区和未追踪文件。
- PR diff、staged diff 或最新增量 diff，确认文件范围只包含本次任务。
- 变更边界对应的 HMM skill 约束，尤其是安全、Tauri、Rust crate、frontend、task/concurrency 和治理变更。
- 高风险区域是否有聚焦测试，或是否明确说明无法测试的原因。
- 是否包含禁入产物、真实 Mod/save、token、cookie、API key、私有路径、会话日志或生成/缓存产物。
- docs、contract、TODO 或设计文档是否需要同步。

如果本地自审后又产生新的 commit，必须至少复审新增 diff；若改动影响安全链路、公共契约或治理规则，应重新跑完整本地自审。

## 必读上下文

Review 前，读或扫描：

- `AGENTS.md`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/ROADMAP.md`
- `CONTRIBUTING.md`
- `docs/TESTING.md`
- `docs/GOVERNANCE.md`
- `SECURITY.md`
- 涉及 multi-agent work 时读 `docs/MULTI_AGENT_COLLABORATION.md`。

根据 touched files 和行为选择边界 skills，不只在高风险工作时才使用：

- Frontend UI、state、CSS、typed API wrappers、accessibility、responsive behavior 或 browser smoke：`hmm-frontend-workflow`。
- Tauri commands、DTOs、command errors、task events、custom protocols 或 frontend/backend contract shape：读 `docs/FRONTEND_BACKEND_CONTRACT.md` 并使用 `hmm-tauri-command`。
- Rust crate placement、dependency direction、app/ports/infra boundaries、game adapters 或 DTO/domain mapping：`hmm-rust-crate-boundary`。
- TaskManager、long-running tasks、cancellation、progress phases、queues、locks 或 database/write serialization：`hmm-task-and-concurrency`。
- Mod import、archive extraction、staging、path validation、game writes、overwrite/delete、manifest、backup、uninstall、rollback、save backup、audit logging、diagnostics 或 data-safety flow：读 `docs/LOGGING.md` 并使用 `hmm-install-safety`。

## Review 顺序

1. 检查 `git status --short --branch`，识别 unrelated/user changes。不要回退它们。
2. 检查当前 diff 或 staged diff；PR 前还要检查即将推送到 PR 的增量 diff。
3. 按边界分类 changed files：frontend、Tauri、Rust crate、safety flow、task/concurrency、docs/governance、generated/runtime artifacts。
4. 先列 findings，并按严重级别排序。可行时提供文件和行号引用。
5. 检查 tests/verification 是否匹配 touched boundary，且是否实际运行。
6. 当 architecture、command DTOs、error codes、task phase codes、typed API wrappers、custom protocols、safety rules、user settings、logging/audit behavior 或 game adapter behavior 变化时，检查 docs/contract 是否同步更新。
7. 检查 repository hygiene：不得有 `.planning/`、`.plan-attestation`、`__pycache__/`、`*.pyc`、dist/cache/backup outputs、真实 Mod/save data、tokens、cookies、API keys、private paths 或 session logs。
8. 按统一报告格式输出 findings、状态、结论、已执行 checks、未执行 checks 及原因、residual risk。

## 严重级别

| 级别 | 用于 |
| --- | --- |
| Critical | 玩家数据丢失、没有安全链路的真实 game/save 写入、secret 泄漏、暴露危险 filesystem command。 |
| Important | 架构边界破坏、高风险代码缺少必要测试、public DTO/event 变化但 contract/docs 过期。 |
| Moderate | 可维护性风险、超大/混合职责文件、edge coverage 不完整、错误不清晰。 |
| Minor | typo、小文档缺口、格式、低风险 polish。 |

如果没有 findings，明确说明，同时仍要提到未验证区域或残余风险。

## Review 报告格式

报告必须 findings first。每条 finding 至少包含：

- 级别：Critical / Important / Moderate / Minor。
- 状态：待处理 / 已修复 / 误报 / 暂缓 / 接受风险 / 需要用户决策。
- 位置：文件和行号；没有稳定行号时写清楚模块、函数、命令或文档章节。
- 问题：当前代码或文档哪里不符合预期。
- 风险：可能导致什么行为、数据安全、架构边界、维护性或用户体验影响。
- 证据：来自 diff、源码、测试输出、文档约束或复现步骤的依据。
- 建议：最小可行修复、替代方案，或需要用户确认的取舍。

如果 review 结论是“无发现”，仍要列出已检查范围、已执行验证、未覆盖区域和残余风险。

有 findings 时使用这个结构或等价结构：

```markdown
Findings:
- [Important][待处理] path/to/file:123
  问题：
  风险：
  证据：
  建议：

Open questions / assumptions:
- ...

Summary:
- ...

Verification:
- 已执行：...
- 未执行：...，原因：...

Residual risk:
- ...
```

无 findings 时使用这个结构或等价结构：

```markdown
Findings:
- 无。

Checked scope:
- ...

Verification:
- 已执行：...
- 未执行：...，原因：...

Residual risk:
- ...
```

## 误报和解决状态

允许把 finding 标记为误报、暂缓或接受风险，但必须写清楚理由和证据：

- 误报：说明原判断为什么不成立，并引用源码、测试、文档或运行结果。
- 暂缓：说明为什么本 PR 不处理、后续跟踪位置，以及当前风险是否可接受。
- 接受风险：说明风险边界、用户或维护者的决策依据，以及不会扩大到哪些场景。

修复 review finding 后，必须复审相关 diff。若修复引入新 commit，至少检查新增 diff；涉及安全链路、公共契约或治理规则时，重新执行完整本地自审。

## Review 记录持久化

默认不要把每次 review 报告作为新文件提交到仓库，避免制造噪音或写入本地路径、会话上下文、误判过程、截图、日志、真实 Mod/save 或敏感信息。

持久化位置按场景选择：

- PR review：写在 GitHub review、PR 评论、commit/PR 描述或最终交付回复中。
- 长任务、多 agent、跨回合跟踪：可以写入 `.planning/findings.md`、任务进度或外部任务系统，但 `.planning/` 不能提交。
- 长期有效的规则、TODO、设计决策或架构约束：同步到正式 docs、TODO 文档或对应治理文件。

任何持久化 review 记录都要带状态；未解决的 Critical/Important finding 必须有处理决定或用户决策记录。

## 硬性停止条件

- 必要验证失败，或未运行且没有说明时，不要标记工作完成。
- 创建 PR、推送 review 修复、将草稿 PR 标记为 ready 或最终交付前，如果最后一次代码/文档变更后尚未执行本地自审，不要继续发布或声称 PR ready。
- 不要把 CodeRabbit、CI、GitHub review 或其他外部 reviewer 当作唯一 review 来源。
- 有未解决的 Critical/Important finding，且没有修复、误报证据、暂缓理由、接受风险说明或用户决策记录时，不要声称 PR ready。
- `.codex/`、`.agents/`、policy、script、hook、workflow 或 core doc 改动未指出需要治理 review 时，不要批准。
- 不要因为 generated/runtime artifacts 是 untracked 就忽略。
- 不要让 frontend、Tauri command 或 generic core code 接管 install/safety/game-adapter rules。
- 除非测试在当前相关上下文中成功运行，否则不要声称上一轮测试通过。

## 验证

详细 review 使用 `references/review-gate-checklist.md`。优先运行：

```powershell
git status --short --branch
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

按 `docs/TESTING.md` 补边界专属命令。任何检查未运行，final handoff 必须说明原因。

## 常见错误

- 只写 summary，把 defects 埋起来。
- 凭记忆 review，而不是看当前文件。
- 把 governance skill edits 当成普通 docs。
- 把 “untracked” 当成 “irrelevant”。
- 把误报、暂缓或接受风险写成一句话结论，没有证据和边界。
- 把临时 review 报告提交进仓库，夹带本地路径或会话上下文。
- 不列验证证据就说 “looks good”。
