use hmm_ports::{
    AppSettings, AppSettingsRepository, AppSettingsRepositoryError, DebugLogControl,
    NoopDebugLogControl, MIN_LOG_STORAGE_MAX_BYTES,
};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AppSettingsServiceError {
    #[error("thumbnail cache max bytes must be greater than zero")]
    InvalidThumbnailCacheMaxBytes,
    #[error("thumbnail cache max age days must be greater than zero")]
    InvalidThumbnailCacheMaxAgeDays,
    #[error("log storage max bytes must be at least one MiB")]
    InvalidLogStorageMaxBytes,
    #[error("app settings unavailable")]
    SettingsUnavailable,
}

pub struct AppSettingsService {
    repository: Arc<dyn AppSettingsRepository>,
    debug_log_control: Arc<dyn DebugLogControl>,
    update_lock: std::sync::Mutex<()>,
}

impl AppSettingsService {
    pub fn new(repository: Arc<dyn AppSettingsRepository>) -> Self {
        Self::new_with_debug_log_control(repository, Arc::new(NoopDebugLogControl))
    }

    pub fn new_with_debug_log_control(
        repository: Arc<dyn AppSettingsRepository>,
        debug_log_control: Arc<dyn DebugLogControl>,
    ) -> Self {
        Self {
            repository,
            debug_log_control,
            update_lock: std::sync::Mutex::new(()),
        }
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
        if matches!(max_bytes, Some(0)) {
            return Err(AppSettingsServiceError::InvalidThumbnailCacheMaxBytes);
        }

        self.update_settings(|settings| {
            settings.thumbnail_cache_max_bytes = max_bytes;
        })
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

        self.update_settings(|settings| {
            settings.thumbnail_cache_max_bytes = max_bytes;
            settings.thumbnail_cache_max_age_days = max_age_days;
        })
    }

    pub fn update_log_storage_settings(
        &self,
        max_bytes: Option<u64>,
    ) -> Result<AppSettings, AppSettingsServiceError> {
        if max_bytes.is_some_and(|value| value < MIN_LOG_STORAGE_MAX_BYTES) {
            return Err(AppSettingsServiceError::InvalidLogStorageMaxBytes);
        }

        self.update_settings(|settings| {
            settings.log_storage_max_bytes = max_bytes;
        })
    }

    pub fn update_debug_log_enabled(
        &self,
        enabled: bool,
    ) -> Result<AppSettings, AppSettingsServiceError> {
        let _guard = self
            .update_lock
            .lock()
            .map_err(|_| AppSettingsServiceError::SettingsUnavailable)?;
        let mut settings = self.get_settings()?;
        settings.debug_log_enabled = enabled;
        self.repository
            .save_settings(&settings)
            .map_err(settings_error_to_service_error)?;
        self.debug_log_control.set_enabled(enabled);
        Ok(settings)
    }

