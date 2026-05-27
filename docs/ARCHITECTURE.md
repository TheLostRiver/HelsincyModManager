# Architecture

## Product Direction

Helsincy Mod Manager is not designed as a simple archive extractor. It is a local game mod management platform with game-specific adapters. Monster Hunter: World - Iceborne is the first supported game, but the architecture should allow future adapters for Monster Hunter Rise, Monster Hunter Wilds, and other games with similar asset replacement workflows.

The first version should focus on a reliable Windows experience while keeping Linux and Steam Deck support possible through platform abstraction.

## Core Principles

- UI and core logic are separated.
- Application use cases depend on traits/interfaces, not concrete infrastructure implementations.
- Game-specific behavior lives in adapters.
- Installation is based on plans and manifests, not direct ad hoc file copying.
- User-visible rules are data-driven where practical.
- Heavy work runs as background tasks with progress events.
- Game directory writes are serialized per game instance.
- All destructive operations must be reversible or recoverable.

## High-Level Layers

```text
Frontend UI
  React + TypeScript
  Presentation, interaction, filtering, dialogs, progress display

Tauri Commands
  Thin command boundary between UI and Rust
  Parameter validation and DTO mapping

Application Layer
  Use cases such as import mod, install mod, disable mod, backup saves, launch game

Domain Layer
  Mod, Game, Profile, InstallPlan, Conflict, Manifest, Dependency, ReplacementTarget

Ports / Traits
  File system, archive extraction, database repositories, game adapters, launcher, task runner

Infrastructure
  SQLite, real file system, archive tools, hashers, Steam library scan, platform APIs

Game Adapters
  Monster Hunter: World - Iceborne first
  Monster Hunter Rise / Wilds later
```

## Proposed Rust Workspace

```text
src-tauri/
  crates/
    hmm-core/          # Pure domain models and rules
    hmm-ports/         # Traits/interfaces used by application logic
    hmm-app/           # Use cases and orchestration
    hmm-infra/         # SQLite, file system, archive, hash, Steam scan
    hmm-games-mhw/     # MHW:I adapter and game rules
    hmm-tauri/         # Tauri state, commands, events, app bootstrap
```

The frontend can be organized by feature:

```text
src/
  features/
    mods/
    categories/
    profiles/
    conflicts/
    backups/
    games/
    settings/
  shared/
    api/
    components/
    state/
    types/
```

## Main Modules

### Game Discovery

Find installed games through multiple strategies:

- Steam library scanning
- Running process detection
- Manual user selection

The discovery layer returns `GameInstance` values and does not assume one fixed install path.

### Game Launcher

Launch games through the proper platform strategy:

- Steam protocol when possible
- Direct executable launch as fallback
- Future Linux / Steam Deck launch behavior through platform-specific implementations

Before launch, the app can warn about missing dependencies, unresolved conflicts, or incomplete install tasks.

### Mod Import Pipeline

Imported archives go through a validation pipeline before they become installable:

```text
Select archive
Inspect archive
Reject unsafe paths
Extract to sandbox cache
Analyze files
Extract and validate preview image
Infer mod type
Generate metadata
Generate candidate install plan
```

The importer must defend against:

- Path traversal such as `../`
- Absolute paths
- Archive bombs
- Unsupported or suspicious file types
- Fake image extensions
- Case-insensitive path collisions

### Package Analyzer

The analyzer identifies package contents such as:

- `nativePC` files
- Root-level DLL files
- Executables or helper tools
- INI/JSON/config files
- Readme files
- Preview images
- Asset IDs used by appearance, weapon, or voice replacements

It should output structured package information rather than forcing installation rules into the frontend.

### Category and Tag System

Categories and tags must support many-to-many relationships.

Default categories can include:

- Appearance
- Player appearance
- NPC appearance
- Palico appearance
- Weapon replacement
- Voice replacement
- Functional mod
- Weapon effect
- Prerequisite
- Tool

Users must be able to create custom categories and assign one mod to multiple categories or tags.

### Dependency Checker

Many Monster Hunter mods depend on prerequisite files or loaders. Dependency checks should be data-driven.

Example dependency rule shape:

```text
DependencyRule
  id
  display_name
  severity
  detection_rules
```

Detection can support rules such as:

- File exists in game root
- File exists under `nativePC`
- Known hash matches
- Known manifest entry exists

Missing required dependencies should block installation or show a clear warning depending on severity.

### Replacement Mapping

Appearance, weapon, and voice mods often replace official game asset slots. The app should model this explicitly instead of treating it as plain file copying.

Core models:

```text
ReplacementTarget
  Official game asset slot
  Example: armor set, armor part, weapon, voice slot

ReplacementBinding
  User-selected mapping from mod asset to official target

RetargetPlan
  Staging changes needed to retarget package files
```

Armor replacement should support:

- Piecewise armor: head, chest, arms, waist, legs
- Full-body armor: fixed full set replacement
- Future advanced split/transform workflows through plugin-like transformers

Important rules:

