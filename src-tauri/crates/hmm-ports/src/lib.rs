mod app_settings;
mod audit;
mod batch;
mod cancellation;
mod category;
mod diagnostics_environment;
mod diagnostics_health;
mod external_import;
mod game_launch;
mod game_prerequisites;
mod game_running;
mod game_setup;
mod install;
mod mod_import;
mod mod_library_projection;
mod mod_metadata;
mod preview_image;
mod profile;
mod reinstall;
mod replacement;
mod save_backup;
mod save_directory;
mod staging;
mod task_log;
mod text_log;

use anyhow::Result;

pub type PortResult<T> = anyhow::Result<T>;

pub use app_settings::{
    AppSettings, AppSettingsRepository, AppSettingsRepositoryError, AppSettingsRepositoryResult,
};
pub use audit::{
    AuditLogEvent, AuditLogReadRequest, AuditLogReader, AuditLogWriter, AuditWriteFailurePolicy,
};
pub use batch::{
    BatchAttemptAdmission, BatchAttemptAdmissionRequest, BatchLifecycleRepository,
    BatchPlanFactsProvider, BatchRetryAttemptCreation, BatchRetryAttemptRequest,
    BatchSealRepository, BatchSealRequest,
};
pub use cancellation::{CancellationToken, NeverCancelled};
pub use category::CategoryRepository;
pub use diagnostics_environment::{DiagnosticsEnvironmentProvider, DiagnosticsEnvironmentSummary};
pub use diagnostics_health::{DiagnosticsEvidenceHealth, DiagnosticsEvidenceHealthSnapshot};
pub use external_import::{
    ExternalImportBatchRepository, ExternalImportCandidatePage, ExternalImportItemResultPage,
    ExternalImportMaterializationOutcome, ExternalImportMaterializeRequest,
    ExternalImportMaterializedPackage, ExternalImportMaterializer, ExternalImportScanRequest,
    ExternalImportScanResult, ExternalImportScanner, ExternalImportSealAndStartRequest,
    ExternalImportSealAndStartResult, ExternalImportSelectionCompareAndSwapRequest,
    ExternalImportSelectionCompareAndSwapResult, ExternalImportSourceRegistration,
    ExternalImportSourceRegistry,
};
pub use game_launch::{
    GameLaunchError, GameLaunchMethod, GameLaunchReceipt, GameLaunchRunner, GameLauncher,
};
pub use game_prerequisites::{
    summarize_prerequisite_items, GamePrerequisiteIssue, GamePrerequisiteIssueCode,
    GamePrerequisiteItem, GamePrerequisiteItemStatus, GamePrerequisiteJsonCheckRule,
    GamePrerequisiteReport, GamePrerequisiteReportState, GamePrerequisiteRule,
    GamePrerequisiteRuleRepository, GamePrerequisiteRuleRepositoryError, GamePrerequisiteRuleSet,
    GamePrerequisiteSignatureRule, GamePrerequisiteSummaryStatus,
};
pub use game_running::{GameRunningDetector, GameRunningStatus};
pub use game_setup::{
    GameAdapter, GameCandidate, GameCandidateSource, GameConfigRepository,
    GameConfigRepositoryError, GameConfigRepositoryResult, GameDirectoryProbe,
    GameDirectoryProbeFactory, GameDiscoveryError, GameDiscoveryRequest, GameDiscoveryService,
};
pub use install::{
    InstallBackupStore, InstallGameFileSystem, InstallManifestRepository,
    InstallRecoveryRecordRepository, InstallSourceFileReader,
};
pub use mod_import::{
    DiagnosticPackageEntry, DiagnosticPackageExportRequest, DiagnosticPackageExportResult,
    DiagnosticPackageExporter, ModImportArchiveReader, ModImportCatalogSnapshot,
    ModImportCatalogUpsert, ModImportExternalCatalogAdmissionError, ModImportExternalCatalogUpsert,
    ModImportExternalDisplayNameAdmission, ModImportPackagePrepareReaderRequest,
    ModImportPackagePrepareRequest, ModImportPackagePreparer, ModImportResultRepository,
    ModImportSandboxLocator, ModPackageInstallFile, ModPackageInstallFileScanRequest,
    ModPackageInstallFileScanner, ModPackageMetadata, ModPackageMetadataAnalyzer,
    PreparedModPackage, StoredImportPreviewImage, StoredLogicalMod, StoredModImportAnalysis,
    StoredModOriginProvenance, StoredModPackageMetadata, StoredModRevision,
    MOD_IMPORT_UPSERT_CHUNK_SIZE, MOD_IMPORT_UPSERT_MAX_ENTRIES,
};
pub use mod_library_projection::{
    normalize_mod_library_query_key, ModLibraryProfileProjection, ModLibraryProfileProjectionState,
    ModLibraryProjectionLabel, ModLibraryProjectionPageItem, ModLibraryProjectionProfileQuery,
    ModLibraryProjectionQueryError, ModLibraryProjectionQueryFilter, ModLibraryProjectionQueryPage,
    ModLibraryProjectionQueryRepository, ModLibraryProjectionQueryRequest,
    ModLibraryProjectionQueryStatus, ModLibraryProjectionReadiness, ModLibraryProjectionRecord,
    ModLibraryProjectionRepository, ModLibraryProjectionSnapshot, ModLibraryProjectionState,
    ModLibraryProjectionStatus, ModLibraryProjectionStatusRecord,
    MOD_LIBRARY_PROJECTION_SCHEMA_VERSION, MOD_LIBRARY_QUERY_KEY_VERSION,
};
pub use mod_metadata::ModMetadataRepository;
pub use preview_image::{
    PackagePreviewScanner, PreviewImageCandidate, PreviewImageProcessRequest,
    PreviewImageProcessingResult, PreviewImageProcessor, PreviewImageScanRequest,
    PreviewImageSourceRef, ProcessedPreviewImage, ThumbnailCacheMaintenance,
    ThumbnailCacheMaintenanceRequest, ThumbnailRef, ThumbnailStore,
};
pub use profile::{
    ProfileRepository, ProfileSaveDirectoryValidator, ProfileSaveSettingsRepository,
};
pub use reinstall::{ReinstallRecoveryTransactionRepository, ReinstallSnapshotStore};
pub use replacement::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAdapterResult,
    ReplacementAnalysisRequest, ReplacementAsset, ReplacementCatalogError,
    ReplacementCatalogProvider, ReplacementCatalogResult, RetargetPlanRequest,
};
pub use save_backup::{
    SaveBackupBackgroundRegistry, SaveBackupBackgroundRegistryError,
    SaveBackupBackgroundRegistryResult, SaveBackupBackgroundSettingsRepository,
    SaveBackupRepository, SaveBackupSchedulerStateRepository, SaveBackupWriteRequest,
    SaveBackupWriteResult, SaveBackupWriter, SAVE_BACKUP_BACKGROUND_REGISTRY_SCHEMA_VERSION,
};
pub use save_directory::{
    GameSaveDirectoryRule, PendingSaveDirectoryCandidate, PendingSaveDirectoryCandidateStore,
    PendingSaveDirectoryDiscovery, ScannedSaveDirectoryCandidate, SteamAccountProfileClient,
    SteamUserdataScanRequest, SteamUserdataScanner,
};
pub use staging::{RetargetStagingError, RetargetStagingFile, RetargetStagingMaterializer};
pub use task_log::{TaskLogRecord, TaskLogWriter};
pub use text_log::{TextLogKind, TextLogLine, TextLogReadRequest, TextLogReader};

pub trait AppClock: Send + Sync {
    fn now_unix_millis(&self) -> Result<u128>;
}
