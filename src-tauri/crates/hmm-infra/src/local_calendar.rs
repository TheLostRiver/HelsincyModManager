use anyhow::{ensure, Context, Result};
use chrono::{DateTime, Local, LocalResult, NaiveDate, TimeDelta, TimeZone};
use hmm_ports::{LocalCalendar, LocalTimeMapping};

/// 不保存启动时的偏移；每次查询都使用操作系统中目标日期的时区/DST 规则。
pub struct SystemLocalCalendar;

impl LocalCalendar for SystemLocalCalendar {
    fn local_day_at(&self, unix_millis: u128) -> Result<i64> {
        // chrono 的 Local UTC->local 接口在平台时区查询失败时可能 panic。
        // 将其限制在 adapter 内，不让调度器回退 UTC 或启动错误时刻的备份。
        std::panic::catch_unwind(|| local_day_at(&Local, unix_millis))
            .map_err(|_| anyhow::anyhow!("system local calendar unavailable"))?
    }

    fn resolve_local_minute(&self, day: i64, minute_of_day: u16) -> Result<LocalTimeMapping> {
        resolve_local_minute(&Local, day, minute_of_day)
    }
}

fn epoch_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(1970, 1, 1).expect("valid epoch date")
}

fn local_day_at<T: TimeZone>(zone: &T, unix_millis: u128) -> Result<i64> {
    let utc = DateTime::from_timestamp_millis(i64::try_from(unix_millis)?)
        .context("timestamp out of calendar range")?;
    let local_date = utc.with_timezone(zone).date_naive();
    Ok(local_date.signed_duration_since(epoch_date()).num_days())
}

fn resolve_local_minute<T: TimeZone>(
    zone: &T,
    day: i64,
    minute_of_day: u16,
) -> Result<LocalTimeMapping> {
    ensure!(minute_of_day < 1440, "invalid local minute");
    let date = epoch_date()
        .checked_add_signed(TimeDelta::try_days(day).context("local date out of range")?)
        .context("local date out of range")?;
    let local = date
        .and_hms_opt(
            u32::from(minute_of_day / 60),
            u32::from(minute_of_day % 60),
            0,
        )
        .context("invalid local time")?;
    Ok(match zone.from_local_datetime(&local) {
        LocalResult::Single(instant) => {
            LocalTimeMapping::Single(i128::from(instant.timestamp_millis()))
        }
        LocalResult::Ambiguous(first, second) => LocalTimeMapping::Ambiguous(
            i128::from(first.timestamp_millis()),
            i128::from(second.timestamp_millis()),
        ),
        LocalResult::None => LocalTimeMapping::Nonexistent,
    })
}

#[cfg(test)]
mod tests;