- The original imported mod package remains read-only.
- Retargeting happens in a staging directory.
- The manifest records the selected replacement binding.
- Conflict detection uses final target paths, not original archive paths.
- Changing target mapping is treated as uninstalling the old binding and installing the new one.

### Install Planner

Installation must start by creating an `InstallPlan`.

Example actions:

```text
CopyFile
CreateDirectory
BackupExistingFile
RemoveFile
WriteManifest
```

The planner is responsible for:

- Translating package contents into game target paths
- Applying replacement bindings
- Detecting conflicts
- Checking dependencies
- Estimating work for progress reporting

### Install Executor

The executor applies an `InstallPlan`.

Requirements:

- Back up overwritten files before writing.
- Write an installation manifest.
- Roll back on failure where possible.
- Serialize writes per game instance.
- Record enough state for recovery after a crash or forced shutdown.

### Save Backup Service

Save backups are independent from mod installation.

Required features:

- Manual backup
- Automatic backup
- User-selected backup directory
- Default backup directory when the user has not selected one
- Configurable automatic backup interval
- Retention policy by count, age, or size
- Backup manifest with hashes

The default backup location should live under the application data directory rather than inside the game directory.

### Task Manager

Long-running operations are background tasks:

- Archive extraction
- Package scanning
- Hash calculation
- Conflict analysis
- Install planning
- Install execution
- Save backup compression

The frontend starts tasks through Tauri commands and receives progress through events.

## Concurrency Model

The concurrency rule is:

```text
Read and prepare work may be parallel.
Writes to the same game instance must be serialized.
```

Recommended task groups:

- CPU pool: hashing and conflict analysis
- IO pool: scanning, archive extraction, file copy preparation
- Game write queue: one serialized queue per game instance
- Database transactions: short, explicit writes
- Event bus: progress and log messages

Use a two-phase workflow:

```text
Prepare phase
  Extract, hash, analyze, check dependencies, generate plan
  Parallel and cancellable

Commit phase
  Acquire game write lock
  Revalidate assumptions
  Backup, copy, remove, write manifest
  Short, serialized, recoverable
```

Avoid holding game write locks during long extraction or hashing work.

## Data Storage

SQLite stores user and runtime state:

- Game instances
- Imported mods
- Categories and tags
- Profiles
- Replacement bindings
- Install manifests
- Backup history
- User settings

JSON or TOML game data stores rule-like content:

- Default categories
- Official replacement target catalogs
- Dependency rules
- Save path rules
- Mod type detection rules
- Backup policy defaults
- Limits such as preview image size and archive size

## Key Domain Models

```text
GameDefinition
  id
  display_name
  adapter_id
  supported_platforms

GameInstance
  id
  game_id
  install_path
  platform
  launcher

ModEntry
  id
  name
  version
  package_ref
  categories
  tags
  dependencies

ModPackage
  id
  archive_path
  extracted_cache_path
  detected_type
  files
  preview_image
  metadata

ReplacementTarget
  id
  game_id
  target_type
  internal_id
  display_name
  part
  is_full_body

ReplacementBinding
  id
  mod_id
  profile_id
  source_asset
  target_id

InstallPlan
  id
  actions
  conflicts
  dependency_result
  replacement_bindings

InstallManifest
  id
  mod_id
  profile_id
  installed_files
  backups
  hashes
  replacement_bindings
```

## Important Traits

```rust
pub trait GameAdapter {
    fn game_id(&self) -> GameId;
    fn detect_instances(&self) -> Result<Vec<GameInstance>>;
    fn analyze_package(&self, package: &ModPackage) -> Result<GamePackageInfo>;
    fn build_install_plan(&self, request: InstallRequest) -> Result<InstallPlan>;
    fn dependency_rules(&self) -> Result<Vec<DependencyRule>>;
    fn replacement_catalog(&self) -> Result<Vec<ReplacementTarget>>;
}

pub trait FileSystem {
    fn exists(&self, path: &Path) -> bool;
    fn copy_file(&self, from: &Path, to: &Path) -> Result<()>;
    fn remove_file(&self, path: &Path) -> Result<()>;
    fn create_dir_all(&self, path: &Path) -> Result<()>;
}

pub trait ArchiveExtractor {
    fn inspect(&self, archive: &Path) -> Result<ArchiveInfo>;
    fn extract_to(&self, archive: &Path, target: &Path) -> Result<()>;
}

pub trait ModRepository {
    fn save(&self, mod_entry: &ModEntry) -> Result<()>;
    fn get(&self, id: ModId) -> Result<Option<ModEntry>>;
}
```

## MVP Scope

The first build should include:

- MHW:I game directory detection and manual selection
- Mod archive import and safety validation
- Preview image extraction with validation
- Category and tag management
- Dependency check baseline
- Install / uninstall with manifest
- Conflict detection based on final paths
- Manual save backup
- One-click game launch

## Next Scope

After the MVP:

- Appearance, weapon, and voice replacement target selection
- Profiles
- Automatic save backups
- Advanced rollback and recovery UI
- Task queue UI
- Linux / Steam Deck experimental packaging and community testing
