use anyhow::{Context, Result};
use fs2::FileExt;
use hmm_ports::{
    CancellationToken, ModImportPackagePrepareRequest, ModImportPackagePreparer,
    ModImportResultRepository, ModImportSandboxLocator, ModPackageMetadata,
    ModPackageMetadataAnalyzer, PreparedModPackage, StoredModImportAnalysis,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MOD_IMPORT_RESULTS_SCHEMA_VERSION: u32 = 1;
const METADATA_MAX_BYTES: u64 = 64 * 1024;
const METADATA_MAX_SCAN_DEPTH: usize = 2;
const METADATA_MAX_DISPLAY_NAME_CHARS: usize = 80;
const DEFAULT_ZIP_MAX_ENTRIES: usize = 16 * 1024;
const DEFAULT_ZIP_MAX_SINGLE_FILE_BYTES: u64 = 1024 * 1024 * 1024;
const DEFAULT_ZIP_MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 4 * 1024 * 1024 * 1024;

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
    limits: ZipExtractionLimits,
}

pub struct TaskScopedModImportSandboxLocator {
    sandbox_root: PathBuf,
}

impl ZipModImportPackagePreparer {
    pub fn new(sandbox_root: PathBuf) -> Self {
        Self {
            sandbox_root,
            limits: ZipExtractionLimits::default(),
        }
    }
}

impl TaskScopedModImportSandboxLocator {
    pub fn new(sandbox_root: PathBuf) -> Self {
        Self { sandbox_root }
    }
}

impl ModImportSandboxLocator for TaskScopedModImportSandboxLocator {
    fn sandbox_root_for_package(&self, package_id: &str) -> Result<PathBuf> {
        validate_task_id_segment(package_id)?;
        Ok(self.sandbox_root.join(package_id))
    }
}

#[derive(Debug, Clone, Copy)]
struct ZipExtractionLimits {
    max_entries: usize,
    max_single_file_bytes: u64,
    max_total_uncompressed_bytes: u64,
}

impl Default for ZipExtractionLimits {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_ZIP_MAX_ENTRIES,
            max_single_file_bytes: DEFAULT_ZIP_MAX_SINGLE_FILE_BYTES,
            max_total_uncompressed_bytes: DEFAULT_ZIP_MAX_TOTAL_UNCOMPRESSED_BYTES,
        }
    }
}

