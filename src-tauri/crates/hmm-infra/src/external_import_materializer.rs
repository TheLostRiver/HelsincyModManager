use crate::external_import_scanner::{
    metadata_matches_preview, validate_materialization_content, ContentValidationError,
    ValidatedContentFile,
};
use crate::external_import_source_registry::{
    is_symlink_or_reparse_point, HuntingBoxDirectorySourceRegistry,
    HUNTING_BOX_DIRECTORY_V1_ADAPTER_ID,
};
use anyhow::{anyhow, Context, Result};
use hmm_core::ExternalImportCandidate;
use hmm_ports::{
    CancellationToken, ExternalImportMaterializationOutcome, ExternalImportMaterializeRequest,
    ExternalImportMaterializedPackage, ExternalImportMaterializer, ModImportPackagePrepareRequest,
    ModImportPackagePreparer,
};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

const COPY_BUFFER_BYTES: usize = 64 * 1024;

/// Revalidates a selected external candidate, builds a normalized internal ZIP under app data,
/// and delegates archive extraction to the existing single-package safety boundary.
pub struct HuntingBoxDirectoryMaterializer {
    registry: Arc<HuntingBoxDirectorySourceRegistry>,
    artifact_root: PathBuf,
    package_preparer: Arc<dyn ModImportPackagePreparer>,
}

impl HuntingBoxDirectoryMaterializer {
    pub fn new(
        registry: Arc<HuntingBoxDirectorySourceRegistry>,
        artifact_root: PathBuf,
        package_preparer: Arc<dyn ModImportPackagePreparer>,
    ) -> Self {
        Self {
            registry,
            artifact_root,
            package_preparer,
        }
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
        let Some(selected_paths) =
            find_selected_files_directory(&self.registry, &registration, request.candidate)?
        else {
            return Ok(ExternalImportMaterializationOutcome::SourceChanged);
        };
        if !metadata_matches_preview(
            &selected_paths.info_xml,
            request.candidate.preview_status,
            &request.candidate.metadata_hint,
        ) {
            return Ok(ExternalImportMaterializationOutcome::SourceChanged);
        }

        let content = match validate_materialization_content(
            &selected_paths.files_directory,
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
        let scope_directory = self.artifact_root.join(task_scope_id(request.task_id));
        fs::create_dir_all(&scope_directory)
            .context("failed to create external import materialization scope")?;
        let archive_path = scope_directory.join(format!("{package_id}.zip"));

        let write_result =
            write_normalized_archive(&archive_path, &content.files, request.cancellation_token);
        if write_result.is_err() {
            let _ = fs::remove_file(&archive_path);
        }
        match write_result? {
            ArchiveWriteOutcome::SourceChanged => {
                let _ = fs::remove_file(&archive_path);
                return Ok(ExternalImportMaterializationOutcome::SourceChanged);
            }
            ArchiveWriteOutcome::Written => {}
        }

        let prepared = self
            .package_preparer
            .prepare_package(ModImportPackagePrepareRequest {
                task_id: &package_id,
                archive_path: &archive_path,
                cancellation_token: request.cancellation_token,
            });
        let _ = fs::remove_file(&archive_path);
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

struct SelectedCandidatePaths {
    files_directory: PathBuf,
    info_xml: PathBuf,
}

fn find_selected_files_directory(
    registry: &HuntingBoxDirectorySourceRegistry,
    registration: &crate::external_import_source_registry::RegisteredHuntingBoxSource,
    candidate: &ExternalImportCandidate,
) -> Result<Option<SelectedCandidatePaths>> {
    let root_metadata = match fs::symlink_metadata(&registration.root_directory) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(None),
    };
    if is_symlink_or_reparse_point(&root_metadata) || !root_metadata.is_dir() {
        return Ok(None);
    }

    let entries = match fs::read_dir(&registration.root_directory) {
        Ok(entries) => entries,
        Err(_) => return Ok(None),
    };
    for entry in entries {
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

        let item_path = entry.path();
        let files_directory = item_path.join("files");
        let info_xml = item_path.join("info.xml");
        if is_regular_directory(&item_path)
            && is_regular_directory(&files_directory)
            && is_regular_file(&info_xml)
        {
            return Ok(Some(SelectedCandidatePaths {
                files_directory,
                info_xml,
            }));
        }
        return Ok(None);
    }
    Ok(None)
}

fn is_regular_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| !is_symlink_or_reparse_point(&metadata) && metadata.is_dir())
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| !is_symlink_or_reparse_point(&metadata) && metadata.is_file())
}

enum ArchiveWriteOutcome {
    Written,
    SourceChanged,
}

fn write_normalized_archive(
    archive_path: &Path,
    files: &[ValidatedContentFile],
    cancellation_token: &dyn CancellationToken,
) -> Result<ArchiveWriteOutcome> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(archive_path)
        .context("failed to create external import internal archive")?;
    let mut archive = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for content_file in files {
        ensure_not_cancelled(cancellation_token)?;
        archive
            .start_file(&content_file.archive_path, options)
            .context("failed to create external import internal archive entry")?;
        match copy_verified_file(&mut archive, content_file, cancellation_token)? {
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
    expected: &ValidatedContentFile,
    cancellation_token: &dyn CancellationToken,
) -> Result<ArchiveWriteOutcome> {
    let Some(before) = regular_file_metadata(&expected.source_path, expected.size_bytes) else {
        return Ok(ArchiveWriteOutcome::SourceChanged);
    };
    let before_modified = match before.modified() {
        Ok(value) => value,
        Err(_) => return Ok(ArchiveWriteOutcome::SourceChanged),
    };
    let mut input = match File::open(&expected.source_path) {
        Ok(file) => file,
        Err(_) => return Ok(ArchiveWriteOutcome::SourceChanged),
    };
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

    let Some(after) = regular_file_metadata(&expected.source_path, expected.size_bytes) else {
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

fn regular_file_metadata(path: &Path, expected_size: u64) -> Option<fs::Metadata> {
    fs::symlink_metadata(path).ok().filter(|metadata| {
        !is_symlink_or_reparse_point(metadata)
            && metadata.is_file()
            && metadata.len() == expected_size
    })
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
            app_data.path().join("materialized"),
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
        assert!(locator
            .sandbox_root_for_package(&package.package_id)
            .expect("sandbox path")
            .join("nativepc")
            .join("fixture.bin")
            .is_file());
        assert_eq!(
            fs::read(&source_file).expect("read source fixture after import"),
            original
        );
        assert!(
            !app_data
                .path()
                .join("materialized")
                .to_string_lossy()
                .contains("fixture-source"),
            "materialization scope does not derive from the source path"
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
        let artifact_root = app_data.path().join("materialized");
        let materializer = HuntingBoxDirectoryMaterializer::new(
            Arc::clone(&registry),
            artifact_root.clone(),
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
            app_data.path().join("materialized"),
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

    fn write_fixture(root: &Path) -> PathBuf {
        let candidate = root.join("1001");
        let files = candidate.join("files").join("nativePC");
        fs::create_dir_all(&files).expect("create fixture files");
        let source_file = files.join("fixture.bin");
        fs::write(&source_file, b"fixture source content").expect("write source fixture");
        fs::write(
            candidate.join("info.xml"),
            b"<module><moduleName>Fixture Mod</moduleName></module>",
        )
        .expect("write fixture metadata");
        source_file
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
}
