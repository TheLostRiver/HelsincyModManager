use crate::dto::TaskStartedDto;
use hmm_app::{SaveBackupAutoCheckResult, SaveBackupAutoCheckStatus};
use hmm_core::{
    GameId, ProfileId, SaveBackupBackgroundProtectionStatus, SaveBackupSchedulerPendingReason,
    SaveBackupSchedulerState, SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
};
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckAutoSaveBackupRequestDto {
    pub game_id: String,
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileAutoSaveBackupCheckDto {
    pub game_id: String,
    pub profile_id: String,
    pub client_runtime_only: bool,
    pub status: ProfileAutoSaveBackupCheckStatusDto,
    pub checked_at: u64,
    pub last_due_at: Option<u64>,
    pub next_due_at: Option<u64>,
    pub last_auto_backup_at: Option<u64>,
    pub started_task: Option<TaskStartedDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileAutoSaveBackupCheckStatusDto {
    ManualOnly,
    NotDue,
    Due,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSaveBackupBackgroundStatusRequestDto {
    pub game_id: String,
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBackupBackgroundStatusDto {
    pub game_id: String,
    pub profile_id: String,
    pub status: SaveBackupBackgroundStatusKindDto,
    pub background_protection_enabled: bool,
    pub last_checked_at: Option<u64>,
    pub last_attempt_at: Option<u64>,
    pub last_success_at: Option<u64>,
    pub next_due_at: Option<u64>,
    pub pending_reason: Option<SaveBackupPendingReasonDto>,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveBackupBackgroundStatusKindDto {
    Protected,
    TrayOnly,
    NotEnabled,
    RegistrationFailed,
    WorkerUnhealthy,
    PermissionRequired,
    UnsupportedPlatform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveBackupPendingReasonDto {
    GameRunning,
    GameRunningUnknown,
    SourceInvalid,
    DestinationUnavailable,
    TaskConflict,
}

impl SaveBackupBackgroundStatusDto {
    pub fn from_state(
        game_id: &GameId,
        profile_id: &ProfileId,
        state: Option<SaveBackupSchedulerState>,
    ) -> Self {
        // 白名单映射：lease_owner、lease_expires_at、worker_instance_id 和任何路径
        // 都不得进入前端 DTO。
        match state {
            Some(state) => Self {
                game_id: game_id.as_str().to_owned(),
                profile_id: profile_id.as_str().to_owned(),
                status: state.background_status.into(),
                background_protection_enabled: state.background_protection_enabled,
                last_checked_at: state.last_checked_at.map(|value| value as u64),
                last_attempt_at: state.last_attempt_at.map(|value| value as u64),
                last_success_at: state.last_success_at.map(|value| value as u64),
                next_due_at: state.next_due_at.map(|value| value as u64),
                pending_reason: state.pending_reason.map(Into::into),
                last_error_code: state.last_error_code,
            },
            None => Self {
                game_id: game_id.as_str().to_owned(),
                profile_id: profile_id.as_str().to_owned(),
                status: SaveBackupBackgroundStatusKindDto::NotEnabled,
                background_protection_enabled: false,
                last_checked_at: None,
                last_attempt_at: None,
                last_success_at: None,
                next_due_at: None,
                pending_reason: None,
                last_error_code: None,
            },
        }
    }
}

impl From<SaveBackupBackgroundProtectionStatus> for SaveBackupBackgroundStatusKindDto {
    fn from(status: SaveBackupBackgroundProtectionStatus) -> Self {
        match status {
            SaveBackupBackgroundProtectionStatus::Protected => Self::Protected,
            SaveBackupBackgroundProtectionStatus::TrayOnly => Self::TrayOnly,
            SaveBackupBackgroundProtectionStatus::NotEnabled => Self::NotEnabled,
            SaveBackupBackgroundProtectionStatus::RegistrationFailed => Self::RegistrationFailed,
            SaveBackupBackgroundProtectionStatus::WorkerUnhealthy => Self::WorkerUnhealthy,
            SaveBackupBackgroundProtectionStatus::PermissionRequired => Self::PermissionRequired,
            SaveBackupBackgroundProtectionStatus::UnsupportedPlatform => Self::UnsupportedPlatform,
        }
    }
}

impl From<SaveBackupSchedulerPendingReason> for SaveBackupPendingReasonDto {
    fn from(reason: SaveBackupSchedulerPendingReason) -> Self {
        match reason {
            SaveBackupSchedulerPendingReason::GameRunning => Self::GameRunning,
            SaveBackupSchedulerPendingReason::GameRunningUnknown => Self::GameRunningUnknown,
            SaveBackupSchedulerPendingReason::SourceInvalid => Self::SourceInvalid,
            SaveBackupSchedulerPendingReason::DestinationUnavailable => {
                Self::DestinationUnavailable
            }
            SaveBackupSchedulerPendingReason::TaskConflict => Self::TaskConflict,
        }
    }
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

impl ProfileAutoSaveBackupCheckDto {
    pub fn from_result(
        result: SaveBackupAutoCheckResult,
        started_task: Option<TaskStartedDto>,
    ) -> Self {
        Self {
            game_id: result.game_id.as_str().to_owned(),
            profile_id: result.profile_id.as_str().to_owned(),
            client_runtime_only: true,
            status: result.status.into(),
            checked_at: result.checked_at as u64,
            last_due_at: result.last_due_at.map(|value| value as u64),
            next_due_at: result.next_due_at.map(|value| value as u64),
            last_auto_backup_at: result.last_auto_backup_at.map(|value| value as u64),
            started_task,
        }
    }
}

impl From<SaveBackupAutoCheckStatus> for ProfileAutoSaveBackupCheckStatusDto {
    fn from(status: SaveBackupAutoCheckStatus) -> Self {
        match status {
            SaveBackupAutoCheckStatus::ManualOnly => Self::ManualOnly,
            SaveBackupAutoCheckStatus::NotDue => Self::NotDue,
            SaveBackupAutoCheckStatus::Due => Self::Due,
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
    fn serializes_auto_save_backup_check_without_paths_or_scheduler_state() {
        let dto = ProfileAutoSaveBackupCheckDto {
            game_id: "mhw".to_owned(),
            profile_id: "default".to_owned(),
            client_runtime_only: true,
            status: ProfileAutoSaveBackupCheckStatusDto::Due,
            checked_at: 42,
            last_due_at: Some(40),
            next_due_at: Some(80),
            last_auto_backup_at: Some(10),
            started_task: Some(TaskStartedDto {
                task_id: "save-backup-1".to_owned(),
                kind: crate::dto::TaskKindDto::SaveBackup,
                status: crate::dto::TaskStatusDto::Queued,
            }),
        };

        let value = serde_json::to_value(dto).expect("serialize auto check");

        assert_eq!(value["gameId"], "mhw");
        assert_eq!(value["profileId"], "default");
        assert_eq!(value["clientRuntimeOnly"], true);
        assert_eq!(value["status"], "due");
        assert_eq!(value["checkedAt"], 42);
        assert_eq!(value["lastDueAt"], 40);
        assert_eq!(value["nextDueAt"], 80);
        assert_eq!(value["lastAutoBackupAt"], 10);
        assert_eq!(value["startedTask"]["taskId"], "save-backup-1");
        assert!(value.get("path").is_none());
        assert!(value.get("manifest").is_none());
        assert!(value.get("backupRef").is_none());
        assert!(value.get("hash").is_none());
        assert!(!value.to_string().contains("C:/"));
    }

    #[test]
    fn serializes_background_status_without_lease_worker_or_paths() {
        let dto = SaveBackupBackgroundStatusDto::from_state(
            &hmm_core::GameId::mhw(),
            &hmm_core::ProfileId::new("default"),
            Some(hmm_core::SaveBackupSchedulerState {
                game_id: hmm_core::GameId::mhw(),
                profile_id: hmm_core::ProfileId::new("default"),
                enabled: true,
                background_protection_enabled: false,
                background_status: SaveBackupBackgroundProtectionStatus::TrayOnly,
                last_checked_at: Some(100),
                last_attempt_at: Some(90),
                last_success_at: Some(80),
                next_due_at: Some(200),
                pending_reason: Some(SaveBackupSchedulerPendingReason::GameRunning),
                last_error_code: Some("save_backup_auto_skipped_game_running".to_owned()),
                worker_instance_id: Some("worker-a".to_owned()),
                lease_owner: Some("client-runtime:mhw:default:1".to_owned()),
                lease_expires_at: Some(300),
                updated_at: 100,
            }),
        );

        let value = serde_json::to_value(dto).expect("serialize background status");

        assert_eq!(value["gameId"], "mhw");
        assert_eq!(value["profileId"], "default");
        assert_eq!(value["status"], "tray_only");
        assert_eq!(value["backgroundProtectionEnabled"], false);
        assert_eq!(value["lastCheckedAt"], 100);
        assert_eq!(value["lastAttemptAt"], 90);
        assert_eq!(value["lastSuccessAt"], 80);
        assert_eq!(value["nextDueAt"], 200);
        assert_eq!(value["pendingReason"], "game_running");
        assert_eq!(value["lastErrorCode"], "save_backup_auto_skipped_game_running");
        assert!(value.get("leaseOwner").is_none());
        assert!(value.get("leaseExpiresAt").is_none());
        assert!(value.get("workerInstanceId").is_none());
        assert!(value.get("enabled").is_none());
        assert!(value.get("path").is_none());
        assert!(!value.to_string().contains("worker-a"));
        assert!(!value.to_string().contains("client-runtime:"));
        assert!(!value.to_string().contains("C:/"));
    }

    #[test]
    fn missing_background_state_maps_to_not_enabled() {
        let dto = SaveBackupBackgroundStatusDto::from_state(
            &hmm_core::GameId::mhw(),
            &hmm_core::ProfileId::new("default"),
            None,
        );

        let value = serde_json::to_value(dto).expect("serialize missing state");

        assert_eq!(value["status"], "not_enabled");
        assert_eq!(value["backgroundProtectionEnabled"], false);
        assert_eq!(value["lastCheckedAt"], serde_json::Value::Null);
        assert_eq!(value["pendingReason"], serde_json::Value::Null);
        assert_eq!(value["lastErrorCode"], serde_json::Value::Null);
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
            backup_directory: hmm_core::ProfileDirectorySelection {
                mode: hmm_core::ProfileDirectoryMode::Custom,
                status: hmm_core::ProfileDirectoryStatus::Valid,
                directory: Some("D:/Backups".to_owned()),
                path_label: Some("Backups".to_owned()),
                messages: Vec::new(),
            },
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
        assert!(value.get("backupDirectory").is_none());
        assert!(value.get("manifest").is_none());
        assert!(value.get("path").is_none());
        assert!(!value.to_string().contains("D:/Backups"));
        assert!(!value.to_string().contains("C:/"));
        assert!(!value.to_string().contains("sha256:"));
    }
}
