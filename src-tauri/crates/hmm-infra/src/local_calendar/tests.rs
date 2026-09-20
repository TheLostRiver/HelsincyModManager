use super::*;
use chrono::FixedOffset;
use chrono_tz::{America::New_York, Asia::Shanghai, Europe::Berlin};

fn millis(value: &str) -> u128 {
    DateTime::parse_from_rfc3339(value)
        .unwrap()
        .timestamp_millis() as u128
}

fn day(value: &str) -> i64 {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .unwrap()
        .signed_duration_since(epoch_date())
        .num_days()
}

fn single(zone: &impl TimeZone, date: &str, hour: u16) -> i128 {
    match resolve_local_minute(zone, day(date), hour * 60).unwrap() {
        LocalTimeMapping::Single(value) => value,
        other => panic!("expected unique local time, got {other:?}"),
    }
}

#[test]
fn local_date_uses_the_timezone_not_the_utc_date() {
    assert_eq!(
        local_day_at(&Shanghai, millis("2026-09-19T20:00:00Z")).unwrap(),
        day("2026-09-20")
    );
    assert_eq!(
        local_day_at(&New_York, millis("2026-09-20T01:00:00Z")).unwrap(),
        day("2026-09-19")
    );
}

#[test]
fn local_eight_maps_to_each_regions_target_date_offset() {
    for (zone, date, expected) in [
        (Shanghai, "2026-09-20", "2026-09-20T00:00:00Z"),
        (New_York, "2026-09-20", "2026-09-20T12:00:00Z"),
        (New_York, "2026-01-15", "2026-01-15T13:00:00Z"),
        (Berlin, "2026-09-20", "2026-09-20T06:00:00Z"),
        (Berlin, "2026-01-15", "2026-01-15T07:00:00Z"),
        (
            chrono_tz::Asia::Kathmandu,
            "2026-09-20",
            "2026-09-20T02:15:00Z",
        ),
    ] {
        assert_eq!(
            single(&zone, date, 8),
            millis(expected) as i128,
            "{zone} {date}"
        );
    }
}

#[test]
fn daily_calendar_slots_span_twenty_three_or_twenty_five_hours() {
    for (before, after, hours) in [
        ("2026-03-07", "2026-03-08", 23),
        ("2026-10-31", "2026-11-01", 25),
    ] {
        assert_eq!(
            single(&New_York, after, 8) - single(&New_York, before, 8),
            hours * 3_600_000
        );
    }
}

#[test]
fn adapter_preserves_gap_and_overlap_instead_of_guessing_an_offset() {
    assert_eq!(
        resolve_local_minute(&New_York, day("2026-03-08"), 150).unwrap(),
        LocalTimeMapping::Nonexistent
    );
    assert_eq!(
        resolve_local_minute(&Berlin, day("2026-03-29"), 150).unwrap(),
        LocalTimeMapping::Nonexistent
    );
    for (zone, date, minute, first, second) in [
        (
            New_York,
            "2026-11-01",
            90,
            "2026-11-01T05:30:00Z",
            "2026-11-01T06:30:00Z",
        ),
        (
            Berlin,
            "2026-10-25",
            150,
            "2026-10-25T00:30:00Z",
            "2026-10-25T01:30:00Z",
        ),
    ] {
        let LocalTimeMapping::Ambiguous(a, b) =
            resolve_local_minute(&zone, day(date), minute).unwrap()
        else {
            panic!("expected repeated local minute");
        };
        assert_eq!(
            [a.min(b), a.max(b)],
            [millis(first) as i128, millis(second) as i128]
        );
    }
}

#[test]
fn half_hour_dst_gap_and_skipped_calendar_date_are_explicit() {
    assert_eq!(
        resolve_local_minute(&chrono_tz::Australia::Lord_Howe, day("2026-10-04"), 135).unwrap(),
        LocalTimeMapping::Nonexistent
    );
    assert!(matches!(
        resolve_local_minute(&chrono_tz::Australia::Lord_Howe, day("2026-10-04"), 150).unwrap(),
        LocalTimeMapping::Single(_)
    ));
    assert_eq!(
        resolve_local_minute(&chrono_tz::Pacific::Apia, day("2011-12-30"), 0).unwrap(),
        LocalTimeMapping::Nonexistent
    );
}

#[test]
fn epoch_leap_day_and_calendar_boundaries_are_checked_without_panics() {
    let utc = FixedOffset::east_opt(0).unwrap();
    assert_eq!(
        single(&utc, "2024-02-29", 8),
        millis("2024-02-29T08:00:00Z") as i128
    );
    assert_eq!(
        single(&utc, "2027-01-01", 8),
        millis("2027-01-01T08:00:00Z") as i128
    );
    assert_eq!(
        resolve_local_minute(&utc, -1, 0).unwrap(),
        LocalTimeMapping::Single(-86_400_000)
    );
    assert!(local_day_at(&utc, u128::MAX).is_err());
    assert!(resolve_local_minute(&utc, i64::MAX, 0).is_err());
    assert!(resolve_local_minute(&utc, 0, 1440).is_err());
}
