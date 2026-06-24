use hmm_app::{
    GameSetupService, LimitedPreviewImageProcessor, ModImportAnalysisService,
    ModImportPrepareService, ModImportTaskRunner, ModImportTaskService, ModLibraryService,
    PreviewImageService, TaskManager, ThumbnailCacheMaintenanceScheduler,
    DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY, DEFAULT_THUMBNAIL_CACHE_MAINTENANCE_INTERVAL,
};
use hmm_core::PreviewImagePolicy;
use hmm_games_mhw::MonsterHunterWorldAdapter;
use hmm_infra::{
    FileSystemThumbnailStore, ImageCratePreviewImageProcessor, JsonAppSettingsRepository,
    JsonGameConfigRepository, JsonModImportResultRepository, PlatformSteamRootProvider,
    RealGameDirectoryProbeFactory, SandboxModPackageMetadataAnalyzer, SandboxPackagePreviewScanner,
    SteamGameDiscoveryService, SystemClock, ZipModImportPackagePreparer,
};
use hmm_ports::{AppSettingsRepository, ModImportResultRepository, ThumbnailCacheMaintenance};
use std::fmt::Display;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub struct AppState {
    pub game_setup: Arc<GameSetupService>,
    pub mod_library: Arc<ModLibraryService>,
    pub mod_import_task_runner: Arc<ModImportTaskRunner>,
    pub mod_import_tasks: Arc<ModImportTaskService>,
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

        let task_manager = Arc::new(TaskManager::new());
        let mod_import_result_repository: Arc<dyn ModImportResultRepository> =
            Arc::new(JsonModImportResultRepository::new(mod_import_results_path));
        let thumbnail_cache_maintenance: Arc<dyn ThumbnailCacheMaintenance> =
            Arc::new(FileSystemThumbnailStore::new(app_data_dir.clone()));
        let app_settings_repository: Arc<dyn AppSettingsRepository> =
            Arc::new(JsonAppSettingsRepository::new(settings_path));
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
            Box::new(ZipModImportPackagePreparer::new(
                app_data_dir.join("mod-import").join("sandboxes"),
            )),
            ModImportAnalysisService::new(
                Box::new(preview_image_service),
                Box::new(FileSystemThumbnailStore::new(app_data_dir.clone())),
                Box::new(SandboxModPackageMetadataAnalyzer),
            ),
        ));
        let mod_library = Arc::new(ModLibraryService::new(Arc::clone(
            &mod_import_result_repository,
        )));
        let mod_import_task_runner = Arc::new(
            ModImportTaskRunner::new(
                Arc::clone(&task_manager),
                mod_import_prepare_service,
                mod_import_result_repository,
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
            mod_import_task_runner,
            mod_import_tasks: Arc::new(ModImportTaskService::new(Arc::clone(&task_manager))),
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
