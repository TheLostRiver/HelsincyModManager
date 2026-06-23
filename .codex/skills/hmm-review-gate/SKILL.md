---
name: hmm-review-gate
description: Use when reviewing Helsincy Mod Manager changes, preparing final handoff or PR readiness, checking governance edits, auditing tests, scanning for forbidden artifacts, or validating safety and architecture boundaries before completion.
---

# HMM Review Gate

## Overview

Review HMM changes before handoff with findings first, verified evidence, and project-specific safety gates. Treat `.codex/`, `.agents/`, policy, scripts, hooks, workflows, and core docs as governance changes that need human review.

HMM-specific skills belong under `.codex/skills/` in this repository, not in global skill directories.

## Required Context

Before reviewing, read or scan:

- `AGENTS.md`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/ROADMAP.md`
- `CONTRIBUTING.md`
- `docs/TESTING.md`
- `docs/GOVERNANCE.md`
- `SECURITY.md`
- `docs/MULTI_AGENT_COLLABORATION.md` when multi-agent work is involved.

Use boundary skills based on touched files and behavior, not only high-risk work:

- Frontend UI, state, CSS, typed API wrappers, accessibility, responsive behavior, or browser smoke: `hmm-frontend-workflow`.
- Tauri commands, DTOs, command errors, task events, custom protocols, or frontend/backend contract shape: read `docs/FRONTEND_BACKEND_CONTRACT.md` and use `hmm-tauri-command`.
- Rust crate placement, dependency direction, app/ports/infra boundaries, game adapters, or DTO/domain mapping: `hmm-rust-crate-boundary`.
- TaskManager, long-running tasks, cancellation, progress phases, queues, locks, or database/write serialization: `hmm-task-and-concurrency`.
- Mod import, archive extraction, staging, path validation, game writes, overwrite/delete, manifest, backup, uninstall, rollback, save backup, audit logging, diagnostics, or data-safety flow: read `docs/LOGGING.md` and use `hmm-install-safety`.

## Review Order

1. Check `git status --short --branch` and identify unrelated/user changes. Do not revert them.
2. Classify changed files by boundary: frontend, Tauri, Rust crate, safety flow, task/concurrency, docs/governance, generated/runtime artifacts.
3. Lead with findings ordered by severity. Include file and line references where possible.
4. Check that tests/verification match the touched boundary and were actually run.
5. Check docs/contract updates when architecture, command DTOs, error codes, task phase codes, typed API wrappers, custom protocols, safety rules, user settings, logging/audit behavior, or game adapter behavior changed.
6. Check repository hygiene: no `.planning/`, `.plan-attestation`, `__pycache__/`, `*.pyc`, dist/cache/backup outputs, real Mod/save data, tokens, cookies, API keys, private paths, or session logs.
7. End with concise summary, executed checks, omitted checks with reasons, and residual risk.

## Severity Guide

| Severity | Use for |
| --- | --- |
| Critical | Player data loss, real game/save writes without safety chain, secret leakage, dangerous filesystem command exposure. |
| Important | Architecture boundary break, missing required tests for high-risk code, stale contract/docs for public DTO/event changes. |
| Moderate | Maintainability risk, oversized/mixed-responsibility files, incomplete edge coverage, unclear errors. |
| Minor | Typos, small docs gaps, formatting, low-risk polish. |

If there are no findings, say so clearly and still mention unverified areas or residual risk.

## Hard Stops

- Do not mark work complete if required verification failed or was not run without explanation.
- Do not approve `.codex/`, `.agents/`, policy, script, hook, workflow, or core doc changes without calling out governance review.
- Do not ignore generated/runtime artifacts just because they are untracked.
- Do not let frontend, Tauri command, or generic core code take over install/safety/game-adapter rules.
- Do not claim tests passed from a previous turn unless they were run successfully in the current relevant context.

## Verification

Use `references/review-gate-checklist.md` for detailed review. Prefer:

```powershell
git status --short --branch
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Add boundary-specific commands from `docs/TESTING.md`. If any check is not run, final handoff must say why.

## Common Mistakes

- Writing only a summary and burying defects.
- Reviewing from memory instead of current files.
- Treating governance skill edits as ordinary docs.
- Letting "untracked" mean "irrelevant".
- Saying "looks good" without naming verification evidence.
