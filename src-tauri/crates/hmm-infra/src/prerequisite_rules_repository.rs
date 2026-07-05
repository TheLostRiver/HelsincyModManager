use fs2::FileExt;
use hmm_core::GameId;
use hmm_ports::{
    GamePrerequisiteRuleRepository, GamePrerequisiteRuleRepositoryError, GamePrerequisiteRuleSet,
};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const CURRENT_SCHEMA_VERSION: u32 = 1;

pub struct JsonGamePrerequisiteRuleRepository {
    file_path: PathBuf,
    write_lock: Mutex<()>,
}

impl JsonGamePrerequisiteRuleRepository {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            write_lock: Mutex::new(()),
        }
    }

    fn ensure_seeded(&self, bundled_default: &str) -> Result<(), GamePrerequisiteRuleRepositoryError> {
        if self.file_path.exists() {
            return Ok(());
        }

        let _guard = self.write_lock.lock().map_err(|_| {
            GamePrerequisiteRuleRepositoryError::StorageFailed("write lock poisoned".to_owned())
        })?;
        let lock_file = self.open_lock_file()?;
        lock_file
            .lock_exclusive()
            .map_err(|error| GamePrerequisiteRuleRepositoryError::StorageFailed(error.to_string()))?;

        if !self.file_path.exists() {
            self.save_default_file(bundled_default)?;
        }

        lock_file
            .unlock()
            .map_err(|error| GamePrerequisiteRuleRepositoryError::StorageFailed(error.to_string()))
    }

    fn save_default_file(
        &self,
        bundled_default: &str,
    ) -> Result<(), GamePrerequisiteRuleRepositoryError> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| GamePrerequisiteRuleRepositoryError::StorageFailed(error.to_string()))?;
        }

        let temp_path = self.unique_temp_path();

        {
            let mut temp_file = File::create(&temp_path)
                .map_err(|error| GamePrerequisiteRuleRepositoryError::StorageFailed(error.to_string()))?;
            temp_file
                .write_all(bundled_default.as_bytes())
                .map_err(|error| GamePrerequisiteRuleRepositoryError::StorageFailed(error.to_string()))?;
            temp_file
                .sync_all()
                .map_err(|error| GamePrerequisiteRuleRepositoryError::StorageFailed(error.to_string()))?;
        }

        fs::rename(&temp_path, &self.file_path)
            .map_err(|error| GamePrerequisiteRuleRepositoryError::StorageFailed(error.to_string()))?;
        self.sync_parent_directory()?;

        Ok(())
    }

    fn load_file(
        &self,
        game_id: &GameId,
    ) -> Result<GamePrerequisiteRuleSet, GamePrerequisiteRuleRepositoryError> {
        let bytes = fs::read(&self.file_path)
            .map_err(|error| GamePrerequisiteRuleRepositoryError::StorageFailed(error.to_string()))?;
        let content = String::from_utf8(bytes)
            .map_err(|_| GamePrerequisiteRuleRepositoryError::StorageCorrupted)?;
        let rules: GamePrerequisiteRuleSet = serde_json::from_str(&content)
            .map_err(|_| GamePrerequisiteRuleRepositoryError::StorageCorrupted)?;

        if rules.version != CURRENT_SCHEMA_VERSION || &rules.game_id != game_id {
            return Err(GamePrerequisiteRuleRepositoryError::StorageCorrupted);
        }

        Ok(rules)
    }

    fn sync_parent_directory(&self) -> Result<(), GamePrerequisiteRuleRepositoryError> {
        let Some(parent) = self.file_path.parent() else {
            return Ok(());
        };

        open_directory_for_sync(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| GamePrerequisiteRuleRepositoryError::StorageFailed(error.to_string()))
    }

    fn lock_file_path(&self) -> PathBuf {
        let lock_name = self
            .file_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("{name}.lock"))
            .unwrap_or_else(|| "prerequisite-rules.json.lock".to_owned());

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
            .unwrap_or_else(|| {
                format!(
                    "prerequisite-rules.{}.{}.json.tmp",
                    std::process::id(),
                    nonce
                )
            });

        self.file_path
            .parent()
            .map(|parent| parent.join(&temp_name))
            .unwrap_or_else(|| PathBuf::from(temp_name))
    }

    fn open_lock_file(&self) -> Result<File, GamePrerequisiteRuleRepositoryError> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| GamePrerequisiteRuleRepositoryError::StorageFailed(error.to_string()))?;
        }

        OpenOptions::new()
            .create(true)
            .read(true)
            .truncate(false)
            .write(true)
            .open(self.lock_file_path())
            .map_err(|error| GamePrerequisiteRuleRepositoryError::StorageFailed(error.to_string()))
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

impl GamePrerequisiteRuleRepository for JsonGamePrerequisiteRuleRepository {
    fn load_rules(
        &self,
        game_id: &GameId,
        bundled_default: &str,
    ) -> Result<GamePrerequisiteRuleSet, GamePrerequisiteRuleRepositoryError> {
        self.ensure_seeded(bundled_default)?;
        self.load_file(game_id)
    }
}
