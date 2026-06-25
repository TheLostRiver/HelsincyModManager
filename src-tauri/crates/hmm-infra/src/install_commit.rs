use anyhow::{Context, Result};
use hmm_core::{InstallManifest, InstallTargetPath, PackageFileId};
use hmm_ports::{
    InstallBackupStore, InstallGameFileSystem, InstallManifestRepository, InstallSourceFileReader,
};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

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

        Ok(Some(
            fs::read(path).context("failed to read install target")?,
        ))
    }

    fn write_game_file(&self, target_path: &InstallTargetPath, bytes: &[u8]) -> Result<()> {
        let path = contained_path(&self.game_root, target_path.as_str())?;
        let parent = path
            .parent()
            .context("install target path has no parent directory")?;
        fs::create_dir_all(parent).context("failed to create install target parent")?;
        ensure_contained_existing_path(&self.game_root, parent)?;
        ensure_safe_write_target(&self.game_root, &path)?;

        let mut file = File::create(&path).context("failed to open install target for writing")?;
        file.write_all(bytes)
            .context("failed to write install target")?;
        file.sync_all().context("failed to sync install target")?;

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
        let mut file = File::create(&backup_path).context("failed to create install backup")?;
        file.write_all(bytes)
            .context("failed to write install backup")?;
        file.sync_all().context("failed to sync install backup")?;

        Ok(backup_ref)
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
    fn save_manifest(&self, manifest: &InstallManifest) -> Result<()> {
        fs::create_dir_all(&self.manifest_root)
            .context("failed to create install manifest root")?;
        ensure_contained_existing_path(&self.manifest_root, &self.manifest_root)?;
        let file_name = manifest_file_name(manifest.profile_id.as_str())?;
        let manifest_path = self.manifest_root.join(file_name);
        ensure_safe_write_target(&self.manifest_root, &manifest_path)?;
        let serialized = serde_json::to_string_pretty(manifest)
            .context("failed to serialize install manifest")?;
        let mut file = File::create(manifest_path).context("failed to create install manifest")?;
        file.write_all(serialized.as_bytes())
            .context("failed to write install manifest")?;
        file.sync_all().context("failed to sync install manifest")?;

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

fn ensure_safe_write_target(root: &Path, path: &Path) -> Result<()> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path).context("failed to inspect install write target")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            anyhow::bail!("install write target is not a regular file");
        }
        ensure_contained_existing_path(root, path)?;
    } else if let Some(parent) = path.parent().filter(|parent| parent.exists()) {
        ensure_contained_existing_path(root, parent)?;
    }

    Ok(())
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
            fs::read(backup_root.join(backup_ref)).expect("backup"),
            b"old model"
        );
        let manifest = fs::read_to_string(manifest_root.join("default.json")).expect("manifest");
        assert!(manifest.contains("\"profile_id\"") || manifest.contains("\"profileId\""));
        assert!(manifest.contains("nativePC/models/player.mod3"));
        assert!(!manifest.contains(temp.path().to_string_lossy().as_ref()));
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

    #[cfg(unix)]
    fn try_create_file_symlink(
        original: impl AsRef<Path>,
        link: impl AsRef<Path>,
    ) -> bool {
        std::os::unix::fs::symlink(original, link).is_ok()
    }

    #[cfg(windows)]
    fn try_create_file_symlink(
        original: impl AsRef<Path>,
        link: impl AsRef<Path>,
    ) -> bool {
        std::os::windows::fs::symlink_file(original, link).is_ok()
    }
}
