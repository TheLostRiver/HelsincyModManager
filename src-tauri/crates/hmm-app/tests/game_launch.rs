use hmm_app::{GameLaunchService, GameLaunchServiceError};
use hmm_core::{GameDirectoryStatus, GameId, GameInstance};
use hmm_ports::{
    GameConfigRepository, GameConfigRepositoryError, GameConfigRepositoryResult, GameLaunchError,
    GameLaunchMethod, GameLaunchReceipt, GameLauncher,
};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeGameConfigRepository {
    instance: Mutex<Option<GameInstance>>,
}

impl FakeGameConfigRepository {
    fn with_instance(instance: GameInstance) -> Self {
        Self {
            instance: Mutex::new(Some(instance)),
        }
    }
}

impl GameConfigRepository for FakeGameConfigRepository {
    fn load_game_instance(
        &self,
        _game_id: &GameId,
    ) -> GameConfigRepositoryResult<Option<GameInstance>> {
        Ok(self.instance.lock().expect("repo lock").clone())
    }

    fn save_game_instance(&self, _instance: &GameInstance) -> GameConfigRepositoryResult<()> {
        Err(GameConfigRepositoryError::StorageFailed(
            "not used in launch tests".to_owned(),
        ))
    }
}

#[derive(Default)]
struct FakeGameLauncher {
    launched_instances: Mutex<Vec<String>>,
}

impl GameLauncher for FakeGameLauncher {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn launch(&self, instance: &GameInstance) -> Result<GameLaunchReceipt, GameLaunchError> {
        self.launched_instances
            .lock()
            .expect("launcher lock")
            .push(instance.id.clone());
        Ok(GameLaunchReceipt {
            game_id: instance.game_id.clone(),
            method: GameLaunchMethod::SteamProtocol,
        })
    }
}

fn configured_mhw_instance() -> GameInstance {
    GameInstance {
        id: "mhw-default".to_owned(),
        game_id: GameId::mhw(),
        display_name: "Monster Hunter: World - Iceborne".to_owned(),
        root_dir: "C:/Games/Monster Hunter World".into(),
        status: GameDirectoryStatus::Configured,
        configured_at_unix_millis: 1,
    }
}

#[test]
fn launch_game_uses_configured_instance_and_launcher_port() {
    let launcher = Arc::new(FakeGameLauncher::default());
    let service = GameLaunchService::new(
        vec![launcher.clone()],
        Arc::new(FakeGameConfigRepository::with_instance(configured_mhw_instance())),
    );

    let receipt = service
        .launch_game(GameId::mhw())
        .expect("configured game should launch");

    assert_eq!(receipt.game_id, GameId::mhw());
    assert_eq!(receipt.method, GameLaunchMethod::SteamProtocol);
    assert_eq!(
        launcher.launched_instances.lock().expect("launcher lock").as_slice(),
        ["mhw-default"]
    );
}

#[test]
fn launch_game_rejects_missing_configuration_without_calling_launcher() {
    let launcher = Arc::new(FakeGameLauncher::default());
    let service = GameLaunchService::new(
        vec![launcher.clone()],
        Arc::new(FakeGameConfigRepository::default()),
    );

    let error = service
        .launch_game(GameId::mhw())
        .expect_err("missing game setup must block launch");

    assert!(matches!(error, GameLaunchServiceError::GameNotConfigured));
    assert!(launcher
        .launched_instances
        .lock()
        .expect("launcher lock")
        .is_empty());
}
