# Task and Concurrency Checklist

Use this checklist for HMM task manager, event, cancellation, lock, queue, and concurrent workflow changes.

## Task Identity

- Start command returns a task id and stable kind/status.
- Every event carries `taskId`.
- Event name and payload match `docs/FRONTEND_BACKEND_CONTRACT.md`.
- Phase code is stable, documented, and not inferred from user-visible text.
- Large results are referenced, not embedded in progress events.

## Boundary Routing

- Rust crate placement, dependency direction, AppState services, repositories, or DTO/domain mapping changes also use `hmm-rust-crate-boundary`.
- Command/event DTO, custom protocol, or frontend/backend contract changes also use `hmm-tauri-command`.
- React task UI, frontend listeners, typed API wrappers, task state, or browser-visible workflow changes also use `hmm-frontend-workflow`.
- File write/delete/backup/restore/install/uninstall/rollback changes also use `hmm-install-safety`.

## Cancellation

- Queued/running/completed/failed/cancelled states are explicit.
- Cancellation has safe points and deterministic results.
- Unknown task and non-cancellable states return stable errors.
- Frontend listener reconciles events by `taskId`.

## Locks and Queues

- Same game instance writes serialize.
- Same profile enable/disable/install/uninstall operations serialize.
- Prepare work runs outside game write locks.
- Database write transactions stay short.
- Locks are released on error/cancel paths.

## Logging and Audit

- Task logs and progress share the same task id.
- User-visible messages do not include raw paths or sensitive content.
- Write/overwrite/delete/backup/restore/manifest/rollback operations emit Audit Log entries when those paths exist.
- `RollbackFailed` and `DataSafetyRisk` style failures are auditable.

## Verification

- Rust tests cover task id propagation and phase/status mapping.
- Rust task/concurrency changes run `cargo clippy --workspace --all-targets -- -D warnings` unless a documented reason prevents it.
- Concurrency tests use fake services or temp fixtures.
- No automated test requires real game directories, real saves, or third-party Mod packages.
- Bridge/frontend tests cover listener matching and `hmm-frontend-workflow` checks if UI task behavior changed.
