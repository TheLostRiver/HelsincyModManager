use hmm_ports::{
    AppSettings, AppSettingsRepository, AppSettingsRepositoryError, AppSettingsRepositoryResult,
};
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::fs::OpenOptions;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettingsFile {
    version: u32,
    #[serde(default)]
    thumbnail_cache_max_bytes: Option<u64>,
    #[serde(default)]
    thumbnail_cache_max_age_days: Option<u32>,
    #[serde(default)]
    log_storage_max_bytes: Option<u64>,
    #[serde(default)]
    debug_log_enabled: bool,
}

impl Default for AppSettingsFile {
    fn default() -> Self {
        Self {
            version: CURRENT_SCHEMA_VERSION,
            thumbnail_cache_max_bytes: None,
            thumbnail_cache_max_age_days: None,
            log_storage_max_bytes: None,
            debug_log_enabled: false,
        }
    }
}

pub struct JsonAppSettingsRepository {
    file_path: PathBuf,
    write_lock: Mutex<()>,
}

impl JsonAppSettingsRepository {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            write_lock: Mutex::new(()),
        }
    }

    fn load_file(&self) -> AppSettingsRepositoryResult<AppSettingsFile> {
        if !self.file_path.exists() {
            return Ok(AppSettingsFile::default());
        }

        let bytes = fs::read(&self.file_path)
            .map_err(|error| AppSettingsRepositoryError::StorageFailed(error.to_string()))?;
        let content =
            String::from_utf8(bytes).map_err(|_| AppSettingsRepositoryError::StorageCorrupted)?;
        let config: AppSettingsFile = serde_json::from_str(&content)
            .map_err(|_| AppSettingsRepositoryError::StorageCorrupted)?;

        if config.version != CURRENT_SCHEMA_VERSION {
            return Err(AppSettingsRepositoryError::StorageCorrupted);
        }

        Ok(config)
    }

    fn save_file(&self, config: &AppSettingsFile) -> AppSettingsRepositoryResult<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| AppSettingsRepositoryError::StorageFailed(error.to_string()))?;
        }

        let serialized = serde_json::to_string_pretty(config)
            .map_err(|error| AppSettingsRepositoryError::StorageFailed(error.to_string()))?;
        let temp_path = self.unique_temp_path();

        {
            let mut temp_file = File::create(&temp_path)
                .map_err(|error| AppSettingsRepositoryError::StorageFailed(error.to_string()))?;
            temp_file
                .write_all(serialized.as_bytes())
                .map_err(|error| AppSettingsRepositoryError::StorageFailed(error.to_string()))?;
            temp_file
                .sync_all()
                .map_err(|error| AppSettingsRepositoryError::StorageFailed(error.to_string()))?;
        }

        fs::rename(&temp_path, &self.file_path)
            .map_err(|error| AppSettingsRepositoryError::StorageFailed(error.to_string()))?;
        self.sync_parent_directory()?;

        Ok(())
    }

    fn sync_parent_directory(&self) -> AppSettingsRepositoryResult<()> {
        let Some(parent) = self.file_path.parent() else {
            return Ok(());
        };

        open_directory_for_sync(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| AppSettingsRepositoryError::StorageFailed(error.to_string()))?;

        Ok(())
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
            .unwrap_or_else(|| format!("settings.{}.{}.json.tmp", std::process::id(), nonce));

        self.file_path
            .parent()
            .map(|parent| parent.join(&temp_name))
            .unwrap_or_else(|| PathBuf::from(temp_name))
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

impl AppSettingsRepository for JsonAppSettingsRepository {
    fn load_settings(&self) -> AppSettingsRepositoryResult<AppSettings> {
        let config = self.load_file()?;

        Ok(AppSettings {
            thumbnail_cache_max_bytes: config.thumbnail_cache_max_bytes,
            thumbnail_cache_max_age_days: config.thumbnail_cache_max_age_days,
            log_storage_max_bytes: config.log_storage_max_bytes,
            debug_log_enabled: config.debug_log_enabled,
        })
    }

    fn save_settings(&self, settings: &AppSettings) -> AppSettingsRepositoryResult<()> {
        let _guard = self.write_lock.lock().map_err(|_| {
            AppSettingsRepositoryError::StorageFailed("write lock poisoned".to_owned())
        })?;
        self.save_file(&AppSettingsFile {
            version: CURRENT_SCHEMA_VERSION,
            thumbnail_cache_max_bytes: settings.thumbnail_cache_max_bytes,
            thumbnail_cache_max_age_days: settings.thumbnail_cache_max_age_days,
            log_storage_max_bytes: settings.log_storage_max_bytes,
            debug_log_enabled: settings.debug_log_enabled,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_ports::AppSettingsRepository;
    use std::fs;
    use std::path::PathBuf;

    fn test_file(name: &str) -> PathBuf {
        tempfile::tempdir()
            .expect("temp dir")
            .keep()
            .join("config")
            .join(format!("{name}.json"))
    }

    #[test]
    fn missing_settings_file_loads_default_settings() {
        let repo = JsonAppSettingsRepository::new(test_file("missing"));

        let settings = repo.load_settings().expect("load settings");

        assert_eq!(settings.thumbnail_cache_max_bytes, None);
        assert_eq!(settings.thumbnail_cache_max_age_days, None);
        assert_eq!(settings.log_storage_max_bytes, None);
        assert!(!settings.debug_log_enabled);
    }

    #[test]
    fn loads_thumbnail_cache_size_limit_from_settings_json() {
        let path = test_file("custom-cache-limit");
        fs::create_dir_all(path.parent().expect("settings parent")).expect("create parent");
        fs::write(
            &path,
            r#"{
                "version": 1,
                "thumbnailCacheMaxBytes": 67108864,
                "thumbnailCacheMaxAgeDays": 30
            }"#,
        )
        .expect("write settings");
        let repo = JsonAppSettingsRepository::new(path);

        let settings = repo.load_settings().expect("load settings");

        assert_eq!(settings.thumbnail_cache_max_bytes, Some(64 * 1024 * 1024));
        assert_eq!(settings.thumbnail_cache_max_age_days, Some(30));
        assert_eq!(settings.log_storage_max_bytes, None);
        assert!(!settings.debug_log_enabled);
    }

    #[test]
    fn saves_thumbnail_cache_size_limit_to_settings_json() {
        let path = test_file("save-cache-limit");
        let repo = JsonAppSettingsRepository::new(path);

        repo.save_settings(&AppSettings {
            thumbnail_cache_max_bytes: Some(128 * 1024 * 1024),
            thumbnail_cache_max_age_days: Some(14),
            log_storage_max_bytes: Some(64 * 1024 * 1024),
            debug_log_enabled: true,
        })
        .expect("save settings");
        let settings = repo.load_settings().expect("load settings");

        assert_eq!(settings.thumbnail_cache_max_bytes, Some(128 * 1024 * 1024));
        assert_eq!(settings.thumbnail_cache_max_age_days, Some(14));
        assert_eq!(settings.log_storage_max_bytes, Some(64 * 1024 * 1024));
        assert!(settings.debug_log_enabled);
    }

    #[test]
    fn corrupted_settings_json_returns_storage_corrupted() {
        let path = test_file("corrupted");
        fs::create_dir_all(path.parent().expect("settings parent")).expect("create parent");
        fs::write(&path, "{not json").expect("write settings");
        let repo = JsonAppSettingsRepository::new(path);

        let error = repo
            .load_settings()
            .expect_err("corrupted settings should fail");

        assert_eq!(
            error,
            hmm_ports::AppSettingsRepositoryError::StorageCorrupted
        );
    }
}
