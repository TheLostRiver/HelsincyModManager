use hmm_core::{GameDirectoryStatus, GameId, GameInstance};
use hmm_games_mhw::MonsterHunterWorldLauncher;
use hmm_ports::{GameLaunchError, GameLaunchMethod, GameLaunchRunner, GameLauncher};
use std::sync::Mutex;

#[derive(Default)]
struct FakeGameLaunchRunner {
    opened_uris: Mutex<Vec<String>>,
}

impl GameLaunchRunner for FakeGameLaunchRunner {
    fn open_uri(&self, uri: &str) -> Result<(), GameLaunchError> {
        self.opened_uris
            .lock()
            .expect("runner lock")
            .push(uri.to_owned());
        Ok(())
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
fn mhw_launcher_opens_steam_protocol_without_launching_real_steam_in_tests() {
    let runner = std::sync::Arc::new(FakeGameLaunchRunner::default());
    let launcher = MonsterHunterWorldLauncher::new(runner.clone());

    let receipt = launcher
        .launch(&configured_mhw_instance())
        .expect("steam protocol launch should be delegated to runner");

    assert_eq!(receipt.game_id, GameId::mhw());
    assert_eq!(receipt.method, GameLaunchMethod::SteamProtocol);
    assert_eq!(
        runner.opened_uris.lock().expect("runner lock").as_slice(),
        ["steam://rungameid/582010"]
    );
}
