---
name: hmm-feature-router
description: Use when starting or scoping Helsincy Mod Manager work, especially feature requests, bug fixes, architecture decisions, frontend/backend boundary changes, Rust crate edits, Tauri commands, task/concurrency work, safety-sensitive flows, review gates, or choosing verification.
---

# HMM Feature Router

## Overview

Route HMM work before editing. Identify the module boundary, load only the relevant project docs, then choose the narrowest safe implementation and verification path.

Project-specific skills belong in this repository under `.codex/skills/`. Do not create or update HMM skills in global skill directories.

## Start Here

1. Read or scan the required baseline files before any edit: `AGENTS.md`, `README.md`, `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`, `CONTRIBUTING.md`, `docs/TESTING.md`, `docs/GOVERNANCE.md`, `SECURITY.md`.
2. Check `git status --short`; do not revert user or other agent changes.
3. Classify the task with the routing table below.
4. Open the referenced docs or skills for that class. References guide navigation; source docs and current code remain authoritative.
5. State the module boundary and verification plan before making non-trivial edits.

## Routing Table

| Task signal | Primary boundary | Read next | Hard stop |
| --- | --- | --- | --- |
| UI, page, component, CSS, route, local UI state, typed API wrapper | `src/` | `.codex/skills/hmm-frontend-workflow` if present, `docs/TESTING.md`, `.codex/skills/hmm-project-guardrails/references/frontend-backend-boundary.md` | Do not put file-system safety, install rules, MHW paths, retarget, backup, or rollback logic in the frontend. |
| Tauri command, DTO, app state, event bridge | `src-tauri/src/` | `docs/FRONTEND_BACKEND_CONTRACT.md`, `.codex/skills/hmm-tauri-command` if present | Do not expose broad file-system commands; commands must stay narrow use-case entry points. |
| Domain model, install plan, manifest, conflict, replacement target | `src-tauri/crates/hmm-core/` | `.codex/skills/hmm-rust-crate-boundary` if present, `docs/ARCHITECTURE.md`, `.codex/skills/hmm-project-guardrails/references/architecture-map.md` | Do not depend on Tauri, real file systems, databases, or MHW-specific path parsing. |
| Application use case, orchestration, task manager | `src-tauri/crates/hmm-app/` | `.codex/skills/hmm-rust-crate-boundary` if present, `.codex/skills/hmm-task-and-concurrency` when task/lock/event behavior is touched, `docs/ARCHITECTURE.md`, `docs/TESTING.md` | Do not bind directly to concrete infra; use ports/traits. |
| Real file I/O, config, Steam discovery, hash, staging, archive handling | `src-tauri/crates/hmm-infra/` | `.codex/skills/hmm-rust-crate-boundary` if present, `SECURITY.md`, `docs/TESTING.md`, `docs/LOGGING.md`, `.codex/skills/hmm-project-guardrails/references/safety-boundary.md` | Do not operate on real game/save paths in tests by default. |
| MHW:I adapter, `nativePC`, `plNNN_VVVV`, game-specific catalog/rules | `src-tauri/crates/hmm-games-mhw/` | `.codex/skills/hmm-rust-crate-boundary` if present, `docs/ARCHITECTURE.md`, `docs/TESTING.md` game adapter section, game adapter docs if relevant | Do not leak MHW rules into generic core or generic frontend; do not require a real game install for automated tests. |
| Install, uninstall, backup, rollback, overwrite, delete, path validation | Cross-crate high-risk flow | `.codex/skills/hmm-install-safety` if present, `SECURITY.md`, `docs/TESTING.md`, `docs/LOGGING.md` | Do not bypass `InstallPlan`, manifest, backup, rollback, staging, or path containment. |
| Long task, progress event, cancellation, lock, concurrency | `hmm-app`, `src-tauri/src/task_events.rs` | `.codex/skills/hmm-task-and-concurrency` if present, `docs/FRONTEND_BACKEND_CONTRACT.md`, `docs/TESTING.md` concurrency section, `docs/LOGGING.md` when events/logs/audit are touched | Do not hold game write locks during long hash/extract/analyze work; progress events need task id. |
| Review, final handoff, PR readiness, artifact audit | Cross-boundary review | `.codex/skills/hmm-review-gate` if present, `docs/TESTING.md`, `docs/GOVERNANCE.md` | Do not claim completion without current verification evidence or explicit omissions. |
| Governance, `.codex/`, `.agents/`, policy, scripts, hooks, CI | Governance files | `.codex/skills/hmm-review-gate` if present, `docs/GOVERNANCE.md`, `docs/MULTI_AGENT_COLLABORATION.md` | Treat as human-review work; do not write secrets, session logs, private local paths, player data, or real Mod content. |

## Verification Map

Prefer the smallest meaningful check from `docs/TESTING.md`; run full verification when scope crosses boundaries or risk is unclear.

| Change type | Minimum useful checks |
| --- | --- |
| Docs/governance/skills only | `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1` when feasible; otherwise relevant policy/link/secret checks and explain omissions. |
| Frontend only | `hmm-frontend-workflow` checks: frontend typecheck/lint/build through project scripts, plus boundary/browser smoke when relevant. |
| Tauri bridge | Rust tests/checks plus frontend typed wrapper checks; smoke command shape when possible. |
| Rust core/app/infra/game adapter | `hmm-rust-crate-boundary` checks: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, plus focused tests for the touched crate. |
| High-risk file/install/save/concurrency | Focused safety/task tests with temp fixtures, no real game/save directories, Audit Log/redaction checks when relevant, plus full verify when feasible. |
| Review/final handoff | `hmm-review-gate` checks: findings first when reviewing, current verification evidence, artifact hygiene, and explicit omissions. |

## Common Mistakes

- Treating `.codex/skills/` as runtime cache. It is versioned project governance.
- Updating global skills for HMM-specific rules. Keep HMM rules inside this repository.
- Reading only a reference summary when the source doc or current code is relevant.
- Letting frontend code decide file-system, install, backup, rollback, or game-adapter rules.
- Claiming a test passed without running it successfully in the current turn.
