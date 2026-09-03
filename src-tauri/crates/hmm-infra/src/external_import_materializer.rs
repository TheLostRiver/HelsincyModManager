use crate::controlled_fs::{
    create_new_regular_file, open_child_directory_nofollow, open_existing_directory_nofollow,
    open_or_create_child_directory, open_or_create_directory_chain,
    open_or_create_directory_nofollow, open_regular_file_nofollow, remove_empty_child_directory,
};
use crate::external_import_scanner::{
    metadata_matches_preview, validate_materialization_content, ContentValidationError,
    ValidatedContentFile,
};
use crate::external_import_source_registry::{
    HuntingBoxDirectorySourceRegistry, RegisteredHuntingBoxSource,
    HUNTING_BOX_DIRECTORY_V1_ADAPTER_ID,
};
use anyhow::{anyhow, Context, Result};
use cap_std::fs::{Dir, File, Metadata};
use hmm_core::ExternalImportCandidate;
use hmm_ports::{
    CancellationToken, ExternalImportMaterializationOutcome, ExternalImportMaterializeRequest,
    ExternalImportMaterializedPackage, ExternalImportMaterializer,
    ModImportPackagePrepareReaderRequest, ModImportPackagePreparer,
};
use sha2::{Digest, Sha256};
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

const COPY_BUFFER_BYTES: usize = 64 * 1024;

/// Revalidates a selected external candidate, builds a normalized internal ZIP under app data,
/// and delegates archive extraction to the existing single-package safety boundary.
pub struct HuntingBoxDirectoryMaterializer {
    registry: Arc<HuntingBoxDirectorySourceRegistry>,
    app_data_root: PathBuf,
    package_preparer: Arc<dyn ModImportPackagePreparer>,
}

impl HuntingBoxDirectoryMaterializer {
    pub fn new(
        registry: Arc<HuntingBoxDirectorySourceRegistry>,
        app_data_root: PathBuf,
        package_preparer: Arc<dyn ModImportPackagePreparer>,
    ) -> Self {
        Self {
            registry,
            app_data_root,
            package_preparer,
        }
    }

    fn open_artifact_root(&self) -> Result<Dir> {
        let app_data_root = open_or_create_directory_nofollow(
            &self.app_data_root,
            "external import app data root",
        )?;
        open_or_create_directory_chain(
            &app_data_root,
            &["external-import", "materialized"],
            "external import materialization directory",
        )
    }
}

