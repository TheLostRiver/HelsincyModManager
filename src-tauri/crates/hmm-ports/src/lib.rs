mod game_setup;

use anyhow::Result;

pub use game_setup::{
    GameAdapter, GameCandidate, GameConfigRepository, GameConfigRepositoryError,
    GameConfigRepositoryResult, GameDirectoryProbe, GameDirectoryProbeFactory, GameDiscoveryError,
    GameDiscoveryService,
};

pub trait AppClock: Send + Sync {
    fn now_unix_millis(&self) -> Result<u128>;
}
