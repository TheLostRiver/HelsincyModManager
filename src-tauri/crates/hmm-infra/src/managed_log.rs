use crate::controlled_fs::{
    ensure_regular_file_metadata, is_not_found, open_existing_directory_chain,
    open_existing_directory_nofollow, open_or_create_directory_chain,
    open_or_create_directory_nofollow, open_regular_file_nofollow,
};
use anyhow::{Context, Result};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt as _};
use cap_std::fs::Metadata;
#[cfg(any(unix, windows))]
use cap_std::fs::MetadataExt as _;
use cap_std::fs::{Dir, File, OpenOptions};
use std::ffi::OsStr;
use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RegularFileFingerprint {
    len: u64,
    modified: SystemTime,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    creation_time: u64,
    #[cfg(windows)]
    last_write_time: u64,
    #[cfg(windows)]
    file_attributes: u32,
}

pub(crate) fn open_or_create_log_directory(
    app_data_root: &Path,
    category: &str,
    label: &str,
) -> Result<Dir> {
    let app_data = open_or_create_directory_nofollow(app_data_root, "app data directory")?;
    open_or_create_directory_chain(&app_data, &["logs", category], label)
}

pub(crate) fn open_existing_log_directory(
    app_data_root: &Path,
    category: &str,
    label: &str,
) -> Result<Option<Dir>> {
    let app_data = match open_existing_directory_nofollow(app_data_root, "app data directory") {
        Ok(directory) => directory,
        Err(error) if is_not_found(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
    match open_existing_directory_chain(&app_data, &["logs", category], label) {
        Ok(directory) => Ok(Some(directory)),
        Err(error) if is_not_found(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

pub(crate) fn open_append_regular_file(directory: &Dir, name: &OsStr, label: &str) -> Result<File> {
    match directory.symlink_metadata(name) {
        Ok(metadata) => ensure_regular_file_metadata(&metadata, label)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("failed to inspect {label}")),
    }

    let mut options = OpenOptions::new();
    options.append(true).create(true).follow(FollowSymlinks::No);
    let file = directory
        .open_with(name, &options)
        .with_context(|| format!("failed to open {label}"))?;
    ensure_regular_file_metadata(
        &file
            .metadata()
            .with_context(|| format!("failed to inspect opened {label}"))?,
        label,
    )?;
    Ok(file)
}

pub(crate) fn open_read_log_file(directory: &Dir, name: &OsStr, label: &str) -> Result<File> {
    open_regular_file_nofollow(directory, name, label)
}

pub(crate) fn regular_file_fingerprint(
    metadata: &Metadata,
    label: &str,
) -> Result<RegularFileFingerprint> {
    ensure_regular_file_metadata(metadata, label)?;
    Ok(RegularFileFingerprint {
        len: metadata.len(),
        modified: metadata
            .modified()
            .with_context(|| format!("failed to inspect {label} modified time"))?
            .into_std(),
        #[cfg(unix)]
        device: metadata.dev(),
        #[cfg(unix)]
        inode: metadata.ino(),
        #[cfg(windows)]
        creation_time: metadata.creation_time(),
        #[cfg(windows)]
        last_write_time: metadata.last_write_time(),
        #[cfg(windows)]
        file_attributes: metadata.file_attributes(),
    })
}

impl RegularFileFingerprint {
    pub(crate) fn len(self) -> u64 {
        self.len
    }
}

pub(crate) fn remove_regular_file_if_unchanged(
    directory: &Dir,
    name: &OsStr,
    expected: RegularFileFingerprint,
    label: &str,
) -> Result<()> {
    let before_open = directory
        .symlink_metadata(name)
        .with_context(|| format!("failed to inspect {label}"))?;
    if regular_file_fingerprint(&before_open, label)? != expected {
        anyhow::bail!("{label} changed before open");
    }

    let opened = open_regular_file_nofollow(directory, name, label)?;
    let opened_metadata = opened
        .metadata()
        .with_context(|| format!("failed to inspect opened {label}"))?;
    if regular_file_fingerprint(&opened_metadata, label)? != expected {
        anyhow::bail!("{label} changed while opening");
    }
    drop(opened);

    let before_remove = directory
        .symlink_metadata(name)
        .with_context(|| format!("failed to revalidate {label}"))?;
    if regular_file_fingerprint(&before_remove, label)? != expected {
        anyhow::bail!("{label} changed before deletion");
    }
    directory
        .remove_file(name)
        .with_context(|| format!("failed to remove {label}"))
}

pub(crate) fn is_task_log_file_name(file_name: &str) -> bool {
    let Some(task_id) = file_name
        .strip_prefix("task-")
        .and_then(|value| value.strip_suffix(".log"))
    else {
        return false;
    };
    !task_id.is_empty()
        && task_id.len() <= 160
        && task_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

pub(crate) fn dated_log_day_from_file_name(file_name: &str, prefix: &str) -> Option<i64> {
    let expected_len = prefix.len() + "1970-01-01.log".len();
    let bytes = file_name.as_bytes();
    if bytes.len() != expected_len || !file_name.starts_with(prefix) || !file_name.ends_with(".log")
    {
        return None;
    }
    let date_start = prefix.len();
    let year_end = date_start + 4;
    let month_start = year_end + 1;
    let month_end = month_start + 2;
    let day_start = month_end + 1;
    let day_end = day_start + 2;
    if !bytes[date_start..year_end].iter().all(u8::is_ascii_digit)
        || bytes[year_end] != b'-'
        || !bytes[month_start..month_end].iter().all(u8::is_ascii_digit)
        || bytes[month_end] != b'-'
        || !bytes[day_start..day_end].iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    let year = file_name[date_start..year_end].parse::<i32>().ok()?;
    let month = file_name[month_start..month_end].parse::<u32>().ok()?;
    let day = file_name[day_start..day_end].parse::<u32>().ok()?;
    if year < 1970 || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let days_since_epoch = days_from_civil(year, month, day);
    (civil_from_days(days_since_epoch) == (year, month, day)).then_some(days_since_epoch)
}

pub(crate) fn dated_log_file_name(prefix: &str, days_since_epoch: i64) -> Result<String> {
    let (year, month, day) = civil_from_days(days_since_epoch);
    if !(0..=9999).contains(&year) {
        anyhow::bail!("managed log date is out of supported range");
    }
    Ok(format!("{prefix}{year:04}-{month:02}-{day:02}.log"))
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let days = days_since_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (year as i32, month as u32, day as u32)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = i64::from(year) - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn strict_dated_log_parser_rejects_invalid_calendar_dates() {
        assert_eq!(
            dated_log_day_from_file_name("app-1970-01-01.log", "app-"),
            Some(0)
        );
        assert_eq!(
            dated_log_day_from_file_name("debug-2024-02-29.log", "debug-"),
            Some(19_782)
        );
        for invalid in [
            "app-1969-12-31.log",
            "app-2023-02-29.log",
            "app-2024-13-01.log",
            "app-2024-1-01.log",
            "notes.log",
        ] {
            assert_eq!(
                dated_log_day_from_file_name(invalid, "app-"),
                None,
                "{invalid}"
            );
        }
    }

    #[test]
    fn replacement_between_scan_and_delete_is_rejected() {
        let temp = tempfile::tempdir().expect("temp dir");
        let directory = Dir::open_ambient_dir(temp.path(), cap_std::ambient_authority())
            .expect("open temp directory");
        let name = OsStr::new("task-install-race.log");
        fs::write(temp.path().join(name), "first").expect("write original");
        let metadata = directory.symlink_metadata(name).expect("inspect original");
        let fingerprint = regular_file_fingerprint(&metadata, "race fixture").expect("fingerprint");
        fs::remove_file(temp.path().join(name)).expect("remove original");
        fs::write(temp.path().join(name), "replacement").expect("write replacement");

        let error = remove_regular_file_if_unchanged(&directory, name, fingerprint, "race fixture")
            .expect_err("replacement rejected");

        assert!(error.to_string().contains("changed"));
        assert_eq!(
            fs::read_to_string(temp.path().join(name)).unwrap(),
            "replacement"
        );
    }
}
