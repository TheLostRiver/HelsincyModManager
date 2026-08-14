use hmm_core::{
    GameId, ProfileDirectorySelection, ProfileId, SaveBackupSummary, SaveRestoreTransaction,
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSaveRestoreSource {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub backup_id: String,
    pub evidence_digest: String,
    pub file_count: u32,
    pub total_uncompressed_bytes: u64,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SaveRestoreSourceError {
    #[error("save restore backup directory is unavailable")]
    BackupDirectoryUnavailable,
    #[error("save restore archive is unavailable")]
    ArchiveUnavailable,
    #[error("save restore manifest is unavailable")]
    ManifestUnavailable,
    #[error("save restore manifest is invalid")]
    ManifestInvalid,
    #[error("save restore archive is invalid")]
    ArchiveInvalid,
    #[error("save restore source hash does not match")]
    HashMismatch,
    #[error("save restore source contains an unsafe path")]
    UnsafePath,
    #[error("save restore source exceeds safety limits")]
    SizeLimitExceeded,
    #[error("save restore validation staging is unavailable")]
    StagingUnavailable,
}

impl SaveRestoreSourceError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::BackupDirectoryUnavailable => "save_restore_backup_directory_unavailable",
            Self::ArchiveUnavailable => "save_restore_archive_unavailable",
            Self::ManifestUnavailable => "save_restore_manifest_unavailable",
            Self::ManifestInvalid => "save_restore_manifest_invalid",
            Self::ArchiveInvalid => "save_restore_archive_invalid",
            Self::HashMismatch => "save_restore_hash_mismatch",
            Self::UnsafePath => "save_restore_path_unsafe",
            Self::SizeLimitExceeded => "save_restore_size_limit_exceeded",
            Self::StagingUnavailable => "save_restore_staging_unavailable",
        }
    }
}

pub trait SaveRestoreSourceValidator: Send + Sync {
    fn validate_source(
        &self,
        summary: &SaveBackupSummary,
    ) -> Result<ValidatedSaveRestoreSource, SaveRestoreSourceError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRestorePrepareRequest {
    pub transaction_id: String,
    pub summary: SaveBackupSummary,
    pub target_directory: ProfileDirectorySelection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSaveRestore {
    pub prepared_id: String,
    pub evidence_digest: String,
    pub file_count: u32,
    pub total_uncompressed_bytes: u64,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SaveRestorePrepareError {
    #[error("save restore source validation failed: {0}")]
    Source(SaveRestoreSourceError),
    #[error("save restore target is unavailable")]
    TargetUnavailable,
    #[error("save restore target is unsafe")]
    TargetUnsafe,
    #[error("save restore staging is unavailable")]
    StagingUnavailable,
}

impl SaveRestorePrepareError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Source(error) => error.code(),
            Self::TargetUnavailable => "save_restore_target_unavailable",
            Self::TargetUnsafe => "save_restore_target_unsafe",
            Self::StagingUnavailable => "save_restore_staging_unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRestoreCommitRequest {
    pub transaction_id: String,
    pub prepared_id: String,
    pub summary: SaveBackupSummary,
    pub target_directory: ProfileDirectorySelection,
    pub pre_restore_summary: Option<SaveBackupSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRestoreCommitResult {
    pub restored_file_count: u32,
    pub rollback_performed: bool,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SaveRestoreCommitError {
    #[error("prepared save restore staging is missing")]
    PreparedMissing,
    #[error("save restore target is unavailable")]
    TargetUnavailable,
    #[error("save restore target changed after preparation")]
    TargetChanged,
    #[error("save restore commit failed")]
    CommitFailed,
    #[error("save restore commit was rolled back")]
    RolledBack,
    #[error("save restore recovery is required")]
    RecoveryRequired,
}

impl SaveRestoreCommitError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::PreparedMissing => "save_restore_prepared_missing",
            Self::TargetUnavailable => "save_restore_target_unavailable",
            Self::TargetChanged => "save_restore_target_changed",
            Self::CommitFailed => "save_restore_commit_failed",
            Self::RolledBack => "save_restore_rolled_back",
            Self::RecoveryRequired => "save_restore_recovery_required",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRestoreFinalizeRequest {
    pub transaction_id: String,
    pub target_directory: ProfileDirectorySelection,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SaveRestoreFinalizeError {
    #[error("save restore target is unavailable")]
    TargetUnavailable,
    #[error("save restore recovery evidence is unsafe")]
    RecoveryEvidenceUnsafe,
    #[error("save restore recovery evidence cleanup failed")]
    CleanupFailed,
}

impl SaveRestoreFinalizeError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::TargetUnavailable => "save_restore_target_unavailable",
            Self::RecoveryEvidenceUnsafe => "save_restore_recovery_evidence_unsafe",
            Self::CleanupFailed => "save_restore_recovery_cleanup_failed",
        }
    }
}

pub trait SaveRestoreFileSystem: Send + Sync {
    fn prepare_restore(
        &self,
        request: SaveRestorePrepareRequest,
    ) -> Result<PreparedSaveRestore, SaveRestorePrepareError>;

    fn discard_prepared(&self, prepared_id: &str);

    fn commit_restore(
        &self,
        request: SaveRestoreCommitRequest,
    ) -> Result<SaveRestoreCommitResult, SaveRestoreCommitError>;

    /// Deletes rollback evidence only after the caller has durably persisted the commit or
    /// rollback result. Successful restores remain non-terminal until this idempotent cleanup
    /// succeeds and the caller persists `Completed`.
    fn finalize_restore(
        &self,
        request: SaveRestoreFinalizeRequest,
    ) -> Result<(), SaveRestoreFinalizeError>;
}

pub trait SaveRestoreTransactionRepository: Send + Sync {
    fn save_transaction(&self, transaction: &SaveRestoreTransaction) -> anyhow::Result<()>;

    fn get_transaction(
        &self,
        transaction_id: &str,
    ) -> anyhow::Result<Option<SaveRestoreTransaction>>;

    fn has_incomplete_transaction_excluding(
        &self,
        game_id: &hmm_core::GameId,
        profile_id: &hmm_core::ProfileId,
        excluded_transaction_id: Option<&str>,
    ) -> anyhow::Result<bool>;

    fn has_incomplete_transaction(
        &self,
        game_id: &hmm_core::GameId,
        profile_id: &hmm_core::ProfileId,
    ) -> anyhow::Result<bool> {
        self.has_incomplete_transaction_excluding(game_id, profile_id, None)
    }
}
