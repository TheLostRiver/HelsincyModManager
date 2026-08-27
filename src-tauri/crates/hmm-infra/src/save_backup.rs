use crate::controlled_fs::{
    ensure_regular_file_metadata, is_not_found, open_existing_directory_chain,
    open_existing_directory_nofollow, open_or_create_directory_chain,
    open_or_create_directory_nofollow, open_regular_file_nofollow,
};
use crate::save_path::{normalize_save_relative_path, MAX_SAVE_DIRECTORY_COUNT};
use anyhow::{anyhow, bail, Context, Result};
use hmm_core::{
    ProfileDirectoryMode, ProfileDirectorySelection, SaveBackupManifest, SaveBackupManifestFile,
    SaveBackupManifestSource, SaveBackupStatus, SaveBackupSummary,
};
use hmm_ports::{
    SaveBackupDeleteReport, SaveBackupDirectoryLocator, SaveBackupFileDeleteDisposition,
    SaveBackupFileDeleteResult, SaveBackupWriteRequest, SaveBackupWriteResult, SaveBackupWriter,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use zip::write::SimpleFileOptions;

const MAX_FILE_COUNT: usize = 200;
const MAX_SINGLE_FILE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;

pub struct FileSystemSaveBackupWriter {
    save_backup_root: PathBuf,
}

impl FileSystemSaveBackupWriter {
    pub fn new(save_backup_root: PathBuf) -> Self {
        Self { save_backup_root }
    }
}

impl SaveBackupWriter for FileSystemSaveBackupWriter {
    fn write_backup(&self, request: SaveBackupWriteRequest) -> Result<SaveBackupWriteResult> {
        let source_directory = request
            .source_directory
            .as_deref()
            .ok_or_else(|| anyhow!("save source directory is missing"))?;
        let source_root = canonical_existing_directory(source_directory)?;
        let backup_dir = self.backup_directory_for(&request)?;
        reject_containment(&source_root, &backup_dir)?;

        let scanned_files = scan_save_files(&source_root)?;
        if scanned_files.is_empty() {
            bail!("save source directory contains no files");
        }

        fs::create_dir_all(&backup_dir).context("failed to create save backup directory")?;
        let timestamp = timestamp_parts(request.created_at_unix_millis);
        let profile_fragment = safe_id_fragment(request.profile_id.as_str());
        let base_stem = backup_base_stem(
            &timestamp.file_label,
            request.game_id.as_str(),
            &profile_fragment,
            request.trigger.as_str(),
        );
        let base_name = unique_backup_base_name(&backup_dir, &base_stem);
        let archive_file_name = format!("{base_name}.zip");
        let manifest_file_name = format!("{base_name}.manifest.json");
        let archive_path = backup_dir.join(&archive_file_name);
        let manifest_path = backup_dir.join(&manifest_file_name);

        let archived_files = write_zip_archive(&source_root, &archive_path, &scanned_files)?;
        let archive_size_bytes = fs::metadata(&archive_path)
            .context("failed to inspect save backup archive")?
            .len();
        let archive_sha256 = sha256_file(&archive_path)?;
        let backup_id = backup_id_for(
            request.game_id.as_str(),
            request.profile_id.as_str(),
            &timestamp.file_label,
            request.trigger.as_str(),
            &base_stem,
            &base_name,
        );
        let source_path_label = source_path_label(&source_root);
        let source_path_hash = sha256_string(&path_hash_input(&source_root));

        let manifest = SaveBackupManifest {
            schema_version: hmm_core::SAVE_BACKUP_MANIFEST_SCHEMA_VERSION,
            backup_id: backup_id.clone(),
            game_id: request.game_id.clone(),
            profile_id: request.profile_id.clone(),
            trigger: request.trigger,
            created_at_utc: timestamp.utc_label,
            created_at_utc_label: timestamp.utc_display_label,
            archive_file_name: archive_file_name.clone(),
            archive_size_bytes,
            archive_sha256: archive_sha256.clone(),
            source: SaveBackupManifestSource {
                mode: match request.source_directory_selection.mode {
                    ProfileDirectoryMode::Custom => "custom".to_owned(),
                    ProfileDirectoryMode::Default => "default".to_owned(),
                    ProfileDirectoryMode::Unset => "unset".to_owned(),
                },
                path_label: source_path_label.clone(),
                path_hash: source_path_hash.clone(),
            },
            files: archived_files
                .iter()
                .map(|file| SaveBackupManifestFile {
                    relative_path: file.relative_path.clone(),
                    size_bytes: file.size_bytes,
                    sha256: file.sha256.clone(),
                    modified_at_utc: file.modified_at_utc.clone(),
                })
                .collect(),
            notes: request.note.clone(),
        };
        let manifest_json =
            serde_json::to_string_pretty(&manifest).context("failed to serialize manifest")?;
        fs::write(&manifest_path, manifest_json).context("failed to write save backup manifest")?;

        Ok(SaveBackupWriteResult {
            summary: SaveBackupSummary {
                backup_id,
                game_id: request.game_id,
                profile_id: request.profile_id,
                trigger: request.trigger,
                status: SaveBackupStatus::Completed,
                archive_file_name,
                manifest_file_name,
                archive_size_bytes,
                retention_released_bytes: 0,
                archive_sha256,
                file_count: archived_files.len() as u32,
                created_at: request.created_at_unix_millis,
                source_path_label,
                source_path_hash,
                backup_directory: request.backup_directory,
                notes: request.note,
            },
        })
    }

    fn delete_backup_files(
        &self,
        backup_directory: &ProfileDirectorySelection,
        summary: &SaveBackupSummary,
    ) -> Result<()> {
        let backup_dir = self.backup_directory_from_selection(
            backup_directory,
            summary.game_id.as_str(),
            summary.profile_id.as_str(),
        )?;
        let backup_dir = backup_directory_for_trigger(backup_dir, summary.trigger);
        remove_safe_child_file(&backup_dir, &summary.archive_file_name)?;
        remove_safe_child_file(&backup_dir, &summary.manifest_file_name)?;
        Ok(())
    }

    fn delete_backup_files_report(
        &self,
        backup_directory: &ProfileDirectorySelection,
        summary: &SaveBackupSummary,
    ) -> Result<SaveBackupDeleteReport> {
        let directory = match open_retention_backup_directory(
            &self.save_backup_root,
            backup_directory,
            summary.game_id.as_str(),
            summary.profile_id.as_str(),
            summary.trigger,
        ) {
            Ok(directory) => directory,
            Err(_) => {
                return Ok(SaveBackupDeleteReport {
                    archive: blocked_file("save_backup_retention_directory_unavailable"),
                    manifest: blocked_file("save_backup_retention_directory_unavailable"),
                });
            }
        };

        Ok(SaveBackupDeleteReport {
            archive: remove_verified_child_file(&directory, &summary.archive_file_name, true),
            manifest: remove_verified_child_file(&directory, &summary.manifest_file_name, false),
        })
    }
}

fn open_retention_backup_directory(
    save_backup_root: &Path,
    selection: &ProfileDirectorySelection,
    game_id: &str,
    profile_id: &str,
    trigger: hmm_core::SaveBackupTrigger,
) -> Result<cap_std::fs::Dir> {
    // 逐级 nofollow 打开，因此布局必须是 component 列表；列表由
    // managed_backup_profile_layout 单点提供，与写入/打开路径共享同一份真相。
    let (root_path, mut components) =
        managed_backup_profile_layout(save_backup_root, selection, game_id, profile_id)?;
    if trigger == hmm_core::SaveBackupTrigger::PreRestore {
        components.push("pre-restore".to_owned());
    }

    let root = open_existing_directory_nofollow(&root_path, "save backup retention root")?;
    let components = components.iter().map(String::as_str).collect::<Vec<_>>();
    open_existing_directory_chain(&root, &components, "save backup retention directory")
}

impl FileSystemSaveBackupWriter {
    fn backup_directory_for(&self, request: &SaveBackupWriteRequest) -> Result<PathBuf> {
        let backup_dir = self.backup_directory_from_selection(
            &request.backup_directory,
            request.game_id.as_str(),
            request.profile_id.as_str(),
        )?;
        Ok(backup_directory_for_trigger(backup_dir, request.trigger))
    }

    fn backup_directory_from_selection(
        &self,
        selection: &ProfileDirectorySelection,
        game_id: &str,
        profile_id: &str,
    ) -> Result<PathBuf> {
        managed_backup_profile_directory(&self.save_backup_root, selection, game_id, profile_id)
    }
}

pub(crate) fn managed_backup_directory_for_summary(
    save_backup_root: &Path,
    summary: &SaveBackupSummary,
) -> Result<PathBuf> {
    let profile_dir = managed_backup_profile_directory(
        save_backup_root,
        &summary.backup_directory,
        summary.game_id.as_str(),
        summary.profile_id.as_str(),
    )?;
    Ok(backup_directory_for_trigger(profile_dir, summary.trigger))
}

/// 自定义备份根下再套一层的目录名，避免直接把归档散落在玩家选的目录里。
/// 默认根本身已经是这个名字，因此不再重复套一层。
pub(crate) const CUSTOM_BACKUP_DIRECTORY_NAME: &str = "HelsincyModManager";
/// 备份根之下的固定布局，默认模式与自定义模式共用。
pub(crate) const SAVE_BACKUP_SEGMENT: &str = "saves";

fn managed_backup_profile_directory(
    save_backup_root: &Path,
    selection: &ProfileDirectorySelection,
    game_id: &str,
    profile_id: &str,
) -> Result<PathBuf> {
    let (root, components) =
        managed_backup_profile_layout(save_backup_root, selection, game_id, profile_id)?;
    Ok(components
        .iter()
        .fold(root, |path, component| path.join(component)))
}

/// 备份目录布局的单一来源：返回「capability 根 + 根下逐级 component」。
/// join 消费方折叠成完整路径；nofollow 消费方（retention 整理、打开入口的按需创建）
/// 逐级走 component。两类消费方共享同一份布局，不再靠注释约定保持一致。
fn managed_backup_profile_layout(
    save_backup_root: &Path,
    selection: &ProfileDirectorySelection,
    game_id: &str,
    profile_id: &str,
) -> Result<(PathBuf, Vec<String>)> {
    let profile_dir = format!("profile-{}", safe_id_fragment(profile_id));
    // 两种模式布局完全一致，只有根目录不同：默认根由运行时解析到文档目录下，
    // 自定义根是玩家选的目录再套一层应用名。保持同构可以让玩家在两者之间搬迁
    // 备份时不需要重排目录结构。
    Ok(match selection.mode {
        ProfileDirectoryMode::Custom => {
            let root = selection
                .directory
                .as_deref()
                .ok_or_else(|| anyhow!("custom save backup root is missing"))?;
            (
                PathBuf::from(root),
                vec![
                    CUSTOM_BACKUP_DIRECTORY_NAME.to_owned(),
                    SAVE_BACKUP_SEGMENT.to_owned(),
                    game_id.to_owned(),
                    profile_dir,
                ],
            )
        }
        ProfileDirectoryMode::Default | ProfileDirectoryMode::Unset => (
            save_backup_root.to_path_buf(),
            vec![
                SAVE_BACKUP_SEGMENT.to_owned(),
                game_id.to_owned(),
                profile_dir,
            ],
        ),
    })
}

/// 「打开文件夹」入口的备份目录定位器：与写入/整理路径共享同一份托管布局。
pub struct FileSystemSaveBackupDirectoryLocator {
    save_backup_root: PathBuf,
}

impl FileSystemSaveBackupDirectoryLocator {
    pub fn new(save_backup_root: PathBuf) -> Self {
        Self { save_backup_root }
    }
}

impl SaveBackupDirectoryLocator for FileSystemSaveBackupDirectoryLocator {
    fn backup_directory_for_profile(
        &self,
        selection: &ProfileDirectorySelection,
        game_id: &str,
        profile_id: &str,
    ) -> Result<PathBuf> {
        let (root_path, components) =
            managed_backup_profile_layout(&self.save_backup_root, selection, game_id, profile_id)?;
        // 根的归属决定能否补建:默认根(<文档>/HelsincyModManager)是应用自有目录,
        // 缺最后一段可以补;Custom 根是玩家自选目录,必须已存在。根的父级(如文档
        // 目录本身)缺失时直接报错——打开失败优于静默造出一棵树,这与 write_backup
        // 里 create_dir_all 的取舍不同,是有意的。
        let root = match selection.mode {
            ProfileDirectoryMode::Custom => {
                open_existing_directory_nofollow(&root_path, "save backup custom root")?
            }
            ProfileDirectoryMode::Default | ProfileDirectoryMode::Unset => {
                open_or_create_directory_nofollow(&root_path, "save backup root")?
            }
        };
        // 根以下全部是应用自有布局,逐级 nofollow 补建。不用 create_dir_all:
        // 它会跟随中间层的 symlink/junction,与 retention 整理走的强度不一致。
        let component_refs = components.iter().map(String::as_str).collect::<Vec<_>>();
        open_or_create_directory_chain(&root, &component_refs, "save backup profile directory")?;
        Ok(components
            .into_iter()
            .fold(root_path, |path, component| path.join(component)))
    }
}

fn backup_directory_for_trigger(
    backup_dir: PathBuf,
    trigger: hmm_core::SaveBackupTrigger,
) -> PathBuf {
    if trigger == hmm_core::SaveBackupTrigger::PreRestore {
        backup_dir.join("pre-restore")
    } else {
        backup_dir
    }
}

#[derive(Debug, Clone)]
struct ScannedSaveFile {
    absolute_path: PathBuf,
    relative_path: String,
}

#[derive(Debug, Clone)]
struct ArchivedSaveFile {
    relative_path: String,
    size_bytes: u64,
    sha256: String,
    modified_at_utc: Option<String>,
}

#[derive(Debug, Clone)]
struct TimestampParts {
    file_label: String,
    utc_label: String,
    utc_display_label: String,
}

fn canonical_existing_directory(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    let metadata = fs::symlink_metadata(path).context("failed to inspect save directory")?;
    if is_symlink_or_reparse_point(&metadata) {
        bail!("save directory must not be a link or reparse point");
    }
    if !metadata.is_dir() {
        bail!("save directory must be a directory");
    }

    path.canonicalize()
        .context("failed to canonicalize save directory")
}

fn scan_save_files(source_root: &Path) -> Result<Vec<ScannedSaveFile>> {
    let mut files = Vec::new();
    let mut normalized_paths = BTreeSet::new();
    let mut total_bytes = 0_u64;
    let mut directory_count = 0_usize;
    scan_directory(
        source_root,
        source_root,
        &mut files,
        &mut normalized_paths,
        &mut total_bytes,
        &mut directory_count,
    )?;
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

fn scan_directory(
    source_root: &Path,
    current: &Path,
    files: &mut Vec<ScannedSaveFile>,
    normalized_paths: &mut BTreeSet<String>,
    total_bytes: &mut u64,
    directory_count: &mut usize,
) -> Result<()> {
    for entry in fs::read_dir(current).context("failed to read save directory")? {
        let entry = entry.context("failed to read save entry")?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).context("failed to inspect save entry")?;
        if is_symlink_or_reparse_point(&metadata) {
            bail!("save backup does not follow links or reparse points");
        }

        if metadata.is_dir() {
            relative_path_label(source_root, &path)?;
            *directory_count = directory_count
                .checked_add(1)
                .ok_or_else(|| anyhow!("save backup directory count limit exceeded"))?;
            if *directory_count > MAX_SAVE_DIRECTORY_COUNT {
                bail!("save backup directory count limit exceeded");
            }
            scan_directory(
                source_root,
                &path,
                files,
                normalized_paths,
                total_bytes,
                directory_count,
            )?;
            continue;
        }

        if !metadata.is_file() {
            continue;
        }
        if files.len() >= MAX_FILE_COUNT {
            bail!("save backup file count limit exceeded");
        }
        if metadata.len() > MAX_SINGLE_FILE_BYTES {
            bail!("save backup single file size limit exceeded");
        }

        *total_bytes = total_bytes
            .checked_add(metadata.len())
            .ok_or_else(|| anyhow!("save backup size limit exceeded"))?;
        if *total_bytes > MAX_TOTAL_BYTES {
            bail!("save backup total size limit exceeded");
        }

        let relative_path = relative_path_label(source_root, &path)?;
        let collision_key = relative_path.to_lowercase();
        if !normalized_paths.insert(collision_key) {
            bail!("save backup path case collision");
        }

        files.push(ScannedSaveFile {
            absolute_path: path,
            relative_path,
        });
    }

    Ok(())
}

fn relative_path_label(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .context("failed to create save backup relative path")?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(
                part.to_str()
                    .ok_or_else(|| anyhow!("save backup relative path is not UTF-8"))?,
            ),
            _ => bail!("save backup relative path is unsafe"),
        }
    }
    if parts.is_empty() {
        bail!("save backup relative path is empty");
    }
    let relative_path = parts.join("/");
    let normalized = normalize_save_relative_path(&relative_path)
        .ok_or_else(|| anyhow!("save backup relative path is unsafe"))?;
    if normalized != relative_path {
        bail!("save backup relative path is unsafe");
    }
    Ok(normalized)
}

