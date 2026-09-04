use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

/// Marker written into a user-chosen Mod storage directory the first time HMM claims it.
/// Byte-exact like the sandbox marker: a non-empty directory without it is never adopted, so
/// deletion and migration only ever touch directories HMM created or was explicitly handed.
pub const MOD_STORAGE_MARKER_NAME: &str = ".hmm-mod-storage.json";
pub const MOD_STORAGE_MARKER_SCHEMA: &str = "{\"kind\":\"hmm.mod-storage\",\"schemaVersion\":1}\n";
/// Child of the storage root that holds one directory per imported package.
pub const MOD_STORAGE_SANDBOX_DIRECTORY: &str = "sandboxes";
/// Child of the app-data root that is the storage root when the user has not configured one.
pub const DEFAULT_MOD_STORAGE_DIRECTORY: &str = "mod-import";

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ModStorageDirectoryError {
    #[error("mod storage directory must be an absolute path")]
    NotAbsolute,
    #[error("mod storage directory contains an unsafe path component")]
    UnsafeComponent,
    #[error("mod storage directory must not be a file system root")]
    FileSystemRoot,
    #[error("mod storage directory parent does not exist")]
    ParentMissing,
    #[error("mod storage directory is not a directory")]
    NotDirectory,
    #[error("mod storage directory path goes through a link or reparse point")]
    LinkRejected,
    #[error("non-empty mod storage directory has no HMM marker")]
    MarkerRequired,
    #[error("mod storage directory marker is invalid")]
    MarkerInvalid,
    #[error("mod storage directory is not writable")]
    NotWritable,
    #[error("mod storage directory overlaps a configured game directory")]
    OverlapsGameRoot,
    /// The candidate is, contains, or lies inside the storage root currently in effect. A root
    /// nested in another root would make one root's packages appear as foreign entries of the
    /// other, so neither `set` nor a migration ever accepts it.
    #[error("mod storage directory overlaps the current storage root")]
    OverlapsCurrentRoot,
    #[error("mod storage directory could not be inspected")]
    Unavailable,
}

impl ModStorageDirectoryError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotAbsolute => "mod_storage_dir_not_absolute",
            Self::UnsafeComponent => "mod_storage_dir_unsafe",
            Self::FileSystemRoot => "mod_storage_dir_filesystem_root",
            Self::ParentMissing => "mod_storage_dir_parent_missing",
            Self::NotDirectory => "mod_storage_dir_not_directory",
            Self::LinkRejected => "mod_storage_dir_link_rejected",
            Self::MarkerRequired => "mod_storage_dir_marker_required",
            Self::MarkerInvalid => "mod_storage_dir_marker_invalid",
            Self::NotWritable => "mod_storage_dir_not_writable",
            Self::OverlapsGameRoot => "mod_storage_dir_overlaps_game_root",
            Self::OverlapsCurrentRoot => "mod_storage_dir_overlaps_current_root",
            Self::Unavailable => "mod_storage_dir_unavailable",
        }
    }
}

/// Read-only verdict about a candidate storage directory. `claimed` means a valid marker is
/// present; `exists` false means the directory itself is absent but its parent is usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModStorageDirectoryInspection {
    pub exists: bool,
    pub claimed: bool,
}

pub struct ModStorageDirectoryInspectionRequest<'a> {
    pub path: &'a Path,
    /// Directories the storage root may neither contain nor live inside (configured game roots).
    pub exclusive_roots: &'a [PathBuf],
    /// Storage root in effect for the running process; a candidate overlapping it fails with
    /// [`ModStorageDirectoryError::OverlapsCurrentRoot`]. `None` skips the check.
    pub current_root: Option<&'a Path>,
}

/// File-system facts about Mod storage directories. Implementations own every IO decision
/// (no-follow opens, marker bytes, write probes); services only combine the verdicts.
pub trait ModStorageDirectoryInspector: Send + Sync {
    /// Validates shape and file-system state without changing anything except a transient
    /// write-probe file that is removed again.
    fn inspect(
        &self,
        request: ModStorageDirectoryInspectionRequest<'_>,
    ) -> Result<ModStorageDirectoryInspection, ModStorageDirectoryError>;

    /// Creates the directory when absent and writes the marker when the directory is empty or
    /// only holds HMM's own layout. Anything else fails closed.
    fn claim(&self, path: &Path) -> Result<(), ModStorageDirectoryError>;

    /// Startup check for a directory HMM claimed earlier: it must still exist as a plain
    /// directory (no link or reparse point on the way) and hold either a valid marker or nothing
    /// but HMM's own layout. No write probe, no marker rewrite — startup stays read-only.
    fn verify_claimed(&self, path: &Path) -> Result<(), ModStorageDirectoryError>;

