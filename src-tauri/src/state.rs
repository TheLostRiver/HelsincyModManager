use hmm_app::{GameSetupService, ModImportTaskService, TaskManager};
use hmm_games_mhw::MonsterHunterWorldAdapter;
use hmm_infra::{
    JsonGameConfigRepository, PlatformSteamRootProvider, RealGameDirectoryProbeFactory,
    SteamGameDiscoveryService, SystemClock,
};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub struct AppState {
    pub game_setup: Arc<GameSetupService>,
    pub mod_import_tasks: Arc<ModImportTaskService>,
}

impl AppState {
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|error| format!("failed to resolve app data dir: {error}"))?;
        let config_path = app_data_dir.join("config").join("games.json");

        let task_manager = Arc::new(TaskManager::new());

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
            mod_import_tasks: Arc::new(ModImportTaskService::new(Arc::clone(&task_manager))),
        })
    }
}
