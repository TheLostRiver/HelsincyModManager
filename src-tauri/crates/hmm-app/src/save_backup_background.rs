use hmm_core::{
    GameId, ProfileId, SaveBackupBackgroundProtectionStatus,
    SaveBackupBackgroundRegistrationStatus, SaveBackupBackgroundSettings, SaveBackupSchedulerState,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, SaveBackupBackgroundRegistry,
    SaveBackupBackgroundRegistryError, SaveBackupBackgroundSettingsRepository,
    SaveBackupSchedulerStateRepository, SAVE_BACKUP_BACKGROUND_REGISTRY_SCHEMA_VERSION,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use thiserror::Error;

pub const SAVE_BACKUP_BACKGROUND_HEARTBEAT_TTL_MILLIS: u128 = 45 * 60_000;
// The Windows task's first periodic run can arrive after its 15-minute interval.
pub const SAVE_BACKUP_BACKGROUND_STARTUP_GRACE_MILLIS: u128 = 20 * 60_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupBackgroundStatus {
    pub scheduler_state: Option<SaveBackupSchedulerState>,
    pub status: SaveBackupBackgroundProtectionStatus,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupBackgroundControlStatus {
    pub desired_enabled: bool,
    pub status: SaveBackupBackgroundProtectionStatus,
    pub enabled_at: Option<u128>,
    pub last_heartbeat_at: Option<u128>,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupBackgroundRegistrationResult {
    pub status: SaveBackupBackgroundRegistrationStatus,
    pub error_code: Option<String>,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SaveBackupBackgroundServiceError {
    #[error("save backup scheduler state is unavailable")]
    SchedulerStateUnavailable,
    #[error("save backup background settings are unavailable")]
    SettingsUnavailable,
    #[error("app clock is unavailable")]
    ClockUnavailable,
    #[error("audit log is unavailable")]
    AuditUnavailable,
}

impl SaveBackupBackgroundServiceError {
    pub fn code(self) -> &'static str {
        match self {
            Self::SchedulerStateUnavailable => "save_backup_scheduler_unavailable",
            Self::SettingsUnavailable => "save_backup_background_settings_unavailable",
            Self::ClockUnavailable => "save_backup_clock_unavailable",
            Self::AuditUnavailable => "save_backup_background_audit_unavailable",
        }
    }
}

pub struct SaveBackupBackgroundService {
    registry: Arc<dyn SaveBackupBackgroundRegistry>,
    scheduler_state_repository: Arc<dyn SaveBackupSchedulerStateRepository>,
    background_settings_repository: Option<Arc<dyn SaveBackupBackgroundSettingsRepository>>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
    registration_transition: Mutex<()>,
}

impl SaveBackupBackgroundService {
    pub fn new(
        registry: Arc<dyn SaveBackupBackgroundRegistry>,
        scheduler_state_repository: Arc<dyn SaveBackupSchedulerStateRepository>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            registry,
            scheduler_state_repository,
            background_settings_repository: None,
            audit_log,
            clock,
            registration_transition: Mutex::new(()),
        }
    }

    pub fn new_with_settings_repository(
        registry: Arc<dyn SaveBackupBackgroundRegistry>,
        scheduler_state_repository: Arc<dyn SaveBackupSchedulerStateRepository>,
        background_settings_repository: Arc<dyn SaveBackupBackgroundSettingsRepository>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            registry,
            scheduler_state_repository,
            background_settings_repository: Some(background_settings_repository),
            audit_log,
            clock,
            registration_transition: Mutex::new(()),
        }
    }

    pub fn status(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<SaveBackupBackgroundStatus, SaveBackupBackgroundServiceError> {
        let state = self
            .scheduler_state_repository
            .get_state(game_id, profile_id)
            .map_err(|_| SaveBackupBackgroundServiceError::SchedulerStateUnavailable)?;
        let Some(state) = state else {
            return Ok(status_result(
                None,
                SaveBackupBackgroundProtectionStatus::NotEnabled,
                None,
            ));
        };
        if !state.enabled {
            return Ok(status_result(
                Some(state),
                SaveBackupBackgroundProtectionStatus::NotEnabled,
                None,
            ));
        }
        if self.background_settings_repository.is_some() {
            let control = self.control_status()?;
            let status = if control.status == SaveBackupBackgroundProtectionStatus::NotEnabled {
                SaveBackupBackgroundProtectionStatus::TrayOnly
            } else {
                control.status
            };
            let error = control
                .last_error_code
                .or_else(|| retained_scheduler_error(state.last_error_code.clone()));
            return Ok(status_result(Some(state), status, error));
        }
        if !state.background_protection_enabled {
            let error = retained_scheduler_error(state.last_error_code.clone());
            return Ok(status_result(
                Some(state),
                SaveBackupBackgroundProtectionStatus::TrayOnly,
                error,
            ));
        }

        let registration = match self.registry.inspect() {
            Ok(status) => status,
            Err(error) => {
                return Ok(status_result(
                    Some(state),
                    SaveBackupBackgroundProtectionStatus::RegistrationFailed,
                    Some(error.code().to_owned()),
                ));
            }
        };
        if registration != SaveBackupBackgroundRegistrationStatus::Registered {
            let (status, code) = registration_failure(registration);
            return Ok(status_result(Some(state), status, Some(code.to_owned())));
        }

        let now = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveBackupBackgroundServiceError::ClockUnavailable)?;
        let fresh = state.worker_heartbeat_at.is_some_and(|heartbeat| {
            heartbeat <= now && now - heartbeat <= SAVE_BACKUP_BACKGROUND_HEARTBEAT_TTL_MILLIS
        });
        if !fresh {
            return Ok(status_result(
                Some(state),
                SaveBackupBackgroundProtectionStatus::WorkerUnhealthy,
                Some("save_backup_background_worker_unhealthy".to_owned()),
            ));
        }

        let error = retained_scheduler_error(state.last_error_code.clone());
        Ok(status_result(
            Some(state),
            SaveBackupBackgroundProtectionStatus::Protected,
            error,
        ))
    }

    pub fn control_status(
        &self,
    ) -> Result<SaveBackupBackgroundControlStatus, SaveBackupBackgroundServiceError> {
        let settings = self
            .background_settings_repository
            .as_ref()
            .ok_or(SaveBackupBackgroundServiceError::SettingsUnavailable)?
            .load()
            .map_err(|_| SaveBackupBackgroundServiceError::SettingsUnavailable)?;

        if !settings.desired_enabled {
            return Ok(control_status_result(
                settings,
                SaveBackupBackgroundProtectionStatus::NotEnabled,
                None,
            ));
        }

        let registration = match self.registry.inspect() {
            Ok(status) => status,
            Err(error) => {
                return Ok(control_status_result(
                    settings,
                    SaveBackupBackgroundProtectionStatus::RegistrationFailed,
                    Some(error.code().to_owned()),
                ));
            }
        };
        if registration != SaveBackupBackgroundRegistrationStatus::Registered {
            let (status, code) = registration_failure(registration);
            return Ok(control_status_result(
                settings,
                status,
                Some(code.to_owned()),
            ));
        }

        let now = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveBackupBackgroundServiceError::ClockUnavailable)?;
        let Some(enabled_at) = settings.enabled_at else {
            return Ok(worker_unhealthy_control_status(settings));
        };
        let heartbeat = settings.last_worker_heartbeat_at;
        let heartbeat_is_valid = heartbeat.is_some_and(|heartbeat| {
            heartbeat >= enabled_at
                && heartbeat <= now
                && now - heartbeat <= SAVE_BACKUP_BACKGROUND_HEARTBEAT_TTL_MILLIS
        });
        if heartbeat_is_valid {
            return Ok(control_status_result(
                settings,
                SaveBackupBackgroundProtectionStatus::Protected,
                None,
            ));
        }

        let has_no_current_enable_heartbeat = heartbeat.is_none_or(|value| value < enabled_at);
        let is_within_startup_grace =
            enabled_at <= now && now - enabled_at <= SAVE_BACKUP_BACKGROUND_STARTUP_GRACE_MILLIS;
        if has_no_current_enable_heartbeat && is_within_startup_grace {
            return Ok(control_status_result(
                settings,
                SaveBackupBackgroundProtectionStatus::Starting,
                None,
            ));
        }

        Ok(worker_unhealthy_control_status(settings))
    }

    pub fn register(
        &self,
    ) -> Result<SaveBackupBackgroundRegistrationResult, SaveBackupBackgroundServiceError> {
        let _transition = self.lock_registration_transition();
        self.change_registration(RegistrationOperation::Register)
    }

    pub fn unregister(
        &self,
    ) -> Result<SaveBackupBackgroundRegistrationResult, SaveBackupBackgroundServiceError> {
        let _transition = self.lock_registration_transition();
        self.change_registration(RegistrationOperation::Unregister)
    }

    pub fn enable(
        &self,
    ) -> Result<SaveBackupBackgroundControlStatus, SaveBackupBackgroundServiceError> {
        let _transition = self.lock_registration_transition();
        let settings_repository = self.settings_repository()?;
        let timestamp_unix_millis = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveBackupBackgroundServiceError::ClockUnavailable)?;
        settings_repository
            .begin_enable(timestamp_unix_millis)
            .map_err(|_| SaveBackupBackgroundServiceError::SettingsUnavailable)?;
        let settings = SaveBackupBackgroundSettings {
            desired_enabled: true,
            enabled_at: Some(timestamp_unix_millis),
            last_worker_heartbeat_at: None,
            updated_at: timestamp_unix_millis,
        };

        let registration = self.registration_operation_result(RegistrationOperation::Register);
        self.record_registration_audit(
            timestamp_unix_millis,
            RegistrationOperation::Register,
            &registration,
        )?;

        if registration.status == SaveBackupBackgroundRegistrationStatus::Registered
            && registration.error_code.is_none()
        {
            return Ok(control_status_result(
                settings,
                SaveBackupBackgroundProtectionStatus::Starting,
                None,
            ));
        }
        Ok(control_status_from_registration_result(
            settings,
            registration,
        ))
    }

    pub fn disable(
        &self,
    ) -> Result<SaveBackupBackgroundControlStatus, SaveBackupBackgroundServiceError> {
        let _transition = self.lock_registration_transition();
        let settings_repository = self.settings_repository()?;
        let settings = settings_repository
            .load()
            .map_err(|_| SaveBackupBackgroundServiceError::SettingsUnavailable)?;
        let timestamp_unix_millis = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveBackupBackgroundServiceError::ClockUnavailable)?;
        let registration = self.registration_operation_result(RegistrationOperation::Unregister);

        if registration.status == SaveBackupBackgroundRegistrationStatus::NotRegistered
            && registration.error_code.is_none()
        {
            if settings_repository
                .finish_disable(timestamp_unix_millis)
                .is_err()
            {
                let settings_failure = registration_result(
                    SaveBackupBackgroundRegistrationStatus::NotRegistered,
                    Some(SaveBackupBackgroundServiceError::SettingsUnavailable.code()),
                );
                self.record_registration_audit(
                    timestamp_unix_millis,
                    RegistrationOperation::Unregister,
                    &settings_failure,
                )?;
                return Err(SaveBackupBackgroundServiceError::SettingsUnavailable);
            }
            self.record_registration_audit(
                timestamp_unix_millis,
                RegistrationOperation::Unregister,
                &registration,
            )?;
            return Ok(control_status_result(
                SaveBackupBackgroundSettings {
                    desired_enabled: false,
                    enabled_at: None,
                    last_worker_heartbeat_at: None,
                    updated_at: timestamp_unix_millis,
                },
                SaveBackupBackgroundProtectionStatus::NotEnabled,
                None,
            ));
        }

        self.record_registration_audit(
            timestamp_unix_millis,
            RegistrationOperation::Unregister,
            &registration,
        )?;
        Ok(control_status_from_registration_result(
            settings,
            registration,
        ))
    }

    fn change_registration(
        &self,
        operation: RegistrationOperation,
    ) -> Result<SaveBackupBackgroundRegistrationResult, SaveBackupBackgroundServiceError> {
        let timestamp_unix_millis = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveBackupBackgroundServiceError::ClockUnavailable)?;
        let result = self.registration_operation_result(operation);
        self.record_registration_audit(timestamp_unix_millis, operation, &result)?;
        Ok(result)
    }

    fn settings_repository(
        &self,
    ) -> Result<&Arc<dyn SaveBackupBackgroundSettingsRepository>, SaveBackupBackgroundServiceError>
    {
        self.background_settings_repository
            .as_ref()
            .ok_or(SaveBackupBackgroundServiceError::SettingsUnavailable)
    }

    fn lock_registration_transition(&self) -> MutexGuard<'_, ()> {
        self.registration_transition
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn registration_operation_result(
        &self,
        operation: RegistrationOperation,
    ) -> SaveBackupBackgroundRegistrationResult {
        let operation_result = match operation {
            RegistrationOperation::Register => self.registry.register(),
            RegistrationOperation::Unregister => self.registry.unregister(),
        };

        match operation_result {
            Err(error) => registry_error_result(error),
            Ok(status) if status == operation.expected_status() => {
                registration_result(status, None)
            }
            Ok(status) => registration_operation_failure(operation, status),
        }
    }

    fn record_registration_audit(
        &self,
        timestamp_unix_millis: u128,
        operation: RegistrationOperation,
        result: &SaveBackupBackgroundRegistrationResult,
    ) -> Result<(), SaveBackupBackgroundServiceError> {
        let success = result.status == operation.expected_status() && result.error_code.is_none();
        self.audit_log
            .record(AuditLogEvent {
                timestamp_unix_millis,
                category: "save_backup".to_owned(),
                operation: "background_registration".to_owned(),
                result: if success { "success" } else { "failure" }.to_owned(),
                fields: BTreeMap::from([
                    (
                        "registration_status".to_owned(),
                        result.status.as_str().to_owned(),
                    ),
                    (
                        "task_schema_version".to_owned(),
                        SAVE_BACKUP_BACKGROUND_REGISTRY_SCHEMA_VERSION.to_string(),
                    ),
                    (
                        "error_code".to_owned(),
                        result.error_code.clone().unwrap_or_default(),
                    ),
                ]),
            })
            .map_err(|_| SaveBackupBackgroundServiceError::AuditUnavailable)?;
        Ok(())
    }
}