fn write_zip_archive(
    source_root: &Path,
    archive_path: &Path,
    files: &[ScannedSaveFile],
) -> Result<Vec<ArchivedSaveFile>> {
    let archive_file = fs::File::create(archive_path).context("failed to create backup archive")?;
    let mut zip = zip::ZipWriter::new(archive_file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut archived_files = Vec::new();
    let mut total_bytes = 0_u64;

    for file in files {
        revalidate_scanned_file(source_root, file)?;

        let mut input = fs::File::open(&file.absolute_path).context("failed to read save file")?;
        revalidate_scanned_file(source_root, file)?;

        zip.start_file(&file.relative_path, options)
            .context("failed to add save file to archive")?;

        let mut hasher = Sha256::new();
        let mut archived_size_bytes = 0_u64;
        let mut buffer = [0_u8; 8192];
        loop {
            let read = input
                .read(&mut buffer)
                .context("failed to read save file")?;
            if read == 0 {
                break;
            }

            archived_size_bytes = archived_size_bytes
                .checked_add(read as u64)
                .ok_or_else(|| anyhow!("save backup size limit exceeded"))?;
            if archived_size_bytes > MAX_SINGLE_FILE_BYTES {
                bail!("save backup single file size limit exceeded");
            }
            total_bytes = total_bytes
                .checked_add(read as u64)
                .ok_or_else(|| anyhow!("save backup size limit exceeded"))?;
            if total_bytes > MAX_TOTAL_BYTES {
                bail!("save backup total size limit exceeded");
            }

            hasher.update(&buffer[..read]);
            zip.write_all(&buffer[..read])
                .context("failed to write save file to archive")?;
        }

        let metadata = revalidate_scanned_file(source_root, file)?;
        if metadata.len() != archived_size_bytes {
            bail!("save file changed during backup");
        }

        archived_files.push(ArchivedSaveFile {
            relative_path: file.relative_path.clone(),
            size_bytes: archived_size_bytes,
            sha256: sha256_from_hasher(hasher),
            modified_at_utc: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| timestamp_parts(duration.as_millis()).utc_label),
        });
    }

    zip.finish().context("failed to finish backup archive")?;
    Ok(archived_files)
}