impl ModImportPackagePreparer for ZipModImportPackagePreparer {
    fn prepare_package(
        &self,
        request: ModImportPackagePrepareRequest<'_>,
    ) -> Result<PreparedModPackage> {
        let task_id = request.task_id;
        let archive_path = request.archive_path;
        validate_task_id_segment(task_id)?;

        fs::create_dir_all(&self.sandbox_root)
            .context("failed to create mod import sandbox root")?;
        let sandbox_root = self.sandbox_root.join(task_id);
        fs::create_dir(&sandbox_root).context("failed to create task-scoped mod import sandbox")?;

        if let Err(error) = extract_zip_archive_with_limits(
            archive_path,
            &sandbox_root,
            request.cancellation_token,
            self.limits,
        ) {
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

        let mut metadata = ModPackageMetadata::default();

        for path in manifest_candidates {
            if let Some(manifest_metadata) = read_manifest_metadata(&path)? {
                merge_missing_metadata(&mut metadata, manifest_metadata);
            }
        }

        if metadata.display_name.is_none() {
            for path in readme_candidates {
                if let Some(display_name) = read_readme_display_name(&path)? {
                    metadata.display_name = Some(display_name);
                    break;
                }
            }
        }

        Ok(metadata)
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

fn read_manifest_metadata(path: &Path) -> Result<Option<ModPackageMetadata>> {
    let Ok(content) = fs::read_to_string(path) else {
        return Ok(None);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return Ok(None);
    };
    let Some(object) = value.as_object() else {
        return Ok(None);
    };

    let metadata = ModPackageMetadata {
        display_name: read_manifest_string(
            object,
            &["displayName", "display_name", "name", "title"],
        ),
        version: read_manifest_string(object, &["version", "modVersion", "mod_version"]),
        author: read_manifest_author(object, &["author", "authors", "createdBy", "created_by"]),
        category: read_manifest_string(object, &["category", "type"]),
        tags: read_manifest_string_list(object, &["tags", "tag"]),
        dependencies: read_manifest_string_list(object, &["dependencies", "depends", "requires"]),
    };

    if metadata_has_value(&metadata) {
        Ok(Some(metadata))
    } else {
        Ok(None)
    }
}

fn merge_missing_metadata(target: &mut ModPackageMetadata, source: ModPackageMetadata) {
    if target.display_name.is_none() {
        target.display_name = source.display_name;
    }
    if target.version.is_none() {
        target.version = source.version;
    }
    if target.author.is_none() {
        target.author = source.author;
    }
    if target.category.is_none() {
        target.category = source.category;
    }
    append_unique_metadata_values(&mut target.tags, source.tags);
    append_unique_metadata_values(&mut target.dependencies, source.dependencies);
}

fn append_unique_metadata_values(target: &mut Vec<String>, source: Vec<String>) {
    for value in source {
        if !target.iter().any(|existing| existing == &value) {
            target.push(value);
        }
    }
}

fn read_manifest_string(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter().find_map(|key| {
        object
            .get(*key)
            .and_then(|value| value.as_str())
            .and_then(sanitize_metadata_text)
    })
}

fn read_manifest_author(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter().find_map(|key| {
        let value = object.get(*key)?;
        if let Some(text) = value.as_str().and_then(sanitize_metadata_text) {
            return Some(text);
        }

        let authors = value.as_array()?;
        let authors = authors
            .iter()
            .filter_map(|value| value.as_str().and_then(sanitize_metadata_text))
            .collect::<Vec<_>>();

        if authors.is_empty() {
            None
        } else {
            Some(authors.join(", "))
        }
    })
}

fn read_manifest_string_list(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Vec<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(metadata_value_to_string_list))
        .unwrap_or_default()
}

fn metadata_value_to_string_list(value: &serde_json::Value) -> Option<Vec<String>> {
    if let Some(text) = value.as_str().and_then(sanitize_metadata_text) {
        return Some(vec![text]);
    }

    value.as_array().map(|values| {
        values
            .iter()
            .filter_map(|value| value.as_str().and_then(sanitize_metadata_text))
            .collect()
    })
}

fn metadata_has_value(metadata: &ModPackageMetadata) -> bool {
    metadata.display_name.is_some()
        || metadata.version.is_some()
        || metadata.author.is_some()
        || metadata.category.is_some()
        || !metadata.tags.is_empty()
        || !metadata.dependencies.is_empty()
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

        if let Some(display_name) = sanitize_metadata_text(heading) {
            return Ok(Some(display_name));
        }
    }

    Ok(None)
}

fn sanitize_metadata_text(value: &str) -> Option<String> {
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

fn extract_zip_archive_with_limits(
    archive_path: &Path,
    sandbox_root: &Path,
    cancellation_token: &dyn CancellationToken,
    limits: ZipExtractionLimits,
) -> Result<()> {
    let archive_file = fs::File::open(archive_path).context("failed to open archive")?;
    let mut archive = zip::ZipArchive::new(archive_file).context("failed to read zip archive")?;
    reject_too_many_archive_entries(archive.len(), limits.max_entries)?;
    let mut seen_paths = HashSet::new();
    let mut total_uncompressed_bytes = 0_u64;

    for index in 0..archive.len() {
        ensure_not_cancelled(cancellation_token)?;
        let mut entry = archive
            .by_index(index)
            .context("failed to read zip archive entry")?;
        reject_symlink_entry(&entry)?;
        reject_oversized_archive_entry(&entry, limits.max_single_file_bytes)?;

        let relative_path = safe_zip_entry_path(entry.name())?;
        reject_case_insensitive_collision(&mut seen_paths, &relative_path)?;
        let target_path = sandbox_root.join(&relative_path);

        if entry.is_dir() {
            fs::create_dir_all(&target_path).context("failed to create archive directory")?;
            continue;
        }

        total_uncompressed_bytes = total_uncompressed_bytes.saturating_add(entry.size());
        reject_oversized_archive_total(
            total_uncompressed_bytes,
            limits.max_total_uncompressed_bytes,
        )?;

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).context("failed to create archive parent directory")?;
        }

        let mut target_file =
            fs::File::create(&target_path).context("failed to create extracted file")?;
        copy_with_cancellation(&mut entry, &mut target_file, cancellation_token)
            .context("failed to extract archive file")?;
    }

