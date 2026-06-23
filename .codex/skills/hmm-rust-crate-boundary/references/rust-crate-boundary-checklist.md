# Rust Crate Boundary Checklist

Use this checklist for HMM Rust crate placement, dependency direction, and workspace work.

## Placement

- New type/function is in the narrowest correct crate.
- Domain concepts are pure and testable without Tauri, FS, DB, Steam, or platform APIs.
- Ports are traits/interfaces, not concrete infra leakage.
- App services orchestrate use cases through ports.
- Infra contains I/O details but not cross-cutting domain policy.
- Game-specific rules live in `hmm-games-*`.

## Dependency Direction

- `hmm-core` has no dependency on app/ports/infra/games/Tauri.
- `hmm-app` depends on ports and domain, not infra concretes.
- Tauri shell depends outward to app/state wiring and maps DTOs explicitly.
- Tauri/DTO bridge changes were checked against `docs/FRONTEND_BACKEND_CONTRACT.md`.
- New shared helpers do not invert dependencies for convenience.

## Game Adapter

- MHW rules stay in `hmm-games-mhw`.
- Future game logic does not branch inside MHW adapter.
- Automated tests do not require a real MHW install.
- Catalog/rule additions are data-driven where practical.

## Safety and Errors

- File writes still flow through plan/manifest/backup/rollback design.
- Errors preserve stable codes/categories for command mapping.
- Sensitive raw paths or player data do not cross into logs or UI DTOs.

## Tasks and Concurrency

- TaskManager, task events, cancellation, progress phases, locks, queues, and database/write serialization changes also satisfy `hmm-task-and-concurrency`.
- Long scan/hash/extract/analyze work stays outside game write locks.
- Same game instance writes and same profile enable/disable/install/uninstall paths are serialized.
- Progress and task logs carry explicit task identity.

## Verification

- `cargo test --workspace` ran for Rust changes.
- `cargo clippy --workspace --all-targets -- -D warnings` ran for core/app/ports/infra/game changes, or omission is explained.
- Tauri bridge changes also ran `cargo check --workspace`.
- Task/concurrency changes ran focused checks for task identity, phase codes, cancellation state, lock/queue ordering, or database write serialization as touched.
- Architecture docs were checked when boundaries changed.