fn control_status_result(
    settings: SaveBackupBackgroundSettings,
    status: SaveBackupBackgroundProtectionStatus,
    last_error_code: Option<String>,
) -> SaveBackupBackgroundControlStatus {
    SaveBackupBackgroundControlStatus {
        desired_enabled: settings.desired_enabled,
        status,
        enabled_at: settings.enabled_at,
        last_heartbeat_at: settings.last_worker_heartbeat_at,
        last_error_code,
    }
}

fn worker_unhealthy_control_status(
    settings: SaveBackupBackgroundSettings,
) -> SaveBackupBackgroundControlStatus {
    control_status_result(
        settings,
        SaveBackupBackgroundProtectionStatus::WorkerUnhealthy,
        Some("save_backup_background_worker_unhealthy".to_owned()),
    )
}

fn control_status_from_registration_result(
    settings: SaveBackupBackgroundSettings,
    registration: SaveBackupBackgroundRegistrationResult,
) -> SaveBackupBackgroundControlStatus {
    let status = match registration.status {
        SaveBackupBackgroundRegistrationStatus::PermissionRequired => {
            SaveBackupBackgroundProtectionStatus::PermissionRequired
        }
        SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform => {
            SaveBackupBackgroundProtectionStatus::UnsupportedPlatform
        }
        SaveBackupBackgroundRegistrationStatus::NotRegistered
        | SaveBackupBackgroundRegistrationStatus::Registered
        | SaveBackupBackgroundRegistrationStatus::ConfigurationDrift
        | SaveBackupBackgroundRegistrationStatus::RegistrationFailed => {
            SaveBackupBackgroundProtectionStatus::RegistrationFailed
        }
    };
    control_status_result(settings, status, registration.error_code)
}