    /// Whether `<storage_root>/sandboxes` holds any entry at all (packages, orphans, or
    /// in-flight task directories). A missing directory counts as empty.
    fn sandbox_directory_has_entries(
        &self,
        storage_root: &Path,
    ) -> Result<bool, ModStorageDirectoryError>;

    /// Whether one directory contains the other (or both are the same directory) after
    /// canonicalisation. Unresolvable paths are treated as overlapping.
    fn directories_overlap(&self, left: &Path, right: &Path) -> bool;
}

/// Stable failure codes of a storage-root migration (#275 slice 2). Every code is prefixed
/// `mod_storage_migration_` so task events and command errors stay grep-able as one family.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ModStorageMigrationError {
    #[error("source storage root is unavailable")]
    SourceUnavailable,
    #[error("target storage root is unavailable")]
    TargetUnavailable,
    #[error("a package below the storage root could not be read")]
    PackageUnreadable,
    #[error("copying a package failed")]
    CopyFailed,
    #[error("a copied package does not match its source")]
    VerifyMismatch,
    /// Startup cleanup found a switched journal whose target lacks a listed package; the source
    /// copy is kept and the journal stays until the target is complete again.
    #[error("a migrated package is missing from the target storage root")]
    TargetPackageMissing,
    #[error("the migration journal could not be read or written")]
    JournalUnavailable,
    #[error("migration was cancelled")]
    Cancelled,
}

impl ModStorageMigrationError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::SourceUnavailable => "mod_storage_migration_source_unavailable",
            Self::TargetUnavailable => "mod_storage_migration_target_unavailable",
            Self::PackageUnreadable => "mod_storage_migration_package_unreadable",
            Self::CopyFailed => "mod_storage_migration_copy_failed",
            Self::VerifyMismatch => "mod_storage_migration_verify_mismatch",
            Self::TargetPackageMissing => "mod_storage_migration_target_package_missing",
            Self::JournalUnavailable => "mod_storage_migration_journal_unavailable",
            Self::Cancelled => "mod_storage_migration_cancelled",
        }
    }
}

/// Where a migration stands, persisted before each irreversible step so a crash can be
/// finished or rolled back at the next start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModStorageMigrationState {
    /// Packages are being copied into the target; the setting still names the source.
    Copying,
    /// Every package was copied and verified; the setting names the target; the source copies
    /// are deleted at the next start, once the new root is in effect.
    Switched,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModStorageMigrationJournal {
    pub version: u32,
    pub state: ModStorageMigrationState,
    pub source_root: PathBuf,
    pub target_root: PathBuf,
    /// Package directory names (direct children of `<root>/sandboxes`) covered by this run.
    pub packages: Vec<String>,
    pub started_at_unix_millis: u128,
}

pub const MOD_STORAGE_MIGRATION_JOURNAL_VERSION: u32 = 1;

pub trait ModStorageMigrationJournalRepository: Send + Sync {
    fn load(&self) -> Result<Option<ModStorageMigrationJournal>, ModStorageMigrationError>;
    fn save(&self, journal: &ModStorageMigrationJournal) -> Result<(), ModStorageMigrationError>;
    fn clear(&self) -> Result<(), ModStorageMigrationError>;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModStoragePackageCopyReport {
    pub file_count: u64,
    pub byte_count: u64,
}

/// File-system operations of a migration. Every operation works on `<root>/sandboxes/<pkg>`
/// through no-follow handles; a link or reparse point anywhere inside a package fails closed.
pub trait ModStorageMigrator: Send + Sync {
    /// Direct children of `<storage_root>/sandboxes`, each of which must be a plain directory.
    /// A missing `sandboxes/` directory is an empty library.
    fn list_packages(&self, storage_root: &Path) -> Result<Vec<String>, ModStorageMigrationError>;

    /// Copies one package into the target root, replacing any leftover with the same name, and
    /// fsyncs every written file. Checks `cancellation` between files.
    fn copy_package(
        &self,
        source_root: &Path,
        target_root: &Path,
        package_id: &str,
        cancellation: &dyn crate::CancellationToken,
    ) -> Result<ModStoragePackageCopyReport, ModStorageMigrationError>;

    /// Re-reads both copies and requires identical file sets, sizes and SHA-256 digests.
    fn verify_package(
        &self,
        source_root: &Path,
        target_root: &Path,
        package_id: &str,
        cancellation: &dyn crate::CancellationToken,
    ) -> Result<(), ModStorageMigrationError>;

