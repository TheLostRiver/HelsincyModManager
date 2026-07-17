mod app_settings;
mod audit;
mod cancellation;
mod category;
mod diagnostics_environment;
mod diagnostics_health;
mod game_launch;
mod game_prerequisites;
mod game_running;
mod game_setup;
mod install;
mod mod_import;
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
pub use cancellation::{CancellationToken, NeverCancelled};
pub use category::CategoryRepository;
pub use diagnostics_environment::{DiagnosticsEnvironmentProvider, DiagnosticsEnvironmentSummary};
pub use diagnostics_health::{DiagnosticsEvidenceHealth, DiagnosticsEvidenceHealthSnapshot};
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
    DiagnosticPackageExporter, ModImportPackagePrepareRequest, ModImportPackagePreparer,
    ModImportResultRepository, ModImportSandboxLocator, ModPackageInstallFile,
    ModPackageInstallFileScanRequest, ModPackageInstallFileScanner, ModPackageMetadata,
    ModPackageMetadataAnalyzer, PreparedModPackage, StoredImportPreviewImage, StoredLogicalMod,
    StoredModImportAnalysis, StoredModOriginProvenance, StoredModPackageMetadata,
    StoredModRevision,
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