    Ok(())
}

fn reject_too_many_archive_entries(actual_entries: usize, max_entries: usize) -> Result<()> {
    if actual_entries > max_entries {
        anyhow::bail!("unsafe archive: archive entry limit exceeded");
    }

    Ok(())
}

fn reject_oversized_archive_entry(
    entry: &zip::read::ZipFile<'_>,
    max_single_file_bytes: u64,
) -> Result<()> {
    if !entry.is_dir() && entry.size() > max_single_file_bytes {
        anyhow::bail!("unsafe archive: archive file size limit exceeded");
    }

    Ok(())
}

fn reject_oversized_archive_total(
    total_uncompressed_bytes: u64,
    max_total_bytes: u64,
) -> Result<()> {
    if total_uncompressed_bytes > max_total_bytes {
        anyhow::bail!("unsafe archive: archive total size limit exceeded");
    }

    Ok(())
}

fn copy_with_cancellation<R, W>(
    reader: &mut R,
    writer: &mut W,
    cancellation_token: &dyn CancellationToken,
) -> Result<()>
where
    R: Read,
    W: Write,
{
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        ensure_not_cancelled(cancellation_token)?;
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
    }

    Ok(())
}

fn ensure_not_cancelled(cancellation_token: &dyn CancellationToken) -> Result<()> {
    if cancellation_token.is_cancelled() {
        anyhow::bail!("mod import prepare cancelled");
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
        CancellationToken, ModImportPackagePrepareRequest, ModImportPackagePreparer,
        ModImportResultRepository, ModPackageMetadataAnalyzer, NeverCancelled,
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
        let prepared =
            prepare_package(&preparer, "task-1", &archive_path).expect("prepare package");

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
        let error =
            prepare_package(&preparer, "task-1", &archive_path).expect_err("unsafe entry rejected");

        assert!(error.to_string().contains("unsafe archive path"));
        assert!(!temp.path().join("escape.txt").exists());
    }

    #[test]
    fn rejects_zip_entries_that_are_absolute_paths() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("evil.zip");
        create_zip(&archive_path, &[("/absolute.txt", b"bad".as_slice())]);

        let preparer = ZipModImportPackagePreparer::new(temp.path().join("sandboxes"));
        let error =
            prepare_package(&preparer, "task-1", &archive_path).expect_err("unsafe entry rejected");

        assert!(error.to_string().contains("unsafe archive path"));
    }

    #[test]
    fn rejects_zip_entries_that_are_symlinks() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("evil.zip");
        create_zip_with_symlink(&archive_path, "link-to-outside", "../outside.txt");

        let preparer = ZipModImportPackagePreparer::new(temp.path().join("sandboxes"));
        let error = prepare_package(&preparer, "task-1", &archive_path)
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
        let error = prepare_package(&preparer, "task-1", &archive_path)
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
    fn loads_legacy_mod_import_results_without_metadata() {
        let path = test_file("legacy-results");
        fs::create_dir_all(path.parent().expect("results parent")).expect("create parent");
        fs::write(
            &path,
            r#"{
                "version": 1,
                "records": [{
                    "mod_id": "pkg-1",
                    "task_id": "task-1",
                    "package_id": "pkg-1",
                    "display_name": "pkg-1",
                    "preview_image": {
                        "kind": "fallback",
                        "reason": "missing"
                    }
                }]
            }"#,
        )
        .expect("write legacy results");
        let repo = JsonModImportResultRepository::new(path);

        let record = repo
            .get_analysis("pkg-1")
            .expect("read legacy analysis")
            .expect("legacy record exists");

        assert_eq!(record.metadata, Default::default());
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
        let error =
            prepare_package(&preparer, "task-1", &archive_path).expect_err("unsafe entry rejected");

        assert!(error.to_string().contains("unsafe archive path"));
        assert!(!sandbox_root.join("task-1").exists());
        assert!(!temp.path().join("escape.txt").exists());
    }

    #[test]
    fn cancels_zip_extraction_and_cleans_task_sandbox() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("cancel.zip");
        create_zip(
            &archive_path,
            &[
                ("one.txt", b"hello".as_slice()),
                ("two.txt", b"world".as_slice()),
            ],
        );

        let sandbox_root = temp.path().join("sandboxes");
        let preparer = ZipModImportPackagePreparer::new(sandbox_root.clone());
        let cancellation_token = AlwaysCancelled;
        let error = preparer
            .prepare_package(ModImportPackagePrepareRequest {
                task_id: "task-1",
                archive_path: &archive_path,
                cancellation_token: &cancellation_token,
            })
            .expect_err("cancelled extraction should stop");

        assert!(error.to_string().contains("cancelled"));
        assert!(!sandbox_root.join("task-1").exists());
    }

    #[test]
    fn sandbox_locator_resolves_package_inside_controlled_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("sandboxes");
        let locator = TaskScopedModImportSandboxLocator::new(sandbox_root.clone());

        let resolved = locator
            .sandbox_root_for_package("task-1")
            .expect("sandbox root resolves");

        assert_eq!(resolved, sandbox_root.join("task-1"));
    }

    #[test]
    fn sandbox_locator_rejects_unsafe_package_segments() {
        let temp = tempfile::tempdir().expect("temp dir");
        let locator = TaskScopedModImportSandboxLocator::new(temp.path().join("sandboxes"));

        let error = locator
            .sandbox_root_for_package("../escape")
            .expect_err("unsafe package id rejected");

        assert!(error.to_string().contains("unsafe task id segment"));
    }

    #[test]
    fn rejects_zip_archives_with_too_many_entries_and_cleans_task_sandbox() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("too-many.zip");
        create_numbered_zip_entries(&archive_path, 3);
        let sandbox_root = temp.path().join("sandboxes");
        let limits = ZipExtractionLimits {
            max_entries: 2,
            max_single_file_bytes: 1024,
            max_total_uncompressed_bytes: 4096,
        };
        let preparer = ZipModImportPackagePreparer {
            sandbox_root: sandbox_root.clone(),
            limits,
        };

        let error = prepare_package(&preparer, "task-1", &archive_path)
            .expect_err("entry limit should reject archive");

        assert!(error.to_string().contains("archive entry limit exceeded"));
        assert!(!sandbox_root.join("task-1").exists());
    }

    #[test]
    fn rejects_zip_entries_over_single_file_limit_and_cleans_task_sandbox() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("single-too-large.zip");
        create_zip(&archive_path, &[("large.bin", b"large".as_slice())]);
        let sandbox_root = temp.path().join("sandboxes");
        let limits = ZipExtractionLimits {
            max_entries: 10,
            max_single_file_bytes: 4,
            max_total_uncompressed_bytes: 4096,
        };
        let preparer = ZipModImportPackagePreparer {
            sandbox_root: sandbox_root.clone(),
            limits,
        };

        let error = prepare_package(&preparer, "task-1", &archive_path)
            .expect_err("single file limit should reject archive");

        assert!(error
            .to_string()
            .contains("archive file size limit exceeded"));
        assert!(!sandbox_root.join("task-1").exists());
    }

    #[test]
    fn rejects_zip_archives_over_total_uncompressed_limit_and_cleans_task_sandbox() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("total-too-large.zip");
        create_zip(
            &archive_path,
            &[
                ("first.bin", b"1234".as_slice()),
                ("second.bin", b"5678".as_slice()),
            ],
        );
        let sandbox_root = temp.path().join("sandboxes");
        let limits = ZipExtractionLimits {
            max_entries: 10,
            max_single_file_bytes: 1024,
            max_total_uncompressed_bytes: 7,
        };
        let preparer = ZipModImportPackagePreparer {
            sandbox_root: sandbox_root.clone(),
            limits,
        };

        let error = prepare_package(&preparer, "task-1", &archive_path)
            .expect_err("total size limit should reject archive");

        assert!(error
            .to_string()
            .contains("archive total size limit exceeded"));
        assert!(!sandbox_root.join("task-1").exists());
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
    fn metadata_analyzer_reads_schema_fields_from_manifest_json() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(
            temp.path().join("manifest.json"),
            r#"{
                "displayName": "Better Mod Name",
                "version": "1.2.3",
                "author": "A Hunter",
                "category": "Visual",
                "tags": ["armor", "hd"],
                "dependencies": ["stracker-loader"]
            }"#,
        )
        .expect("write manifest");

        let metadata = SandboxModPackageMetadataAnalyzer
            .analyze_metadata("pkg-1", temp.path())
            .expect("analyze metadata");

        assert_eq!(metadata.display_name.as_deref(), Some("Better Mod Name"));
        assert_eq!(metadata.version.as_deref(), Some("1.2.3"));
        assert_eq!(metadata.author.as_deref(), Some("A Hunter"));
        assert_eq!(metadata.category.as_deref(), Some("Visual"));
        assert_eq!(metadata.tags, vec!["armor", "hd"]);
        assert_eq!(metadata.dependencies, vec!["stracker-loader"]);
    }

    #[test]
    fn metadata_analyzer_merges_manifest_candidates_and_author_arrays() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(
            temp.path().join("metadata.json"),
            r#"{
                "version": "1.2.3",
                "authors": ["A Hunter", "Another Hunter"]
            }"#,
        )
        .expect("write metadata");
        fs::write(
            temp.path().join("manifest.json"),
            r#"{
                "displayName": "Better Mod Name",
                "category": "Visual"
            }"#,
        )
        .expect("write manifest");

        let metadata = SandboxModPackageMetadataAnalyzer
            .analyze_metadata("pkg-1", temp.path())
            .expect("analyze metadata");

        assert_eq!(metadata.display_name.as_deref(), Some("Better Mod Name"));
        assert_eq!(metadata.version.as_deref(), Some("1.2.3"));
        assert_eq!(metadata.author.as_deref(), Some("A Hunter, Another Hunter"));
        assert_eq!(metadata.category.as_deref(), Some("Visual"));
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

    fn create_numbered_zip_entries(path: &Path, count: usize) {
        let file = fs::File::create(path).expect("create zip file");
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();

        for index in 0..count {
            zip.start_file(format!("file-{index}.txt"), options)
                .expect("start zip file");
            zip.write_all(b"x").expect("write zip contents");
        }

        zip.finish().expect("finish zip");
    }

    fn prepare_package(
        preparer: &ZipModImportPackagePreparer,
        task_id: &str,
        archive_path: &Path,
    ) -> Result<PreparedModPackage> {
        let cancellation_token = NeverCancelled;
        preparer.prepare_package(ModImportPackagePrepareRequest {
            task_id,
            archive_path,
            cancellation_token: &cancellation_token,
        })
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
            metadata: Default::default(),
            preview_image: StoredImportPreviewImage::Fallback { reason },
        }
    }

    struct AlwaysCancelled;

    impl CancellationToken for AlwaysCancelled {
        fn is_cancelled(&self) -> bool {
            true
        }
    }
}
