---
name: hmm-install-safety
description: Use when Helsincy Mod Manager work touches Mod import, archive extraction, staging, path validation, install plans, game directory writes, overwrite/delete, manifest, backup, uninstall, rollback, save backup, audit logging, or data-safety tests.
---

# HMM Install Safety

## Overview

Protect player data first. Any flow that can write, overwrite, delete, back up, restore, or infer game/save paths must preserve the `InstallPlan -> backup -> commit -> manifest -> rollback/recover` chain.

HMM-specific skills and safety rules live in this repository under `.codex/skills/`, never in a global skill directory.

## Required Context

Before editing, read or scan:

- `AGENTS.md`
- `README.md`
- `SECURITY.md`
- `docs/ARCHITECTURE.md`
- `docs/ROADMAP.md`
- `CONTRIBUTING.md`
- `docs/TESTING.md`
- `docs/GOVERNANCE.md`
- `docs/LOGGING.md`
- `.codex/skills/hmm-project-guardrails/references/safety-boundary.md`

If the task includes retarget or MHW paths, also read the current retarget design docs before touching code.

## Boundary Routing

- If safety work changes or exposes Tauri commands, DTOs, AppState wiring, task events, custom protocols, command errors, or frontend/backend contract shape, also use `hmm-tauri-command`.
- If it changes Rust crate placement, app/ports/infra dependency direction, repositories, game adapters, domain/app mapping, or DTO/domain conversion, also use `hmm-rust-crate-boundary`.
- If it changes TaskManager, long-running tasks, cancellation, progress phases, game/profile locks, queues, concurrent scan/hash/extract/analyze work, or database/write serialization, also use `hmm-task-and-concurrency`.
- If it changes React UI, frontend state, task listeners, typed API wrappers, thumbnail/resource display, accessibility, responsive behavior, or browser-visible workflow, also use `hmm-frontend-workflow`.

## Safety Workflow

1. Classify whether the change affects import-only, staging-only, plan-building, real game writes, uninstall, save backup, logging, or concurrency.
2. Keep original imported Mod packages read-only. Derived variants belong in sandbox/cache/staging and must be disposable.
3. Reject unsafe archive entries before extraction: parent traversal, absolute paths, links/junction traps, suspicious file types, archive bombs, and case-insensitive collisions.
4. Generate or consume an `InstallPlan` before any real game directory write.
5. Back up existing files before overwrite; write manifest after commit; uninstall from manifest, not from package guesses.
6. On failure, roll back as far as possible and leave recoverable state plus an Audit Log event.
7. For save backup or restore, keep the default backup directory outside the game install directory, write a backup manifest, validate before restore, require restore confirmation, and preserve configurable interval/retention behavior.
8. Use temp fixtures, fake file systems, or artificial minimal packages in tests. Do not require a real MHW install, real save directory, or third-party Mod package.

## Hard Stops

- Do not copy, remove, rename, or overwrite files in a game directory outside the install executor path.
- Do not implement real game directory writes before logging/telemetry, `task_id` propagation, redaction helpers, log directory resolution, Audit Log writing, and related tests exist.
- Do not restore saves without prior validation, explicit confirmation, and a backup manifest.
- Do not let frontend or Tauri command code compute final install paths or replacement targets.
- Do not hold the game write lock while extracting, hashing, scanning, analyzing, or building long-running plans.
- Do not log full local paths, usernames, Steam IDs, tokens, cookies, real save contents, or third-party Mod contents.
- Do not treat staging as a source of truth. Source of truth is imported package metadata, bindings/configuration, and manifest.

## Minimum Tests

Use `references/install-safety-checklist.md` as a focused checklist. At minimum, cover the changed risk:

| Risk | Required test shape |
| --- | --- |
| Archive path safety | Parent traversal, absolute path, case collision, and sandbox containment with artificial archives or entry fixtures. |
| Staging | Relative target normalization and escape rejection; assert output remains under staging root. |
| Install plan | Conflict detection by final target path; no direct write before plan exists. |
| Overwrite/delete | Backup before overwrite; manifest records changes; rollback restores temp game directory. |
| Uninstall | Manifest-driven removal; unknown files preserved. |
| Save backup/restore | Default backup outside game install, custom backup directory, backup manifest, restore validation and confirmation, retention limits, unwritable backup directory. |
| Concurrency | Same game/profile writes serialize; long analysis work happens outside write lock; progress carries task id. |
| Logging | Audit Log for write/overwrite/delete/backup/restore/manifest/rollback; sensitive paths are redacted. |

Run the smallest meaningful commands from `docs/TESTING.md`, then prefer full `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1` before final handoff when feasible.

## Common Mistakes

- Building a convenient direct-copy path because install MVP is incomplete.
- Testing with a local real game folder because it is faster.
- Treating current package contents as uninstall truth after installation.
- Letting `nativePC` or MHW slot parsing leak into generic core or frontend code.
- Reporting "safe" without a focused test or a clear reason why testing was not possible.
