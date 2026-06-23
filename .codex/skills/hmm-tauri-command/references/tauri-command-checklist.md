# Tauri Command Checklist

Use this checklist when adding or changing HMM Tauri commands, DTOs, task events, custom protocols, or frontend typed APIs.

## Boundary

- Command name describes a use case, not a raw filesystem primitive.
- Command body validates input, maps DTOs, calls `AppState`, and returns DTO/errors.
- Domain decisions live in `hmm-core`/`hmm-app`; real I/O lives behind `hmm-ports` and `hmm-infra`.
- MHW-specific parsing lives in `hmm-games-mhw`, not in command code or generic frontend code.

## DTO

- Rust structs crossing Tauri use `#[serde(rename_all = "camelCase")]`.
- Rust enum values crossing Tauri use stable strings, usually `snake_case`.
- TypeScript DTO types match the actual JSON shape.
- DTOs expose display labels, ids, summaries, or controlled URLs instead of raw sensitive paths.
- `metadata` is display/context data; frontend must not derive install paths from it.

## Errors

- Error `code` is stable and suitable for frontend tests.
- User-visible `message` is not used for logic.
- Errors avoid full local paths and sensitive data.
- High-risk commands include enough category/recoverability detail when the contract has those fields.

## Events and Long Tasks

- Starting command returns `TaskStartedDto` or equivalent controlled identity.
- Progress events use the documented `hmm://task-progress` contract.
- Every progress event carries `taskId`, stable kind/status values, and a registered phase.
- Phase codes use the documented `<task_kind>.<stage>.<sub>` style and are registered in `docs/FRONTEND_BACKEND_CONTRACT.md`.
- Large final results are fetched by result reference or query command, not stuffed into progress events.
- Cancellation and result lookup follow the documented task contract instead of page-local assumptions.

## Custom Protocols

- Thumbnail/resource URLs are controlled protocol URLs backed by opaque refs, never raw disk paths.
- Handlers validate cache/storage root containment and reject traversal, absolute paths, symlinks, and unregistered refs.
- Handlers set accurate `Content-Type` and cache behavior.
- DTOs, logs, and frontend code do not expose real cache paths or thumbnail file extensions.

## Frontend

- Add or update a feature-local API wrapper.
- Use the shared invoke helper only for common mechanics.
- Do not use `convertFileSrc`, asset protocol, raw cache paths, or arbitrary local path reads unless the contract explicitly allows it.
- View models map DTOs to UI state without recreating backend rules.

## Verification

- Parser rejects empty/relative/invalid inputs when paths are accepted.
- DTO serialization or source tests cover shape and command names.
- Frontend tests cover wrapper command names and forbidden APIs for sensitive flows.
- Tauri/Rust bridge changes run at least `cargo test --workspace` and `cargo check --workspace`, or the final handoff states why they could not run.
- Contract, governance, or `.codex/` changes run the project verification script when feasible and call out that human review is expected.
