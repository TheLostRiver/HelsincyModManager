use anyhow::Result;
use hmm_core::{
    GameId, ProfileBackupRetention, ProfileDirectorySelection, ProfileId,
    SaveBackupBackgroundRegistrationStatus, SaveBackupBackgroundSettings,
    SaveBackupRetentionReason, SaveBackupSchedulerLeaseRenewalRequest,
    SaveBackupSchedulerLeaseRequest, SaveBackupSchedulerState, SaveBackupStatus, SaveBackupSummary,
    SaveBackupTrigger, SaveBackupWorkerHeartbeat,
};
use std::path::PathBuf;

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
pub enum SaveBackupFileDeleteDisposition {
    Deleted,
    AlreadyMissing,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupFileDeleteResult {
    pub disposition: SaveBackupFileDeleteDisposition,
    pub released_bytes: u64,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupDeleteReport {
    pub archive: SaveBackupFileDeleteResult,
    pub manifest: SaveBackupFileDeleteResult,
}

impl SaveBackupDeleteReport {
    pub fn converged(&self) -> bool {
        self.archive.disposition != SaveBackupFileDeleteDisposition::Blocked
            && self.manifest.disposition != SaveBackupFileDeleteDisposition::Blocked
    }

    pub fn released_archive_bytes(&self) -> u64 {
        self.archive.released_bytes
    }

    pub fn stable_error_code(&self) -> Option<&str> {
        self.archive
            .error_code
            .as_deref()
            .or(self.manifest.error_code.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupCenterRepositoryQuery {
    pub game_id: GameId,
    pub profile_id: Option<ProfileId>,
    pub trigger: Option<SaveBackupTrigger>,
    pub status: Option<SaveBackupStatus>,
    pub search: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveBackupCenterRepositoryFacts {
    pub backup_count: u32,
    pub archive_bytes: u64,
    pub protected_count: u32,
    pub attention_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupCenterRepositoryProfileFacts {
    pub profile_id: ProfileId,
    pub facts: SaveBackupCenterRepositoryFacts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupCenterRepositoryItem {
    pub profile_name: String,
    pub backup: SaveBackupSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupCenterRepositoryPage {
    pub total_count: usize,
    pub summary: SaveBackupCenterRepositoryFacts,
    pub profiles: Vec<SaveBackupCenterRepositoryProfileFacts>,
    pub items: Vec<SaveBackupCenterRepositoryItem>,
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

    /// Returns `Registered` only after the written registration was read back and verified.
    fn register(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>;

    /// Returns `NotRegistered` only after absence was read back and verified.
    fn unregister(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>;
}

pub trait SaveBackupBackgroundSettingsRepository: Send + Sync {
    fn load(&self) -> Result<SaveBackupBackgroundSettings>;

    fn begin_enable(&self, enabled_at: u128) -> Result<()>;

    fn finish_disable(&self, updated_at: u128) -> Result<()>;

    fn record_worker_heartbeat(&self, heartbeat_at: u128) -> Result<()>;
}

pub trait SaveBackupWriter: Send + Sync {
    fn write_backup(&self, request: SaveBackupWriteRequest) -> Result<SaveBackupWriteResult>;

    fn delete_backup_files(
        &self,
        backup_directory: &ProfileDirectorySelection,
        summary: &SaveBackupSummary,
    ) -> Result<()>;

    fn delete_backup_files_report(
        &self,
        backup_directory: &ProfileDirectorySelection,
        summary: &SaveBackupSummary,
    ) -> Result<SaveBackupDeleteReport> {
        self.delete_backup_files(backup_directory, summary)?;
        Ok(SaveBackupDeleteReport {
            archive: SaveBackupFileDeleteResult {
                disposition: SaveBackupFileDeleteDisposition::Deleted,
                released_bytes: summary.archive_size_bytes,
                error_code: None,
            },
            manifest: SaveBackupFileDeleteResult {
                disposition: SaveBackupFileDeleteDisposition::Deleted,
                released_bytes: 0,
                error_code: None,
            },
        })
    }
}

/// 解析某个 profile 的备份目录并按需创建缺失的受控子树。
///
/// 「打开文件夹」入口消费:defaulted 的备份目录在第一次成功备份前并不存在,
/// 补建后上层才能打开。实现方必须逐级 nofollow 创建,且只隐式创建应用自有的
/// 托管布局;Custom 模式的根是玩家自选目录,必须已存在——绝不因为一次打开
/// 动作替玩家凭空造出他们选择的根目录。
pub trait SaveBackupDirectoryLocator: Send + Sync {
    fn backup_directory_for_profile(
        &self,
        selection: &ProfileDirectorySelection,
        game_id: &str,
        profile_id: &str,
    ) -> Result<PathBuf>;
}

pub trait SaveBackupRepository: Send + Sync {
    fn save(&self, summary: &SaveBackupSummary) -> Result<()>;

    fn get_for_restore(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        backup_id: &str,
    ) -> Result<Option<SaveBackupSummary>> {
        let _ = (game_id, profile_id, backup_id);
        Ok(None)
    }

    fn list_for_profile(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        limit: Option<usize>,
    ) -> Result<Vec<SaveBackupSummary>>;

    fn list_for_game(&self, _game_id: &GameId) -> Result<Vec<SaveBackupSummary>> {
        Ok(Vec::new())
    }

    /// Returns a database-backed page and aggregate facts when the repository can evaluate
    /// the complete center query without materializing the full history in the application.
    /// Small fake repositories may return `None` and use the service fallback.
    fn query_for_center(
        &self,
        _request: &SaveBackupCenterRepositoryQuery,
    ) -> Result<Option<SaveBackupCenterRepositoryPage>> {
        Ok(None)
    }

    fn mark_status(&self, backup_id: &str, status: SaveBackupStatus) -> Result<()>;

    fn begin_retention(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        backup_id: &str,
        reasons: &[SaveBackupRetentionReason],
        attempted_at: u128,
    ) -> Result<bool> {
        let _ = (game_id, profile_id, reasons, attempted_at);
        self.mark_status(backup_id, SaveBackupStatus::RetentionPending)?;
        Ok(true)
    }

    fn finish_retention(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        backup_id: &str,
        status: SaveBackupStatus,
        error_code: Option<&str>,
        released_bytes: u64,
    ) -> Result<()> {
        let _ = (game_id, profile_id, error_code, released_bytes);
        self.mark_status(backup_id, status)
    }

    fn update_note(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
        _backup_id: &str,
        _note: Option<&str>,
    ) -> Result<bool> {
        Ok(false)
    }
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
