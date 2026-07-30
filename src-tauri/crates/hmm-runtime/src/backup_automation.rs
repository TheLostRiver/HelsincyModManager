use crate::game_automation::is_canonically_within;
use crate::{production_app_data_dir, RuntimeEnvironment};
use anyhow::Result;
use hmm_app::{SaveBackupBackgroundService, SaveBackupBackgroundStatus};
use hmm_core::{
    GameId, ProfileId, SaveBackupBackgroundRegistrationStatus, SaveBackupSchedulerPendingReason,
    SaveBackupSummary,
};
#[cfg(not(target_os = "windows"))]
use hmm_infra::UnsupportedSaveBackupBackgroundRegistry;
#[cfg(target_os = "windows")]
use hmm_infra::WindowsScheduledTaskRegistry;
use hmm_infra::{
    open_database_read_only, SqliteSaveBackupBackgroundSettingsRepository,
    SqliteSaveBackupRepository, SqliteSaveBackupSchedulerStateRepository, SystemClock,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, SaveBackupBackgroundRegistry,
    SaveBackupBackgroundRegistryError, SaveBackupBackgroundRegistryResult,
    SaveBackupBackgroundSettingsRepository, SaveBackupRepository,
    SaveBackupSchedulerStateRepository,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const DEFAULT_BACKUP_LIST_LIMIT: usize = 50;
const MAX_BACKUP_LIST_LIMIT: usize = 200;
const BACKGROUND_FIXTURE_PATH: [&str; 3] = ["fixtures", "background", "status.json"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupListSnapshot {
    pub game_id: String,
    pub profile_id: String,
    pub item_count: usize,
    pub items: Vec<BackupListItemSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupListItemSnapshot {
    pub backup_id: String,
    pub trigger: &'static str,
    pub status: &'static str,
    pub created_at: u64,
    pub size_bytes: u64,
    pub file_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupBackgroundStatusSnapshot {
    pub game_id: String,
    pub profile_id: String,
    pub status: &'static str,
    pub background_protection_enabled: bool,
    pub last_checked_at: Option<u64>,
    pub last_attempt_at: Option<u64>,
    pub last_success_at: Option<u64>,
    pub next_due_at: Option<u64>,
    pub pending_reason: Option<&'static str>,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnlyBackupAutomationError {
    AppDataUnavailable,
    UnsupportedGame,
    ProfileIdInvalid,
    LimitInvalid,
    SandboxStoragePathRejected,
    DatabaseUnavailable,
    BackupStateInvalid,
    BackgroundFixtureUnavailable,
    BackgroundStatusUnavailable,
}

impl ReadOnlyBackupAutomationError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::AppDataUnavailable => "app_data_unavailable",
            Self::UnsupportedGame => "unsupported_game",
            Self::ProfileIdInvalid => "profile_id_invalid",
            Self::LimitInvalid => "backup_limit_invalid",
            Self::SandboxStoragePathRejected => "sandbox_storage_path_rejected",
            Self::DatabaseUnavailable => "backup_database_unavailable",
            Self::BackupStateInvalid => "backup_state_invalid",
            Self::BackgroundFixtureUnavailable => "backup_background_fixture_unavailable",
            Self::BackgroundStatusUnavailable => "backup_background_status_unavailable",
        }
    }
}

impl fmt::Display for ReadOnlyBackupAutomationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ReadOnlyBackupAutomationError {}

enum BackgroundEnvironment {
    Production,
    Sandbox { fixture_path: PathBuf },
}

pub struct ReadOnlyBackupAutomation {
    backup_repository: Arc<dyn SaveBackupRepository>,
    scheduler_state_repository: Arc<dyn SaveBackupSchedulerStateRepository>,
    background_settings_repository: Arc<dyn SaveBackupBackgroundSettingsRepository>,
    background_environment: BackgroundEnvironment,
}

impl ReadOnlyBackupAutomation {
    pub fn from_environment(
        environment: &RuntimeEnvironment,
    ) -> Result<Self, ReadOnlyBackupAutomationError> {
        let (app_data_dir, background_environment) =
            if let Some(data_dir) = environment.sandbox_data_dir() {
                validate_sandbox_storage_paths(data_dir)?;
                (
                    data_dir.to_path_buf(),
                    BackgroundEnvironment::Sandbox {
                        fixture_path: background_fixture_path(data_dir),
                    },
                )
            } else {
                (
                    production_app_data_dir()
                        .ok_or(ReadOnlyBackupAutomationError::AppDataUnavailable)?,
                    BackgroundEnvironment::Production,
                )
            };
        let connection = open_database_read_only(&app_data_dir.join("hmm.db"))
            .map_err(|_| ReadOnlyBackupAutomationError::DatabaseUnavailable)?;
        let connection = Arc::new(Mutex::new(connection));

        Ok(Self {
            backup_repository: Arc::new(SqliteSaveBackupRepository::new(Arc::clone(&connection))),
            scheduler_state_repository: Arc::new(SqliteSaveBackupSchedulerStateRepository::new(
                Arc::clone(&connection),
            )),
            background_settings_repository: Arc::new(
                SqliteSaveBackupBackgroundSettingsRepository::new(connection),
            ),
            background_environment,
        })
    }

    pub fn list(
        &self,
        game_id: &str,
        profile_id: &str,
        limit: Option<usize>,
    ) -> Result<BackupListSnapshot, ReadOnlyBackupAutomationError> {
        let game_id = parse_game_id(game_id)?;
        let profile_id = parse_profile_id(profile_id)?;
        let limit = limit.unwrap_or(DEFAULT_BACKUP_LIST_LIMIT);
        if !(1..=MAX_BACKUP_LIST_LIMIT).contains(&limit) {
            return Err(ReadOnlyBackupAutomationError::LimitInvalid);
        }

        let summaries = self
            .backup_repository
            .list_for_profile(&game_id, &profile_id, Some(limit))
            .map_err(|_| ReadOnlyBackupAutomationError::DatabaseUnavailable)?;
        let items = summaries
            .into_iter()
            .map(project_backup_summary)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(BackupListSnapshot {
            game_id: game_id.as_str().to_owned(),
            profile_id: profile_id.as_str().to_owned(),
            item_count: items.len(),
            items,
        })
    }

    pub fn background_status(
        &self,
        game_id: &str,
        profile_id: &str,
    ) -> Result<BackupBackgroundStatusSnapshot, ReadOnlyBackupAutomationError> {
        let game_id = parse_game_id(game_id)?;
        let profile_id = parse_profile_id(profile_id)?;
        let (registry, clock): (Arc<dyn SaveBackupBackgroundRegistry>, Arc<dyn AppClock>) =
            match &self.background_environment {
                BackgroundEnvironment::Production => production_background_dependencies(),
                BackgroundEnvironment::Sandbox { fixture_path } => {
                    let fixture = read_background_fixture(fixture_path)?;
                    (
                        Arc::new(FixedBackgroundRegistry {
                            status: fixture.registration_status.into(),
                        }),
                        Arc::new(FixedClock {
                            now_unix_millis: fixture.now_unix_millis,
                        }),
                    )
                }
            };
        let service = SaveBackupBackgroundService::new_with_settings_repository(
            registry,
            Arc::clone(&self.scheduler_state_repository),
            Arc::clone(&self.background_settings_repository),
            Arc::new(ReadOnlyAuditLog),
            clock,
        );
        let status = service
            .status(&game_id, &profile_id)
            .map_err(|_| ReadOnlyBackupAutomationError::BackgroundStatusUnavailable)?;

        project_background_status(&game_id, &profile_id, status)
    }
}

fn project_backup_summary(
    summary: SaveBackupSummary,
) -> Result<BackupListItemSnapshot, ReadOnlyBackupAutomationError> {
    if !is_safe_backup_id(&summary.backup_id)
        || summary.created_at > u64::MAX as u128
        || summary.archive_size_bytes > i64::MAX as u64
        || summary.file_count > i32::MAX as u32
    {
        return Err(ReadOnlyBackupAutomationError::BackupStateInvalid);
    }

    Ok(BackupListItemSnapshot {
        backup_id: summary.backup_id,
        trigger: summary.trigger.as_str(),
        status: summary.status.as_str(),
        created_at: summary.created_at as u64,
        size_bytes: summary.archive_size_bytes,
        file_count: summary.file_count,
    })
}

fn project_background_status(
    game_id: &GameId,
    profile_id: &ProfileId,
    background: SaveBackupBackgroundStatus,
) -> Result<BackupBackgroundStatusSnapshot, ReadOnlyBackupAutomationError> {
    let SaveBackupBackgroundStatus {
        scheduler_state,
        status,
        last_error_code,
    } = background;
    if last_error_code
        .as_deref()
        .is_some_and(|code| !is_safe_error_code(code))
    {
        return Err(ReadOnlyBackupAutomationError::BackupStateInvalid);
    }

    let Some(state) = scheduler_state else {
        return Ok(BackupBackgroundStatusSnapshot {
            game_id: game_id.as_str().to_owned(),
            profile_id: profile_id.as_str().to_owned(),
            status: status.as_str(),
            background_protection_enabled: false,
            last_checked_at: None,
            last_attempt_at: None,
            last_success_at: None,
            next_due_at: None,
            pending_reason: None,
            last_error_code,
        });
    };
    if state.game_id != *game_id
        || state.profile_id != *profile_id
        || !timestamps_fit_u64([
            state.last_checked_at,
            state.last_attempt_at,
            state.last_success_at,
            state.next_due_at,
        ])
    {
        return Err(ReadOnlyBackupAutomationError::BackupStateInvalid);
    }

    Ok(BackupBackgroundStatusSnapshot {
        game_id: game_id.as_str().to_owned(),
        profile_id: profile_id.as_str().to_owned(),
        status: status.as_str(),
        background_protection_enabled: state.background_protection_enabled,
        last_checked_at: state.last_checked_at.map(|value| value as u64),
        last_attempt_at: state.last_attempt_at.map(|value| value as u64),
        last_success_at: state.last_success_at.map(|value| value as u64),
        next_due_at: state.next_due_at.map(|value| value as u64),
        pending_reason: state.pending_reason.map(pending_reason_code),
        last_error_code,
    })
}

fn validate_sandbox_storage_paths(data_dir: &Path) -> Result<(), ReadOnlyBackupAutomationError> {
    if !data_dir.is_dir() || !is_canonically_within(data_dir, data_dir) {
        return Err(ReadOnlyBackupAutomationError::SandboxStoragePathRejected);
    }
    let database_path = data_dir.join("hmm.db");
    let managed_paths = [
        database_path.clone(),
        sqlite_sidecar_path(&database_path, "-wal"),
        sqlite_sidecar_path(&database_path, "-shm"),
        background_fixture_path(data_dir),
    ];
    if managed_paths
        .iter()
        .any(|path| !is_canonically_within(path, data_dir))
    {
        return Err(ReadOnlyBackupAutomationError::SandboxStoragePathRejected);
    }
    Ok(())
}

fn sqlite_sidecar_path(database_path: &Path, suffix: &str) -> PathBuf {
    let mut sidecar = database_path.as_os_str().to_os_string();
    sidecar.push(suffix);
    sidecar.into()
}

fn background_fixture_path(data_dir: &Path) -> PathBuf {
    BACKGROUND_FIXTURE_PATH
        .iter()
        .fold(data_dir.to_path_buf(), |path, component| {
            path.join(component)
        })
}

fn parse_game_id(value: &str) -> Result<GameId, ReadOnlyBackupAutomationError> {
    let game_id =
        GameId::parse(value).map_err(|_| ReadOnlyBackupAutomationError::UnsupportedGame)?;
    if game_id.as_str() != "mhw" {
        return Err(ReadOnlyBackupAutomationError::UnsupportedGame);
    }
    Ok(game_id)
}

fn parse_profile_id(value: &str) -> Result<ProfileId, ReadOnlyBackupAutomationError> {
    let value = value.trim();
    if !is_safe_short_id(value) {
        return Err(ReadOnlyBackupAutomationError::ProfileIdInvalid);
    }
    Ok(ProfileId::new(value))
}

fn is_safe_short_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn is_safe_backup_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':'))
}