fn status_result(
    scheduler_state: Option<SaveBackupSchedulerState>,
    status: SaveBackupBackgroundProtectionStatus,
    last_error_code: Option<String>,
) -> SaveBackupBackgroundStatus {
    SaveBackupBackgroundStatus {
        scheduler_state,
        status,
        last_error_code,
    }
}

fn retained_scheduler_error(error_code: Option<String>) -> Option<String> {
    error_code.filter(|code| !code.starts_with("save_backup_background_"))
}

fn registration_failure(
    status: SaveBackupBackgroundRegistrationStatus,
) -> (SaveBackupBackgroundProtectionStatus, &'static str) {
    match status {
        SaveBackupBackgroundRegistrationStatus::NotRegistered => (
            SaveBackupBackgroundProtectionStatus::RegistrationFailed,
            "save_backup_background_not_registered",
        ),
        SaveBackupBackgroundRegistrationStatus::ConfigurationDrift => (
            SaveBackupBackgroundProtectionStatus::RegistrationFailed,
            "save_backup_background_configuration_drift",
        ),
        SaveBackupBackgroundRegistrationStatus::RegistrationFailed => (
            SaveBackupBackgroundProtectionStatus::RegistrationFailed,
            "save_backup_background_registration_failed",
        ),
        SaveBackupBackgroundRegistrationStatus::PermissionRequired => (
            SaveBackupBackgroundProtectionStatus::PermissionRequired,
            "save_backup_background_permission_required",
        ),
        SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform => (
            SaveBackupBackgroundProtectionStatus::UnsupportedPlatform,
            "save_backup_background_unsupported_platform",
        ),
        SaveBackupBackgroundRegistrationStatus::Registered => {
            unreachable!("registered handled before failure mapping")
        }
    }
}

