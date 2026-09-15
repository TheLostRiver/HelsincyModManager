use crate::controlled_fs::open_child_directory_nofollow;
use crate::TaskScopedModImportSandboxLocator;
use anyhow::Result;
use hmm_ports::{ModImportSandboxLocator, ModPackageDirectoryOpener, SystemDirectoryOpener};
use std::{path::PathBuf, sync::Arc};

pub struct SandboxModDirectoryOpener {
    locator: TaskScopedModImportSandboxLocator,
    opener: Arc<dyn SystemDirectoryOpener>,
}

impl SandboxModDirectoryOpener {
    pub fn new(storage_root: PathBuf, opener: Arc<dyn SystemDirectoryOpener>) -> Self {
        Self {
            locator: TaskScopedModImportSandboxLocator::new_in_storage_root(storage_root),
            opener,
        }
    }
}

impl ModPackageDirectoryOpener for SandboxModDirectoryOpener {
    fn open_package_directory(&self, package_id: &str) -> Result<()> {
        let path = self.locator.sandbox_root_for_package(package_id)?;
        let root = self.locator.open_existing_sandbox_root()?;
        let _directory = open_child_directory_nofollow(
            &root,
            std::ffi::OsStr::new(package_id),
            "Mod package directory",
        )?;
        self.opener.open_directory(&path)
    }
}

#[cfg(test)]
mod tests;
