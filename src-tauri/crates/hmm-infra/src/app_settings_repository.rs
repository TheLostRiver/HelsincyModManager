use hmm_ports::{
    AppSettings, AppSettingsRepository, AppSettingsRepositoryError, AppSettingsRepositoryResult,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettingsFile {
    version: u32,
    #[serde(default)]
    thumbnail_cache_max_bytes: Option<u64>,
}

impl Default for AppSettingsFile {
    fn default() -> Self {
        Self {
            version: CURRENT_SCHEMA_VERSION,
            thumbnail_cache_max_bytes: None,
        }
    }
}

pub struct JsonAppSettingsRepository {
    file_path: PathBuf,
}

impl JsonAppSettingsRepository {
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }
}

impl AppSettingsRepository for JsonAppSettingsRepository {
    fn load_settings(&self) -> AppSettingsRepositoryResult<AppSettings> {
        if !self.file_path.exists() {
            return Ok(AppSettings::default());
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

        Ok(AppSettings {
            thumbnail_cache_max_bytes: config.thumbnail_cache_max_bytes,
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
    }

    #[test]
    fn loads_thumbnail_cache_size_limit_from_settings_json() {
        let path = test_file("custom-cache-limit");
        fs::create_dir_all(path.parent().expect("settings parent")).expect("create parent");
        fs::write(
            &path,
            r#"{
                "version": 1,
                "thumbnailCacheMaxBytes": 67108864
            }"#,
        )
        .expect("write settings");
        let repo = JsonAppSettingsRepository::new(path);

        let settings = repo.load_settings().expect("load settings");

        assert_eq!(settings.thumbnail_cache_max_bytes, Some(64 * 1024 * 1024));
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
