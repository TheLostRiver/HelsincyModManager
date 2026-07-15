use crate::mod_import_diagnostics::{
    preview_image_diagnostics_from_stored, PreviewImageDiagnosticsSummary,
};
use hmm_core::{CategoryLabel, ModId, ModRevisionId, PreviewImageRejectionReason};
use hmm_ports::{
    AppSettingsRepository, CancellationToken, CategoryRepository,
    ModImportPackagePrepareRequest,
    ModImportPackagePreparer, ModImportResultRepository, ModMetadataRepository,
    ModPackageMetadataAnalyzer,
    NeverCancelled, PreviewImageProcessingResult, StoredImportPreviewImage,
    StoredLogicalMod, StoredModImportAnalysis, StoredModOriginProvenance,
    StoredModPackageMetadata, StoredModRevision, ThumbnailCacheMaintenance,
    ThumbnailCacheMaintenanceRequest, ThumbnailRef, ThumbnailStore,
};
use std::collections::BTreeSet;
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
    cause: Option<String>,
}

impl ModImportTaskRunError {
    pub fn cause(&self) -> Option<&str> {
        self.cause.as_deref()
    }
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
pub struct ModRevisionList {
    pub mod_id: ModId,
    pub origin_revision_id: ModRevisionId,
    pub display_revision_id: ModRevisionId,
    pub revision_ids: Vec<ModRevisionId>,
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

enum ModImportCatalogTarget {
    NewLogicalMod,
    ExistingLogicalMod(ModId),
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
        self.run_prepare_task_for_target(
            task_id,
            archive_path,
            ModImportCatalogTarget::NewLogicalMod,
        )
    }

    pub fn run_prepare_revision_task(
        &self,
        task_id: &str,
        archive_path: PathBuf,
        mod_id: ModId,
    ) -> Result<Vec<crate::TaskProgressEvent>, ModImportTaskRunError> {
        self.run_prepare_task_for_target(
            task_id,
            archive_path,
            ModImportCatalogTarget::ExistingLogicalMod(mod_id),
        )
    }

    fn run_prepare_task_for_target(
        &self,
        task_id: &str,
        archive_path: PathBuf,
        target: ModImportCatalogTarget,
    ) -> Result<Vec<crate::TaskProgressEvent>, ModImportTaskRunError> {
        if self.task_manager.start_task(task_id).is_err() {
            return Err(ModImportTaskRunError {
                events: Vec::new(),
                cause: None,
            });
        }

        if let ModImportCatalogTarget::ExistingLogicalMod(mod_id) = &target {
            match self.result_repository.get_mod(mod_id) {
                Ok(Some(_)) => {}
                Ok(None) => {
                    let _ = self.task_manager.fail_task(task_id);
                    return Err(ModImportTaskRunError {
                        events: vec![failed_event(task_id)],
                        cause: None,
                    });
                }
                Err(error) => {
                    let _ = self.task_manager.fail_task(task_id);
                    return Err(ModImportTaskRunError {
                        events: vec![failed_event(task_id)],
                        cause: Some(error.to_string()),
                    });
                }
            }
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
                    return Err(ModImportTaskRunError {
                        events: Vec::new(),
                        cause: None,
                    });
                }

                let mod_id = match &target {
                    ModImportCatalogTarget::NewLogicalMod => {
                        ModId::new(&result.analysis.package_id)
                    }
                    ModImportCatalogTarget::ExistingLogicalMod(mod_id) => mod_id.clone(),
                };
                let revision = stored_revision_from_result(&mod_id, &result.analysis);
                let save_result = match target {
                    ModImportCatalogTarget::NewLogicalMod => {
                        let logical_mod = StoredLogicalMod {
                            mod_id,
                            origin_revision_id: revision.revision_id.clone(),
                            display_revision_id: revision.revision_id.clone(),
                            origin_provenance: StoredModOriginProvenance::Imported,
                        };
                        self.result_repository.save_new_mod(&logical_mod, &revision)
                    }
                    ModImportCatalogTarget::ExistingLogicalMod(_) => {
                        self.result_repository.append_revision(&revision)
                    }
                };
                if save_result.is_err() {
                    let _ = self.task_manager.fail_task(task_id);
                    return Err(ModImportTaskRunError {
                        events: vec![failed_event(task_id)],
                        cause: None,
                    });
                }

                self.maintain_thumbnail_cache();

                result.events
            }
            Err(_) => {
                if self.is_task_cancelled(task_id) {
                    self.maintain_thumbnail_cache();
                    return Err(ModImportTaskRunError {
                        events: Vec::new(),
                        cause: None,
                    });
                }

                let _ = self.task_manager.fail_task(task_id);
                return Err(ModImportTaskRunError {
                    events: vec![failed_event(task_id)],
                    cause: None,
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
                    cause: None,
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

    pub fn get_mod_revisions(&self, mod_id: &ModId) -> anyhow::Result<Option<ModRevisionList>> {
        let Some(logical_mod) = self.result_repository.get_mod(mod_id)? else {
            return Ok(None);
        };
        anyhow::ensure!(
            logical_mod.mod_id == *mod_id,
            "logical Mod id does not match query"
        );
        let revisions = self.result_repository.list_revisions(mod_id)?;
        anyhow::ensure!(
            revisions.iter().all(|revision| revision.mod_id == *mod_id),
            "revision owner does not match logical Mod"
        );
        let revision_ids = revisions
            .into_iter()
            .map(|revision| revision.revision_id)
            .collect::<Vec<_>>();
        let unique_revision_ids = revision_ids.iter().collect::<BTreeSet<_>>();
        anyhow::ensure!(
            unique_revision_ids.len() == revision_ids.len()
                && unique_revision_ids.contains(&logical_mod.origin_revision_id)
                && unique_revision_ids.contains(&logical_mod.display_revision_id),
            "logical Mod revision catalog is inconsistent"
        );

        Ok(Some(ModRevisionList {
            mod_id: logical_mod.mod_id,
            origin_revision_id: logical_mod.origin_revision_id,
            display_revision_id: logical_mod.display_revision_id,
            revision_ids,
        }))
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

fn stored_analysis_from_result(
    mod_id: &ModId,
    result: &ModImportAnalysisResult,
) -> StoredModImportAnalysis {
    StoredModImportAnalysis {
        mod_id: mod_id.as_str().to_owned(),
        task_id: result.task_id.clone(),
        package_id: result.package_id.clone(),
        display_name: result.display_name.clone(),
        metadata: stored_metadata_from_package_metadata(&result.metadata),
        preview_image: stored_preview_from_import(&result.preview_image),
    }
}

fn stored_revision_from_result(
    mod_id: &ModId,
    result: &ModImportAnalysisResult,
) -> StoredModRevision {
    let analysis = stored_analysis_from_result(mod_id, result);
    StoredModRevision {
        revision_id: ModRevisionId::new(&analysis.package_id),
        mod_id: mod_id.clone(),
        import_task_id: analysis.task_id,
        package_id: analysis.package_id,
        display_name: analysis.display_name,
        metadata: analysis.metadata,
        preview_image: analysis.preview_image,
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
mod revision_tests;
#[cfg(test)]
mod tests;
