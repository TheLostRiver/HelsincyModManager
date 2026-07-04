use hmm_core::GameId;
use hmm_ports::GameSaveDirectoryRule;

pub struct MonsterHunterWorldSaveDirectoryRule;

impl GameSaveDirectoryRule for MonsterHunterWorldSaveDirectoryRule {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn steam_app_id(&self) -> u32 {
        582010
    }

    fn steam_remote_relative_path(&self) -> &'static str {
        "582010/remote"
    }

    fn known_save_file_names(&self) -> &'static [&'static str] {
        &["SAVEDATA1000"]
    }

    fn path_label(&self) -> &'static str {
        "Steam/userdata/<account>/582010/remote"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mhw_save_rule_points_to_steam_remote_directory() {
        let rule = MonsterHunterWorldSaveDirectoryRule;

        assert_eq!(rule.game_id().as_str(), "mhw");
        assert_eq!(rule.steam_app_id(), 582010);
        assert_eq!(rule.steam_remote_relative_path(), "582010/remote");
        assert_eq!(rule.known_save_file_names(), &["SAVEDATA1000"]);
        assert_eq!(rule.path_label(), "Steam/userdata/<account>/582010/remote");
    }
}
