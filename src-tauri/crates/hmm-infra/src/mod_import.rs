use crate::controlled_fs::{
    create_new_regular_file, open_child_directory_nofollow, open_existing_directory_chain,
    open_existing_directory_nofollow, open_or_create_child_directory,
    open_or_create_directory_chain, open_or_create_directory_nofollow, open_regular_file_nofollow,
    remove_child_tree_nofollow,
};
use anyhow::{Context, Result};
use cap_std::fs::Dir;
use hmm_ports::{
    CancellationToken, DiagnosticPackageExportRequest, DiagnosticPackageExportResult,
    DiagnosticPackageExporter, ModImportPackagePrepareReaderRequest,
    ModImportPackagePrepareRequest, ModImportPackagePreparer, ModImportSandboxLocator,
    ModPackageMetadata, ModPackageMetadataAnalyzer, PreparedModPackage,
};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

const METADATA_MAX_BYTES: u64 = 64 * 1024;
const METADATA_MAX_SCAN_DEPTH: usize = 2;
const METADATA_MAX_DISPLAY_NAME_CHARS: usize = 80;
const DEFAULT_ZIP_MAX_ENTRIES: usize = 16 * 1024;
const DEFAULT_ZIP_MAX_SINGLE_FILE_BYTES: u64 = 1024 * 1024 * 1024;
const DEFAULT_ZIP_MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const DIAGNOSTIC_PACKAGE_DIR: &str = "diagnostics";
const MOD_IMPORT_APP_DATA_DIRECTORY: &str = "mod-import";
const MOD_IMPORT_SANDBOX_DIRECTORY: &str = "sandboxes";

