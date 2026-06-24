use crate::CancellationToken;
use anyhow::Result;
use hmm_core::{PreviewImagePolicy, PreviewImageRejectionReason};
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreviewImageSourceRef {
    pub package_id: String,
    pub logical_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewImageCandidate {
    pub source_ref: PreviewImageSourceRef,
    pub file_name: String,
    pub compressed_size: u64,
    pub priority: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThumbnailRef {
    pub package_id: String,
    pub content_hash: String,
    pub variant: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedPreviewImage {
    pub thumbnail_ref: ThumbnailRef,
    pub width: u32,
    pub height: u32,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewImageProcessingResult {
    Thumbnail(ProcessedPreviewImage),
    Fallback(PreviewImageRejectionReason),
}

pub struct PreviewImageScanRequest<'a> {
    pub package_id: &'a str,
    pub sandbox_root: &'a Path,
    pub policy: &'a PreviewImagePolicy,
    pub cancellation_token: &'a dyn CancellationToken,
}

pub struct PreviewImageProcessRequest<'a> {
    pub sandbox_root: &'a Path,
    pub candidate: &'a PreviewImageCandidate,
    pub policy: &'a PreviewImagePolicy,
    pub cancellation_token: &'a dyn CancellationToken,
}

pub trait PackagePreviewScanner: Send + Sync {
    fn scan_candidates(
        &self,
        request: PreviewImageScanRequest<'_>,
    ) -> Result<Vec<PreviewImageCandidate>>;
}

pub trait PreviewImageProcessor: Send + Sync {
    fn process_candidate(
        &self,
        request: PreviewImageProcessRequest<'_>,
    ) -> Result<PreviewImageProcessingResult>;
}

pub trait ThumbnailStore: Send + Sync {
    fn put_thumbnail(
        &self,
        package_id: &str,
        content_hash: &str,
        extension: &str,
        bytes: &[u8],
    ) -> Result<ThumbnailRef>;

    fn resolve_url(&self, thumbnail_ref: &ThumbnailRef) -> Result<String>;
}

pub struct ThumbnailCacheMaintenanceRequest<'a> {
    pub retained: &'a [ThumbnailRef],
    pub max_bytes: Option<u64>,
    pub max_age: Option<Duration>,
}

pub trait ThumbnailCacheMaintenance: Send + Sync {
    fn maintain_thumbnail_cache(&self, request: ThumbnailCacheMaintenanceRequest<'_>)
        -> Result<()>;
}