fn revalidate_scanned_file(source_root: &Path, file: &ScannedSaveFile) -> Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(&file.absolute_path)
        .context("failed to revalidate save file before archiving")?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_file() {
        bail!("save file changed during backup");
    }
    if metadata.len() > MAX_SINGLE_FILE_BYTES {
        bail!("save backup single file size limit exceeded");
    }

    let canonical = file
        .absolute_path
        .canonicalize()
        .context("failed to canonicalize save file before archiving")?;
    if !canonical.starts_with(source_root) {
        bail!("save file escaped source directory");
    }

    let relative_path = relative_path_label(source_root, &canonical)?;
    if relative_path != file.relative_path {
        bail!("save file changed during backup");
    }

    Ok(metadata)
}

fn is_symlink_or_reparse_point(metadata: &fs::Metadata) -> bool {
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

fn reject_containment(source_root: &Path, backup_dir: &Path) -> Result<()> {
    let normalized_source = normalized_absolute_path(source_root);
    let normalized_backup = normalized_absolute_path(backup_dir);
    if normalized_backup.starts_with(&format!("{normalized_source}/"))
        || normalized_backup == normalized_source
    {
        bail!("save backup destination must not be inside source");
    }
    if normalized_source.starts_with(&format!("{normalized_backup}/"))
        || normalized_source == normalized_backup
    {
        bail!("save backup source must not be inside destination");
    }
    Ok(())
}

fn normalized_absolute_path(path: &Path) -> String {
    let normalized = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/")
        .trim_end_matches('/')
        .to_lowercase();

    normalized
        .strip_prefix("//?/")
        .unwrap_or(&normalized)
        .to_owned()
}

fn unique_backup_base_name(backup_dir: &Path, base: &str) -> String {
    if !backup_dir.join(format!("{base}.zip")).exists()
        && !backup_dir.join(format!("{base}.manifest.json")).exists()
    {
        return base.to_owned();
    }

    for index in 2..100 {
        let candidate = format!("{base}_{index:02}");
        if !backup_dir.join(format!("{candidate}.zip")).exists()
            && !backup_dir
                .join(format!("{candidate}.manifest.json"))
                .exists()
        {
            return candidate;
        }
    }

    format!("{base}_99")
}

fn backup_base_stem(
    timestamp_label: &str,
    game_id: &str,
    profile_fragment: &str,
    trigger: &str,
) -> String {
    format!("{timestamp_label}_{game_id}_profile-{profile_fragment}_{trigger}")
}

fn backup_id_for(
    game_id: &str,
    profile_id: &str,
    timestamp_label: &str,
    trigger: &str,
    base_stem: &str,
    base_name: &str,
) -> String {
    let sequence_suffix = base_name
        .strip_prefix(base_stem)
        .unwrap_or_default()
        .trim_start_matches('_');
    if sequence_suffix.is_empty() {
        format!("{game_id}:{profile_id}:{timestamp_label}:{trigger}")
    } else {
        format!("{game_id}:{profile_id}:{timestamp_label}:{trigger}:{sequence_suffix}")
    }
}

fn safe_id_fragment(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.trim_matches('_').is_empty() {
        "unnamed".to_owned()
    } else {
        out
    }
}

fn source_path_label(source_root: &Path) -> Option<String> {
    source_root
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
}

fn path_hash_input(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).context("failed to open file for hashing")?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .context("failed to read file for hashing")?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(sha256_from_hasher(hasher))
}

