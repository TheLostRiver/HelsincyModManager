# Profile Save Settings UI Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the Profile page from a visual CRUD deck into a practical configuration workspace with save-directory selection, backup-directory selection, and an automatic-backup schedule UI backed by narrow, validated Tauri commands.

**Architecture:** This first slice adds a controlled profile save-settings contract and rewires the Profile page to consume it. The frontend may pass a directory selected by the user through the system dialog, but only backend services validate, persist, and summarize save/backup directory state. This plan does not execute real save backups, restore saves, or retention cleanup.

**Tech Stack:** React + TypeScript, lucide-react, `@tauri-apps/plugin-dialog`, Tauri commands, Rust workspace (`hmm-core`, `hmm-ports`, `hmm-app`, `hmm-infra`, `src-tauri/src`), SQLite migration, Node test runner, Cargo tests.

---

## Scope Check

The full product request covers three subsystems: profile UI, save/backup settings persistence, and automatic backup execution. This plan intentionally implements only the first two so the UI becomes real without creating a long-running backup scheduler or touching real save files. A later plan should add `SaveBackupService` task execution, backup manifests, restore confirmation, retention cleanup, audit events for actual backup runs, and task progress phases.

## File Structure

- Modify `docs/FRONTEND_BACKEND_CONTRACT.md`: register the new profile save-settings commands and DTO rules.
- Modify `src-tauri/crates/hmm-core/src/profile.rs`: add profile save-settings domain value objects that do not perform file I/O.
- Modify `src-tauri/crates/hmm-ports/src/profile.rs`: extend profile ports with a settings repository and directory validator interfaces.
- Modify `src-tauri/crates/hmm-app/src/profile.rs`: add app-service methods for reading, validating, and saving settings.
- Modify `src-tauri/crates/hmm-app/tests/profile.rs`: add fake-port tests for validation and persistence behavior.
- Modify `src-tauri/crates/hmm-infra/src/sqlite/migrations.rs`: register the new migration.
- Create `src-tauri/crates/hmm-infra/src/sqlite/migrations/003_profile_save_settings.sql`: persist per-profile save settings.
- Modify `src-tauri/crates/hmm-infra/src/sqlite/profile_repository.rs`: implement settings storage methods.
- Modify `src-tauri/src/dto.rs`: add DTOs and serialization tests.
- Modify `src-tauri/src/profile_commands.rs`: add narrow Tauri commands.
- Modify `src-tauri/src/lib.rs`: register commands.
- Create `src/features/profiles/profileSaveSettingsTypes.ts`: frontend DTOs and view types.
- Create `src/features/profiles/profileSaveSettingsApi.ts`: feature-local typed API wrapper.
- Create `src/features/profiles/BackupSchedulePicker.tsx`: reusable schedule and time-popover UI.
- Create `src/features/profiles/ProfileListPanel.tsx`: profile list and create/edit/delete shell.
- Create `src/features/profiles/SaveDirectoryPanel.tsx`: save source and backup target directory UI.
- Create `src/features/profiles/BackupPolicyPanel.tsx`: automatic backup schedule UI.
- Create `src/features/profiles/profileViewModel.ts`: profile metrics and status-label mapping.
- Modify `src/features/profiles/ProfilePage.tsx`: compose the new workspace.
- Replace large sections of `src/features/profiles/ProfilePage.css`: practical workspace layout, directory rows, panels, popover states.
- Modify `src/features/profiles/profileApi.test.mjs`: keep profile CRUD API path restrictions and add save-settings wrapper checks.
- Modify `src/features/profiles/profileFrontendIntegration.test.mjs`: assert the new page structure and boundary constraints.
- Add `src/features/profiles/profileSaveSettingsViewModel.test.mjs`: focused frontend view-model tests.

---

### Task 1: Register The Contract

**Files:**
- Modify: `docs/FRONTEND_BACKEND_CONTRACT.md`
- Modify: `src/features/profiles/profileApi.test.mjs`

- [ ] **Step 1: Add failing frontend contract assertions**

Append this test to `src/features/profiles/profileApi.test.mjs`:

```js
test("profile save settings API uses narrow settings commands", () => {
  const source = readSource("src/features/profiles/profileSaveSettingsApi.ts");
  const typesSource = readSource("src/features/profiles/profileSaveSettingsTypes.ts");

  assert.match(source, /invoke<ProfileSaveSettingsDto>\("get_profile_save_settings"/);
  assert.match(source, /invoke<ProfileDirectoryValidationDto>\("validate_profile_save_directory"/);
  assert.match(source, /invoke<ProfileDirectoryValidationDto>\("validate_profile_backup_directory"/);
  assert.match(source, /invoke<ProfileSaveSettingsDto>\("set_profile_save_settings"/);
  assert.match(typesSource, /export type BackupCadence = "manual" \| "daily" \| "weekly"/);
  assert.match(typesSource, /pathLabel:\s*string\s*\|\s*null/);
  assert.doesNotMatch(typesSource, /manifestPath|backupRoot|backupRef|targetPath|sandbox|cache/i);
});
```

