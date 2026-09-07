use crate::archive_extraction::{
    extract_archive, pump_reader_into_sink, ArchiveChunkSink, ArchiveEntryHeader, ArchiveEntryKind,
    ArchiveExtractionLimits, ArchiveGate, ArchiveSource,
};
use crate::controlled_fs::{
    create_new_regular_file, open_child_directory_nofollow, open_existing_directory_chain,
    open_existing_directory_nofollow, open_or_create_child_directory,
    open_or_create_directory_chain, open_or_create_directory_nofollow, open_regular_file_nofollow,
    remove_child_tree_nofollow,
};
use crate::rar_archive_source;
use crate::sevenz_archive_source;
use anyhow::{Context, Result};
use cap_std::fs::Dir;
use hmm_core::sanitize_mod_metadata_text;
use hmm_ports::{
    CancellationToken, DiagnosticPackageExportRequest, DiagnosticPackageExportResult,
    DiagnosticPackageExporter, ModImportPackagePrepareReaderRequest,
    ModImportPackagePrepareRequest, ModImportPackagePreparer, ModImportPrepareError,
    ModImportSandboxLocator, ModPackageMetadata, ModPackageMetadataAnalysis,
    ModPackageMetadataAnalyzer, NonArchiveFile, PreparedModPackage, UnsupportedArchiveFormat,
};
use std::fs::{self, File};
use std::io::{self, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

const METADATA_MAX_BYTES: u64 = 64 * 1024;
const METADATA_MAX_SCAN_DEPTH: usize = 2;
// 必须与 hmm-core 的 DEFAULT_EXTERNAL_IMPORT_MATERIALIZATION_MAX_* 保持一致:
// 同一个 Mod 不能走第三方迁移能进、打包成 zip 手动导入反而被拒。
// 2026-08-27 依真实库实测放宽,依据见设计文档 Slice 1 定稿。
const DEFAULT_ZIP_MAX_ENTRIES: usize = 64 * 1024;
const DEFAULT_ZIP_MAX_SINGLE_FILE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const DEFAULT_ZIP_MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const DIAGNOSTIC_PACKAGE_DIR: &str = "diagnostics";
const MOD_IMPORT_APP_DATA_DIRECTORY: &str = hmm_ports::DEFAULT_MOD_STORAGE_DIRECTORY;
const MOD_IMPORT_SANDBOX_DIRECTORY: &str = hmm_ports::MOD_STORAGE_SANDBOX_DIRECTORY;

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
    storage_root: Option<PathBuf>,
    limits: ArchiveExtractionLimits,
}

pub struct FileSystemDiagnosticPackageExporter {
    app_data_root: PathBuf,
}

pub struct TaskScopedModImportSandboxLocator {
    sandbox_root: PathBuf,
    storage_root: Option<PathBuf>,
}

impl ZipModImportPackagePreparer {
    pub fn new(sandbox_root: PathBuf) -> Self {
        Self {
            sandbox_root,
            storage_root: None,
            limits: default_extraction_limits(),
        }
    }

    /// Production composition: `storage_root` is the resolved Mod storage root (default
    /// `<app-data>/mod-import`, or the user-configured directory, #275); sandboxes live in its
    /// fixed `sandboxes/` child and every component is opened no-follow.
    pub fn new_in_storage_root(storage_root: PathBuf) -> Self {
        Self {
            sandbox_root: mod_import_sandbox_root_path(&storage_root),
            storage_root: Some(storage_root),
            limits: default_extraction_limits(),
        }
    }

    fn open_sandbox_root(&self) -> Result<Dir> {
        match self.storage_root.as_deref() {
            Some(storage_root) => open_managed_sandbox_root(storage_root, true),
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
            storage_root: None,
        }
    }

    /// Keeps cleanup rooted below the resolved Mod storage root in production composition
    /// (see `ZipModImportPackagePreparer::new_in_storage_root`).
    pub fn new_in_storage_root(storage_root: PathBuf) -> Self {
        Self {
            sandbox_root: mod_import_sandbox_root_path(&storage_root),
            storage_root: Some(storage_root),
        }
    }

    /// The `sandboxes/` directory this locator resolves packages under.
    pub fn sandbox_root(&self) -> &Path {
        &self.sandbox_root
    }

