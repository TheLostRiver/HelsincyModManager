use std::collections::BTreeMap;
use std::sync::Arc;

use hmm_core::{
    BackupCadence, GameId, ProfileBackupSchedule, ProfileId, SaveBackupBackgroundProtectionStatus,
    SaveBackupSchedulerLeaseRequest, SaveBackupSchedulerPendingReason, SaveBackupSchedulerState,
    SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, GameRunningDetector, GameRunningStatus,
    ProfileRepository, ProfileSaveSettingsRepository, SaveBackupRepository,
    SaveBackupSchedulerStateRepository,
};
use thiserror::Error;

use crate::StartSaveBackupTaskRequest;

const MILLIS_PER_MINUTE: u128 = 60_000;
const MILLIS_PER_DAY: u128 = 86_400_000;
const MINUTES_PER_DAY: u32 = 24 * 60;
const UNIX_EPOCH_WEEKDAY: u8 = 4;
const SCHEDULER_LEASE_TTL_MILLIS: u128 = 5 * MILLIS_PER_MINUTE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupAutoCheckRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveBackupAutoCheckStatus {
    ManualOnly,
    NotDue,
    Due,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupAutoCheckResult {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub status: SaveBackupAutoCheckStatus,
    pub checked_at: u128,
    pub last_due_at: Option<u128>,
    pub next_due_at: Option<u128>,
    pub last_auto_backup_at: Option<u128>,
    pub pending_reason: Option<SaveBackupSchedulerPendingReason>,
    pub due_task: Option<StartSaveBackupTaskRequest>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SaveBackupAutoSchedulerError {
    #[error("profile is missing")]
    ProfileMissing,
    #[error("profile save settings are unavailable")]
    SettingsUnavailable,
    #[error("save backup history is unavailable")]
    HistoryUnavailable,
    #[error("save backup scheduler state is unavailable")]
    SchedulerStateUnavailable,
    #[error("app clock is unavailable")]
    ClockUnavailable,
}

impl SaveBackupAutoSchedulerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProfileMissing => "save_backup_auto_profile_missing",
            Self::SettingsUnavailable => "save_backup_auto_settings_unavailable",
            Self::HistoryUnavailable => "save_backup_auto_history_unavailable",
            Self::SchedulerStateUnavailable => "save_backup_scheduler_unavailable",
            Self::ClockUnavailable => "save_backup_auto_clock_unavailable",
        }
    }
}

pub struct SaveBackupAutoSchedulerService {
    profile_repository: Arc<dyn ProfileRepository>,
    save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
    backup_repository: Arc<dyn SaveBackupRepository>,
    scheduler_state_repository: Arc<dyn SaveBackupSchedulerStateRepository>,
    game_running_detector: Arc<dyn GameRunningDetector>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
}

