use crate::mod_import_diagnostics::{
    preview_image_diagnostics_from_stored, PreviewImageDiagnosticsSummary,
};
use hmm_core::{
    deduplicate_mod_display_name, mod_display_name_from_archive_path, normalize_mod_display_name,
    CategoryLabel, ModId, ModRevisionId, PreviewImageRejectionReason,
};
use hmm_ports::{
    AppSettingsRepository, CancellationToken, CategoryRepository, ModImportPackagePrepareRequest,
    ModImportPackagePreparer, ModImportResultRepository, ModImportSandboxLocator,
    ModLibraryProjectionLabel, ModLibraryProjectionRecord, ModMetadataRepository,
    ModPackageMetadataAnalyzer, NeverCancelled, PreviewImageProcessingResult,
    StoredImportPreviewImage, StoredLogicalMod, StoredModImportAnalysis, StoredModOriginProvenance,
    StoredModPackageMetadata, StoredModRevision, ThumbnailCacheMaintenance,
    ThumbnailCacheMaintenanceRequest, ThumbnailRef, ThumbnailStore,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
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
    /// 压缩包文件名派生出的展示名候选，仅在包内元数据没有声明名称时使用。
    ///
    /// 命名点出它的地位：这是 hint 而非权威来源。只有走归档路径的普通导入能提供，
    /// reader 入口（外部导入）没有可用文件名，填 `None`。
    pub archive_display_name_hint: Option<String>,
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
    /// 仅用于展示名去重时读取用户改名。缺省时按 catalog 名字去重。
    metadata_repository: Option<Arc<dyn ModMetadataRepository>>,
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
            metadata_repository: None,
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

    pub fn with_metadata_repository(
        mut self,
        metadata_repository: Arc<dyn ModMetadataRepository>,
    ) -> Self {
        self.metadata_repository = Some(metadata_repository);
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
                let mut revision = stored_revision_from_result(&mod_id, &result.analysis);
                // The package id fallback identifies a revision; it must not rename its logical Mod.
                if result.analysis.metadata.display_name.is_none() {
                    if let ModImportCatalogTarget::ExistingLogicalMod(existing_mod_id) = &target {
                        let inherited_display_name = self
                            .result_repository
                            .get_mod(existing_mod_id)
                            .and_then(|logical_mod| {
                                logical_mod.ok_or_else(|| anyhow::anyhow!("logical Mod not found"))
                            })
                            .and_then(|logical_mod| {
                                self.result_repository
                                    .get_revision(&logical_mod.display_revision_id)
                            })
                            .and_then(|display_revision| {
                                display_revision
                                    .ok_or_else(|| anyhow::anyhow!("display revision not found"))
                            });
                        match inherited_display_name {
                            Ok(display_revision) => {
                                revision.display_name = display_revision.display_name;
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
                }
                let save_result = match target {
                    ModImportCatalogTarget::NewLogicalMod => {
                        // 只对新建 logical Mod 去重。revision 追加要么继承既有 Mod 的
                        // 名字（上一段），要么沿用作者在元数据里声明的名字，
                        // 两者都不该被追加序号——那会让同一个 Mod 每更新一版就改名。
                        revision.display_name =
                            self.deduplicated_display_name(&revision.display_name);
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

    /// 让新建 Mod 的展示名在库内唯一，与既有 Mod 冲突时追加序号。
    ///
    /// 展示名自从继承压缩包文件名后就不再天然唯一：同一个 Mod 的两个版本、或不同
    /// 作者的同名作品，导入后在库里是两条无法区分的条目——卡片上除名字外只有版本号，
    /// 而无元数据的包一律显示 v1.0.0。
    ///
    /// 读不到 catalog 时按原名写入。展示名只用于前端展示，不参与身份判定，
    /// 为它中断一次已经解包完成的导入不划算；退化行为就是本次改动之前的样子。
    fn deduplicated_display_name(&self, display_name: &str) -> String {
        let Ok(snapshot) = self.result_repository.catalog_snapshot() else {
            return display_name.to_owned();
        };
        // 必须按用户实际看到的名字比较，也就是 overlay 覆盖后的结果（见库查询里
        // overlay.display_name 对 item.name 的覆写）。拿 catalog 名字去比会两头错：
        // 用户改名造成的重复漏判，而对着一个已被改掉、界面上根本不存在的旧名字
        // 又会凭空加出序号。
        let renamed = self
            .metadata_repository
            .as_ref()
            .and_then(|repository| repository.list_all().ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|overlay| Some((overlay.mod_id, overlay.display_name?)))
            .collect::<BTreeMap<_, _>>();
        let revisions_by_id = snapshot
            .revisions
            .iter()
            .map(|revision| (&revision.revision_id, revision))
            .collect::<BTreeMap<_, _>>();
        // 每个 logical Mod 只贡献一个名字，且只取当前展示的那条 revision：
        // 历史 revision 的旧名字不出现在界面上，让它占用名字会凭空产生序号。
        let taken = snapshot
            .logical_mods
            .iter()
            .filter_map(|logical_mod| {
                renamed.get(&logical_mod.mod_id).cloned().or_else(|| {
                    revisions_by_id
                        .get(&logical_mod.display_revision_id)
                        .map(|revision| revision.display_name.clone())
                })
            })
            .map(|name| normalize_mod_display_name(&name))
            .collect::<BTreeSet<_>>();

        deduplicate_mod_display_name(display_name, |candidate| taken.contains(candidate))
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

pub(crate) struct ModLibrarySnapshotItem {
    pub(crate) item: ModLibraryItem,
    pub(crate) user_category_ids: Vec<String>,
    pub(crate) projection_labels: Vec<ModLibraryProjectionLabel>,
    pub(crate) package_id: String,
    pub(crate) display_revision_id: ModRevisionId,
    pub(crate) stored_preview_image: StoredImportPreviewImage,
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
        Ok(self
            .get_mod_library_snapshot()?
            .into_iter()
            .map(|entry| entry.item)
            .collect())
    }

    pub(crate) fn get_mod_library_snapshot(&self) -> anyhow::Result<Vec<ModLibrarySnapshotItem>> {
        let records = self.result_repository.list_analysis()?;
        let display_revisions = self
            .result_repository
            .list_mods()?
            .into_iter()
            .map(|logical_mod| {
                (
                    logical_mod.mod_id.as_str().to_owned(),
                    logical_mod.display_revision_id,
                )
            })
            .collect::<HashMap<_, _>>();
        let overlays = self.metadata_repository.list_all()?;
        let overlay_map: std::collections::HashMap<_, _> =
            overlays.iter().map(|o| (o.mod_id.as_str(), o)).collect();
        let pairs = self.category_repository.list_mod_category_pairs()?;
        let mut user_cat_id_map: std::collections::HashMap<_, Vec<_>> =
            std::collections::HashMap::new();
        let mut user_projection_label_map: HashMap<String, Vec<ModLibraryProjectionLabel>> =
            HashMap::new();
        for (mod_id, category) in &pairs {
            user_cat_id_map
                .entry(mod_id.clone())
                .or_default()
                .push(category.id.clone());
            user_projection_label_map
                .entry(mod_id.clone())
                .or_default()
                .push(ModLibraryProjectionLabel {
                    category_id: Some(category.id.clone()),
                    name: category.name.clone(),
                    color: category.color.clone(),
                });
        }
        let mut user_cat_map = crate::category::build_user_category_map(pairs);
        Ok(records
            .into_iter()
            .map(|record| {
                let mod_id = record.mod_id.clone();
                let package_id = record.package_id.clone();
                let stored_preview_image = record.preview_image.clone();
                let overlay = overlay_map.get(record.mod_id.as_str()).copied();
                let mut item = library_item_from_stored(record);
                let user_category_ids = user_cat_id_map.remove(&mod_id).unwrap_or_default();
                let mut projection_labels = user_projection_label_map
                    .remove(&mod_id)
                    .unwrap_or_default();
                if let Some(o) = overlay {
                    if let Some(name) = &o.display_name {
                        item.name = name.clone();
                    }
                    if let Some(author) = &o.author {
                        item.author = Some(author.clone());
                    }
                    if let Some(v) = &o.version {
                        item.version_label = Some(format_version_label(v));
                    }
                }
                if let Some(user_cats) = user_cat_map.remove(&mod_id) {
                    let import_labels = std::mem::take(&mut item.category_labels);
                    for import_label in &import_labels {
                        if !projection_labels
                            .iter()
                            .any(|label| label.name == import_label.name)
                        {
                            projection_labels.push(ModLibraryProjectionLabel {
                                category_id: None,
                                name: import_label.name.clone(),
                                color: import_label.color.clone(),
                            });
                        }
                    }
                    item.category_labels =
                        crate::category::merge_category_labels(user_cats, import_labels);
                } else {
                    let import_labels = std::mem::take(&mut item.category_labels);
                    for import_label in &import_labels {
                        projection_labels.push(ModLibraryProjectionLabel {
                            category_id: None,
                            name: import_label.name.clone(),
                            color: import_label.color.clone(),
                        });
                    }
                    item.category_labels = import_labels;
                }
                ModLibrarySnapshotItem {
                    item,
                    user_category_ids,
                    projection_labels,
                    package_id: package_id.clone(),
                    display_revision_id: display_revisions
                        .get(&mod_id)
                        .cloned()
                        .unwrap_or_else(|| ModRevisionId::new(&package_id)),
                    stored_preview_image,
                }
            })
            .collect())
    }

    pub(crate) fn get_mod_library_projection_records(
        &self,
    ) -> anyhow::Result<Vec<ModLibraryProjectionRecord>> {
        Ok(self
            .get_mod_library_snapshot()?
            .into_iter()
            .map(|entry| ModLibraryProjectionRecord {
                mod_id: ModId::new(&entry.item.id),
                display_revision_id: entry.display_revision_id,
                package_id: entry.package_id,
                display_name: entry.item.name,
                author: entry.item.author,
                version_label: entry.item.version_label,
                size_label: entry.item.size_label,
                preview_image: entry.stored_preview_image,
                labels: entry.projection_labels,
            })
            .collect())
    }

    pub(crate) fn category_exists(&self, category_id: &str) -> anyhow::Result<bool> {
        Ok(self.category_repository.get(category_id)?.is_some())
    }

    pub fn get_mod_detail(&self, mod_id: &str) -> anyhow::Result<Option<ModDetail>> {
        let record = self.result_repository.get_analysis(mod_id)?;
        match record {
            Some(record) => {
                let mut detail = detail_from_stored(record);
                if let Some(o) = self.metadata_repository.get(mod_id)? {
                    if let Some(name) = &o.display_name {
                        detail.name = name.clone();
                    }
                    if let Some(author) = &o.author {
                        detail.metadata.author = Some(author.clone());
                    }
                    if let Some(version) = &o.version {
                        detail.metadata.version = Some(version.clone());
                    }
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

pub(crate) fn stored_revision_from_result(
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

pub(crate) fn import_preview_from_stored(
    preview_image: StoredImportPreviewImage,
) -> ImportPreviewImage {
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
                archive_display_name_hint: mod_display_name_from_archive_path(
                    &request.archive_path,
                ),
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

    /// Analyzes an already prepared package through the controlled package-id-to-sandbox seam.
    /// Callers never receive or supply the sandbox filesystem path.
    pub fn analyze_prepared_package_with_cancellation(
        &self,
        task_id: String,
        package_id: String,
        sandbox_locator: &dyn ModImportSandboxLocator,
        cancellation_token: &dyn CancellationToken,
    ) -> anyhow::Result<ModImportAnalysisResult> {
        let sandbox_root = sandbox_locator.sandbox_root_for_package(&package_id)?;
        self.analysis_service.analyze_sandbox_with_cancellation(
            ModImportAnalysisRequest {
                task_id,
                package_id,
                sandbox_root,
                // 这条入口只拿到 package_id 与 sandbox，没有原始归档文件名。
                // 外部导入走这里，它有更好的名称来源（适配器提供的元数据 hint）。
                archive_display_name_hint: None,
            },
            cancellation_token,
        )
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
        // 三级优先：包内元数据声明的名称 → 压缩包文件名 → package_id。
        //
        // 只写 analysis.display_name，绝不回填 metadata.display_name：后者是 revision
        // 继承的判据（见本文件 catalog 保存分支对 metadata.display_name.is_none() 的判断），
        // 把文件名写进去会让 revision 导入用新压缩包的文件名重命名既有 logical Mod。
        //
        // 末端必须是 package_id 而非空串：投影仓储在 display_name 为空时会硬失败
        // 整个写入，而文件名可能净化成 None（纯空白或非 UTF-8 的 stem）。
        let display_name = metadata
            .display_name
            .clone()
            .or_else(|| request.archive_display_name_hint.clone())
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
