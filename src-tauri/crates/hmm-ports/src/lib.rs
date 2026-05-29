use anyhow::Result;
use hmm_core::GameId;

pub trait GameAdapter {
    fn game_id(&self) -> GameId;
    fn display_name(&self) -> &'static str;
}

pub trait AppClock {
    fn now_unix_millis(&self) -> Result<u128>;
}
