mod app_settings;
mod audit;
mod cancellation;
mod category;
mod diagnostics_environment;
mod game_setup;
mod install;
mod mod_import;
mod mod_metadata;
mod preview_image;
mod profile;
mod text_log;

use anyhow::Result;

pub use app_settings::{
    AppSettings, AppSettingsRepository, AppSettingsRepositoryError, AppSettingsRepositoryResult,
};
pub use audit::{AuditLogEvent, AuditLogReadRequest, AuditLogReader, AuditLogWriter};
pub use cancellation::{CancellationToken, NeverCancelled};
pub use category::CategoryRepository;
pub use diagnostics_environment::{DiagnosticsEnvironmentProvider, DiagnosticsEnvironmentSummary};
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
    ModPackageMetadataAnalyzer, PreparedModPackage, StoredImportPreviewImage,
    StoredModImportAnalysis, StoredModPackageMetadata,
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
pub use text_log::{TextLogKind, TextLogLine, TextLogReadRequest, TextLogReader};

pub trait AppClock: Send + Sync {
    fn now_unix_millis(&self) -> Result<u128>;
}
