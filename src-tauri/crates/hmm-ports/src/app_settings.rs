use std::path::PathBuf;
use thiserror::Error;

pub const MIN_LOG_STORAGE_MAX_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppSettings {
    pub thumbnail_cache_max_bytes: Option<u64>,
    pub thumbnail_cache_max_age_days: Option<u32>,
    pub log_storage_max_bytes: Option<u64>,
    pub debug_log_enabled: bool,
    /// User-chosen Mod storage root (the directory that holds `sandboxes/`). `None` keeps the
    /// default `<app-data>/mod-import`. Stored as given; validated on read by the runtime.
    pub mod_storage_dir: Option<PathBuf>,
    /// #275 "move instead of copy": remove the user's source archive once a zip import has been
    /// persisted. Off by default; the import itself never depends on it.
    pub delete_archive_after_import: bool,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AppSettingsRepositoryError {
    #[error("storage corrupted")]
    StorageCorrupted,
    #[error("storage failed: {0}")]
    StorageFailed(String),
}

pub type AppSettingsRepositoryResult<T> = Result<T, AppSettingsRepositoryError>;

pub trait AppSettingsRepository: Send + Sync {
    fn load_settings(&self) -> AppSettingsRepositoryResult<AppSettings>;
    fn save_settings(&self, settings: &AppSettings) -> AppSettingsRepositoryResult<()>;
}
