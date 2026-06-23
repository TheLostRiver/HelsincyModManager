# Frontend Workflow Checklist

Use this checklist for HMM React UI, CSS, appearance, page, and typed API work.

## Structure

- File lives in the expected boundary: `src/app`, `src/features`, or `src/shared`.
- Component owns presentation and local UI state only.
- Feature state and view models consume typed DTOs without recreating backend rules.
- New files do not turn one component/page into a mixed UI/state/API/rules file.

## API Boundary

- Feature-local wrapper exists for feature command calls.
- `docs/FRONTEND_BACKEND_CONTRACT.md` was checked for command names, DTO shape, errors, task events, phase codes, thumbnail/custom protocol rules, and required doc updates.
- Shared Tauri helper only provides common invoke mechanics.
- DTO field names match actual camelCase contract.
- UI does not submit final install paths, cache paths, thumbnail disk paths, or retargeted `nativePC` paths.
- `message` text is display-only; branching uses stable code/kind/status/phase values.

## Appearance and Layout

- Uses semantic CSS variables/tokens instead of broad hard-coded palettes.
- Component CSS has a local namespace.
- Shell/sidebar/theme/density variants do not duplicate business pages.
- Navigation source remains single.
- Text fits inside controls/cards across relevant widths.
- Icon-only controls have accessible names or tooltips.
- Shell/sidebar/Dashboard visual baselines cover `1440x900`, `1366x768`, and `1280x800` when those areas are touched.

## Safety

- No filesystem safety, install, backup, rollback, manifest, conflict, MHW path, or save logic in frontend.
- No `convertFileSrc`, asset protocol, raw cache path, or arbitrary local path read unless contract explicitly permits it.
- No real player paths, Steam IDs, Mod contents, save contents, tokens, cookies, or API keys in fixtures, logs, docs, or screenshots.

## Verification

- Typecheck, lint, and build ran, or final handoff states why not.
- UI workflow/state/helper changes ran `cmd /c corepack pnpm run test` when relevant, or final handoff states why not.
- App shell/sidebar/Dashboard work ran `scripts/check-frontend-boundaries.ps1`.
- Visual changes were checked in relevant states/viewports and findings are recorded.
- Cross-boundary changes also satisfy `hmm-tauri-command` checks.
