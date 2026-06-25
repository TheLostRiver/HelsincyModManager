use anyhow::Result;
use hmm_core::PreviewImageRejectionReason;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedModPackage {
    pub package_id: String,
    pub sandbox_root: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModPackageMetadata {
    pub display_name: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub dependencies: Vec<String>,
}

pub struct ModImportPackagePrepareRequest<'a> {
    pub task_id: &'a str,
    pub archive_path: &'a Path,
    pub cancellation_token: &'a dyn crate::CancellationToken,
}

pub trait ModImportPackagePreparer: Send + Sync {
    fn prepare_package(
        &self,
        request: ModImportPackagePrepareRequest<'_>,
    ) -> Result<PreparedModPackage>;
}

pub trait ModPackageMetadataAnalyzer: Send + Sync {
    fn analyze_metadata(&self, package_id: &str, sandbox_root: &Path)
        -> Result<ModPackageMetadata>;
}

pub trait ModImportSandboxLocator: Send + Sync {
    fn sandbox_root_for_package(&self, package_id: &str) -> Result<PathBuf>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModPackageInstallFile {
    pub package_file_id: String,
    pub target_path: String,
}

pub struct ModPackageInstallFileScanRequest<'a> {
    pub package_id: &'a str,
    pub sandbox_root: &'a Path,
}

pub trait ModPackageInstallFileScanner: Send + Sync {
    fn scan_install_files(
        &self,
        request: ModPackageInstallFileScanRequest<'_>,
    ) -> Result<Vec<ModPackageInstallFile>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredModImportAnalysis {
    pub mod_id: String,
    pub task_id: String,
    pub package_id: String,
    pub display_name: String,
    #[serde(default)]
    pub metadata: StoredModPackageMetadata,
    #[serde(default = "default_preview_image")]
    pub preview_image: StoredImportPreviewImage,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredModPackageMetadata {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum StoredImportPreviewImage {
    Thumbnail {
        thumbnail_url: String,
        width: u32,
        height: u32,
        content_hash: String,
        #[serde(default = "default_preview_thumbnail_variant")]
        variant: String,
    },
    Fallback {
        reason: PreviewImageRejectionReason,
    },
}

fn default_preview_thumbnail_variant() -> String {
    "preview-768".to_owned()
}

fn default_preview_image() -> StoredImportPreviewImage {
    StoredImportPreviewImage::Fallback {
        reason: PreviewImageRejectionReason::Missing,
    }
}

pub trait ModImportResultRepository: Send + Sync {
    fn save_analysis(&self, analysis: &StoredModImportAnalysis) -> Result<()>;
    fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>>;
    fn get_analysis(&self, mod_id: &str) -> Result<Option<StoredModImportAnalysis>>;
}

pub struct DiagnosticPackageEntry<'a> {
    pub name: &'a str,
    pub bytes: &'a [u8],
}

pub struct DiagnosticPackageExportRequest<'a> {
    pub file_name: &'a str,
    pub entries: &'a [DiagnosticPackageEntry<'a>],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticPackageExportResult {
    pub export_id: String,
    pub file_name: String,
    pub size_bytes: u64,
}

pub trait DiagnosticPackageExporter: Send + Sync {
    fn export_package(
        &self,
        request: DiagnosticPackageExportRequest<'_>,
    ) -> Result<DiagnosticPackageExportResult>;
}