    fn open_existing_sandbox_root(&self) -> Result<Dir> {
        match self.storage_root.as_deref() {
            Some(storage_root) => open_managed_sandbox_root(storage_root, false),
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

/// 默认限额。类型来自外壳——两处各定义一份迟早漂移。
fn default_extraction_limits() -> ArchiveExtractionLimits {
    ArchiveExtractionLimits {
        max_entries: DEFAULT_ZIP_MAX_ENTRIES,
        max_single_file_bytes: DEFAULT_ZIP_MAX_SINGLE_FILE_BYTES,
        max_total_uncompressed_bytes: DEFAULT_ZIP_MAX_TOTAL_UNCOMPRESSED_BYTES,
    }
}

impl ModImportPackagePreparer for ZipModImportPackagePreparer {
    fn prepare_package(
        &self,
        request: ModImportPackagePrepareRequest<'_>,
    ) -> std::result::Result<PreparedModPackage, ModImportPrepareError> {
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
    ) -> std::result::Result<PreparedModPackage, ModImportPrepareError> {
        validate_task_id_segment(request.task_id)?;
        let root = self.open_sandbox_root()?;
        match root.create_dir(request.task_id) {
            Ok(()) => {}
            Err(error) => {
                return Err(anyhow::Error::new(error)
                    .context("failed to create task-scoped mod import sandbox")
                    .into());
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
                return Err(error.into());
            }
        };

        // rar 需要一个真实路径（unrar 的 DLL API 只接受路径），所以要有地方落一份。
        // 暂存目录开在**包沙箱之外**——沙箱里的内容会被原样提交成 Mod 版本，
        // 把原始压缩包丢进去就是往玩家的包里塞垃圾。
        let scratch = ArchiveScratch::new(&self.sandbox_root, request.task_id);
        let extraction = extract_archive_with_limits(
            request.archive,
            &root,
            &scratch,
            &sandbox,
            request.cancellation_token,
            self.limits,
        );
        // 无论成败都要清掉暂存——它不该活过这次调用。
        scratch.cleanup(&root);

        if let Err(error) = extraction {
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
    ) -> Result<ModPackageMetadataAnalysis> {
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

        // manifest 声明的展示名要在 readme 回填之前截获：metadata.display_name
        // 保持"manifest ?? readme 首行"的既有语义，供上层做继承判定；
        // manifest_display_name 只承载 manifest 显式声明，供上层把压缩包
        // 文件名插到 readme 之前。
        let manifest_display_name = metadata.display_name.clone();

        if metadata.display_name.is_none() {
            for path in readme_candidates {
                if let Some(display_name) = read_readme_display_name(&path)? {
                    metadata.display_name = Some(display_name);
                    break;
                }
            }
        }

        Ok(ModPackageMetadataAnalysis {
            metadata,
            manifest_display_name,
        })
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
            .and_then(sanitize_mod_metadata_text)
    })
}

fn read_manifest_author(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter().find_map(|key| {
        let value = object.get(*key)?;
        if let Some(text) = value.as_str().and_then(sanitize_mod_metadata_text) {
            return Some(text);
        }

        let authors = value.as_array()?;
        let authors = authors
            .iter()
            .filter_map(|value| value.as_str().and_then(sanitize_mod_metadata_text))
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
    if let Some(text) = value.as_str().and_then(sanitize_mod_metadata_text) {
        return Some(vec![text]);
    }

    value.as_array().map(|values| {
        values
            .iter()
            .filter_map(|value| value.as_str().and_then(sanitize_mod_metadata_text))
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

        if let Some(display_name) = sanitize_mod_metadata_text(heading) {
            return Ok(Some(display_name));
        }
    }

    Ok(None)
}

pub(crate) fn validate_task_id_segment(task_id: &str) -> Result<()> {
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

/// Default Mod storage root: the app-data child that historically held `results.json` and
/// `sandboxes/`. A user-configured root replaces this directory, not its `sandboxes/` child.
pub fn default_mod_storage_root(app_data_root: &Path) -> PathBuf {
    app_data_root.join(MOD_IMPORT_APP_DATA_DIRECTORY)
}

fn mod_import_sandbox_root_path(storage_root: &Path) -> PathBuf {
    storage_root.join(MOD_IMPORT_SANDBOX_DIRECTORY)
}

fn open_managed_sandbox_root(storage_root: &Path, create: bool) -> Result<Dir> {
    let storage = if create {
        open_or_create_directory_nofollow(storage_root, "mod storage root")?
    } else {
        open_existing_directory_nofollow(storage_root, "mod storage root")?
    };
    if create {
        open_or_create_directory_chain(
            &storage,
            &[MOD_IMPORT_SANDBOX_DIRECTORY],
            "mod import sandbox root",
        )
    } else {
        open_existing_directory_chain(
            &storage,
            &[MOD_IMPORT_SANDBOX_DIRECTORY],
            "mod import sandbox root",
        )
    }
}

/// 拖拽清单的**容器层预检**（T22，#366）：只判断「这个文件能不能导入」，不解包。
///
/// 返回值刻意与 `prepare_package` **同一个错误类型**——于是清单里的档位与导入失败的
/// 档位是同一套词汇，前端不必维护第二张映射表。`Ok(())` = 可导入。
///
/// ## 只读头，不落盘
///
/// 与真正解包的区别只有一条：这里**不为 rar 落暂存**，直接用玩家给的路径调 unrar。
/// 理由是这一步只读归档头、一个字节都不写；而真正解包时仍然落一份暂存再交给它
/// （见 `extract_archive_with_limits`）。拖一次可能有几十个文件，
/// 为了预检把每个都完整拷一遍是不可接受的。
///
/// ## 它与解包链路是两个实现，靠测试钉住一致
///
/// 两处各写一遍「依次尝试已支持格式」难免漂移。共用代码的话，rar 的暂存与
/// 生命周期会把签名扭得很难看，所以选了另一条：
/// `probe_and_import_agree_on_every_fixture` 拿同一组语料同时跑预检与真实导入，
/// 断言两者给出**同一个错误码**。漂了就红。
pub fn probe_mod_archive(path: &Path) -> std::result::Result<(), ModImportPrepareError> {
    let mut archive = open_archive_file_nofollow(path)?;

    // 顺序与解包链路一致：zip → 7z → rar。
    if zip::ZipArchive::new(&mut archive).is_ok() {
        return Ok(());
    }

    archive
        .seek(io::SeekFrom::Start(0))
        .context("failed to rewind the mod import archive")?;
    match sevenz_archive_source::SevenZipArchive::open(&mut archive) {
        // 与 rar 同理：内容加密的 7z **是能打开的**，要密码要等解内容才暴露。
        // 所以额外看一眼编解码链里有没有 AES（不解码任何数据）。
        Ok(archive) if archive.is_content_encrypted() => {
            return Err(ModImportPrepareError::UnsupportedArchiveFeature(
                hmm_ports::UnsupportedArchiveFeature::Encrypted,
            ))
        }
        Ok(_) => return Ok(()),
        Err(error @ ModImportPrepareError::UnsupportedArchiveFeature(_)) => return Err(error),
        Err(_) => {}
    }

    match rar_archive_source::RarArchiveSource::open(path) {
        // **打开成功还不够**：rar 的加密与跨卷是**逐条目**的标志，`open` 看不见。
        // 所以把头走一遍（只读头、不解压，代价很小）。
        // 这条是被 `probe_and_import_agree_on_every_fixture` 逼出来的——
        // 第一版只 open，于是加密包在清单里显示成「可导入」，
        // 玩家确认之后才发现被骗。
        Ok(mut source) => return walk_rar_headers(&mut source),
        Err(error @ ModImportPrepareError::UnsupportedArchiveFeature(_)) => return Err(error),
        Err(_) => {}
    }

    archive
        .seek(io::SeekFrom::Start(0))
        .context("failed to rewind the mod import archive")?;
    let unopenable = anyhow::anyhow!("failed to read archive");
    Err(explain_unopenable_archive(&mut archive, unopenable))
}

/// 把 rar 的条目头走一遍，只为触发**容器级**的判定（加密、跨卷）。
/// 不调 `write_current_to`，所以不解压、不落盘。
fn walk_rar_headers(
    source: &mut rar_archive_source::RarArchiveSource,
) -> std::result::Result<(), ModImportPrepareError> {
    loop {
        match source.next_entry() {
            Ok(Some(_)) => {}
            Ok(None) => return Ok(()),
            Err(error) => {
                return Err(source
                    .take_structured_failure()
                    .unwrap_or(ModImportPrepareError::Other(error)))
            }
        }
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

/// 包沙箱**之外**的一次性暂存目录，专供「只接受路径的解压器」用。
///
/// 名字是 `<task_id>.staging`。任务 id 只允许 `[A-Za-z0-9_-]`（见
/// `validate_task_id_segment`），所以带 `.` 的这个名字**永远不可能与真实任务沙箱撞名**
/// ——这不是巧合，是挑出来的。
struct ArchiveScratch {
    directory_name: String,
    /// 真实路径。unrar 的 DLL API 只接受路径，cap-std 的 `Dir` 给不出来。
    archive_path: PathBuf,
}

/// 暂存目录里那份压缩包的固定文件名。它是我们自己创建的，不取玩家的原名
/// ——原名可能带任何字符，而这里不需要它有任何意义。
const SCRATCH_ARCHIVE_FILE_NAME: &str = "archive.bin";

impl ArchiveScratch {
    fn new(sandbox_root: &Path, task_id: &str) -> Self {
        let directory_name = format!("{task_id}.staging");
        let archive_path = sandbox_root
            .join(&directory_name)
            .join(SCRATCH_ARCHIVE_FILE_NAME);
        Self {
            directory_name,
            archive_path,
        }
    }

    /// 把 reader 原样落一份到暂存目录，返回给解压器用的路径。
    ///
    /// 目录与文件都经 cap-std 在沙箱根的能力下创建，不走操作系统全路径。
    fn spill<R>(&self, root: &Dir, archive_file: &mut R) -> Result<&Path>
    where
        R: Read + Seek + ?Sized,
    {
        let directory = open_or_create_child_directory(
            root,
            std::ffi::OsStr::new(&self.directory_name),
            "mod import archive staging",
        )?;
        let mut target = create_new_regular_file(
            &directory,
            std::ffi::OsStr::new(SCRATCH_ARCHIVE_FILE_NAME),
            "staged mod import archive",
        )?;
        archive_file
            .seek(io::SeekFrom::Start(0))
            .context("failed to rewind the mod import archive")?;
        io::copy(archive_file, &mut target).context("failed to stage the mod import archive")?;
        target
            .sync_all()
            .context("failed to flush the staged mod import archive")?;
        Ok(&self.archive_path)
    }

    /// 无条件清掉。**不返回错误**：清不掉不该把一次成功的导入变成失败，
    /// 而残留物落在沙箱根下、与任务沙箱同一套清理规则。
    fn cleanup(&self, root: &Dir) {
        let _ = remove_child_tree_nofollow(
            root,
            std::ffi::OsStr::new(&self.directory_name),
            "mod import archive staging",
        );
    }
}

/// 依次尝试每一种**已支持格式**的打开；全部失败才嗅探，且嗅探只用来解释失败。
///
/// 「第 1 步是每一种已支持格式，不是 zip」这句不是措辞讲究：自解压 RAR 是个 `MZ`
/// 开头的 `.exe`，若第 1 步写死成 zip，它会被嗅探判成「不是压缩包」，
/// 而 unrar 其实打得开。**支持集合会增长，判别必须跟着长。**
///
/// **顺序：zip → 7z → rar**，两条依据叠在一起：
///
/// 1. 常见程度：zip 最多。
/// 2. **要落盘的排最后。** zip 与 7z 都直接吃 reader，尝试失败零代价；
///    rar 那一步必须先把 reader 完整落一份盘（unrar 的 DLL API 只认路径）。
///    若 rar 排在 7z 前面，导入一个 7z 就会白拷一遍整包。
fn extract_archive_with_limits<R>(
    archive_file: &mut R,
    root: &Dir,
    scratch: &ArchiveScratch,
    sandbox_root: &Dir,
    cancellation_token: &dyn CancellationToken,
    limits: ArchiveExtractionLimits,
) -> std::result::Result<(), ModImportPrepareError>
where
    R: Read + Seek + ?Sized,
{
    // ── 格式 1：zip ──
    // 显式重借：泛型参数会把 `&mut R` 直接 move 进去，失败分支就再也拿不到 reader 了。
    let zip_error = match zip::ZipArchive::new(&mut *archive_file) {
        Ok(mut archive) => {
            // 七条门禁全部由与格式无关的外壳施加（T21-B）。zip 这里只当一个
            // 「按顺序产出条目」的适配器——新增格式时不得再复制一份门禁。
            let mut source = ZipArchiveSource {
                archive: &mut archive,
                index: 0,
            };
            extract_archive(&mut source, sandbox_root, cancellation_token, limits)?;
            return Ok(());
        }
        Err(error) => anyhow::Error::new(error).context("failed to read zip archive"),
    };

    // ── 格式 2：7z ──
    // 直接吃 reader，不需要落盘暂存。它是**推驱动**的（只有 for_each_entries
    // 这一种接口），所以不实现 ArchiveSource，而是在它的回调里调 ArchiveGate
    // ——门禁仍是同一份。
    archive_file
        .seek(io::SeekFrom::Start(0))
        .context("failed to rewind the mod import archive")?;
    match sevenz_archive_source::SevenZipArchive::open(&mut *archive_file) {
        Ok(mut archive) => {
            let mut gate = ArchiveGate::new(sandbox_root, cancellation_token, limits);
            return archive.extract_into(&mut gate);
        }
        // 打得开但用了不支持的特性（加密）——直接报，不要退回嗅探。
        Err(error @ ModImportPrepareError::UnsupportedArchiveFeature(_)) => return Err(error),
        Err(_) => {}
    }

    // ── 格式 3：rar ──
    // unrar 只能按路径打开，所以先落一份到包沙箱之外的暂存目录。
    // 这样它读到的**只是我们刚创建的文件**：玩家给的路径不进它的视野，
    // symlink 跟随与 TOCTOU 一并消失。代价是一次完整拷贝，见设计文档。
    let staged = scratch.spill(root, archive_file)?;
    match rar_archive_source::RarArchiveSource::open(staged) {
        Ok(mut source) => {
            let outcome = extract_archive(&mut source, sandbox_root, cancellation_token, limits);
            return match outcome {
                Ok(()) => Ok(()),
                // 适配器把「加密」「分卷」这类语义单独存着，不靠 downcast 反推
                // ——外壳的接口是 anyhow，语义没法从里面还原。
                Err(error) => Err(source
                    .take_structured_failure()
                    .unwrap_or(ModImportPrepareError::Other(error))),
            };
        }
        // 打不开，但认得出是什么问题（加密包等）——直接报，不要退回嗅探。
        Err(error @ ModImportPrepareError::UnsupportedArchiveFeature(_)) => return Err(error),
        // 就是打不开。继续往下走嗅探。
        Err(_) => {}
    }

    // ── 都打不开：嗅探，只为解释失败 ──
    Err(explain_unopenable_archive(archive_file, zip_error))
}

/// zip 的格式适配器。**只做三件事**：报条目总数、产出条目头、把字节推给 sink。
/// 任何门禁都不在这里——它们在 `archive_extraction` 里，对所有格式一视同仁。
struct ZipArchiveSource<'a, R: Read + Seek + ?Sized> {
    archive: &'a mut zip::ZipArchive<&'a mut R>,
    index: usize,
}

impl<R: Read + Seek + ?Sized> ArchiveSource for ZipArchiveSource<'_, R> {
    fn declared_entry_count(&self) -> Option<usize> {
        // zip 有中央目录，总数可预取——外壳因此能在写第一个字节前就拒。
        // rar / tar 拿不到，会返回 None 并降级为边读边数。
        Some(self.archive.len())
    }

    fn next_entry(&mut self) -> Result<Option<ArchiveEntryHeader>> {
        if self.index >= self.archive.len() {
            return Ok(None);
        }
        let entry = self
            .archive
            .by_index(self.index)
            .context("failed to read zip archive entry")?;
        self.index += 1;
        let kind = if entry.is_symlink() {
            ArchiveEntryKind::Symlink
        } else if entry.is_dir() {
            ArchiveEntryKind::Directory
        } else {
            ArchiveEntryKind::File
        };
        Ok(Some(ArchiveEntryHeader {
            name: entry.name().to_owned(),
            kind,
            // 声明值只供外壳做写盘前的快速失败；它可以说谎（#367），承重的是实际字节。
            declared_size: Some(entry.size()),
        }))
    }

    fn write_current_to(&mut self, sink: &mut dyn ArchiveChunkSink) -> Result<()> {
        // `by_index` 只是定位 + 解本地文件头，重开一次是 O(1)。这样适配器就不必持有
        // 一个借用 archive 的条目句柄（那会变成自引用结构）。
        let mut entry = self
            .archive
            .by_index(self.index - 1)
            .context("failed to read zip archive entry")?;
        pump_reader_into_sink(&mut entry, sink)
    }
}

/// tar 的 magic 不在首字节，而在偏移 257 —— 所以判别**不能**写成「统一读前 N 字节比对」。
const TAR_MAGIC_OFFSET: u64 = 257;
const TAR_MAGIC: &[u8] = b"ustar";
/// 首字节签名表。判别只在所有「已支持格式」的打开尝试都失败之后执行。
/// **rar 已从这张表里移走**（T21-C）：它现在是「已支持格式」，判别第 1 步就会尝试打开。
/// 留在这里的话，一个**损坏的** rar 会被报成「格式不支持」——那只是把误导换个方向，
/// 正是本设计三条不可让步判据里的第一条禁止的事。
///
/// 这条移动是「支持集合增长时判别自动收敛」的实例：某格式从第 3 步升到第 1 步，
/// 这张表里对应的行就该失效，不需要额外维护档位。**7z 同理已移走**（T21-D）。
///
/// 表里现在只剩「认得出、但确实不打算支持」的那些：压缩流（gzip/xz/bzip2/zstd）
/// 与 tar 家族。它们不是归档容器就是本轮延后的目标。
const ARCHIVE_SIGNATURES: &[(&[u8], UnsupportedArchiveFormat)] = &[
    (b"\xfd7zXZ\x00", UnsupportedArchiveFormat::Xz),
    (b"\x1f\x8b", UnsupportedArchiveFormat::Gzip),
    (b"BZh", UnsupportedArchiveFormat::Bzip2),
    (b"\x28\xb5\x2f\xfd", UnsupportedArchiveFormat::Zstd),
];
const NON_ARCHIVE_SIGNATURES: &[(&[u8], NonArchiveFile)] = &[
    (b"MZ", NonArchiveFile::WindowsExecutable),
    (
        b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1",
        NonArchiveFile::CompoundDocument,
    ),
];

/// 解释「打不开」，而不是拦截「不该开」。
///
/// 认不出就原样退回打开失败的错误——也就是既有的 `retry-hint` 行为。**这一点是硬要求**：
/// 损坏的 zip 必须留在原档位，不能被新档位吃掉，否则只是把误导换了个方向。
fn explain_unopenable_archive<R>(reader: &mut R, zip_error: anyhow::Error) -> ModImportPrepareError
where
    R: Read + Seek + ?Sized,
{
    match sniff_container(reader) {
        Some(Ok(format)) => ModImportPrepareError::UnsupportedArchiveFormat(format),
        Some(Err(non_archive)) => ModImportPrepareError::NotAnArchive(non_archive),
        None => ModImportPrepareError::Other(zip_error),
    }
}

/// `Some(Ok(_))` = 认得出的归档容器；`Some(Err(_))` = 认得出的非归档文件；`None` = 认不出。
fn sniff_container<R>(
    reader: &mut R,
) -> Option<std::result::Result<UnsupportedArchiveFormat, NonArchiveFile>>
where
    R: Read + Seek + ?Sized,
{
    let mut head = [0_u8; 8];
    reader.seek(io::SeekFrom::Start(0)).ok()?;
    let head_len = read_up_to(reader, &mut head).ok()?;
    let head = &head[..head_len];

    for (signature, format) in ARCHIVE_SIGNATURES {
        if head.starts_with(signature) {
            return Some(Ok(*format));
        }
    }
    for (signature, kind) in NON_ARCHIVE_SIGNATURES {
        if head.starts_with(signature) {
            return Some(Err(*kind));
        }
    }

    let mut tar_magic = [0_u8; 5];
    if reader.seek(io::SeekFrom::Start(TAR_MAGIC_OFFSET)).is_ok()
        && read_up_to(reader, &mut tar_magic).ok()? == tar_magic.len()
        && tar_magic == TAR_MAGIC
    {
        return Some(Ok(UnsupportedArchiveFormat::Tar));
    }

    None
}

/// 读到缓冲区满或流结束为止，返回实际读到的长度。短文件不算错误——它只是认不出。
fn read_up_to<R>(reader: &mut R, buffer: &mut [u8]) -> io::Result<usize>
where
    R: Read + ?Sized,
{
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
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
    fn storage_root_preparer_creates_sandboxes_child_under_the_storage_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let storage_root = temp.path().join("HMMMods");
        fs::create_dir(&storage_root).expect("create storage root");
        let archive_path = temp.path().join("sample.zip");
        create_zip(
            &archive_path,
            &[("nativePC/readme.txt", b"hello".as_slice())],
        );

        let preparer = ZipModImportPackagePreparer::new_in_storage_root(storage_root.clone());
        let prepared =
            prepare_package(&preparer, "task-1", &archive_path).expect("prepare package");

        assert_eq!(
            prepared.sandbox_root,
            storage_root.join("sandboxes").join("task-1")
        );
        assert_eq!(
            fs::read_to_string(prepared.sandbox_root.join("nativePC/readme.txt"))
                .expect("read extracted file"),
            "hello"
        );
        assert!(
            !storage_root.join("mod-import").exists(),
            "a configured storage root must not grow a nested mod-import directory"
        );
    }

    #[test]
    fn storage_root_preparer_creates_a_missing_default_root_below_app_data() {
        let temp = tempfile::tempdir().expect("temp dir");
        let app_data = temp.path().join("app-data");
        fs::create_dir(&app_data).expect("create app data");
        let archive_path = temp.path().join("sample.zip");
        create_zip(
            &archive_path,
            &[("nativePC/readme.txt", b"hello".as_slice())],
        );

        let preparer =
            ZipModImportPackagePreparer::new_in_storage_root(default_mod_storage_root(&app_data));
        let prepared =
            prepare_package(&preparer, "task-1", &archive_path).expect("prepare package");

        assert_eq!(
            prepared.sandbox_root,
            app_data.join("mod-import").join("sandboxes").join("task-1")
        );
    }

    #[test]
    fn storage_root_locator_resolves_and_cleans_below_the_storage_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let storage_root = temp.path().join("HMMMods");
        let package_root = storage_root.join("sandboxes").join("task-1");
        fs::create_dir_all(package_root.join("nested")).expect("create package sandbox");
        fs::write(package_root.join("nested").join("fixture.bin"), b"fixture")
            .expect("write fixture");
        let sibling = storage_root.join("sandboxes").join("task-2");
        fs::create_dir_all(&sibling).expect("create sibling sandbox");

        let locator = TaskScopedModImportSandboxLocator::new_in_storage_root(storage_root.clone());
        assert_eq!(locator.sandbox_root(), storage_root.join("sandboxes"));
        assert_eq!(
            locator
                .sandbox_root_for_package("task-1")
                .expect("resolve package"),
            package_root
        );

        locator
            .cleanup_sandbox_for_package("task-1")
            .expect("cleanup task sandbox");

        assert!(!package_root.exists());
        assert!(sibling.exists());
    }

    #[test]
    fn storage_root_locator_cleanup_is_a_no_op_when_the_root_is_missing() {
        let temp = tempfile::tempdir().expect("temp dir");
        let storage_root = temp.path().join("unplugged-drive").join("HMMMods");

        let locator = TaskScopedModImportSandboxLocator::new_in_storage_root(storage_root);

        locator
            .cleanup_sandbox_for_package("task-1")
            .expect("missing root has nothing to clean");
    }

    #[test]
    fn storage_root_locator_rejects_a_linked_storage_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside root");
        fs::create_dir_all(outside.path().join("sandboxes").join("task-1"))
            .expect("create outside package");
        let linked_root = temp.path().join("HMMMods");
        if !try_create_directory_link(outside.path(), &linked_root) {
            return;
        }

        let locator = TaskScopedModImportSandboxLocator::new_in_storage_root(linked_root.clone());
        let error = locator
            .cleanup_sandbox_for_package("task-1")
            .expect_err("linked storage root must be rejected");

        remove_directory_link(&linked_root);
        assert!(error.to_string().contains("mod storage root"));
        assert!(
            outside.path().join("sandboxes").join("task-1").exists(),
            "cleanup through a linked root must not delete the target"
        );
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
        let limits = ArchiveExtractionLimits {
            max_entries: 2,
            max_single_file_bytes: 1024,
            max_total_uncompressed_bytes: 4096,
        };
        let preparer = ZipModImportPackagePreparer {
            sandbox_root: sandbox_root.clone(),
            storage_root: None,
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
        let limits = ArchiveExtractionLimits {
            max_entries: 10,
            max_single_file_bytes: 4,
            max_total_uncompressed_bytes: 4096,
        };
        let preparer = ZipModImportPackagePreparer {
            sandbox_root: sandbox_root.clone(),
            storage_root: None,
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
        let limits = ArchiveExtractionLimits {
            max_entries: 10,
            max_single_file_bytes: 1024,
            max_total_uncompressed_bytes: 7,
        };
        let preparer = ZipModImportPackagePreparer {
            sandbox_root: sandbox_root.clone(),
            storage_root: None,
            limits,
        };

        let error = prepare_package(&preparer, "task-1", &archive_path)
            .expect_err("total size limit should reject archive");

        assert!(error
            .to_string()
            .contains("archive total size limit exceeded"));
        assert!(!sandbox_root.join("task-1").exists());
    }

    /// #367：声明值可以说谎，所以承重的必须是实际写入的字节。
    ///
    /// 这个包声明每条 4 字节（过得了预检），实际每条 4096 字节。修复前它会**导入成功**，
    /// 把 8 KiB 写进沙箱。
    #[test]
    fn a_zip_that_lies_about_its_declared_size_cannot_exceed_the_single_file_limit() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("lying-single.zip");
        create_zip_lying_about_declared_size(&archive_path, "payload.bin", &[0_u8; 4096], 4);
        let sandbox_root = temp.path().join("sandboxes");
        let preparer = ZipModImportPackagePreparer {
            sandbox_root: sandbox_root.clone(),
            storage_root: None,
            limits: ArchiveExtractionLimits {
                max_entries: 10,
                max_single_file_bytes: 64,
                max_total_uncompressed_bytes: 1024 * 1024,
            },
        };

        let error = prepare_package(&preparer, "task-1", &archive_path)
            .expect_err("a lying declared size must not buy a bigger file");

        assert!(
            error
                .to_string()
                .contains("archive file size limit exceeded")
                || format!("{error:#}").contains("archive file size limit exceeded"),
            "unexpected error: {error:#}"
        );
        assert!(
            !sandbox_root.join("task-1").exists(),
            "sandbox must be cleaned"
        );
    }

    /// 同一条谎言用来撑爆总量：单文件限额放得很宽，总量卡在 64 字节。
    #[test]
    fn a_zip_that_lies_about_its_declared_size_cannot_exceed_the_total_limit() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("lying-total.zip");
        create_zip_lying_about_declared_size(&archive_path, "payload.bin", &[0_u8; 4096], 4);
        let sandbox_root = temp.path().join("sandboxes");
        let preparer = ZipModImportPackagePreparer {
            sandbox_root: sandbox_root.clone(),
            storage_root: None,
            limits: ArchiveExtractionLimits {
                max_entries: 10,
                max_single_file_bytes: 1024 * 1024,
                max_total_uncompressed_bytes: 64,
            },
        };

        let error = prepare_package(&preparer, "task-1", &archive_path)
            .expect_err("a lying declared size must not buy a bigger total");

        assert!(
            format!("{error:#}").contains("archive total size limit exceeded"),
            "unexpected error: {error:#}"
        );
        assert!(
            !sandbox_root.join("task-1").exists(),
            "sandbox must be cleaned"
        );
    }

    /// 控制组：断言的是**等价性**（内容逐字节不变），不是「没报错」——
    /// 只断言不报错的话，一个把所有内容截断成 0 字节的实现也能过。
    #[test]
    fn an_honest_archive_still_extracts_byte_for_byte() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("honest.zip");
        let first = vec![7_u8; 5000];
        let second = b"second entry contents".to_vec();
        create_zip(
            &archive_path,
            &[
                ("a/first.bin", first.as_slice()),
                ("second.txt", second.as_slice()),
            ],
        );
        let sandbox_root = temp.path().join("sandboxes");
        let preparer = ZipModImportPackagePreparer {
            sandbox_root: sandbox_root.clone(),
            storage_root: None,
            limits: ArchiveExtractionLimits {
                max_entries: 10,
                max_single_file_bytes: 8192,
                max_total_uncompressed_bytes: 8192,
            },
        };

        let prepared = prepare_package(&preparer, "task-1", &archive_path)
            .expect("an honest archive within the limits must still import");

        assert_eq!(
            fs::read(prepared.sandbox_root.join("a/first.bin")).expect("read first"),
            first
        );
        assert_eq!(
            fs::read(prepared.sandbox_root.join("second.txt")).expect("read second"),
            second
        );
    }

    /// 快速失败没有退化：诚实声明超限的包仍然在**写第一个字节之前**被拒。
    /// 判据是沙箱里连目标文件都不存在（沙箱整棵树也会被清掉）。
    #[test]
    fn an_honest_oversized_declaration_is_still_rejected_before_writing() {
        let temp = tempfile::tempdir().expect("temp dir");
        let archive_path = temp.path().join("honest-oversized.zip");
        create_zip(&archive_path, &[("payload.bin", [0_u8; 4096].as_slice())]);
        let sandbox_root = temp.path().join("sandboxes");
        let preparer = ZipModImportPackagePreparer {
            sandbox_root: sandbox_root.clone(),
            storage_root: None,
            limits: ArchiveExtractionLimits {
                max_entries: 10,
                max_single_file_bytes: 64,
                max_total_uncompressed_bytes: 1024 * 1024,
            },
        };

        let error = prepare_package(&preparer, "task-1", &archive_path)
            .expect_err("an honest oversized declaration must still fail fast");

        assert!(
            format!("{error:#}").contains("archive file size limit exceeded"),
            "unexpected error: {error:#}"
        );
        assert!(
            !sandbox_root.join("task-1").exists(),
            "sandbox must be cleaned"
        );
    }

    // ---- #348 容器判别 ----
    //
    // 这些用例把**我们的映射**钉住：某个字节前缀 -> 某个档位。
    //
    // 除 zip（由 `zip` crate 真实产出）与 Windows 下的真实 PE 之外，其余签名是按公开格式
    // 文档构造的字节，不是从真实样本文件捕获的。这个取舍是有意的，因为**判别写错会
    // fail closed**：认不出就退回既有的 `retry-hint`，也就是今天的行为，不会造出新的失败
    // 模式，更不会放行本来该拒的东西。真实样本的覆盖留给 T21-C/D 接入各自格式时补。

    fn prepare_bytes(temp: &tempfile::TempDir, name: &str, bytes: &[u8]) -> ModImportPrepareError {
        let archive_path = temp.path().join(name);
        fs::write(&archive_path, bytes).expect("write fixture");
        let preparer = ZipModImportPackagePreparer::new(temp.path().join("sandboxes"));
        prepare_package(&preparer, name, &archive_path).expect_err("fixture must not import")
    }

    fn assert_unsupported(error: &ModImportPrepareError, expected: UnsupportedArchiveFormat) {
        match error {
            ModImportPrepareError::UnsupportedArchiveFormat(format) => {
                assert_eq!(*format, expected)
            }
            other => panic!("expected UnsupportedArchiveFormat({expected:?}), got {other:?}"),
        }
    }

    #[test]
    fn known_archive_containers_are_reported_as_unsupported_formats() {
        let temp = tempfile::tempdir().expect("temp dir");
        // **rar（T21-C）与 7z（T21-D）都不在这张表里了**：它们已是「已支持格式」，
        // 带其签名却打不开的文件是**损坏的包**，必须留在 retry-hint 档
        // ——见 `a_truncated_rar_stays_in_the_retry_hint_tier`
        // 与 `a_truncated_sevenz_stays_in_the_retry_hint_tier`。
        //
        // 剩下的都是「认得出、但确实不打算支持」：压缩流与 tar 家族。
        let cases: &[(&str, &[u8], UnsupportedArchiveFormat)] = &[
            ("gzip", b"\x1f\x8bpayload", UnsupportedArchiveFormat::Gzip),
            ("xz", b"\xfd7zXZ\x00payload", UnsupportedArchiveFormat::Xz),
            ("bzip2", b"BZh9payload", UnsupportedArchiveFormat::Bzip2),
            (
                "zstd",
                b"\x28\xb5\x2f\xfdpayload",
                UnsupportedArchiveFormat::Zstd,
            ),
        ];

        for (name, bytes, expected) in cases {
            let error = prepare_bytes(&temp, name, bytes);
            assert_unsupported(&error, *expected);
            assert_eq!(error.code(), "mod_import_unsupported_archive_format");
        }
    }

    /// tar 的 magic 在偏移 257，不在首字节——写成「统一读前 N 字节比对」它就会被静默
    /// 归到「认不出」。这条用例专门守这个实现陷阱。
    #[test]
    fn a_tar_archive_is_recognised_by_its_magic_at_offset_257() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut bytes = vec![0_u8; 512];
        bytes[..8].copy_from_slice(b"name.txt");
        bytes[257..262].copy_from_slice(b"ustar");

        let error = prepare_bytes(&temp, "tar", &bytes);
        assert_unsupported(&error, UnsupportedArchiveFormat::Tar);
    }

    #[test]
    fn known_non_archive_files_are_reported_as_such() {
        let temp = tempfile::tempdir().expect("temp dir");
        let cases: &[(&str, &[u8], NonArchiveFile)] = &[
            (
                "pe",
                b"MZ\x90\x00\x03\x00\x00\x00",
                NonArchiveFile::WindowsExecutable,
            ),
            (
                "ole",
                b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1padding",
                NonArchiveFile::CompoundDocument,
            ),
        ];

        for (name, bytes, expected) in cases {
            let error = prepare_bytes(&temp, name, bytes);
            match &error {
                ModImportPrepareError::NotAnArchive(kind) => assert_eq!(kind, expected),
                other => panic!("expected NotAnArchive({expected:?}), got {other:?}"),
            }
            assert_eq!(error.code(), "mod_import_not_an_archive");
        }
    }

    /// 唯一一条用真实样本的判别用例：测试二进制自己就是一个真的 PE。
    /// CI 跑在 ubuntu，所以只在 Windows 下有意义。
    #[cfg(windows)]
    #[test]
    fn a_real_windows_executable_is_reported_as_not_an_archive() {
        let temp = tempfile::tempdir().expect("temp dir");
        let real_pe = std::env::current_exe().expect("current exe");
        let bytes = fs::read(&real_pe).expect("read the test binary itself");
        assert_eq!(&bytes[..2], b"MZ", "the test binary should be a PE");

        let error = prepare_bytes(&temp, "real-pe", &bytes);
        assert!(
            matches!(
                error,
                ModImportPrepareError::NotAnArchive(NonArchiveFile::WindowsExecutable)
            ),
            "got {error:?}"
        );
    }

    /// **硬边界**：损坏的 zip 必须留在既有档位，不能被新档位吃掉——
    /// 否则只是把误导换了个方向。
    #[test]
    fn a_truncated_zip_still_falls_back_to_the_generic_failure() {
        let temp = tempfile::tempdir().expect("temp dir");
        let honest = temp.path().join("honest.zip");
        create_zip(&honest, &[("a.txt", b"hello".as_slice())]);
        let mut bytes = fs::read(&honest).expect("read honest zip");
        bytes.truncate(bytes.len() / 2);

        let error = prepare_bytes(&temp, "truncated", &bytes);
        assert!(
            matches!(error, ModImportPrepareError::Other(_)),
            "a damaged zip must stay in the generic tier, got {error:?}"
        );
        assert_eq!(error.code(), "mod_import_prepare_failed");
    }

    #[test]
    fn unrecognised_and_tiny_inputs_fall_back_to_the_generic_failure() {
        let temp = tempfile::tempdir().expect("temp dir");
        for (name, bytes) in [
            ("empty", b"".as_slice()),
            ("one-byte", b"x".as_slice()),
            ("noise", b"not a container at all".as_slice()),
        ] {
            let error = prepare_bytes(&temp, name, bytes);
            assert!(
                matches!(error, ModImportPrepareError::Other(_)),
                "{name} should stay in the generic tier, got {error:?}"
            );
        }
    }

    /// 回归守卫：归档不保证从首字节开始。判别若被写成预检，这条会立刻转红。
    ///
    /// 断言的是**等价性**——加不加前缀，结果必须一致。这样不论 `zip` 当前是否支持
    /// 自解压形态，这条用例都成立，不依赖任何未验证的假设。
    ///
    /// 2026-09-08 实测：`zip` 2.4.2 **能**打开带前缀的归档（本用例走的是 `(Ok, Ok)` 分支，
    /// 若只有 `plain` 成功会 panic）。也就是说按 `MZ` 首字节预检会**当场弄坏本来能导入的
    /// 自解压 zip**——这正是「先开后嗅」不可让步的原因。
    #[test]
    fn prepending_a_payload_to_a_zip_does_not_change_the_outcome() {
        let temp = tempfile::tempdir().expect("temp dir");
        let plain_path = temp.path().join("plain.zip");
        create_zip(&plain_path, &[("a.txt", b"hello".as_slice())]);
        let plain_bytes = fs::read(&plain_path).expect("read zip");

        let mut prefixed_bytes = b"MZ\x90\x00self-extracting stub".to_vec();
        prefixed_bytes.extend_from_slice(&plain_bytes);
        let prefixed_path = temp.path().join("prefixed.zip");
        fs::write(&prefixed_path, &prefixed_bytes).expect("write prefixed zip");

        let preparer = ZipModImportPackagePreparer::new(temp.path().join("sandboxes"));
        let plain = prepare_package(&preparer, "plain", &plain_path);
        let prefixed = prepare_package(&preparer, "prefixed", &prefixed_path);

        match (&plain, &prefixed) {
            (Ok(_), Ok(prepared)) => {
                assert_eq!(
                    fs::read(prepared.sandbox_root.join("a.txt")).expect("read extracted"),
                    b"hello",
                    "a prefixed zip that opens must extract the same content"
                );
            }
            (Err(_), Err(error)) => {
                // 打不开也可以，但**绝不能**被判成「根本不是压缩包」——那是判别越权。
                assert!(
                    !matches!(error, ModImportPrepareError::NotAnArchive(_)),
                    "a prefixed but valid zip must not be labelled NotAnArchive, got {error:?}"
                );
            }
            (plain, prefixed) => panic!(
                "prefixing changed the outcome: plain={:?} prefixed={:?}",
                plain.is_ok(),
                prefixed.is_ok()
            ),
        }
    }

    // ---- #348 切片 C：rar ----
    //
    // 语料由 `hmm-unrar-sys::fixture` 现场合成——`.rar` 提交不进仓库，开发机上也没有
    // 任何 RAR 压缩器。合成不会造成假绿：拼错的话 unrar 直接打不开、用例硬失败，
    // 而读取器不是我们写的。

    use hmm_unrar_sys::fixture::{Rar5Archive, Rar5Entry};
    use hmm_unrar_sys::{FSREDIR_HARDLINK, FSREDIR_UNIXSYMLINK};

    fn rar_preparer(
        temp: &tempfile::TempDir,
        limits: ArchiveExtractionLimits,
    ) -> ZipModImportPackagePreparer {
        ZipModImportPackagePreparer {
            sandbox_root: temp.path().join("sandboxes"),
            storage_root: None,
            limits,
        }
    }

    fn write_rar(temp: &tempfile::TempDir, name: &str, archive: &Rar5Archive) -> PathBuf {
        let path = temp.path().join(name);
        fs::write(&path, archive.build()).expect("write rar fixture");
        path
    }

    /// 正向：一个真实（合成的）RAR5 现在能导入，内容逐字节一致。
    ///
    /// 断言的是**等价性**而不是「没报错」——只断言不报错的话，
    /// 一个把内容丢光的实现也能过。
    #[test]
    fn a_rar_archive_now_imports_with_byte_identical_content() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = write_rar(
            &temp,
            "mod.rar",
            &Rar5Archive::new(vec![
                Rar5Entry::file("readme.txt", b"hello from rar".to_vec()),
                Rar5Entry::directory("nativePC"),
                Rar5Entry::file("nativePC/data.bin", vec![0x5a; 9000]),
            ]),
        );
        let preparer = rar_preparer(&temp, default_extraction_limits());
        let prepared = prepare_package(&preparer, "rar-1", &path).expect("a good rar must import");

        assert_eq!(
            fs::read(prepared.sandbox_root.join("readme.txt")).expect("read"),
            b"hello from rar"
        );
        assert_eq!(
            fs::read(prepared.sandbox_root.join("nativePC/data.bin")).expect("read"),
            vec![0x5a; 9000]
        );
    }

    /// **判别顺序的活体断言。**
    ///
    /// 自解压 RAR 是个 `MZ` 开头的 `.exe`。T21-A 时它落在 `not-an-archive`；
    /// C 之后必须**正常导入**。翻不过来就说明判别第 1 步被写死成了固定格式列表，
    /// 而不是「当前已支持的格式集合」——这正是设计要防的那个实现陷阱。
    #[test]
    fn a_self_extracting_rar_now_imports_instead_of_being_called_not_an_archive() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = write_rar(
            &temp,
            "sfx.exe",
            &Rar5Archive::new(vec![Rar5Entry::file("a.txt", b"sfx payload".to_vec())])
                .with_sfx_prefix(b"MZ\x90\x00\x03\x00\x00\x00stub".to_vec()),
        );
        assert_eq!(
            &fs::read(&path).expect("read")[..2],
            b"MZ",
            "语料本身必须是 MZ 开头，否则这条用例证明不了任何事"
        );

        let preparer = rar_preparer(&temp, default_extraction_limits());
        let prepared = prepare_package(&preparer, "sfx-1", &path)
            .expect("自解压 RAR 必须能导入——报 not-an-archive 就是判别被写死了");
        assert_eq!(
            fs::read(prepared.sandbox_root.join("a.txt")).expect("read"),
            b"sfx payload"
        );
    }

