use hmm_ports::{AppSettings, AppSettingsRepository, AppSettingsRepositoryError};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AppSettingsServiceError {
    #[error("thumbnail cache max bytes must be greater than zero")]
    InvalidThumbnailCacheMaxBytes,
    #[error("app settings unavailable")]
    SettingsUnavailable,
}

pub struct AppSettingsService {
    repository: Arc<dyn AppSettingsRepository>,
}

impl AppSettingsService {
    pub fn new(repository: Arc<dyn AppSettingsRepository>) -> Self {
        Self { repository }
    }

    pub fn update_thumbnail_cache_max_bytes(
        &self,
        max_bytes: Option<u64>,
    ) -> Result<AppSettings, AppSettingsServiceError> {
        if matches!(max_bytes, Some(0)) {
            return Err(AppSettingsServiceError::InvalidThumbnailCacheMaxBytes);
        }

        let settings = AppSettings {
            thumbnail_cache_max_bytes: max_bytes,
        };
        self.repository
            .save_settings(&settings)
            .map_err(settings_error_to_service_error)?;

        Ok(settings)
    }
}

fn settings_error_to_service_error(_error: AppSettingsRepositoryError) -> AppSettingsServiceError {
    AppSettingsServiceError::SettingsUnavailable
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_ports::AppSettingsRepositoryResult;
    use std::sync::Mutex;

    #[test]
    fn updates_thumbnail_cache_size_limit() {
        let repository = std::sync::Arc::new(FakeAppSettingsRepository::default());
        let service = AppSettingsService::new(repository.clone());

        let settings = service
            .update_thumbnail_cache_max_bytes(Some(96 * 1024 * 1024))
            .expect("settings update succeeds");

        assert_eq!(settings.thumbnail_cache_max_bytes, Some(96 * 1024 * 1024));
        assert_eq!(
            repository
                .saved_settings
                .lock()
                .expect("settings lock")
                .as_ref(),
            Some(&AppSettings {
                thumbnail_cache_max_bytes: Some(96 * 1024 * 1024),
            })
        );
    }

    #[test]
    fn rejects_zero_thumbnail_cache_size_limit() {
        let service =
            AppSettingsService::new(std::sync::Arc::new(FakeAppSettingsRepository::default()));

        let error = service
            .update_thumbnail_cache_max_bytes(Some(0))
            .expect_err("zero limit rejected");

        assert_eq!(
            error,
            AppSettingsServiceError::InvalidThumbnailCacheMaxBytes
        );
    }

    #[derive(Default)]
    struct FakeAppSettingsRepository {
        saved_settings: Mutex<Option<AppSettings>>,
    }

    impl AppSettingsRepository for FakeAppSettingsRepository {
        fn load_settings(&self) -> AppSettingsRepositoryResult<AppSettings> {
            Ok(self
                .saved_settings
                .lock()
                .expect("settings lock")
                .clone()
                .unwrap_or_default())
        }

        fn save_settings(&self, settings: &AppSettings) -> AppSettingsRepositoryResult<()> {
            *self.saved_settings.lock().expect("settings lock") = Some(settings.clone());
            Ok(())
        }
    }
}
