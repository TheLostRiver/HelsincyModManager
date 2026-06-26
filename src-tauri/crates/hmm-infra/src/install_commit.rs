use anyhow::{Context, Result};
use hmm_core::{InstallManifest, InstallTargetPath, PackageFileId, ProfileId};
use hmm_ports::{
    InstallBackupStore, InstallGameFileSystem, InstallManifestRepository, InstallSourceFileReader,
};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct FileSystemInstallSourceFileReader {
    source_root: PathBuf,
}

impl FileSystemInstallSourceFileReader {
    pub fn new(source_root: PathBuf) -> Self {
        Self { source_root }
    }
}

impl InstallSourceFileReader for FileSystemInstallSourceFileReader {
    fn read_source_file(&self, package_file_id: &PackageFileId) -> Result<Vec<u8>> {
        let path = contained_path(&self.source_root, package_file_id.as_str())?;
        let metadata =
            fs::symlink_metadata(&path).context("failed to inspect install source file")?;

        if !metadata.is_file() || metadata.file_type().is_symlink() {
            anyhow::bail!("install source file is not a regular file");
        }

        fs::read(path).context("failed to read install source file")
    }
}

pub struct FileSystemInstallGameFileSystem {
    game_root: PathBuf,
}

impl FileSystemInstallGameFileSystem {
    pub fn new(game_root: PathBuf) -> Self {
        Self { game_root }
    }
}

impl InstallGameFileSystem for FileSystemInstallGameFileSystem {
    fn read_game_file(&self, target_path: &InstallTargetPath) -> Result<Option<Vec<u8>>> {
        let path = contained_path(&self.game_root, target_path.as_str())?;

        if !path.exists() {
            return Ok(None);
        }

        let metadata = fs::symlink_metadata(&path).context("failed to inspect install target")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            anyhow::bail!("install target is not a regular file");
        }
        ensure_contained_existing_path(&self.game_root, &path)?;

        Ok(Some(
            fs::read(path).context("failed to read install target")?,
        ))
    }

    fn write_game_file(&self, target_path: &InstallTargetPath, bytes: &[u8]) -> Result<()> {
        let path = contained_path(&self.game_root, target_path.as_str())?;
        let parent = path
            .parent()
            .context("install target path has no parent directory")?;
        ensure_nearest_existing_ancestor_contained(&self.game_root, parent)?;
        fs::create_dir_all(parent).context("failed to create install target parent")?;
        ensure_safe_write_target(&self.game_root, &path)?;

        atomic_write_file(&path, bytes).context("failed to write install target")?;

        Ok(())
    }

    fn remove_game_file(&self, target_path: &InstallTargetPath) -> Result<()> {
        let path = contained_path(&self.game_root, target_path.as_str())?;

        if !path.exists() {
            return Ok(());
        }

        let metadata = fs::symlink_metadata(&path).context("failed to inspect install target")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            anyhow::bail!("install target is not a regular file");
        }
        ensure_contained_existing_path(&self.game_root, &path)?;

        fs::remove_file(path).context("failed to remove install target")
    }
}

pub struct FileSystemInstallBackupStore {
    backup_root: PathBuf,
}

impl FileSystemInstallBackupStore {
    pub fn new(backup_root: PathBuf) -> Self {
        Self { backup_root }
    }
}

impl InstallBackupStore for FileSystemInstallBackupStore {
    fn store_backup(&self, target_path: &InstallTargetPath, bytes: &[u8]) -> Result<String> {
        fs::create_dir_all(&self.backup_root).context("failed to create install backup root")?;
        ensure_contained_existing_path(&self.backup_root, &self.backup_root)?;

        let base_name = format!("backup-{}", target_path.as_str().replace('/', "-"));
        let backup_ref = unique_backup_ref(&self.backup_root, &base_name);
        let backup_path = self.backup_root.join(&backup_ref);
        ensure_safe_write_target(&self.backup_root, &backup_path)?;
        atomic_write_file(&backup_path, bytes).context("failed to write install backup")?;

        Ok(backup_ref)
    }