    /// 损坏的 rar 必须留在 `retry-hint`，**不能**被说成「格式不支持」。
    ///
    /// rar 已是已支持格式，所以「带 rar 签名但打不开」只有一个含义：包坏了。
    /// 这是三条不可让步判据里的第一条在 rar 侧的对应用例。
    #[test]
    fn a_truncated_rar_stays_in_the_retry_hint_tier() {
        let temp = tempfile::tempdir().expect("temp dir");
        let full = Rar5Archive::new(vec![Rar5Entry::file("a.txt", vec![1_u8; 4096])]).build();
        for (label, bytes) in [
            // 数据区被砍掉一半：头读得出来，解到一半断流。
            ("halved", full[..full.len() / 2].to_vec()),
            // 签名之后全是垃圾：连主头都解析不了。
            ("garbage-after-signature", {
                let mut bytes = b"Rar!\x1a\x07\x01\x00".to_vec();
                bytes.extend_from_slice(&[0xff_u8; 64]);
                bytes
            }),
        ] {
            let error = prepare_bytes(&temp, label, &bytes);
            assert!(
                matches!(error, ModImportPrepareError::Other(_)),
                "{label}: 损坏的 rar 必须留在 retry-hint，得到 {error:?}"
            );
        }
    }

    /// **空归档：rar 与 zip 必须行为一致。**
    ///
    /// 一个只有签名的 RAR5 在 unrar 眼里不是「损坏」，而是**合法的空归档**
    /// （首次 `read_header` 直接返回 `END_ARCHIVE`）——落地时实测才发现，
    /// 我原先想当然把它当成截断包了。
    ///
    /// 所以这里断言的是**等价性**，不是某个具体结果：「空包该不该拒」是导入链路的
    /// 既有语义，不该因为换了个容器格式就变。真要改，那是另一件事、另一个 issue。
    #[test]
    fn an_empty_rar_behaves_the_same_as_an_empty_zip() {
        let temp = tempfile::tempdir().expect("temp dir");
        let zip_path = temp.path().join("empty.zip");
        create_zip(&zip_path, &[]);
        let rar_path = write_rar(&temp, "empty.rar", &Rar5Archive::new(vec![]));

        let preparer = rar_preparer(&temp, default_extraction_limits());
        let zip_outcome = prepare_package(&preparer, "empty-zip", &zip_path);
        let rar_outcome = prepare_package(&preparer, "empty-rar", &rar_path);

        assert_eq!(
            zip_outcome.is_ok(),
            rar_outcome.is_ok(),
            "空 zip 与空 rar 的结果必须一致：zip={:?} rar={:?}",
            zip_outcome
                .as_ref()
                .map(|_| ())
                .map_err(|e| format!("{e:?}")),
            rar_outcome
                .as_ref()
                .map(|_| ())
                .map_err(|e| format!("{e:?}")),
        );
    }

