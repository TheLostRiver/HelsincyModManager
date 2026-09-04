//! File-system side of a storage-root migration (#275 slice 2).
//!
//! Every package copy walks both trees through capability handles opened no-follow, so a link
//! or reparse point planted inside a package cannot redirect the copy or the verification. A
//! package is only ever *added* to the target and *removed* from a root as one whole directory;
//! nothing here rewrites files in place.

use crate::controlled_fs::{
    create_new_child_directory, create_new_regular_file, is_link_or_reparse,
    open_child_directory_nofollow, open_existing_directory_chain, open_existing_directory_nofollow,
    open_or_create_directory_chain, open_or_create_directory_nofollow, open_regular_file_nofollow,
    remove_child_tree_nofollow,
};
use crate::mod_import::validate_task_id_segment;
use cap_std::fs::Dir;
use hmm_ports::{
    CancellationToken, ModStorageMigrationError, ModStorageMigrationJournal,
    ModStorageMigrationJournalRepository, ModStorageMigrator, ModStoragePackageCopyReport,
    MOD_STORAGE_MIGRATION_JOURNAL_VERSION, MOD_STORAGE_SANDBOX_DIRECTORY,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct FileSystemModStorageMigrator;

type TreeDigest = BTreeMap<PathBuf, (u64, [u8; 32])>;

impl ModStorageMigrator for FileSystemModStorageMigrator {
    fn list_packages(&self, storage_root: &Path) -> Result<Vec<String>, ModStorageMigrationError> {
        let root = open_existing_directory_nofollow(storage_root, "mod storage root")
            .map_err(|_| ModStorageMigrationError::SourceUnavailable)?;
        let sandboxes = match open_existing_directory_chain(
            &root,
            &[MOD_STORAGE_SANDBOX_DIRECTORY],
            "mod import sandbox root",
        ) {
            Ok(sandboxes) => sandboxes,
            Err(error) if crate::controlled_fs::is_not_found(&error) => return Ok(Vec::new()),
            Err(_) => return Err(ModStorageMigrationError::SourceUnavailable),
        };
        let mut packages = Vec::new();
        for entry in sandboxes
            .entries()
            .map_err(|_| ModStorageMigrationError::SourceUnavailable)?
        {
            let entry = entry.map_err(|_| ModStorageMigrationError::SourceUnavailable)?;
            let name = entry.file_name();
            let metadata = sandboxes
                .symlink_metadata(&name)
                .map_err(|_| ModStorageMigrationError::PackageUnreadable)?;
            let name = name
                .to_str()
                .ok_or(ModStorageMigrationError::PackageUnreadable)?
                .to_owned();
            if !metadata.is_dir()
                || is_link_or_reparse(&metadata)
                || validate_task_id_segment(&name).is_err()
            {
                return Err(ModStorageMigrationError::PackageUnreadable);
            }
            packages.push(name);
        }
        packages.sort();
        Ok(packages)
    }

    fn copy_package(
        &self,
        source_root: &Path,
        target_root: &Path,
        package_id: &str,
        cancellation: &dyn CancellationToken,
    ) -> Result<ModStoragePackageCopyReport, ModStorageMigrationError> {
        validate_task_id_segment(package_id)
            .map_err(|_| ModStorageMigrationError::PackageUnreadable)?;
        let source_package = open_source_package(source_root, package_id)?;
        let target_root = open_or_create_directory_nofollow(target_root, "mod storage root")
            .map_err(|_| ModStorageMigrationError::TargetUnavailable)?;
        let target_sandboxes = open_or_create_directory_chain(
            &target_root,
            &[MOD_STORAGE_SANDBOX_DIRECTORY],
            "mod import sandbox root",
        )
        .map_err(|_| ModStorageMigrationError::TargetUnavailable)?;
        // A previous, interrupted attempt may have left a partial copy behind; it is never
        // trusted, always rebuilt from the source.
        remove_child_tree_nofollow(
            &target_sandboxes,
            OsStr::new(package_id),
            "mod import sandbox",
        )
        .map_err(|_| ModStorageMigrationError::TargetUnavailable)?;
        let target_package = create_new_child_directory(
            &target_sandboxes,
            OsStr::new(package_id),
            "mod import sandbox",
        )
        .map_err(|_| ModStorageMigrationError::CopyFailed)?;
        let mut report = ModStoragePackageCopyReport::default();
        copy_tree(&source_package, &target_package, cancellation, &mut report)?;
        Ok(report)
    }

    fn verify_package(
        &self,
        source_root: &Path,
        target_root: &Path,
        package_id: &str,
        cancellation: &dyn CancellationToken,
    ) -> Result<(), ModStorageMigrationError> {
        validate_task_id_segment(package_id)
            .map_err(|_| ModStorageMigrationError::PackageUnreadable)?;
        let source_package = open_source_package(source_root, package_id)?;
        let target_package = open_existing_package(target_root, package_id)
            .map_err(|_| ModStorageMigrationError::VerifyMismatch)?;
        let mut source_digest = TreeDigest::new();
        digest_tree(
            &source_package,
            PathBuf::new(),
            cancellation,
            &mut source_digest,
            ModStorageMigrationError::PackageUnreadable,
        )?;
        let mut target_digest = TreeDigest::new();
        digest_tree(
            &target_package,
            PathBuf::new(),
            cancellation,
            &mut target_digest,
            ModStorageMigrationError::VerifyMismatch,
        )?;
        if source_digest == target_digest {
            Ok(())
        } else {
            Err(ModStorageMigrationError::VerifyMismatch)
        }
    }

    fn remove_package(
        &self,
        storage_root: &Path,
        package_id: &str,
    ) -> Result<(), ModStorageMigrationError> {
        validate_task_id_segment(package_id)
            .map_err(|_| ModStorageMigrationError::PackageUnreadable)?;
        let root = match open_existing_directory_nofollow(storage_root, "mod storage root") {
            Ok(root) => root,
            Err(error) if crate::controlled_fs::is_not_found(&error) => return Ok(()),
            Err(_) => return Err(ModStorageMigrationError::SourceUnavailable),
        };
        let sandboxes = match open_existing_directory_chain(
            &root,
            &[MOD_STORAGE_SANDBOX_DIRECTORY],
            "mod import sandbox root",
        ) {
            Ok(sandboxes) => sandboxes,
            Err(error) if crate::controlled_fs::is_not_found(&error) => return Ok(()),
            Err(_) => return Err(ModStorageMigrationError::SourceUnavailable),
        };
        remove_child_tree_nofollow(&sandboxes, OsStr::new(package_id), "mod import sandbox")
            .map_err(|_| ModStorageMigrationError::PackageUnreadable)
    }

    fn package_exists(
        &self,
        storage_root: &Path,
        package_id: &str,
    ) -> Result<bool, ModStorageMigrationError> {
        validate_task_id_segment(package_id)
            .map_err(|_| ModStorageMigrationError::PackageUnreadable)?;
        match fs::symlink_metadata(
            storage_root
                .join(MOD_STORAGE_SANDBOX_DIRECTORY)
                .join(package_id),
        ) {
            Ok(metadata) => {
                if is_std_link_or_reparse(&metadata) {
                    return Err(ModStorageMigrationError::PackageUnreadable);
                }
                Ok(metadata.is_dir())
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
            Err(_) => Err(ModStorageMigrationError::SourceUnavailable),
        }
    }
}

fn open_source_package(
    source_root: &Path,
    package_id: &str,
) -> Result<Dir, ModStorageMigrationError> {
    let root = open_existing_directory_nofollow(source_root, "mod storage root")
        .map_err(|_| ModStorageMigrationError::SourceUnavailable)?;
    let sandboxes = open_existing_directory_chain(
        &root,
        &[MOD_STORAGE_SANDBOX_DIRECTORY],
        "mod import sandbox root",
    )
    .map_err(|_| ModStorageMigrationError::SourceUnavailable)?;
    open_child_directory_nofollow(&sandboxes, OsStr::new(package_id), "mod import sandbox")
        .map_err(|_| ModStorageMigrationError::PackageUnreadable)
}

fn open_existing_package(storage_root: &Path, package_id: &str) -> anyhow::Result<Dir> {
    let root = open_existing_directory_nofollow(storage_root, "mod storage root")?;
    let sandboxes = open_existing_directory_chain(
        &root,
        &[MOD_STORAGE_SANDBOX_DIRECTORY],
        "mod import sandbox root",
    )?;
    open_child_directory_nofollow(&sandboxes, OsStr::new(package_id), "mod import sandbox")
}

fn copy_tree(
    source: &Dir,
    target: &Dir,
    cancellation: &dyn CancellationToken,
    report: &mut ModStoragePackageCopyReport,
) -> Result<(), ModStorageMigrationError> {
    let entries = source
        .entries()
        .map_err(|_| ModStorageMigrationError::PackageUnreadable)?;
    for entry in entries {
        if cancellation.is_cancelled() {
            return Err(ModStorageMigrationError::Cancelled);
        }
        let entry = entry.map_err(|_| ModStorageMigrationError::PackageUnreadable)?;
        let name = entry.file_name();
        let metadata = source
            .symlink_metadata(&name)
            .map_err(|_| ModStorageMigrationError::PackageUnreadable)?;
        if is_link_or_reparse(&metadata) {
            return Err(ModStorageMigrationError::PackageUnreadable);
        }
        if metadata.is_dir() {
            let source_child = open_child_directory_nofollow(source, &name, "mod import sandbox")
                .map_err(|_| ModStorageMigrationError::PackageUnreadable)?;
            let target_child = create_new_child_directory(target, &name, "mod import sandbox")
                .map_err(|_| ModStorageMigrationError::CopyFailed)?;
            copy_tree(&source_child, &target_child, cancellation, report)?;
        } else if metadata.is_file() {
            let mut source_file = open_regular_file_nofollow(source, &name, "mod import file")
                .map_err(|_| ModStorageMigrationError::PackageUnreadable)?;
            let mut target_file = create_new_regular_file(target, &name, "mod import file")
                .map_err(|_| ModStorageMigrationError::CopyFailed)?;
            let copied = io::copy(&mut source_file, &mut target_file)
                .map_err(|_| ModStorageMigrationError::CopyFailed)?;
            target_file
                .sync_all()
                .map_err(|_| ModStorageMigrationError::CopyFailed)?;
            report.file_count += 1;
            report.byte_count += copied;
        } else {
            return Err(ModStorageMigrationError::PackageUnreadable);
        }
    }
    Ok(())
}

fn digest_tree(
    directory: &Dir,
    relative: PathBuf,
    cancellation: &dyn CancellationToken,
    digest: &mut TreeDigest,
    unreadable: ModStorageMigrationError,
) -> Result<(), ModStorageMigrationError> {
    let entries = directory.entries().map_err(|_| unreadable)?;
    for entry in entries {
        if cancellation.is_cancelled() {
            return Err(ModStorageMigrationError::Cancelled);
        }
        let entry = entry.map_err(|_| unreadable)?;
        let name = entry.file_name();
        let metadata = directory.symlink_metadata(&name).map_err(|_| unreadable)?;
        if is_link_or_reparse(&metadata) {
            return Err(unreadable);
        }
        let child_relative = relative.join(&name);
        if metadata.is_dir() {
            let child = open_child_directory_nofollow(directory, &name, "mod import sandbox")
                .map_err(|_| unreadable)?;
            digest_tree(&child, child_relative, cancellation, digest, unreadable)?;
        } else if metadata.is_file() {
            let mut file = open_regular_file_nofollow(directory, &name, "mod import file")
                .map_err(|_| unreadable)?;
            let mut hasher = Sha256::new();
            let mut buffer = [0u8; 64 * 1024];
            let mut size = 0u64;
            loop {
                let read = file.read(&mut buffer).map_err(|_| unreadable)?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
                size += read as u64;
            }
            digest.insert(child_relative, (size, hasher.finalize().into()));
        } else {
            return Err(unreadable);
        }
    }
    Ok(())
}

fn is_std_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

/// `<app-data>/mod-import/migration.json`, written atomically (temp file + rename + parent
/// fsync) so a crash never leaves a half-written journal.
pub struct JsonModStorageMigrationJournalRepository {
    file_path: PathBuf,
}

impl JsonModStorageMigrationJournalRepository {
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    pub fn now_unix_millis() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis())
            .unwrap_or_default()
    }
}

