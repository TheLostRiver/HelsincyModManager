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
