use anyhow::{Context, Result};
use fs2::FileExt;
use hmm_ports::{
    ModImportPackagePreparer, ModImportResultRepository, ModPackageMetadata,
    ModPackageMetadataAnalyzer, PreparedModPackage, StoredModImportAnalysis,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MOD_IMPORT_RESULTS_SCHEMA_VERSION: u32 = 1;
const METADATA_MAX_BYTES: u64 = 64 * 1024;
const METADATA_MAX_SCAN_DEPTH: usize = 2;
const METADATA_MAX_DISPLAY_NAME_CHARS: usize = 80;

#[derive(Debug, Serialize, Deserialize)]
struct ModImportResultsFile {
    version: u32,
    records: Vec<StoredModImportAnalysis>,
}

impl Default for ModImportResultsFile {
    fn default() -> Self {
        Self {
            version: MOD_IMPORT_RESULTS_SCHEMA_VERSION,
            records: Vec::new(),
        }
    }
}

pub struct JsonModImportResultRepository {
    file_path: PathBuf,
    write_lock: Mutex<()>,
}

impl JsonModImportResultRepository {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            write_lock: Mutex::new(()),
        }
    }

    fn load_file(&self) -> Result<ModImportResultsFile> {
        if !self.file_path.exists() {
            return Ok(ModImportResultsFile::default());
        }

        let bytes = fs::read(&self.file_path).context("failed to read mod import result store")?;
        let content = String::from_utf8(bytes).context("mod import result store is corrupted")?;
        let file: ModImportResultsFile =
            serde_json::from_str(&content).context("mod import result store is corrupted")?;

        if file.version != MOD_IMPORT_RESULTS_SCHEMA_VERSION {
            anyhow::bail!("mod import result store is corrupted");
        }

        Ok(file)
    }

    fn save_file(&self, file: &ModImportResultsFile) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).context("failed to create mod import result directory")?;
        }

        let serialized =
            serde_json::to_string_pretty(file).context("failed to serialize mod import results")?;
        let temp_path = self.unique_temp_path();

        {
            let mut temp_file =
                File::create(&temp_path).context("failed to create mod import result temp file")?;
            temp_file
                .write_all(serialized.as_bytes())
                .context("failed to write mod import result temp file")?;
            temp_file
                .sync_all()
                .context("failed to sync mod import result temp file")?;
        }

        fs::rename(&temp_path, &self.file_path).context("failed to replace mod import results")?;
        self.sync_parent_directory()?;

        Ok(())
    }

    fn sync_parent_directory(&self) -> Result<()> {
        let Some(parent) = self.file_path.parent() else {
            return Ok(());
        };

        open_directory_for_sync(parent)
            .and_then(|directory| directory.sync_all())
            .context("failed to sync mod import result directory")?;

        Ok(())
    }

    fn lock_file_path(&self) -> PathBuf {
        let lock_name = self
            .file_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("{name}.lock"))
            .unwrap_or_else(|| "mod-import-results.json.lock".to_owned());

        self.file_path
            .parent()
            .map(|parent| parent.join(&lock_name))
            .unwrap_or_else(|| PathBuf::from(lock_name))
    }

    fn unique_temp_path(&self) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let temp_name = self
            .file_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("{name}.{}.{}.tmp", std::process::id(), nonce))
            .unwrap_or_else(|| {
                format!(
                    "mod-import-results.{}.{}.json.tmp",
                    std::process::id(),
                    nonce
                )
            });

        self.file_path
            .parent()
            .map(|parent| parent.join(&temp_name))
            .unwrap_or_else(|| PathBuf::from(temp_name))
    }

    fn open_lock_file(&self) -> Result<File> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).context("failed to create mod import result directory")?;
        }

        OpenOptions::new()
            .create(true)
            .read(true)
            .truncate(false)
            .write(true)
            .open(self.lock_file_path())
            .context("failed to open mod import result lock")
    }
}

