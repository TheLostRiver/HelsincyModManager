---
name: hmm-frontend-workflow
description: Use when Helsincy Mod Manager work touches React UI, feature pages, components, CSS, appearance system, frontend state, typed API wrappers, accessibility, responsive behavior, or browser smoke tests.
---

# HMM Frontend Workflow

## Overview

Keep the frontend focused on presentation, interaction, view models, and typed command calls. File-system safety, game rules, install logic, retargeting, backup, and rollback stay behind Tauri/app/Rust boundaries.

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
- `.codex/skills/hmm-project-guardrails/references/frontend-backend-boundary.md`

For appearance, shell, Dashboard, or sidebar work, also read:

- `docs/APPEARANCE_SYSTEM.md`
- `docs/APPEARANCE_EXTENSION_GUIDE.md`
- `docs/DASHBOARD_V2_SIDEBAR_MODES.md` when Dashboard/sidebar behavior is touched.

For typed API wrappers, command results, command errors, task events, thumbnail URLs, custom protocols, or DTO shape changes, also read `docs/FRONTEND_BACKEND_CONTRACT.md` and use `hmm-tauri-command`.

## Frontend Boundaries

1. Put feature UI under `src/features/<feature>/`; shared reusable UI belongs under `src/shared/`; app shell and navigation belong under `src/app/`.
2. Keep business pages game-agnostic by default. Use backend-provided capabilities, ids, labels, and catalogs; do not branch on game paths or MHW resource names in generic UI.
3. Use feature-local typed API wrappers. Keep `src/shared/api/tauri.ts` as common invoke helper/re-export, not a dumping ground for every feature call.
4. Map DTOs into view models for display only. Do not reconstruct install paths, retarget paths, backup paths, conflict rules, or adapter decisions in TypeScript.
5. Use semantic tokens from `src/shared/styles/tokens.css` and local component CSS namespaces. Avoid broad hard-coded color/spacing systems.
6. Keep navigation definitions single-sourced. Shell variants decide layout; feature pages must not duplicate themselves per shell/sidebar mode.
7. Build accessible states: keyboard focus, labels/tooltips for icon-only controls, visible disabled states, and text that fits at desktop and mobile widths.

## Hard Stops

- Do not let frontend code read arbitrary local paths, call broad filesystem APIs, or use `convertFileSrc`/asset protocol/raw cache paths unless the contract explicitly allows it.
- Do not put Mod install, archive extraction, staging, path validation, backup, rollback, manifest, dependency, or game adapter rules in UI components.
- Do not make Dashboard or business pages depend on `sidebarMode` / `useSidebarMode` or branch CSS through `[data-sidebar-mode]`.
- Do not duplicate whole pages for shell variants, themes, games, or density modes.
- Do not introduce new frontend dependencies or mutate lockfiles without a clear reason and user-visible scope.

## Verification

Use `references/frontend-workflow-checklist.md` for review. Minimum checks:

| Change | Minimum checks |
| --- | --- |
| Component/CSS/page | `cmd /c corepack pnpm run typecheck`, `cmd /c corepack pnpm run lint`, `cmd /c corepack pnpm run build`. |
| UI workflow/state behavior | Frontend checks plus `cmd /c corepack pnpm run test` when behavior, state, routing, or helper tests are relevant; record why if omitted. |
| App shell/sidebar/Dashboard | Frontend checks plus `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1`. |
| Typed API wrapper | Read `docs/FRONTEND_BACKEND_CONTRACT.md`; run frontend typecheck plus focused test/source check for command name, DTO shape, stable error/kind/status/phase values, contract docs, and forbidden path APIs. |
| Visual workflow | Browser or manual smoke at relevant states and widths; for shell/sidebar/Dashboard visual baselines include `1440x900`, `1366x768`, and `1280x800`; record what was checked and what was not. |
| Crosses into Tauri/Rust | Also use `hmm-tauri-command` and run bridge checks from `docs/TESTING.md`. |

Prefer full `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1` before final handoff when frontend changes cross boundaries or risk is unclear.

## Common Mistakes

- Making a page pretty by copying layout instead of extending shell/appearance primitives.
- Letting TypeScript infer install behavior from a display label or `metadata`.
- Adding feature-specific API calls into a shared barrel.
- Claiming visual correctness without checking the relevant viewport/state.
- Fixing frontend symptoms by weakening governance, lint, boundary, or contract checks.