impl SaveBackupAutoSchedulerService {
    pub fn new(
        profile_repository: Arc<dyn ProfileRepository>,
        save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
        backup_repository: Arc<dyn SaveBackupRepository>,
        scheduler_state_repository: Arc<dyn SaveBackupSchedulerStateRepository>,
        game_running_detector: Arc<dyn GameRunningDetector>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            profile_repository,
            save_settings_repository,
            backup_repository,
            scheduler_state_repository,
            game_running_detector,
            audit_log,
            clock,
        }
    }

    pub fn check_profile(
        &self,
        request: SaveBackupAutoCheckRequest,
    ) -> Result<SaveBackupAutoCheckResult, SaveBackupAutoSchedulerError> {
        self.profile_repository
            .get(request.profile_id.as_str())
            .map_err(|_| SaveBackupAutoSchedulerError::SettingsUnavailable)?
            .ok_or(SaveBackupAutoSchedulerError::ProfileMissing)?;

        let checked_at = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveBackupAutoSchedulerError::ClockUnavailable)?;
        let settings = self
            .save_settings_repository
            .get_settings(request.profile_id.as_str())
            .map_err(|_| SaveBackupAutoSchedulerError::SettingsUnavailable)?;

        let Some(settings) = settings else {
            self.save_scheduler_state(
                &request.game_id,
                &request.profile_id,
                None,
                SchedulerStateUpdate {
                    enabled: false,
                    checked_at,
                    next_due_at: None,
                    last_success_at: None,
                    pending_reason: None,
                },
            )?;
            return Ok(SaveBackupAutoCheckResult {
                game_id: request.game_id,
                profile_id: request.profile_id,
                status: SaveBackupAutoCheckStatus::ManualOnly,
                checked_at,
                last_due_at: None,
                next_due_at: None,
                last_auto_backup_at: None,
                pending_reason: None,
                due_task: None,
            });
        };

        if settings.schedule.cadence == BackupCadence::Manual {
            let last_auto_backup_at = latest_completed_auto_backup_at(
                &self.backup_repository,
                &request.game_id,
                &request.profile_id,
            )?;
            self.save_scheduler_state(
                &request.game_id,
                &request.profile_id,
                None,
                SchedulerStateUpdate {
                    enabled: false,
                    checked_at,
                    next_due_at: None,
                    last_success_at: last_auto_backup_at,
                    pending_reason: None,
                },
            )?;
            return Ok(SaveBackupAutoCheckResult {
                game_id: request.game_id,
                profile_id: request.profile_id,
                status: SaveBackupAutoCheckStatus::ManualOnly,
                checked_at,
                last_due_at: None,
                next_due_at: None,
                last_auto_backup_at,
                pending_reason: None,
                due_task: None,
            });
        }

        let schedule_window = schedule_window(&settings.schedule, checked_at);
        let last_auto_backup_at = latest_completed_auto_backup_at(
            &self.backup_repository,
            &request.game_id,
            &request.profile_id,
        )?;
        let due = schedule_window
            .last_due_at
            .map(|last_due_at| last_auto_backup_at.is_none_or(|backup_at| backup_at < last_due_at))
            .unwrap_or(false);

        // due 时先做游戏运行检测：运行中或无法判断都保守延后，
        // 不获取租约、不启动任务，避免复制游戏正在写入的存档。
        let pending_reason = if due {
            match self
                .game_running_detector
                .game_running_status(&request.game_id)
            {
                GameRunningStatus::Running => Some(SaveBackupSchedulerPendingReason::GameRunning),
                GameRunningStatus::Unknown => {
                    Some(SaveBackupSchedulerPendingReason::GameRunningUnknown)
                }
                GameRunningStatus::NotRunning => None,
            }
        } else {
            None
        };

        let existing_state = self
            .scheduler_state_repository
            .get_state(&request.game_id, &request.profile_id)
            .map_err(|_| SaveBackupAutoSchedulerError::SchedulerStateUnavailable)?;
        let previous_pending_reason = existing_state
            .as_ref()
            .and_then(|state| state.pending_reason);

        self.save_scheduler_state(
            &request.game_id,
            &request.profile_id,
            existing_state,
            SchedulerStateUpdate {
                enabled: true,
                checked_at,
                next_due_at: schedule_window.next_due_at,
                last_success_at: last_auto_backup_at,
                pending_reason,
            },
        )?;

        // 只在 pending 原因翻转时记一条审计，避免游戏运行期间每次检查都刷屏。
        if pending_reason != previous_pending_reason {
            if let Some(reason) = pending_reason {
                self.record_deferred_audit(
                    &request.game_id,
                    &request.profile_id,
                    reason,
                    checked_at,
                );
            }
        }

        let due_task = if due && pending_reason.is_none() {
            let lease_owner =
                scheduler_lease_owner(&request.game_id, &request.profile_id, checked_at);
            let acquired = self
                .scheduler_state_repository
                .acquire_due_lease(SaveBackupSchedulerLeaseRequest {
                    game_id: request.game_id.clone(),
                    profile_id: request.profile_id.clone(),
                    lease_owner: lease_owner.clone(),
                    lease_expires_at: checked_at + SCHEDULER_LEASE_TTL_MILLIS,
                    now_unix_millis: checked_at,
                    last_checked_at: Some(checked_at),
                    next_due_at: schedule_window.next_due_at,
                })
                .map_err(|_| SaveBackupAutoSchedulerError::SchedulerStateUnavailable)?;

            acquired.map(|_| StartSaveBackupTaskRequest {
                game_id: request.game_id.clone(),
                profile_id: request.profile_id.clone(),
                trigger: SaveBackupTrigger::Auto,
                note: None,
                scheduler_lease_owner: Some(lease_owner),
            })
        } else {
            None
        };

        Ok(SaveBackupAutoCheckResult {
            game_id: request.game_id,
            profile_id: request.profile_id,
            status: if due {
                SaveBackupAutoCheckStatus::Due
            } else {
                SaveBackupAutoCheckStatus::NotDue
            },
            checked_at,
            last_due_at: schedule_window.last_due_at,
            next_due_at: schedule_window.next_due_at,
            last_auto_backup_at,
            pending_reason,
            due_task,
        })
    }

    fn record_deferred_audit(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        reason: SaveBackupSchedulerPendingReason,
        checked_at: u128,
    ) {
        let error_code = match reason {
            SaveBackupSchedulerPendingReason::GameRunning => "save_backup_auto_skipped_game_running",
            SaveBackupSchedulerPendingReason::GameRunningUnknown => {
                "save_backup_auto_skipped_game_running_unknown"
            }
            SaveBackupSchedulerPendingReason::SourceInvalid => "save_backup_auto_source_invalid",
            SaveBackupSchedulerPendingReason::DestinationUnavailable => {
                "save_backup_auto_destination_unavailable"
            }
            SaveBackupSchedulerPendingReason::TaskConflict => "save_backup_auto_task_conflict",
        };
        let fields = BTreeMap::from([
            ("game_id".to_owned(), game_id.as_str().to_owned()),
            ("profile_id".to_owned(), profile_id.as_str().to_owned()),
            ("trigger".to_owned(), SaveBackupTrigger::Auto.as_str().to_owned()),
            ("error_code".to_owned(), error_code.to_owned()),
        ]);
        // 审计写失败不阻塞调度检查。
        let _ = self.audit_log.record(AuditLogEvent {
            timestamp_unix_millis: checked_at,
            category: "save_backup".to_owned(),
            operation: "auto_backup".to_owned(),
            result: "deferred".to_owned(),
            fields,
        });
    }

    fn save_scheduler_state(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        existing: Option<SaveBackupSchedulerState>,
        update: SchedulerStateUpdate,
    ) -> Result<(), SaveBackupAutoSchedulerError> {
        let existing = match existing {
            Some(state) => Some(state),
            None => self
                .scheduler_state_repository
                .get_state(game_id, profile_id)
                .map_err(|_| SaveBackupAutoSchedulerError::SchedulerStateUnavailable)?,
        };
        let state = scheduler_state_for_check(game_id, profile_id, existing, update);
        self.scheduler_state_repository
            .upsert_state(&state)
            .map_err(|_| SaveBackupAutoSchedulerError::SchedulerStateUnavailable)
    }
}

