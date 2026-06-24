use anyhow::Result;
use hmm_core::PreviewImageRejectionReason;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedModPackage {
    pub package_id: String,
    pub sandbox_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModPackageMetadata {
    pub display_name: Option<String>,
}

pub trait ModImportPackagePreparer: Send + Sync {
    fn prepare_package(&self, task_id: &str, archive_path: &Path) -> Result<PreparedModPackage>;
}

pub trait ModPackageMetadataAnalyzer: Send + Sync {
    fn analyze_metadata(&self, package_id: &str, sandbox_root: &Path)
        -> Result<ModPackageMetadata>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredModImportAnalysis {
    pub mod_id: String,
    pub task_id: String,
    pub package_id: String,
    pub display_name: String,
    pub preview_image: StoredImportPreviewImage,
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
    },
    Fallback {
        reason: PreviewImageRejectionReason,
    },
}

pub trait ModImportResultRepository: Send + Sync {
    fn save_analysis(&self, analysis: &StoredModImportAnalysis) -> Result<()>;
    fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>>;
    fn get_analysis(&self, mod_id: &str) -> Result<Option<StoredModImportAnalysis>>;
}
