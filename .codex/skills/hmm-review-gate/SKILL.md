---
name: hmm-review-gate
description: Use when reviewing Helsincy Mod Manager changes, preparing final handoff or PR readiness, checking governance edits, auditing tests, scanning for forbidden artifacts, or validating safety and architecture boundaries before completion.
---

# HMM Review Gate

## 概览

交付前 review HMM 变更，必须 findings first、基于已验证证据，并套用项目专属安全门禁。`.codex/`、`.agents/`、policy、scripts、hooks、workflows 和核心 docs 都视为需要人工 review 的治理变更。

HMM 专属 skills 属于本仓库 `.codex/skills/`，不属于全局 skill 目录。

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
2. 按边界分类 changed files：frontend、Tauri、Rust crate、safety flow、task/concurrency、docs/governance、generated/runtime artifacts。
3. 先列 findings，并按严重级别排序。可行时提供文件和行号引用。
4. 检查 tests/verification 是否匹配 touched boundary，且是否实际运行。
5. 当 architecture、command DTOs、error codes、task phase codes、typed API wrappers、custom protocols、safety rules、user settings、logging/audit behavior 或 game adapter behavior 变化时，检查 docs/contract 是否同步更新。
6. 检查 repository hygiene：不得有 `.planning/`、`.plan-attestation`、`__pycache__/`、`*.pyc`、dist/cache/backup outputs、真实 Mod/save data、tokens、cookies、API keys、private paths 或 session logs。
7. 结尾给出简短 summary、已执行 checks、未执行 checks 及原因、residual risk。

## 严重级别

| 级别 | 用于 |
| --- | --- |
| Critical | 玩家数据丢失、没有安全链路的真实 game/save 写入、secret 泄漏、暴露危险 filesystem command。 |
| Important | 架构边界破坏、高风险代码缺少必要测试、public DTO/event 变化但 contract/docs 过期。 |
| Moderate | 可维护性风险、超大/混合职责文件、edge coverage 不完整、错误不清晰。 |
| Minor | typo、小文档缺口、格式、低风险 polish。 |

如果没有 findings，明确说明，同时仍要提到未验证区域或残余风险。

## 硬性停止条件

- 必要验证失败，或未运行且没有说明时，不要标记工作完成。
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
- 不列验证证据就说 “looks good”。
