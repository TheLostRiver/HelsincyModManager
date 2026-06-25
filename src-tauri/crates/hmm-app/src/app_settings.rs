use hmm_ports::{AppSettings, AppSettingsRepository, AppSettingsRepositoryError};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AppSettingsServiceError {
    #[error("thumbnail cache max bytes must be greater than zero")]
    InvalidThumbnailCacheMaxBytes,
    #[error("thumbnail cache max age days must be greater than zero")]
    InvalidThumbnailCacheMaxAgeDays,
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

    pub fn get_settings(&self) -> Result<AppSettings, AppSettingsServiceError> {
        self.repository
            .load_settings()
            .map_err(settings_error_to_service_error)
    }

    pub fn update_thumbnail_cache_max_bytes(
        &self,
        max_bytes: Option<u64>,
    ) -> Result<AppSettings, AppSettingsServiceError> {
        self.update_thumbnail_cache_settings(max_bytes, None)
    }

    pub fn update_thumbnail_cache_settings(
        &self,
        max_bytes: Option<u64>,
        max_age_days: Option<u32>,
    ) -> Result<AppSettings, AppSettingsServiceError> {
        if matches!(max_bytes, Some(0)) {
            return Err(AppSettingsServiceError::InvalidThumbnailCacheMaxBytes);
        }
        if matches!(max_age_days, Some(0)) {
            return Err(AppSettingsServiceError::InvalidThumbnailCacheMaxAgeDays);
        }

        let settings = AppSettings {
            thumbnail_cache_max_bytes: max_bytes,
            thumbnail_cache_max_age_days: max_age_days,
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
            .update_thumbnail_cache_settings(Some(96 * 1024 * 1024), Some(30))
            .expect("settings update succeeds");

        assert_eq!(settings.thumbnail_cache_max_bytes, Some(96 * 1024 * 1024));
        assert_eq!(settings.thumbnail_cache_max_age_days, Some(30));
        assert_eq!(
            repository
                .saved_settings
                .lock()
                .expect("settings lock")
                .as_ref(),
            Some(&AppSettings {
                thumbnail_cache_max_bytes: Some(96 * 1024 * 1024),
                thumbnail_cache_max_age_days: Some(30),
            })
        );
    }

    #[test]
    fn returns_current_thumbnail_cache_settings_without_saving() {
        let repository = std::sync::Arc::new(FakeAppSettingsRepository {
            saved_settings: Mutex::new(Some(AppSettings {
                thumbnail_cache_max_bytes: Some(96 * 1024 * 1024),
                thumbnail_cache_max_age_days: Some(30),
            })),
            save_count: Mutex::new(0),
        });
        let service = AppSettingsService::new(repository.clone());

        let settings = service.get_settings().expect("settings can be read");

        assert_eq!(settings.thumbnail_cache_max_bytes, Some(96 * 1024 * 1024));
        assert_eq!(settings.thumbnail_cache_max_age_days, Some(30));
        assert_eq!(
            repository
                .save_count
                .lock()
                .expect("save count lock")
                .to_owned(),
            0
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

    #[test]
    fn rejects_zero_thumbnail_cache_max_age_days() {
        let service =
            AppSettingsService::new(std::sync::Arc::new(FakeAppSettingsRepository::default()));

        let error = service
            .update_thumbnail_cache_settings(Some(64 * 1024 * 1024), Some(0))
            .expect_err("zero retention days rejected");

        assert_eq!(
            error,
            AppSettingsServiceError::InvalidThumbnailCacheMaxAgeDays
        );
    }

    #[derive(Default)]
    struct FakeAppSettingsRepository {
        saved_settings: Mutex<Option<AppSettings>>,
        save_count: Mutex<usize>,
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
            *self.save_count.lock().expect("save count lock") += 1;
            Ok(())
        }
    }
}