fn is_safe_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn timestamps_fit_u64(values: [Option<u128>; 4]) -> bool {
    values
        .into_iter()
        .flatten()
        .all(|value| value <= u64::MAX as u128)
}

fn pending_reason_code(reason: SaveBackupSchedulerPendingReason) -> &'static str {
    reason.as_str()
}

fn production_background_dependencies() -> (Arc<dyn SaveBackupBackgroundRegistry>, Arc<dyn AppClock>)
{
    #[cfg(target_os = "windows")]
    let registry: Arc<dyn SaveBackupBackgroundRegistry> =
        Arc::new(WindowsScheduledTaskRegistry::from_current_exe());
    #[cfg(not(target_os = "windows"))]
    let registry: Arc<dyn SaveBackupBackgroundRegistry> =
        Arc::new(UnsupportedSaveBackupBackgroundRegistry);

    (registry, Arc::new(SystemClock))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackgroundFixture {
    registration_status: BackgroundFixtureRegistrationStatus,
    now_unix_millis: u128,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BackgroundFixtureRegistrationStatus {
    NotRegistered,
    Registered,
    ConfigurationDrift,
    RegistrationFailed,
    PermissionRequired,
    UnsupportedPlatform,
}

impl From<BackgroundFixtureRegistrationStatus> for SaveBackupBackgroundRegistrationStatus {
    fn from(value: BackgroundFixtureRegistrationStatus) -> Self {
        match value {
            BackgroundFixtureRegistrationStatus::NotRegistered => Self::NotRegistered,
            BackgroundFixtureRegistrationStatus::Registered => Self::Registered,
            BackgroundFixtureRegistrationStatus::ConfigurationDrift => Self::ConfigurationDrift,
            BackgroundFixtureRegistrationStatus::RegistrationFailed => Self::RegistrationFailed,
            BackgroundFixtureRegistrationStatus::PermissionRequired => Self::PermissionRequired,
            BackgroundFixtureRegistrationStatus::UnsupportedPlatform => Self::UnsupportedPlatform,
        }
    }
}

fn read_background_fixture(
    fixture_path: &Path,
) -> Result<BackgroundFixture, ReadOnlyBackupAutomationError> {
    let bytes = fs::read(fixture_path)
        .map_err(|_| ReadOnlyBackupAutomationError::BackgroundFixtureUnavailable)?;
    serde_json::from_slice(&bytes)
        .map_err(|_| ReadOnlyBackupAutomationError::BackgroundFixtureUnavailable)
}

struct FixedBackgroundRegistry {
    status: SaveBackupBackgroundRegistrationStatus,
}

impl SaveBackupBackgroundRegistry for FixedBackgroundRegistry {
    fn inspect(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        Ok(self.status)
    }

    fn register(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        Err(SaveBackupBackgroundRegistryError::OperationFailed)
    }

    fn unregister(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        Err(SaveBackupBackgroundRegistryError::OperationFailed)
    }
}

struct FixedClock {
    now_unix_millis: u128,
}

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(self.now_unix_millis)
    }
}