impl ExternalImportMaterializer for HuntingBoxDirectoryMaterializer {
    fn materialize(
        &self,
        request: ExternalImportMaterializeRequest<'_>,
    ) -> Result<ExternalImportMaterializationOutcome> {
        if request.candidate.batch_id != *request.batch_id
            || request.candidate.content_fingerprint != request.expected_content_fingerprint
        {
            return Ok(ExternalImportMaterializationOutcome::SourceChanged);
        }
        ensure_not_cancelled(request.cancellation_token)?;

        let registration = match self.registry.resolve_directory(request.source_id)? {
            Some(registration) => registration,
            None => return Ok(ExternalImportMaterializationOutcome::SourceChanged),
        };
        if registration.source.adapter_id.as_str() != HUNTING_BOX_DIRECTORY_V1_ADAPTER_ID {
            return Ok(ExternalImportMaterializationOutcome::SourceChanged);
        }
        let Some(mut selected_paths) = find_selected_candidate(
            &self.registry,
            &registration,
            request.candidate,
            request.resource_budget.max_total_candidates,
            request.cancellation_token,
        )?
        else {
            return Ok(ExternalImportMaterializationOutcome::SourceChanged);
        };
        if !metadata_matches_preview(
            &mut selected_paths.info_xml,
            request.candidate.preview_status,
            &request.candidate.metadata_hint,
        ) {
            return Ok(ExternalImportMaterializationOutcome::SourceChanged);
        }

        let content = match validate_materialization_content(
            selected_paths.files_directory,
            request.resource_budget,
            request.cancellation_token,
        ) {
            Ok(content) => content,
            Err(ContentValidationError::Cancelled) => {
                ensure_not_cancelled(request.cancellation_token)?;
                return Err(anyhow!("external import materialization cancelled"));
            }
            Err(ContentValidationError::Rejected) => {
                return Ok(ExternalImportMaterializationOutcome::SourceChanged);
            }
        };
        if content.content_fingerprint != request.expected_content_fingerprint
            || content.usage != request.candidate.resource_usage
        {
            return Ok(ExternalImportMaterializationOutcome::SourceChanged);
        }

        let package_id = format!("external-import-package-{}", Uuid::new_v4());
        let artifact_root = self.open_artifact_root()?;
        let scope_name = task_scope_id(request.task_id);
        let scope_directory = open_or_create_child_directory(
            &artifact_root,
            OsStr::new(&scope_name),
            "external import materialization task scope",
        )?;
        let archive_name = OsString::from(format!("{package_id}.zip"));

        let write_result = write_normalized_archive(
            &scope_directory,
            &archive_name,
            &content.source_directory,
            &content.files,
            request.cancellation_token,
        );
        let write_result = match write_result {
            Ok(result) => result,
            Err(error) => {
                cleanup_materialized_archive(
                    &artifact_root,
                    scope_directory,
                    &scope_name,
                    &archive_name,
                );
                return Err(error);
            }
        };
        match write_result {
            ArchiveWriteOutcome::SourceChanged => {
                cleanup_materialized_archive(
                    &artifact_root,
                    scope_directory,
                    &scope_name,
                    &archive_name,
                );
                return Ok(ExternalImportMaterializationOutcome::SourceChanged);
            }
            ArchiveWriteOutcome::Written => {}
        }

        let prepared = (|| -> Result<_> {
            let mut archive = open_regular_file_nofollow(
                &scope_directory,
                &archive_name,
                "external import materialized archive",
            )?;
            self.package_preparer.prepare_package_from_reader(
                ModImportPackagePrepareReaderRequest {
                    task_id: &package_id,
                    archive: &mut archive,
                    cancellation_token: request.cancellation_token,
                },
            )
        })();
        cleanup_materialized_archive(&artifact_root, scope_directory, &scope_name, &archive_name);
        let prepared = prepared.context("failed to prepare external import internal package")?;
        if prepared.package_id != package_id {
            return Err(anyhow!(
                "external import preparer returned an unexpected package id"
            ));
        }

        Ok(ExternalImportMaterializationOutcome::Materialized(
            ExternalImportMaterializedPackage {
                candidate_id: request.candidate.candidate_id.clone(),
                package_id,
                content_fingerprint: content.content_fingerprint,
                resource_usage: content.usage,
            },
        ))
    }
}

struct SelectedCandidateHandles {
    files_directory: Dir,
    info_xml: File,
}

fn find_selected_candidate(
    registry: &HuntingBoxDirectorySourceRegistry,
    registration: &RegisteredHuntingBoxSource,
    candidate: &ExternalImportCandidate,
    max_candidates: u64,
    cancellation_token: &dyn CancellationToken,
) -> Result<Option<SelectedCandidateHandles>> {
    let root_directory = match open_existing_directory_nofollow(
        &registration.root_directory,
        "external import source root",
    ) {
        Ok(directory) => directory,
        Err(_) => return Ok(None),
    };
    let entries = match root_directory.entries() {
        Ok(entries) => entries,
        Err(_) => return Ok(None),
    };
    let mut entries_seen = 0_u64;
    for entry in entries {
        ensure_not_cancelled(cancellation_token)?;
        entries_seen = match entries_seen.checked_add(1) {
            Some(value) if value <= max_candidates => value,
            _ => return Ok(None),
        };
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => return Ok(None),
        };
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let item_key_hash = registry
            .source_item_key_hash(registration, format!("numeric-directory:{name}").as_bytes());
        if item_key_hash != candidate.source_item_key_hash {
            continue;
        }

        let item_directory = match open_child_directory_nofollow(
            &root_directory,
            &file_name,
            "external import candidate directory",
        ) {
            Ok(directory) => directory,
            Err(_) => return Ok(None),
        };
        let files_directory = match open_child_directory_nofollow(
            &item_directory,
            OsStr::new("files"),
            "external import candidate files directory",
        ) {
            Ok(directory) => directory,
            Err(_) => return Ok(None),
        };
        let info_xml = match open_regular_file_nofollow(
            &item_directory,
            OsStr::new("info.xml"),
            "external import candidate metadata",
        ) {
            Ok(file) => file,
            Err(_) => return Ok(None),
        };
        return Ok(Some(SelectedCandidateHandles {
            files_directory,
            info_xml,
        }));
    }
    Ok(None)
}