fn sha256_string(value: &str) -> String {
    sha256_bytes(value.as_bytes())
}

fn sha256_bytes(value: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value);
    sha256_from_hasher(hasher)
}

fn sha256_from_hasher(hasher: Sha256) -> String {
    let digest = hasher.finalize();
    format!("sha256:{}", hex_digest(&digest))
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn timestamp_parts(timestamp_unix_millis: u128) -> TimestampParts {
    let seconds = (timestamp_unix_millis / 1000) as i64;
    let (year, month, day, hour, minute, second) = unix_seconds_to_utc(seconds);
    TimestampParts {
        file_label: format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}"),
        utc_label: format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"),
        utc_display_label: format!(
            "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC"
        ),
    }
}

fn unix_seconds_to_utc(seconds: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = (seconds_of_day / 3600) as u32;
    let minute = ((seconds_of_day % 3600) / 60) as u32;
    let second = (seconds_of_day % 60) as u32;
    (year, month, day, hour, minute, second)
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

fn remove_safe_child_file(root: &Path, file_name: &str) -> Result<()> {
    if file_name.contains('/') || file_name.contains('\\') || file_name.trim().is_empty() {
        bail!("save backup file name is unsafe");
    }
    let path = root.join(file_name);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("failed to remove save backup file"),
    }
}

