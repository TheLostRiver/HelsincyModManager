use fs2::FileExt;
use hmm_core::{GameId, GameInstance};
use hmm_ports::{GameConfigRepository, GameConfigRepositoryError, GameConfigRepositoryResult};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct GamesConfigFile {
    version: u32,
    games: Vec<GameInstance>,
}

impl Default for GamesConfigFile {
    fn default() -> Self {
        Self {
            version: CURRENT_SCHEMA_VERSION,
            games: Vec::new(),
        }
    }
}

pub struct JsonGameConfigRepository {
    file_path: PathBuf,
    write_lock: Mutex<()>,
    #[cfg(test)]
    parent_directory_synced: AtomicBool,
}

impl JsonGameConfigRepository {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            write_lock: Mutex::new(()),
            #[cfg(test)]
            parent_directory_synced: AtomicBool::new(false),
        }
    }

    fn load_file(&self) -> GameConfigRepositoryResult<GamesConfigFile> {
        if !self.file_path.exists() {
            return Ok(GamesConfigFile::default());
        }

        let bytes = fs::read(&self.file_path)
            .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;
        let content = String::from_utf8(bytes).map_err(|_| GameConfigRepositoryError::StorageCorrupted)?;

        let config: GamesConfigFile =
            serde_json::from_str(&content).map_err(|_| GameConfigRepositoryError::StorageCorrupted)?;

        if config.version != CURRENT_SCHEMA_VERSION {
            return Err(GameConfigRepositoryError::StorageCorrupted);
        }

        Ok(config)
    }

    fn save_file(&self, config: &GamesConfigFile) -> GameConfigRepositoryResult<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;
        }

        let serialized = serde_json::to_string_pretty(config)
            .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;
        let temp_path = self.unique_temp_path();

        {
            let mut temp_file = File::create(&temp_path)
                .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;
            temp_file
                .write_all(serialized.as_bytes())
                .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;
            temp_file
                .sync_all()
                .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;
        }

        fs::rename(&temp_path, &self.file_path)
            .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;
        self.sync_parent_directory()?;

        Ok(())
    }

    fn sync_parent_directory(&self) -> GameConfigRepositoryResult<()> {
        let Some(parent) = self.file_path.parent() else {
            return Ok(());
        };

        open_directory_for_sync(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;

        #[cfg(test)]
        self.parent_directory_synced.store(true, Ordering::SeqCst);

        Ok(())
    }

    #[cfg(test)]
    fn parent_directory_synced_for_test(&self) -> bool {
        self.parent_directory_synced.load(Ordering::SeqCst)
    }

    fn lock_file_path(&self) -> PathBuf {
        let lock_name = self
            .file_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("{name}.lock"))
            .unwrap_or_else(|| "games.json.lock".to_owned());

        self.file_path
            .parent()
            .map(|parent| parent.join(&lock_name))
            .unwrap_or_else(|| PathBuf::from(lock_name))
    }

    fn unique_temp_path(&self) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let temp_name = self
            .file_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("{name}.{}.{}.tmp", std::process::id(), nonce))
            .unwrap_or_else(|| format!("games.{}.{}.json.tmp", std::process::id(), nonce));

        self.file_path
            .parent()
            .map(|parent| parent.join(&temp_name))
            .unwrap_or_else(|| PathBuf::from(temp_name))
    }

    fn open_lock_file(&self) -> GameConfigRepositoryResult<File> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;
        }

        OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(self.lock_file_path())
            .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))
    }
}

#[cfg(windows)]
fn open_directory_for_sync(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;

    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

#[cfg(not(windows))]
fn open_directory_for_sync(path: &Path) -> std::io::Result<File> {
    File::open(path)
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
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| GameConfigRepositoryError::StorageFailed("write lock poisoned".to_owned()))?;
        let lock_file = self.open_lock_file()?;
        lock_file
            .lock_exclusive()
            .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;

        let mut config = self.load_file()?;
        config.games.retain(|item| item.game_id != instance.game_id);
        config.games.push(instance.clone());
        let result = self.save_file(&config);
        let unlock_result = lock_file
            .unlock()
            .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()));

        result.and(unlock_result)
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
    fn non_utf8_config_returns_storage_corrupted() {
        let path = test_file("non-utf8");
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(&path, [0xff, 0xfe, 0xfd]).expect("write non utf8 file");
        let repo = JsonGameConfigRepository::new(path);

        let error = repo
            .load_game_instance(&GameId::mhw())
            .expect_err("non utf8 config should fail");

        assert_eq!(error, GameConfigRepositoryError::StorageCorrupted);
    }

    #[test]
    fn unsupported_schema_version_returns_storage_corrupted() {
        let path = test_file("unsupported-version");
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(&path, r#"{"version":999,"games":[]}"#).expect("write unsupported version");
        let repo = JsonGameConfigRepository::new(path);

        let error = repo
            .load_game_instance(&GameId::mhw())
            .expect_err("unsupported version should fail");

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

    #[test]
    fn save_syncs_parent_directory_after_rename() {
        let path = test_file("sync-parent");
        let repo = JsonGameConfigRepository::new(path.clone());

        repo.save_game_instance(&instance("C:/MHW"))
            .expect("save should succeed");

        assert!(repo.parent_directory_synced_for_test());
        assert!(path.exists());
    }

    #[test]
    fn concurrent_saves_are_serialized() {
        let path = test_file("concurrent");
        let repo = std::sync::Arc::new(JsonGameConfigRepository::new(path));
        let start = std::sync::Arc::new(std::sync::Barrier::new(8));
        let mut handles = Vec::new();

        for index in 0..8 {
            let repo = std::sync::Arc::clone(&repo);
            let start = std::sync::Arc::clone(&start);
            handles.push(std::thread::spawn(move || {
                start.wait();
                repo.save_game_instance(&instance(&format!("C:/MHW-{index}")))
            }));
        }

        for handle in handles {
            handle
                .join()
                .expect("worker should not panic")
                .expect("save should not fail");
        }

        assert!(repo
            .load_game_instance(&GameId::mhw())
            .expect("load should succeed")
            .is_some());
    }
}
