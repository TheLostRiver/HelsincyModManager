mod app_settings;
mod cancellation;
mod game_setup;
mod mod_import;
mod preview_image;

use anyhow::Result;

pub use app_settings::{
    AppSettings, AppSettingsRepository, AppSettingsRepositoryError, AppSettingsRepositoryResult,
};
pub use cancellation::{CancellationToken, NeverCancelled};
pub use game_setup::{
    GameAdapter, GameCandidate, GameCandidateSource, GameConfigRepository,
    GameConfigRepositoryError, GameConfigRepositoryResult, GameDirectoryProbe,
    GameDirectoryProbeFactory, GameDiscoveryError, GameDiscoveryRequest, GameDiscoveryService,
};
pub use mod_import::{
    ModImportPackagePrepareRequest, ModImportPackagePreparer, ModImportResultRepository,
    ModPackageMetadata, ModPackageMetadataAnalyzer, PreparedModPackage, StoredImportPreviewImage,
    StoredModImportAnalysis, StoredModPackageMetadata,
};
pub use preview_image::{
    PackagePreviewScanner, PreviewImageCandidate, PreviewImageProcessRequest,
    PreviewImageProcessingResult, PreviewImageProcessor, PreviewImageScanRequest,
    PreviewImageSourceRef, ProcessedPreviewImage, ThumbnailCacheMaintenance,
    ThumbnailCacheMaintenanceRequest, ThumbnailRef, ThumbnailStore,
};

pub trait AppClock: Send + Sync {
    fn now_unix_millis(&self) -> Result<u128>;
}
