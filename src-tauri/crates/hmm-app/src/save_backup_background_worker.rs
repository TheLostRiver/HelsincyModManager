use std::collections::BTreeMap;
use std::sync::Arc;

use hmm_core::{
    BackupCadence, GameId, ProfileId, SaveBackupBackgroundProtectionStatus, SaveBackupTrigger,
    SaveBackupWorkerHeartbeat,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, ProfileRepository, ProfileSaveSettingsRepository,
    SaveBackupSchedulerStateRepository,
};
use thiserror::Error;

use crate::{
    SaveBackupAutoCheckRequest, SaveBackupAutoSchedulerService, SaveBackupTaskRunner,
    SaveBackupTaskService, StartSaveBackupTaskRequest,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SaveBackupBackgroundWorkerRunSummary {
    pub checked_profiles: u32,
    pub started_tasks: u32,
    pub deferred_profiles: u32,
    pub failed_profiles: u32,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SaveBackupBackgroundWorkerError {
    #[error("profile list unavailable")]
    ProfileListUnavailable,
    #[error("worker clock unavailable")]
    ClockUnavailable,
}

impl SaveBackupBackgroundWorkerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProfileListUnavailable => "save_backup_background_profile_list_unavailable",
            Self::ClockUnavailable => "save_backup_background_clock_unavailable",
        }
    }
}

pub struct SaveBackupBackgroundWorker {
    game_ids: Vec<GameId>,
    profile_repository: Arc<dyn ProfileRepository>,
    save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
    scheduler: Arc<SaveBackupAutoSchedulerService>,
    task_service: Arc<SaveBackupTaskService>,
    task_runner: Arc<SaveBackupTaskRunner>,
    scheduler_state_repository: Arc<dyn SaveBackupSchedulerStateRepository>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
}

impl SaveBackupBackgroundWorker {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        game_ids: Vec<GameId>,
        profile_repository: Arc<dyn ProfileRepository>,
        save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
        scheduler: Arc<SaveBackupAutoSchedulerService>,
        task_service: Arc<SaveBackupTaskService>,
        task_runner: Arc<SaveBackupTaskRunner>,
        scheduler_state_repository: Arc<dyn SaveBackupSchedulerStateRepository>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            game_ids,
            profile_repository,
            save_settings_repository,
            scheduler,
            task_service,
            task_runner,
            scheduler_state_repository,
            audit_log,
            clock,
        }
    }

    pub fn run_once(
        &self,
        worker_instance_id: &str,
    ) -> Result<SaveBackupBackgroundWorkerRunSummary, SaveBackupBackgroundWorkerError> {
        let worker_started_at = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveBackupBackgroundWorkerError::ClockUnavailable)?;
        let profiles = self
            .profile_repository
            .list_all()
            .map_err(|_| SaveBackupBackgroundWorkerError::ProfileListUnavailable)?;
        let mut summary = SaveBackupBackgroundWorkerRunSummary::default();

        for game_id in &self.game_ids {
            for profile in &profiles {
                let settings = match self.save_settings_repository.get_settings(&profile.id) {
                    Ok(Some(settings)) => settings,
                    Ok(None) => continue,
                    Err(_) => {
                        summary.failed_profiles += 1;
                        self.record_profile_error(
                            worker_started_at,
                            game_id,
                            &profile.id,
                            "save_backup_auto_settings_unavailable",
                        );
                        continue;
                    }
                };

                if settings.schedule.cadence == BackupCadence::Manual {
                    continue;
                }

                let profile_id = ProfileId::new(profile.id.clone());
                let check = match self.scheduler.check_profile(SaveBackupAutoCheckRequest {
                    game_id: game_id.clone(),
                    profile_id: profile_id.clone(),
                }) {
                    Ok(check) => check,
                    Err(error) => {
                        summary.failed_profiles += 1;
                        self.record_profile_error(
                            worker_started_at,
                            game_id,
                            &profile.id,
                            error.code(),
                        );
                        continue;
                    }
                };

                if self
                    .scheduler_state_repository
                    .record_worker_heartbeat(SaveBackupWorkerHeartbeat {
                        game_id: game_id.clone(),
                        profile_id: profile_id.clone(),
                        worker_instance_id: worker_instance_id.to_owned(),
                        checked_at: check.checked_at,
                        status: SaveBackupBackgroundProtectionStatus::TrayOnly,
                    })
                    .is_err()
                {
                    if let Some(request) = check.due_task.as_ref() {
                        self.release_task_start_lease(request);
                    }
                    summary.failed_profiles += 1;
                    self.record_profile_error(
                        worker_started_at,
                        game_id,
                        &profile.id,
                        "save_backup_scheduler_unavailable",
                    );
                    continue;
                }

                if check.pending_reason.is_some() {
                    summary.deferred_profiles += 1;
                    summary.checked_profiles += 1;
                    continue;
                }

                if let Some(request) = check.due_task {
                    let task = match self.task_service.start_save_backup_task(request.clone()) {
                        Ok(task) => task,
                        Err(_) => {
                            self.release_task_start_lease(&request);
                            summary.failed_profiles += 1;
                            self.record_profile_error(
                                worker_started_at,
                                game_id,
                                &profile.id,
                                "save_backup_background_task_start_failed",
                            );
                            continue;
                        }
                    };

                    if self
                        .task_runner
                        .run_save_backup_task(&task.task_id, request)
                        .is_err()
                    {
                        summary.failed_profiles += 1;
                        self.record_profile_error(
                            worker_started_at,
                            game_id,
                            &profile.id,
                            "save_backup_background_task_run_failed",
                        );
                        continue;
                    }

                    summary.started_tasks += 1;
                }

                summary.checked_profiles += 1;
            }
        }

        Ok(summary)
    }

    fn release_task_start_lease(&self, request: &StartSaveBackupTaskRequest) {
        let Some(lease_owner) = request.scheduler_lease_owner.as_deref() else {
            return;
        };

        let _ = self.scheduler_state_repository.release_lease(
            &request.game_id,
            &request.profile_id,
            lease_owner,
        );
    }

    fn record_profile_error(
        &self,
        timestamp_unix_millis: u128,
        game_id: &GameId,
        profile_id: &str,
        error_code: &str,
    ) {
        let fields = BTreeMap::from([
            ("game_id".to_owned(), game_id.as_str().to_owned()),
            ("profile_id".to_owned(), profile_id.to_owned()),
            (
                "trigger".to_owned(),
                SaveBackupTrigger::Auto.as_str().to_owned(),
            ),
            ("error_code".to_owned(), error_code.to_owned()),
        ]);
        let _ = self.audit_log.record(AuditLogEvent {
            timestamp_unix_millis,
            category: "save_backup".to_owned(),
            operation: "background_worker".to_owned(),
            result: "failure".to_owned(),
            fields,
        });
    }
}