#[cfg(windows)]
fn open_directory_for_sync(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;

    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

#[cfg(not(windows))]
fn open_directory_for_sync(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

impl ModImportResultRepository for JsonModImportResultRepository {
    fn save_analysis(&self, analysis: &StoredModImportAnalysis) -> Result<()> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("mod import result write lock poisoned"))?;
        let lock_file = self.open_lock_file()?;
        lock_file
            .lock_exclusive()
            .context("failed to lock mod import results")?;

        let mut file = self.load_file()?;
        file.records
            .retain(|record| record.mod_id != analysis.mod_id);
        file.records.push(analysis.clone());
        let result = self.save_file(&file);
        let unlock_result = lock_file
            .unlock()
            .context("failed to unlock mod import results");

        result.and(unlock_result)
    }

    fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>> {
        Ok(self.load_file()?.records)
    }

    fn get_analysis(&self, mod_id: &str) -> Result<Option<StoredModImportAnalysis>> {
        Ok(self
            .load_file()?
            .records
            .into_iter()
            .find(|record| record.mod_id == mod_id))
    }
}

pub struct ZipModImportPackagePreparer {
    sandbox_root: PathBuf,
}

impl ZipModImportPackagePreparer {
    pub fn new(sandbox_root: PathBuf) -> Self {
        Self { sandbox_root }
    }
}

impl ModImportPackagePreparer for ZipModImportPackagePreparer {
    fn prepare_package(&self, task_id: &str, archive_path: &Path) -> Result<PreparedModPackage> {
        validate_task_id_segment(task_id)?;

        fs::create_dir_all(&self.sandbox_root)
            .context("failed to create mod import sandbox root")?;
        let sandbox_root = self.sandbox_root.join(task_id);
        fs::create_dir(&sandbox_root).context("failed to create task-scoped mod import sandbox")?;

        if let Err(error) = extract_zip_archive(archive_path, &sandbox_root) {
            let _ = fs::remove_dir_all(&sandbox_root);
            return Err(error);
        }

        Ok(PreparedModPackage {
            package_id: task_id.to_owned(),
            sandbox_root,
        })
    }
}

pub struct SandboxModPackageMetadataAnalyzer;

impl ModPackageMetadataAnalyzer for SandboxModPackageMetadataAnalyzer {
    fn analyze_metadata(
        &self,
        _package_id: &str,
        sandbox_root: &Path,
    ) -> Result<ModPackageMetadata> {
        let mut manifest_candidates = Vec::new();
        let mut readme_candidates = Vec::new();
        collect_metadata_candidates(
            sandbox_root,
            0,
            &mut manifest_candidates,
            &mut readme_candidates,
        )?;

        for path in manifest_candidates {
            if let Some(display_name) = read_manifest_display_name(&path)? {
                return Ok(ModPackageMetadata {
                    display_name: Some(display_name),
                });
            }
        }

        for path in readme_candidates {
            if let Some(display_name) = read_readme_display_name(&path)? {
                return Ok(ModPackageMetadata {
                    display_name: Some(display_name),
                });
            }
        }

        Ok(ModPackageMetadata { display_name: None })
    }
}

fn collect_metadata_candidates(
    directory: &Path,
    depth: usize,
    manifest_candidates: &mut Vec<PathBuf>,
    readme_candidates: &mut Vec<PathBuf>,
) -> Result<()> {
    if depth >= METADATA_MAX_SCAN_DEPTH {
        return Ok(());
    }

    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }

        if metadata.is_dir() {
            collect_metadata_candidates(&path, depth + 1, manifest_candidates, readme_candidates)?;
            continue;
        }

        if !metadata.is_file() || metadata.len() > METADATA_MAX_BYTES {
            continue;
        }

        let Some(file_name) = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_ascii_lowercase())
        else {
            continue;
        };

        if is_manifest_file_name(&file_name) {
            manifest_candidates.push(path);
        } else if is_readme_file_name(&file_name) {
            readme_candidates.push(path);
        }
    }

    Ok(())
}

