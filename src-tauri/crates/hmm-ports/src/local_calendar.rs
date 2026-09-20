use anyhow::Result;

/// 本地钟点到 UTC 的映射。允许 epoch 前的有符号毫秒，调用方决定是否保留。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalTimeMapping {
    Single(i128),
    Ambiguous(i128, i128),
    Nonexistent,
}

/// 系统本地日历能力；实现方读取目标日期的时区规则，不能缓存「当前 UTC 偏移」。
///
/// `day` 是公历日期相对 1970-01-01 的有符号日序号，不是 UTC 时间戳除以 24 小时。
/// `minute_of_day` 为 0..1440 的本地墙上钟点；歧义/跳时策略由应用层决定。
pub trait LocalCalendar: Send + Sync {
    fn local_day_at(&self, unix_millis: u128) -> Result<i64>;
    fn resolve_local_minute(&self, day: i64, minute_of_day: u16) -> Result<LocalTimeMapping>;
}
