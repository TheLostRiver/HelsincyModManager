mod batch;
mod category;
mod external_import;
mod game;
mod install;
mod mod_metadata;
mod preview_image;
mod profile;
mod reinstall;
mod replacement;
mod retarget;
mod save_backup;
mod save_directory;
mod save_restore;

pub use batch::{
    build_batch_plan, BatchActionSummary, BatchAttempt, BatchAttemptStatus, BatchExecutionPolicy,
    BatchId, BatchItemFacts, BatchItemId, BatchItemInput, BatchItemPlan, BatchItemResult,
    BatchItemStatus, BatchOperation, BatchPlan, BatchPlanError, BatchPlanFacts, BatchPlanRequest,
    BatchPlanStatus, BatchPreflightDecision, BatchPreflightStatus, BatchReasonSummary,
    BatchResource, BatchResourceLimits, BatchResourceUsage, BatchResultSummary, BatchTargetClaim,
    BatchTargetWriteKind, InstallBatchItemInput, NormalizedBatchPlanRequest,
    ReinstallBatchItemInput, SealedBatch, SealedBatchItem, UninstallBatchItemInput,
    BATCH_PLAN_SCHEMA_VERSION, BATCH_RESOURCE_LIMITS_VERSION, DEFAULT_BATCH_MAX_CANONICAL_BYTES,
    DEFAULT_BATCH_MAX_ITEMS, DEFAULT_BATCH_MAX_TARGET_ACTIONS,
    DEFAULT_BATCH_PREVIEW_TOKEN_TTL_MILLIS,
};
pub use category::{Category, CategoryLabel};
pub use external_import::{
    ExternalImportAdapterId, ExternalImportBatch, ExternalImportBatchId,
    ExternalImportBatchImportStatus, ExternalImportCandidate, ExternalImportCandidateId,
    ExternalImportCandidateStatus, ExternalImportConflictKind, ExternalImportConflictResolution,
    ExternalImportItemResult, ExternalImportItemStatus, ExternalImportMaterializationBudget,
    ExternalImportMetadataHint, ExternalImportProvenance, ExternalImportProvenanceError,
    ExternalImportReasonCode, ExternalImportResourceBudget, ExternalImportResourceUsage,
    ExternalImportScanStatus, ExternalImportSelection, ExternalImportSelectionDecision,
    ExternalImportSelectionEntry, ExternalImportSelectionError, ExternalImportSelectionId,
    ExternalImportSelectionMutation, ExternalImportSelectionMutationResult,
    ExternalImportSelectionStatus, ExternalImportSource, ExternalImportSourceId,
    DEFAULT_EXTERNAL_IMPORT_BATCH_MAX_FILES,
    DEFAULT_EXTERNAL_IMPORT_BATCH_MAX_MATERIALIZATION_BYTES,
    DEFAULT_EXTERNAL_IMPORT_BATCH_MAX_SOURCE_BYTES,
    DEFAULT_EXTERNAL_IMPORT_MATERIALIZATION_MAX_DEPTH,
    DEFAULT_EXTERNAL_IMPORT_MATERIALIZATION_MAX_FILES,
    DEFAULT_EXTERNAL_IMPORT_MATERIALIZATION_MAX_SINGLE_FILE_BYTES,
    DEFAULT_EXTERNAL_IMPORT_MATERIALIZATION_MAX_TOTAL_BYTES, EXTERNAL_IMPORT_SELECTION_MAX_ITEMS,
    EXTERNAL_IMPORT_SELECTION_MUTATION_MAX_ITEMS,
};
pub use game::{
    GameDirectoryEvidence, GameDirectoryEvidenceKind, GameDirectoryStatus, GameDirectoryValidation,
    GameId, GameIdError, GameInstance, GameSetupErrorCode, GameSetupStatus, MHW_GAME_ID,
};
pub use install::{
    FileLayer, InstallAction, InstallConflict, InstallFileProvider, InstallManifest,
    InstallManifestEntry, InstallManifestStatus, InstallManifestStatusConsumption,
    InstallManifestValidationError, InstallPlan, InstallPlanValidationError, InstallRecoveryRecord,
    InstallRecoveryRecordEntry, InstallRecoveryRecordStatus, InstallRecoveryRecordTransitionError,
    InstallTargetPath, InstallTargetPathError, InstalledFileSummary, ModId, ModRevisionId,
    PackageFileId, ProfileId, INSTALL_MANIFEST_SCHEMA_VERSION, INSTALL_MANIFEST_SCHEMA_VERSION_V1,
    INSTALL_MANIFEST_SCHEMA_VERSION_V2,
};
pub use mod_metadata::ModMetadataOverlay;
pub use preview_image::{
    PreviewImageOutputFormat, PreviewImagePolicy, PreviewImagePolicyError,
    PreviewImageRejectionReason, PreviewImageStatus,
};
pub use profile::{
    BackupCadence, Profile, ProfileBackupRetention, ProfileBackupSchedule, ProfileDirectoryMode,
    ProfileDirectorySelection, ProfileDirectoryStatus, ProfileSaveSettings, DEFAULT_PROFILE_ID,
};
pub use reinstall::{
    classify_reinstall_targets, is_same_revision_replacement_target_switch,
    replace_entries_and_bindings_for_mod, replace_entries_for_mod, resolve_installed_revision,
    ReinstallClassificationError, ReinstallManifestError, ReinstallRecoveryTarget,
    ReinstallRecoveryTransaction, ReinstallRecoveryTransactionStatus,
    ReinstallRecoveryTransactionTransitionError, ReinstallRecoveryTransactionValidationError,
    ReinstallSnapshotCleanupOwner, ReinstallSnapshotPurpose, ReinstallSnapshotState,
    ReinstallTargetClass, ReinstallTargetClassification, ReinstallTargetState,
};
pub use replacement::{
    ContentTransformInvocation, ContentTransformerIdentity, LocalizedText, ReplacementAdapterFacts,
    ReplacementBinding, ReplacementBindingId, ReplacementBindingSnapshot, ReplacementCatalog,
    ReplacementCatalogVersion, ReplacementError, ReplacementSourceId, ReplacementTarget,
    ReplacementTargetId, ReplacementTargetKind, CONTENT_TRANSFORM_INVOCATION_SCHEMA_VERSION,
    REPLACEMENT_ADAPTER_FACTS_SCHEMA_VERSION,
};
pub use retarget::{
    ReplacementAnalysis, ReplacementSource, ReplacementWarning, RetargetAction, RetargetError,
    RetargetPlan,
};
pub use save_backup::{
    SaveBackupBackgroundProtectionStatus, SaveBackupBackgroundRegistrationStatus,
    SaveBackupBackgroundSettings, SaveBackupManifest, SaveBackupManifestFile,
    SaveBackupManifestSource, SaveBackupSchedulerLeaseRenewalRequest,
    SaveBackupSchedulerLeaseRequest, SaveBackupSchedulerPendingReason, SaveBackupSchedulerState,
    SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger, SaveBackupWorkerHeartbeat,
    SAVE_BACKUP_MANIFEST_SCHEMA_VERSION,
};
pub use save_directory::{
    steam_id64_from_account_id32, SaveDirectoryCandidateConfidence, SaveDirectoryCandidateSource,
    SaveDirectoryCandidateSummary, SaveDirectoryDiscoveryOutcome, SaveDirectoryDiscoveryResult,
    SteamAccountProfileSummary, STEAM_ID64_ACCOUNT_ID_OFFSET,
};
pub use save_restore::{SaveRestoreTransaction, SaveRestoreTransactionStatus};

#[cfg(test)]
mod external_import_tests;
