---
name: hmm-review-gate
description: Use only when reviewing HMM changes, preparing a PR or final handoff, handling review feedback, or evaluating merge readiness. Performs findings-first review, risk-proportionate verification checks, artifact and secret hygiene, comment triage, and required CI/merge gating without forcing a full local verification after every small fix.
---

# HMM 审查门禁

审查当前 diff，不依赖对旧状态的记忆。`.codex/`、`.agents/`、policy、scripts、hooks、workflows、
`AGENTS.md` 或核心治理文档的变更都需要显式治理 review。需要完整 PR readiness checklist 时读取
`references/review-gate-checklist.md`。

## 范围与证据

1. 检查 `git status --short --branch`、完整 PR diff 和全部未追踪文件。
2. 使用 `hmm-feature-router` 分类边界，只加载匹配的 router reference。
3. 确认 PR 交付一个用户可见纵向切片或关闭一个 release blocker。服务于同一切片的跨层修改有效，
   但 commit 应让设计、后端、adapter/UI、测试和文档保持可 review。
4. 按需求检查实现、测试、contract、文档和任务状态。
5. 检查 `.planning/`、attestation、cache、生成产物、backup、真实 Mod/save、secret、session log、
   私有路径和无关用户改动。
6. Findings first，按严重度排序并提供文件/行号证据。

同一纵向能力内的文档同步、测试搬迁、dead-code 清理、文件拆分或内部前置不要求独立 PR。只有工作
彼此无关、需要独立回滚、安全风险明显扩大，或 diff 已无法连贯 review 时才拆分。

## 验证门禁

| 变更类型 | 本地证据 |
| --- | --- |
| 低风险文档或隔离的内部/UI 变更 | 与触及文件匹配的聚焦检查。 |
| 跨层行为、public contract、task/event 语义 | 聚焦检查，加 PR candidate 的一次完整 `scripts/verify.ps1`。 |
| 安装/存档/安全/并发或治理/CI | 正负聚焦检查、一次完整 `scripts/verify.ps1` 和全 diff 自审。 |

Review 修复后重跑受影响行为的聚焦检查。仅当修改扩大高风险边界、改变 public contract 或治理规则、
改变依赖/基线，或让旧完整结果失效时，才重复完整本地验证。最终 commit 的 required CI 必须运行并
达到终态 `success`。

## Review 与合并门禁

- 阅读全部 review、inline thread 和 comment，并分类为真实 bug、测试/contract 缺口、维护性问题、
  误报、接受风险、暂缓项或维护者决策。
- 修复真实 bug，并在可行时补回归测试。只有源码、测试、contract 或命令证据支持时才关闭误报；
  不得因评论来自自动 reviewer 或初看无问题就忽略。
- CodeRabbit 缺席时执行一次独立全 diff 自审并记录证据；不要无限等待额度，也不要把缺席当成批准。
- 存在未解决 Critical/Important finding、未解决 thread、缺少必要证据，或 required check 处于
  pending、failed、cancelled、timed out、skipped、neutral、action-required 时，不得合并。
- 优先正常合并。只有获得明确授权且内容、review、CI 门禁全部满足时才能使用 `--admin`，不得用它
  绕过真实门禁。

## Finding 格式

每条 finding 包含 severity、status、location、problem、risk、evidence 和最小可靠修复：

- `Critical`：玩家数据丢失、不安全真实写入、secret 泄漏或危险的宽泛文件系统能力。
- `Important`：架构边界破坏、高风险覆盖缺失或 public contract 过期。
- `Moderate`：可维护性或边界覆盖风险。
- `Minor`：措辞、格式或低风险 polish。

没有 finding 时也要明确说明，并报告检查范围、实际运行命令、未运行命令及原因和残余风险。命令未在
相关 commit 上成功完成时，不得声称通过。