struct ReadOnlyAuditLog;

impl AuditLogWriter for ReadOnlyAuditLog {
    fn record(&self, _event: AuditLogEvent) -> Result<()> {
        anyhow::bail!("read-only backup automation cannot write audit events")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_infra::open_database;
    use rusqlite::params;

    fn sandbox_environment(root: &Path) -> RuntimeEnvironment {
        RuntimeEnvironment::sandbox(root.to_path_buf()).expect("sandbox environment")
    }

    fn create_database(root: &Path) {
        let connection = open_database(&root.join("hmm.db")).expect("create fixture database");
        connection
            .execute(
                "INSERT INTO save_backups (
                    backup_id, game_id, profile_id, trigger, status, archive_file_name,
                    manifest_file_name, archive_size_bytes, archive_sha256, file_count,
                    created_at, source_path_label, source_path_hash, notes,
                    backup_directory_mode, backup_directory
                 ) VALUES (?1, 'mhw', 'default', 'manual', 'completed', 'private.zip',
                    'private.manifest.json', 42, 'sha256:private', 2, 1000,
                    '582010/remote', 'sha256:source', 'private note', 'custom',
                    'C:/Users/Player/private')",
                params!["mhw:profile-default:1000:manual"],
            )
            .expect("insert backup fixture");
        connection
            .execute(
                "INSERT INTO save_backup_scheduler_state (
                    game_id, profile_id, enabled, background_protection_enabled,
                    background_status, last_checked_at, last_attempt_at, last_success_at,
                    next_due_at, pending_reason, last_error_code, worker_instance_id,
                    worker_heartbeat_at, lease_owner, lease_expires_at, updated_at
                 ) VALUES (
                    'mhw', 'default', 1, 1, 'protected', 900, 910, 920, 2000,
                    NULL, NULL, 'private-worker', 950, 'private-lease', 3000, 950
                 )",
                [],
            )
            .expect("insert scheduler fixture");
        connection
            .execute(
                "INSERT INTO save_backup_background_settings (
                    singleton_id, desired_enabled, enabled_at,
                    last_worker_heartbeat_at, updated_at
                 ) VALUES (1, 1, 800, 950, 950)",
                [],
            )
            .expect("insert background settings fixture");
    }

    fn write_background_fixture(root: &Path) {
        let fixture_path = background_fixture_path(root);
        fs::create_dir_all(fixture_path.parent().expect("fixture parent"))
            .expect("create fixture parent");
        fs::write(
            fixture_path,
            r#"{"registrationStatus":"registered","nowUnixMillis":1000}"#,
        )
        .expect("write background fixture");
    }

    #[test]
    fn list_projects_only_safe_backup_summary_fields_without_writing() {
        let temp = tempfile::tempdir().expect("temp dir");
        create_database(temp.path());
        let before = fs::read(temp.path().join("hmm.db")).expect("read database before");
        let automation =
            ReadOnlyBackupAutomation::from_environment(&sandbox_environment(temp.path()))
                .expect("read-only backup automation");

        let snapshot = automation
            .list("mhw", "default", Some(10))
            .expect("list backups");

        assert_eq!(snapshot.item_count, 1);
        assert_eq!(
            snapshot.items[0].backup_id,
            "mhw:profile-default:1000:manual"
        );
        let serialized = serde_json::to_string(&snapshot).expect("serialize snapshot");
        for forbidden in [
            "private.zip",
            "private.manifest.json",
            "sha256:private",
            "582010/remote",
            "private note",
            "C:/Users/Player/private",
        ] {
            assert!(!serialized.contains(forbidden));
        }
        drop(automation);
        assert_eq!(
            fs::read(temp.path().join("hmm.db")).expect("read database after"),
            before
        );
    }

    #[test]
    fn background_status_uses_fixed_fixture_without_exposing_worker_or_lease() {
        let temp = tempfile::tempdir().expect("temp dir");
        create_database(temp.path());
        write_background_fixture(temp.path());
        let automation =
            ReadOnlyBackupAutomation::from_environment(&sandbox_environment(temp.path()))
                .expect("read-only backup automation");

        let snapshot = automation
            .background_status("mhw", "default")
            .expect("background status");

        assert_eq!(snapshot.status, "protected");
        assert_eq!(snapshot.last_checked_at, Some(900));
        let serialized = serde_json::to_string(&snapshot).expect("serialize snapshot");
        assert!(!serialized.contains("private-worker"));
        assert!(!serialized.contains("private-lease"));
        assert!(!serialized.contains("lease"));
    }

    #[test]
    fn path_like_profile_and_tampered_backup_id_fail_closed() {
        let temp = tempfile::tempdir().expect("temp dir");
        create_database(temp.path());
        let connection =
            rusqlite::Connection::open(temp.path().join("hmm.db")).expect("open fixture database");
        connection
            .execute(
                "UPDATE save_backups SET backup_id = '../private' WHERE profile_id = 'default'",
                [],
            )
            .expect("tamper backup id");
        drop(connection);
        let automation =
            ReadOnlyBackupAutomation::from_environment(&sandbox_environment(temp.path()))
                .expect("read-only backup automation");

        assert_eq!(
            automation.list("mhw", "../private", Some(10)),
            Err(ReadOnlyBackupAutomationError::ProfileIdInvalid)
        );
        assert_eq!(
            automation.list("mhw", "default", Some(10)),
            Err(ReadOnlyBackupAutomationError::BackupStateInvalid)
        );
    }

    #[test]
    #[cfg(unix)]
    fn sandbox_rejects_database_sidecar_symlink_escape() {
        let temp = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside dir");
        create_database(temp.path());
        let outside_wal = outside.path().join("outside-wal");
        fs::write(&outside_wal, b"outside").expect("create outside WAL");
        let database_path = temp.path().join("hmm.db");
        std::os::unix::fs::symlink(&outside_wal, sqlite_sidecar_path(&database_path, "-wal"))
            .expect("create WAL symlink");

        assert_eq!(
            ReadOnlyBackupAutomation::from_environment(&sandbox_environment(temp.path()))
                .map(|_| ()),
            Err(ReadOnlyBackupAutomationError::SandboxStoragePathRejected)
        );
    }
}
