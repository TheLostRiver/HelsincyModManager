use super::*;
use hmm_app::{SaveBackupAutoCheckResult, SaveBackupAutoSchedulerError};
use hmm_ports::{LocalCalendar, LocalTimeMapping};

const LOCAL_DAY: i64 = 20_000;

fn at(day: i64, minute: i64, offset_seconds: i32) -> u128 {
    (i128::from(day) * DAY_MS as i128 + i128::from(minute) * 60_000
        - i128::from(offset_seconds) * 1_000) as u128
}

fn schedule(hour: u8, minute: u8, weekdays: Vec<u8>) -> ProfileBackupSchedule {
    ProfileBackupSchedule {
        cadence: if weekdays.is_empty() {
            BackupCadence::Daily
        } else {
            BackupCadence::Weekly
        },
        hour: Some(hour),
        minute: Some(minute),
        weekdays,
    }
}

fn harness(
    now: u128,
    calendar: Arc<dyn LocalCalendar>,
    schedule: ProfileBackupSchedule,
) -> Harness {
    let harness = Harness::with_calendar(now, calendar);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(schedule));
    harness
}

fn check(harness: &Harness) -> Result<SaveBackupAutoCheckResult, SaveBackupAutoSchedulerError> {
    harness.scheduler.check_profile(SaveBackupAutoCheckRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
    })
}

#[test]
fn daily_offsets_preserve_minutes_and_do_not_fire_before_the_local_slot() {
    for offset in [
        0,
        8 * 3600,
        -4 * 3600,
        -5 * 3600,
        3600,
        2 * 3600,
        20_700,
        45_900,
        -12_600,
    ] {
        let due = at(LOCAL_DAY, 8 * 60 + 17, offset);
        for now in [due - 1, due, due + 1] {
            let harness = harness(
                now,
                Arc::new(FixedLocalCalendar(offset)),
                schedule(8, 17, vec![]),
            );
            harness.insert_backup(auto_summary("previous", due - DAY_MS + 60_000));
            let result = check(&harness).unwrap();
            assert_eq!(
                result.status,
                if now < due {
                    SaveBackupAutoCheckStatus::NotDue
                } else {
                    SaveBackupAutoCheckStatus::Due
                },
                "offset={offset} now={now}"
            );
            assert_eq!(
                result.next_due_at,
                Some(if now < due { due } else { due + DAY_MS })
            );
            if now >= due {
                harness.insert_backup(auto_summary("current", now));
                assert_eq!(
                    check(&harness).unwrap().status,
                    SaveBackupAutoCheckStatus::NotDue
                );
            }
        }
    }
}

#[test]
fn weekly_uses_local_weekday_even_when_utc_is_on_another_date() {
    let sunday = LOCAL_DAY + 2;
    for (offset, hour) in [(8 * 3600, 1), (8 * 3600, 20), (-5 * 3600, 23)] {
        let due = at(sunday, i64::from(hour) * 60, offset);
        for now in [due - 1, due] {
            let harness = harness(
                now,
                Arc::new(FixedLocalCalendar(offset)),
                schedule(hour, 0, vec![0]),
            );
            harness.insert_backup(auto_summary("previous", due - 7 * DAY_MS + 1));
            let result = check(&harness).unwrap();
            assert_eq!(
                result.next_due_at,
                Some(if now < due { due } else { due + 7 * DAY_MS })
            );
            assert_eq!(
                result.status,
                if now < due {
                    SaveBackupAutoCheckStatus::NotDue
                } else {
                    SaveBackupAutoCheckStatus::Due
                }
            );
        }
    }
}

struct TransitionCalendar {
    transition: u128,
    before: i32,
    after: i32,
}

impl TransitionCalendar {
    fn offset_at(&self, instant: i128) -> i32 {
        if instant < self.transition as i128 {
            self.before
        } else {
            self.after
        }
    }
}

