mod game_setup;
mod preview_image;

use anyhow::Result;

pub use game_setup::{
    GameAdapter, GameCandidate, GameCandidateSource, GameConfigRepository,
    GameConfigRepositoryError, GameConfigRepositoryResult, GameDirectoryProbe,
    GameDirectoryProbeFactory, GameDiscoveryError, GameDiscoveryRequest, GameDiscoveryService,
};
pub use preview_image::{
    PackagePreviewScanner, PreviewImageCandidate, PreviewImageProcessingResult,
    PreviewImageProcessor, PreviewImageSourceRef, ProcessedPreviewImage, ThumbnailRef,
    ThumbnailStore,
};

pub trait AppClock: Send + Sync {
    fn now_unix_millis(&self) -> Result<u128>;
}
