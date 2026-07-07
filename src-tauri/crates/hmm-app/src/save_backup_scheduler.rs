use std::sync::Arc;

use hmm_core::{
    BackupCadence, GameId, ProfileBackupSchedule, ProfileId, SaveBackupStatus, SaveBackupSummary,
    SaveBackupTrigger,
};
use hmm_ports::{AppClock, ProfileRepository, ProfileSaveSettingsRepository, SaveBackupRepository};
use thiserror::Error;

use crate::StartSaveBackupTaskRequest;

const MILLIS_PER_MINUTE: u128 = 60_000;
const MILLIS_PER_DAY: u128 = 86_400_000;
const MINUTES_PER_DAY: u32 = 24 * 60;
const UNIX_EPOCH_WEEKDAY: u8 = 4;

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
    #[error("app clock is unavailable")]
    ClockUnavailable,
}

impl SaveBackupAutoSchedulerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProfileMissing => "save_backup_auto_profile_missing",
            Self::SettingsUnavailable => "save_backup_auto_settings_unavailable",
            Self::HistoryUnavailable => "save_backup_auto_history_unavailable",
            Self::ClockUnavailable => "save_backup_auto_clock_unavailable",
        }
    }
}

pub struct SaveBackupAutoSchedulerService {
    profile_repository: Arc<dyn ProfileRepository>,
    save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
    backup_repository: Arc<dyn SaveBackupRepository>,
    clock: Arc<dyn AppClock>,
}

impl SaveBackupAutoSchedulerService {
    pub fn new(
        profile_repository: Arc<dyn ProfileRepository>,
        save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
        backup_repository: Arc<dyn SaveBackupRepository>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            profile_repository,
            save_settings_repository,
            backup_repository,
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
            return Ok(SaveBackupAutoCheckResult {
                game_id: request.game_id,
                profile_id: request.profile_id,
                status: SaveBackupAutoCheckStatus::ManualOnly,
                checked_at,
                last_due_at: None,
                next_due_at: None,
                last_auto_backup_at: None,
                due_task: None,
            });
        };

        if settings.schedule.cadence == BackupCadence::Manual {
            let last_auto_backup_at = latest_completed_auto_backup_at(
                &self.backup_repository,
                &request.game_id,
                &request.profile_id,
            )?;
            return Ok(SaveBackupAutoCheckResult {
                game_id: request.game_id,
                profile_id: request.profile_id,
                status: SaveBackupAutoCheckStatus::ManualOnly,
                checked_at,
                last_due_at: None,
                next_due_at: None,
                last_auto_backup_at,
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

        let due_task = due.then(|| StartSaveBackupTaskRequest {
            game_id: request.game_id.clone(),
            profile_id: request.profile_id.clone(),
            trigger: SaveBackupTrigger::Auto,
            note: None,
        });

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
            due_task,
        })
    }
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
