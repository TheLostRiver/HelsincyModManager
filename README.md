# Helsincy Mod Manager

Helsincy Mod Manager is a planned cross-platform desktop mod manager for Monster Hunter games, starting with Monster Hunter: World - Iceborne on PC.

The project is currently in the architecture and planning stage. The intended stack is:

- Desktop shell: Tauri 2
- Frontend: React + TypeScript
- Backend core: Rust
- Storage: SQLite
- First supported game: Monster Hunter: World - Iceborne
- Future targets: Monster Hunter Rise, Monster Hunter Wilds, Linux / Steam Deck experimental support

## Design Goals

- Keep mod installation safe, reversible, and traceable.
- Treat game support as adapters, not hard-coded one-off logic.
- Use data-driven rules for categories, dependencies, replacement targets, backup policies, and platform-specific paths.
- Support package validation before installation.
- Support preview images, category/tag management, dependency checks, save backups, one-click game launch, and asset replacement mapping.
- Keep heavy work off the UI thread and use controlled concurrency for scanning, hashing, extraction, and installation planning.

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Roadmap](docs/ROADMAP.md)

## Repository Status

This repository has been initialized for planning. Application scaffolding will be added after the architecture baseline is reviewed.
