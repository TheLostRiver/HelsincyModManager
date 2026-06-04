use hmm_core::{GameId, GameInstance};
use hmm_ports::{GameConfigRepository, GameConfigRepositoryError, GameConfigRepositoryResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
struct GamesConfigFile {
    version: u32,
    games: Vec<GameInstance>,
}

impl Default for GamesConfigFile {
    fn default() -> Self {
        Self {
            version: 1,
            games: Vec::new(),
        }
    }
}

pub struct JsonGameConfigRepository {
    file_path: PathBuf,
}

impl JsonGameConfigRepository {
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    fn load_file(&self) -> GameConfigRepositoryResult<GamesConfigFile> {
        if !self.file_path.exists() {
            return Ok(GamesConfigFile::default());
        }

        let content = fs::read_to_string(&self.file_path)
            .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;

        serde_json::from_str(&content).map_err(|_| GameConfigRepositoryError::StorageCorrupted)
    }

    fn save_file(&self, config: &GamesConfigFile) -> GameConfigRepositoryResult<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;
        }

        let serialized = serde_json::to_string_pretty(config)
            .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;
        let temp_path = self.file_path.with_extension("json.tmp");

        fs::write(&temp_path, serialized)
            .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;
        fs::rename(&temp_path, &self.file_path)
            .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;

        Ok(())
    }
}

impl GameConfigRepository for JsonGameConfigRepository {
    fn load_game_instance(
        &self,
        game_id: &GameId,
    ) -> GameConfigRepositoryResult<Option<GameInstance>> {
        let config = self.load_file()?;
        Ok(config
            .games
            .into_iter()
            .find(|instance| instance.game_id == *game_id))
    }

    fn save_game_instance(&self, instance: &GameInstance) -> GameConfigRepositoryResult<()> {
        let mut config = self.load_file()?;
        config.games.retain(|item| item.game_id != instance.game_id);
        config.games.push(instance.clone());
        self.save_file(&config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{GameDirectoryStatus, GameId};

    fn test_file(name: &str) -> PathBuf {
        let unique = format!(
            "hmm-json-repo-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        std::env::temp_dir()
            .join(unique)
            .join("config")
            .join("games.json")
    }

    fn instance(root: &str) -> GameInstance {
        GameInstance {
            id: "mhw-default".to_owned(),
            game_id: GameId::mhw(),
            display_name: "Monster Hunter: World - Iceborne".to_owned(),
            root_dir: PathBuf::from(root),
            status: GameDirectoryStatus::Configured,
            configured_at_unix_millis: 42,
        }
    }

    #[test]
    fn missing_file_loads_empty_config() {
        let repo = JsonGameConfigRepository::new(test_file("missing"));

        let loaded = repo
            .load_game_instance(&GameId::mhw())
            .expect("load should succeed");

        assert!(loaded.is_none());
    }

    #[test]
    fn save_creates_parent_directory_and_loads_instance() {
        let path = test_file("save");
        let repo = JsonGameConfigRepository::new(path);

        repo.save_game_instance(&instance("C:/MHW"))
            .expect("save should succeed");
        let loaded = repo
            .load_game_instance(&GameId::mhw())
            .expect("load should succeed");

        assert_eq!(loaded.expect("instance").root_dir, PathBuf::from("C:/MHW"));
    }

    #[test]
    fn corrupted_json_returns_storage_corrupted() {
        let path = test_file("corrupted");
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(&path, "{ broken json").expect("write broken file");
        let repo = JsonGameConfigRepository::new(path);

        let error = repo
            .load_game_instance(&GameId::mhw())
            .expect_err("broken json should fail");

        assert_eq!(error, GameConfigRepositoryError::StorageCorrupted);
    }

    #[test]
    fn save_replaces_existing_game_instance() {
        let path = test_file("replace");
        let repo = JsonGameConfigRepository::new(path);

        repo.save_game_instance(&instance("C:/Old"))
            .expect("first save");
        repo.save_game_instance(&instance("D:/New"))
            .expect("second save");
        let loaded = repo
            .load_game_instance(&GameId::mhw())
            .expect("load should succeed");

        assert_eq!(loaded.expect("instance").root_dir, PathBuf::from("D:/New"));
    }
}
