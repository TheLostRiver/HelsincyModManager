---
name: hmm-task-and-concurrency
description: Use when Helsincy Mod Manager work touches TaskManager, long-running tasks, task events, cancellation, progress phases, game/profile locks, queues, concurrent scanning/hash/extract/analyze work, or database/write serialization.
---

# HMM Task And Concurrency

## Overview

Keep heavy work explicit, cancellable, and traceable. Prepare work may run in parallel, but writes to the same game instance/profile must be serialized and progress must always carry task identity.

HMM-specific skills belong under `.codex/skills/` in this repository, not in global skill directories.

## Required Context

Before editing, read or scan:

- `AGENTS.md`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/ROADMAP.md`
- `CONTRIBUTING.md`
- `docs/TESTING.md`
- `docs/GOVERNANCE.md`
- `SECURITY.md`
- `docs/FRONTEND_BACKEND_CONTRACT.md`
- `docs/LOGGING.md`
- `.codex/skills/hmm-project-guardrails/references/safety-boundary.md`

If the task exposes or changes command/event DTOs, also use `hmm-tauri-command`. If it changes Rust crate placement, app/ports/infra dependency direction, AppState services, repositories, or DTO/domain mapping, also use `hmm-rust-crate-boundary`. If it changes React task UI, frontend listeners, typed API wrappers, task state, or browser-visible workflow, also use `hmm-frontend-workflow`. If it writes, deletes, backs up, restores, installs, uninstalls, or rolls back files, also use `hmm-install-safety`.

## Task Pattern

1. Start long work through a narrow command that returns `TaskStartedDto` or equivalent identity.
2. Emit progress through the documented `hmm://task-progress` contract with `taskId`, kind, status, phase, progress, error, and optional result reference.
3. Keep phase codes stable and registered in `docs/FRONTEND_BACKEND_CONTRACT.md`.
4. Fetch large final results by `resultRef` or query command, not by stuffing results into events.
5. Support cancellation at explicit safe points and keep cancellation state consistent.
6. Use structured logs with the same task id; do not put raw sensitive paths into messages.

## Concurrency Rules

| Work | Allowed shape |
| --- | --- |
| Scan, hash, archive inspect, sandbox extract, package analyze, dependency check, plan preview | Parallel where resources permit; cancellable; no game directory writes. |
| Same game instance write | Serialized through a game write queue/lock. |
| Same profile enable/disable/install/uninstall | Serialized through profile or game/profile coordination. |
| Database write transaction | Short, explicit, and not held across long I/O. |
| Commit/install/backup/restore | Revalidate before write, keep lock short, write audit data, preserve recovery path. |

Do not hold game write locks while extracting, hashing, scanning, analyzing, or building long-running plans.

## Hard Stops

- Do not match events by "only one task is active"; task identity must be explicit.
- Do not start real game writes before logging/redaction/task/audit foundations required by `docs/LOGGING.md` exist.
- Do not leave partial manifest or half-committed state after cancellation/failure without recovery/audit handling.
- Do not use global mutable task state without clear locking and tests.
- Do not let frontend infer task ownership from page state alone.

## Verification

Use `references/task-concurrency-checklist.md` for review. Minimum checks:

| Change | Minimum checks |
| --- | --- |
| Rust task manager/app/infra logic | `cargo test --workspace`, `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, plus focused task/concurrency tests. |
| Task identity/event DTO | `cargo test --workspace`, `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, serialization/phase tests, plus `hmm-tauri-command` checks when the bridge changes. |
| Cancellation | Tests for queued/running/unknown/already-finished task behavior and stable state after cancel. |
| Locks/queues | Tests proving same game/profile writes serialize and long prepare work happens outside write lock. |
| Install/save/write task | `hmm-install-safety` checks with temp/fake fixtures; Audit Log/redaction tests when relevant. |
| Frontend listener/task UI | `hmm-frontend-workflow` checks, frontend typecheck, and listener matching by `taskId`, not page-local assumptions. |

Prefer full `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1` before final handoff for task/concurrency changes.

## Common Mistakes

- Adding a progress event without stable phase codes.
- Letting cancellation flip UI state but not backend state.
- Holding a write lock while doing archive or hash work.
- Treating task logs as manifest or rollback state.
- Forgetting that failed install tasks must not leave half-written manifests.
