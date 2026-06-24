use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedModPackage {
    pub package_id: String,
    pub sandbox_root: PathBuf,
}

pub trait ModImportPackagePreparer: Send + Sync {
    fn prepare_package(&self, task_id: &str, archive_path: &Path) -> Result<PreparedModPackage>;
}
