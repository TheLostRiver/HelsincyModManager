use crate::mod_import_diagnostics::{
    preview_image_diagnostics_from_stored, PreviewImageDiagnosticsSummary,
};
use hmm_core::{CategoryLabel, PreviewImageRejectionReason};
use hmm_ports::{
    AppSettingsRepository, CancellationToken, CategoryRepository,
    ModImportPackagePrepareRequest,
    ModImportPackagePreparer, ModImportResultRepository, ModMetadataRepository,
    ModPackageMetadataAnalyzer,
    NeverCancelled, PreviewImageProcessingResult, StoredImportPreviewImage,
    StoredModImportAnalysis, StoredModPackageMetadata, ThumbnailCacheMaintenance,
    ThumbnailCacheMaintenanceRequest, ThumbnailRef, ThumbnailStore,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const MOD_IMPORT_UNPACK_STARTED_PHASE: &str = "mod_import.unpack.started";
const MOD_IMPORT_UNPACK_COMPLETED_PHASE: &str = "mod_import.unpack.completed";
const MOD_IMPORT_UNPACK_FAILED_PHASE: &str = "mod_import.unpack.failed";
const MOD_IMPORT_PREVIEW_IMAGE_PROCESSING_PHASE: &str = "mod_import.preview_image.processing";
const MOD_IMPORT_PREVIEW_IMAGE_FALLBACK_PHASE: &str = "mod_import.preview_image.fallback";
const MOD_IMPORT_PREPARE_COMPLETED_PHASE: &str = "mod_import.prepare.completed";
const MOD_IMPORT_PREPARE_FAILED_ERROR: &str = "mod_import_prepare_failed";
pub const DEFAULT_THUMBNAIL_CACHE_MAX_BYTES: u64 = 512 * 1024 * 1024;
pub const DEFAULT_THUMBNAIL_CACHE_MAINTENANCE_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

pub trait ImportPreviewImageProcessor: Send + Sync {
    fn process_package_preview(
        &self,
        task_id: &str,
        package_id: &str,
        sandbox_root: &Path,
    ) -> anyhow::Result<PreviewImageProcessingResult>;

    fn process_package_preview_with_cancellation(
        &self,
        task_id: &str,
        package_id: &str,
        sandbox_root: &Path,
        _cancellation_token: &dyn CancellationToken,
    ) -> anyhow::Result<PreviewImageProcessingResult> {
        self.process_package_preview(task_id, package_id, sandbox_root)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModImportAnalysisRequest {
    pub task_id: String,
    pub package_id: String,
    pub sandbox_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModImportAnalysisResult {
    pub task_id: String,
    pub package_id: String,
    pub display_name: String,
    pub metadata: hmm_ports::ModPackageMetadata,
    pub preview_image: ImportPreviewImage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModImportPrepareRequest {
    pub task_id: String,
    pub archive_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModImportPrepareResult {
    pub analysis: ModImportAnalysisResult,
    pub events: Vec<crate::TaskProgressEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModImportTaskRunError {
    pub events: Vec<crate::TaskProgressEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryItem {
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    pub version_label: Option<String>,
    pub status: ModLibraryStatus,
    pub size_label: String,
    pub category_labels: Vec<CategoryLabel>,
    pub preview_image: ImportPreviewImage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDetail {
    pub id: String,
    pub name: String,
    pub package_id: String,
    pub metadata: ModPackageMetadataSummary,
    pub description: Option<String>,
    pub nexus_mod_id: Option<u64>,
    pub preview_image: ImportPreviewImage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModPackageMetadataSummary {
    pub version: Option<String>,
    pub author: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModLibraryStatus {
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportPreviewImage {
    Thumbnail {
        thumbnail_url: String,
        width: u32,
        height: u32,
        content_hash: String,
        variant: String,
    },
    Fallback {
        reason: PreviewImageRejectionReason,
    },
}

pub struct ModImportTaskRunner {
    task_manager: Arc<crate::TaskManager>,
    prepare_service: Arc<ModImportPrepareService>,
    result_repository: Arc<dyn ModImportResultRepository>,
    thumbnail_cache_maintenance: Option<Arc<dyn ThumbnailCacheMaintenance>>,
    app_settings_repository: Option<Arc<dyn AppSettingsRepository>>,
}

#[derive(Clone)]
pub struct ThumbnailCacheMaintenanceScheduler {
    runner: Arc<ModImportTaskRunner>,
    interval: Duration,
}

impl ThumbnailCacheMaintenanceScheduler {
    pub fn new(runner: Arc<ModImportTaskRunner>, interval: Duration) -> Self {
        Self { runner, interval }
    }

    pub fn run_one_cycle_with_sleep(&self, sleep: impl FnOnce(Duration)) {
        sleep(self.interval);
        self.runner.maintain_thumbnail_cache();
    }

    pub fn run_forever(self) {
        loop {
            self.run_one_cycle_with_sleep(std::thread::sleep);
        }
    }
}

impl ModImportTaskRunner {
    pub fn new(
        task_manager: Arc<crate::TaskManager>,
        prepare_service: Arc<ModImportPrepareService>,
        result_repository: Arc<dyn ModImportResultRepository>,
    ) -> Self {
        Self {
            task_manager,
            prepare_service,
            result_repository,
            thumbnail_cache_maintenance: None,
            app_settings_repository: None,
        }
    }

    pub fn with_thumbnail_cache_maintenance(
        mut self,
        thumbnail_cache_maintenance: Arc<dyn ThumbnailCacheMaintenance>,
    ) -> Self {
        self.thumbnail_cache_maintenance = Some(thumbnail_cache_maintenance);
        self
    }

    pub fn with_app_settings_repository(
        mut self,
        app_settings_repository: Arc<dyn AppSettingsRepository>,
    ) -> Self {
        self.app_settings_repository = Some(app_settings_repository);
        self
    }

    pub fn run_prepare_task(
        &self,
        task_id: &str,
        archive_path: PathBuf,
    ) -> Result<Vec<crate::TaskProgressEvent>, ModImportTaskRunError> {
        if self.task_manager.start_task(task_id).is_err() {
            return Err(ModImportTaskRunError { events: Vec::new() });
        }

        let request = ModImportPrepareRequest {
            task_id: task_id.to_owned(),
            archive_path,
        };
        let cancellation_token = TaskManagerCancellationToken {
            task_manager: Arc::clone(&self.task_manager),
            task_id: task_id.to_owned(),
        };
        let mut events = match self
            .prepare_service
            .prepare_import_with_cancellation(request, &cancellation_token)
        {
            Ok(result) => {
                if self.is_task_cancelled(task_id) {
                    self.maintain_thumbnail_cache();
                    return Err(ModImportTaskRunError { events: Vec::new() });
                }

                if self
                    .result_repository
                    .save_analysis(&stored_analysis_from_result(&result.analysis))
                    .is_err()
                {
                    let _ = self.task_manager.fail_task(task_id);
                    return Err(ModImportTaskRunError {
                        events: vec![failed_event(task_id)],
                    });
                }

                self.maintain_thumbnail_cache();

                result.events
            }
            Err(_) => {
                if self.is_task_cancelled(task_id) {
                    self.maintain_thumbnail_cache();
                    return Err(ModImportTaskRunError { events: Vec::new() });
                }

                let _ = self.task_manager.fail_task(task_id);
                return Err(ModImportTaskRunError {
                    events: vec![failed_event(task_id)],
                });
            }
        };

        match self.task_manager.complete_task(task_id) {
            Ok(task) => {
                events.push(crate::TaskProgressEvent::new(
                    task.task_id,
                    task.kind,
                    task.status,
                    MOD_IMPORT_PREPARE_COMPLETED_PHASE,
                ));
                Ok(events)
            }
            Err(_) => {
                let _ = self.task_manager.fail_task(task_id);
                Err(ModImportTaskRunError {
                    events: vec![failed_event(task_id)],
                })
            }
        }
    }

    pub fn maintain_thumbnail_cache_now(&self) {
        self.maintain_thumbnail_cache();
    }

    fn is_task_cancelled(&self, task_id: &str) -> bool {
        self.task_manager.task_status(task_id) == Some(crate::TaskStatus::Cancelled)
    }

    fn maintain_thumbnail_cache(&self) {
        let Some(thumbnail_cache_maintenance) = &self.thumbnail_cache_maintenance else {
            return;
        };

        let Ok(records) = self.result_repository.list_analysis() else {
            return;
        };

        let retained = retained_thumbnail_refs(&records);
        let settings = self
            .app_settings_repository
            .as_ref()
            .and_then(|repository| repository.load_settings().ok());
        let max_bytes = Some(
            settings
                .as_ref()
                .and_then(|settings| settings.thumbnail_cache_max_bytes)
                .unwrap_or(DEFAULT_THUMBNAIL_CACHE_MAX_BYTES),
        );
        let max_age = settings
            .as_ref()
            .and_then(|settings| settings.thumbnail_cache_max_age_days)
            .map(|days| Duration::from_secs(u64::from(days) * 24 * 60 * 60));
        let _ = thumbnail_cache_maintenance.maintain_thumbnail_cache(
            ThumbnailCacheMaintenanceRequest {
                retained: &retained,
                max_bytes,
                max_age,
            },
        );
    }
}

pub struct ModLibraryService {
    result_repository: Arc<dyn ModImportResultRepository>,
    metadata_repository: Arc<dyn ModMetadataRepository>,
    category_repository: Arc<dyn CategoryRepository>,
}

impl ModLibraryService {
    pub fn new(
        result_repository: Arc<dyn ModImportResultRepository>,
        metadata_repository: Arc<dyn ModMetadataRepository>,
        category_repository: Arc<dyn CategoryRepository>,
    ) -> Self {
        Self {
            result_repository,
            metadata_repository,
            category_repository,
        }
    }

    pub fn get_mod_library(&self) -> anyhow::Result<Vec<ModLibraryItem>> {
        let records = self.result_repository.list_analysis()?;
        let overlays = self.metadata_repository.list_all()?;
        let overlay_map: std::collections::HashMap<_, _> =
            overlays.iter().map(|o| (o.mod_id.as_str(), o)).collect();
        let pairs = self.category_repository.list_mod_category_pairs()?;
        let mut user_cat_map = crate::category::build_user_category_map(pairs);
        Ok(records
            .into_iter()
            .map(|record| {
                let mod_id = record.mod_id.clone();
                let overlay = overlay_map.get(record.mod_id.as_str()).copied();
                let mut item = library_item_from_stored(record);
                if let Some(o) = overlay {
                    if let Some(name) = &o.display_name { item.name = name.clone(); }
                    if let Some(author) = &o.author { item.author = Some(author.clone()); }
                    if let Some(v) = &o.version { item.version_label = Some(format_version_label(v)); }
                }
                if let Some(user_cats) = user_cat_map.remove(&mod_id) {
                    let import_labels = std::mem::take(&mut item.category_labels);
                    item.category_labels = crate::category::merge_category_labels(user_cats, import_labels);
                }
                item
            })
            .collect())
    }

    pub fn get_mod_detail(&self, mod_id: &str) -> anyhow::Result<Option<ModDetail>> {
        let record = self.result_repository.get_analysis(mod_id)?;
        match record {
            Some(record) => {
                let mut detail = detail_from_stored(record);
                if let Some(o) = self.metadata_repository.get(mod_id)? {
                    if let Some(name) = &o.display_name { detail.name = name.clone(); }
                    if let Some(author) = &o.author { detail.metadata.author = Some(author.clone()); }
                    if let Some(version) = &o.version { detail.metadata.version = Some(version.clone()); }
                    detail.description = o.description.clone();
                    detail.nexus_mod_id = o.nexus_mod_id;
                }
                Ok(Some(detail))
            }
            None => Ok(None),
        }
    }

    pub fn get_preview_image_diagnostics(&self) -> anyhow::Result<PreviewImageDiagnosticsSummary> {
        let records = self.result_repository.list_analysis()?;
        Ok(preview_image_diagnostics_from_stored(&records))
    }
}

pub struct ModImportPrepareService {
    package_preparer: Box<dyn ModImportPackagePreparer>,
    analysis_service: ModImportAnalysisService,
}

fn stored_analysis_from_result(result: &ModImportAnalysisResult) -> StoredModImportAnalysis {
    StoredModImportAnalysis {
        mod_id: result.package_id.clone(),
        task_id: result.task_id.clone(),
        package_id: result.package_id.clone(),
        display_name: result.display_name.clone(),
        metadata: stored_metadata_from_package_metadata(&result.metadata),
        preview_image: stored_preview_from_import(&result.preview_image),
    }
}

fn stored_metadata_from_package_metadata(
    metadata: &hmm_ports::ModPackageMetadata,
) -> StoredModPackageMetadata {
    StoredModPackageMetadata {
        version: metadata.version.clone(),
        author: metadata.author.clone(),
        category: metadata.category.clone(),
        tags: metadata.tags.clone(),
        dependencies: metadata.dependencies.clone(),
    }
}

fn stored_preview_from_import(preview_image: &ImportPreviewImage) -> StoredImportPreviewImage {
    match preview_image {
        ImportPreviewImage::Thumbnail {
            thumbnail_url,
            width,
            height,
            content_hash,
            variant,
        } => StoredImportPreviewImage::Thumbnail {
            thumbnail_url: thumbnail_url.clone(),
            width: *width,
            height: *height,
            content_hash: content_hash.clone(),
            variant: variant.clone(),
        },
        ImportPreviewImage::Fallback { reason } => {
            StoredImportPreviewImage::Fallback { reason: *reason }
        }
    }
}

fn import_preview_from_stored(preview_image: StoredImportPreviewImage) -> ImportPreviewImage {
    match preview_image {
        StoredImportPreviewImage::Thumbnail {
            thumbnail_url,
            width,
            height,
            content_hash,
            variant,
        } => ImportPreviewImage::Thumbnail {
            thumbnail_url,
            width,
            height,
            content_hash,
            variant,
        },
        StoredImportPreviewImage::Fallback { reason } => ImportPreviewImage::Fallback { reason },
    }
}

fn retained_thumbnail_refs(records: &[StoredModImportAnalysis]) -> Vec<ThumbnailRef> {
    records
        .iter()
        .filter_map(|record| match &record.preview_image {
            StoredImportPreviewImage::Thumbnail {
                content_hash,
                variant,
                ..
            } => Some(ThumbnailRef {
                package_id: record.package_id.clone(),
                content_hash: content_hash.clone(),
                variant: variant.clone(),
            }),
            StoredImportPreviewImage::Fallback { .. } => None,
        })
        .collect()
}

fn library_item_from_stored(record: StoredModImportAnalysis) -> ModLibraryItem {
    let category_labels = crate::category::category_labels_from_metadata(&record.metadata);
    let author = non_empty_metadata_value(&record.metadata.author);
    let version_label = version_label_from_metadata(&record.metadata);

    ModLibraryItem {
        id: record.mod_id,
        name: record.display_name,
        author,
        version_label,
        status: ModLibraryStatus::Disabled,
        size_label: "导入完成".to_owned(),
        category_labels,
        preview_image: import_preview_from_stored(record.preview_image),
    }
}

fn detail_from_stored(record: StoredModImportAnalysis) -> ModDetail {
    let metadata = metadata_summary_from_stored(&record.metadata);

    ModDetail {
        id: record.mod_id,
        name: record.display_name,
        package_id: record.package_id,
        metadata,
        description: None,
        nexus_mod_id: None,
        preview_image: import_preview_from_stored(record.preview_image),
    }
}

fn format_version_label(version: &str) -> String {
    if version.starts_with('v') || version.starts_with('V') {
        version.to_owned()
    } else {
        format!("v{version}")
    }
}

fn metadata_summary_from_stored(metadata: &StoredModPackageMetadata) -> ModPackageMetadataSummary {
    ModPackageMetadataSummary {
        version: non_empty_metadata_value(&metadata.version),
        author: non_empty_metadata_value(&metadata.author),
        category: non_empty_metadata_value(&metadata.category),
        tags: unique_non_empty_metadata_values(&metadata.tags),
        dependencies: unique_non_empty_metadata_values(&metadata.dependencies),
    }
}

fn version_label_from_metadata(metadata: &StoredModPackageMetadata) -> Option<String> {
    non_empty_metadata_value(&metadata.version).map(|v| format_version_label(&v))
}

fn non_empty_metadata_value(value: &Option<String>) -> Option<String> {
    value.as_ref().filter(|value| !value.is_empty()).cloned()
}

fn unique_non_empty_metadata_values(values: &[String]) -> Vec<String> {
    let mut unique = Vec::new();
    for value in values {
        if !value.is_empty() && !unique.iter().any(|existing| existing == value) {
            unique.push(value.clone());
        }
    }
    unique
}

impl ModImportPrepareService {
    pub fn new(
        package_preparer: Box<dyn ModImportPackagePreparer>,
        analysis_service: ModImportAnalysisService,
    ) -> Self {
        Self {
            package_preparer,
            analysis_service,
        }
    }

    pub fn prepare_import(
        &self,
        request: ModImportPrepareRequest,
    ) -> anyhow::Result<ModImportPrepareResult> {
        self.prepare_import_with_cancellation(request, &NeverCancelled)
    }

    pub fn prepare_import_with_cancellation(
        &self,
        request: ModImportPrepareRequest,
        cancellation_token: &dyn CancellationToken,
    ) -> anyhow::Result<ModImportPrepareResult> {
        let mut events = Vec::new();
        events.push(running_event(
            &request.task_id,
            MOD_IMPORT_UNPACK_STARTED_PHASE,
        ));

        let prepared_package =
            self.package_preparer
                .prepare_package(ModImportPackagePrepareRequest {
                    task_id: &request.task_id,
                    archive_path: &request.archive_path,
                    cancellation_token,
                })?;
        events.push(running_event(
            &request.task_id,
            MOD_IMPORT_UNPACK_COMPLETED_PHASE,
        ));
        ensure_not_cancelled(cancellation_token)?;
        events.push(running_event(
            &request.task_id,
            MOD_IMPORT_PREVIEW_IMAGE_PROCESSING_PHASE,
        ));

        let analysis = self.analysis_service.analyze_sandbox_with_cancellation(
            ModImportAnalysisRequest {
                task_id: request.task_id.clone(),
                package_id: prepared_package.package_id,
                sandbox_root: prepared_package.sandbox_root,
            },
            cancellation_token,
        )?;

        if let ImportPreviewImage::Fallback { reason } = &analysis.preview_image {
            let mut event =
                running_event(&request.task_id, MOD_IMPORT_PREVIEW_IMAGE_FALLBACK_PHASE);
            event.error = Some(preview_image_rejection_reason_key(*reason).to_owned());
            events.push(event);
        }

        Ok(ModImportPrepareResult { analysis, events })
    }
}

struct TaskManagerCancellationToken {
    task_manager: Arc<crate::TaskManager>,
    task_id: String,
}

impl CancellationToken for TaskManagerCancellationToken {
    fn is_cancelled(&self) -> bool {
        self.task_manager.task_status(&self.task_id) == Some(crate::TaskStatus::Cancelled)
    }
}

fn ensure_not_cancelled(cancellation_token: &dyn CancellationToken) -> anyhow::Result<()> {
    if cancellation_token.is_cancelled() {
        anyhow::bail!("mod import prepare cancelled");
    }

    Ok(())
}

fn running_event(task_id: &str, phase: &str) -> crate::TaskProgressEvent {
    crate::TaskProgressEvent::new(
        task_id.to_owned(),
        crate::TaskKind::ModImport,
        crate::TaskStatus::Running,
        phase,
    )
}

fn failed_event(task_id: &str) -> crate::TaskProgressEvent {
    let mut event = crate::TaskProgressEvent::new(
        task_id.to_owned(),
        crate::TaskKind::ModImport,
        crate::TaskStatus::Failed,
        MOD_IMPORT_UNPACK_FAILED_PHASE,
    );
    event.error = Some(MOD_IMPORT_PREPARE_FAILED_ERROR.to_owned());
    event
}

fn preview_image_rejection_reason_key(reason: PreviewImageRejectionReason) -> &'static str {
    match reason {
        PreviewImageRejectionReason::Missing => "missing",
        PreviewImageRejectionReason::TooLarge => "too_large",
        PreviewImageRejectionReason::TooManyCandidates => "too_many_candidates",
        PreviewImageRejectionReason::UnsupportedFormat => "unsupported_format",
        PreviewImageRejectionReason::DecodeFailed => "decode_failed",
        PreviewImageRejectionReason::PixelLimitExceeded => "pixel_limit_exceeded",
        PreviewImageRejectionReason::CacheWriteFailed => "cache_write_failed",
    }
}

pub struct ModImportAnalysisService {
    preview_image_processor: Box<dyn ImportPreviewImageProcessor>,
    thumbnail_store: Box<dyn ThumbnailStore>,
    metadata_analyzer: Box<dyn ModPackageMetadataAnalyzer>,
}

impl ModImportAnalysisService {
    pub fn new(
        preview_image_processor: Box<dyn ImportPreviewImageProcessor>,
        thumbnail_store: Box<dyn ThumbnailStore>,
        metadata_analyzer: Box<dyn ModPackageMetadataAnalyzer>,
    ) -> Self {
        Self {
            preview_image_processor,
            thumbnail_store,
            metadata_analyzer,
        }
    }

    pub fn analyze_sandbox(
        &self,
        request: ModImportAnalysisRequest,
    ) -> anyhow::Result<ModImportAnalysisResult> {
        self.analyze_sandbox_with_cancellation(request, &NeverCancelled)
    }

    pub fn analyze_sandbox_with_cancellation(
        &self,
        request: ModImportAnalysisRequest,
        cancellation_token: &dyn CancellationToken,
    ) -> anyhow::Result<ModImportAnalysisResult> {
        ensure_not_cancelled(cancellation_token)?;
        let preview_image = match self
            .preview_image_processor
            .process_package_preview_with_cancellation(
                &request.task_id,
                &request.package_id,
                &request.sandbox_root,
                cancellation_token,
            )? {
            PreviewImageProcessingResult::Thumbnail(thumbnail) => {
                match self.thumbnail_store.resolve_url(&thumbnail.thumbnail_ref) {
                    Ok(thumbnail_url) => ImportPreviewImage::Thumbnail {
                        thumbnail_url,
                        width: thumbnail.width,
                        height: thumbnail.height,
                        content_hash: thumbnail.content_hash,
                        variant: thumbnail.thumbnail_ref.variant,
                    },
                    Err(_) => ImportPreviewImage::Fallback {
                        reason: PreviewImageRejectionReason::CacheWriteFailed,
                    },
                }
            }
            PreviewImageProcessingResult::Fallback(reason) => {
                ImportPreviewImage::Fallback { reason }
            }
        };
        ensure_not_cancelled(cancellation_token)?;

        let metadata = self
            .metadata_analyzer
            .analyze_metadata(&request.package_id, &request.sandbox_root)
            .unwrap_or_default();
        let display_name = metadata
            .display_name
            .clone()
            .unwrap_or_else(|| request.package_id.clone());

        Ok(ModImportAnalysisResult {
            task_id: request.task_id,
            package_id: request.package_id,
            display_name,
            metadata,
            preview_image,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mod_import_diagnostics::{
        PreviewImageDiagnosticExportCategory, PreviewImageDiagnosticExportCategoryId,
        PreviewImageDiagnosticExportCategoryStatus, PreviewImageDiagnosticExportExclusionReason,
        PreviewImageFallbackDiagnostic,
    };
    use hmm_core::{ModMetadataOverlay, PreviewImageRejectionReason};
    use hmm_ports::{
        AppSettings, AppSettingsRepository, AppSettingsRepositoryResult,
        ModImportPackagePrepareRequest, ModImportPackagePreparer, ModImportResultRepository,
        ModPackageMetadata, ModPackageMetadataAnalyzer, PreparedModPackage,
        PreviewImageProcessingResult, ProcessedPreviewImage, StoredImportPreviewImage,
        StoredModImportAnalysis, ThumbnailCacheMaintenance, ThumbnailCacheMaintenanceRequest,
        ThumbnailRef, ThumbnailStore,
    };
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn analyze_sandbox_includes_preview_thumbnail() {
        let service = ModImportAnalysisService::new(
            Box::new(FakePreviewImageProcessor {
                result: sample_thumbnail_result(),
            }),
            Box::new(FakeThumbnailStore::default()),
            Box::new(FakeMetadataAnalyzer::default()),
        );

        let result = service
            .analyze_sandbox(ModImportAnalysisRequest {
                task_id: "task-1".to_owned(),
                package_id: "pkg-1".to_owned(),
                sandbox_root: Path::new("sandbox").to_path_buf(),
            })
            .expect("analysis succeeds");

        assert_eq!(result.task_id, "task-1");
        assert_eq!(result.package_id, "pkg-1");
        assert_eq!(
            result.preview_image,
            ImportPreviewImage::Thumbnail {
                thumbnail_url: "thumbnail://pkg-1/preview-768/hash-1".to_owned(),
                width: 320,
                height: 180,
                content_hash: "hash-1".to_owned(),
                variant: "preview-768".to_owned(),
            }
        );
    }

    #[test]
    fn analyze_sandbox_uses_package_metadata_display_name() {
        let service = ModImportAnalysisService::new(
            Box::new(FakePreviewImageProcessor {
                result: PreviewImageProcessingResult::Fallback(
                    PreviewImageRejectionReason::Missing,
                ),
            }),
            Box::new(FakeThumbnailStore::default()),
            Box::new(FakeMetadataAnalyzer {
                display_name: Some("Better Mod Name".to_owned()),
                ..FakeMetadataAnalyzer::default()
            }),
        );

        let result = service
            .analyze_sandbox(ModImportAnalysisRequest {
                task_id: "task-1".to_owned(),
                package_id: "pkg-1".to_owned(),
                sandbox_root: Path::new("sandbox").to_path_buf(),
            })
            .expect("analysis succeeds");

        assert_eq!(result.display_name, "Better Mod Name");
        assert_eq!(
            stored_analysis_from_result(&result).display_name,
            "Better Mod Name"
        );
    }

    #[test]
    fn analyze_sandbox_persists_package_metadata_schema_fields() {
        let service = ModImportAnalysisService::new(
            Box::new(FakePreviewImageProcessor {
                result: PreviewImageProcessingResult::Fallback(
                    PreviewImageRejectionReason::Missing,
                ),
            }),
            Box::new(FakeThumbnailStore::default()),
            Box::new(FakeMetadataAnalyzer {
                display_name: Some("Better Mod Name".to_owned()),
                version: Some("1.2.3".to_owned()),
                author: Some("A Hunter".to_owned()),
                category: Some("Visual".to_owned()),
                tags: vec!["armor".to_owned(), "hd".to_owned()],
                dependencies: vec!["stracker-loader".to_owned()],
            }),
        );

        let result = service
            .analyze_sandbox(ModImportAnalysisRequest {
                task_id: "task-1".to_owned(),
                package_id: "pkg-1".to_owned(),
                sandbox_root: Path::new("sandbox").to_path_buf(),
            })
            .expect("analysis succeeds");
        let stored = stored_analysis_from_result(&result);
        let library_item = library_item_from_stored(stored.clone());

        assert_eq!(result.metadata.version.as_deref(), Some("1.2.3"));
        assert_eq!(stored.metadata.author.as_deref(), Some("A Hunter"));
        assert_eq!(stored.metadata.dependencies, vec!["stracker-loader"]);
        assert_eq!(library_item.category_labels, vec![
            CategoryLabel { name: "Visual".to_owned(), color: None },
            CategoryLabel { name: "armor".to_owned(), color: None },
            CategoryLabel { name: "hd".to_owned(), color: None },
        ]);
    }

    #[test]
    fn analyze_sandbox_keeps_import_result_when_preview_falls_back() {
        let service = ModImportAnalysisService::new(
            Box::new(FakePreviewImageProcessor {
                result: PreviewImageProcessingResult::Fallback(
                    PreviewImageRejectionReason::DecodeFailed,
                ),
            }),
            Box::new(FakeThumbnailStore::default()),
            Box::new(FakeMetadataAnalyzer::default()),
        );

        let result = service
            .analyze_sandbox(ModImportAnalysisRequest {
                task_id: "task-1".to_owned(),
                package_id: "pkg-1".to_owned(),
                sandbox_root: Path::new("sandbox").to_path_buf(),
            })
            .expect("analysis succeeds");

        assert_eq!(
            result.preview_image,
            ImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::DecodeFailed,
            }
        );
    }

    #[test]
    fn analyze_sandbox_passes_cancellation_token_to_preview_processor() {
        let observed = std::sync::Arc::new(Mutex::new(Vec::new()));
        let service = ModImportAnalysisService::new(
            Box::new(CancellationObservingPreviewImageProcessor {
                observed: std::sync::Arc::clone(&observed),
            }),
            Box::new(FakeThumbnailStore::default()),
            Box::new(FakeMetadataAnalyzer::default()),
        );
        let cancellation_token = TestCancellationToken { cancelled: false };

        let result = service
            .analyze_sandbox_with_cancellation(
                ModImportAnalysisRequest {
                    task_id: "task-1".to_owned(),
                    package_id: "pkg-1".to_owned(),
                    sandbox_root: Path::new("sandbox").to_path_buf(),
                },
                &cancellation_token,
            )
            .expect("analysis succeeds");

        assert_eq!(
            result.preview_image,
            ImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::Missing,
            }
        );
        assert_eq!(observed.lock().expect("observed lock").as_slice(), &[false]);
    }

    #[test]
    fn analyze_sandbox_falls_back_when_thumbnail_url_resolution_fails() {
        let service = ModImportAnalysisService::new(
            Box::new(FakePreviewImageProcessor {
                result: sample_thumbnail_result(),
            }),
            Box::new(FakeThumbnailStore { fail_resolve: true }),
            Box::new(FakeMetadataAnalyzer::default()),
        );

        let result = service
            .analyze_sandbox(ModImportAnalysisRequest {
                task_id: "task-1".to_owned(),
                package_id: "pkg-1".to_owned(),
                sandbox_root: Path::new("sandbox").to_path_buf(),
            })
            .expect("analysis succeeds");

        assert_eq!(
            result.preview_image,
            ImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::CacheWriteFailed,
            }
        );
    }

    #[test]
    fn prepare_import_runs_preparer_and_preview_analysis_with_task_events() {
        let service = ModImportPrepareService::new(
            Box::new(FakePackagePreparer::new(
                "task-1",
                Path::new("C:/mods/sample.zip"),
                "pkg-1",
                Path::new("sandbox"),
            )),
            ModImportAnalysisService::new(
                Box::new(FakePreviewImageProcessor {
                    result: sample_thumbnail_result(),
                }),
                Box::new(FakeThumbnailStore::default()),
                Box::new(FakeMetadataAnalyzer::default()),
            ),
        );

        let result = service
            .prepare_import(ModImportPrepareRequest {
                task_id: "task-1".to_owned(),
                archive_path: Path::new("C:/mods/sample.zip").to_path_buf(),
            })
            .expect("prepare succeeds");

        assert_eq!(result.analysis.task_id, "task-1");
        assert_eq!(result.analysis.package_id, "pkg-1");
        assert_eq!(
            result.analysis.preview_image,
            ImportPreviewImage::Thumbnail {
                thumbnail_url: "thumbnail://pkg-1/preview-768/hash-1".to_owned(),
                width: 320,
                height: 180,
                content_hash: "hash-1".to_owned(),
                variant: "preview-768".to_owned(),
            }
        );
        assert_eq!(
            event_phases(&result.events),
            vec![
                "mod_import.unpack.started",
                "mod_import.unpack.completed",
                "mod_import.preview_image.processing",
            ]
        );
        assert!(result.events.iter().all(|event| event.task_id == "task-1"
            && event.kind == crate::TaskKind::ModImport
            && event.status == crate::TaskStatus::Running));
    }

    #[test]
    fn prepare_import_emits_preview_fallback_event_when_preview_falls_back() {
        let service = ModImportPrepareService::new(
            Box::new(FakePackagePreparer::new(
                "task-1",
                Path::new("C:/mods/sample.zip"),
                "pkg-1",
                Path::new("sandbox"),
            )),
            ModImportAnalysisService::new(
                Box::new(FakePreviewImageProcessor {
                    result: PreviewImageProcessingResult::Fallback(
                        PreviewImageRejectionReason::DecodeFailed,
                    ),
                }),
                Box::new(FakeThumbnailStore::default()),
                Box::new(FakeMetadataAnalyzer::default()),
            ),
        );

        let result = service
            .prepare_import(ModImportPrepareRequest {
                task_id: "task-1".to_owned(),
                archive_path: Path::new("C:/mods/sample.zip").to_path_buf(),
            })
            .expect("prepare succeeds");

        assert_eq!(
            result.analysis.preview_image,
            ImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::DecodeFailed,
            }
        );
        assert_eq!(
            event_phases(&result.events),
            vec![
                "mod_import.unpack.started",
                "mod_import.unpack.completed",
                "mod_import.preview_image.processing",
                "mod_import.preview_image.fallback",
            ]
        );
        let fallback_event = result
            .events
            .iter()
            .find(|event| event.phase == "mod_import.preview_image.fallback")
            .expect("fallback event is emitted");
        assert_eq!(fallback_event.error.as_deref(), Some("decode_failed"));
        assert_eq!(fallback_event.message, None);
        assert_eq!(fallback_event.result_ref, None);
    }

    #[test]
    fn preview_image_rejection_reason_key_returns_contract_values() {
        assert_eq!(
            [
                preview_image_rejection_reason_key(PreviewImageRejectionReason::Missing),
                preview_image_rejection_reason_key(PreviewImageRejectionReason::TooLarge),
                preview_image_rejection_reason_key(PreviewImageRejectionReason::TooManyCandidates),
                preview_image_rejection_reason_key(PreviewImageRejectionReason::UnsupportedFormat),
                preview_image_rejection_reason_key(PreviewImageRejectionReason::DecodeFailed),
                preview_image_rejection_reason_key(PreviewImageRejectionReason::PixelLimitExceeded),
                preview_image_rejection_reason_key(PreviewImageRejectionReason::CacheWriteFailed),
            ],
            [
                "missing",
                "too_large",
                "too_many_candidates",
                "unsupported_format",
                "decode_failed",
                "pixel_limit_exceeded",
                "cache_write_failed",
            ]
        );
    }

    #[test]
    fn task_runner_executes_prepare_and_marks_task_completed() {
        let task_manager = std::sync::Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::ModImport)
            .expect("task can be created");
        let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
        let runner = ModImportTaskRunner::new(
            std::sync::Arc::clone(&task_manager),
            std::sync::Arc::new(ModImportPrepareService::new(
                Box::new(FakePackagePreparer::new(
                    &task.task_id,
                    Path::new("C:/mods/sample.zip"),
                    "pkg-1",
                    Path::new("sandbox"),
                )),
                ModImportAnalysisService::new(
                    Box::new(FakePreviewImageProcessor {
                        result: PreviewImageProcessingResult::Fallback(
                            PreviewImageRejectionReason::Missing,
                        ),
                    }),
                    Box::new(FakeThumbnailStore::default()),
                    Box::new(FakeMetadataAnalyzer {
                        display_name: Some("Better Mod Name".to_owned()),
                        ..FakeMetadataAnalyzer::default()
                    }),
                ),
            )),
            result_repository,
        );

        let events = runner
            .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
            .expect("runner succeeds");

        assert_eq!(
            event_phases(&events),
            vec![
                "mod_import.unpack.started",
                "mod_import.unpack.completed",
                "mod_import.preview_image.processing",
                "mod_import.preview_image.fallback",
                "mod_import.prepare.completed",
            ]
        );
        assert!(events.iter().all(|event| event.task_id == task.task_id));
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Completed)
        );
    }

    #[test]
    fn task_runner_persists_prepare_analysis_for_library_queries() {
        let task_manager = std::sync::Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::ModImport)
            .expect("task can be created");
        let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
        let runner = ModImportTaskRunner::new(
            std::sync::Arc::clone(&task_manager),
            std::sync::Arc::new(ModImportPrepareService::new(
                Box::new(FakePackagePreparer::new(
                    &task.task_id,
                    Path::new("C:/mods/sample.zip"),
                    "pkg-1",
                    Path::new("sandbox"),
                )),
                ModImportAnalysisService::new(
                    Box::new(FakePreviewImageProcessor {
                        result: sample_thumbnail_result(),
                    }),
                    Box::new(FakeThumbnailStore::default()),
                    Box::new(FakeMetadataAnalyzer {
                        display_name: Some("Better Mod Name".to_owned()),
                        ..FakeMetadataAnalyzer::default()
                    }),
                ),
            )),
            result_repository.clone(),
        );

        runner
            .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
            .expect("runner succeeds");

        let stored = result_repository
            .get_analysis("pkg-1")
            .expect("repository read succeeds")
            .expect("analysis was saved");

        assert_eq!(stored.mod_id, "pkg-1");
        assert_eq!(stored.package_id, "pkg-1");
        assert_eq!(stored.display_name, "Better Mod Name");
        assert_eq!(stored.task_id, task.task_id);
        assert_eq!(
            stored.preview_image,
            StoredImportPreviewImage::Thumbnail {
                thumbnail_url: "thumbnail://pkg-1/preview-768/hash-1".to_owned(),
                width: 320,
                height: 180,
                content_hash: "hash-1".to_owned(),
                variant: "preview-768".to_owned(),
            }
        );
    }

    #[test]
    fn task_runner_prunes_thumbnail_cache_using_all_persisted_thumbnail_refs() {
        let task_manager = std::sync::Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::ModImport)
            .expect("task can be created");
        let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
        result_repository
            .save_analysis(&StoredModImportAnalysis {
                mod_id: "pkg-old".to_owned(),
                task_id: "task-old".to_owned(),
                package_id: "pkg-old".to_owned(),
                display_name: "Old Mod".to_owned(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: StoredImportPreviewImage::Thumbnail {
                    thumbnail_url: "thumbnail://pkg-old/preview-1024/hash-old".to_owned(),
                    width: 320,
                    height: 180,
                    content_hash: "hash-old".to_owned(),
                    variant: "preview-1024".to_owned(),
                },
            })
            .expect("seed old analysis");
        let thumbnail_cache_maintenance =
            std::sync::Arc::new(FakeThumbnailCacheMaintenance::default());
        let runner = ModImportTaskRunner::new(
            std::sync::Arc::clone(&task_manager),
            std::sync::Arc::new(ModImportPrepareService::new(
                Box::new(FakePackagePreparer::new(
                    &task.task_id,
                    Path::new("C:/mods/sample.zip"),
                    "pkg-1",
                    Path::new("sandbox"),
                )),
                ModImportAnalysisService::new(
                    Box::new(FakePreviewImageProcessor {
                        result: sample_thumbnail_result(),
                    }),
                    Box::new(FakeThumbnailStore::default()),
                    Box::new(FakeMetadataAnalyzer::default()),
                ),
            )),
            result_repository,
        )
        .with_thumbnail_cache_maintenance(thumbnail_cache_maintenance.clone());

        runner
            .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
            .expect("runner succeeds");

        let calls = thumbnail_cache_maintenance
            .calls
            .lock()
            .expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].retained,
            vec![
                ThumbnailRef {
                    package_id: "pkg-old".to_owned(),
                    variant: "preview-1024".to_owned(),
                    content_hash: "hash-old".to_owned(),
                },
                ThumbnailRef {
                    package_id: "pkg-1".to_owned(),
                    variant: "preview-768".to_owned(),
                    content_hash: "hash-1".to_owned(),
                },
            ]
        );
        assert_eq!(calls[0].max_bytes, Some(DEFAULT_THUMBNAIL_CACHE_MAX_BYTES));
    }

    #[test]
    fn task_runner_uses_configured_thumbnail_cache_size_limit_for_maintenance() {
        let task_manager = std::sync::Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::ModImport)
            .expect("task can be created");
        let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
        let thumbnail_cache_maintenance =
            std::sync::Arc::new(FakeThumbnailCacheMaintenance::default());
        let settings_repository = fake_app_settings_repository(64 * 1024 * 1024, 14);
        let runner = ModImportTaskRunner::new(
            std::sync::Arc::clone(&task_manager),
            std::sync::Arc::new(ModImportPrepareService::new(
                Box::new(FakePackagePreparer::new(
                    &task.task_id,
                    Path::new("C:/mods/sample.zip"),
                    "pkg-1",
                    Path::new("sandbox"),
                )),
                ModImportAnalysisService::new(
                    Box::new(FakePreviewImageProcessor {
                        result: PreviewImageProcessingResult::Fallback(
                            PreviewImageRejectionReason::Missing,
                        ),
                    }),
                    Box::new(FakeThumbnailStore::default()),
                    Box::new(FakeMetadataAnalyzer::default()),
                ),
            )),
            result_repository,
        )
        .with_thumbnail_cache_maintenance(thumbnail_cache_maintenance.clone())
        .with_app_settings_repository(settings_repository.clone());

        runner
            .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
            .expect("runner succeeds");

        let calls = thumbnail_cache_maintenance
            .calls
            .lock()
            .expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].max_bytes, Some(64 * 1024 * 1024));
        assert_eq!(calls[0].max_age, Some(days(14)));
        assert_eq!(settings_repository.load_count(), 1);
    }

    #[test]
    fn scheduled_thumbnail_cache_maintenance_runs_one_cycle_after_interval() {
        let task_manager = std::sync::Arc::new(crate::TaskManager::new());
        let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
        result_repository
            .save_analysis(&StoredModImportAnalysis {
                mod_id: "pkg-1".to_owned(),
                task_id: "task-1".to_owned(),
                package_id: "pkg-1".to_owned(),
                display_name: "Mod".to_owned(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: StoredImportPreviewImage::Thumbnail {
                    thumbnail_url: "thumbnail://pkg-1/preview-768/hash-1".to_owned(),
                    width: 320,
                    height: 180,
                    content_hash: "hash-1".to_owned(),
                    variant: "preview-768".to_owned(),
                },
            })
            .expect("seed analysis");
        let thumbnail_cache_maintenance =
            std::sync::Arc::new(FakeThumbnailCacheMaintenance::default());
        let settings_repository = fake_app_settings_repository(32 * 1024 * 1024, 7);
        let runner = std::sync::Arc::new(
            ModImportTaskRunner::new(
                task_manager,
                std::sync::Arc::new(ModImportPrepareService::new(
                    Box::new(FakePackagePreparer::new(
                        "unused-task",
                        Path::new("C:/mods/unused.zip"),
                        "pkg-unused",
                        Path::new("sandbox"),
                    )),
                    ModImportAnalysisService::new(
                        Box::new(FakePreviewImageProcessor {
                            result: PreviewImageProcessingResult::Fallback(
                                PreviewImageRejectionReason::Missing,
                            ),
                        }),
                        Box::new(FakeThumbnailStore::default()),
                        Box::new(FakeMetadataAnalyzer::default()),
                    ),
                )),
                result_repository,
            )
            .with_thumbnail_cache_maintenance(thumbnail_cache_maintenance.clone())
            .with_app_settings_repository(settings_repository),
        );
        let scheduler = ThumbnailCacheMaintenanceScheduler::new(runner, Duration::from_secs(3600));
        let slept_intervals = Mutex::new(Vec::new());

        scheduler.run_one_cycle_with_sleep(|duration| {
            slept_intervals
                .lock()
                .expect("slept intervals lock")
                .push(duration);
        });

        assert_eq!(
            *slept_intervals.lock().expect("slept intervals lock"),
            vec![Duration::from_secs(3600)]
        );
        let calls = thumbnail_cache_maintenance
            .calls
            .lock()
            .expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].max_bytes, Some(32 * 1024 * 1024));
        assert_eq!(calls[0].max_age, Some(days(7)));
        assert_eq!(
            calls[0].retained,
            vec![ThumbnailRef {
                package_id: "pkg-1".to_owned(),
                variant: "preview-768".to_owned(),
                content_hash: "hash-1".to_owned(),
            }]
        );
    }

    #[test]
    fn manual_thumbnail_cache_maintenance_uses_retained_refs_and_settings() {
        let task_manager = std::sync::Arc::new(crate::TaskManager::new());
        let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
        result_repository
            .save_analysis(&StoredModImportAnalysis {
                mod_id: "pkg-1".to_owned(),
                task_id: "task-1".to_owned(),
                package_id: "pkg-1".to_owned(),
                display_name: "Mod".to_owned(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: StoredImportPreviewImage::Thumbnail {
                    thumbnail_url: "thumbnail://pkg-1/preview-768/hash-1".to_owned(),
                    width: 320,
                    height: 180,
                    content_hash: "hash-1".to_owned(),
                    variant: "preview-768".to_owned(),
                },
            })
            .expect("seed analysis");
        let thumbnail_cache_maintenance =
            std::sync::Arc::new(FakeThumbnailCacheMaintenance::default());
        let settings_repository = fake_app_settings_repository(96 * 1024 * 1024, 3);
        let runner = ModImportTaskRunner::new(
            task_manager,
            std::sync::Arc::new(ModImportPrepareService::new(
                Box::new(FakePackagePreparer::new(
                    "unused-task",
                    Path::new("C:/mods/unused.zip"),
                    "pkg-unused",
                    Path::new("sandbox"),
                )),
                ModImportAnalysisService::new(
                    Box::new(FakePreviewImageProcessor {
                        result: PreviewImageProcessingResult::Fallback(
                            PreviewImageRejectionReason::Missing,
                        ),
                    }),
                    Box::new(FakeThumbnailStore::default()),
                    Box::new(FakeMetadataAnalyzer::default()),
                ),
            )),
            result_repository,
        )
        .with_thumbnail_cache_maintenance(thumbnail_cache_maintenance.clone())
        .with_app_settings_repository(settings_repository);

        runner.maintain_thumbnail_cache_now();

        let calls = thumbnail_cache_maintenance
            .calls
            .lock()
            .expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].max_bytes, Some(96 * 1024 * 1024));
        assert_eq!(calls[0].max_age, Some(days(3)));
        assert_eq!(
            calls[0].retained,
            vec![ThumbnailRef {
                package_id: "pkg-1".to_owned(),
                variant: "preview-768".to_owned(),
                content_hash: "hash-1".to_owned(),
            }]
        );
    }

    #[test]
    fn task_runner_completes_when_thumbnail_cache_maintenance_fails() {
        let task_manager = std::sync::Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::ModImport)
            .expect("task can be created");
        let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
        let runner = ModImportTaskRunner::new(
            std::sync::Arc::clone(&task_manager),
            std::sync::Arc::new(ModImportPrepareService::new(
                Box::new(FakePackagePreparer::new(
                    &task.task_id,
                    Path::new("C:/mods/sample.zip"),
                    "pkg-1",
                    Path::new("sandbox"),
                )),
                ModImportAnalysisService::new(
                    Box::new(FakePreviewImageProcessor {
                        result: sample_thumbnail_result(),
                    }),
                    Box::new(FakeThumbnailStore::default()),
                    Box::new(FakeMetadataAnalyzer::default()),
                ),
            )),
            result_repository.clone(),
        )
        .with_thumbnail_cache_maintenance(std::sync::Arc::new(
            FakeThumbnailCacheMaintenance {
                fail: true,
                ..FakeThumbnailCacheMaintenance::default()
            },
        ));

        let events = runner
            .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
            .expect("maintenance failure does not fail import");

        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Completed)
        );
        assert_eq!(
            event_phases(&events).last(),
            Some(&"mod_import.prepare.completed")
        );
        assert!(
            result_repository
                .get_analysis("pkg-1")
                .expect("repository read succeeds")
                .is_some(),
            "analysis remains persisted when cache maintenance fails"
        );
    }

    #[test]
    fn library_service_summarizes_preview_image_diagnostics_without_content() {
        let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
        for record in [
            StoredModImportAnalysis {
                mod_id: "pkg-thumbnail".to_owned(),
                task_id: "task-1".to_owned(),
                package_id: "pkg-thumbnail".to_owned(),
                display_name: "Thumbnail Mod".to_owned(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: StoredImportPreviewImage::Thumbnail {
                    thumbnail_url: "thumbnail://pkg-thumbnail/preview-768/hash-1".to_owned(),
                    width: 320,
                    height: 180,
                    content_hash: "hash-1".to_owned(),
                    variant: "preview-768".to_owned(),
                },
            },
            StoredModImportAnalysis {
                mod_id: "pkg-missing".to_owned(),
                task_id: "task-2".to_owned(),
                package_id: "pkg-missing".to_owned(),
                display_name: "Missing Preview".to_owned(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: StoredImportPreviewImage::Fallback {
                    reason: PreviewImageRejectionReason::Missing,
                },
            },
            StoredModImportAnalysis {
                mod_id: "pkg-decode".to_owned(),
                task_id: "task-3".to_owned(),
                package_id: "pkg-decode".to_owned(),
                display_name: "Decode Failed".to_owned(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: StoredImportPreviewImage::Fallback {
                    reason: PreviewImageRejectionReason::DecodeFailed,
                },
            },
            StoredModImportAnalysis {
                mod_id: "pkg-decode-2".to_owned(),
                task_id: "task-4".to_owned(),
                package_id: "pkg-decode-2".to_owned(),
                display_name: "Decode Failed Again".to_owned(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: StoredImportPreviewImage::Fallback {
                    reason: PreviewImageRejectionReason::DecodeFailed,
                },
            },
        ] {
            result_repository
                .save_analysis(&record)
                .expect("save analysis");
        }
        let service = ModLibraryService::new(result_repository, empty_metadata_repo(), empty_category_repo());

        let summary = service
            .get_preview_image_diagnostics()
            .expect("diagnostics query succeeds");

        assert_eq!(summary.total_imported_mods, 4);
        assert_eq!(summary.thumbnail_count, 1);
        assert_eq!(summary.fallback_count, 3);
        assert_eq!(
            summary.fallback_reasons,
            vec![
                PreviewImageFallbackDiagnostic {
                    reason: PreviewImageRejectionReason::Missing,
                    count: 1,
                },
                PreviewImageFallbackDiagnostic {
                    reason: PreviewImageRejectionReason::DecodeFailed,
                    count: 2,
                },
            ]
        );
        assert_eq!(
            summary.export_categories,
            vec![
                PreviewImageDiagnosticExportCategory {
                    category: PreviewImageDiagnosticExportCategoryId::PreviewImageSummary,
                    status: PreviewImageDiagnosticExportCategoryStatus::Included,
                    reason: None,
                },
                PreviewImageDiagnosticExportCategory {
                    category: PreviewImageDiagnosticExportCategoryId::ThumbnailFiles,
                    status: PreviewImageDiagnosticExportCategoryStatus::Excluded,
                    reason: Some(PreviewImageDiagnosticExportExclusionReason::DerivedImageContent),
                },
                PreviewImageDiagnosticExportCategory {
                    category: PreviewImageDiagnosticExportCategoryId::ThumbnailUrls,
                    status: PreviewImageDiagnosticExportCategoryStatus::Excluded,
                    reason: Some(
                        PreviewImageDiagnosticExportExclusionReason::OpaqueResourceReference
                    ),
                },
                PreviewImageDiagnosticExportCategory {
                    category: PreviewImageDiagnosticExportCategoryId::RawPackageContent,
                    status: PreviewImageDiagnosticExportCategoryStatus::Excluded,
                    reason: Some(PreviewImageDiagnosticExportExclusionReason::ThirdPartyModContent),
                },
            ]
        );
    }

    #[test]
    fn task_runner_marks_task_failed_without_exposing_paths() {
        let task_manager = std::sync::Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::ModImport)
            .expect("task can be created");
        let archive_path = Path::new("C:/Users/Alice/Mods/bad.zip").to_path_buf();
        let runner = ModImportTaskRunner::new(
            std::sync::Arc::clone(&task_manager),
            std::sync::Arc::new(ModImportPrepareService::new(
                Box::new(FailingPackagePreparer),
                ModImportAnalysisService::new(
                    Box::new(FakePreviewImageProcessor {
                        result: PreviewImageProcessingResult::Fallback(
                            PreviewImageRejectionReason::Missing,
                        ),
                    }),
                    Box::new(FakeThumbnailStore::default()),
                    Box::new(FakeMetadataAnalyzer::default()),
                ),
            )),
            std::sync::Arc::new(FakeModImportResultRepository::default()),
        );

        let error = runner
            .run_prepare_task(&task.task_id, archive_path)
            .expect_err("runner fails");

        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Failed)
        );
        assert_eq!(
            event_phases(&error.events),
            vec!["mod_import.unpack.failed"]
        );
        let failure = error.events.last().expect("failure event exists");
        assert_eq!(failure.status, crate::TaskStatus::Failed);
        assert_eq!(failure.error.as_deref(), Some("mod_import_prepare_failed"));
        assert!(!failure.error.as_deref().unwrap().contains("Alice"));
        assert!(!failure.error.as_deref().unwrap().contains("bad.zip"));
    }

    #[test]
    fn task_runner_does_not_emit_failed_event_for_already_cancelled_task() {
        let task_manager = std::sync::Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::ModImport)
            .expect("task can be created");
        task_manager
            .cancel_task(&task.task_id)
            .expect("task can be cancelled");
        let runner = ModImportTaskRunner::new(
            std::sync::Arc::clone(&task_manager),
            std::sync::Arc::new(ModImportPrepareService::new(
                Box::new(FakePackagePreparer::new(
                    &task.task_id,
                    Path::new("C:/mods/sample.zip"),
                    "pkg-1",
                    Path::new("sandbox"),
                )),
                ModImportAnalysisService::new(
                    Box::new(FakePreviewImageProcessor {
                        result: PreviewImageProcessingResult::Fallback(
                            PreviewImageRejectionReason::Missing,
                        ),
                    }),
                    Box::new(FakeThumbnailStore::default()),
                    Box::new(FakeMetadataAnalyzer::default()),
                ),
            )),
            std::sync::Arc::new(FakeModImportResultRepository::default()),
        );

        let error = runner
            .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
            .expect_err("cancelled task does not run");

        assert!(error.events.is_empty());
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Cancelled)
        );
    }

    #[test]
    fn task_runner_does_not_complete_or_persist_when_cancelled_during_prepare() {
        let task_manager = std::sync::Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::ModImport)
            .expect("task can be created");
        let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
        let runner = ModImportTaskRunner::new(
            std::sync::Arc::clone(&task_manager),
            std::sync::Arc::new(ModImportPrepareService::new(
                Box::new(CancellingPackagePreparer {
                    task_manager: std::sync::Arc::clone(&task_manager),
                    task_id: task.task_id.clone(),
                }),
                ModImportAnalysisService::new(
                    Box::new(FakePreviewImageProcessor {
                        result: sample_thumbnail_result(),
                    }),
                    Box::new(FakeThumbnailStore::default()),
                    Box::new(FakeMetadataAnalyzer::default()),
                ),
            )),
            result_repository.clone(),
        );

        let error = runner
            .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
            .expect_err("cancelled running task stops after prepare checkpoint");

        assert!(error.events.is_empty());
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Cancelled)
        );
        assert!(
            result_repository
                .get_analysis("pkg-1")
                .expect("repository read succeeds")
                .is_none(),
            "cancelled prepare result must not be persisted"
        );
    }

    #[test]
    fn task_runner_maintains_thumbnail_cache_when_cancelled_after_preview_processing() {
        let task_manager = std::sync::Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::ModImport)
            .expect("task can be created");
        let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
        let thumbnail_cache_maintenance =
            std::sync::Arc::new(FakeThumbnailCacheMaintenance::default());
        let runner = ModImportTaskRunner::new(
            std::sync::Arc::clone(&task_manager),
            std::sync::Arc::new(ModImportPrepareService::new(
                Box::new(FakePackagePreparer::new(
                    &task.task_id,
                    Path::new("C:/mods/sample.zip"),
                    "pkg-1",
                    Path::new("sandbox"),
                )),
                ModImportAnalysisService::new(
                    Box::new(CancellingPreviewImageProcessor {
                        task_manager: std::sync::Arc::clone(&task_manager),
                        task_id: task.task_id.clone(),
                    }),
                    Box::new(FakeThumbnailStore::default()),
                    Box::new(FakeMetadataAnalyzer::default()),
                ),
            )),
            result_repository.clone(),
        )
        .with_thumbnail_cache_maintenance(thumbnail_cache_maintenance.clone());

        let error = runner
            .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
            .expect_err("cancelled running task stops after preview checkpoint");

        assert!(error.events.is_empty());
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Cancelled)
        );
        assert!(result_repository
            .list_analysis()
            .expect("repository read succeeds")
            .is_empty());

        let calls = thumbnail_cache_maintenance
            .calls
            .lock()
            .expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert!(calls[0].retained.is_empty());
        assert_eq!(calls[0].max_bytes, Some(DEFAULT_THUMBNAIL_CACHE_MAX_BYTES));
    }

    #[test]
    fn task_runner_passes_running_cancellation_token_to_preparer() {
        let task_manager = std::sync::Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::ModImport)
            .expect("task can be created");
        let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
        let observed = std::sync::Arc::new(Mutex::new(Vec::new()));
        let runner = ModImportTaskRunner::new(
            std::sync::Arc::clone(&task_manager),
            std::sync::Arc::new(ModImportPrepareService::new(
                Box::new(CancellationObservingPackagePreparer {
                    task_manager: std::sync::Arc::clone(&task_manager),
                    task_id: task.task_id.clone(),
                    observed: std::sync::Arc::clone(&observed),
                }),
                ModImportAnalysisService::new(
                    Box::new(FakePreviewImageProcessor {
                        result: PreviewImageProcessingResult::Fallback(
                            PreviewImageRejectionReason::Missing,
                        ),
                    }),
                    Box::new(FakeThumbnailStore::default()),
                    Box::new(FakeMetadataAnalyzer::default()),
                ),
            )),
            result_repository.clone(),
        );

        let error = runner
            .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
            .expect_err("cancelled running task stops after prepare checkpoint");

        assert!(error.events.is_empty());
        assert_eq!(
            observed.lock().expect("observed lock").as_slice(),
            &[false, true]
        );
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Cancelled)
        );
        assert!(result_repository
            .list_analysis()
            .expect("repository read succeeds")
            .is_empty());
    }

    struct FakePreviewImageProcessor {
        result: PreviewImageProcessingResult,
    }

    impl ImportPreviewImageProcessor for FakePreviewImageProcessor {
        fn process_package_preview(
            &self,
            _task_id: &str,
            _package_id: &str,
            _sandbox_root: &Path,
        ) -> anyhow::Result<PreviewImageProcessingResult> {
            Ok(self.result.clone())
        }
    }

    struct CancellationObservingPreviewImageProcessor {
        observed: std::sync::Arc<Mutex<Vec<bool>>>,
    }

    impl ImportPreviewImageProcessor for CancellationObservingPreviewImageProcessor {
        fn process_package_preview(
            &self,
            _task_id: &str,
            _package_id: &str,
            _sandbox_root: &Path,
        ) -> anyhow::Result<PreviewImageProcessingResult> {
            anyhow::bail!("preview processor should receive cancellation-aware call")
        }

        fn process_package_preview_with_cancellation(
            &self,
            _task_id: &str,
            _package_id: &str,
            _sandbox_root: &Path,
            cancellation_token: &dyn CancellationToken,
        ) -> anyhow::Result<PreviewImageProcessingResult> {
            self.observed
                .lock()
                .expect("observed lock")
                .push(cancellation_token.is_cancelled());
            Ok(PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::Missing,
            ))
        }
    }

    struct CancellingPreviewImageProcessor {
        task_manager: std::sync::Arc<crate::TaskManager>,
        task_id: String,
    }

    impl ImportPreviewImageProcessor for CancellingPreviewImageProcessor {
        fn process_package_preview(
            &self,
            _task_id: &str,
            _package_id: &str,
            _sandbox_root: &Path,
        ) -> anyhow::Result<PreviewImageProcessingResult> {
            anyhow::bail!("preview processor should receive cancellation-aware call")
        }

        fn process_package_preview_with_cancellation(
            &self,
            _task_id: &str,
            _package_id: &str,
            _sandbox_root: &Path,
            cancellation_token: &dyn CancellationToken,
        ) -> anyhow::Result<PreviewImageProcessingResult> {
            assert!(!cancellation_token.is_cancelled());
            self.task_manager
                .cancel_task(&self.task_id)
                .expect("running task can be cancelled");

            Ok(sample_thumbnail_result())
        }
    }

    struct TestCancellationToken {
        cancelled: bool,
    }

    impl CancellationToken for TestCancellationToken {
        fn is_cancelled(&self) -> bool {
            self.cancelled
        }
    }

    #[derive(Default)]
    struct FakeMetadataAnalyzer {
        display_name: Option<String>,
        version: Option<String>,
        author: Option<String>,
        category: Option<String>,
        tags: Vec<String>,
        dependencies: Vec<String>,
    }

    impl ModPackageMetadataAnalyzer for FakeMetadataAnalyzer {
        fn analyze_metadata(
            &self,
            _package_id: &str,
            _sandbox_root: &Path,
        ) -> anyhow::Result<ModPackageMetadata> {
            Ok(ModPackageMetadata {
                display_name: self.display_name.clone(),
                version: self.version.clone(),
                author: self.author.clone(),
                category: self.category.clone(),
                tags: self.tags.clone(),
                dependencies: self.dependencies.clone(),
            })
        }
    }

    struct FakePackagePreparer {
        expected_task_id: String,
        expected_archive_path: std::path::PathBuf,
        package_id: String,
        sandbox_root: std::path::PathBuf,
    }

    impl FakePackagePreparer {
        fn new(
            expected_task_id: &str,
            expected_archive_path: &Path,
            package_id: &str,
            sandbox_root: &Path,
        ) -> Self {
            Self {
                expected_task_id: expected_task_id.to_owned(),
                expected_archive_path: expected_archive_path.to_path_buf(),
                package_id: package_id.to_owned(),
                sandbox_root: sandbox_root.to_path_buf(),
            }
        }
    }

    impl ModImportPackagePreparer for FakePackagePreparer {
        fn prepare_package(
            &self,
            request: ModImportPackagePrepareRequest<'_>,
        ) -> anyhow::Result<PreparedModPackage> {
            assert_eq!(request.task_id, self.expected_task_id);
            assert_eq!(request.archive_path, self.expected_archive_path);

            Ok(PreparedModPackage {
                package_id: self.package_id.clone(),
                sandbox_root: self.sandbox_root.clone(),
            })
        }
    }

    struct FailingPackagePreparer;

    impl ModImportPackagePreparer for FailingPackagePreparer {
        fn prepare_package(
            &self,
            _request: ModImportPackagePrepareRequest<'_>,
        ) -> anyhow::Result<PreparedModPackage> {
            anyhow::bail!("failed to prepare C:/Users/Alice/Mods/bad.zip")
        }
    }

    struct CancellingPackagePreparer {
        task_manager: std::sync::Arc<crate::TaskManager>,
        task_id: String,
    }

    impl ModImportPackagePreparer for CancellingPackagePreparer {
        fn prepare_package(
            &self,
            request: ModImportPackagePrepareRequest<'_>,
        ) -> anyhow::Result<PreparedModPackage> {
            assert_eq!(request.task_id, self.task_id);
            self.task_manager
                .cancel_task(&self.task_id)
                .expect("running task can be cancelled");

            Ok(PreparedModPackage {
                package_id: "pkg-1".to_owned(),
                sandbox_root: Path::new("sandbox").to_path_buf(),
            })
        }
    }

    struct CancellationObservingPackagePreparer {
        task_manager: std::sync::Arc<crate::TaskManager>,
        task_id: String,
        observed: std::sync::Arc<Mutex<Vec<bool>>>,
    }

    impl ModImportPackagePreparer for CancellationObservingPackagePreparer {
        fn prepare_package(
            &self,
            request: ModImportPackagePrepareRequest<'_>,
        ) -> anyhow::Result<PreparedModPackage> {
            assert_eq!(request.task_id, self.task_id);
            self.observed
                .lock()
                .expect("observed lock")
                .push(request.cancellation_token.is_cancelled());
            self.task_manager
                .cancel_task(&self.task_id)
                .expect("running task can be cancelled");
            self.observed
                .lock()
                .expect("observed lock")
                .push(request.cancellation_token.is_cancelled());

            Ok(PreparedModPackage {
                package_id: "pkg-1".to_owned(),
                sandbox_root: Path::new("sandbox").to_path_buf(),
            })
        }
    }

    #[derive(Default)]
    struct FakeThumbnailStore {
        fail_resolve: bool,
    }

    impl ThumbnailStore for FakeThumbnailStore {
        fn put_thumbnail(
            &self,
            _package_id: &str,
            _content_hash: &str,
            _variant: &str,
            _extension: &str,
            _bytes: &[u8],
        ) -> anyhow::Result<ThumbnailRef> {
            unreachable!("import analysis should not write thumbnails")
        }

        fn resolve_url(&self, thumbnail_ref: &ThumbnailRef) -> anyhow::Result<String> {
            if self.fail_resolve {
                anyhow::bail!("thumbnail url unavailable");
            }

            Ok(format!(
                "thumbnail://{}/{}/{}",
                thumbnail_ref.package_id, thumbnail_ref.variant, thumbnail_ref.content_hash
            ))
        }
    }

    #[derive(Default)]
    struct FakeModImportResultRepository {
        records: Mutex<Vec<StoredModImportAnalysis>>,
    }

    impl ModImportResultRepository for FakeModImportResultRepository {
        fn save_analysis(&self, analysis: &StoredModImportAnalysis) -> anyhow::Result<()> {
            let mut records = self.records.lock().expect("records lock");
            records.retain(|record| record.mod_id != analysis.mod_id);
            records.push(analysis.clone());
            Ok(())
        }

        fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
            Ok(self.records.lock().expect("records lock").clone())
        }

        fn get_analysis(&self, mod_id: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
            Ok(self
                .records
                .lock()
                .expect("records lock")
                .iter()
                .find(|record| record.mod_id == mod_id)
                .cloned())
        }
    }

    struct EmptyMetadataRepo;    impl ModMetadataRepository for EmptyMetadataRepo {
        fn get(&self, _: &str) -> anyhow::Result<Option<ModMetadataOverlay>> { Ok(None) }
        fn save(&self, _: &ModMetadataOverlay) -> anyhow::Result<()> { Ok(()) }
        fn delete(&self, _: &str) -> anyhow::Result<()> { Ok(()) }
        fn list_all(&self) -> anyhow::Result<Vec<ModMetadataOverlay>> { Ok(vec![]) }
    }

    fn empty_metadata_repo() -> Arc<EmptyMetadataRepo> {
        Arc::new(EmptyMetadataRepo)
    }
    use crate::category::test_support::empty_category_repo;

    #[derive(Default)]
    struct FakeThumbnailCacheMaintenance {
        calls: Mutex<Vec<FakeThumbnailCacheMaintenanceCall>>,
        fail: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct FakeThumbnailCacheMaintenanceCall {
        retained: Vec<ThumbnailRef>,
        max_bytes: Option<u64>,
        max_age: Option<Duration>,
    }

    impl ThumbnailCacheMaintenance for FakeThumbnailCacheMaintenance {
        fn maintain_thumbnail_cache(
            &self,
            request: ThumbnailCacheMaintenanceRequest<'_>,
        ) -> anyhow::Result<()> {
            self.calls
                .lock()
                .expect("calls lock")
                .push(FakeThumbnailCacheMaintenanceCall {
                    retained: request.retained.to_vec(),
                    max_bytes: request.max_bytes,
                    max_age: request.max_age,
                });

            if self.fail {
                anyhow::bail!("cache maintenance unavailable");
            }

            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeAppSettingsRepository {
        settings: AppSettings,
        load_count: Mutex<usize>,
    }

    impl AppSettingsRepository for FakeAppSettingsRepository {
        fn load_settings(&self) -> AppSettingsRepositoryResult<AppSettings> {
            *self.load_count.lock().expect("load count lock") += 1;
            Ok(self.settings.clone())
        }

        fn save_settings(&self, _settings: &AppSettings) -> AppSettingsRepositoryResult<()> {
            Ok(())
        }
    }

    impl FakeAppSettingsRepository {
        fn load_count(&self) -> usize {
            *self.load_count.lock().expect("load count lock")
        }
    }

    fn fake_app_settings_repository(
        max_bytes: u64,
        max_age_days: u32,
    ) -> Arc<FakeAppSettingsRepository> {
        Arc::new(FakeAppSettingsRepository {
            settings: AppSettings {
                thumbnail_cache_max_bytes: Some(max_bytes),
                thumbnail_cache_max_age_days: Some(max_age_days),
            },
            load_count: Mutex::new(0),
        })
    }

    fn days(days: u64) -> Duration {
        Duration::from_secs(days * 24 * 60 * 60)
    }

    fn event_phases(events: &[crate::TaskProgressEvent]) -> Vec<&str> {
        events.iter().map(|event| event.phase.as_str()).collect()
    }

    fn sample_thumbnail_result() -> PreviewImageProcessingResult {
        PreviewImageProcessingResult::Thumbnail(ProcessedPreviewImage {
            thumbnail_ref: ThumbnailRef {
                package_id: "pkg-1".to_owned(),
                variant: "preview-768".to_owned(),
                content_hash: "hash-1".to_owned(),
            },
            width: 320,
            height: 180,
            content_hash: "hash-1".to_owned(),
        })
    }
}