enum ArchiveWriteOutcome {
    Written,
    SourceChanged,
}

fn write_normalized_archive(
    scope_directory: &Dir,
    archive_name: &OsStr,
    source_directory: &Dir,
    files: &[ValidatedContentFile],
    cancellation_token: &dyn CancellationToken,
) -> Result<ArchiveWriteOutcome> {
    let file = create_new_regular_file(
        scope_directory,
        archive_name,
        "external import materialized archive",
    )?;
    let mut archive = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for content_file in files {
        ensure_not_cancelled(cancellation_token)?;
        // 条目名用来源目录的原始大小写（#309）。扫描器里 NFKC + 小写的归一化键只服务于
        // 碰撞检测与指纹；拿它落盘会把内容根写成 `nativepc`，下游大小写敏感的安装根匹配
        // 全部落空。下游解压边界（mod_import）自带大小写不敏感的碰撞拒绝，这里不必再归一化。
        let entry_name = content_file
            .archive_entry_name()
            .ok_or_else(|| anyhow!("external import content path is not valid UTF-8"))?;
        archive
            .start_file(&entry_name, options)
            .context("failed to create external import internal archive entry")?;
        match copy_verified_file(
            &mut archive,
            source_directory,
            content_file,
            cancellation_token,
        )? {
            ArchiveWriteOutcome::Written => {}
            ArchiveWriteOutcome::SourceChanged => return Ok(ArchiveWriteOutcome::SourceChanged),
        }
    }

    let file = archive
        .finish()
        .context("failed to finish external import internal archive")?;
    file.sync_all()
        .context("failed to sync external import internal archive")?;
    Ok(ArchiveWriteOutcome::Written)
}

fn copy_verified_file(
    archive: &mut zip::ZipWriter<File>,
    source_directory: &Dir,
    expected: &ValidatedContentFile,
    cancellation_token: &dyn CancellationToken,
) -> Result<ArchiveWriteOutcome> {
    let Some((parent, file_name)) =
        open_source_file_parent(source_directory, &expected.source_segments)
    else {
        return Ok(ArchiveWriteOutcome::SourceChanged);
    };
    let Some(before) = regular_file_metadata(&parent, &file_name, expected.size_bytes) else {
        return Ok(ArchiveWriteOutcome::SourceChanged);
    };
    let before_modified = match before.modified() {
        Ok(value) => value,
        Err(_) => return Ok(ArchiveWriteOutcome::SourceChanged),
    };
    let mut input = match open_regular_file_nofollow(
        &parent,
        &file_name,
        "external import source file during materialization",
    ) {
        Ok(file) => file,
        Err(_) => return Ok(ArchiveWriteOutcome::SourceChanged),
    };
    let opened = match input.metadata() {
        Ok(metadata) if metadata.is_file() && metadata.len() == expected.size_bytes => metadata,
        _ => return Ok(ArchiveWriteOutcome::SourceChanged),
    };
    if opened.modified().ok() != Some(before_modified) {
        return Ok(ArchiveWriteOutcome::SourceChanged);
    }
    let mut hasher = Sha256::new();
    let mut bytes_read = 0_u64;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        ensure_not_cancelled(cancellation_token)?;
        let count = input
            .read(&mut buffer)
            .context("failed to read external import source file")?;
        if count == 0 {
            break;
        }
        bytes_read = bytes_read
            .checked_add(count as u64)
            .ok_or_else(|| anyhow!("external import source file length overflow"))?;
        if bytes_read > expected.size_bytes {
            return Ok(ArchiveWriteOutcome::SourceChanged);
        }
        hasher.update(&buffer[..count]);
        archive
            .write_all(&buffer[..count])
            .context("failed to write external import internal archive entry")?;
    }

    let Some(after) = regular_file_metadata(&parent, &file_name, expected.size_bytes) else {
        return Ok(ArchiveWriteOutcome::SourceChanged);
    };
    if bytes_read != expected.size_bytes
        || after.modified().ok() != Some(before_modified)
        || hasher.finalize().as_slice() != expected.content_hash
    {
        return Ok(ArchiveWriteOutcome::SourceChanged);
    }
    Ok(ArchiveWriteOutcome::Written)
}

