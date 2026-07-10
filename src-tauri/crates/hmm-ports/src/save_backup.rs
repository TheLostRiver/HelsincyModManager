use anyhow::Result;
use hmm_core::{
    GameId, ProfileBackupRetention, ProfileDirectorySelection, ProfileId,
    SaveBackupBackgroundRegistrationStatus, SaveBackupSchedulerLeaseRenewalRequest,
    SaveBackupSchedulerLeaseRequest, SaveBackupSchedulerState, SaveBackupStatus, SaveBackupSummary,
    SaveBackupTrigger, SaveBackupWorkerHeartbeat,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupWriteRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub trigger: SaveBackupTrigger,
    pub source_directory: Option<String>,
    pub source_directory_selection: ProfileDirectorySelection,
    pub backup_directory: ProfileDirectorySelection,
    pub retention: ProfileBackupRetention,
    pub note: Option<String>,
    pub created_at_unix_millis: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupWriteResult {
    pub summary: SaveBackupSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveBackupBackgroundRegistryError {
    TaskOwnershipConflict,
    WorkerBinaryUnavailable,
    CommandTimeout,
    CommandInvalidOutput,
    OperationFailed,
}

impl SaveBackupBackgroundRegistryError {
    pub fn code(self) -> &'static str {
        match self {
            Self::TaskOwnershipConflict => "save_backup_background_task_ownership_conflict",
            Self::WorkerBinaryUnavailable => "save_backup_background_worker_binary_unavailable",
            Self::CommandTimeout => "save_backup_background_command_timeout",
            Self::CommandInvalidOutput => "save_backup_background_command_invalid_output",
            Self::OperationFailed => "save_backup_background_registration_failed",
        }
    }
}

impl std::fmt::Display for SaveBackupBackgroundRegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SaveBackupBackgroundRegistryError {}

pub type SaveBackupBackgroundRegistryResult<T> =
    std::result::Result<T, SaveBackupBackgroundRegistryError>;

pub const SAVE_BACKUP_BACKGROUND_REGISTRY_SCHEMA_VERSION: u32 = 1;

pub trait SaveBackupBackgroundRegistry: Send + Sync {
    fn inspect(&self)
        -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>;

    fn register(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>;

    fn unregister(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>;
}

pub trait SaveBackupWriter: Send + Sync {
    fn write_backup(&self, request: SaveBackupWriteRequest) -> Result<SaveBackupWriteResult>;

    fn delete_backup_files(
        &self,
        backup_directory: &ProfileDirectorySelection,
        summary: &SaveBackupSummary,
    ) -> Result<()>;
}

pub trait SaveBackupRepository: Send + Sync {
    fn save(&self, summary: &SaveBackupSummary) -> Result<()>;

    fn list_for_profile(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        limit: Option<usize>,
    ) -> Result<Vec<SaveBackupSummary>>;

    fn mark_status(&self, backup_id: &str, status: SaveBackupStatus) -> Result<()>;
}

pub trait SaveBackupSchedulerStateRepository: Send + Sync {
    fn get_state(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<Option<SaveBackupSchedulerState>>;

    fn upsert_state(&self, state: &SaveBackupSchedulerState) -> Result<()>;

    fn acquire_due_lease(
        &self,
        request: SaveBackupSchedulerLeaseRequest,
    ) -> Result<Option<SaveBackupSchedulerState>>;

    fn renew_lease(&self, _request: SaveBackupSchedulerLeaseRenewalRequest) -> Result<bool> {
        Ok(false)
    }

    fn release_lease(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        lease_owner: &str,
    ) -> Result<()>;

    fn record_worker_heartbeat(&self, heartbeat: SaveBackupWorkerHeartbeat) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_registry_errors_have_stable_codes() {
        assert_eq!(
            SaveBackupBackgroundRegistryError::TaskOwnershipConflict.code(),
            "save_backup_background_task_ownership_conflict"
        );
        assert_eq!(
            SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable.code(),
            "save_backup_background_worker_binary_unavailable"
        );
        assert_eq!(
            SaveBackupBackgroundRegistryError::CommandTimeout.code(),
            "save_backup_background_command_timeout"
        );
        assert_eq!(
            SaveBackupBackgroundRegistryError::CommandInvalidOutput.code(),
            "save_backup_background_command_invalid_output"
        );
        assert_eq!(
            SaveBackupBackgroundRegistryError::OperationFailed.code(),
            "save_backup_background_registration_failed"
        );
        assert_eq!(SAVE_BACKUP_BACKGROUND_REGISTRY_SCHEMA_VERSION, 1);
    }
}
