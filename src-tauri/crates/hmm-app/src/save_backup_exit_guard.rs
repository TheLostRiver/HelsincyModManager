use crate::SaveBackupBackgroundService;
use hmm_core::{BackupCadence, SaveBackupBackgroundProtectionStatus};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, ProfileRepository, ProfileSaveSettingsRepository,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveBackupExitReason {
    BackgroundStarting,
    BackgroundNotEnabled,
    RegistrationFailed,
    WorkerUnhealthy,
    PermissionRequired,
    UnsupportedPlatform,
    StatusUnavailable,
}

impl SaveBackupExitReason {
    fn protection_status(self) -> &'static str {
        match self {
            Self::BackgroundStarting => "starting",
            Self::BackgroundNotEnabled => "not_enabled",
            Self::RegistrationFailed => "registration_failed",
            Self::WorkerUnhealthy => "worker_unhealthy",
            Self::PermissionRequired => "permission_required",
            Self::UnsupportedPlatform => "unsupported_platform",
            Self::StatusUnavailable => "status_unavailable",
        }
    }

    fn error_code(self) -> &'static str {
        match self {
            Self::BackgroundStarting => "",
            Self::BackgroundNotEnabled => "save_backup_background_not_enabled",
            Self::RegistrationFailed => "save_backup_background_registration_failed",
            Self::WorkerUnhealthy => "save_backup_background_worker_unhealthy",
            Self::PermissionRequired => "save_backup_background_permission_required",
            Self::UnsupportedPlatform => "save_backup_background_unsupported_platform",
            Self::StatusUnavailable => "save_backup_background_status_unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveBackupExitDecision {
    Safe,
    ConfirmationRequired { reason: SaveBackupExitReason },
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SaveBackupExitGuardError {
    #[error("save backup exit guard clock is unavailable")]
    ClockUnavailable,
    #[error("save backup exit guard audit is unavailable")]
    AuditUnavailable,
}

impl SaveBackupExitGuardError {
    pub fn code(self) -> &'static str {
        match self {
            Self::ClockUnavailable => "save_backup_clock_unavailable",
            Self::AuditUnavailable => "save_backup_background_audit_unavailable",
        }
    }
}

pub struct SaveBackupExitGuard {
    profile_repository: Arc<dyn ProfileRepository>,
    profile_save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
    background_service: Arc<SaveBackupBackgroundService>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
}

impl SaveBackupExitGuard {
    pub fn new(
        profile_repository: Arc<dyn ProfileRepository>,
        profile_save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
        background_service: Arc<SaveBackupBackgroundService>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            profile_repository,
            profile_save_settings_repository,
            background_service,
            audit_log,
            clock,
        }
    }

    pub fn evaluate(&self) -> Result<SaveBackupExitDecision, SaveBackupExitGuardError> {
        let profiles = match self.profile_repository.list_all() {
            Ok(profiles) => profiles,
            Err(_) => return Ok(status_unavailable_decision()),
        };
        let mut has_automatic_profile = false;
        for profile in profiles {
            let settings = match self
                .profile_save_settings_repository
                .get_settings(&profile.id)
            {
                Ok(settings) => settings,
                Err(_) => return Ok(status_unavailable_decision()),
            };
            if settings.is_some_and(|settings| settings.schedule.cadence != BackupCadence::Manual) {
                has_automatic_profile = true;
            }
        }

        if !has_automatic_profile {
            return Ok(SaveBackupExitDecision::Safe);
        }

        let control_status = match self.background_service.control_status() {
            Ok(status) => status.status,
            Err(_) => return Ok(status_unavailable_decision()),
        };
        Ok(decision_for_status(control_status))
    }

    pub fn record_override(
        &self,
        reason: SaveBackupExitReason,
    ) -> Result<(), SaveBackupExitGuardError> {
        let timestamp_unix_millis = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveBackupExitGuardError::ClockUnavailable)?;
        self.audit_log
            .record(AuditLogEvent {
                timestamp_unix_millis,
                category: "save_backup".to_owned(),
                operation: "background_exit_override".to_owned(),
                result: "success".to_owned(),
                fields: BTreeMap::from([
                    (
                        "protection_status".to_owned(),
                        reason.protection_status().to_owned(),
                    ),
                    ("error_code".to_owned(), reason.error_code().to_owned()),
                ]),
            })
            .map_err(|_| SaveBackupExitGuardError::AuditUnavailable)
    }
}

fn status_unavailable_decision() -> SaveBackupExitDecision {
    SaveBackupExitDecision::ConfirmationRequired {
        reason: SaveBackupExitReason::StatusUnavailable,
    }
}

fn decision_for_status(status: SaveBackupBackgroundProtectionStatus) -> SaveBackupExitDecision {
    let reason = match status {
        SaveBackupBackgroundProtectionStatus::Protected => return SaveBackupExitDecision::Safe,
        SaveBackupBackgroundProtectionStatus::Starting => SaveBackupExitReason::BackgroundStarting,
        SaveBackupBackgroundProtectionStatus::TrayOnly
        | SaveBackupBackgroundProtectionStatus::NotEnabled => {
            SaveBackupExitReason::BackgroundNotEnabled
        }
        SaveBackupBackgroundProtectionStatus::RegistrationFailed => {
            SaveBackupExitReason::RegistrationFailed
        }
        SaveBackupBackgroundProtectionStatus::WorkerUnhealthy => {
            SaveBackupExitReason::WorkerUnhealthy
        }
        SaveBackupBackgroundProtectionStatus::PermissionRequired => {
            SaveBackupExitReason::PermissionRequired
        }
        SaveBackupBackgroundProtectionStatus::UnsupportedPlatform => {
            SaveBackupExitReason::UnsupportedPlatform
        }
    };
    SaveBackupExitDecision::ConfirmationRequired { reason }
}