fn open_source_file_parent(
    source_directory: &Dir,
    source_segments: &[OsString],
) -> Option<(Dir, OsString)> {
    let (file_name, parent_segments) = source_segments.split_last()?;
    let mut parent = source_directory.try_clone().ok()?;
    for segment in parent_segments {
        parent = open_child_directory_nofollow(
            &parent,
            segment,
            "external import source content directory",
        )
        .ok()?;
    }
    Some((parent, file_name.clone()))
}

fn regular_file_metadata(parent: &Dir, file_name: &OsStr, expected_size: u64) -> Option<Metadata> {
    parent.symlink_metadata(file_name).ok().filter(|metadata| {
        metadata.is_file()
            && metadata.len() == expected_size
            && open_regular_file_nofollow(parent, file_name, "external import source file").is_ok()
    })
}

fn cleanup_materialized_archive(
    artifact_root: &Dir,
    scope_directory: Dir,
    scope_name: &str,
    archive_name: &OsStr,
) {
    let _ = scope_directory.remove_file(archive_name);
    drop(scope_directory);
    let _ = remove_empty_child_directory(
        artifact_root,
        OsStr::new(scope_name),
        "external import materialization task scope",
    );
}

fn ensure_not_cancelled(cancellation_token: &dyn CancellationToken) -> Result<()> {
    if cancellation_token.is_cancelled() {
        anyhow::bail!("external import materialization cancelled");
    }
    Ok(())
}

