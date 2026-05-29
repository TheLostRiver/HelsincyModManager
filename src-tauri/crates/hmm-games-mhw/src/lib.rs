use hmm_core::GameId;
use hmm_ports::GameAdapter;

pub struct MonsterHunterWorldAdapter;

impl GameAdapter for MonsterHunterWorldAdapter {
    fn game_id(&self) -> GameId {
        GameId::new("mhw")
    }

    fn display_name(&self) -> &'static str {
        "Monster Hunter: World - Iceborne"
    }
}

#[cfg(test)]
mod tests {
    use super::MonsterHunterWorldAdapter;
    use hmm_ports::GameAdapter;

    #[test]
    fn adapter_reports_game_id() {
        let adapter = MonsterHunterWorldAdapter;
        assert_eq!(adapter.game_id().as_str(), "mhw");
    }
}
