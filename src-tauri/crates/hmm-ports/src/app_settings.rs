use thiserror::Error;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppSettings {
    pub thumbnail_cache_max_bytes: Option<u64>,
    pub thumbnail_cache_max_age_days: Option<u32>,
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