fn task_scope_id(task_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"hmm.external-import.materialization-task-scope.v1");
    hasher.update(task_id.as_bytes());
    let digest = hasher.finalize();
    let mut value = String::with_capacity(32);
    for byte in digest.iter().take(16) {
        use std::fmt::Write as _;
        let _ = write!(&mut value, "{byte:02x}");
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        HuntingBoxDirectoryScanner, TaskScopedModImportSandboxLocator, ZipModImportPackagePreparer,
    };
    use hmm_core::{
        ExternalImportAdapterId, ExternalImportBatch, ExternalImportBatchId,
        ExternalImportBatchImportStatus, ExternalImportScanStatus,
    };
    use hmm_ports::{
        ExternalImportScanRequest, ExternalImportScanner, ModImportSandboxLocator, NeverCancelled,
    };
    use std::fs;
    use std::path::Path;

    #[test]
    fn materializer_builds_a_sandboxed_package_without_mutating_the_source_fixture() {
        let app_data = tempfile::tempdir().expect("app data");
        let source_root = tempfile::tempdir().expect("source root");
        let source_file = write_fixture(source_root.path());
        let original = fs::read(&source_file).expect("read source fixture before import");
        let registry = Arc::new(
            HuntingBoxDirectorySourceRegistry::new(app_data.path()).expect("source registry"),
        );
        let source = registry
            .register_directory(source_root.path().to_path_buf())
            .expect("register source");
        let batch = batch_for(&source);
        let scanner = HuntingBoxDirectoryScanner::new(Arc::clone(&registry));
        let candidate = scanner
            .scan(ExternalImportScanRequest {
                source: &source,
                batch: &batch,
                resource_budget: &Default::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("scan fixture")
            .candidates
            .into_iter()
            .next()
            .expect("candidate");
        let sandbox_root = app_data.path().join("sandboxes");
        let materializer = HuntingBoxDirectoryMaterializer::new(
            Arc::clone(&registry),
            app_data.path().to_path_buf(),
            Arc::new(ZipModImportPackagePreparer::new(sandbox_root.clone())),
        );

        let outcome = materializer
            .materialize(ExternalImportMaterializeRequest {
                source_id: &source.source_id,
                batch_id: &batch.batch_id,
                candidate: &candidate,
                expected_content_fingerprint: &candidate.content_fingerprint,
                task_id: "mod-import-fixture-task",
                resource_budget: &Default::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("materialize fixture");

        let ExternalImportMaterializationOutcome::Materialized(package) = outcome else {
            panic!("fixture should materialize");
        };
        let locator = TaskScopedModImportSandboxLocator::new(sandbox_root);
        let package_root = locator
            .sandbox_root_for_package(&package.package_id)
            .expect("sandbox path");
        // 逐字比对目录项名字，而不是 `join("nativePC").is_file()`：NTFS 大小写不敏感，
        // 后者在 Windows 上对 `nativepc` 也为真，测不出物化有没有把大小写弄丢（#309）。
        assert_eq!(exact_child_names(&package_root), vec!["nativePC"]);
        assert_eq!(
            exact_child_names(&package_root.join("nativePC")),
            vec!["fixture.bin"]
        );
        assert_eq!(
            fs::read(&source_file).expect("read source fixture after import"),
            original
        );
        assert!(
            !app_data
                .path()
                .join("external-import")
                .join("materialized")
                .to_string_lossy()
                .contains("fixture-source"),
            "materialization scope does not derive from the source path"
        );
        assert!(
            !app_data
                .path()
                .join("external-import")
                .join("materialized")
                .join(task_scope_id("mod-import-fixture-task"))
                .exists(),
            "successful materialization removes its transient task scope"
        );
    }

    /// #309：狩技盒子包的每一级目录名都要原样进沙箱。MHW 适配器的 `allowed_install_roots`
    /// 大小写敏感，内容根一旦变成 `nativepc`，整包既装不上也比不了。
    #[test]
    fn materializer_preserves_source_path_case_at_every_level() {
        let app_data = tempfile::tempdir().expect("app data");
        let source_root = tempfile::tempdir().expect("source root");
        let segments = ["nativePC", "wp", "Swo", "SWO035", "Mod", "Swo035.MOD3"];
        write_fixture_file(source_root.path(), &segments);

        let (_package, package_root) = materialize_fixture(app_data.path(), source_root.path());

        let mut current = package_root;
        for segment in segments {
            assert_eq!(
                exact_child_names(&current),
                vec![segment],
                "sandbox must keep the source spelling of `{segment}`"
            );
            current.push(segment);
        }
        assert_eq!(
            fs::read(&current).expect("read materialized file"),
            b"fixture source content"
        );
    }

    #[test]
    fn materializer_rejects_a_changed_candidate_before_creating_a_package() {
        let app_data = tempfile::tempdir().expect("app data");
        let source_root = tempfile::tempdir().expect("source root");
        let source_file = write_fixture(source_root.path());
        let registry = Arc::new(
            HuntingBoxDirectorySourceRegistry::new(app_data.path()).expect("source registry"),
        );
        let source = registry
            .register_directory(source_root.path().to_path_buf())
            .expect("register source");
        let batch = batch_for(&source);
        let scanner = HuntingBoxDirectoryScanner::new(Arc::clone(&registry));
        let candidate = scanner
            .scan(ExternalImportScanRequest {
                source: &source,
                batch: &batch,
                resource_budget: &Default::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("scan fixture")
            .candidates
            .into_iter()
            .next()
            .expect("candidate");
        fs::write(&source_file, b"changed fixture content").expect("change fixture source");
        let artifact_root = app_data.path().join("external-import").join("materialized");
        let materializer = HuntingBoxDirectoryMaterializer::new(
            Arc::clone(&registry),
            app_data.path().to_path_buf(),
            Arc::new(ZipModImportPackagePreparer::new(
                app_data.path().join("sandboxes"),
            )),
        );

        let outcome = materializer
            .materialize(ExternalImportMaterializeRequest {
                source_id: &source.source_id,
                batch_id: &batch.batch_id,
                candidate: &candidate,
                expected_content_fingerprint: &candidate.content_fingerprint,
                task_id: "mod-import-fixture-task",
                resource_budget: &Default::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("changed source is an expected outcome");

        assert_eq!(outcome, ExternalImportMaterializationOutcome::SourceChanged);
        assert!(!artifact_root.exists() || !artifact_root.join("unexpected.zip").exists());
    }

    #[test]
    fn materializer_rejects_changed_metadata_before_creating_a_package() {
        let app_data = tempfile::tempdir().expect("app data");
        let source_root = tempfile::tempdir().expect("source root");
        write_fixture(source_root.path());
        let registry = Arc::new(
            HuntingBoxDirectorySourceRegistry::new(app_data.path()).expect("source registry"),
        );
        let source = registry
            .register_directory(source_root.path().to_path_buf())
            .expect("register source");
        let batch = batch_for(&source);
        let scanner = HuntingBoxDirectoryScanner::new(Arc::clone(&registry));
        let candidate = scanner
            .scan(ExternalImportScanRequest {
                source: &source,
                batch: &batch,
                resource_budget: &Default::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("scan fixture")
            .candidates
            .into_iter()
            .next()
            .expect("candidate");
        fs::write(
            source_root.path().join("1001").join("info.xml"),
            b"<module><moduleName>Changed Fixture Mod</moduleName></module>",
        )
        .expect("change fixture metadata");
        let materializer = HuntingBoxDirectoryMaterializer::new(
            Arc::clone(&registry),
            app_data.path().to_path_buf(),
            Arc::new(ZipModImportPackagePreparer::new(
                app_data.path().join("sandboxes"),
            )),
        );

        let outcome = materializer
            .materialize(ExternalImportMaterializeRequest {
                source_id: &source.source_id,
                batch_id: &batch.batch_id,
                candidate: &candidate,
                expected_content_fingerprint: &candidate.content_fingerprint,
                task_id: "mod-import-fixture-task",
                resource_budget: &Default::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("changed metadata is an expected outcome");

        assert_eq!(outcome, ExternalImportMaterializationOutcome::SourceChanged);
    }

    #[test]
    fn materializer_rejects_a_files_directory_replaced_with_a_link() {
        let app_data = tempfile::tempdir().expect("app data");
        let source_root = tempfile::tempdir().expect("source root");
        write_fixture(source_root.path());
        let registry = Arc::new(
            HuntingBoxDirectorySourceRegistry::new(app_data.path()).expect("source registry"),
        );
        let source = registry
            .register_directory(source_root.path().to_path_buf())
            .expect("register source");
        let batch = batch_for(&source);
        let scanner = HuntingBoxDirectoryScanner::new(Arc::clone(&registry));
        let candidate = scanner
            .scan(ExternalImportScanRequest {
                source: &source,
                batch: &batch,
                resource_budget: &Default::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("scan fixture")
            .candidates
            .into_iter()
            .next()
            .expect("candidate");
        let outside = tempfile::tempdir().expect("outside root");
        let sentinel = outside.path().join("sentinel.txt");
        fs::write(&sentinel, b"outside remains untouched").expect("write outside sentinel");
        let files_directory = source_root.path().join("1001").join("files");
        fs::remove_dir_all(&files_directory).expect("remove scanned files directory");
        if !try_create_directory_link(outside.path(), &files_directory) {
            return;
        }
        let materializer = HuntingBoxDirectoryMaterializer::new(
            Arc::clone(&registry),
            app_data.path().to_path_buf(),
            Arc::new(ZipModImportPackagePreparer::new(
                app_data.path().join("sandboxes"),
            )),
        );

        let outcome = materializer
            .materialize(ExternalImportMaterializeRequest {
                source_id: &source.source_id,
                batch_id: &batch.batch_id,
                candidate: &candidate,
                expected_content_fingerprint: &candidate.content_fingerprint,
                task_id: "mod-import-linked-files",
                resource_budget: &Default::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("linked source is an expected source change");

        assert_eq!(outcome, ExternalImportMaterializationOutcome::SourceChanged);
        assert_eq!(
            fs::read(&sentinel).expect("read outside sentinel"),
            b"outside remains untouched"
        );
        assert!(!app_data
            .path()
            .join("external-import")
            .join("materialized")
            .join(task_scope_id("mod-import-linked-files"))
            .exists());
        remove_directory_link(&files_directory);
    }

    #[test]
    fn materializer_rejects_metadata_replaced_with_a_link() {
        let app_data = tempfile::tempdir().expect("app data");
        let source_root = tempfile::tempdir().expect("source root");
        write_fixture(source_root.path());
        let registry = Arc::new(
            HuntingBoxDirectorySourceRegistry::new(app_data.path()).expect("source registry"),
        );
        let source = registry
            .register_directory(source_root.path().to_path_buf())
            .expect("register source");
        let batch = batch_for(&source);
        let scanner = HuntingBoxDirectoryScanner::new(Arc::clone(&registry));
        let candidate = scanner
            .scan(ExternalImportScanRequest {
                source: &source,
                batch: &batch,
                resource_budget: &Default::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("scan fixture")
            .candidates
            .into_iter()
            .next()
            .expect("candidate");
        let outside = tempfile::tempdir().expect("outside root");
        let sentinel = outside.path().join("sentinel.txt");
        fs::write(&sentinel, b"outside remains untouched").expect("write outside sentinel");
        let metadata_path = source_root.path().join("1001").join("info.xml");
        fs::remove_file(&metadata_path).expect("remove scanned metadata");
        if !try_create_directory_link(outside.path(), &metadata_path) {
            return;
        }
        let materializer = HuntingBoxDirectoryMaterializer::new(
            Arc::clone(&registry),
            app_data.path().to_path_buf(),
            Arc::new(ZipModImportPackagePreparer::new(
                app_data.path().join("sandboxes"),
            )),
        );

        let outcome = materializer
            .materialize(ExternalImportMaterializeRequest {
                source_id: &source.source_id,
                batch_id: &batch.batch_id,
                candidate: &candidate,
                expected_content_fingerprint: &candidate.content_fingerprint,
                task_id: "mod-import-linked-metadata",
                resource_budget: &Default::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("linked metadata is an expected source change");

        assert_eq!(outcome, ExternalImportMaterializationOutcome::SourceChanged);
        assert_eq!(
            fs::read(&sentinel).expect("read outside sentinel"),
            b"outside remains untouched"
        );
        remove_directory_link(&metadata_path);
    }

    #[test]
    fn materializer_refuses_a_linked_app_data_artifact_root() {
        let registry_data = tempfile::tempdir().expect("registry app data");
        let app_data_parent = tempfile::tempdir().expect("app data parent");
        let outside = tempfile::tempdir().expect("outside root");
        let app_data_link = app_data_parent.path().join("linked-app-data");
        let sentinel = outside.path().join("sentinel.txt");
        fs::write(&sentinel, b"outside remains untouched").expect("write outside sentinel");
        if !try_create_directory_link(outside.path(), &app_data_link) {
            return;
        }
        let source_root = tempfile::tempdir().expect("source root");
        write_fixture(source_root.path());
        let registry = Arc::new(
            HuntingBoxDirectorySourceRegistry::new(registry_data.path()).expect("source registry"),
        );
        let source = registry
            .register_directory(source_root.path().to_path_buf())
            .expect("register source");
        let batch = batch_for(&source);
        let scanner = HuntingBoxDirectoryScanner::new(Arc::clone(&registry));
        let candidate = scanner
            .scan(ExternalImportScanRequest {
                source: &source,
                batch: &batch,
                resource_budget: &Default::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("scan fixture")
            .candidates
            .into_iter()
            .next()
            .expect("candidate");
        let materializer = HuntingBoxDirectoryMaterializer::new(
            registry,
            app_data_link.clone(),
            Arc::new(ZipModImportPackagePreparer::new(
                registry_data.path().join("sandboxes"),
            )),
        );

        let error = materializer
            .materialize(ExternalImportMaterializeRequest {
                source_id: &source.source_id,
                batch_id: &batch.batch_id,
                candidate: &candidate,
                expected_content_fingerprint: &candidate.content_fingerprint,
                task_id: "mod-import-linked-app-data",
                resource_budget: &Default::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect_err("linked app data root must be rejected");

        assert!(!error
            .to_string()
            .contains(outside.path().to_string_lossy().as_ref()));
        assert_eq!(
            fs::read(&sentinel).expect("read outside sentinel"),
            b"outside remains untouched"
        );
        assert!(!outside
            .path()
            .join("external-import")
            .join("materialized")
            .exists());
        remove_directory_link(&app_data_link);
    }

    fn write_fixture(root: &Path) -> PathBuf {
        write_fixture_file(root, &["nativePC", "fixture.bin"])
    }

    /// 在候选 `1001/files/` 下按给定路径段写一个文件，段的大小写原样落盘。
    fn write_fixture_file(root: &Path, relative_segments: &[&str]) -> PathBuf {
        let candidate = root.join("1001");
        let mut source_file = candidate.join("files");
        for segment in relative_segments {
            source_file.push(segment);
        }
        fs::create_dir_all(source_file.parent().expect("fixture file parent"))
            .expect("create fixture files");
        fs::write(&source_file, b"fixture source content").expect("write source fixture");
        fs::write(
            candidate.join("info.xml"),
            b"<module><moduleName>Fixture Mod</moduleName></module>",
        )
        .expect("write fixture metadata");
        source_file
    }

    /// 目录项名字的**逐字**列表（已排序）。`Path::join(..).exists()` 在 NTFS 上大小写不敏感，
    /// 断言大小写是否保留只能靠枚举目录项比对名字。
    fn exact_child_names(directory: &Path) -> Vec<String> {
        let mut names = fs::read_dir(directory)
            .expect("read sandbox directory")
            .map(|entry| {
                entry
                    .expect("sandbox directory entry")
                    .file_name()
                    .to_str()
                    .expect("utf-8 sandbox entry name")
                    .to_owned()
            })
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    fn materialize_fixture(
        app_data: &Path,
        source_root: &Path,
    ) -> (ExternalImportMaterializedPackage, PathBuf) {
        let registry =
            Arc::new(HuntingBoxDirectorySourceRegistry::new(app_data).expect("source registry"));
        let source = registry
            .register_directory(source_root.to_path_buf())
            .expect("register source");
        let batch = batch_for(&source);
        let scanner = HuntingBoxDirectoryScanner::new(Arc::clone(&registry));
        let candidate = scanner
            .scan(ExternalImportScanRequest {
                source: &source,
                batch: &batch,
                resource_budget: &Default::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("scan fixture")
            .candidates
            .into_iter()
            .next()
            .expect("candidate");
        let sandbox_root = app_data.join("sandboxes");
        let materializer = HuntingBoxDirectoryMaterializer::new(
            Arc::clone(&registry),
            app_data.to_path_buf(),
            Arc::new(ZipModImportPackagePreparer::new(sandbox_root.clone())),
        );
        let outcome = materializer
            .materialize(ExternalImportMaterializeRequest {
                source_id: &source.source_id,
                batch_id: &batch.batch_id,
                candidate: &candidate,
                expected_content_fingerprint: &candidate.content_fingerprint,
                task_id: "mod-import-case-task",
                resource_budget: &Default::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("materialize fixture");
        let ExternalImportMaterializationOutcome::Materialized(package) = outcome else {
            panic!("fixture should materialize");
        };
        let package_root = TaskScopedModImportSandboxLocator::new(sandbox_root)
            .sandbox_root_for_package(&package.package_id)
            .expect("sandbox path");
        (package, package_root)
    }

    fn batch_for(source: &hmm_core::ExternalImportSource) -> ExternalImportBatch {
        ExternalImportBatch {
            batch_id: ExternalImportBatchId::new("external-import-batch-fixture"),
            source_id: Some(source.source_id.clone()),
            adapter_id: ExternalImportAdapterId::new(HUNTING_BOX_DIRECTORY_V1_ADAPTER_ID),
            source_fingerprint: "private-fingerprint".to_owned(),
            scan_status: ExternalImportScanStatus::Completed,
            import_status: ExternalImportBatchImportStatus::Pending,
            created_at_unix_millis: 1,
        }
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
}