    /// 加密包落到自己的档位，而不是一句「请检查压缩包后重试」。
    #[test]
    fn an_encrypted_rar_lands_in_its_own_tier() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = write_rar(
            &temp,
            "secret.rar",
            &Rar5Archive::new(vec![
                Rar5Entry::file("secret.txt", b"cipher".to_vec()).with_encrypted_flag()
            ]),
        );
        let preparer = rar_preparer(&temp, default_extraction_limits());
        let error = prepare_package(&preparer, "enc-1", &path).expect_err("加密包必须被拒");
        assert!(
            matches!(
                error,
                ModImportPrepareError::UnsupportedArchiveFeature(
                    hmm_ports::UnsupportedArchiveFeature::Encrypted
                )
            ),
            "得到 {error:?}"
        );
        assert_eq!(error.code(), "mod_import_archive_encrypted");
    }

    /// 分卷包落到自己的档位。**打开时就报得出来**，不必解到一半才发现。
    #[test]
    fn a_multi_volume_rar_lands_in_its_own_tier() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = write_rar(
            &temp,
            "part1.rar",
            &Rar5Archive::new(vec![Rar5Entry::file("a.txt", b"vol".to_vec())]).as_volume(),
        );
        let preparer = rar_preparer(&temp, default_extraction_limits());
        let error = prepare_package(&preparer, "vol-1", &path).expect_err("分卷包必须被拒");
        assert!(
            matches!(
                error,
                ModImportPrepareError::UnsupportedArchiveFeature(
                    hmm_ports::UnsupportedArchiveFeature::MultiVolume
                )
            ),
            "得到 {error:?}"
        );
        assert_eq!(error.code(), "mod_import_archive_multi_volume");
    }

    /// **T21-B 完成定义在 rar 上的兑现**：整组共享负测跑在 rar 上，
    /// 断言与期望文本**一条都不为 rar 重写**。要重写就说明外壳没抽干净。
    ///
    /// `LyingDeclaredSize` 一例 rar 按「声明未知」来造：store 结构上没法声明得比实际小
    /// （`extract.cpp:902` 的 `UnstoreFile` 以 `UnpSize` 为界）。该用例的意图是
    /// 「配额不能依赖声明值」，「未知」对这条意图的检验不比「说谎」弱。
    #[test]
    fn the_shared_negative_suite_holds_for_rar_without_rewriting_a_single_assertion() {
        use crate::archive_extraction::shared_negative_suite::{
            expected_message, NegativeCase, ALL_CASES,
        };

        let generous = ArchiveExtractionLimits {
            max_entries: 16,
            max_single_file_bytes: 1024,
            max_total_uncompressed_bytes: 4096,
        };

        for case in ALL_CASES {
            let temp = tempfile::tempdir().expect("temp dir");
            let (archive, limits) = match case {
                NegativeCase::PathEscape => (
                    Rar5Archive::new(vec![Rar5Entry::file("../escape.txt", b"bad".to_vec())]),
                    generous,
                ),
                NegativeCase::AbsolutePath => (
                    Rar5Archive::new(vec![Rar5Entry::file("/abs.txt", b"bad".to_vec())]),
                    generous,
                ),
                NegativeCase::SymlinkEntry => (
                    Rar5Archive::new(vec![Rar5Entry::redirect(
                        "link",
                        FSREDIR_UNIXSYMLINK,
                        "../outside",
                    )]),
                    generous,
                ),
                NegativeCase::CaseCollision => (
                    Rar5Archive::new(vec![
                        Rar5Entry::file("Same.txt", b"a".to_vec()),
                        Rar5Entry::file("same.txt", b"b".to_vec()),
                    ]),
                    generous,
                ),
                NegativeCase::TooManyEntries => (
                    Rar5Archive::new(
                        (0..5)
                            .map(|i| Rar5Entry::file(&format!("f{i}.txt"), b"x".to_vec()))
                            .collect(),
                    ),
                    ArchiveExtractionLimits {
                        max_entries: 3,
                        ..generous
                    },
                ),
                NegativeCase::OversizedSingleFile => (
                    Rar5Archive::new(vec![Rar5Entry::file("big.bin", vec![0; 500])]),
                    ArchiveExtractionLimits {
                        max_single_file_bytes: 64,
                        ..generous
                    },
                ),
                NegativeCase::OversizedTotal => (
                    Rar5Archive::new(vec![
                        Rar5Entry::file("a.bin", vec![0; 40]),
                        Rar5Entry::file("b.bin", vec![0; 40]),
                    ]),
                    ArchiveExtractionLimits {
                        max_total_uncompressed_bytes: 64,
                        ..generous
                    },
                ),
                NegativeCase::LyingDeclaredSize => (
                    // rar 的对应形态：声明「未知」，实际 500 字节。
                    // 预检拿不到可比的声明值，只能由字节流配额兜住。
                    Rar5Archive::new(vec![
                        Rar5Entry::file("liar.bin", vec![0; 500]).with_unknown_unpacked_size()
                    ]),
                    ArchiveExtractionLimits {
                        max_single_file_bytes: 64,
                        ..generous
                    },
                ),
            };

            let path = write_rar(&temp, "case.rar", &archive);
            let preparer = rar_preparer(&temp, limits);
            let Err(error) = prepare_package(&preparer, "case-1", &path) else {
                panic!("{case:?} 必须被拒，却导入成功了");
            };
            let text = format!("{error:#}");
            assert!(
                text.contains(expected_message(*case)),
                "{case:?}: 期望包含 {:?}，实际 {text}",
                expected_message(*case)
            );
        }
    }

    /// **声明「解压大小未知」的条目必须能正常导入。**
    ///
    /// unrar 对 `FHFL_UNPUNKNOWN` 报的是哨兵 `0x7fffffff_7fffffff`。适配器若不把它
    /// 翻译成「未知」，外壳的声明值预检会拿 9.2e18 去比 4 GiB 上限，
    /// **把一个内容只有几十字节的好包直接拒掉**。
    ///
    /// 这条是反向验证逼出来的：原先只有负测覆盖这个哨兵，而负测在「翻译」与
    /// 「不翻译」两种实现下**报的是同一句话**（都命中「超出单文件上限」），
    /// 根本区分不了。能区分的只有正向用例——包是好的，就必须进得来。
    #[test]
    fn an_entry_with_an_unknown_declared_size_still_imports_when_it_fits() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = write_rar(
            &temp,
            "streamed.rar",
            &Rar5Archive::new(vec![
                Rar5Entry::file("streamed.bin", vec![7_u8; 64]).with_unknown_unpacked_size()
            ]),
        );
        let preparer = rar_preparer(&temp, default_extraction_limits());
        let prepared = prepare_package(&preparer, "unk-1", &path)
            .expect("声明未知但实际很小的包必须能导入——拒掉说明哨兵没被翻译成「未知」");
        assert_eq!(
            fs::read(prepared.sandbox_root.join("streamed.bin")).expect("read"),
            vec![7_u8; 64]
        );
    }

    /// 归档里存**字面反斜杠**的条目名不会被当成目录分隔——它是一个文件。
    ///
    /// 这条钉住的是「为什么不做分隔符归一」。unrar 报出的已经是宿主原生形态：
    /// Windows 上字面反斜杠是非法文件名字符、被消毒成下划线；Linux 上原样保留、
    /// `Path` 也把它当普通字符。**两个平台都只产出一个文件**，与归一后的
    /// 「解成两层目录」正相反。
    ///
    /// 断言写成「只有一个条目、且不是目录」而不是写死文件名，
    /// 是因为文件名本身随平台不同（下划线 vs 反斜杠），而**结构**是一致的
    /// ——一致的那个才是该断言的东西。
    #[test]
    fn a_literal_backslash_in_an_entry_name_is_not_a_directory_separator() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = write_rar(
            &temp,
            "backslash.rar",
            &Rar5Archive::new(vec![Rar5Entry::file(r"dir\file.txt", b"flat".to_vec())]),
        );
        let preparer = rar_preparer(&temp, default_extraction_limits());
        let prepared = prepare_package(&preparer, "sep-1", &path).expect("import");

        let entries: Vec<_> = fs::read_dir(&prepared.sandbox_root)
            .expect("read sandbox")
            .map(|entry| entry.expect("entry"))
            .collect();
        assert_eq!(entries.len(), 1, "只该有一个条目");
        assert!(
            entries[0].file_type().expect("file type").is_file(),
            "字面反斜杠不该被解成目录：{:?}",
            entries[0].file_name()
        );
        assert_eq!(fs::read(entries[0].path()).expect("read"), b"flat");
    }

    /// hardlink 条目也要被拒——rar 里它是一等条目类型，不是 zip 那种边缘特性。
    #[test]
    fn rar_hard_link_entries_are_rejected() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = write_rar(
            &temp,
            "hardlink.rar",
            &Rar5Archive::new(vec![Rar5Entry::redirect(
                "link",
                FSREDIR_HARDLINK,
                "../outside",
            )]),
        );
        let preparer = rar_preparer(&temp, default_extraction_limits());
        let error = prepare_package(&preparer, "hl-1", &path).expect_err("hardlink 必须被拒");
        assert!(
            format!("{error:#}").contains("hard link entries are not allowed"),
            "得到 {error:#}"
        );
    }

    /// 暂存目录用完即删——**成功与失败两条路都要清干净**。
    ///
    /// 它开在包沙箱之外，所以清不掉不会污染 Mod 版本；但留着就是每导入一个 rar
    /// 就多一份原始压缩包的副本，磁盘会被慢慢吃掉。
    #[test]
    fn the_rar_staging_directory_is_removed_on_both_success_and_failure() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("sandboxes");

        let good = write_rar(
            &temp,
            "good.rar",
            &Rar5Archive::new(vec![Rar5Entry::file("a.txt", b"ok".to_vec())]),
        );
        let preparer = rar_preparer(&temp, default_extraction_limits());
        prepare_package(&preparer, "ok-1", &good).expect("import");
        assert!(
            !sandbox_root.join("ok-1.staging").exists(),
            "成功后暂存目录必须已删除"
        );

        let bad = write_rar(
            &temp,
            "bad.rar",
            &Rar5Archive::new(vec![Rar5Entry::file("../escape.txt", b"bad".to_vec())]),
        );
        prepare_package(&preparer, "bad-1", &bad).expect_err("逃逸必须被拒");
        assert!(
            !sandbox_root.join("bad-1.staging").exists(),
            "失败后暂存目录同样必须已删除"
        );
        assert!(
            !sandbox_root.join("bad-1").exists(),
            "失败后包沙箱也必须被清掉"
        );
    }

    /// 暂存的原始压缩包**绝不能**混进包沙箱——那会被原样提交成 Mod 版本的内容。
    #[test]
    fn the_staged_archive_never_lands_inside_the_package_sandbox() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = write_rar(
            &temp,
            "mod.rar",
            &Rar5Archive::new(vec![Rar5Entry::file("only.txt", b"content".to_vec())]),
        );
        let preparer = rar_preparer(&temp, default_extraction_limits());
        let prepared = prepare_package(&preparer, "clean-1", &path).expect("import");

        let entries: Vec<_> = fs::read_dir(&prepared.sandbox_root)
            .expect("read sandbox")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        assert_eq!(
            entries.len(),
            1,
            "包沙箱里只该有归档里的内容，实际是 {entries:?}"
        );
        assert_eq!(entries[0], std::ffi::OsStr::new("only.txt"));
    }

    // ---- #348 切片 D：7z ----
    //
    // 与 rar 不同，7z 的语料可以用同一个 crate 的 writer 造（`compress` 特性只在
    // dev-dependencies 里开）——写 7z 没有许可障碍，不必手搓容器格式。

    /// 造一个 7z。`entries` 是 (名字, 内容)；内容为 `None` 表示目录条目。
    fn write_sevenz(
        temp: &tempfile::TempDir,
        name: &str,
        entries: &[(&str, Option<&[u8]>)],
    ) -> PathBuf {
        write_sevenz_with_attributes(
            temp,
            name,
            &entries
                .iter()
                .map(|(name, data)| (*name, *data, 0_u32))
                .collect::<Vec<_>>(),
        )
    }

    /// 同上，但能指定 Windows 属性位——用来造 symlink 条目。
    fn write_sevenz_with_attributes(
        temp: &tempfile::TempDir,
        name: &str,
        entries: &[(&str, Option<&[u8]>, u32)],
    ) -> PathBuf {
        let path = temp.path().join(name);
        let file = File::create(&path).expect("create 7z");
        let mut writer = sevenz_rust2::ArchiveWriter::new(file).expect("7z writer");
        for (entry_name, data, attributes) in entries {
            let mut entry = sevenz_rust2::ArchiveEntry::new_file(entry_name);
            // 属性字段有一个**单独的存在标志**：不置 `has_windows_attributes`，
            // 写出来的包里根本不带属性，于是 symlink 用例会静默变成普通文件而通过
            // ——那是最难发现的一类假绿。第一版就漏了这一行。
            entry.has_windows_attributes = *attributes != 0;
            entry.windows_attributes = *attributes;
            match data {
                Some(bytes) => {
                    writer
                        .push_archive_entry(entry, Some(std::io::Cursor::new(bytes.to_vec())))
                        .expect("push entry");
                }
                None => {
                    entry.is_directory = true;
                    entry.has_stream = false;
                    writer
                        .push_archive_entry::<std::io::Cursor<Vec<u8>>>(entry, None)
                        .expect("push dir");
                }
            }
        }
        writer.finish().expect("finish 7z");
        path
    }

    /// 正向：真实（自造的）7z 能导入，内容逐字节一致。
    #[test]
    fn a_sevenz_archive_now_imports_with_byte_identical_content() {
        let temp = tempfile::tempdir().expect("temp dir");
        let big = vec![0x3c_u8; 9000];
        let path = write_sevenz(
            &temp,
            "mod.7z",
            &[
                ("readme.txt", Some(b"hello from 7z".as_slice())),
                ("nativePC/data.bin", Some(big.as_slice())),
            ],
        );
        let preparer = rar_preparer(&temp, default_extraction_limits());
        let prepared = prepare_package(&preparer, "sz-1", &path).expect("a good 7z must import");

        assert_eq!(
            fs::read(prepared.sandbox_root.join("readme.txt")).expect("read"),
            b"hello from 7z"
        );
        assert_eq!(
            fs::read(prepared.sandbox_root.join("nativePC/data.bin")).expect("read"),
            big
        );
    }

    /// 损坏的 7z 必须留在 `retry-hint`，**不能**被说成「格式不支持」。
    #[test]
    fn a_truncated_sevenz_stays_in_the_retry_hint_tier() {
        let temp = tempfile::tempdir().expect("temp dir");
        let full_path = write_sevenz(&temp, "full.7z", &[("a.txt", Some(b"x".as_slice()))]);
        let full = fs::read(&full_path).expect("read");
        for (label, bytes) in [
            ("signature-only", b"7z\xbc\xaf\x27\x1c".to_vec()),
            ("halved", full[..full.len() / 2].to_vec()),
        ] {
            let error = prepare_bytes(&temp, label, &bytes);
            assert!(
                matches!(error, ModImportPrepareError::Other(_)),
                "{label}: 损坏的 7z 必须留在 retry-hint，得到 {error:?}"
            );
        }
    }

    /// **共享负测整组跑在 7z 上，断言与期望文本一条都不为它重写。**
    ///
    /// 7z 是**推驱动**的（只有 `for_each_entries`），不实现 `ArchiveSource`
    /// 而是在别人的回调里调 `ArchiveGate`。这条用例证明「换了驱动方向，
    /// 门禁仍是同一份」——那正是外壳存在的理由。
    #[test]
    fn the_shared_negative_suite_holds_for_sevenz_without_rewriting_a_single_assertion() {
        use crate::archive_extraction::shared_negative_suite::{
            expected_message, NegativeCase, ALL_CASES,
        };

        let generous = ArchiveExtractionLimits {
            max_entries: 16,
            max_single_file_bytes: 1024,
            max_total_uncompressed_bytes: 4096,
        };
        // 0x8000 = 属性高 16 位是 Unix mode；0xA000 = S_IFLNK。
        let symlink_attributes = 0x8000_u32 | (0xA000_u32 << 16);
        let big = vec![0_u8; 500];
        let forty = vec![0_u8; 40];

        for case in ALL_CASES {
            let temp = tempfile::tempdir().expect("temp dir");
            let (path, limits) = match case {
                NegativeCase::PathEscape => (
                    write_sevenz(
                        &temp,
                        "case.7z",
                        &[("../escape.txt", Some(b"bad".as_slice()))],
                    ),
                    generous,
                ),
                NegativeCase::AbsolutePath => (
                    write_sevenz(&temp, "case.7z", &[("/abs.txt", Some(b"bad".as_slice()))]),
                    generous,
                ),
                NegativeCase::SymlinkEntry => (
                    write_sevenz_with_attributes(
                        &temp,
                        "case.7z",
                        &[("link", Some(b"../outside".as_slice()), symlink_attributes)],
                    ),
                    generous,
                ),
                NegativeCase::CaseCollision => (
                    write_sevenz(
                        &temp,
                        "case.7z",
                        &[
                            ("Same.txt", Some(b"a".as_slice())),
                            ("same.txt", Some(b"b".as_slice())),
                        ],
                    ),
                    generous,
                ),
                NegativeCase::TooManyEntries => {
                    let names: Vec<String> = (0..5).map(|i| format!("f{i}.txt")).collect();
                    let entries: Vec<(&str, Option<&[u8]>)> = names
                        .iter()
                        .map(|name| (name.as_str(), Some(b"x".as_slice())))
                        .collect();
                    (
                        write_sevenz(&temp, "case.7z", &entries),
                        ArchiveExtractionLimits {
                            max_entries: 3,
                            ..generous
                        },
                    )
                }
                NegativeCase::OversizedSingleFile => (
                    write_sevenz(&temp, "case.7z", &[("big.bin", Some(big.as_slice()))]),
                    ArchiveExtractionLimits {
                        max_single_file_bytes: 64,
                        ..generous
                    },
                ),
                NegativeCase::OversizedTotal => (
                    write_sevenz(
                        &temp,
                        "case.7z",
                        &[
                            ("a.bin", Some(forty.as_slice())),
                            ("b.bin", Some(forty.as_slice())),
                        ],
                    ),
                    ArchiveExtractionLimits {
                        max_total_uncompressed_bytes: 64,
                        ..generous
                    },
                ),
                // 7z 的头里带真实解压大小，声明不了假的；与 rar 一样，
                // 该用例的意图（配额不能依赖声明值）由「实际字节超限」来检验。
                NegativeCase::LyingDeclaredSize => (
                    write_sevenz(&temp, "case.7z", &[("liar.bin", Some(big.as_slice()))]),
                    ArchiveExtractionLimits {
                        max_single_file_bytes: 64,
                        ..generous
                    },
                ),
            };

            let preparer = rar_preparer(&temp, limits);
            let Err(error) = prepare_package(&preparer, "case-1", &path) else {
                panic!("{case:?} 必须被拒，却导入成功了");
            };
            let text = format!("{error:#}");
            assert!(
                text.contains(expected_message(*case)),
                "{case:?}: 期望包含 {:?}，实际 {text}",
                expected_message(*case)
            );
        }
    }

    /// 造一个加密的 7z。`encrypt_header` 为真时连头也加密。
    fn write_encrypted_sevenz(
        temp: &tempfile::TempDir,
        name: &str,
        encrypt_header: bool,
    ) -> PathBuf {
        let path = temp.path().join(name);
        let file = File::create(&path).expect("create");
        let mut writer = sevenz_rust2::ArchiveWriter::new(file).expect("writer");
        writer.set_content_methods(vec![
            sevenz_rust2::encoder_options::AesEncoderOptions::new(sevenz_rust2::Password::from(
                "hunter2",
            ))
            .into(),
            sevenz_rust2::EncoderMethod::LZMA2.into(),
        ]);
        if encrypt_header {
            writer.set_encrypt_header(true);
        }
        writer
            .push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("secret.txt"),
                Some(std::io::Cursor::new(b"cipher".to_vec())),
            )
            .expect("push");
        writer.finish().expect("finish");
        path
    }

    /// 加密的 7z 落到自己的档位，而不是一句「请检查压缩包后重试」。
    ///
    /// 内容加密（不加密头）：包能打开、条目名读得出来，解内容时才发现要密码。
    #[test]
    fn an_encrypted_sevenz_lands_in_its_own_tier() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = write_encrypted_sevenz(&temp, "secret.7z", false);
        let preparer = rar_preparer(&temp, default_extraction_limits());
        let error = prepare_package(&preparer, "szenc-1", &path).expect_err("加密包必须被拒");
        assert!(
            matches!(
                error,
                ModImportPrepareError::UnsupportedArchiveFeature(
                    hmm_ports::UnsupportedArchiveFeature::Encrypted
                )
            ),
            "得到 {error:?}"
        );
        assert_eq!(error.code(), "mod_import_archive_encrypted");
    }

    /// 头加密的 7z：连条目名都读不出来，`open` 阶段就该报「要密码」。
    #[test]
    fn a_header_encrypted_sevenz_lands_in_its_own_tier() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = write_encrypted_sevenz(&temp, "header-secret.7z", true);
        let preparer = rar_preparer(&temp, default_extraction_limits());
        let error = prepare_package(&preparer, "szhdr-1", &path).expect_err("头加密包必须被拒");
        assert_eq!(
            error.code(),
            "mod_import_archive_encrypted",
            "得到 {error:?}"
        );
    }

    /// Windows 重解析点位也要被认成 symlink——只认 Unix mode 那一条等于对
    /// Windows 侧打的包不设防。
    #[test]
    fn sevenz_reparse_point_entries_are_rejected_as_symlinks() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = write_sevenz_with_attributes(
            &temp,
            "reparse.7z",
            &[("link", Some(b"..\\outside".as_slice()), 0x0400)],
        );
        let preparer = rar_preparer(&temp, default_extraction_limits());
        let error = prepare_package(&preparer, "rp-1", &path).expect_err("重解析点必须被拒");
        assert!(
            format!("{error:#}").contains("symlink entries are not allowed"),
            "得到 {error:#}"
        );
    }

    /// 7z **不需要落盘暂存**——它直接吃 reader。所以导入 7z 之后
    /// 沙箱根下不该出现任何 `.staging` 目录。
    #[test]
    fn importing_a_sevenz_never_creates_a_staging_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("sandboxes");
        let path = write_sevenz(&temp, "mod.7z", &[("a.txt", Some(b"ok".as_slice()))]);
        let preparer = rar_preparer(&temp, default_extraction_limits());
        prepare_package(&preparer, "nostage-1", &path).expect("import");
        assert!(
            !sandbox_root.join("nostage-1.staging").exists(),
            "7z 走 reader，不该有暂存目录"
        );
    }

    // ---- T22（#366）：拖拽清单的容器层预检 ----

    /// **预检与真实导入必须对每一份语料给出同一个档位。**
    ///
    /// 两者是两个实现（预检只读头、不为 rar 落暂存），难免漂移。这条用例拿同一组语料
    /// 同时跑两边，断言错误码逐份相同——漂了就红。
    ///
    /// 这是清单可信的全部依据：清单说「可导入」而实际导入失败，或者反过来，
    /// 都会让玩家在确认之后才发现被骗。
    #[test]
    fn probe_and_import_agree_on_every_fixture() {
        let temp = tempfile::tempdir().expect("temp dir");

        let good_zip = temp.path().join("good.zip");
        create_zip(&good_zip, &[("a.txt", b"hello".as_slice())]);

        let good_rar = write_rar(
            &temp,
            "good.rar",
            &Rar5Archive::new(vec![Rar5Entry::file("a.txt", b"hi".to_vec())]),
        );
        let encrypted_rar = write_rar(
            &temp,
            "enc.rar",
            &Rar5Archive::new(vec![
                Rar5Entry::file("a.txt", b"x".to_vec()).with_encrypted_flag()
            ]),
        );
        let volume_rar = write_rar(
            &temp,
            "vol.rar",
            &Rar5Archive::new(vec![Rar5Entry::file("a.txt", b"x".to_vec())]).as_volume(),
        );
        let good_sevenz = write_sevenz(&temp, "good.7z", &[("a.txt", Some(b"hi".as_slice()))]);
        let encrypted_sevenz = write_encrypted_sevenz(&temp, "enc.7z", false);
        let header_encrypted_sevenz = write_encrypted_sevenz(&temp, "enc-hdr.7z", true);

        let write_raw = |name: &str, bytes: &[u8]| {
            let path = temp.path().join(name);
            fs::write(&path, bytes).expect("write");
            path
        };
        let exe = write_raw("thing.exe", b"MZ\x90\x00\x03\x00\x00\x00stub");
        let gzip = write_raw("thing.gz", b"\x1f\x8bpayload");
        let noise = write_raw("noise.bin", b"not a container at all");
        let empty = write_raw("empty.bin", b"");

        let cases: &[(&str, &Path)] = &[
            ("good zip", &good_zip),
            ("good rar", &good_rar),
            ("good 7z", &good_sevenz),
            ("encrypted rar", &encrypted_rar),
            ("encrypted 7z", &encrypted_sevenz),
            ("header-encrypted 7z", &header_encrypted_sevenz),
            ("volume rar", &volume_rar),
            ("windows executable", &exe),
            ("gzip stream", &gzip),
            ("noise", &noise),
            ("empty", &empty),
        ];

        for (index, (label, path)) in cases.iter().enumerate() {
            let probe = probe_mod_archive(path);
            let preparer = rar_preparer(&temp, default_extraction_limits());
            let import = prepare_package(&preparer, &format!("agree-{index}"), path);

            let probe_code = probe.as_ref().err().map(|error| error.code());
            let import_code = import.as_ref().err().map(|error| error.code());
            assert_eq!(
                probe_code, import_code,
                "{label}: 预检说 {probe_code:?}，真实导入说 {import_code:?}"
            );
        }
    }

    /// 目录、以及根本不存在的路径，都要落到明确失败而不是 panic。
    #[test]
    fn probing_a_directory_or_a_missing_path_fails_cleanly() {
        let temp = tempfile::tempdir().expect("temp dir");
        let directory = temp.path().join("a-directory");
        fs::create_dir(&directory).expect("mkdir");

        for (label, path) in [
            ("directory", directory),
            ("missing", temp.path().join("nope.zip")),
        ] {
            let error = probe_mod_archive(&path).expect_err("{label} 必须失败");
            assert_eq!(
                error.code(),
                "mod_import_prepare_failed",
                "{label}: 得到 {error:?}"
            );
        }
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
            .expect("analyze metadata")
            .metadata;

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
            .expect("analyze metadata")
            .metadata;

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
            .expect("analyze metadata")
            .metadata;

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
            .expect("analyze metadata")
            .metadata;

        assert_eq!(metadata.display_name.as_deref(), Some("Better Readme Name"));
    }

    #[test]
    fn metadata_analyzer_falls_back_to_readme_when_manifest_is_invalid() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(temp.path().join("manifest.json"), "{not json").expect("write manifest");
        fs::write(temp.path().join("README.md"), "# Readme Name").expect("write readme");

        let metadata = SandboxModPackageMetadataAnalyzer
            .analyze_metadata("pkg-1", temp.path())
            .expect("analyze metadata")
            .metadata;

        assert_eq!(metadata.display_name.as_deref(), Some("Readme Name"));
    }

    #[test]
    fn metadata_analyzer_exposes_manifest_declared_name_separately() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(
            temp.path().join("manifest.json"),
            r#"{"displayName":"Manifest Name"}"#,
        )
        .expect("write manifest");
        fs::write(temp.path().join("README.md"), "# Readme Name").expect("write readme");

        let analysis = SandboxModPackageMetadataAnalyzer
            .analyze_metadata("pkg-1", temp.path())
            .expect("analyze metadata");

        // manifest 声明名单独携带，供上层把压缩包文件名插到 readme 之前。
        assert_eq!(
            analysis.manifest_display_name.as_deref(),
            Some("Manifest Name")
        );
        assert_eq!(
            analysis.metadata.display_name.as_deref(),
            Some("Manifest Name")
        );
    }

    #[test]
    fn metadata_analyzer_leaves_manifest_declared_name_empty_when_only_readme_declares() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(temp.path().join("README.md"), "# Readme Name").expect("write readme");

        let analysis = SandboxModPackageMetadataAnalyzer
            .analyze_metadata("pkg-1", temp.path())
            .expect("analyze metadata");

        assert_eq!(analysis.manifest_display_name, None);
        assert_eq!(
            analysis.metadata.display_name.as_deref(),
            Some("Readme Name")
        );
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

    /// 造一个「声明大小说谎」的 zip：内容是真的，但本地文件头与中央目录里的解压后大小
    /// 被改小。**CRC 不动**，所以除了那两个字段之外归档完全自洽，读侧不会报错（#367）。
    fn create_zip_lying_about_declared_size(
        path: &Path,
        name: &str,
        contents: &[u8],
        declared: u32,
    ) {
        let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .expect("start zip file");
        writer.write_all(contents).expect("write zip contents");
        let mut bytes = writer.finish().expect("finish zip").into_inner();

        assert_eq!(&bytes[0..4], b"PK\x03\x04", "expected a local file header");
        let flags = u16::from_le_bytes([bytes[6], bytes[7]]);
        assert_eq!(
            flags & 0x08,
            0,
            "data descriptor flag set: the size fields are not in the local header, \
             this fixture would not be testing what it claims"
        );
        bytes[22..26].copy_from_slice(&declared.to_le_bytes());

        let central = bytes
            .windows(4)
            .position(|window| window == b"PK\x01\x02")
            .expect("expected a central directory header");
        bytes[central + 24..central + 28].copy_from_slice(&declared.to_le_bytes());

        fs::write(path, bytes).expect("write crafted zip");
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
    ) -> std::result::Result<PreparedModPackage, ModImportPrepareError> {
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
