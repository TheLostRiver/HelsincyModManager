mod game_setup;
mod mod_import;
mod preview_image;

use anyhow::Result;

pub use game_setup::{
    GameAdapter, GameCandidate, GameCandidateSource, GameConfigRepository,
    GameConfigRepositoryError, GameConfigRepositoryResult, GameDirectoryProbe,
    GameDirectoryProbeFactory, GameDiscoveryError, GameDiscoveryRequest, GameDiscoveryService,
};
pub use mod_import::{
    CancellationToken, ModImportPackagePrepareRequest, ModImportPackagePreparer,
    ModImportResultRepository, ModPackageMetadata, ModPackageMetadataAnalyzer, NeverCancelled,
    PreparedModPackage, StoredImportPreviewImage, StoredModImportAnalysis,
};
pub use preview_image::{
    PackagePreviewScanner, PreviewImageCandidate, PreviewImageProcessingResult,
    PreviewImageProcessor, PreviewImageSourceRef, ProcessedPreviewImage, ThumbnailCacheMaintenance,
    ThumbnailRef, ThumbnailStore,
};

pub trait AppClock: Send + Sync {
    fn now_unix_millis(&self) -> Result<u128>;
}
