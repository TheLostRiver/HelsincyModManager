use hmm_app::GameSetupService;
use hmm_games_mhw::MonsterHunterWorldAdapter;
use hmm_infra::{
    JsonGameConfigRepository, NoopGameDiscoveryService, RealGameDirectoryProbeFactory, SystemClock,
};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

pub struct AppState {
    pub game_setup: Mutex<GameSetupService>,
}

impl AppState {
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|error| format!("failed to resolve app data dir: {error}"))?;
        let config_path = app_data_dir.join("config").join("games.json");

        Ok(Self {
            game_setup: Mutex::new(GameSetupService::new(
                vec![Arc::new(MonsterHunterWorldAdapter)],
                Arc::new(JsonGameConfigRepository::new(config_path)),
                Arc::new(RealGameDirectoryProbeFactory),
                Arc::new(NoopGameDiscoveryService),
                Arc::new(SystemClock),
            )),
        })
    }
}
