# Review Gate Checklist

用于 HMM code review、PR readiness 和 final handoff。

## Workspace

- 已检查 `git status --short --branch`。
- 已识别 unrelated user/agent changes，且未回退。
- 新 untracked files 是有意且相关的。
- Generated/runtime artifacts 不在 commit scope 中。

## 治理

- `.codex/`、`.agents/`、`policy/`、`scripts/`、`.githooks/`、`.github/workflows/`、`.github/CODEOWNERS`、`AGENTS.md`、`README.md`、`CHANGELOG.md`、`CONTRIBUTING.md`、`SECURITY.md`、`docs/GOVERNANCE.md`、`docs/LOGGING.md`、`docs/TESTING.md`、`docs/release/` 和核心 docs 都视为 governance changes。
- Governance changes 说明规则为何改变，以及是否影响 agent behavior、CI、hooks 或 review requirements。
- Governance files 中不得写入 token、session log、player data、private local path、real Mod content 或 IDE scratch path。

## 架构

- Frontend 只处理 UI/view model/typed API。
- Tauri commands 是薄 DTO/app-state boundaries。
- Rust crates 保持 domain/app/ports/infra/game adapter 方向。
- Game-specific rules 留在 adapters。
- Install/save/high-risk flows 不绕过 plan/manifest/backup/rollback/audit rules。

## 边界 Skills

- Frontend UI/state/CSS/API wrapper reviews 使用 `hmm-frontend-workflow`。
- Tauri command/DTO/error/task event/custom protocol 或 contract reviews 使用 `hmm-tauri-command` 和 `docs/FRONTEND_BACKEND_CONTRACT.md`。
- Rust crate placement/dependency/app/ports/infra/game adapter reviews 使用 `hmm-rust-crate-boundary`。
- Task/cancellation/progress/queue/lock/database serialization reviews 使用 `hmm-task-and-concurrency`。
- Install/save/file-write/audit/diagnostic/data-safety reviews 使用 `hmm-install-safety` 和 `docs/LOGGING.md`。

## Tests 和 Docs

- 每个 touched boundary 的 verification 匹配 `docs/TESTING.md`。
- 记录实际 commands 和 results。
- 省略的 checks 有具体原因。
- Contract docs 随 command names、DTOs、errors、task phase codes、typed API wrappers、custom protocols 或 frontend/backend contract changes 更新。
- Behavior、safety boundaries、verification、audit、packaging 或 release behavior 改变时，更新 architecture/security/testing/logging/release docs。

## 仓库卫生

- 不提交 `.planning/`、`.plan-attestation`、`__pycache__/`、`*.pyc`、build outputs、backups、real saves、real Mod packages、tokens、cookies、API keys 或 private paths。
- Test fixtures 人工构造且最小化。
- Logs 和 screenshots 已脱敏。

## Review 输出

- Findings first，按 severity 排序。
- File/line references 紧凑且可执行。
- Open questions 和 assumptions 明确。
- Summary 和 verification evidence 简洁。
- 不做没有证据支持的测试通过声明。
