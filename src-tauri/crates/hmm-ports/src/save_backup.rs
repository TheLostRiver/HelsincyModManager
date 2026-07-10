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

pub trait SaveBackupBackgroundRegistry: Send + Sync {
    fn inspect(&self) -> Result<SaveBackupBackgroundRegistrationStatus>;

    fn register(&self) -> Result<SaveBackupBackgroundRegistrationStatus>;

    fn unregister(&self) -> Result<SaveBackupBackgroundRegistrationStatus>;
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
