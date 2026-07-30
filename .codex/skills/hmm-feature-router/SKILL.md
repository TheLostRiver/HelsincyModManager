---
name: hmm-feature-router
description: Primary HMM repository entry point. Use when starting or scoping Helsincy Mod Manager feature, bug-fix, architecture, frontend, Tauri, Rust, task/concurrency, game-adapter, governance, or verification work to identify touched boundaries, load only relevant project references, and choose risk-proportionate checks. Use hmm-install-safety additionally only for player-data or real filesystem write paths, and hmm-review-gate only for review or PR readiness.
---

# HMM Feature Router

Route HMM work without loading every project rule into context.

## Workflow

1. Read `AGENTS.md`, inspect `git status --short`, and inspect the current source, tests, and task document.
2. Define one user-visible vertical slice or release blocker and list the boundaries it touches.
3. Load only the matching references below. Current source and formal docs remain authoritative.
4. Invoke `hmm-install-safety` only when the slice can affect player data, archives, game/save paths, or real writes.
5. Run focused checks while iterating. Use the risk rules below to decide when full verification is required.
6. Invoke `hmm-review-gate` when reviewing, preparing a PR, handling review fixes, or evaluating merge readiness.

Do not load a project reference merely because it exists. Do not recreate separate frontend, Rust, Tauri, or
concurrency skills; their checklists live here for progressive disclosure.

## Boundary Routing

| Signal | Read when relevant |
| --- | --- |
| Product scope or feature-specific docs | `references/feature-doc-index.md` |
| React UI, CSS, state, accessibility, responsive behavior | `references/frontend-workflow-checklist.md` |
| Frontend/backend ownership or typed API shape | `references/frontend-backend-boundary.md` and `docs/FRONTEND_BACKEND_CONTRACT.md` |
| Tauri command, DTO, event, AppState wiring | `references/tauri-command-checklist.md` |
| Rust crate placement or dependency direction | `references/architecture-map.md` and `references/rust-crate-boundary-checklist.md` |
| TaskManager, cancellation, progress, locks, queues | `references/task-concurrency-checklist.md`; add `docs/LOGGING.md` for events or evidence |
| MHW:I adapter, catalog, path grammar, retargeting | `references/architecture-map.md`, the matching feature doc, and current adapter code |
| Archive, staging, install, uninstall, rollback, save backup/restore | `hmm-install-safety` and `references/safety-boundary.md` |
| Governance or multi-agent work | `docs/GOVERNANCE.md` and, only when agents collaborate, `references/multi-agent-workflow.md` |
| Selecting focused checks | `references/testing-map.md` and the relevant section of `docs/TESTING.md` |

## Non-Negotiable Boundaries

- Keep filesystem, install, backup, rollback, and game-adapter rules out of React.
- Keep Tauri commands narrow: validate DTOs, map errors, and call application services.
- Keep `hmm-core` independent of Tauri, concrete infrastructure, databases, and MHW:I path parsing.
- Keep game-specific catalog and path grammar in the game adapter.
- Keep long scan/hash/extract/analyze work outside game write locks; serialize writes per game/profile.
- Preserve `InstallPlan`, backup, manifest, rollback/recovery, staging containment, and task/audit identity.
- Never test against real game directories, real saves, or third-party Mod packages by default.

## Verification By Risk

| Risk | During implementation | Before PR ready |
| --- | --- | --- |
| Low: docs, local refactor, isolated UI | Run the smallest relevant policy, link, unit, type, or browser check. | Focused checks are normally sufficient; required CI remains mandatory. |
| Medium: cross-layer behavior, public DTO, task/event contract | Run focused tests for every touched boundary. | Run one full `scripts/verify.ps1` on the PR candidate. |
| High: install/save writes, rollback, concurrency, security, governance/CI | Run focused positive and negative tests with temp/fake fixtures. | Run one full `scripts/verify.ps1` and a findings-first self-review. |

After a small review fix, rerun the affected focused checks. Repeat full local verification only when the fix
changes a high-risk boundary, public contract, governance rule, dependency baseline, or invalidates the previous
full result. Required remote CI must still reach terminal success for the final commit.