    fn read_backup(&self, backup_ref: &str) -> Result<Option<Vec<u8>>> {
        let backup_path = contained_path(&self.backup_root, backup_ref)?;

        if !backup_path.exists() {
            return Ok(None);
        }

        let metadata =
            fs::symlink_metadata(&backup_path).context("failed to inspect install backup")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            anyhow::bail!("install backup is not a regular file");
        }
        ensure_contained_existing_path(&self.backup_root, &backup_path)?;

        Ok(Some(
            fs::read(backup_path).context("failed to read install backup")?,
        ))
    }

    fn remove_backup(&self, backup_ref: &str) -> Result<()> {
        let backup_path = contained_path(&self.backup_root, backup_ref)?;

        if !backup_path.exists() {
            return Ok(());
        }

        let metadata =
            fs::symlink_metadata(&backup_path).context("failed to inspect install backup")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            anyhow::bail!("install backup is not a regular file");
        }

        fs::remove_file(backup_path).context("failed to remove install backup")
    }
}

pub struct JsonInstallManifestRepository {
    manifest_root: PathBuf,
}

impl JsonInstallManifestRepository {
    pub fn new(manifest_root: PathBuf) -> Self {
        Self { manifest_root }
    }
}

impl InstallManifestRepository for JsonInstallManifestRepository {
    fn load_manifest(&self, profile_id: &ProfileId) -> Result<Option<InstallManifest>> {
        let file_name = manifest_file_name(profile_id.as_str())?;

        match fs::symlink_metadata(&self.manifest_root) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("failed to inspect install manifest root"),
        }

        ensure_existing_directory(&self.manifest_root, "install manifest root")?;
        ensure_contained_existing_path(&self.manifest_root, &self.manifest_root)?;
        let manifest_path = self.manifest_root.join(file_name);

        let metadata = match fs::symlink_metadata(&manifest_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("failed to inspect install manifest"),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            anyhow::bail!("install manifest is not a regular file");
        }
        ensure_contained_existing_path(&self.manifest_root, &manifest_path)?;

        let serialized =
            fs::read_to_string(&manifest_path).context("failed to read install manifest")?;
        let manifest: InstallManifest =
            serde_json::from_str(&serialized).context("failed to deserialize install manifest")?;
        if manifest.profile_id != *profile_id {
            anyhow::bail!("install manifest profile id does not match request");
        }

        Ok(Some(manifest))
    }

    fn save_manifest(&self, manifest: &InstallManifest) -> Result<()> {
        fs::create_dir_all(&self.manifest_root)
            .context("failed to create install manifest root")?;
        ensure_existing_directory(&self.manifest_root, "install manifest root")?;
        ensure_contained_existing_path(&self.manifest_root, &self.manifest_root)?;
        let file_name = manifest_file_name(manifest.profile_id.as_str())?;
        let manifest_path = self.manifest_root.join(file_name);
        ensure_safe_write_target(&self.manifest_root, &manifest_path)?;
        let serialized = serde_json::to_string_pretty(manifest)
            .context("failed to serialize install manifest")?;
        atomic_write_file(&manifest_path, serialized.as_bytes())
            .context("failed to write install manifest")?;

        Ok(())
    }
}

fn contained_path(root: &Path, logical_path: &str) -> Result<PathBuf> {
    let segments = safe_relative_segments(logical_path)?;
    let mut path = root.to_path_buf();
    for segment in segments {
        path.push(segment);
    }

    if path.exists() {
        ensure_contained_existing_path(root, &path)?;
    } else if let Some(parent) = path.parent().filter(|parent| parent.exists()) {
        ensure_contained_existing_path(root, parent)?;
    }

    Ok(path)
}

