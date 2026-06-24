use hmm_app::{
    AppSettingsService, GameSetupService, LimitedPreviewImageProcessor, ModImportAnalysisService,
    ModImportPrepareService, ModImportTaskRunner, ModImportTaskService, ModLibraryService,
    PreviewImageCandidateListService, PreviewImageCandidateSelectionService,
    PreviewImageDetailService, PreviewImageDiagnosticsExportService, PreviewImageService,
    TaskManager, ThumbnailCacheMaintenanceScheduler, DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY,
    DEFAULT_THUMBNAIL_CACHE_MAINTENANCE_INTERVAL,
};
use hmm_core::PreviewImagePolicy;
use hmm_games_mhw::MonsterHunterWorldAdapter;
use hmm_infra::{
    FileSystemAuditLogWriter, FileSystemDiagnosticPackageExporter, FileSystemThumbnailStore,
    ImageCratePreviewImageProcessor, JsonAppSettingsRepository, JsonGameConfigRepository,
    JsonModImportResultRepository, PlatformSteamRootProvider, RealGameDirectoryProbeFactory,
    SandboxModPackageMetadataAnalyzer, SandboxPackagePreviewScanner, SteamGameDiscoveryService,
    SystemClock, TaskScopedModImportSandboxLocator, ZipModImportPackagePreparer,
};
use hmm_ports::{
    AppSettingsRepository, AuditLogWriter, DiagnosticPackageExporter, ModImportResultRepository,
    ModImportSandboxLocator, ThumbnailCacheMaintenance,
};
use std::fmt::Display;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub struct AppState {
    pub game_setup: Arc<GameSetupService>,
    pub mod_library: Arc<ModLibraryService>,
    pub preview_image_candidates: Arc<PreviewImageCandidateListService>,
    pub preview_image_selection: Arc<PreviewImageCandidateSelectionService>,
    pub preview_image_detail: Arc<PreviewImageDetailService>,
    pub preview_image_diagnostics_export: Arc<PreviewImageDiagnosticsExportService>,
    pub mod_import_task_runner: Arc<ModImportTaskRunner>,
    pub mod_import_tasks: Arc<ModImportTaskService>,
    pub app_settings: Arc<AppSettingsService>,
    pub task_manager: Arc<TaskManager>,
}