struct SchedulerStateUpdate {
    enabled: bool,
    checked_at: u128,
    next_due_at: Option<u128>,
    last_success_at: Option<u128>,
    pending_reason: Option<SaveBackupSchedulerPendingReason>,
}

fn scheduler_state_for_check(
    game_id: &GameId,
    profile_id: &ProfileId,
    existing: Option<SaveBackupSchedulerState>,
    update: SchedulerStateUpdate,
) -> SaveBackupSchedulerState {
    let background_status = if update.enabled {
        existing
            .as_ref()
            .map(|state| match state.background_status {
                SaveBackupBackgroundProtectionStatus::NotEnabled => {
                    SaveBackupBackgroundProtectionStatus::TrayOnly
                }
                status => status,
            })
            .unwrap_or(SaveBackupBackgroundProtectionStatus::TrayOnly)
    } else {
        SaveBackupBackgroundProtectionStatus::NotEnabled
    };

    SaveBackupSchedulerState {
        game_id: game_id.clone(),
        profile_id: profile_id.clone(),
        enabled: update.enabled,
        background_protection_enabled: update.enabled
            && existing
                .as_ref()
                .is_some_and(|state| state.background_protection_enabled),
        background_status,
        last_checked_at: Some(update.checked_at),
        last_attempt_at: existing.as_ref().and_then(|state| state.last_attempt_at),
        last_success_at: update
            .last_success_at
            .or_else(|| existing.as_ref().and_then(|state| state.last_success_at)),
        next_due_at: update.next_due_at,
        pending_reason: if update.enabled {
            update.pending_reason
        } else {
            None
        },
        last_error_code: existing
            .as_ref()
            .and_then(|state| state.last_error_code.clone()),
        worker_instance_id: existing
            .as_ref()
            .and_then(|state| state.worker_instance_id.clone()),
        worker_heartbeat_at: existing
            .as_ref()
            .and_then(|state| state.worker_heartbeat_at),
        // 租约字段只能由 release_lease 按 owner 清理；
        // 禁用/切换 Manual 时保留在途租约，避免重新启用后并发启动第二个 auto 任务。
        lease_owner: existing
            .as_ref()
            .and_then(|state| state.lease_owner.clone()),
        lease_expires_at: existing.as_ref().and_then(|state| state.lease_expires_at),
        updated_at: update.checked_at,
    }
}

fn scheduler_lease_owner(game_id: &GameId, profile_id: &ProfileId, checked_at: u128) -> String {
    format!(
        "client-runtime:{}:{}:{checked_at}",
        game_id.as_str(),
        profile_id.as_str()
    )
}

struct ScheduleWindow {
    last_due_at: Option<u128>,
    next_due_at: Option<u128>,
}