fn safe_relative_segments(value: &str) -> Result<Vec<String>> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || has_windows_drive_prefix(trimmed)
    {
        anyhow::bail!("install path is not a safe relative path");
    }

    let normalized = trimmed.replace('\\', "/");
    let mut segments = Vec::new();
    for segment in normalized.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            anyhow::bail!("install path is not a safe relative path");
        }
        segments.push(segment.to_owned());
    }

    Ok(segments)
}

fn ensure_contained_existing_path(root: &Path, path: &Path) -> Result<()> {
    let canonical_root = root
        .canonicalize()
        .context("failed to resolve install root")?;
    let canonical_path = path
        .canonicalize()
        .context("failed to resolve install path")?;

    if !canonical_path.starts_with(&canonical_root) {
        anyhow::bail!("install path escaped its root");
    }

    Ok(())
}

fn ensure_existing_directory(path: &Path, label: &str) -> Result<()> {
    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("failed to inspect {label}"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        anyhow::bail!("{label} is not a directory");
    }

    Ok(())
}

fn ensure_nearest_existing_ancestor_contained(root: &Path, path: &Path) -> Result<()> {
    let mut current = Some(path);

    while let Some(candidate) = current {
        if candidate.exists() {
            return ensure_contained_existing_path(root, candidate);
        }

        current = candidate.parent();
    }

    anyhow::bail!("install path has no existing ancestor")
}

fn ensure_safe_write_target(root: &Path, path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                anyhow::bail!("install write target is not a regular file");
            }
            ensure_contained_existing_path(root, path)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                ensure_nearest_existing_ancestor_contained(root, parent)?;
            }
        }
        Err(error) => return Err(error).context("failed to inspect install write target"),
    }

    Ok(())
}

fn atomic_write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .context("install write target has no parent directory")?;
    let temp_path = unique_temp_path(path);

    let result = (|| -> Result<()> {
        let mut temp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .context("failed to create install temp file")?;
        temp_file
            .write_all(bytes)
            .context("failed to write install temp file")?;
        temp_file
            .sync_all()
            .context("failed to sync install temp file")?;
        drop(temp_file);

        fs::rename(&temp_path, path).context("failed to rename install temp file")?;
        sync_directory(parent).context("failed to sync install parent directory")?;

        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    result
}

fn unique_temp_path(path: &Path) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temp_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("{name}.{}.{}.tmp", std::process::id(), nonce))
        .unwrap_or_else(|| format!("install.{}.{}.tmp", std::process::id(), nonce));

    path.parent()
        .map(|parent| parent.join(&temp_name))
        .unwrap_or_else(|| PathBuf::from(temp_name))
}

fn sync_directory(path: &Path) -> Result<()> {
    open_directory_for_sync(path)
        .and_then(|directory| directory.sync_all())
        .context("failed to sync directory")
}