    /// Removes `<storage_root>/sandboxes/<package_id>`; a missing package is not an error.
    fn remove_package(
        &self,
        storage_root: &Path,
        package_id: &str,
    ) -> Result<(), ModStorageMigrationError>;

    fn package_exists(
        &self,
        storage_root: &Path,
        package_id: &str,
    ) -> Result<bool, ModStorageMigrationError>;
}

/// Pure shape rules shared by every layer; no file-system access.
pub fn validate_mod_storage_directory_shape(path: &Path) -> Result<(), ModStorageDirectoryError> {
    if !path.is_absolute() {
        return Err(ModStorageDirectoryError::NotAbsolute);
    }
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        || path
            .as_os_str()
            .to_string_lossy()
            .split(['/', '\\'])
            .any(|component| matches!(component, "." | ".."))
    {
        return Err(ModStorageDirectoryError::UnsafeComponent);
    }
    if path.file_name().is_none()
        || !path
            .components()
            .any(|component| matches!(component, Component::Normal(_)))
    {
        return Err(ModStorageDirectoryError::FileSystemRoot);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    const ABSOLUTE: &str = "D:\\HMMMods";
    #[cfg(not(windows))]
    const ABSOLUTE: &str = "/srv/hmm-mods";

    #[test]
    fn marker_schema_is_byte_exact_json_line() {
        assert_eq!(MOD_STORAGE_MARKER_SCHEMA.len(), 45);
        assert!(MOD_STORAGE_MARKER_SCHEMA.ends_with('\n'));
        let parsed: serde_json::Value =
            serde_json::from_str(MOD_STORAGE_MARKER_SCHEMA).expect("marker is json");
        assert_eq!(parsed["kind"], "hmm.mod-storage");
        assert_eq!(parsed["schemaVersion"], 1);
    }

    #[test]
    fn shape_accepts_a_plain_absolute_directory() {
        assert_eq!(
            validate_mod_storage_directory_shape(Path::new(ABSOLUTE)),
            Ok(())
        );
    }

    #[test]
    fn shape_rejects_relative_paths() {
        assert_eq!(
            validate_mod_storage_directory_shape(Path::new("mods")),
            Err(ModStorageDirectoryError::NotAbsolute)
        );
    }

    #[test]
    fn shape_rejects_dot_components() {
        let path = PathBuf::from(ABSOLUTE).join("..").join("other");
        assert_eq!(
            validate_mod_storage_directory_shape(&path),
            Err(ModStorageDirectoryError::UnsafeComponent)
        );
    }

    #[test]
    fn shape_rejects_file_system_roots() {
        #[cfg(windows)]
        let root = Path::new("D:\\");
        #[cfg(not(windows))]
        let root = Path::new("/");
        assert_eq!(
            validate_mod_storage_directory_shape(root),
            Err(ModStorageDirectoryError::FileSystemRoot)
        );
    }

    #[test]
    fn every_error_code_is_unique_and_prefixed() {
        let all = [
            ModStorageDirectoryError::NotAbsolute,
            ModStorageDirectoryError::UnsafeComponent,
            ModStorageDirectoryError::FileSystemRoot,
            ModStorageDirectoryError::ParentMissing,
            ModStorageDirectoryError::NotDirectory,
            ModStorageDirectoryError::LinkRejected,
            ModStorageDirectoryError::MarkerRequired,
            ModStorageDirectoryError::MarkerInvalid,
            ModStorageDirectoryError::NotWritable,
            ModStorageDirectoryError::OverlapsGameRoot,
            ModStorageDirectoryError::OverlapsCurrentRoot,
            ModStorageDirectoryError::Unavailable,
        ];
        let codes = all.iter().map(|error| error.code()).collect::<Vec<_>>();
        let unique = codes.iter().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), all.len());
        assert!(codes
            .iter()
            .all(|code| code.starts_with("mod_storage_dir_")));
    }

    #[test]
    fn every_migration_error_code_is_unique_and_prefixed() {
        let all = [
            ModStorageMigrationError::SourceUnavailable,
            ModStorageMigrationError::TargetUnavailable,
            ModStorageMigrationError::PackageUnreadable,
            ModStorageMigrationError::CopyFailed,
            ModStorageMigrationError::VerifyMismatch,
            ModStorageMigrationError::TargetPackageMissing,
            ModStorageMigrationError::JournalUnavailable,
            ModStorageMigrationError::Cancelled,
        ];
        let codes = all.iter().map(|error| error.code()).collect::<Vec<_>>();
        let unique = codes.iter().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), all.len());
        assert!(codes
            .iter()
            .all(|code| code.starts_with("mod_storage_migration_")));
    }
}
