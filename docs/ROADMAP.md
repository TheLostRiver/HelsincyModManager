# Roadmap

## Phase 0: Architecture Baseline

- Initialize repository.
- Document architecture, module boundaries, and MVP scope.
- Decide first implementation stack.
- Define first game adapter target: Monster Hunter: World - Iceborne.

## Phase 1: Project Scaffold

- Create Tauri 2 application scaffold.
- Add React + TypeScript frontend.
- Create Rust workspace crates.
- Add formatting, linting, and basic CI.
- Add initial SQLite migration structure.

## Phase 2: MHW:I MVP Core

- Add game directory detection.
- Add manual game directory selection.
- Add archive inspection and sandbox extraction.
- Add package analyzer for `nativePC`, DLL, image, and readme detection.
- Add safe preview image extraction.
- Add category and tag storage.
- Add install plan generation.
- Add install executor with manifest and rollback baseline.
- Add basic conflict detection.
- Add manual save backup.
- Add one-click launch.

## Phase 3: Player Workflow

- Add profile support.
- Add dependency rule catalog.
- Add missing prerequisite warnings.
- Add automatic save backup scheduling.
- Add mod enable/disable batch operations.
- Add task progress and cancellation UI.

## Phase 4: Replacement Mapping

- Add official target catalogs for MHW:I.
- Add armor replacement mapping.
- Add weapon replacement mapping.
- Add voice replacement mapping.
- Add binding-aware conflict detection.
- Add retarget staging workflow.

## Phase 5: Cross-Platform Preparation

- Add Linux path abstractions.
- Add Steam library scanning on Linux.
- Package Linux builds.
- Run community tests for Steam Deck Desktop Mode.

## Phase 6: More Games

- Add Monster Hunter Rise adapter.
- Add Monster Hunter Wilds adapter when modding patterns are understood.
- Extract shared Monster Hunter adapter utilities.
