mod game_config_repository;
mod game_directory_probe;
mod game_discovery;
mod steam_discovery;

use anyhow::Result;
use hmm_ports::AppClock;
use std::time::{SystemTime, UNIX_EPOCH};

pub use game_config_repository::JsonGameConfigRepository;
pub use game_directory_probe::{RealGameDirectoryProbe, RealGameDirectoryProbeFactory};
pub use game_discovery::{NoopGameDiscoveryService, SteamGameDiscoveryService};
pub use steam_discovery::PlatformSteamRootProvider;

pub struct SystemClock;

impl AppClock for SystemClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
    }
}
