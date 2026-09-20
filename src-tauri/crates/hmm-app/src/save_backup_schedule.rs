use anyhow::{bail, Context, Result};
use hmm_core::{BackupCadence, ProfileBackupSchedule};
use hmm_ports::{LocalCalendar, LocalTimeMapping};

const MINUTES_PER_DAY: u16 = 24 * 60;

#[derive(Default)]
pub(super) struct ScheduleWindow {
    pub last_due_at: Option<u128>,
    pub next_due_at: Option<u128>,
}

pub(super) fn schedule_window(
    schedule: &ProfileBackupSchedule,
    now_unix_millis: u128,
    calendar: &dyn LocalCalendar,
) -> Result<ScheduleWindow> {
    let mut window = ScheduleWindow::default();
    let (Some(hour), Some(minute)) = (schedule.hour, schedule.minute) else {
        return Ok(window);
    };
    if schedule.cadence == BackupCadence::Manual || hour >= 24 || minute >= 60 {
        return Ok(window);
    }
    if schedule.cadence == BackupCadence::Weekly && !schedule.weekdays.iter().any(|day| *day <= 6) {
        return Ok(window);
    }

    // 这里加减的是当地公历日期，绝不能把 UTC instant 加减固定 24 小时当作下一天。
    let today = calendar.local_day_at(now_unix_millis)?;
    let minute_of_day = u16::from(hour) * 60 + u16::from(minute);
    for delta in -7..=7 {
        let day = today
            .checked_add(delta)
            .context("local date out of range")?;
        let weekday = ((day.rem_euclid(7) + 4) % 7) as u8;
        if schedule.cadence == BackupCadence::Weekly && !schedule.weekdays.contains(&weekday) {
            continue;
        }
        let Some(slot) = resolve_slot(calendar, day, minute_of_day)? else {
            continue;
        };
        if slot <= now_unix_millis {
            window.last_due_at = Some(window.last_due_at.map_or(slot, |last| last.max(slot)));
        } else {
            window.next_due_at = Some(window.next_due_at.map_or(slot, |next| next.min(slot)));
        }
    }
    Ok(window)
}

fn resolve_slot(
    calendar: &dyn LocalCalendar,
    day: i64,
    minute_of_day: u16,
) -> Result<Option<u128>> {
    // 春季跳时：顺延到第一个有效分钟。也覆盖国际日期变更线造成的整日跳过。
    // 有限扫描避免时区不可用被当作无限重试或静默回退 UTC。
    for forward in 0..=MINUTES_PER_DAY {
        let minute = minute_of_day + forward;
        let candidate_day = day
            .checked_add(i64::from(minute / MINUTES_PER_DAY))
            .context("local date out of range")?;
        let instant =
            match calendar.resolve_local_minute(candidate_day, minute % MINUTES_PER_DAY)? {
                LocalTimeMapping::Single(instant) => instant,
                // 秋季回拨：始终使用第一次出现的钟点，后续检查也不会生成第二个槽位。
                LocalTimeMapping::Ambiguous(first, second) => first.min(second),
                LocalTimeMapping::Nonexistent => continue,
            };
        return Ok(u128::try_from(instant).ok());
    }
    bail!("local schedule time could not be resolved")
}