impl LocalCalendar for TransitionCalendar {
    fn local_day_at(&self, instant: u128) -> Result<i64> {
        FixedLocalCalendar(self.offset_at(instant as i128)).local_day_at(instant)
    }

    fn resolve_local_minute(&self, day: i64, minute: u16) -> Result<LocalTimeMapping> {
        let mut candidates = Vec::new();
        for offset in [self.before, self.after] {
            let instant = at(day, i64::from(minute), offset) as i128;
            if self.offset_at(instant) == offset {
                candidates.push(instant);
            }
        }
        candidates.sort();
        candidates.dedup();
        Ok(match candidates.as_slice() {
            [] => LocalTimeMapping::Nonexistent,
            [one] => LocalTimeMapping::Single(*one),
            [first, second] => LocalTimeMapping::Ambiguous(*first, *second),
            _ => unreachable!(),
        })
    }
}

#[test]
fn dst_transition_changes_daily_elapsed_interval_not_the_local_eight() {
    for (before, after, transition_hour, elapsed_hours) in [(-5, -4, 7, 23), (-4, -5, 6, 25)] {
        let calendar = Arc::new(TransitionCalendar {
            transition: at(LOCAL_DAY, transition_hour * 60, 0),
            before: before * 3600,
            after: after * 3600,
        });
        let previous = at(LOCAL_DAY - 1, 8 * 60, before * 3600);
        let due = at(LOCAL_DAY, 8 * 60, after * 3600);
        let harness = harness(previous, calendar, schedule(8, 0, vec![]));
        harness.insert_backup(auto_summary("previous", previous));
        let result = check(&harness).unwrap();
        assert_eq!(result.next_due_at, Some(due));
        assert_eq!(due - previous, elapsed_hours * HOUR_MS);
    }
}

#[test]
fn spring_gap_moves_to_first_valid_minute_not_an_hour_later() {
    let transition = at(LOCAL_DAY, 7 * 60, 0); // 本地 02:00 -> 03:00。
    for now in [transition - 1, transition] {
        let calendar = Arc::new(TransitionCalendar {
            transition,
            before: -5 * 3600,
            after: -4 * 3600,
        });
        let harness = harness(now, calendar, schedule(2, 30, vec![]));
        harness.insert_backup(auto_summary("previous", at(LOCAL_DAY - 1, 150, -5 * 3600)));
        let result = check(&harness).unwrap();
        assert_eq!(
            result.next_due_at,
            Some(if now < transition {
                transition
            } else {
                at(LOCAL_DAY + 1, 150, -4 * 3600)
            })
        );
        assert_eq!(
            result.status,
            if now < transition {
                SaveBackupAutoCheckStatus::NotDue
            } else {
                SaveBackupAutoCheckStatus::Due
            }
        );
    }
}

#[test]
fn fall_repeated_minute_uses_first_occurrence_and_never_starts_a_second_backup() {
    let transition = at(LOCAL_DAY, 6 * 60, 0);
    let first = at(LOCAL_DAY, 90, -4 * 3600);
    for now in [first, transition + 45 * 60_000] {
        let calendar = Arc::new(TransitionCalendar {
            transition,
            before: -4 * 3600,
            after: -5 * 3600,
        });
        let harness = harness(now, calendar, schedule(1, 30, vec![]));
        harness.insert_backup(auto_summary("completed", first));
        let result = check(&harness).unwrap();
        assert_eq!(result.last_due_at, Some(first));
        assert_eq!(result.next_due_at, Some(at(LOCAL_DAY + 1, 90, -5 * 3600)));
        assert_eq!(result.status, SaveBackupAutoCheckStatus::NotDue);
        assert!(result.due_task.is_none());
        assert!(harness
            .scheduler_state_repository
            .lease_requests()
            .is_empty());
    }
}

struct MutableCalendar(Mutex<i32>);
impl LocalCalendar for MutableCalendar {
    fn local_day_at(&self, instant: u128) -> Result<i64> {
        FixedLocalCalendar(*self.0.lock().unwrap()).local_day_at(instant)
    }
    fn resolve_local_minute(&self, day: i64, minute: u16) -> Result<LocalTimeMapping> {
        FixedLocalCalendar(*self.0.lock().unwrap()).resolve_local_minute(day, minute)
    }
}