fn registration_result(
    status: SaveBackupBackgroundRegistrationStatus,
    error_code: Option<&str>,
) -> SaveBackupBackgroundRegistrationResult {
    SaveBackupBackgroundRegistrationResult {
        status,
        error_code: error_code.map(str::to_owned),
    }
}

fn registry_error_result(
    error: SaveBackupBackgroundRegistryError,
) -> SaveBackupBackgroundRegistrationResult {
    registration_result(
        SaveBackupBackgroundRegistrationStatus::RegistrationFailed,
        Some(error.code()),
    )
}

fn registration_operation_failure(
    operation: RegistrationOperation,
    status: SaveBackupBackgroundRegistrationStatus,
) -> SaveBackupBackgroundRegistrationResult {
    if operation == RegistrationOperation::Unregister
        && status == SaveBackupBackgroundRegistrationStatus::Registered
    {
        return registration_result(
            SaveBackupBackgroundRegistrationStatus::RegistrationFailed,
            Some("save_backup_background_registration_failed"),
        );
    }
    let (_, code) = registration_failure(status);
    registration_result(status, Some(code))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegistrationOperation {
    Register,
    Unregister,
}

impl RegistrationOperation {
    fn expected_status(self) -> SaveBackupBackgroundRegistrationStatus {
        match self {
            Self::Register => SaveBackupBackgroundRegistrationStatus::Registered,
            Self::Unregister => SaveBackupBackgroundRegistrationStatus::NotRegistered,
        }
    }
}