impl ModStorageMigrationJournalRepository for JsonModStorageMigrationJournalRepository {
    fn load(&self) -> Result<Option<ModStorageMigrationJournal>, ModStorageMigrationError> {
        let bytes = match fs::read(&self.file_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(ModStorageMigrationError::JournalUnavailable),
        };
        let journal: ModStorageMigrationJournal = serde_json::from_slice(&bytes)
            .map_err(|_| ModStorageMigrationError::JournalUnavailable)?;
        if journal.version != MOD_STORAGE_MIGRATION_JOURNAL_VERSION {
            return Err(ModStorageMigrationError::JournalUnavailable);
        }
        Ok(Some(journal))
    }

    fn save(&self, journal: &ModStorageMigrationJournal) -> Result<(), ModStorageMigrationError> {
        let parent = self
            .file_path
            .parent()
            .ok_or(ModStorageMigrationError::JournalUnavailable)?;
        fs::create_dir_all(parent).map_err(|_| ModStorageMigrationError::JournalUnavailable)?;
        let serialized = serde_json::to_vec_pretty(journal)
            .map_err(|_| ModStorageMigrationError::JournalUnavailable)?;
        let temp_path = parent.join(format!(
            "migration.{}.{}.json.tmp",
            std::process::id(),
            Self::now_unix_millis()
        ));
        {
            let mut file = File::create(&temp_path)
                .map_err(|_| ModStorageMigrationError::JournalUnavailable)?;
            file.write_all(&serialized)
                .and_then(|_| file.sync_all())
                .map_err(|_| ModStorageMigrationError::JournalUnavailable)?;
        }
        fs::rename(&temp_path, &self.file_path).map_err(|_| {
            let _ = fs::remove_file(&temp_path);
            ModStorageMigrationError::JournalUnavailable
        })?;
        let _ = open_directory_for_sync(parent).and_then(|directory| directory.sync_all());
        Ok(())
    }