- [ ] **Step 2: Run the focused test and confirm it fails**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/features/profiles/profileApi.test.mjs
```

Expected: FAIL because `profileSaveSettingsApi.ts` and `profileSaveSettingsTypes.ts` do not exist.

- [ ] **Step 3: Document the new command contract**

Add a subsection under `### 3. Profile 管理` in `docs/FRONTEND_BACKEND_CONTRACT.md`:

````markdown
Profile save settings commands:

```text
get_profile_save_settings(profileId)
validate_profile_save_directory({ gameId, profileId, directory })
validate_profile_backup_directory({ gameId, profileId, directory })
set_profile_save_settings(input)
```

Boundary:

- These commands configure save backup settings for a profile; they do not execute backup, restore, retention cleanup, install, uninstall, manifest writes, or rollback.
- The frontend may pass a directory selected through the system directory picker, but every command must validate it again.
- Response DTOs expose `pathLabel`, status, schedule values, and stable validation codes. They do not expose `manifestPath`, `backupRoot`, `backupRef`, sandbox/cache paths, raw save contents, or third-party Mod content.
- `validate_profile_save_directory` validates the source save directory using game/app rules and returns a display-safe label.
- `validate_profile_backup_directory` validates the target backup directory and must reject locations inside the current game install directory when the backend can determine that relationship.
- `set_profile_save_settings` stores configuration only after app-service validation and writes an Audit Log event for automatic backup setting changes once audit support is wired for this settings domain.

DTO shape:

```ts
type BackupCadence = "manual" | "daily" | "weekly";

type ProfileDirectoryStatusDto =
  | "unset"
  | "valid"
  | "invalid"
  | "defaulted";

type ProfileDirectorySelectionDto = {
  mode: "unset" | "custom" | "default";
  status: ProfileDirectoryStatusDto;
  pathLabel: string | null;
  messages: string[];
};

type ProfileBackupScheduleDto = {
  cadence: BackupCadence;
  hour: number | null;
  minute: number | null;
  weekdays: number[];
};

type ProfileBackupRetentionDto = {
  maxCount: number;
  maxAgeDays: number | null;
};

type ProfileSaveSettingsDto = {
  profileId: string;
  saveDirectory: ProfileDirectorySelectionDto;
  backupDirectory: ProfileDirectorySelectionDto;
  schedule: ProfileBackupScheduleDto;
  retention: ProfileBackupRetentionDto;
  updatedAt: number;
};
```
````

- [ ] **Step 4: Run the focused test and confirm the contract task still fails for missing files**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/features/profiles/profileApi.test.mjs
```

Expected: FAIL for missing frontend files only; the documentation edit has no automated check in this focused test.

---

### Task 2: Add Backend Domain, Ports, And App Service

**Files:**
- Modify: `src-tauri/crates/hmm-core/src/profile.rs`
- Modify: `src-tauri/crates/hmm-ports/src/profile.rs`
- Modify: `src-tauri/crates/hmm-app/src/profile.rs`
- Modify: `src-tauri/crates/hmm-app/tests/profile.rs`

- [ ] **Step 1: Write failing app-service tests**

Append tests to `src-tauri/crates/hmm-app/tests/profile.rs` proving settings are profile-scoped and validated:

```rust
#[test]
fn save_settings_rejects_unknown_profile() {
    let (service, _repo) = make_service();

    let result = service.set_profile_save_settings(hmm_app::SetProfileSaveSettingsRequest {
        profile_id: "missing".to_owned(),
        game_id: "mhw".to_owned(),
        save_directory: Some("C:/Users/Test/Saves".to_owned()),
        backup_directory: None,
        schedule: hmm_core::ProfileBackupSchedule::manual(),
        retention: hmm_core::ProfileBackupRetention::default(),
    });

    assert!(result.is_err());
}

