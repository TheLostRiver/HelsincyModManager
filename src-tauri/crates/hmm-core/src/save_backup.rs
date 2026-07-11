use crate::{GameId, ProfileDirectorySelection, ProfileId};
use serde::{Deserialize, Serialize};

pub const SAVE_BACKUP_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveBackupTrigger {
    Manual,
    Auto,
    PreInstall,
}

impl SaveBackupTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
            Self::PreInstall => "pre_install",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveBackupStatus {
    Completed,
    DeletedByRetention,
    Missing,
    Invalid,
}

impl SaveBackupStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::DeletedByRetention => "deleted_by_retention",
            Self::Missing => "missing",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupSummary {
    pub backup_id: String,
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub trigger: SaveBackupTrigger,
    pub status: SaveBackupStatus,
    pub archive_file_name: String,
    pub manifest_file_name: String,
    pub archive_size_bytes: u64,
    pub archive_sha256: String,
    pub file_count: u32,
    pub created_at: u128,
    pub source_path_label: Option<String>,
    pub source_path_hash: String,
    pub backup_directory: ProfileDirectorySelection,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveBackupBackgroundRegistrationStatus {
    NotRegistered,
    Registered,
    ConfigurationDrift,
    RegistrationFailed,
    PermissionRequired,
    UnsupportedPlatform,
}

impl SaveBackupBackgroundRegistrationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRegistered => "not_registered",
            Self::Registered => "registered",
            Self::ConfigurationDrift => "configuration_drift",
            Self::RegistrationFailed => "registration_failed",
            Self::PermissionRequired => "permission_required",
            Self::UnsupportedPlatform => "unsupported_platform",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveBackupBackgroundProtectionStatus {
    Protected,
    TrayOnly,
    NotEnabled,
    Starting,
    RegistrationFailed,
    WorkerUnhealthy,
    PermissionRequired,
    UnsupportedPlatform,
}

impl SaveBackupBackgroundProtectionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Protected => "protected",
            Self::TrayOnly => "tray_only",
            Self::NotEnabled => "not_enabled",
            Self::Starting => "starting",
            Self::RegistrationFailed => "registration_failed",
            Self::WorkerUnhealthy => "worker_unhealthy",
            Self::PermissionRequired => "permission_required",
            Self::UnsupportedPlatform => "unsupported_platform",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupBackgroundSettings {
    pub desired_enabled: bool,
    pub enabled_at: Option<u128>,
    pub last_worker_heartbeat_at: Option<u128>,
    pub updated_at: u128,
}

impl SaveBackupBackgroundSettings {
    pub fn disabled() -> Self {
        Self {
            desired_enabled: false,
            enabled_at: None,
            last_worker_heartbeat_at: None,
            updated_at: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveBackupSchedulerPendingReason {
    GameRunning,
    GameRunningUnknown,
    SourceInvalid,
    DestinationUnavailable,
    TaskConflict,
}

impl SaveBackupSchedulerPendingReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GameRunning => "game_running",
            Self::GameRunningUnknown => "game_running_unknown",
            Self::SourceInvalid => "source_invalid",
            Self::DestinationUnavailable => "destination_unavailable",
            Self::TaskConflict => "task_conflict",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupSchedulerState {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub enabled: bool,
    pub background_protection_enabled: bool,
    pub background_status: SaveBackupBackgroundProtectionStatus,
    pub last_checked_at: Option<u128>,
    pub last_attempt_at: Option<u128>,
    pub last_success_at: Option<u128>,
    pub next_due_at: Option<u128>,
    pub pending_reason: Option<SaveBackupSchedulerPendingReason>,
    pub last_error_code: Option<String>,
    pub worker_instance_id: Option<String>,
    pub worker_heartbeat_at: Option<u128>,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<u128>,
    pub updated_at: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupSchedulerLeaseRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub lease_owner: String,
    pub lease_expires_at: u128,
    pub now_unix_millis: u128,
    pub last_checked_at: Option<u128>,
    pub next_due_at: Option<u128>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupSchedulerLeaseRenewalRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub lease_owner: String,
    pub lease_expires_at: u128,
    pub now_unix_millis: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupWorkerHeartbeat {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub worker_instance_id: String,
    pub heartbeat_at: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBackupManifest {
    pub schema_version: u32,
    pub backup_id: String,
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub trigger: SaveBackupTrigger,
    pub created_at_utc: String,
    pub created_at_utc_label: String,
    pub archive_file_name: String,
    pub archive_size_bytes: u64,
    pub archive_sha256: String,
    pub source: SaveBackupManifestSource,
    pub files: Vec<SaveBackupManifestFile>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBackupManifestSource {
    pub mode: String,
    pub path_label: Option<String>,
    pub path_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBackupManifestFile {
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub modified_at_utc: Option<String>,
}

impl SaveBackupManifest {
    pub fn new(
        backup_id: impl Into<String>,
        game_id: GameId,
        profile_id: ProfileId,
        trigger: SaveBackupTrigger,
        created_at_utc: impl Into<String>,
        created_at_utc_label: impl Into<String>,
        archive_file_name: impl Into<String>,
        archive_size_bytes: u64,
        archive_sha256: impl Into<String>,
        source: SaveBackupManifestSource,
        files: Vec<SaveBackupManifestFile>,
        notes: Option<String>,
    ) -> Self {
        Self {
            schema_version: SAVE_BACKUP_MANIFEST_SCHEMA_VERSION,
            backup_id: backup_id.into(),
            game_id,
            profile_id,
            trigger,
            created_at_utc: created_at_utc.into(),
            created_at_utc_label: created_at_utc_label.into(),
            archive_file_name: archive_file_name.into(),
            archive_size_bytes,
            archive_sha256: archive_sha256.into(),
            source,
            files,
            notes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_control_statuses_have_stable_codes() {
        assert_eq!(
            SaveBackupBackgroundProtectionStatus::Starting.as_str(),
            "starting"
        );

        let state = SaveBackupBackgroundSettings::disabled();
        assert!(!state.desired_enabled);
        assert_eq!(state.enabled_at, None);
        assert_eq!(state.last_worker_heartbeat_at, None);
    }

    #[test]
    fn background_registration_statuses_have_stable_codes() {
        assert_eq!(
            SaveBackupBackgroundRegistrationStatus::NotRegistered.as_str(),
            "not_registered"
        );
        assert_eq!(
            SaveBackupBackgroundRegistrationStatus::Registered.as_str(),
            "registered"
        );
        assert_eq!(
            SaveBackupBackgroundRegistrationStatus::ConfigurationDrift.as_str(),
            "configuration_drift"
        );
        assert_eq!(
            SaveBackupBackgroundRegistrationStatus::RegistrationFailed.as_str(),
            "registration_failed"
        );
        assert_eq!(
            SaveBackupBackgroundRegistrationStatus::PermissionRequired.as_str(),
            "permission_required"
        );
        assert_eq!(
            SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform.as_str(),
            "unsupported_platform"
        );
    }
}
