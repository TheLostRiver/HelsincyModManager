use std::path::{Path, PathBuf};
use thiserror::Error;

/// Facts about the source archive taken when an import starts, so that the archive removed
/// afterwards ("move instead of copy", #275) is provably the file that was unpacked and not
/// something swapped in at the same path while the import ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModImportArchiveFingerprint {
    pub len: u64,
    pub modified_unix_millis: Option<u128>,
    /// Platform file identity (volume + index on Windows, device + inode elsewhere).
    pub identity: Option<ModImportArchiveIdentity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModImportArchiveIdentity {
    pub volume: u64,
    pub index: u64,
}

/// Why the source archive was kept after a successful import. These ride on the *completed*
/// event as a degradation code: the import itself is done, only the optional cleanup did not
/// happen. Prefixed `mod_import_archive_kept_` so the family stays grep-able.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ModImportArchiveConsumeError {
    #[error("the archive is not a regular file")]
    NotRegularFile,
    #[error("the archive lies inside a protected directory")]
    ProtectedLocation,
    #[error("the archive changed since the import started")]
    Changed,
    #[error("the archive could not be inspected")]
    Unavailable,
    #[error("the archive could not be removed")]
    RemoveFailed,
}

impl ModImportArchiveConsumeError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotRegularFile => "mod_import_archive_kept_not_regular_file",
            Self::ProtectedLocation => "mod_import_archive_kept_protected_location",
            Self::Changed => "mod_import_archive_kept_changed",
            Self::Unavailable => "mod_import_archive_kept_unavailable",
            Self::RemoveFailed => "mod_import_archive_kept_remove_failed",
        }
    }
}

/// Removes a user's source archive after its contents were imported. Every check is the
/// implementation's job: no-follow opens, regular-file only, identity comparison against the
/// fingerprint taken at start, and refusal inside any protected root (game directories, the
/// Mod storage root, app-data). Nothing here ever touches the archive's contents.
pub trait ModImportArchiveConsumer: Send + Sync {
    fn fingerprint(
        &self,
        archive_path: &Path,
    ) -> Result<ModImportArchiveFingerprint, ModImportArchiveConsumeError>;

    fn consume(
        &self,
        archive_path: &Path,
        expected: &ModImportArchiveFingerprint,
        protected_roots: &[PathBuf],
    ) -> Result<(), ModImportArchiveConsumeError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kept_code_is_unique_and_prefixed() {
        let all = [
            ModImportArchiveConsumeError::NotRegularFile,
            ModImportArchiveConsumeError::ProtectedLocation,
            ModImportArchiveConsumeError::Changed,
            ModImportArchiveConsumeError::Unavailable,
            ModImportArchiveConsumeError::RemoveFailed,
        ];
        let codes = all.iter().map(|error| error.code()).collect::<Vec<_>>();
        let unique = codes.iter().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), all.len());
        assert!(codes
            .iter()
            .all(|code| code.starts_with("mod_import_archive_kept_")));
    }
}
