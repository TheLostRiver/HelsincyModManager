use hmm_app::{
    GameSetupService, ModImportAnalysisService, ModImportPrepareService, ModImportTaskRunner,
    ModImportTaskService, PreviewImageService, TaskManager,
};
use hmm_core::PreviewImagePolicy;
use hmm_games_mhw::MonsterHunterWorldAdapter;
use hmm_infra::{
    FileSystemThumbnailStore, ImageCratePreviewImageProcessor, JsonGameConfigRepository,
    PlatformSteamRootProvider, RealGameDirectoryProbeFactory, SandboxPackagePreviewScanner,
    SteamGameDiscoveryService, SystemClock, ZipModImportPackagePreparer,
};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub struct AppState {
    pub game_setup: Arc<GameSetupService>,
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

        let task_manager = Arc::new(TaskManager::new());
        let preview_image_service = PreviewImageService::new(
            PreviewImagePolicy::default(),
            Box::new(SandboxPackagePreviewScanner),
            Box::new(ImageCratePreviewImageProcessor::new(Box::new(
                FileSystemThumbnailStore::new(app_data_dir.clone()),
            ))),
        );
        let mod_import_prepare_service = Arc::new(ModImportPrepareService::new(
            Box::new(ZipModImportPackagePreparer::new(
                app_data_dir.join("mod-import").join("sandboxes"),
            )),
            ModImportAnalysisService::new(
                Box::new(preview_image_service),
                Box::new(FileSystemThumbnailStore::new(app_data_dir.clone())),
            ),
        ));

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
            mod_import_task_runner: Arc::new(ModImportTaskRunner::new(
                Arc::clone(&task_manager),
                mod_import_prepare_service,
            )),
            mod_import_tasks: Arc::new(ModImportTaskService::new(Arc::clone(&task_manager))),
            task_manager,
        })
    }
}
