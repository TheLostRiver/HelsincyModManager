---
name: hmm-rust-crate-boundary
description: Use when Helsincy Mod Manager work touches Rust crates, module placement, dependency direction, domain/app/ports/infra boundaries, game adapters, Rust DTO/domain mapping, or workspace verification.
---

# HMM Rust Crate Boundary

## Overview

Keep Rust responsibilities separated by dependency direction. Domain rules stay pure, app services depend on ports, infra implements ports, game-specific rules stay in game adapters, and Tauri remains a thin shell.

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
- `.codex/skills/hmm-project-guardrails/references/architecture-map.md`

If the change touches Tauri commands, DTOs, command errors, task events, custom protocols, or frontend/backend contract shape, also read `docs/FRONTEND_BACKEND_CONTRACT.md` and use `hmm-tauri-command`.

If it touches `TaskManager`, long-running tasks, task events, cancellation, progress phases, game/profile locks, queues, concurrent scan/hash/extract/analyze work, or database/write serialization, also use `hmm-task-and-concurrency`.

If it touches real file operations, install, backup, rollback, staging, saves, audit logging, or data safety, also use `hmm-install-safety`.

## Crate Placement

| Boundary | Put here | Do not put here |
| --- | --- | --- |
| `hmm-core` | Pure domain types/rules: Game, Profile, InstallPlan, Manifest, Conflict, ReplacementTarget | Tauri, SQLite, real FS, Steam, MHW path parsing |
| `hmm-ports` | Traits/interfaces used by app services | Concrete infra, UI DTOs, game-specific parsing |
| `hmm-app` | Use-case orchestration and workflows through ports | Concrete file/database/platform APIs |
| `hmm-infra` | Real filesystem, config/SQLite, archive/hash, Steam/platform integration | Domain decisions that belong in core/app |
| `hmm-games-mhw` | MHW:I directory rules, `nativePC`, loader/DLL, catalog, resource parsing | Rise/Wilds branches or generic install flow |
| `src-tauri/src` | Tauri command registration, state wiring, DTOs, event/protocol boundary | Business rules, adapter parsing, direct install execution |

Only create future game crates when real adapter work lands; avoid empty abstractions.

## Boundary Rules

1. Move shared concepts inward only when they are game-independent and stable.
2. Keep game-specific parsing/catalog/rules inside the matching `hmm-games-*` crate.
3. Make app services depend on traits from `hmm-ports`, not concrete infra structs.
4. Map between DTOs and domain types at the Tauri/app boundary; do not let DTOs replace domain models.
5. Use fake ports, temp dirs, and artificial fixtures in tests; do not require real game installs.
6. Preserve recoverable error information across layers without leaking sensitive paths.

## Hard Stops

- Do not add MHW slot/resource/path parsing to `hmm-core`, `hmm-app`, or generic frontend code.
- Do not make `hmm-app` depend on `hmm-infra` concrete types.
- Do not implement file copies/deletes in Tauri commands or core domain logic.
- Do not add Rise/Wilds logic to `hmm-games-mhw`.
- Do not bypass `InstallPlan`, manifest, backup, rollback, or adapter registration to move faster.

## Verification

Use `references/rust-crate-boundary-checklist.md` for review. Minimum checks:

| Change | Minimum checks |
| --- | --- |
| Rust core/app/ports/infra/game adapter | `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`. |
| Tauri shell/DTO bridge | Read `docs/FRONTEND_BACKEND_CONTRACT.md`; run `cargo test --workspace`, `cargo check --workspace`, plus `hmm-tauri-command` checks. |
| Task/concurrency boundary | `hmm-task-and-concurrency` checks plus focused tests for task identity, phase codes, cancellation, locks/queues, or database write serialization as touched. |
| Game adapter | Focused adapter tests with fake/temp paths; no real game install required. |
| File/install/save/audit | `hmm-install-safety` checks and temp/fake fixtures. |
| Dependency boundary refactor | Inspect crate imports and update docs if architecture boundary changed. |

Prefer full `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1` before final handoff when crate work crosses frontend/Tauri/Rust or governance boundaries.

## Common Mistakes

- Adding a "small helper" in core that secretly encodes MHW path rules.
- Passing concrete infra into app services because tests are easier.
- Treating a DTO enum as the domain model.
- Creating future game crates before there is real code to put there.
- Changing crate boundaries without updating architecture docs or tests.
