use hmm_core::{GameDirectoryStatus, GameId};
use hmm_ports::{
    GameConfigRepository, GameConfigRepositoryError, GameLaunchError, GameLaunchReceipt,
    GameLauncher,
};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GameLaunchServiceError {
    #[error("unsupported game")]
    UnsupportedGame,
    #[error("game is not configured")]
    GameNotConfigured,
    #[error("storage corrupted")]
    StorageCorrupted,
    #[error("storage failed: {0}")]
    StorageFailed(String),
    #[error(transparent)]
    LaunchFailed(#[from] GameLaunchError),
}

pub struct GameLaunchService {
    launchers: Vec<Arc<dyn GameLauncher>>,
    repository: Arc<dyn GameConfigRepository>,
}

impl GameLaunchService {
    pub fn new(
        launchers: Vec<Arc<dyn GameLauncher>>,
        repository: Arc<dyn GameConfigRepository>,
    ) -> Self {
        Self {
            launchers,
            repository,
        }
    }

    pub fn launch_game(
        &self,
        game_id: GameId,
    ) -> Result<GameLaunchReceipt, GameLaunchServiceError> {
        let launcher = self.require_launcher(&game_id)?;
        let instance = self
            .repository
            .load_game_instance(&game_id)
            .map_err(Self::map_storage_error)?
            .ok_or(GameLaunchServiceError::GameNotConfigured)?;

        if instance.status != GameDirectoryStatus::Configured {
            return Err(GameLaunchServiceError::GameNotConfigured);
        }

        launcher
            .launch(&instance)
            .map_err(GameLaunchServiceError::LaunchFailed)
    }

    fn require_launcher(
        &self,
        game_id: &GameId,
    ) -> Result<Arc<dyn GameLauncher>, GameLaunchServiceError> {
        self.launchers
            .iter()
            .find(|launcher| launcher.game_id() == *game_id)
            .cloned()
            .ok_or(GameLaunchServiceError::UnsupportedGame)
    }

    fn map_storage_error(error: GameConfigRepositoryError) -> GameLaunchServiceError {
        match error {
            GameConfigRepositoryError::StorageCorrupted => {
                GameLaunchServiceError::StorageCorrupted
            }
            GameConfigRepositoryError::StorageFailed(message) => {
                GameLaunchServiceError::StorageFailed(message)
            }
        }
    }
}