#[test]
fn save_settings_validates_selected_directories_before_persisting() {
    let (service, repo) = make_service();
    repo.save(&Profile {
        id: "profile-1".to_owned(),
        name: "Profile".to_owned(),
        description: None,
        is_active: false,
        created_at: 1,
        updated_at: 1,
    })
    .unwrap();

    let result = service.set_profile_save_settings(hmm_app::SetProfileSaveSettingsRequest {
        profile_id: "profile-1".to_owned(),
        game_id: "mhw".to_owned(),
        save_directory: Some("C:/Users/Test/Saves".to_owned()),
        backup_directory: Some("D:/HMM/Backups".to_owned()),
        schedule: hmm_core::ProfileBackupSchedule {
            cadence: hmm_core::BackupCadence::Daily,
            hour: Some(3),
            minute: Some(0),
            weekdays: Vec::new(),
        },
        retention: hmm_core::ProfileBackupRetention {
            max_count: 20,
            max_age_days: Some(30),
        },
    });

    let settings = result.expect("settings saved");
    assert_eq!(settings.profile_id, "profile-1");
    assert_eq!(settings.save_directory.path_label.as_deref(), Some("Saves"));
    assert_eq!(settings.backup_directory.path_label.as_deref(), Some("Backups"));
    assert_eq!(settings.schedule.cadence, hmm_core::BackupCadence::Daily);
    assert_eq!(settings.retention.max_count, 20);
}
```

- [ ] **Step 2: Run the app-service tests and confirm they fail**

Run:

```powershell
cargo test -p hmm-app profile_save_settings
```

Expected: FAIL because `SetProfileSaveSettingsRequest`, schedule types, and service methods do not exist.

- [ ] **Step 3: Add core profile save-settings value objects**

Add to `src-tauri/crates/hmm-core/src/profile.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupCadence {
    Manual,
    Daily,
    Weekly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileDirectoryStatus {
    Unset,
    Valid,
    Invalid,
    Defaulted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDirectorySelection {
    pub mode: ProfileDirectoryMode,
    pub status: ProfileDirectoryStatus,
    pub path_label: Option<String>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileDirectoryMode {
    Unset,
    Custom,
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileBackupSchedule {
    pub cadence: BackupCadence,
    pub hour: Option<u8>,
    pub minute: Option<u8>,
    pub weekdays: Vec<u8>,
}

impl ProfileBackupSchedule {
    pub fn manual() -> Self {
        Self {
            cadence: BackupCadence::Manual,
            hour: None,
            minute: None,
            weekdays: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileBackupRetention {
    pub max_count: u32,
    pub max_age_days: Option<u32>,
}

impl Default for ProfileBackupRetention {
    fn default() -> Self {
        Self {
            max_count: 20,
            max_age_days: Some(30),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSaveSettings {
    pub profile_id: String,
    pub save_directory: ProfileDirectorySelection,
    pub backup_directory: ProfileDirectorySelection,
    pub schedule: ProfileBackupSchedule,
    pub retention: ProfileBackupRetention,
    pub updated_at: u128,
}
```

- [ ] **Step 4: Add ports for settings persistence and directory validation**

Extend `src-tauri/crates/hmm-ports/src/profile.rs`:

```rust
use hmm_core::{Profile, ProfileDirectorySelection, ProfileSaveSettings};

pub trait ProfileSaveSettingsRepository: Send + Sync {
    fn get_settings(&self, profile_id: &str) -> Result<Option<ProfileSaveSettings>>;
    fn save_settings(&self, settings: &ProfileSaveSettings) -> Result<()>;
}

pub trait ProfileSaveDirectoryValidator: Send + Sync {
    fn validate_save_directory(
        &self,
        game_id: &str,
        directory: &str,
    ) -> Result<ProfileDirectorySelection>;

    fn validate_backup_directory(
        &self,
        game_id: &str,
        directory: &str,
    ) -> Result<ProfileDirectorySelection>;

    fn default_backup_directory(&self, game_id: &str) -> Result<ProfileDirectorySelection>;
}
```

- [ ] **Step 5: Add app-service request types and methods**

Add to `src-tauri/crates/hmm-app/src/profile.rs`:

```rust
pub struct SetProfileSaveSettingsRequest {
    pub profile_id: String,
    pub game_id: String,
    pub save_directory: Option<String>,
    pub backup_directory: Option<String>,
    pub schedule: hmm_core::ProfileBackupSchedule,
    pub retention: hmm_core::ProfileBackupRetention,
}
```

Extend `ProfileService` to receive `Arc<dyn ProfileSaveSettingsRepository>` and `Arc<dyn ProfileSaveDirectoryValidator>`. Add methods:

```rust
pub fn get_profile_save_settings(&self, profile_id: &str) -> Result<hmm_core::ProfileSaveSettings> {
    self.profile_repository
        .get(profile_id)?
        .ok_or_else(|| anyhow::anyhow!("profile not found: {profile_id}"))?;

    if let Some(settings) = self.save_settings_repository.get_settings(profile_id)? {
        return Ok(settings);
    }

    Ok(hmm_core::ProfileSaveSettings {
        profile_id: profile_id.to_owned(),
        save_directory: hmm_core::ProfileDirectorySelection {
            mode: hmm_core::ProfileDirectoryMode::Unset,
            status: hmm_core::ProfileDirectoryStatus::Unset,
            path_label: None,
            messages: vec!["尚未选择游戏存档目录".to_owned()],
        },
        backup_directory: self.save_directory_validator.default_backup_directory("mhw")?,
        schedule: hmm_core::ProfileBackupSchedule::manual(),
        retention: hmm_core::ProfileBackupRetention::default(),
        updated_at: 0,
    })
}

pub fn validate_profile_save_directory(
    &self,
    game_id: &str,
    directory: &str,
) -> Result<hmm_core::ProfileDirectorySelection> {
    self.save_directory_validator
        .validate_save_directory(game_id, directory)
}

pub fn validate_profile_backup_directory(
    &self,
    game_id: &str,
    directory: &str,
) -> Result<hmm_core::ProfileDirectorySelection> {
    self.save_directory_validator
        .validate_backup_directory(game_id, directory)
}

pub fn set_profile_save_settings(
    &self,
    request: SetProfileSaveSettingsRequest,
) -> Result<hmm_core::ProfileSaveSettings> {
    self.profile_repository
        .get(&request.profile_id)?
        .ok_or_else(|| anyhow::anyhow!("profile not found: {}", request.profile_id))?;

    let save_directory = match request.save_directory {
        Some(directory) => self
            .save_directory_validator
            .validate_save_directory(&request.game_id, &directory)?,
        None => hmm_core::ProfileDirectorySelection {
            mode: hmm_core::ProfileDirectoryMode::Unset,
            status: hmm_core::ProfileDirectoryStatus::Unset,
            path_label: None,
            messages: vec!["尚未选择游戏存档目录".to_owned()],
        },
    };

    let backup_directory = match request.backup_directory {
        Some(directory) => self
            .save_directory_validator
            .validate_backup_directory(&request.game_id, &directory)?,
        None => self
            .save_directory_validator
            .default_backup_directory(&request.game_id)?,
    };

    validate_schedule(&request.schedule)?;
    validate_retention(&request.retention)?;

    let settings = hmm_core::ProfileSaveSettings {
        profile_id: request.profile_id,
        save_directory,
        backup_directory,
        schedule: request.schedule,
        retention: request.retention,
        updated_at: self.clock.now_ms(),
    };

    self.save_settings_repository.save_settings(&settings)?;
    Ok(settings)
}
```

Add helper functions in the same file:

```rust
fn validate_schedule(schedule: &hmm_core::ProfileBackupSchedule) -> Result<()> {
    match schedule.cadence {
        hmm_core::BackupCadence::Manual => Ok(()),
        hmm_core::BackupCadence::Daily => {
            ensure!(schedule.hour.is_some(), "backup hour is required");
            ensure!(schedule.minute.is_some(), "backup minute is required");
            Ok(())
        }
        hmm_core::BackupCadence::Weekly => {
            ensure!(schedule.hour.is_some(), "backup hour is required");
            ensure!(schedule.minute.is_some(), "backup minute is required");
            ensure!(!schedule.weekdays.is_empty(), "weekly backup days are required");
            ensure!(
                schedule.weekdays.iter().all(|day| *day <= 6),
                "weekly backup day must be between 0 and 6"
            );
            Ok(())
        }
    }
}

fn validate_retention(retention: &hmm_core::ProfileBackupRetention) -> Result<()> {
    ensure!(retention.max_count > 0, "backup retention max count must be greater than zero");
    if let Some(max_age_days) = retention.max_age_days {
        ensure!(max_age_days > 0, "backup retention max age days must be greater than zero");
    }
    Ok(())
}
```

- [ ] **Step 6: Update fake test fixtures**

Update `make_service()` in `src-tauri/crates/hmm-app/tests/profile.rs` to construct fake settings repository and fake validator. The validator should return the final path segment as `path_label`:

```rust
fn path_label(path: &str) -> String {
    path.replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_owned()
}
```

- [ ] **Step 7: Run app-service tests**

Run:

```powershell
cargo test -p hmm-app profile
```

Expected: PASS for existing profile tests and new save-settings tests.

---

### Task 3: Persist Settings And Expose Tauri Commands

**Files:**
- Create: `src-tauri/crates/hmm-infra/src/sqlite/migrations/003_profile_save_settings.sql`
- Modify: `src-tauri/crates/hmm-infra/src/sqlite/migrations.rs`
- Modify: `src-tauri/crates/hmm-infra/src/sqlite/profile_repository.rs`
- Modify: `src-tauri/src/dto.rs`
- Modify: `src-tauri/src/profile_commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing DTO serialization tests**

Add to `src-tauri/src/dto.rs` tests:

```rust
#[test]
fn serializes_profile_save_settings_without_raw_storage_paths() {
    let dto: ProfileSaveSettingsDto = hmm_core::ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: hmm_core::ProfileDirectorySelection {
            mode: hmm_core::ProfileDirectoryMode::Custom,
            status: hmm_core::ProfileDirectoryStatus::Valid,
            path_label: Some("Saves".to_owned()),
            messages: Vec::new(),
        },
        backup_directory: hmm_core::ProfileDirectorySelection {
            mode: hmm_core::ProfileDirectoryMode::Default,
            status: hmm_core::ProfileDirectoryStatus::Defaulted,
            path_label: Some("HelsincyModManager/Backups".to_owned()),
            messages: vec!["使用默认备份目录".to_owned()],
        },
        schedule: hmm_core::ProfileBackupSchedule {
            cadence: hmm_core::BackupCadence::Weekly,
            hour: Some(3),
            minute: Some(0),
            weekdays: vec![0],
        },
        retention: hmm_core::ProfileBackupRetention {
            max_count: 20,
            max_age_days: Some(30),
        },
        updated_at: 42,
    }
    .into();

    let value = serde_json::to_value(dto).expect("serialize profile save settings");

    assert_eq!(value["profileId"], "default");
    assert_eq!(value["saveDirectory"]["pathLabel"], "Saves");
    assert_eq!(value["backupDirectory"]["mode"], "default");
    assert_eq!(value["schedule"]["cadence"], "weekly");
    assert_eq!(value["schedule"]["weekdays"][0], 0);
    assert_eq!(value["retention"]["maxCount"], 20);
    assert!(value.get("manifestPath").is_none());
    assert!(value.get("backupRoot").is_none());
    assert!(!value.to_string().contains("C:/Users/"));
}
```

- [ ] **Step 2: Run DTO tests and confirm they fail**

Run:

```powershell
cargo test -p hmm-tauri profile_save_settings
```

Expected: FAIL because DTOs are missing.

- [ ] **Step 3: Add SQLite migration**

Create `src-tauri/crates/hmm-infra/src/sqlite/migrations/003_profile_save_settings.sql`:

```sql
CREATE TABLE profile_save_settings (
    profile_id              TEXT    PRIMARY KEY NOT NULL,
    save_directory          TEXT,
    backup_directory        TEXT,
    backup_cadence          TEXT    NOT NULL DEFAULT 'manual',
    backup_hour             INTEGER,
    backup_minute           INTEGER,
    backup_weekdays         TEXT    NOT NULL DEFAULT '[]',
    retention_max_count     INTEGER NOT NULL DEFAULT 20,
    retention_max_age_days  INTEGER,
    updated_at              INTEGER NOT NULL,
    FOREIGN KEY(profile_id) REFERENCES profiles(profile_id) ON DELETE CASCADE
);
```

Register it in `src-tauri/crates/hmm-infra/src/sqlite/migrations.rs` after migration 002:

```rust
M::up(include_str!("migrations/003_profile_save_settings.sql")),
```

- [ ] **Step 4: Implement repository methods**

In `src-tauri/crates/hmm-infra/src/sqlite/profile_repository.rs`, implement `ProfileSaveSettingsRepository` for `SqliteProfileRepository`. Store raw selected directories only in SQLite; when reading for DTO use the app validator to return `pathLabel`. Repository methods should round-trip raw values into domain selections with `path_label` derived from the final path segment until a validator refreshes them.

- [ ] **Step 5: Add a minimal infra validator**

Add a `SqliteProfileRepository` helper or adjacent struct that implements `ProfileSaveDirectoryValidator` without game-specific save-path discovery. It should:

```text
- reject empty strings
- reject relative-looking inputs when `Path::is_absolute()` is false
- return `ProfileDirectoryStatus::Valid` and a final-segment `pathLabel` for accepted custom directories
- return default backup selection with mode `default`, status `defaulted`, and label `HelsincyModManager/Backups`
```

This first slice does not scan real save files and does not create backup directories.

- [ ] **Step 6: Add DTOs and conversions**

In `src-tauri/src/dto.rs`, add `ProfileSaveSettingsDto`, `ProfileDirectorySelectionDto`, `ProfileBackupScheduleDto`, `ProfileBackupRetentionDto`, and stable snake_case enum DTOs. Implement `From<hmm_core::ProfileSaveSettings>`.

- [ ] **Step 7: Add commands**

In `src-tauri/src/profile_commands.rs`, add:

```rust
#[tauri::command]
pub fn get_profile_save_settings(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<ProfileSaveSettingsDto, CommandErrorDto> {
    state
        .profiles
        .get_profile_save_settings(&profile_id)
        .map(ProfileSaveSettingsDto::from)
        .map_err(profile_error)
}

#[tauri::command]
pub fn validate_profile_save_directory(
    game_id: String,
    profile_id: String,
    directory: String,
    state: State<'_, AppState>,
) -> Result<ProfileDirectorySelectionDto, CommandErrorDto> {
    state
        .profiles
        .validate_profile_save_directory(&game_id, &directory)
        .map(ProfileDirectorySelectionDto::from)
        .map_err(profile_error)
}

#[tauri::command]
pub fn validate_profile_backup_directory(
    game_id: String,
    profile_id: String,
    directory: String,
    state: State<'_, AppState>,
) -> Result<ProfileDirectorySelectionDto, CommandErrorDto> {
    state
        .profiles
        .validate_profile_backup_directory(&game_id, &directory)
        .map(ProfileDirectorySelectionDto::from)
        .map_err(profile_error)
}
```

Use `profile_id` in the validation commands to check the profile exists before returning validation output.

- [ ] **Step 8: Register commands and state wiring**

Modify `src-tauri/src/lib.rs` to import and register the new commands. Modify `src-tauri/src/state.rs` to construct `ProfileService` with the settings repository and validator.

- [ ] **Step 9: Run Rust checks**

Run:

```powershell
cargo test --workspace
cargo check --workspace
```

Expected: PASS.

---

### Task 4: Add Frontend Typed API And View Models

**Files:**
- Create: `src/features/profiles/profileSaveSettingsTypes.ts`
- Create: `src/features/profiles/profileSaveSettingsApi.ts`
- Create: `src/features/profiles/profileViewModel.ts`
- Add: `src/features/profiles/profileSaveSettingsViewModel.test.mjs`

- [ ] **Step 1: Create the frontend DTO types**

Create `src/features/profiles/profileSaveSettingsTypes.ts`:

```ts
export type BackupCadence = "manual" | "daily" | "weekly";

export type ProfileDirectoryStatus = "unset" | "valid" | "invalid" | "defaulted";

export type ProfileDirectorySelectionDto = {
  mode: "unset" | "custom" | "default";
  status: ProfileDirectoryStatus;
  pathLabel: string | null;
  messages: string[];
};

export type ProfileBackupScheduleDto = {
  cadence: BackupCadence;
  hour: number | null;
  minute: number | null;
  weekdays: number[];
};

export type ProfileBackupRetentionDto = {
  maxCount: number;
  maxAgeDays: number | null;
};

export type ProfileSaveSettingsDto = {
  profileId: string;
  saveDirectory: ProfileDirectorySelectionDto;
  backupDirectory: ProfileDirectorySelectionDto;
  schedule: ProfileBackupScheduleDto;
  retention: ProfileBackupRetentionDto;
  updatedAt: number;
};

export type ProfileDirectoryValidationDto = ProfileDirectorySelectionDto;

export type SetProfileSaveSettingsInput = {
  gameId: string;
  profileId: string;
  saveDirectory?: string | null;
  backupDirectory?: string | null;
  schedule: ProfileBackupScheduleDto;
  retention: ProfileBackupRetentionDto;
};
```

- [ ] **Step 2: Create the typed API wrapper**

Create `src/features/profiles/profileSaveSettingsApi.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type {
  ProfileDirectoryValidationDto,
  ProfileSaveSettingsDto,
  SetProfileSaveSettingsInput,
} from "./profileSaveSettingsTypes";

export function getProfileSaveSettings(profileId: string): Promise<ProfileSaveSettingsDto> {
  return invoke<ProfileSaveSettingsDto>("get_profile_save_settings", { profileId });
}

export function validateProfileSaveDirectory(input: {
  gameId: string;
  profileId: string;
  directory: string;
}): Promise<ProfileDirectoryValidationDto> {
  return invoke<ProfileDirectoryValidationDto>("validate_profile_save_directory", input);
}

export function validateProfileBackupDirectory(input: {
  gameId: string;
  profileId: string;
  directory: string;
}): Promise<ProfileDirectoryValidationDto> {
  return invoke<ProfileDirectoryValidationDto>("validate_profile_backup_directory", input);
}

export function setProfileSaveSettings(input: SetProfileSaveSettingsInput): Promise<ProfileSaveSettingsDto> {
  return invoke<ProfileSaveSettingsDto>("set_profile_save_settings", input);
}
```

- [ ] **Step 3: Add view-model tests**

Create `src/features/profiles/profileSaveSettingsViewModel.test.mjs`:

```js
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

test("profile view model maps save settings statuses without exposing raw paths", () => {
  const source = readFileSync("src/features/profiles/profileViewModel.ts", "utf8");

  assert.match(source, /formatDirectoryStatus/);
  assert.match(source, /formatBackupSchedule/);
  assert.match(source, /pathLabel/);
  assert.doesNotMatch(source, /manifestPath|backupRoot|backupRef|targetPath|sandbox|cache/i);
});
```

- [ ] **Step 4: Add view-model helpers**

Create `src/features/profiles/profileViewModel.ts`:

```ts
import type { BackupCadence, ProfileDirectorySelectionDto, ProfileBackupScheduleDto } from "./profileSaveSettingsTypes";
import type { Profile } from "./profileTypes";

export type ProfileMetrics = {
  totalCount: number;
  standbyCount: number;
  deletableCount: number;
};

export function getProfileMetrics(profiles: Profile[]): ProfileMetrics {
  return profiles.reduce<ProfileMetrics>(
    (metrics, profile) => ({
      totalCount: metrics.totalCount + 1,
      standbyCount: metrics.standbyCount + (profile.isActive ? 0 : 1),
      deletableCount: metrics.deletableCount + (profile.id !== "default" && !profile.isActive ? 1 : 0),
    }),
    { totalCount: 0, standbyCount: 0, deletableCount: 0 },
  );
}

export function formatDirectoryStatus(selection: ProfileDirectorySelectionDto) {
  switch (selection.status) {
    case "valid":
      return { label: selection.pathLabel ?? "已配置", tone: "success" as const };
    case "defaulted":
      return { label: selection.pathLabel ?? "默认目录", tone: "neutral" as const };
    case "invalid":
      return { label: selection.pathLabel ?? "目录不可用", tone: "warning" as const };
    case "unset":
      return { label: "未选择", tone: "warning" as const };
  }
}

export function formatBackupSchedule(schedule: ProfileBackupScheduleDto) {
  if (schedule.cadence === "manual") {
    return "仅手动";
  }

  const hour = String(schedule.hour ?? 0).padStart(2, "0");
  const minute = String(schedule.minute ?? 0).padStart(2, "0");

  if (schedule.cadence === "daily") {
    return `每日 ${hour}:${minute}`;
  }

  return `${formatWeekdays(schedule.weekdays)} ${hour}:${minute}`;
}

export function defaultSchedule(cadence: BackupCadence): ProfileBackupScheduleDto {
  if (cadence === "manual") {
    return { cadence, hour: null, minute: null, weekdays: [] };
  }

  if (cadence === "daily") {
    return { cadence, hour: 3, minute: 0, weekdays: [] };
  }

  return { cadence, hour: 3, minute: 0, weekdays: [0] };
}

function formatWeekdays(days: number[]) {
  if (days.length === 0) return "每周";
  if (days.length === 7) return "每天";

  const labels = new Map([
    [1, "一"],
    [2, "二"],
    [3, "三"],
    [4, "四"],
    [5, "五"],
    [6, "六"],
    [0, "日"],
  ]);

  return `每周${days.map((day) => labels.get(day)).filter(Boolean).join("、")}`;
}
```

- [ ] **Step 5: Run focused frontend tests**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/features/profiles/profileApi.test.mjs src/features/profiles/profileSaveSettingsViewModel.test.mjs
```

Expected: PASS.

---

### Task 5: Refactor The Profile Page Into A Workspace

**Files:**
- Create: `src/features/profiles/BackupSchedulePicker.tsx`
- Create: `src/features/profiles/ProfileListPanel.tsx`
- Create: `src/features/profiles/SaveDirectoryPanel.tsx`
- Create: `src/features/profiles/BackupPolicyPanel.tsx`
- Modify: `src/features/profiles/ProfilePage.tsx`
- Modify: `src/features/profiles/ProfilePage.css`
- Modify: `src/features/profiles/profileFrontendIntegration.test.mjs`

- [ ] **Step 1: Add failing integration assertions for the new page shape**

Update `profileFrontendIntegration.test.mjs`:

```js
test("profile page exposes save settings workspace panels without shell coupling", () => {
  const source = readSource("src/features/profiles/ProfilePage.tsx");
  const css = readSource("src/features/profiles/ProfilePage.css");

  assert.match(source, /ProfileListPanel/);
  assert.match(source, /SaveDirectoryPanel/);
  assert.match(source, /BackupPolicyPanel/);
  assert.match(source, /getProfileSaveSettings/);
  assert.match(source, /setProfileSaveSettings/);
  assert.doesNotMatch(source, /useSidebarMode|sidebarMode/);
  assert.doesNotMatch(source, /manifestPath|backupRoot|backupRef|targetPath|sandbox|cache/i);
  assert.match(css, /\.profile-workspace/);
  assert.match(css, /\.profile-settings-panel/);
  assert.match(css, /\.profile-directory-row/);
});
```

- [ ] **Step 2: Run the integration test and confirm it fails**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/features/profiles/profileFrontendIntegration.test.mjs
```

Expected: FAIL because the new components and CSS selectors do not exist.

- [ ] **Step 3: Extract `BackupSchedulePicker`**

Create `BackupSchedulePicker.tsx` by adapting the existing `SettingsPage` `TimePickerPopover` and `ScrollPicker`. Export:

```ts
export function BackupSchedulePicker({
  schedule,
  onChange,
}: {
  schedule: ProfileBackupScheduleDto;
  onChange: (schedule: ProfileBackupScheduleDto) => void;
}) { ... }
```

Keep these behaviors:

```text
- segmented control for manual/daily/weekly
- popover opens for daily and weekly
- hour/minute scroll pickers
- weekly day buttons
- Escape closes the popover
- outside click closes the popover
- reduced-motion CSS disables popover animation
```

- [ ] **Step 4: Create directory and policy panels**

Create `SaveDirectoryPanel.tsx` with two directory rows:

```ts
type SaveDirectoryPanelProps = {
  gameId: string;
  profileId: string;
  settings: ProfileSaveSettingsDto;
  onSettingsChange: (settings: ProfileSaveSettingsDto) => void;
};
```

Use `open({ directory: true, multiple: false })` from `@tauri-apps/plugin-dialog` for both directory buttons, then call the corresponding validation API before updating local form state.

Create `BackupPolicyPanel.tsx`:

```ts
type BackupPolicyPanelProps = {
  settings: ProfileSaveSettingsDto;
  onScheduleChange: (schedule: ProfileBackupScheduleDto) => void;
  onRetentionChange: (retention: ProfileBackupRetentionDto) => void;
};
```

- [ ] **Step 5: Create profile list panel**

Move profile list, create, edit, activate, and delete UI into `ProfileListPanel.tsx`. Preserve existing behavior:

```text
- default profile cannot be deleted
- active profile cannot be deleted
- activation refreshes active profile
- create/edit reject empty names
```

- [ ] **Step 6: Compose the new page**

Rewrite `ProfilePage.tsx` as an orchestrator:

```text
- load profiles
- load active profile
- load save settings for the active profile
- show left profile list
- show main workspace with overview, save-directory panel, backup-policy panel
- save settings through `setProfileSaveSettings`
- show loading/error/empty states
```

Use a constant `CURRENT_GAME_ID = "mhw"` for this first slice, matching existing frontend convention that the current game is MHW:I.

- [ ] **Step 7: Replace profile CSS with a work-focused layout**

Keep `ProfilePage.css` under the existing namespace and add selectors:

```css
.profile-workspace {
  display: grid;
  grid-template-columns: minmax(260px, 320px) minmax(0, 1fr);
  gap: var(--layout-content-gap);
  min-width: 0;
}

.profile-settings-stack {
  display: grid;
  gap: 14px;
  min-width: 0;
}

.profile-settings-panel {
  min-width: 0;
  background: var(--color-surface);
  border: 1px solid var(--color-border-muted);
  border-radius: var(--radius-inner);
  box-shadow: var(--shadow-soft);
}

.profile-directory-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 12px;
  align-items: center;
  padding: 14px 16px;
  border-top: 1px solid var(--color-border-muted);
}
```

Remove the large decorative `PROFILE` watermark and reduce large gradients. Keep responsive breakpoints at `1180px`, `860px`, and `640px`.

- [ ] **Step 8: Run focused frontend tests**

Run:

```powershell
cmd /c corepack pnpm exec node --test src/features/profiles/profileFrontendIntegration.test.mjs src/features/profiles/profileApi.test.mjs src/features/profiles/profileSaveSettingsViewModel.test.mjs
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

Expected: PASS.

---

### Task 6: Verification And Handoff

**Files:**
- No code files unless previous tasks expose issues.

- [ ] **Step 1: Run cross-boundary checks**

Run:

```powershell
cargo test --workspace
cargo check --workspace
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

Expected: PASS.

- [ ] **Step 2: Run full verification**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected: PASS.

- [ ] **Step 3: Manual UI smoke**

Start the app:

```powershell
cmd /c corepack pnpm run tauri:dev
```

Check:

```text
- Profile page loads with the active profile selected.
- Save directory choose button opens a system directory picker.
- Backup directory choose button opens a system directory picker.
- Invalid directory selection returns an inline warning without saving.
- Manual/daily/weekly schedule switching does not resize the whole page.
- Time popover can be opened, saved, closed by outside click, and closed by Escape.
- 1440x900, 1366x768, and 1280x800 layouts do not overlap.
```

- [ ] **Step 4: Record verification**

In the final handoff, report:

```text
已执行：
- cargo test --workspace
- cargo check --workspace
- cmd /c corepack pnpm run typecheck
- cmd /c corepack pnpm run lint
- cmd /c corepack pnpm run build
- powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1

手动检查：
- tauri:dev profile save-settings smoke at listed viewport sizes

未执行：
- Real save backup execution, restore, retention cleanup; out of scope for this first slice.
```

---

## Self-Review

Spec coverage: The plan covers a real Profile-page UI restructure, save-directory selection, backup-directory selection, automatic backup schedule UI, typed API wrappers, Tauri commands, Rust app/ports/infra boundaries, contract docs, and focused tests. It explicitly excludes real backup execution and restore because those require a separate safety plan.

Placeholder scan: No placeholder markers or unspecified test instructions remain. Each task has exact files, command lines, and expected outcomes.

Type consistency: The TypeScript DTO names match the contract section and frontend API. The Rust domain names map one-to-one to DTO conversion names. The command names are snake_case and feature-local frontend wrappers use camelCase input/output types.
