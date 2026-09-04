//! Removes a user's source archive after a zip import succeeded ("move instead of copy", #275).
//!
//! The archive is the user's own file at a path they picked, so the only things that can go
//! wrong are the ones this module refuses: following a link, deleting something that is not a
//! regular file, deleting a file that was swapped in while the import ran, or deleting inside
//! a directory HMM must never write outside its executors (game roots, the Mod storage root,
//! app-data). The check-then-delete sequence mirrors the verified retention delete in
//! `save_backup.rs`: metadata → no-follow open → identity → re-open → compare → remove.

use crate::controlled_fs::{
    ensure_regular_file_metadata, is_not_found, open_existing_directory_nofollow,
    open_regular_file_nofollow,
};
use cap_std::fs::Dir;
use hmm_ports::{
    ModImportArchiveConsumeError, ModImportArchiveConsumer, ModImportArchiveFingerprint,
    ModImportArchiveIdentity,
};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub struct FileSystemModImportArchiveConsumer;

impl ModImportArchiveConsumer for FileSystemModImportArchiveConsumer {
    fn fingerprint(
        &self,
        archive_path: &Path,
    ) -> Result<ModImportArchiveFingerprint, ModImportArchiveConsumeError> {
        let (parent, name) = split_archive_path(archive_path)?;
        let file = open_verified_archive(&parent, name)?;
        fingerprint_of(&file)
    }

    fn consume(
        &self,
        archive_path: &Path,
        expected: &ModImportArchiveFingerprint,
        protected_roots: &[PathBuf],
    ) -> Result<(), ModImportArchiveConsumeError> {
        // Policy first: a protected location is refused even when the file is already gone.
        if lies_inside_protected_root(archive_path, protected_roots) {
            return Err(ModImportArchiveConsumeError::ProtectedLocation);
        }
        let (parent, name) = split_archive_path(archive_path)?;
        let file = open_verified_archive(&parent, name)?;
        let current = fingerprint_of(&file)?;
        if current != *expected {
            return Err(ModImportArchiveConsumeError::Changed);
        }
        // Windows refuses to delete a file with an open handle; the identity was just re-checked.
        drop(file);
        match parent.remove_file(name) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(ModImportArchiveConsumeError::Changed)
            }
            Err(_) => Err(ModImportArchiveConsumeError::RemoveFailed),
        }
    }
}

fn split_archive_path(archive_path: &Path) -> Result<(Dir, &OsStr), ModImportArchiveConsumeError> {
    if !archive_path.is_absolute() {
        return Err(ModImportArchiveConsumeError::Unavailable);
    }
    let parent = archive_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or(ModImportArchiveConsumeError::Unavailable)?;
    let name = archive_path
        .file_name()
        .ok_or(ModImportArchiveConsumeError::Unavailable)?;
    let parent = open_existing_directory_nofollow(parent, "Mod import archive parent directory")
        .map_err(|error| {
            if is_not_found(&error) {
                ModImportArchiveConsumeError::Changed
            } else {
                ModImportArchiveConsumeError::Unavailable
            }
        })?;
    Ok((parent, name))
}

fn open_verified_archive(
    parent: &Dir,
    name: &OsStr,
) -> Result<cap_std::fs::File, ModImportArchiveConsumeError> {
    let metadata = match parent.symlink_metadata(name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ModImportArchiveConsumeError::Changed);
        }
        Err(_) => return Err(ModImportArchiveConsumeError::Unavailable),
    };
    ensure_regular_file_metadata(&metadata, "Mod import archive")
        .map_err(|_| ModImportArchiveConsumeError::NotRegularFile)?;
    match open_regular_file_nofollow(parent, name, "Mod import archive") {
        Ok(file) => Ok(file),
        Err(error) if is_not_found(&error) => Err(ModImportArchiveConsumeError::Changed),
        Err(_) => Err(ModImportArchiveConsumeError::NotRegularFile),
    }
}

fn fingerprint_of(
    file: &cap_std::fs::File,
) -> Result<ModImportArchiveFingerprint, ModImportArchiveConsumeError> {
    let metadata = file
        .metadata()
        .map_err(|_| ModImportArchiveConsumeError::Unavailable)?;
    Ok(ModImportArchiveFingerprint {
        len: metadata.len(),
        modified_unix_millis: metadata
            .modified()
            .ok()
            .and_then(|modified| modified.into_std().duration_since(UNIX_EPOCH).ok())
            .map(|elapsed| elapsed.as_millis()),
        identity: file_identity(file),
    })
}

fn file_identity(file: &cap_std::fs::File) -> Option<ModImportArchiveIdentity> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle as _;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };

        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        let succeeded = unsafe {
            GetFileInformationByHandle(
                file.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE,
                &mut information,
            )
        };
        if succeeded == 0 {
            return None;
        }
        Some(ModImportArchiveIdentity {
            volume: u64::from(information.dwVolumeSerialNumber),
            index: (u64::from(information.nFileIndexHigh) << 32)
                | u64::from(information.nFileIndexLow),
        })
    }
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt as _;
        let metadata = file.metadata().ok()?;
        Some(ModImportArchiveIdentity {
            volume: metadata.dev(),
            index: metadata.ino(),
        })
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = file;
        None
    }
}

