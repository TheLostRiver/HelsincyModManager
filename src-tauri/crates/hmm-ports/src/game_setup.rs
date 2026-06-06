use hmm_core::{GameDirectoryValidation, GameId, GameInstance};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GameConfigRepositoryError {
    #[error("storage corrupted")]
    StorageCorrupted,
    #[error("storage failed: {0}")]
    StorageFailed(String),
}

pub type GameConfigRepositoryResult<T> = Result<T, GameConfigRepositoryError>;

pub trait GameDirectoryProbe: Send + Sync {
    fn root_dir(&self) -> &Path;
    fn root_exists(&self) -> bool;
    fn exists(&self, relative_path: &str) -> bool;
    fn is_file(&self, relative_path: &str) -> bool;
    fn is_dir(&self, relative_path: &str) -> bool;
}

pub trait GameDirectoryProbeFactory: Send + Sync {
    fn create(&self, directory: PathBuf) -> Box<dyn GameDirectoryProbe>;
}

pub trait GameAdapter: Send + Sync {
    fn game_id(&self) -> GameId;
    fn display_name(&self) -> &'static str;
    fn validate_directory(&self, probe: &dyn GameDirectoryProbe) -> GameDirectoryValidation;

    fn steam_app_id(&self) -> Option<u32> {
        None
    }
}

pub trait GameConfigRepository: Send + Sync {
    fn load_game_instance(
        &self,
        game_id: &GameId,
    ) -> GameConfigRepositoryResult<Option<GameInstance>>;

    fn save_game_instance(&self, instance: &GameInstance) -> GameConfigRepositoryResult<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDiscoveryRequest {
    pub game_id: GameId,
    pub display_name: String,
    pub steam_app_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameCandidateSource {
    Steam,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameCandidate {
    pub game_id: GameId,
    pub display_name: String,
    pub root_dir: PathBuf,
    pub source: GameCandidateSource,
    pub source_label: String,
}

pub trait GameDiscoveryService: Send + Sync {
    fn scan_candidates(
        &self,
        request: &GameDiscoveryRequest,
    ) -> Result<Vec<GameCandidate>, GameDiscoveryError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GameDiscoveryError {
    #[error("scan not implemented")]
    ScanNotImplemented,
    #[error("scan failed: {0}")]
    ScanFailed(String),
}