fn latest_completed_auto_backup_at(
    repository: &Arc<dyn SaveBackupRepository>,
    game_id: &GameId,
    profile_id: &ProfileId,
) -> Result<Option<u128>, SaveBackupAutoSchedulerError> {
    let backups = repository
        .list_for_profile(game_id, profile_id, None)
        .map_err(|_| SaveBackupAutoSchedulerError::HistoryUnavailable)?;

    Ok(backups
        .into_iter()
        .filter(is_completed_auto_backup)
        .map(|summary| summary.created_at)
        .max())
}

fn is_completed_auto_backup(summary: &SaveBackupSummary) -> bool {
    summary.trigger == SaveBackupTrigger::Auto && summary.status == SaveBackupStatus::Completed
}

fn schedule_window(schedule: &ProfileBackupSchedule, now_unix_millis: u128) -> ScheduleWindow {
    match schedule.cadence {
        BackupCadence::Manual => ScheduleWindow {
            last_due_at: None,
            next_due_at: None,
        },
        BackupCadence::Daily => daily_window(schedule, now_unix_millis),
        BackupCadence::Weekly => weekly_window(schedule, now_unix_millis),
    }
}

fn daily_window(schedule: &ProfileBackupSchedule, now_unix_millis: u128) -> ScheduleWindow {
    let Some(schedule_minute) = schedule_minute_of_day(schedule) else {
        return ScheduleWindow {
            last_due_at: None,
            next_due_at: None,
        };
    };
    let now_day = (now_unix_millis / MILLIS_PER_DAY) as i128;
    let today_due = slot_at_day(now_day, schedule_minute);
    let last_due_day = if today_due <= now_unix_millis {
        now_day
    } else {
        now_day - 1
    };
    let next_due_day = last_due_day + 1;

    ScheduleWindow {
        last_due_at: slot_at_day_checked(last_due_day, schedule_minute),
        next_due_at: slot_at_day_checked(next_due_day, schedule_minute),
    }
}

fn weekly_window(schedule: &ProfileBackupSchedule, now_unix_millis: u128) -> ScheduleWindow {
    let Some(schedule_minute) = schedule_minute_of_day(schedule) else {
        return ScheduleWindow {
            last_due_at: None,
            next_due_at: None,
        };
    };
    if schedule.weekdays.is_empty() {
        return ScheduleWindow {
            last_due_at: None,
            next_due_at: None,
        };
    }

    let now_day = (now_unix_millis / MILLIS_PER_DAY) as i128;
    let current_weekday = weekday_for_day(now_day);
    let mut last_due_at: Option<u128> = None;
    let mut next_due_at: Option<u128> = None;

    for weekday in schedule.weekdays.iter().copied().filter(|day| *day <= 6) {
        let days_since_weekday =
            (i16::from(current_weekday) - i16::from(weekday)).rem_euclid(7) as i128;
        let mut candidate_day = now_day - days_since_weekday;
        let mut candidate = slot_at_day(candidate_day, schedule_minute);
        if candidate > now_unix_millis {
            candidate_day -= 7;
            candidate = slot_at_day(candidate_day, schedule_minute);
        }
        if candidate_day >= 0 {
            last_due_at = Some(last_due_at.map_or(candidate, |current| current.max(candidate)));
        }

        let days_until_weekday =
            (i16::from(weekday) - i16::from(current_weekday)).rem_euclid(7) as i128;
        let mut next_day = now_day + days_until_weekday;
        let mut next_candidate = slot_at_day(next_day, schedule_minute);
        if next_candidate <= now_unix_millis {
            next_day += 7;
            next_candidate = slot_at_day(next_day, schedule_minute);
        }
        next_due_at =
            Some(next_due_at.map_or(next_candidate, |current| current.min(next_candidate)));
    }

    ScheduleWindow {
        last_due_at,
        next_due_at,
    }
}

fn schedule_minute_of_day(schedule: &ProfileBackupSchedule) -> Option<u32> {
    let hour = u32::from(schedule.hour?);
    let minute = u32::from(schedule.minute?);
    if hour >= 24 || minute >= 60 {
        return None;
    }
    let minute_of_day = hour * 60 + minute;
    if minute_of_day >= MINUTES_PER_DAY {
        return None;
    }
    Some(minute_of_day)
}

fn slot_at_day(day: i128, minute_of_day: u32) -> u128 {
    ((day as u128) * MILLIS_PER_DAY) + (u128::from(minute_of_day) * MILLIS_PER_MINUTE)
}

fn slot_at_day_checked(day: i128, minute_of_day: u32) -> Option<u128> {
    if day < 0 {
        return None;
    }
    Some(slot_at_day(day, minute_of_day))
}

fn weekday_for_day(day: i128) -> u8 {
    ((day + i128::from(UNIX_EPOCH_WEEKDAY)).rem_euclid(7)) as u8
}
