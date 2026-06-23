# Review Gate Checklist

Use this checklist for HMM code review, PR readiness, and final handoff.

## Workspace

- `git status --short --branch` checked.
- Unrelated user/agent changes identified and not reverted.
- New untracked files are intentional and relevant.
- Generated/runtime artifacts are absent from commit scope.

## Governance

- `.codex/`, `.agents/`, `policy/`, `scripts/`, `.githooks/`, `.github/workflows/`, `.github/CODEOWNERS`, `AGENTS.md`, `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, `docs/GOVERNANCE.md`, `docs/LOGGING.md`, `docs/TESTING.md`, `docs/release/`, and core docs are treated as governance changes.
- Governance changes explain why rules changed and whether agent behavior, CI, hooks, or review requirements are affected.
- No token, session log, player data, private local path, real Mod content, or IDE scratch path is written into governance files.

## Architecture

- Frontend only handles UI/view model/typed API.
- Tauri commands are thin DTO/app-state boundaries.
- Rust crates preserve domain/app/ports/infra/game adapter direction.
- Game-specific rules stay in adapters.
- Install/save/high-risk flows do not bypass plan/manifest/backup/rollback/audit rules.

## Boundary Skills

- Frontend UI/state/CSS/API wrapper reviews use `hmm-frontend-workflow`.
- Tauri command/DTO/error/task event/custom protocol or contract reviews use `hmm-tauri-command` and `docs/FRONTEND_BACKEND_CONTRACT.md`.
- Rust crate placement/dependency/app/ports/infra/game adapter reviews use `hmm-rust-crate-boundary`.
- Task/cancellation/progress/queue/lock/database serialization reviews use `hmm-task-and-concurrency`.
- Install/save/file-write/audit/diagnostic/data-safety reviews use `hmm-install-safety` and `docs/LOGGING.md`.

## Tests and Docs

- Verification matches `docs/TESTING.md` for every touched boundary.
- Actual commands and results are recorded.
- Omitted checks have concrete reasons.
- Contract docs update command names, DTOs, errors, task phase codes, typed API wrappers, custom protocols, or frontend/backend contract changes.
- Architecture/security/testing/logging/release docs update when behavior, safety boundaries, verification, audit, packaging, or release behavior changes.

## Repository Hygiene

- Do not commit `.planning/`, `.plan-attestation`, `__pycache__/`, `*.pyc`, build outputs, backups, real saves, real Mod packages, tokens, cookies, API keys, or private paths.
- Test fixtures are artificial and minimal.
- Logs and screenshots are redacted.

## Review Output

- Findings first, ordered by severity.
- File/line references are tight and actionable.
- Open questions and assumptions are explicit.
- Summary and verification evidence are concise.
- No unsupported claims that tests passed.