/// True when the archive is, or lies below, any protected root. Both sides are canonicalised
/// (the archive's parent must exist; a root that cannot be canonicalised is compared lexically
/// after case folding so an unplugged storage root still protects its lexical subtree).
fn lies_inside_protected_root(archive_path: &Path, protected_roots: &[PathBuf]) -> bool {
    let Some(archive_anchor) = canonical_anchor(archive_path) else {
        return true;
    };
    protected_roots.iter().any(|root| {
        let root_anchor = canonical_anchor(root).unwrap_or_else(|| normalize_case(root.clone()));
        archive_anchor.starts_with(&root_anchor)
    })
}

fn canonical_anchor(path: &Path) -> Option<PathBuf> {
    let existing = path.ancestors().find(|ancestor| ancestor.exists())?;
    let canonical = existing.canonicalize().ok()?;
    let tail = path.strip_prefix(existing).ok()?;
    Some(normalize_case(canonical.join(tail)))
}

#[cfg(windows)]
fn normalize_case(path: PathBuf) -> PathBuf {
    PathBuf::from(path.to_string_lossy().to_ascii_lowercase())
}

#[cfg(not(windows))]
fn normalize_case(path: PathBuf) -> PathBuf {
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn consumer() -> FileSystemModImportArchiveConsumer {
        FileSystemModImportArchiveConsumer
    }

    fn write_archive(directory: &Path, name: &str) -> PathBuf {
        fs::create_dir_all(directory).expect("create directory");
        let path = directory.join(name);
        fs::write(&path, b"PK\x03\x04 fixture bytes").expect("write archive");
        path
    }

    #[test]
    fn consumes_the_same_regular_file_it_fingerprinted() {
        let temp = tempfile::tempdir().expect("temp");
        let archive = write_archive(&temp.path().join("downloads"), "mod.zip");
        let sibling = write_archive(&temp.path().join("downloads"), "other.zip");

        let fingerprint = consumer().fingerprint(&archive).expect("fingerprint");
        assert!(fingerprint.len > 0);
        assert!(fingerprint.identity.is_some());
        consumer()
            .consume(&archive, &fingerprint, &[])
            .expect("consume");

        assert!(!archive.exists(), "the imported archive is removed");
        assert!(sibling.exists(), "nothing else in the directory is touched");
    }

    #[test]
    fn a_replaced_archive_is_kept() {
        let temp = tempfile::tempdir().expect("temp");
        let archive = write_archive(temp.path(), "mod.zip");
        let fingerprint = consumer().fingerprint(&archive).expect("fingerprint");

        fs::remove_file(&archive).expect("remove original");
        fs::write(&archive, b"PK\x03\x04 a different file with more bytes").expect("swap in");

        assert_eq!(
            consumer().consume(&archive, &fingerprint, &[]),
            Err(ModImportArchiveConsumeError::Changed)
        );
        assert!(
            archive.exists(),
            "a file that is not the imported one stays"
        );
    }

    #[test]
    fn a_missing_archive_reports_changed_without_failing_hard() {
        let temp = tempfile::tempdir().expect("temp");
        let archive = write_archive(temp.path(), "mod.zip");
        let fingerprint = consumer().fingerprint(&archive).expect("fingerprint");
        fs::remove_file(&archive).expect("remove");

        assert_eq!(
            consumer().consume(&archive, &fingerprint, &[]),
            Err(ModImportArchiveConsumeError::Changed)
        );
    }

    #[test]
    fn archives_inside_protected_roots_are_kept() {
        let temp = tempfile::tempdir().expect("temp");
        let game_root = temp.path().join("Games").join("MHW");
        let archive = write_archive(&game_root.join("mods"), "mod.zip");
        let outside = write_archive(&temp.path().join("Downloads"), "mod.zip");
        let fingerprint = consumer().fingerprint(&archive).expect("fingerprint");
        let outside_fingerprint = consumer().fingerprint(&outside).expect("fingerprint");
        let missing_root = temp.path().join("unplugged-storage");

        assert_eq!(
            consumer().consume(&archive, &fingerprint, std::slice::from_ref(&game_root)),
            Err(ModImportArchiveConsumeError::ProtectedLocation)
        );
        assert!(archive.exists());
        assert_eq!(
            consumer().consume(
                &missing_root.join("mod.zip"),
                &fingerprint,
                std::slice::from_ref(&missing_root)
            ),
            Err(ModImportArchiveConsumeError::ProtectedLocation),
            "a root that does not exist right now still protects its lexical subtree"
        );
        consumer()
            .consume(&outside, &outside_fingerprint, &[game_root, missing_root])
            .expect("an archive next to, not inside, the roots is consumed");
        assert!(!outside.exists());
    }

    #[test]
    fn directories_and_relative_paths_are_never_consumed() {
        let temp = tempfile::tempdir().expect("temp");
        let directory = temp.path().join("mod.zip");
        fs::create_dir_all(&directory).expect("create directory named like an archive");

        assert_eq!(
            consumer().fingerprint(&directory),
            Err(ModImportArchiveConsumeError::NotRegularFile)
        );
        assert_eq!(
            consumer().fingerprint(Path::new("relative/mod.zip")),
            Err(ModImportArchiveConsumeError::Unavailable)
        );
        assert!(directory.is_dir());
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_on_the_archive_path_is_rejected() {
        let temp = tempfile::tempdir().expect("temp");
        let real = temp.path().join("real");
        let archive = write_archive(&real, "mod.zip");
        let link = temp.path().join("link");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&real)
            .status()
            .expect("run mklink");
        assert!(status.success(), "mklink /J must succeed");

        assert!(
            consumer().fingerprint(&link.join("mod.zip")).is_err(),
            "the archive path must not go through a junction"
        );
        assert!(archive.exists());
    }
}