    fn clear(&self) -> Result<(), ModStorageMigrationError> {
        match fs::remove_file(&self.file_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(_) => Err(ModStorageMigrationError::JournalUnavailable),
        }
    }
}

#[cfg(windows)]
fn open_directory_for_sync(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

#[cfg(not(windows))]
fn open_directory_for_sync(path: &Path) -> io::Result<File> {
    File::open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_ports::{ModStorageMigrationState, NeverCancelled};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn write_package(root: &Path, package_id: &str, files: &[(&str, &[u8])]) {
        for (relative, bytes) in files {
            let path = root
                .join(MOD_STORAGE_SANDBOX_DIRECTORY)
                .join(package_id)
                .join(relative);
            fs::create_dir_all(path.parent().expect("parent")).expect("create package dirs");
            fs::write(path, bytes).expect("write package file");
        }
    }

    fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
        fn visit(base: &Path, current: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            for entry in fs::read_dir(current).expect("read dir") {
                let entry = entry.expect("entry");
                let path = entry.path();
                if path.is_dir() {
                    visit(base, &path, out);
                } else {
                    let relative = path
                        .strip_prefix(base)
                        .expect("relative")
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.insert(relative, fs::read(&path).expect("read file"));
                }
            }
        }
        let mut out = BTreeMap::new();
        if root.exists() {
            visit(root, root, &mut out);
        }
        out
    }

    const FILES: &[(&str, &[u8])] = &[
        ("nativePC/wp/one/one001/mod/one001.mod3", b"mod3-bytes"),
        ("nativePC/wp/one/one001/mod/one001.mrl3", b"mrl3-bytes"),
        ("readme.txt", b"readme"),
    ];

    #[test]
    fn list_packages_returns_sorted_directories_and_treats_missing_sandboxes_as_empty() {
        let temp = tempfile::tempdir().expect("temp");
        let root = temp.path().join("mods");
        fs::create_dir(&root).expect("root");
        let migrator = FileSystemModStorageMigrator;

        assert_eq!(
            migrator.list_packages(&root).expect("empty"),
            Vec::<String>::new()
        );
        write_package(&root, "mod-import-2-0", FILES);
        write_package(&root, "mod-import-1-0", FILES);
        assert_eq!(
            migrator.list_packages(&root).expect("two"),
            vec!["mod-import-1-0".to_owned(), "mod-import-2-0".to_owned()]
        );
    }

    #[test]
    fn list_packages_rejects_a_stray_file_below_sandboxes() {
        let temp = tempfile::tempdir().expect("temp");
        let root = temp.path().join("mods");
        write_package(&root, "mod-import-1-0", FILES);
        fs::write(
            root.join(MOD_STORAGE_SANDBOX_DIRECTORY).join("notes.txt"),
            b"x",
        )
        .expect("stray file");

        assert_eq!(
            FileSystemModStorageMigrator.list_packages(&root),
            Err(ModStorageMigrationError::PackageUnreadable)
        );
    }

    #[test]
    fn copy_then_verify_reproduces_the_package_byte_for_byte() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir(&target).expect("target root");
        write_package(&source, "mod-import-1-0", FILES);
        let migrator = FileSystemModStorageMigrator;

        let report = migrator
            .copy_package(&source, &target, "mod-import-1-0", &NeverCancelled)
            .expect("copy");
        migrator
            .verify_package(&source, &target, "mod-import-1-0", &NeverCancelled)
            .expect("verify");

        assert_eq!(report.file_count, 3);
        assert_eq!(report.byte_count, 26);
        assert_eq!(
            tree(
                &target
                    .join(MOD_STORAGE_SANDBOX_DIRECTORY)
                    .join("mod-import-1-0")
            ),
            tree(
                &source
                    .join(MOD_STORAGE_SANDBOX_DIRECTORY)
                    .join("mod-import-1-0")
            )
        );
        assert!(migrator
            .package_exists(&target, "mod-import-1-0")
            .expect("exists"));
    }

    #[test]
    fn copy_replaces_a_leftover_partial_target_copy() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        write_package(&source, "mod-import-1-0", FILES);
        write_package(
            &target,
            "mod-import-1-0",
            &[("stale.bin", b"from an interrupted attempt")],
        );

        FileSystemModStorageMigrator
            .copy_package(&source, &target, "mod-import-1-0", &NeverCancelled)
            .expect("copy over leftover");

        let copied = tree(
            &target
                .join(MOD_STORAGE_SANDBOX_DIRECTORY)
                .join("mod-import-1-0"),
        );
        assert!(!copied.contains_key("stale.bin"));
        assert_eq!(copied.len(), 3);
    }

    #[test]
    fn verify_detects_a_single_changed_byte_and_an_extra_file() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        write_package(&source, "mod-import-1-0", FILES);
        let migrator = FileSystemModStorageMigrator;
        migrator
            .copy_package(&source, &target, "mod-import-1-0", &NeverCancelled)
            .expect("copy");
        let target_file = target
            .join(MOD_STORAGE_SANDBOX_DIRECTORY)
            .join("mod-import-1-0")
            .join("readme.txt");
        fs::write(&target_file, b"readmE").expect("tamper same length");
        assert_eq!(
            migrator.verify_package(&source, &target, "mod-import-1-0", &NeverCancelled),
            Err(ModStorageMigrationError::VerifyMismatch)
        );

        fs::write(&target_file, b"readme").expect("restore");
        fs::write(
            target
                .join(MOD_STORAGE_SANDBOX_DIRECTORY)
                .join("mod-import-1-0")
                .join("extra.bin"),
            b"x",
        )
        .expect("extra file");
        assert_eq!(
            migrator.verify_package(&source, &target, "mod-import-1-0", &NeverCancelled),
            Err(ModStorageMigrationError::VerifyMismatch)
        );
    }

    #[test]
    fn verify_fails_when_the_target_copy_is_missing() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        write_package(&source, "mod-import-1-0", FILES);
        fs::create_dir(&target).expect("target root");

        assert_eq!(
            FileSystemModStorageMigrator.verify_package(
                &source,
                &target,
                "mod-import-1-0",
                &NeverCancelled
            ),
            Err(ModStorageMigrationError::VerifyMismatch)
        );
    }

    #[test]
    fn remove_package_deletes_only_that_package_and_tolerates_absence() {
        let temp = tempfile::tempdir().expect("temp");
        let root = temp.path().join("mods");
        write_package(&root, "mod-import-1-0", FILES);
        write_package(&root, "mod-import-2-0", FILES);
        let migrator = FileSystemModStorageMigrator;

        migrator
            .remove_package(&root, "mod-import-1-0")
            .expect("remove");
        migrator
            .remove_package(&root, "mod-import-1-0")
            .expect("removing again is a no-op");
        migrator
            .remove_package(&temp.path().join("missing-root"), "mod-import-1-0")
            .expect("missing root is a no-op");

        assert!(!migrator
            .package_exists(&root, "mod-import-1-0")
            .expect("exists"));
        assert!(migrator
            .package_exists(&root, "mod-import-2-0")
            .expect("exists"));
    }

    #[test]
    fn unsafe_package_ids_are_rejected_before_any_io() {
        let temp = tempfile::tempdir().expect("temp");
        let migrator = FileSystemModStorageMigrator;
        for unsafe_id in ["../escape", "", "a/b", "with space"] {
            assert_eq!(
                migrator.remove_package(temp.path(), unsafe_id),
                Err(ModStorageMigrationError::PackageUnreadable),
                "{unsafe_id}"
            );
            assert_eq!(
                migrator.package_exists(temp.path(), unsafe_id),
                Err(ModStorageMigrationError::PackageUnreadable),
                "{unsafe_id}"
            );
        }
    }

    struct CancelAfter {
        remaining: AtomicUsize,
    }

    impl CancellationToken for CancelAfter {
        fn is_cancelled(&self) -> bool {
            let previous = self
                .remaining
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    Some(n.saturating_sub(1))
                });
            matches!(previous, Ok(0))
        }
    }

    #[test]
    fn copy_stops_at_a_cancellation_checkpoint_and_the_partial_copy_can_be_removed() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        write_package(&source, "mod-import-1-0", FILES);
        let migrator = FileSystemModStorageMigrator;

        let error = migrator
            .copy_package(
                &source,
                &target,
                "mod-import-1-0",
                &CancelAfter {
                    remaining: AtomicUsize::new(1),
                },
            )
            .expect_err("cancelled");

        assert_eq!(error, ModStorageMigrationError::Cancelled);
        let partial = tree(
            &target
                .join(MOD_STORAGE_SANDBOX_DIRECTORY)
                .join("mod-import-1-0"),
        );
        assert!(partial.len() < 3, "copy must stop early: {partial:?}");
        migrator
            .remove_package(&target, "mod-import-1-0")
            .expect("rollback removes the partial copy");
        assert!(!migrator
            .package_exists(&target, "mod-import-1-0")
            .expect("exists"));
    }

    #[cfg(windows)]
    fn create_directory_link(target: &Path, link: &Path) -> bool {
        std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link.to_str().expect("link path"),
                target.to_str().expect("target path"),
            ])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(unix)]
    fn create_directory_link(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn remove_directory_link(link: &Path) {
        let _ = fs::remove_dir(link);
    }

    #[cfg(unix)]
    fn remove_directory_link(link: &Path) {
        let _ = fs::remove_file(link);
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn copy_refuses_a_link_inside_the_package_and_leaves_the_link_target_alone() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&outside).expect("outside");
        fs::write(outside.join("secret.txt"), b"do not copy").expect("outside file");
        write_package(&source, "mod-import-1-0", FILES);
        let link = source
            .join(MOD_STORAGE_SANDBOX_DIRECTORY)
            .join("mod-import-1-0")
            .join("linked");
        if !create_directory_link(&outside, &link) {
            return;
        }

        let result = FileSystemModStorageMigrator.copy_package(
            &source,
            &target,
            "mod-import-1-0",
            &NeverCancelled,
        );

        remove_directory_link(&link);
        assert_eq!(result, Err(ModStorageMigrationError::PackageUnreadable));
        let copied = tree(
            &target
                .join(MOD_STORAGE_SANDBOX_DIRECTORY)
                .join("mod-import-1-0"),
        );
        assert!(
            !copied.keys().any(|key| key.contains("secret.txt")),
            "link target must never be copied: {copied:?}"
        );
    }

    #[test]
    fn journal_round_trips_and_clears() {
        let temp = tempfile::tempdir().expect("temp");
        let repository = JsonModStorageMigrationJournalRepository::new(
            temp.path().join("mod-import").join("migration.json"),
        );
        assert_eq!(repository.load().expect("load"), None);

        let journal = ModStorageMigrationJournal {
            version: MOD_STORAGE_MIGRATION_JOURNAL_VERSION,
            state: ModStorageMigrationState::Copying,
            source_root: temp.path().join("source"),
            target_root: temp.path().join("target"),
            packages: vec!["mod-import-1-0".to_owned()],
            started_at_unix_millis: 42,
        };
        repository.save(&journal).expect("save");
        assert_eq!(repository.load().expect("load"), Some(journal.clone()));

        let switched = ModStorageMigrationJournal {
            state: ModStorageMigrationState::Switched,
            ..journal
        };
        repository.save(&switched).expect("save switched");
        assert_eq!(repository.load().expect("load"), Some(switched));
        assert!(
            !fs::read_dir(temp.path().join("mod-import"))
                .expect("read")
                .any(|entry| entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".tmp")),
            "atomic write must not leave temp files"
        );

        repository.clear().expect("clear");
        assert_eq!(repository.load().expect("load"), None);
        repository.clear().expect("clearing twice is fine");
    }

    #[test]
    fn journal_rejects_unknown_versions_and_garbage() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("migration.json");
        let repository = JsonModStorageMigrationJournalRepository::new(path.clone());

        fs::write(&path, b"{not json").expect("garbage");
        assert_eq!(
            repository.load(),
            Err(ModStorageMigrationError::JournalUnavailable)
        );
        fs::write(
            &path,
            serde_json::json!({
                "version": 99,
                "state": "copying",
                "source_root": "a",
                "target_root": "b",
                "packages": [],
                "started_at_unix_millis": 1
            })
            .to_string(),
        )
        .expect("future version");
        assert_eq!(
            repository.load(),
            Err(ModStorageMigrationError::JournalUnavailable)
        );
    }
}
