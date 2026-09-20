use anyhow::Result;
use hmm_ports::{LocalCalendar, LocalTimeMapping};

#[derive(Default)]
pub struct FixedLocalCalendar(pub i32);

impl LocalCalendar for FixedLocalCalendar {
    fn local_day_at(&self, unix_millis: u128) -> Result<i64> {
        let local_millis = i128::try_from(unix_millis)? + i128::from(self.0) * 1_000;
        Ok(i64::try_from(local_millis.div_euclid(86_400_000))?)
    }

    fn resolve_local_minute(&self, day: i64, minute_of_day: u16) -> Result<LocalTimeMapping> {
        Ok(LocalTimeMapping::Single(
            i128::from(day) * 86_400_000 + i128::from(minute_of_day) * 60_000
                - i128::from(self.0) * 1_000,
        ))
    }
}
