---
name: hmm-install-safety
description: Use for HMM work that can affect player data or real filesystem state, including Mod import, archive extraction, staging, path validation, install plans, game directory writes, overwrite/delete, manifest, backup, uninstall, rollback/recovery, save backup/restore, Steam save-account selection, audit logging, and their safety tests. Do not trigger for ordinary UI, read-only queries, or unrelated repository work.
---

# HMM Install Safety

Protect player data while keeping the workflow focused on the touched risk.

## Load Context

1. Read `AGENTS.md`, current implementation/tests, `SECURITY.md`, and the relevant `docs/TESTING.md` section.
2. Read `docs/LOGGING.md` when the flow writes, deletes, backs up, restores, or changes evidence health.
3. Read `references/install-safety-checklist.md`.
4. Use `../hmm-feature-router/references/safety-boundary.md` for the detailed project safety map.
5. Load router checklists for Tauri, Rust, task/concurrency, or frontend only when those boundaries are touched.

## Preserve The Safety Chain

```text
sealed input
  -> analyze / preflight
  -> InstallPlan
  -> backup
  -> commit
  -> manifest
  -> rollback / recovery
```

- Keep imported source packages read-only; write derived variants only to disposable staging.
- Reject parent traversal, absolute paths, links/junction escapes, case-insensitive collisions, suspicious types,
  archive bombs, and any target outside the approved root.
- Back up existing targets before overwrite. Drive uninstall from manifest/recovery facts, never package guesses.
- Keep commit short and serialized per game/profile. Do scan/hash/extract/analyze outside the write lock.
- Treat task/audit evidence failure as an explicit degraded result; never falsify player-file rollback.
- Keep save backups outside the game install directory. Record a backup manifest and validate source, target,
  selected Steam account/profile, game state, and confirmation before restore.
- Redact local paths, usernames, Steam IDs, tokens, save contents, and third-party Mod contents from logs and output.

## Stop Conditions

- Do not add a direct-copy, direct-delete, or direct-overwrite path outside the install executor.
- Do not expose a broad filesystem Tauri or CLI command.
- Do not implement real game writes before logging/redaction, task identity, manifest, backup, and recovery exist.
- Do not restore saves without validated containment, explicit confirmation, and a backup manifest.
- Do not use real game/save directories or third-party Mod packages in automated tests.
- Do not put MHW:I path grammar or retarget rules in generic core or frontend code.

## Focused Tests

Cover every changed risk with temp/fake fixtures:

- archive traversal, absolute path, link/junction, collision, size and containment rejection;
- final target normalization, conflict detection, stale plan/binding rejection, and preflight decisions;
- backup-before-overwrite, manifest contents, rollback/restart recovery, and unknown-file preservation;
- uninstall and true reinstall retained/replaced/added/stale behavior;
- save-account selection, backup manifest, unwritable destination, retention, restore validation and confirmation;
- cancellation safe points, per-game/profile serialization, and task id propagation;
- Audit Log coverage, redaction, and explicit evidence-health degradation.

Run focused tests during iteration. Because these paths are high risk, run one full `scripts/verify.ps1` on the PR
candidate and use `hmm-review-gate` before publishing. Small review fixes require another full run only when they
change the safety boundary or invalidate the previous result; required CI still validates the final commit.
