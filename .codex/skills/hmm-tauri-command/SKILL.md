---
name: hmm-tauri-command
description: Use when Helsincy Mod Manager work adds or changes Tauri commands, Rust DTOs, AppState wiring, task events, custom protocols, frontend typed API wrappers, command errors, or frontend/backend contract documentation.
---

# HMM Tauri Command

## Overview

Keep the Tauri boundary thin. Commands validate input shape, convert DTOs, call `AppState` services, return stable errors, and emit controlled events; domain rules and file-system safety stay behind app/ports/infra.

HMM-specific skills belong under `.codex/skills/` in this repository, not in global skill directories.

## Required Context

Before editing, read or scan the project baseline:

- `AGENTS.md`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/ROADMAP.md`
- `CONTRIBUTING.md`
- `docs/TESTING.md`
- `docs/GOVERNANCE.md`
- `SECURITY.md`

Then read the Tauri boundary context:

- `docs/FRONTEND_BACKEND_CONTRACT.md`
- `.codex/skills/hmm-project-guardrails/references/frontend-backend-boundary.md`

If the command touches real file operations, import, install, backup, rollback, preview-image cache, logs, diagnostics, audit events, redaction, or save data, also read `docs/LOGGING.md` and use `hmm-install-safety`.

## Boundary Routing

- If the change adds or updates feature-local typed API wrappers, React task listeners, frontend state, thumbnail display, or browser-visible workflow, also use `hmm-frontend-workflow`.
- If it changes AppState services, Rust crate placement, app/ports/infra dependency direction, repositories, game adapters, or DTO/domain mapping, also use `hmm-rust-crate-boundary`.
- If it changes task identity, task events, long-running work, cancellation, progress phases, result refs, locks, queues, or database/write serialization, also use `hmm-task-and-concurrency`.
- If it changes real file operations, import, install, backup, rollback, logs, diagnostics, audit events, redaction, save data, preview-image cache containment, or any data-safety flow, also use `hmm-install-safety`.

## Command Shape

1. Name commands in `snake_case` by use case, not raw filesystem operation.
2. Put Rust DTOs in `src-tauri/src/` and use `#[serde(rename_all = "camelCase")]` for structs crossing the boundary.
3. Use stable `snake_case` enum/error values for frontend branching.
4. Keep command bodies small: parse/validate input, call an app service from `State<'_, AppState>`, map output/error DTOs.
5. Add or update feature-local frontend typed API wrappers; keep `src/shared/api/tauri.ts` as common helper/re-export, not a feature dumping ground.
6. For long tasks, return a controlled task identity and emit `hmm://task-progress` events with `taskId`, stable `status`, registered `phase` codes, and `resultRef` or query commands for final results; do not put huge results or raw paths into progress events.
7. Update `docs/FRONTEND_BACKEND_CONTRACT.md` when command names, DTO shapes, error codes, phase codes, or typed API contracts change.
8. For custom protocols such as thumbnails, expose only controlled URLs backed by opaque refs; handlers must stay inside controlled app cache/storage roots, reject traversal/absolute/symlink access, set content type, and keep real disk paths out of DTOs, logs, and frontend code.

## Forbidden Shapes

- `copy_file`, `delete_path`, `read_any_file`, `write_any_file`, or other broad filesystem commands.
- Frontend-submitted final install paths, retargeted `nativePC` paths, cache paths, thumbnail disk paths, or game adapter internals.
- Commands that directly implement install, backup, rollback, game directory probing, or MHW parsing instead of calling app services.
- User-visible error messages containing full local paths, usernames, Steam IDs, tokens, cookies, real save content, or third-party Mod content.
- Event matching based on "only one task is active"; task identity must be explicit.
- `convertFileSrc`, asset protocol, base64 data URLs, or raw cache paths as the formal thumbnail/resource contract unless `docs/FRONTEND_BACKEND_CONTRACT.md` is explicitly updated first.

## Verification

Use `references/tauri-command-checklist.md` for review. Minimum checks depend on scope:

| Change | Minimum checks |
| --- | --- |
| Command parser/DTO only | `cargo test --workspace` and `cargo check --workspace`, plus Rust unit tests for validation and DTO serialization shape where practical. |
| Frontend typed API | Frontend typecheck plus focused tests that wrappers call the expected command and avoid forbidden path APIs. |
| Event/long task | `cargo test --workspace` and `cargo check --workspace`, Rust tests for `taskId`, kind/status/phase mapping, and frontend listener behavior if touched. |
| File/safety command | `hmm-install-safety` checks plus bridge tests; no real game/save directories. |
| Custom protocol | Handler tests for opaque refs, content type, cache-root containment, and traversal/absolute/symlink rejection; no real cache paths in DTOs/logs/frontend. |
| Contract change | Update docs and run project verification. |

Prefer full `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1` before final handoff, especially when the command spans frontend and Rust. If any minimum check is not run, state exactly why in the final handoff.

## Common Mistakes

- Letting command code become an application service.
- Returning convenient raw paths because the UI needs to display something.
- Adding one more feature call to a shared API barrel instead of a feature-local wrapper.
- Treating `message` text as frontend logic instead of stable `code` values.
- Forgetting to register task phase codes in the contract doc.
- Serving thumbnails through ad hoc file URLs instead of the documented custom protocol contract.