impl AppState {
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|error| format!("failed to resolve app data dir: {error}"))?;
        let config_path = app_data_dir.join("config").join("games.json");
        let settings_path = app_data_dir.join("config").join("settings.json");
        let mod_import_results_path = app_data_dir.join("mod-import").join("results.json");
        let mod_import_sandbox_root = app_data_dir.join("mod-import").join("sandboxes");

        let task_manager = Arc::new(TaskManager::new());
        let mod_import_result_repository: Arc<dyn ModImportResultRepository> =
            Arc::new(JsonModImportResultRepository::new(mod_import_results_path));
        let mod_import_sandbox_locator: Arc<dyn ModImportSandboxLocator> = Arc::new(
            TaskScopedModImportSandboxLocator::new(mod_import_sandbox_root.clone()),
        );
        let thumbnail_cache_maintenance: Arc<dyn ThumbnailCacheMaintenance> =
            Arc::new(FileSystemThumbnailStore::new(app_data_dir.clone()));
        let diagnostic_package_exporter: Arc<dyn DiagnosticPackageExporter> = Arc::new(
            FileSystemDiagnosticPackageExporter::new(app_data_dir.clone()),
        );
        let audit_log: Arc<dyn AuditLogWriter> =
            Arc::new(FileSystemAuditLogWriter::new(app_data_dir.clone()));
        let app_settings_repository: Arc<dyn AppSettingsRepository> =
            Arc::new(JsonAppSettingsRepository::new(settings_path));
        let app_settings = Arc::new(AppSettingsService::new(Arc::clone(
            &app_settings_repository,
        )));
        let preview_image_service = PreviewImageService::new(
            PreviewImagePolicy::default(),
            Box::new(SandboxPackagePreviewScanner),
            Box::new(LimitedPreviewImageProcessor::new(
                Box::new(ImageCratePreviewImageProcessor::new(Box::new(
                    FileSystemThumbnailStore::new(app_data_dir.clone()),
                ))),
                DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY,
            )),
        );
        let mod_import_prepare_service = Arc::new(ModImportPrepareService::new(
            Box::new(ZipModImportPackagePreparer::new(mod_import_sandbox_root)),
            ModImportAnalysisService::new(
                Box::new(preview_image_service),
                Box::new(FileSystemThumbnailStore::new(app_data_dir.clone())),
                Box::new(SandboxModPackageMetadataAnalyzer),
            ),
        ));
        let mod_library = Arc::new(ModLibraryService::new(Arc::clone(
            &mod_import_result_repository,
        )));
        let preview_image_candidates = Arc::new(PreviewImageCandidateListService::new(
            PreviewImagePolicy::default(),
            Arc::clone(&mod_import_result_repository),
            Arc::clone(&mod_import_sandbox_locator),
            Box::new(SandboxPackagePreviewScanner),
        ));
        let preview_image_selection = Arc::new(PreviewImageCandidateSelectionService::new(
            PreviewImagePolicy::default(),
            Arc::clone(&mod_import_result_repository),
            Arc::clone(&mod_import_sandbox_locator),
            Box::new(SandboxPackagePreviewScanner),
            Box::new(LimitedPreviewImageProcessor::new(
                Box::new(ImageCratePreviewImageProcessor::new(Box::new(
                    FileSystemThumbnailStore::new(app_data_dir.clone()),
                ))),
                DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY,
            )),
            Box::new(FileSystemThumbnailStore::new(app_data_dir.clone())),
        ));
        let preview_image_detail = Arc::new(PreviewImageDetailService::new(
            PreviewImagePolicy {
                output_max_edge_px: 1024,
                ..PreviewImagePolicy::default()
            },
            Arc::clone(&mod_import_result_repository),
            Arc::clone(&mod_import_sandbox_locator),
            Box::new(SandboxPackagePreviewScanner),
            Box::new(LimitedPreviewImageProcessor::new(
                Box::new(ImageCratePreviewImageProcessor::new(Box::new(
                    FileSystemThumbnailStore::new(app_data_dir.clone()),
                ))),
                DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY,
            )),
            Box::new(FileSystemThumbnailStore::new(app_data_dir.clone())),
        ));
        let preview_image_diagnostics_export = Arc::new(PreviewImageDiagnosticsExportService::new(
            Arc::clone(&mod_import_result_repository),
            diagnostic_package_exporter,
            audit_log,
            Arc::new(SystemClock),
        ));
        let mod_import_task_runner = Arc::new(
            ModImportTaskRunner::new(
                Arc::clone(&task_manager),
                mod_import_prepare_service,
                Arc::clone(&mod_import_result_repository),
            )
            .with_thumbnail_cache_maintenance(thumbnail_cache_maintenance)
            .with_app_settings_repository(app_settings_repository),
        );
        let thumbnail_cache_scheduler = ThumbnailCacheMaintenanceScheduler::new(
            Arc::clone(&mod_import_task_runner),
            DEFAULT_THUMBNAIL_CACHE_MAINTENANCE_INTERVAL,
        );
        start_best_effort_background_task("thumbnail-cache-maintenance", || {
            std::thread::Builder::new()
                .name("thumbnail-cache-maintenance".to_owned())
                .spawn(move || thumbnail_cache_scheduler.run_forever())
                .map(|_| ())
        });

        Ok(Self {
            game_setup: Arc::new(GameSetupService::new(
                vec![Arc::new(MonsterHunterWorldAdapter)],
                Arc::new(JsonGameConfigRepository::new(config_path)),
                Arc::new(RealGameDirectoryProbeFactory),
                Arc::new(SteamGameDiscoveryService::new(Arc::new(
                    PlatformSteamRootProvider,
                ))),
                Arc::new(SystemClock),
            )),
            mod_library,
            preview_image_candidates,
            preview_image_selection,
            preview_image_detail,
            preview_image_diagnostics_export,
            mod_import_task_runner,
            mod_import_tasks: Arc::new(ModImportTaskService::new(Arc::clone(&task_manager))),
            app_settings,
            task_manager,
        })
    }
}

fn start_best_effort_background_task<E>(
    task_name: &'static str,
    spawn: impl FnOnce() -> Result<(), E>,
) where
    E: Display,
{
    if let Err(error) = spawn() {
        tracing::warn!(
            task = task_name,
            error = %error,
            "failed to start best-effort background task"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn best_effort_background_task_start_ignores_spawn_failure() {
        start_best_effort_background_task("thumbnail-cache-maintenance", || -> Result<(), &str> {
            Err("spawn failed")
        });
    }
}