#[cfg(windows)]
fn open_directory_for_sync(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

#[cfg(not(windows))]
fn open_directory_for_sync(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

pub struct ZipModImportPackagePreparer {
    sandbox_root: PathBuf,
    app_data_root: Option<PathBuf>,
    limits: ZipExtractionLimits,
}

pub struct FileSystemDiagnosticPackageExporter {
    app_data_root: PathBuf,
}

pub struct TaskScopedModImportSandboxLocator {
    sandbox_root: PathBuf,
    app_data_root: Option<PathBuf>,
}

impl ZipModImportPackagePreparer {
    pub fn new(sandbox_root: PathBuf) -> Self {
        Self {
            sandbox_root,
            app_data_root: None,
            limits: ZipExtractionLimits::default(),
        }
    }

    /// Uses an app-data capability root and fixed child names for production sandboxes.
    pub fn new_in_app_data(app_data_root: PathBuf) -> Self {
        Self {
            sandbox_root: mod_import_sandbox_root_path(&app_data_root),
            app_data_root: Some(app_data_root),
            limits: ZipExtractionLimits::default(),
        }
    }

    fn open_sandbox_root(&self) -> Result<Dir> {
        match self.app_data_root.as_deref() {
            Some(app_data_root) => open_managed_sandbox_root(app_data_root, true),
            None => {
                open_or_create_directory_nofollow(&self.sandbox_root, "mod import sandbox root")
            }
        }
    }
}

impl FileSystemDiagnosticPackageExporter {
    pub fn new(app_data_root: PathBuf) -> Self {
        Self { app_data_root }
    }

    fn export_dir(&self) -> PathBuf {
        self.app_data_root.join("logs").join(DIAGNOSTIC_PACKAGE_DIR)
    }
}

impl TaskScopedModImportSandboxLocator {
    pub fn new(sandbox_root: PathBuf) -> Self {
        Self {
            sandbox_root,
            app_data_root: None,
        }
    }

    /// Keeps cleanup rooted below a verified app-data directory in production composition.
    pub fn new_in_app_data(app_data_root: PathBuf) -> Self {
        Self {
            sandbox_root: mod_import_sandbox_root_path(&app_data_root),
            app_data_root: Some(app_data_root),
        }
    }

    fn open_existing_sandbox_root(&self) -> Result<Dir> {
        match self.app_data_root.as_deref() {
            Some(app_data_root) => open_managed_sandbox_root(app_data_root, false),
            None => open_existing_directory_nofollow(&self.sandbox_root, "mod import sandbox root"),
        }
    }
}

impl ModImportSandboxLocator for TaskScopedModImportSandboxLocator {
    fn sandbox_root_for_package(&self, package_id: &str) -> Result<PathBuf> {
        validate_task_id_segment(package_id)?;
        Ok(self.sandbox_root.join(package_id))
    }

    fn cleanup_sandbox_for_package(&self, package_id: &str) -> Result<()> {
        validate_task_id_segment(package_id)?;
        let root = match self.open_existing_sandbox_root() {
            Ok(root) => root,
            Err(_error) if sandbox_root_is_missing(&self.sandbox_root) => return Ok(()),
            Err(error) => return Err(error),
        };
        remove_child_tree_nofollow(
            &root,
            std::ffi::OsStr::new(package_id),
            "mod import sandbox",
        )
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
        let mut archive = open_archive_file_nofollow(request.archive_path)?;
        self.prepare_package_from_reader(ModImportPackagePrepareReaderRequest {
            task_id: request.task_id,
            archive: &mut archive,
            cancellation_token: request.cancellation_token,
        })
    }

    fn prepare_package_from_reader(
        &self,
        request: ModImportPackagePrepareReaderRequest<'_>,
    ) -> Result<PreparedModPackage> {
        validate_task_id_segment(request.task_id)?;
        let root = self.open_sandbox_root()?;
        match root.create_dir(request.task_id) {
            Ok(()) => {}
            Err(error) => {
                return Err(error).context("failed to create task-scoped mod import sandbox");
            }
        }
        let sandbox = match open_child_directory_nofollow(
            &root,
            std::ffi::OsStr::new(request.task_id),
            "task-scoped mod import sandbox",
        ) {
            Ok(sandbox) => sandbox,
            Err(error) => {
                let _ = remove_child_tree_nofollow(
                    &root,
                    std::ffi::OsStr::new(request.task_id),
                    "task-scoped mod import sandbox",
                );
                return Err(error);
            }
        };

        if let Err(error) = extract_zip_archive_with_limits(
            request.archive,
            &sandbox,
            request.cancellation_token,
            self.limits,
        ) {
            drop(sandbox);
            let _ = remove_child_tree_nofollow(
                &root,
                std::ffi::OsStr::new(request.task_id),
                "task-scoped mod import sandbox",
            );
            return Err(error);
        }

        Ok(PreparedModPackage {
            package_id: request.task_id.to_owned(),
            sandbox_root: self.sandbox_root.join(request.task_id),
        })
    }
}

impl DiagnosticPackageExporter for FileSystemDiagnosticPackageExporter {
    fn export_package(
        &self,
        request: DiagnosticPackageExportRequest<'_>,
    ) -> Result<DiagnosticPackageExportResult> {
        validate_diagnostic_package_file_name(request.file_name)?;
        if request.entries.is_empty() {
            anyhow::bail!("diagnostic package must contain at least one entry");
        }

        for entry in request.entries {
            validate_diagnostic_package_entry_name(entry.name)?;
        }

        let export_dir = self.export_dir();
        fs::create_dir_all(&export_dir).context("failed to create diagnostic export directory")?;
        let export_path = export_dir.join(request.file_name);
        let file = File::create(&export_path).context("failed to create diagnostic package")?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();

        for entry in request.entries {
            zip.start_file(entry.name, options)
                .context("failed to write diagnostic package entry")?;
            zip.write_all(entry.bytes)
                .context("failed to write diagnostic package entry")?;
        }

        let file = zip
            .finish()
            .context("failed to finish diagnostic package")?;
        file.sync_all()
            .context("failed to sync diagnostic package")?;
        open_directory_for_sync(&export_dir)
            .and_then(|directory| directory.sync_all())
            .context("failed to sync diagnostic export directory")?;
        let size_bytes = fs::metadata(&export_path)
            .context("failed to inspect diagnostic package")?
            .len();

        Ok(DiagnosticPackageExportResult {
            export_id: request.file_name.to_owned(),
            file_name: request.file_name.to_owned(),
            size_bytes,
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

fn sandbox_root_is_missing(root: &Path) -> bool {
    matches!(fs::symlink_metadata(root), Err(error) if error.kind() == io::ErrorKind::NotFound)
}

fn mod_import_sandbox_root_path(app_data_root: &Path) -> PathBuf {
    app_data_root
        .join(MOD_IMPORT_APP_DATA_DIRECTORY)
        .join(MOD_IMPORT_SANDBOX_DIRECTORY)
}

fn open_managed_sandbox_root(app_data_root: &Path, create: bool) -> Result<Dir> {
    let app_data = if create {
        open_or_create_directory_nofollow(app_data_root, "mod import app data root")?
    } else {
        open_existing_directory_nofollow(app_data_root, "mod import app data root")?
    };
    if create {
        open_or_create_directory_chain(
            &app_data,
            &[MOD_IMPORT_APP_DATA_DIRECTORY, MOD_IMPORT_SANDBOX_DIRECTORY],
            "mod import sandbox root",
        )
    } else {
        open_existing_directory_chain(
            &app_data,
            &[MOD_IMPORT_APP_DATA_DIRECTORY, MOD_IMPORT_SANDBOX_DIRECTORY],
            "mod import sandbox root",
        )
    }
}

fn open_archive_file_nofollow(path: &Path) -> Result<File> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .context("Mod import archive must have a parent directory")?;
    let file_name = path
        .file_name()
        .context("Mod import archive must have a final path component")?;
    let parent = open_existing_directory_nofollow(parent, "Mod import archive parent directory")?;
    Ok(open_regular_file_nofollow(&parent, file_name, "Mod import archive")?.into_std())
}

fn validate_diagnostic_package_file_name(file_name: &str) -> Result<()> {
    validate_diagnostic_name_segment(file_name, "diagnostic package file name")?;
    if !file_name.ends_with(".zip") {
        anyhow::bail!("diagnostic package file name must end with .zip");
    }

    Ok(())
}

fn validate_diagnostic_package_entry_name(entry_name: &str) -> Result<()> {
    validate_diagnostic_name_segment(entry_name, "diagnostic package entry name")
}

fn validate_diagnostic_name_segment(value: &str, label: &str) -> Result<()> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
        || Path::new(value)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        anyhow::bail!("{label} is unsafe");
    }

    Ok(())
}

fn extract_zip_archive_with_limits<R>(
    archive_file: &mut R,
    sandbox_root: &Dir,
    cancellation_token: &dyn CancellationToken,
    limits: ZipExtractionLimits,
) -> Result<()>
where
    R: Read + Seek + ?Sized,
{
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
        if entry.is_dir() {
            let _ = open_or_create_archive_directory(sandbox_root, &relative_path)?;
            continue;
        }

        total_uncompressed_bytes = total_uncompressed_bytes.saturating_add(entry.size());
        reject_oversized_archive_total(
            total_uncompressed_bytes,
            limits.max_total_uncompressed_bytes,
        )?;

        let parent = relative_path.parent().unwrap_or_else(|| Path::new(""));
        let parent = open_or_create_archive_directory(sandbox_root, parent)?;
        let file_name = relative_path
            .file_name()
            .context("unsafe archive path: missing file name")?;
        let mut target_file =
            create_new_regular_file(&parent, file_name, "extracted archive file")?;
        copy_with_cancellation(&mut entry, &mut target_file, cancellation_token)
            .context("failed to extract archive file")?;
    }

    Ok(())
}

fn open_or_create_archive_directory(root: &Dir, relative_path: &Path) -> Result<Dir> {
    let mut current = root
        .try_clone()
        .context("failed to clone archive sandbox directory handle")?;
    for component in relative_path.components() {
        let Component::Normal(name) = component else {
            anyhow::bail!("unsafe archive path component");
        };
        current = open_or_create_child_directory(&current, name, "archive sandbox directory")?;
    }
    Ok(current)
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
    use hmm_ports::{
        CancellationToken, ModImportPackagePrepareRequest, ModImportPackagePreparer,
        ModPackageMetadataAnalyzer, NeverCancelled,
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
    fn sandbox_locator_cleans_only_a_valid_task_scoped_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("sandboxes");
        let package_root = sandbox_root.join("task-1");
        fs::create_dir_all(package_root.join("nested")).expect("create package sandbox");
        fs::write(package_root.join("nested").join("fixture.bin"), b"fixture")
            .expect("write fixture");
        let sibling = sandbox_root.join("task-2");
        fs::create_dir_all(&sibling).expect("create sibling sandbox");

        let locator = TaskScopedModImportSandboxLocator::new(sandbox_root);
        locator
            .cleanup_sandbox_for_package("task-1")
            .expect("cleanup task sandbox");

        assert!(!package_root.exists());
        assert!(sibling.exists());
    }

    #[test]
    fn sandbox_locator_rejects_a_linked_task_scope_without_touching_outside_sentinel() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("sandboxes");
        fs::create_dir_all(&sandbox_root).expect("create sandbox root");
        let outside = tempfile::tempdir().expect("outside root");
        let sentinel = outside.path().join("sentinel.txt");
        fs::write(&sentinel, b"outside remains untouched").expect("write outside sentinel");
        let linked_scope = sandbox_root.join("task-1");
        if !try_create_directory_link(outside.path(), &linked_scope) {
            return;
        }

        let locator = TaskScopedModImportSandboxLocator::new(sandbox_root);
        let error = locator
            .cleanup_sandbox_for_package("task-1")
            .expect_err("linked task scope must be rejected");

        assert!(error.to_string().contains("mod import sandbox"));
        assert_eq!(
            fs::read(&sentinel).expect("read outside sentinel"),
            b"outside remains untouched"
        );
        assert!(
            fs::symlink_metadata(&linked_scope).is_ok(),
            "cleanup must not remove a rejected linked scope"
        );
        remove_directory_link(&linked_scope);
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
            app_data_root: None,
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
            app_data_root: None,
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
            app_data_root: None,
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
    fn diagnostic_package_exporter_writes_zip_inside_app_data_without_returning_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let exporter = FileSystemDiagnosticPackageExporter::new(temp.path().to_path_buf());
        let payload = br#"{"totalImportedMods":1,"thumbnailCount":0}"#;

        let result = exporter
            .export_package(hmm_ports::DiagnosticPackageExportRequest {
                file_name: "preview-image-diagnostics-42.zip",
                entries: &[hmm_ports::DiagnosticPackageEntry {
                    name: "preview-image-diagnostics.json",
                    bytes: payload,
                }],
            })
            .expect("export package");

        assert_eq!(result.export_id, "preview-image-diagnostics-42.zip");
        assert_eq!(result.file_name, "preview-image-diagnostics-42.zip");
        assert!(result.size_bytes > 0);
        assert!(!result
            .file_name
            .contains(temp.path().to_string_lossy().as_ref()));

        let export_path = temp
            .path()
            .join("logs")
            .join("diagnostics")
            .join("preview-image-diagnostics-42.zip");
        assert!(export_path.exists());
        let file = fs::File::open(export_path).expect("open exported zip");
        let mut archive = zip::ZipArchive::new(file).expect("read exported zip");
        assert_eq!(archive.len(), 1);
        let mut entry = archive
            .by_name("preview-image-diagnostics.json")
            .expect("diagnostic json entry");
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents).expect("read json entry");
        assert_eq!(contents, payload);
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

    #[cfg(unix)]
    fn try_create_directory_link(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn try_create_directory_link(target: &Path, link: &Path) -> bool {
        std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link.to_str().expect("link path"),
                target.to_str().expect("target path"),
            ])
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(unix)]
    fn remove_directory_link(link: &Path) {
        fs::remove_file(link).expect("remove directory symlink");
    }

    #[cfg(windows)]
    fn remove_directory_link(link: &Path) {
        fs::remove_dir(link).expect("remove directory junction");
    }

    struct AlwaysCancelled;

    impl CancellationToken for AlwaysCancelled {
        fn is_cancelled(&self) -> bool {
            true
        }
    }
}