fn is_manifest_file_name(file_name: &str) -> bool {
    matches!(
        file_name,
        "manifest.json" | "mod.json" | "metadata.json" | "info.json"
    )
}

fn is_readme_file_name(file_name: &str) -> bool {
    matches!(file_name, "readme" | "readme.md" | "readme.txt")
}

fn read_manifest_display_name(path: &Path) -> Result<Option<String>> {
    let Ok(content) = fs::read_to_string(path) else {
        return Ok(None);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return Ok(None);
    };
    let Some(object) = value.as_object() else {
        return Ok(None);
    };

    for key in ["displayName", "display_name", "name", "title"] {
        if let Some(display_name) = object
            .get(key)
            .and_then(|value| value.as_str())
            .and_then(sanitize_display_name)
        {
            return Ok(Some(display_name));
        }
    }

    Ok(None)
}

fn read_readme_display_name(path: &Path) -> Result<Option<String>> {
    let content = fs::read_to_string(path).context("failed to read mod readme")?;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let heading = trimmed
            .strip_prefix('#')
            .map(|value| value.trim_start_matches('#').trim())
            .unwrap_or(trimmed);

        if let Some(display_name) = sanitize_display_name(heading) {
            return Ok(Some(display_name));
        }
    }

    Ok(None)
}

fn sanitize_display_name(value: &str) -> Option<String> {
    let normalized = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|character| !character.is_control())
        .take(METADATA_MAX_DISPLAY_NAME_CHARS)
        .collect::<String>();
    let normalized = normalized.trim();

    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_owned())
    }
}

fn validate_task_id_segment(task_id: &str) -> Result<()> {
    if task_id.is_empty()
        || !task_id
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || value == '-' || value == '_')
    {
        anyhow::bail!("unsafe task id segment");
    }

    Ok(())
}

fn extract_zip_archive(archive_path: &Path, sandbox_root: &Path) -> Result<()> {
    let archive_file = fs::File::open(archive_path).context("failed to open archive")?;
    let mut archive = zip::ZipArchive::new(archive_file).context("failed to read zip archive")?;
    let mut seen_paths = HashSet::new();

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .context("failed to read zip archive entry")?;
        reject_symlink_entry(&entry)?;

        let relative_path = safe_zip_entry_path(entry.name())?;
        reject_case_insensitive_collision(&mut seen_paths, &relative_path)?;
        let target_path = sandbox_root.join(&relative_path);

        if entry.is_dir() {
            fs::create_dir_all(&target_path).context("failed to create archive directory")?;
            continue;
        }

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).context("failed to create archive parent directory")?;
        }

        let mut target_file =
            fs::File::create(&target_path).context("failed to create extracted file")?;
        io::copy(&mut entry, &mut target_file).context("failed to extract archive file")?;
    }

    Ok(())
}

fn reject_symlink_entry(entry: &zip::read::ZipFile<'_>) -> Result<()> {
    if entry.is_symlink() {
        anyhow::bail!("unsafe archive path: symlink entries are not allowed");
    }

    Ok(())
}

fn reject_case_insensitive_collision(
    seen_paths: &mut HashSet<String>,
    relative_path: &Path,
) -> Result<()> {
    let key = case_insensitive_path_key(relative_path);

    if !seen_paths.insert(key) {
        anyhow::bail!("unsafe archive path: case-insensitive path collision");
    }

    Ok(())
}