fn remove_verified_child_file(
    directory: &cap_std::fs::Dir,
    file_name: &str,
    count_released_bytes: bool,
) -> SaveBackupFileDeleteResult {
    if file_name.contains('/') || file_name.contains('\\') || file_name.trim().is_empty() {
        return blocked_file("save_backup_retention_unsafe_entry");
    }

    let name = std::ffi::OsStr::new(file_name);
    let before = match directory.symlink_metadata(name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return already_missing_file();
        }
        Err(_) => return blocked_file("save_backup_retention_entry_unavailable"),
    };
    if ensure_regular_file_metadata(&before, "save backup retention file").is_err() {
        return blocked_file("save_backup_retention_entry_untrusted");
    }

    let opened = match open_regular_file_nofollow(directory, name, "save backup retention file") {
        Ok(file) => file,
        Err(error) if is_not_found(&error) => return already_missing_file(),
        Err(_) => return blocked_file("save_backup_retention_entry_untrusted"),
    };
    let opened_metadata = match opened.metadata() {
        Ok(metadata) => metadata,
        Err(_) => return blocked_file("save_backup_retention_entry_unavailable"),
    };
    if before.len() != opened_metadata.len()
        || before.modified().ok() != opened_metadata.modified().ok()
    {
        return blocked_file("save_backup_retention_entry_changed");
    }
    let Some(opened_identity) = file_identity(&opened) else {
        return blocked_file("save_backup_retention_entry_unavailable");
    };
    let released_bytes = if count_released_bytes {
        opened_metadata.len()
    } else {
        0
    };
    let current = match open_regular_file_nofollow(directory, name, "save backup retention file") {
        Ok(file) => file,
        Err(error) if is_not_found(&error) => return already_missing_file(),
        Err(_) => return blocked_file("save_backup_retention_entry_untrusted"),
    };
    if file_identity(&current) != Some(opened_identity) {
        return blocked_file("save_backup_retention_entry_changed");
    }
    drop(current);
    drop(opened);

    match directory.remove_file(name) {
        Ok(()) => SaveBackupFileDeleteResult {
            disposition: SaveBackupFileDeleteDisposition::Deleted,
            released_bytes,
            error_code: None,
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => already_missing_file(),
        Err(_) => blocked_file("save_backup_retention_delete_failed"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileIdentity {
    #[cfg(windows)]
    Windows {
        volume_serial_number: u32,
        file_index: u64,
    },
    #[cfg(unix)]
    Unix { dev: u64, ino: u64 },
    #[cfg(not(any(windows, unix)))]
    Unsupported,
}

fn file_identity(file: &cap_std::fs::File) -> Option<FileIdentity> {
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
        Some(FileIdentity::Windows {
            volume_serial_number: information.dwVolumeSerialNumber,
            file_index: (u64::from(information.nFileIndexHigh) << 32)
                | u64::from(information.nFileIndexLow),
        })
    }
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt as _;
        let metadata = file.metadata().ok()?;
        Some(FileIdentity::Unix {
            dev: metadata.dev(),
            ino: metadata.ino(),
        })
    }
    #[cfg(not(any(windows, unix)))]
    {
        Some(FileIdentity::Unsupported)
    }
}

fn already_missing_file() -> SaveBackupFileDeleteResult {
    SaveBackupFileDeleteResult {
        disposition: SaveBackupFileDeleteDisposition::AlreadyMissing,
        released_bytes: 0,
        error_code: None,
    }
}

fn blocked_file(error_code: &str) -> SaveBackupFileDeleteResult {
    SaveBackupFileDeleteResult {
        disposition: SaveBackupFileDeleteDisposition::Blocked,
        released_bytes: 0,
        error_code: Some(error_code.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use zip::ZipArchive;

    #[test]
    fn write_zip_archive_reports_hashes_from_archived_bytes_after_scan_time_mutation() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_root = temp.path().join("save-root");
        fs::create_dir_all(&source_root).expect("create source");
        let source_root = source_root.canonicalize().expect("canonical source");
        let save_file = source_root.join("SAVEDATA1000");
        fs::write(&save_file, b"old-save").expect("write old save");

        let scanned_files = scan_save_files(&source_root).expect("scan save files");
        fs::write(&save_file, b"new-save").expect("mutate save after scan");

        let archive_path = temp.path().join("backup.zip");
        let archived_files =
            write_zip_archive(&source_root, &archive_path, &scanned_files).expect("write archive");

        let mut archive = ZipArchive::new(fs::File::open(&archive_path).expect("open archive"))
            .expect("read archive");
        let mut archived_bytes = Vec::new();
        archive
            .by_name("SAVEDATA1000")
            .expect("zip entry")
            .read_to_end(&mut archived_bytes)
            .expect("read entry");

        assert_eq!(archived_bytes, b"new-save");
        assert_eq!(archived_files.len(), 1);
        assert_eq!(archived_files[0].size_bytes, archived_bytes.len() as u64);
        assert_eq!(archived_files[0].sha256, sha256_bytes(&archived_bytes));
    }

    #[test]
    fn write_zip_archive_revalidates_scanned_file_before_archiving() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_root = temp.path().join("save-root");
        fs::create_dir_all(&source_root).expect("create source");
        let source_root = source_root.canonicalize().expect("canonical source");
        let save_file = source_root.join("SAVEDATA1000");
        fs::write(&save_file, b"save").expect("write save");

        let scanned_files = scan_save_files(&source_root).expect("scan save files");
        fs::remove_file(&save_file).expect("remove scanned file");
        fs::create_dir(&save_file).expect("replace file with directory");

        let archive_path = temp.path().join("backup.zip");
        let error = write_zip_archive(&source_root, &archive_path, &scanned_files)
            .expect_err("changed file type should be rejected");

        assert!(error.to_string().contains("changed"));
    }

    #[test]
    fn scan_save_files_rejects_paths_deeper_than_restore_budget() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_root = temp.path().join("save-root");
        let mut nested = source_root.clone();
        for index in 0..crate::save_path::MAX_SAVE_PATH_COMPONENTS {
            nested.push(format!("d{index}"));
        }
        fs::create_dir_all(&nested).expect("create deep source");
        fs::write(nested.join("save.bin"), b"save").expect("write deep save");
        let source_root = source_root.canonicalize().expect("canonical source");

        let error = scan_save_files(&source_root).expect_err("deep save path must be rejected");

        assert!(error.to_string().contains("relative path is unsafe"));
    }

    #[test]
    fn scan_save_files_rejects_excess_directory_nodes() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_root = temp.path().join("save-root");
        fs::create_dir(&source_root).expect("create source");
        for index in 0..=MAX_SAVE_DIRECTORY_COUNT {
            fs::create_dir(source_root.join(format!("directory-{index}")))
                .expect("create directory node");
        }
        let source_root = source_root.canonicalize().expect("canonical source");

        let error =
            scan_save_files(&source_root).expect_err("directory node budget must be enforced");

        assert!(error.to_string().contains("directory count limit exceeded"));
    }

    fn defaulted_backup_selection() -> ProfileDirectorySelection {
        ProfileDirectorySelection {
            mode: ProfileDirectoryMode::Default,
            status: hmm_core::ProfileDirectoryStatus::Defaulted,
            directory: None,
            path_label: None,
            messages: Vec::new(),
        }
    }

    fn custom_backup_selection(directory: Option<String>) -> ProfileDirectorySelection {
        ProfileDirectorySelection {
            mode: ProfileDirectoryMode::Custom,
            status: hmm_core::ProfileDirectoryStatus::Valid,
            directory,
            path_label: None,
            messages: Vec::new(),
        }
    }

    #[test]
    fn locator_creates_the_managed_default_layout() {
        let temp = tempfile::tempdir().expect("temp dir");
        // 根的最后一段(应用自有目录)刻意不预建,验证 locator 能补出整棵托管子树。
        let root = temp.path().join("HelsincyModManager");
        let locator = FileSystemSaveBackupDirectoryLocator::new(root.clone());

        let directory = locator
            .backup_directory_for_profile(&defaulted_backup_selection(), "mhw", "p1")
            .expect("locate defaulted backup directory");

        assert_eq!(directory, root.join("saves").join("mhw").join("profile-p1"));
        assert!(directory.is_dir());
    }

    #[test]
    fn locator_is_idempotent_for_an_existing_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let locator = FileSystemSaveBackupDirectoryLocator::new(temp.path().join("root"));

        let first = locator
            .backup_directory_for_profile(&defaulted_backup_selection(), "mhw", "p1")
            .expect("first locate");
        let second = locator
            .backup_directory_for_profile(&defaulted_backup_selection(), "mhw", "p1")
            .expect("second locate");

        assert_eq!(first, second);
    }

    #[test]
    fn locator_matches_the_write_backup_layout() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().join("root");
        let selection = defaulted_backup_selection();
        let locator = FileSystemSaveBackupDirectoryLocator::new(root.clone());

        let located = locator
            .backup_directory_for_profile(&selection, "mhw", "p1")
            .expect("locate defaulted backup directory");
        let managed = managed_backup_profile_directory(&root, &selection, "mhw", "p1")
            .expect("managed backup directory");

        // 打开入口与写入路径必须落在同一个目录,布局不允许分叉。
        assert_eq!(located, managed);
    }

    #[test]
    fn locator_refuses_a_custom_selection_without_a_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let locator = FileSystemSaveBackupDirectoryLocator::new(temp.path().join("root"));

        let error = locator
            .backup_directory_for_profile(&custom_backup_selection(None), "mhw", "p1")
            .expect_err("custom selection without a root must be refused");

        assert!(error
            .to_string()
            .contains("custom save backup root is missing"));
    }

    #[test]
    fn locator_refuses_a_missing_custom_root_without_creating_it() {
        let temp = tempfile::tempdir().expect("temp dir");
        let player_root = temp.path().join("player-chosen");
        let locator = FileSystemSaveBackupDirectoryLocator::new(temp.path().join("root"));

        let selection = custom_backup_selection(Some(player_root.to_string_lossy().into_owned()));
        locator
            .backup_directory_for_profile(&selection, "mhw", "p1")
            .expect_err("a missing player-chosen root must not be created");

        // 玩家自选的根目录绝不因打开动作被补建。
        assert!(!player_root.exists());
    }

    #[cfg(unix)]
    #[test]
    fn locator_refuses_to_traverse_a_linked_component() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");
        let elsewhere = temp.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).expect("create link target");
        std::os::unix::fs::symlink(&elsewhere, root.join("saves")).expect("create symlink");
        let locator = FileSystemSaveBackupDirectoryLocator::new(root);

        locator
            .backup_directory_for_profile(&defaulted_backup_selection(), "mhw", "p1")
            .expect_err("a linked middle component must be refused, not followed");
    }
}
