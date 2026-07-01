use hmm_core::{GameId, GameInstance};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameLaunchMethod {
    SteamProtocol,
    DirectExecutable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameLaunchReceipt {
    pub game_id: GameId,
    pub method: GameLaunchMethod,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GameLaunchError {
    #[error("launcher unavailable: {0}")]
    LauncherUnavailable(String),
    #[error("launch failed: {0}")]
    LaunchFailed(String),
}

pub trait GameLaunchRunner: Send + Sync {
    fn open_uri(&self, uri: &str) -> Result<(), GameLaunchError>;
}

pub trait GameLauncher: Send + Sync {
    fn game_id(&self) -> GameId;
    fn launch(&self, instance: &GameInstance) -> Result<GameLaunchReceipt, GameLaunchError>;
}
