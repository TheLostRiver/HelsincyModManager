use hmm_core::{InstallTargetPath, PackageFileId};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetargetStagingFile {
    package_file_id: PackageFileId,
    target_path: InstallTargetPath,
}

impl RetargetStagingFile {
    pub fn new(package_file_id: PackageFileId, target_path: InstallTargetPath) -> Self {
        Self {
            package_file_id,
            target_path,
        }
    }

    pub fn package_file_id(&self) -> &PackageFileId {
        &self.package_file_id
    }

    pub fn target_path(&self) -> &InstallTargetPath {
        &self.target_path
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RetargetStagingError {
    #[error("retarget staging batch cannot be empty")]
    EmptyBatch,
    #[error("retarget staging batch contains a duplicate package file")]
    DuplicatePackageFile,
    #[error("retarget staging targets collide on a case-insensitive filesystem")]
    CaseInsensitiveTargetCollision,
    #[error("retarget staging source is unavailable")]
    SourceUnavailable,
    #[error("retarget staging destination is unavailable")]
    DestinationUnavailable,
    #[error("retarget staging target is unsafe")]
    UnsafeTarget,
    #[error("retarget staging write failed")]
    WriteFailed,
    #[error("retarget staging publish failed")]
    PublishFailed,
    #[error("retarget staging cleanup failed")]
    CleanupFailed,
}

pub trait RetargetStagingMaterializer: Send + Sync {
    fn materialize(&self, files: &[RetargetStagingFile]) -> Result<(), RetargetStagingError>;
}
