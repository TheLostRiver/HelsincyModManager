use hmm_core::ModId;
use hmm_ports::{ModImportResultRepository, ModMetadataRepository, ModPackageDirectoryOpener};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ModShortcutError {
    #[error("Mod identity is invalid")]
    InvalidMod,
    #[error("Mod is no longer in the library")]
    NotFound,
    #[error("Mod information is unavailable")]
    Unavailable,
    #[error("Mod folder is missing or cannot be opened")]
    FolderUnavailable,
    #[error("Mod has no valid NexusMods ID")]
    NexusIdMissing,
}

impl ModShortcutError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidMod => "mod_shortcut_mod_invalid",
            Self::NotFound => "mod_shortcut_mod_not_found",
            Self::Unavailable => "mod_shortcut_unavailable",
            Self::FolderUnavailable => "mod_folder_unavailable",
            Self::NexusIdMissing => "mod_nexus_id_missing",
        }
    }
}

/// Resolve shortcuts from the same display revision and metadata as the Mod library.
/// Callers supply a Mod identity, never a filesystem path or an arbitrary URL.
pub struct ModShortcutService {
    imports: Arc<dyn ModImportResultRepository>,
    metadata: Arc<dyn ModMetadataRepository>,
    directories: Arc<dyn ModPackageDirectoryOpener>,
    nexus_page_base: &'static str,
}

impl ModShortcutService {
    pub fn new(
        imports: Arc<dyn ModImportResultRepository>,
        metadata: Arc<dyn ModMetadataRepository>,
        directories: Arc<dyn ModPackageDirectoryOpener>,
        nexus_page_base: &'static str,
    ) -> Self {
        Self {
            imports,
            metadata,
            directories,
            nexus_page_base,
        }
    }

    fn package_id(&self, mod_id: &str) -> Result<String, ModShortcutError> {
        if mod_id.trim().is_empty() || mod_id.len() > 256 {
            return Err(ModShortcutError::InvalidMod);
        }
        let mod_id = ModId::new(mod_id);
        let entry = self
            .imports
            .get_mod(&mod_id)
            .map_err(|_| ModShortcutError::Unavailable)?
            .ok_or(ModShortcutError::NotFound)?;
        let revision = self
            .imports
            .get_revision(&entry.display_revision_id)
            .map_err(|_| ModShortcutError::Unavailable)?
            .ok_or(ModShortcutError::Unavailable)?;
        if entry.mod_id != mod_id
            || revision.mod_id != mod_id
            || revision.revision_id != entry.display_revision_id
        {
            return Err(ModShortcutError::Unavailable);
        }
        Ok(revision.package_id)
    }

    pub fn open_mod_folder(&self, mod_id: &str) -> Result<(), ModShortcutError> {
        let package_id = self.package_id(mod_id)?;
        self.directories
            .open_package_directory(&package_id)
            .map_err(|_| ModShortcutError::FolderUnavailable)
    }

    pub fn nexus_page_url(&self, mod_id: &str) -> Result<String, ModShortcutError> {
        self.package_id(mod_id)?;
        let id = self
            .metadata
            .get(mod_id)
            .map_err(|_| ModShortcutError::Unavailable)?
            .filter(|overlay| overlay.mod_id.as_str() == mod_id)
            .and_then(|overlay| overlay.nexus_mod_id)
            .filter(|id| *id > 0)
            .ok_or(ModShortcutError::NexusIdMissing)?;
        Ok(format!("{}{id}", self.nexus_page_base))
    }
}

#[cfg(test)]
mod tests;
