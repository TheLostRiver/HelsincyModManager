---
name: hmm-review-gate
description: Use only when reviewing HMM changes, preparing a PR or final handoff, handling review feedback, or evaluating merge readiness. Performs findings-first review, risk-proportionate verification checks, artifact and secret hygiene, comment triage, and required CI/merge gating without forcing a full local verification after every small fix.
---

# HMM Review Gate

Review the current diff, not a remembered earlier state. Governance changes under `.codex/`, `.agents/`, policy,
scripts, hooks, workflows, `AGENTS.md`, or core governance docs require explicit governance review.
Use `references/review-gate-checklist.md` when a detailed PR-readiness checklist is useful.

## Scope And Evidence

1. Inspect `git status --short --branch`, the complete PR diff, and any untracked files.
2. Classify touched boundaries using `hmm-feature-router`; load only the matching router references.
3. Confirm the PR delivers one user-visible vertical slice or closes one release blocker. Cross-layer changes are
   valid when they serve that slice and commits keep design, backend, adapters/UI, tests, and docs reviewable.
4. Check implementation, tests, contracts, docs, and task state against the requested behavior.
5. Check for `.planning/`, attestations, caches, generated output, backups, real Mod/save data, secrets, session logs,
   private paths, or unrelated user changes.
6. Report findings first, ordered by severity and grounded in file/line evidence.

Do not require a separate PR for docs sync, test movement, dead-code cleanup, file splitting, or an internal
prerequisite when it is part of the same vertical capability. Split only unrelated work, independently reversible
changes, materially expanded safety risk, or a diff too large to review coherently.

## Verification Gate

| Change class | Local evidence required |
| --- | --- |
| Low-risk docs or isolated internal/UI change | Focused checks matching the touched files. |
| Cross-layer behavior, public contract, task/event semantics | Focused checks plus one full `scripts/verify.ps1` on the PR candidate. |
| Install/save/security/concurrency or governance/CI | Focused positive/negative checks plus one full `scripts/verify.ps1` and full-diff self-review. |

After review feedback, rerun focused checks for the changed behavior. Repeat the full local run only if the fix
expands a high-risk boundary, changes a public contract/governance rule, changes dependencies/baseline, or makes
the prior full result inapplicable. Required CI must run on the final commit and reach terminal `success`.

## Review And Merge Gate

- Read every review, inline thread, and comment; classify it as a real bug, test/contract gap, maintainability issue,
  false positive, accepted risk, deferred item, or maintainer decision.
- Fix real bugs and add a regression test where practical. Resolve false positives only with source, test, contract,
  or command evidence; never dismiss feedback because the author is automated or because the first read looks fine.
- If CodeRabbit is absent, do one independent full-diff self-review and record the evidence; do not wait forever and
  do not treat absence as approval.
- Do not merge with unresolved Critical/Important findings, unresolved threads, missing required evidence, or a
  required check that is pending, failed, cancelled, timed out, skipped, neutral, or action-required.
- Prefer normal merge. Use `--admin` only with explicit authorization and only when every content, review, and CI gate
  is already satisfied; never use it to bypass a real gate.

## Finding Format

For each finding include severity, status, location, problem, risk, evidence, and the smallest sound fix. Use:

- `Critical`: player-data loss, unsafe real writes, secret disclosure, or dangerous broad filesystem capability.
- `Important`: architecture boundary break, missing high-risk coverage, or stale public contract.
- `Moderate`: maintainability or edge-coverage risk.
- `Minor`: wording, formatting, or low-risk polish.

If no findings remain, say so and still report checked scope, commands actually run, commands not run and why, and
residual risk. Never claim a command passed unless it completed successfully for the relevant commit.
