mod game_setup;

use anyhow::Result;

pub use game_setup::{
    GameAdapter, GameCandidate, GameCandidateSource, GameConfigRepository,
    GameConfigRepositoryError, GameConfigRepositoryResult, GameDirectoryProbe,
    GameDirectoryProbeFactory, GameDiscoveryError, GameDiscoveryRequest, GameDiscoveryService,
};

pub trait AppClock: Send + Sync {
    fn now_unix_millis(&self) -> Result<u128>;
}
