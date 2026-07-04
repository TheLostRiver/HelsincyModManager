use hmm_core::{SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartSaveBackupTaskRequestDto {
    pub game_id: String,
    pub profile_id: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSaveBackupsRequestDto {
    pub game_id: String,
    pub profile_id: String,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBackupSummaryDto {
    pub backup_id: String,
    pub game_id: String,
    pub profile_id: String,
    pub trigger: SaveBackupTriggerDto,
    pub status: SaveBackupStatusDto,
    pub file_name: String,
    pub created_at: u64,
    pub size_bytes: u64,
    pub file_count: u32,
    pub source_path_label: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveBackupTriggerDto {
    Manual,
    Auto,
    PreInstall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveBackupStatusDto {
    Completed,
    DeletedByRetention,
    Missing,
    Invalid,
}

impl From<SaveBackupSummary> for SaveBackupSummaryDto {
    fn from(summary: SaveBackupSummary) -> Self {
        Self {
            backup_id: summary.backup_id,
            game_id: summary.game_id.as_str().to_owned(),
            profile_id: summary.profile_id.as_str().to_owned(),
            trigger: summary.trigger.into(),
            status: summary.status.into(),
            file_name: summary.archive_file_name,
            created_at: summary.created_at as u64,
            size_bytes: summary.archive_size_bytes,
            file_count: summary.file_count,
            source_path_label: summary.source_path_label,
            notes: summary.notes,
        }
    }
}

impl From<SaveBackupTrigger> for SaveBackupTriggerDto {
    fn from(trigger: SaveBackupTrigger) -> Self {
        match trigger {
            SaveBackupTrigger::Manual => Self::Manual,
            SaveBackupTrigger::Auto => Self::Auto,
            SaveBackupTrigger::PreInstall => Self::PreInstall,
        }
    }
}

impl From<SaveBackupStatus> for SaveBackupStatusDto {
    fn from(status: SaveBackupStatus) -> Self {
        match status {
            SaveBackupStatus::Completed => Self::Completed,
            SaveBackupStatus::DeletedByRetention => Self::DeletedByRetention,
            SaveBackupStatus::Missing => Self::Missing,
            SaveBackupStatus::Invalid => Self::Invalid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_save_backup_task_request_deserializes_without_paths() {
        let value = serde_json::json!({
            "gameId": "mhw",
            "profileId": "default",
            "note": "before hunt"
        });

        let request: StartSaveBackupTaskRequestDto =
            serde_json::from_value(value).expect("deserialize request");

        assert_eq!(request.game_id, "mhw");
        assert_eq!(request.profile_id, "default");
        assert_eq!(request.note.as_deref(), Some("before hunt"));
    }

    #[test]
    fn serializes_save_backup_summary_without_paths_or_hash_lists() {
        let dto: SaveBackupSummaryDto = hmm_core::SaveBackupSummary {
            backup_id: "backup-1".to_owned(),
            game_id: hmm_core::GameId::mhw(),
            profile_id: hmm_core::ProfileId::new("default"),
            trigger: hmm_core::SaveBackupTrigger::Manual,
            status: hmm_core::SaveBackupStatus::Completed,
            archive_file_name: "20260704-221530_mhw_profile-default_manual.zip".to_owned(),
            manifest_file_name: "20260704-221530_mhw_profile-default_manual.manifest.json"
                .to_owned(),
            archive_size_bytes: 128,
            archive_sha256: "sha256:archive".to_owned(),
            file_count: 1,
            created_at: 42,
            source_path_label: Some("remote".to_owned()),
            source_path_hash: "sha256:source".to_owned(),
            notes: Some("manual note".to_owned()),
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize summary");

        assert_eq!(value["backupId"], "backup-1");
        assert_eq!(value["gameId"], "mhw");
        assert_eq!(value["profileId"], "default");
        assert_eq!(value["trigger"], "manual");
        assert_eq!(value["status"], "completed");
        assert_eq!(
            value["fileName"],
            "20260704-221530_mhw_profile-default_manual.zip"
        );
        assert_eq!(value["createdAt"], 42);
        assert_eq!(value["sizeBytes"], 128);
        assert_eq!(value["fileCount"], 1);
        assert_eq!(value["sourcePathLabel"], "remote");
        assert_eq!(value["notes"], "manual note");
        assert!(value.get("archiveSha256").is_none());
        assert!(value.get("sourcePathHash").is_none());
        assert!(value.get("manifest").is_none());
        assert!(value.get("path").is_none());
        assert!(!value.to_string().contains("C:/"));
        assert!(!value.to_string().contains("sha256:"));
    }
}
