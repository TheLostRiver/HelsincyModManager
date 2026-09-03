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
            ModStorageDirectoryError::Unavailable,
        ];
        let codes = all.iter().map(|error| error.code()).collect::<Vec<_>>();
        let unique = codes.iter().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), all.len());
        assert!(codes
            .iter()
            .all(|code| code.starts_with("mod_storage_dir_")));
    }
}