    fn update_settings(
        &self,
        update: impl FnOnce(&mut AppSettings),
    ) -> Result<AppSettings, AppSettingsServiceError> {
        let _guard = self
            .update_lock
            .lock()
            .map_err(|_| AppSettingsServiceError::SettingsUnavailable)?;
        let mut settings = self.get_settings()?;
        update(&mut settings);
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
    use std::sync::{mpsc, Mutex};
    use std::time::Duration;

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
                log_storage_max_bytes: None,
                debug_log_enabled: false,
            })
        );
    }

    #[test]
    fn returns_current_thumbnail_cache_settings_without_saving() {
        let repository = std::sync::Arc::new(FakeAppSettingsRepository {
            saved_settings: Mutex::new(Some(AppSettings {
                thumbnail_cache_max_bytes: Some(96 * 1024 * 1024),
                thumbnail_cache_max_age_days: Some(30),
                log_storage_max_bytes: Some(32 * 1024 * 1024),
                debug_log_enabled: false,
            })),
            save_count: Mutex::new(0),
        });
        let service = AppSettingsService::new(repository.clone());

        let settings = service.get_settings().expect("settings can be read");

        assert_eq!(settings.thumbnail_cache_max_bytes, Some(96 * 1024 * 1024));
        assert_eq!(settings.thumbnail_cache_max_age_days, Some(30));
        assert_eq!(settings.log_storage_max_bytes, Some(32 * 1024 * 1024));
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
    fn updating_thumbnail_cache_max_bytes_preserves_current_max_age() {
        let repository = std::sync::Arc::new(FakeAppSettingsRepository {
            saved_settings: Mutex::new(Some(AppSettings {
                thumbnail_cache_max_bytes: Some(96 * 1024 * 1024),
                thumbnail_cache_max_age_days: Some(30),
                log_storage_max_bytes: Some(32 * 1024 * 1024),
                debug_log_enabled: false,
            })),
            save_count: Mutex::new(0),
        });
        let service = AppSettingsService::new(repository.clone());

        let settings = service
            .update_thumbnail_cache_max_bytes(Some(128 * 1024 * 1024))
            .expect("settings update succeeds");

        assert_eq!(settings.thumbnail_cache_max_bytes, Some(128 * 1024 * 1024));
        assert_eq!(settings.thumbnail_cache_max_age_days, Some(30));
        assert_eq!(settings.log_storage_max_bytes, Some(32 * 1024 * 1024));
        assert_eq!(
            repository
                .saved_settings
                .lock()
                .expect("settings lock")
                .as_ref(),
            Some(&AppSettings {
                thumbnail_cache_max_bytes: Some(128 * 1024 * 1024),
                thumbnail_cache_max_age_days: Some(30),
                log_storage_max_bytes: Some(32 * 1024 * 1024),
                debug_log_enabled: false,
            })
        );
    }

    #[test]
    fn updates_log_storage_limit_without_losing_thumbnail_settings() {
        let repository = std::sync::Arc::new(FakeAppSettingsRepository {
            saved_settings: Mutex::new(Some(AppSettings {
                thumbnail_cache_max_bytes: Some(96 * 1024 * 1024),
                thumbnail_cache_max_age_days: Some(30),
                log_storage_max_bytes: None,
                debug_log_enabled: false,
            })),
            save_count: Mutex::new(0),
        });
        let service = AppSettingsService::new(repository);

        let settings = service
            .update_log_storage_settings(Some(64 * 1024 * 1024))
            .expect("settings update succeeds");

        assert_eq!(settings.log_storage_max_bytes, Some(64 * 1024 * 1024));
        assert_eq!(settings.thumbnail_cache_max_bytes, Some(96 * 1024 * 1024));
        assert_eq!(settings.thumbnail_cache_max_age_days, Some(30));
    }

    #[test]
    fn rejects_log_storage_limit_below_one_mib() {
        let service =
            AppSettingsService::new(std::sync::Arc::new(FakeAppSettingsRepository::default()));

        let error = service
            .update_log_storage_settings(Some(MIN_LOG_STORAGE_MAX_BYTES - 1))
            .expect_err("undersized limit rejected");

        assert_eq!(error, AppSettingsServiceError::InvalidLogStorageMaxBytes);
    }

    #[test]
    fn persists_debug_log_setting_before_updating_process_state() {
        let repository = Arc::new(FakeAppSettingsRepository::default());
        let control = Arc::new(RecordingDebugLogControl::default());
        let service =
            AppSettingsService::new_with_debug_log_control(repository.clone(), control.clone());

        let settings = service
            .update_debug_log_enabled(true)
            .expect("debug setting update succeeds");

        assert!(settings.debug_log_enabled);
        assert!(control.is_enabled());
        assert!(
            repository
                .saved_settings
                .lock()
                .expect("settings lock")
                .as_ref()
                .expect("saved settings")
                .debug_log_enabled
        );
    }

    #[test]
    fn failed_debug_log_save_keeps_process_state_unchanged() {
        let control = Arc::new(RecordingDebugLogControl::default());
        let service = AppSettingsService::new_with_debug_log_control(
            Arc::new(FailingSaveSettingsRepository),
            control.clone(),
        );

        let error = service
            .update_debug_log_enabled(true)
            .expect_err("failed save is reported");

        assert_eq!(error, AppSettingsServiceError::SettingsUnavailable);
        assert!(!control.is_enabled());
    }

    #[test]
    fn settings_updates_share_one_load_modify_save_lock() {
        let repository = Arc::new(FakeAppSettingsRepository::default());
        let service = Arc::new(AppSettingsService::new(repository));
        let guard = service.update_lock.lock().expect("update lock");
        let (completed_tx, completed_rx) = mpsc::sync_channel(1);
        let service_for_update = Arc::clone(&service);
        let update = std::thread::spawn(move || {
            let result = service_for_update.update_log_storage_settings(Some(64 * 1024 * 1024));
            completed_tx.send(result).expect("send update result");
        });

        assert!(completed_rx
            .recv_timeout(Duration::from_millis(50))
            .is_err());
        drop(guard);

        completed_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("update completes after lock release")
            .expect("settings update succeeds");
        update.join().expect("update thread");
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

    #[derive(Default)]
    struct RecordingDebugLogControl {
        enabled: std::sync::atomic::AtomicBool,
    }

    impl DebugLogControl for RecordingDebugLogControl {
        fn is_enabled(&self) -> bool {
            self.enabled.load(std::sync::atomic::Ordering::Acquire)
        }

        fn set_enabled(&self, enabled: bool) {
            self.enabled
                .store(enabled, std::sync::atomic::Ordering::Release);
        }
    }

    struct FailingSaveSettingsRepository;

    impl AppSettingsRepository for FailingSaveSettingsRepository {
        fn load_settings(&self) -> AppSettingsRepositoryResult<AppSettings> {
            Ok(AppSettings::default())
        }

        fn save_settings(&self, _settings: &AppSettings) -> AppSettingsRepositoryResult<()> {
            Err(AppSettingsRepositoryError::StorageFailed(
                "fixture failure".to_owned(),
            ))
        }
    }
}