#[cfg(not(windows))]
fn open_directory_for_sync(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

#[cfg(windows)]
fn open_directory_for_sync(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;

    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

fn has_windows_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn unique_backup_ref(root: &Path, base_name: &str) -> String {
    if !root.join(base_name).exists() {
        return base_name.to_owned();
    }

    for index in 1.. {
        let candidate = format!("{base_name}-{index}");
        if !root.join(&candidate).exists() {
            return candidate;
        }
    }

    unreachable!("unbounded backup ref search should eventually find a free name")
}

fn manifest_file_name(profile_id: &str) -> Result<String> {
    if profile_id.is_empty()
        || !profile_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        anyhow::bail!("profile id is not safe for manifest storage");
    }

    Ok(format!("{profile_id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        FileLayer, InstallManifest, InstallManifestEntry, InstallTargetPath, ModId, PackageFileId,
        ProfileId,
    };
    use hmm_ports::{
        InstallBackupStore, InstallGameFileSystem, InstallManifestRepository,
        InstallSourceFileReader,
    };
    use std::fs;

    #[test]
    fn filesystem_install_adapters_read_write_backup_and_manifest_inside_roots() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_root = temp.path().join("source-package");
        let game_root = temp.path().join("game");
        let backup_root = temp.path().join("backups");
        let manifest_root = temp.path().join("manifests");
        fs::create_dir_all(source_root.join("nativePC/models")).expect("create source dirs");
        fs::create_dir_all(game_root.join("nativePC/models")).expect("create game dirs");
        fs::write(
            source_root.join("nativePC/models/player.mod3"),
            b"new model",
        )
        .expect("write source");
        fs::write(game_root.join("nativePC/models/player.mod3"), b"old model")
            .expect("write old target");
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("valid target");
        let source_reader = FileSystemInstallSourceFileReader::new(source_root);
        let game_files = FileSystemInstallGameFileSystem::new(game_root.clone());
        let backup_store = FileSystemInstallBackupStore::new(backup_root.clone());
        let manifest_repository = JsonInstallManifestRepository::new(manifest_root.clone());

        let source_bytes = source_reader
            .read_source_file(&PackageFileId::new("nativePC/models/player.mod3"))
            .expect("read source file");
        let old_bytes = game_files
            .read_game_file(&target)
            .expect("read game file")
            .expect("existing game file");
        let backup_ref = backup_store
            .store_backup(&target, &old_bytes)
            .expect("store backup");
        game_files
            .write_game_file(&target, &source_bytes)
            .expect("write target");
        let manifest = InstallManifest {
            profile_id: ProfileId::new("default"),
            entries: vec![InstallManifestEntry {
                target_path: target,
                mod_id: ModId::new("mod-a"),
                package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: Some(backup_ref.clone()),
                installed_file: None,
            }],
        };
        manifest_repository
            .save_manifest(&manifest)
            .expect("save manifest");

        assert_eq!(
            fs::read(game_root.join("nativePC/models/player.mod3")).expect("target"),
            b"new model"
        );
        assert_eq!(
            fs::read(backup_root.join(&backup_ref)).expect("backup"),
            b"old model"
        );
        backup_store
            .remove_backup(&backup_ref)
            .expect("remove backup");
        assert!(!backup_root.join(backup_ref).exists());
        let manifest = fs::read_to_string(manifest_root.join("default.json")).expect("manifest");
        assert!(manifest.contains("\"profile_id\"") || manifest.contains("\"profileId\""));
        assert!(manifest.contains("nativePC/models/player.mod3"));
        assert!(!manifest.contains(temp.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn filesystem_backup_reader_reads_backup_inside_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let backup_root = temp.path().join("backups");
        let backup_store = FileSystemInstallBackupStore::new(backup_root);
        let target =
            InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"]).expect("target");
        let backup_ref = backup_store
            .store_backup(&target, b"original model")
            .expect("store backup");

        let bytes = backup_store
            .read_backup(&backup_ref)
            .expect("read backup")
            .expect("backup should exist");

        assert_eq!(bytes, b"original model");
    }

    #[test]
    fn filesystem_backup_reader_returns_none_for_missing_backup() {
        let temp = tempfile::tempdir().expect("temp dir");
        let backup_store = FileSystemInstallBackupStore::new(temp.path().join("backups"));

        let bytes = backup_store
            .read_backup("missing-backup")
            .expect("missing backup should not fail");

        assert_eq!(bytes, None);
    }

    #[test]
    fn filesystem_backup_reader_rejects_parent_traversal_without_path_details() {
        let temp = tempfile::tempdir().expect("temp dir");
        let backup_store = FileSystemInstallBackupStore::new(temp.path().join("backups"));

        let error = backup_store
            .read_backup("../outside-backup")
            .expect_err("backup reader must reject traversal");

        assert!(!error
            .to_string()
            .contains(temp.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn filesystem_source_reader_rejects_package_file_parent_traversal() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_root = temp.path().join("source-package");
        let outside = temp.path().join("outside.bin");
        fs::create_dir_all(&source_root).expect("create source root");
        fs::write(&outside, b"outside").expect("write outside");

        let error = FileSystemInstallSourceFileReader::new(source_root)
            .read_source_file(&PackageFileId::new("../outside.bin"))
            .expect_err("source reader must reject traversal");

        assert!(!error
            .to_string()
            .contains(temp.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn filesystem_game_writer_rejects_existing_symlink_target() {
        let temp = tempfile::tempdir().expect("temp dir");
        let game_root = temp.path().join("game");
        fs::create_dir_all(game_root.join("nativePC/models")).expect("create game dirs");
        fs::write(game_root.join("nativePC/models/real.mod3"), b"real").expect("write real target");
        if !try_create_file_symlink(
            game_root.join("nativePC/models/real.mod3"),
            game_root.join("nativePC/models/link.mod3"),
        ) {
            return;
        }
        let target =
            InstallTargetPath::parse("nativePC/models/link.mod3", ["nativePC"]).expect("target");

        let error = FileSystemInstallGameFileSystem::new(game_root)
            .write_game_file(&target, b"new")
            .expect_err("writer must reject existing symlink target");

        assert!(error
            .to_string()
            .contains("install write target is not a regular file"));
    }

    #[test]
    fn filesystem_game_writer_rejects_broken_symlink_target_without_following_it() {
        let temp = tempfile::tempdir().expect("temp dir");
        let game_root = temp.path().join("game");
        let outside_target = temp.path().join("outside-created.mod3");
        fs::create_dir_all(game_root.join("nativePC/models")).expect("create game dirs");
        if !try_create_file_symlink(&outside_target, game_root.join("nativePC/models/link.mod3")) {
            return;
        }
        let target =
            InstallTargetPath::parse("nativePC/models/link.mod3", ["nativePC"]).expect("target");

        let error = FileSystemInstallGameFileSystem::new(game_root)
            .write_game_file(&target, b"new")
            .expect_err("writer must reject broken symlink target");

        assert!(error
            .to_string()
            .contains("install write target is not a regular file"));
        assert!(!outside_target.exists());
    }

    #[test]
    fn filesystem_game_writer_rejects_symlink_ancestor_before_creating_directories() {
        let temp = tempfile::tempdir().expect("temp dir");
        let game_root = temp.path().join("game");
        let outside_root = temp.path().join("outside");
        let outside_created_dir = outside_root.join("new-dir");
        fs::create_dir_all(game_root.join("nativePC")).expect("create game dirs");
        fs::create_dir_all(&outside_root).expect("create outside root");
        if !try_create_dir_symlink(&outside_root, game_root.join("nativePC/link")) {
            return;
        }
        let target = InstallTargetPath::parse("nativePC/link/new-dir/file.mod3", ["nativePC"])
            .expect("target");

        let error = FileSystemInstallGameFileSystem::new(game_root)
            .write_game_file(&target, b"new")
            .expect_err("writer must reject symlink ancestor");

        assert!(error.to_string().contains("install path escaped its root"));
        assert!(!outside_created_dir.exists());
    }

    #[test]
    fn filesystem_game_reader_rejects_symlink_ancestor() {
        let temp = tempfile::tempdir().expect("temp dir");
        let game_root = temp.path().join("game");
        let outside_root = temp.path().join("outside");
        let outside_file = outside_root.join("player.mod3");
        fs::create_dir_all(game_root.join("nativePC")).expect("create game dirs");
        fs::create_dir_all(&outside_root).expect("create outside root");
        fs::write(&outside_file, b"outside").expect("write outside");
        if !try_create_dir_symlink(&outside_root, game_root.join("nativePC/link")) {
            return;
        }
        let target =
            InstallTargetPath::parse("nativePC/link/player.mod3", ["nativePC"]).expect("target");

        let error = FileSystemInstallGameFileSystem::new(game_root)
            .read_game_file(&target)
            .expect_err("reader must reject symlink ancestor");

        assert!(error.to_string().contains("install path escaped its root"));
    }

    #[test]
    fn filesystem_game_remover_rejects_symlink_ancestor_without_deleting_outside_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let game_root = temp.path().join("game");
        let outside_root = temp.path().join("outside");
        let outside_file = outside_root.join("player.mod3");
        fs::create_dir_all(game_root.join("nativePC")).expect("create game dirs");
        fs::create_dir_all(&outside_root).expect("create outside root");
        fs::write(&outside_file, b"outside").expect("write outside");
        if !try_create_dir_symlink(&outside_root, game_root.join("nativePC/link")) {
            return;
        }
        let target =
            InstallTargetPath::parse("nativePC/link/player.mod3", ["nativePC"]).expect("target");

        let error = FileSystemInstallGameFileSystem::new(game_root)
            .remove_game_file(&target)
            .expect_err("remover must reject symlink ancestor");

        assert!(error.to_string().contains("install path escaped its root"));
        assert_eq!(fs::read(outside_file).expect("outside file"), b"outside");
    }

    #[test]
    fn json_manifest_repository_replaces_manifest_without_temp_artifacts() {
        let temp = tempfile::tempdir().expect("temp dir");
        let manifest_root = temp.path().join("manifests");
        let repository = JsonInstallManifestRepository::new(manifest_root.clone());
        let target =
            InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"]).expect("target");
        let first_manifest = InstallManifest {
            profile_id: ProfileId::new("default"),
            entries: vec![InstallManifestEntry {
                target_path: target.clone(),
                mod_id: ModId::new("mod-a"),
                package_file_id: PackageFileId::new("nativePC/models/old.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: None,
            }],
        };
        let second_manifest = InstallManifest {
            profile_id: ProfileId::new("default"),
            entries: vec![InstallManifestEntry {
                target_path: target,
                mod_id: ModId::new("mod-a"),
                package_file_id: PackageFileId::new("nativePC/models/new.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: None,
            }],
        };

        repository
            .save_manifest(&first_manifest)
            .expect("save first manifest");
        repository
            .save_manifest(&second_manifest)
            .expect("replace manifest");

        let manifest = fs::read_to_string(manifest_root.join("default.json")).expect("manifest");
        assert!(manifest.contains("nativePC/models/new.mod3"));
        assert!(!manifest.contains("nativePC/models/old.mod3"));
        let temp_artifacts = fs::read_dir(&manifest_root)
            .expect("read manifest root")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(temp_artifacts, 0);
    }

    #[test]
    fn json_manifest_repository_load_returns_none_when_manifest_is_missing() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = JsonInstallManifestRepository::new(temp.path().join("manifests"));

        let manifest = repository
            .load_manifest(&ProfileId::new("default"))
            .expect("missing manifest should not fail");

        assert_eq!(manifest, None);
    }

    #[test]
    fn json_manifest_repository_load_round_trips_saved_manifest() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = JsonInstallManifestRepository::new(temp.path().join("manifests"));
        let manifest = InstallManifest {
            profile_id: ProfileId::new("default"),
            entries: vec![InstallManifestEntry {
                target_path: InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
                    .expect("target"),
                mod_id: ModId::new("mod-a"),
                package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: Some("backup-player".to_owned()),
                installed_file: None,
            }],
        };

        repository.save_manifest(&manifest).expect("save manifest");
        let loaded = repository
            .load_manifest(&ProfileId::new("default"))
            .expect("load manifest");

        assert_eq!(loaded, Some(manifest));
    }

    #[test]
    fn json_manifest_repository_load_rejects_file_manifest_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let manifest_root = temp.path().join("manifests");
        fs::write(&manifest_root, b"not a directory").expect("write manifest root fixture");
        let repository = JsonInstallManifestRepository::new(manifest_root);

        let error = repository
            .load_manifest(&ProfileId::new("default"))
            .expect_err("manifest root file must be rejected");

        assert!(error
            .to_string()
            .contains("install manifest root is not a directory"));
    }

    #[test]
    fn json_manifest_repository_load_rejects_symlink_manifest_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let outside_root = temp.path().join("outside-manifests");
        fs::create_dir_all(&outside_root).expect("create outside root");
        let manifest_root = temp.path().join("manifests");
        if !try_create_dir_symlink(&outside_root, &manifest_root) {
            return;
        }
        let repository = JsonInstallManifestRepository::new(manifest_root);

        let error = repository
            .load_manifest(&ProfileId::new("default"))
            .expect_err("manifest root symlink must be rejected");

        assert!(error
            .to_string()
            .contains("install manifest root is not a directory"));
    }

    #[test]
    fn json_manifest_repository_load_rejects_broken_symlink_manifest_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let manifest_root = temp.path().join("manifests");
        if !try_create_dir_symlink(temp.path().join("missing-manifests"), &manifest_root) {
            return;
        }
        let repository = JsonInstallManifestRepository::new(manifest_root);

        let error = repository
            .load_manifest(&ProfileId::new("default"))
            .expect_err("broken manifest root symlink must be rejected");

        assert!(error
            .to_string()
            .contains("install manifest root is not a directory"));
    }

    #[test]
    fn json_manifest_repository_load_rejects_unsafe_profile_id_without_path_details() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = JsonInstallManifestRepository::new(temp.path().join("manifests"));

        let error = repository
            .load_manifest(&ProfileId::new("../outside"))
            .expect_err("unsafe profile id must be rejected");

        assert!(!error
            .to_string()
            .contains(temp.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn json_manifest_repository_load_rejects_broken_symlink_manifest() {
        let temp = tempfile::tempdir().expect("temp dir");
        let manifest_root = temp.path().join("manifests");
        fs::create_dir_all(&manifest_root).expect("create manifest root");
        if !try_create_file_symlink(
            temp.path().join("missing-manifest.json"),
            manifest_root.join("default.json"),
        ) {
            return;
        }
        let repository = JsonInstallManifestRepository::new(manifest_root);

        let error = repository
            .load_manifest(&ProfileId::new("default"))
            .expect_err("manifest symlink must be rejected");

        assert!(error
            .to_string()
            .contains("install manifest is not a regular file"));
    }

    #[test]
    fn json_manifest_repository_load_rejects_profile_id_mismatch() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = JsonInstallManifestRepository::new(temp.path().join("manifests"));
        let manifest = InstallManifest {
            profile_id: ProfileId::new("other"),
            entries: Vec::new(),
        };

        repository.save_manifest(&manifest).expect("save manifest");
        fs::rename(
            temp.path().join("manifests").join("other.json"),
            temp.path().join("manifests").join("default.json"),
        )
        .expect("rename manifest fixture");
        let error = repository
            .load_manifest(&ProfileId::new("default"))
            .expect_err("profile mismatch must be rejected");

        assert!(error
            .to_string()
            .contains("install manifest profile id does not match request"));
    }

    #[cfg(unix)]
    fn try_create_file_symlink(original: impl AsRef<Path>, link: impl AsRef<Path>) -> bool {
        std::os::unix::fs::symlink(original, link).is_ok()
    }

    #[cfg(windows)]
    fn try_create_file_symlink(original: impl AsRef<Path>, link: impl AsRef<Path>) -> bool {
        std::os::windows::fs::symlink_file(original, link).is_ok()
    }

    #[cfg(unix)]
    fn try_create_dir_symlink(original: impl AsRef<Path>, link: impl AsRef<Path>) -> bool {
        std::os::unix::fs::symlink(original, link).is_ok()
    }

    #[cfg(windows)]
    fn try_create_dir_symlink(original: impl AsRef<Path>, link: impl AsRef<Path>) -> bool {
        std::os::windows::fs::symlink_dir(original, link).is_ok()
    }
}