#[test]
fn timezone_change_recalculates_cached_next_due_without_shifting_history_or_lease() {
    let now = at(LOCAL_DAY, 30, 0);
    let previous = at(LOCAL_DAY - 1, 9 * 60, 0);
    let calendar = Arc::new(MutableCalendar(Mutex::new(0)));
    let harness = harness(now, calendar.clone(), schedule(8, 0, vec![]));
    harness.insert_backup(auto_summary("previous", previous));
    assert_eq!(
        check(&harness).unwrap().next_due_at,
        Some(at(LOCAL_DAY, 8 * 60, 0))
    );
    *calendar.0.lock().unwrap() = 8 * 3600;
    let result = check(&harness).unwrap();
    assert_eq!(result.status, SaveBackupAutoCheckStatus::Due);
    assert_eq!(result.last_auto_backup_at, Some(previous));
    let state = harness.scheduler_state_repository.latest_state().unwrap();
    assert_eq!(state.next_due_at, Some(at(LOCAL_DAY + 1, 0, 0)));
    let requests = harness.scheduler_state_repository.lease_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].lease_expires_at, now + 5 * 60_000);
}

struct UnavailableCalendar(bool);
impl LocalCalendar for UnavailableCalendar {
    fn local_day_at(&self, _: u128) -> Result<i64> {
        if self.0 {
            anyhow::bail!("calendar unavailable")
        }
        Ok(LOCAL_DAY)
    }
    fn resolve_local_minute(&self, _: i64, _: u16) -> Result<LocalTimeMapping> {
        Ok(LocalTimeMapping::Nonexistent)
    }
}

#[test]
fn unavailable_calendar_or_unresolvable_gap_fails_before_state_lease_or_task() {
    for unavailable in [true, false] {
        let harness = harness(
            at(LOCAL_DAY, 0, 0),
            Arc::new(UnavailableCalendar(unavailable)),
            schedule(8, 0, vec![]),
        );
        assert_eq!(
            check(&harness).unwrap_err().code(),
            "save_backup_auto_timezone_unavailable"
        );
        assert!(harness
            .scheduler_state_repository
            .lease_requests()
            .is_empty());
        assert!(harness.scheduler_state_repository.latest_state().is_none());
    }
}

#[test]
fn manual_schedule_does_not_require_a_timezone_and_epoch_weekly_does_not_overflow() {
    let manual = harness(
        0,
        Arc::new(UnavailableCalendar(true)),
        ProfileBackupSchedule::manual(),
    );
    assert_eq!(
        check(&manual).unwrap().status,
        SaveBackupAutoCheckStatus::ManualOnly
    );
    let weekly = harness(0, Arc::new(FixedLocalCalendar(0)), schedule(0, 0, vec![4]));
    let result = check(&weekly).unwrap();
    assert_eq!(result.last_due_at, Some(0));
    assert_eq!(result.next_due_at, Some(7 * DAY_MS));
}

#[test]
fn missed_local_slots_catch_up_once_and_first_enable_keeps_existing_catch_up_semantics() {
    let now = at(LOCAL_DAY, 10 * 60, 8 * 3600);
    let harness = harness(
        now,
        Arc::new(FixedLocalCalendar(8 * 3600)),
        schedule(8, 0, vec![]),
    );
    assert_eq!(
        check(&harness).unwrap().status,
        SaveBackupAutoCheckStatus::Due
    );
    harness.insert_backup(auto_summary("catch-up", now));
    let checked = check(&harness).unwrap();
    assert_eq!(checked.status, SaveBackupAutoCheckStatus::NotDue);
    assert_eq!(
        checked.next_due_at,
        Some(at(LOCAL_DAY + 1, 8 * 60, 8 * 3600))
    );
}