fn case_insensitive_path_key(relative_path: &Path) -> String {
    relative_path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_ascii_lowercase()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn safe_zip_entry_path(entry_name: &str) -> Result<PathBuf> {
    let path = Path::new(entry_name);
    let mut safe = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(value) => safe.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("unsafe archive path: {entry_name}");
            }
        }
    }

    if safe.as_os_str().is_empty() {
        anyhow::bail!("unsafe archive path: {entry_name}");
    }

    Ok(safe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::PreviewImageRejectionReason;
    use hmm_ports::{
        ModImportPackagePreparer, ModImportResultRepository, ModPackageMetadataAnalyzer,
        StoredImportPreviewImage, StoredModImportAnalysis,
    };
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    #[test]
    fn prepares_zip_package_inside_task_scoped_sandbox() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("sample.zip");
        create_zip(
            &archive_path,
            &[("nativePC/readme.txt", b"hello".as_slice())],
        );

        let preparer = ZipModImportPackagePreparer::new(temp.path().join("sandboxes"));
        let prepared = preparer
            .prepare_package("task-1", &archive_path)
            .expect("prepare package");

        assert_eq!(prepared.package_id, "task-1");
        assert!(prepared
            .sandbox_root
            .starts_with(temp.path().join("sandboxes")));
        assert_eq!(
            fs::read_to_string(prepared.sandbox_root.join("nativePC/readme.txt"))
                .expect("read extracted file"),
            "hello"
        );
    }

    #[test]
    fn rejects_zip_entries_that_escape_with_parent_segments() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("evil.zip");
        create_zip(&archive_path, &[("../escape.txt", b"bad".as_slice())]);

        let preparer = ZipModImportPackagePreparer::new(temp.path().join("sandboxes"));
        let error = preparer
            .prepare_package("task-1", &archive_path)
            .expect_err("unsafe entry rejected");

        assert!(error.to_string().contains("unsafe archive path"));
        assert!(!temp.path().join("escape.txt").exists());
    }

    #[test]
    fn rejects_zip_entries_that_are_absolute_paths() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("evil.zip");
        create_zip(&archive_path, &[("/absolute.txt", b"bad".as_slice())]);

        let preparer = ZipModImportPackagePreparer::new(temp.path().join("sandboxes"));
        let error = preparer
            .prepare_package("task-1", &archive_path)
            .expect_err("unsafe entry rejected");

        assert!(error.to_string().contains("unsafe archive path"));
    }

    #[test]
    fn rejects_zip_entries_that_are_symlinks() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("evil.zip");
        create_zip_with_symlink(&archive_path, "link-to-outside", "../outside.txt");

        let preparer = ZipModImportPackagePreparer::new(temp.path().join("sandboxes"));
        let error = preparer
            .prepare_package("task-1", &archive_path)
            .expect_err("symlink entry rejected");

        assert!(error.to_string().contains("symlink"));
    }

    #[test]
    fn rejects_case_insensitive_path_collisions() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("collision.zip");
        create_zip(
            &archive_path,
            &[
                ("Preview.PNG", b"first".as_slice()),
                ("preview.png", b"second".as_slice()),
            ],
        );

        let preparer = ZipModImportPackagePreparer::new(temp.path().join("sandboxes"));
        let error = preparer
            .prepare_package("task-1", &archive_path)
            .expect_err("case collision rejected");

        assert!(error
            .to_string()
            .contains("case-insensitive path collision"));
    }

    #[test]
    fn missing_mod_import_result_file_loads_empty_library() {
        let repo = JsonModImportResultRepository::new(test_file("missing-results"));

        let records = repo.list_analysis().expect("list analysis");

        assert!(records.is_empty());
    }

    #[test]
    fn saves_and_loads_mod_import_analysis_results() {
        let repo = JsonModImportResultRepository::new(test_file("save-results"));

        repo.save_analysis(&stored_analysis(
            "pkg-1",
            PreviewImageRejectionReason::Missing,
        ))
        .expect("save analysis");

        let records = repo.list_analysis().expect("list analysis");
        let loaded = repo
            .get_analysis("pkg-1")
            .expect("get analysis")
            .expect("record exists");

        assert_eq!(records.len(), 1);
        assert_eq!(loaded.mod_id, "pkg-1");
        assert_eq!(
            loaded.preview_image,
            StoredImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::Missing,
            }
        );
    }

    #[test]
    fn saving_same_mod_import_result_replaces_existing_record() {
        let repo = JsonModImportResultRepository::new(test_file("replace-results"));

        repo.save_analysis(&stored_analysis(
            "pkg-1",
            PreviewImageRejectionReason::Missing,
        ))
        .expect("first save");
        repo.save_analysis(&stored_analysis(
            "pkg-1",
            PreviewImageRejectionReason::DecodeFailed,
        ))
        .expect("second save");

        let records = repo.list_analysis().expect("list analysis");
        let loaded = repo
            .get_analysis("pkg-1")
            .expect("get analysis")
            .expect("record exists");

        assert_eq!(records.len(), 1);
        assert_eq!(
            loaded.preview_image,
            StoredImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::DecodeFailed,
            }
        );
    }

    #[test]
    fn cleans_task_sandbox_when_extraction_fails() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("partial.zip");
        create_zip(
            &archive_path,
            &[
                ("ok/readme.txt", b"hello".as_slice()),
                ("../escape.txt", b"bad".as_slice()),
            ],
        );

        let sandbox_root = temp.path().join("sandboxes");
        let preparer = ZipModImportPackagePreparer::new(sandbox_root.clone());
        let error = preparer
            .prepare_package("task-1", &archive_path)
            .expect_err("unsafe entry rejected");

        assert!(error.to_string().contains("unsafe archive path"));
        assert!(!sandbox_root.join("task-1").exists());
        assert!(!temp.path().join("escape.txt").exists());
    }

    #[test]
    fn metadata_analyzer_reads_display_name_from_manifest_json() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(
            temp.path().join("manifest.json"),
            r#"{"displayName":"Better Mod Name"}"#,
        )
        .expect("write manifest");

        let metadata = SandboxModPackageMetadataAnalyzer
            .analyze_metadata("pkg-1", temp.path())
            .expect("analyze metadata");

        assert_eq!(metadata.display_name.as_deref(), Some("Better Mod Name"));
    }

    #[test]
    fn metadata_analyzer_reads_display_name_from_readme_heading() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(
            temp.path().join("README.md"),
            "# Better Readme Name\n\nInstall notes",
        )
        .expect("write readme");

        let metadata = SandboxModPackageMetadataAnalyzer
            .analyze_metadata("pkg-1", temp.path())
            .expect("analyze metadata");

        assert_eq!(metadata.display_name.as_deref(), Some("Better Readme Name"));
    }

    #[test]
    fn metadata_analyzer_falls_back_to_readme_when_manifest_is_invalid() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(temp.path().join("manifest.json"), "{not json").expect("write manifest");
        fs::write(temp.path().join("README.md"), "# Readme Name").expect("write readme");

        let metadata = SandboxModPackageMetadataAnalyzer
            .analyze_metadata("pkg-1", temp.path())
            .expect("analyze metadata");

        assert_eq!(metadata.display_name.as_deref(), Some("Readme Name"));
    }

    fn create_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).expect("create zip file");
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();

        for (name, contents) in entries {
            zip.start_file(name, options).expect("start zip file");
            zip.write_all(contents).expect("write zip contents");
        }

        zip.finish().expect("finish zip");
    }

    fn create_zip_with_symlink(path: &Path, name: &str, target: &str) {
        let file = fs::File::create(path).expect("create zip file");
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();

        zip.add_symlink_from_path(PathBuf::from(name), PathBuf::from(target), options)
            .expect("add symlink");
        zip.finish().expect("finish zip");
    }

    fn test_file(name: &str) -> PathBuf {
        let unique = format!(
            "hmm-mod-import-results-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        std::env::temp_dir()
            .join(unique)
            .join("mod-import")
            .join("results.json")
    }

    fn stored_analysis(
        mod_id: &str,
        reason: PreviewImageRejectionReason,
    ) -> StoredModImportAnalysis {
        StoredModImportAnalysis {
            mod_id: mod_id.to_owned(),
            task_id: "task-1".to_owned(),
            package_id: mod_id.to_owned(),
            display_name: mod_id.to_owned(),
            preview_image: StoredImportPreviewImage::Fallback { reason },
        }
    }
}
