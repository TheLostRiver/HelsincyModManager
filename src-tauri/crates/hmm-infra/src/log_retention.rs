use crate::audit_log::audit_log_day_from_file_name;
use crate::controlled_fs::ensure_regular_file_metadata;
use crate::managed_log::{
    dated_log_day_from_file_name, is_task_log_file_name, open_existing_log_directory,
    open_or_create_log_directory, regular_file_fingerprint, remove_regular_file_if_unchanged,
};
use anyhow::{Context, Result};
use cap_std::fs::Dir;
use hmm_ports::DiagnosticsEvidenceHealth;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

pub const DEFAULT_TASK_LOG_RETENTION_DAYS: i64 = 30;
pub const DEFAULT_AUDIT_LOG_RETENTION_DAYS: i64 = 90;
pub const DEFAULT_DEBUG_LOG_RETENTION_DAYS: i64 = 7;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LogRetentionReport {
    pub debug_log_deleted_count: u64,
    pub task_log_deleted_count: u64,
    pub audit_log_deleted_count: u64,
}

pub struct FileSystemLogRetention {
    app_data_root: PathBuf,
    health: Arc<dyn DiagnosticsEvidenceHealth>,
}

impl FileSystemLogRetention {
    pub fn new(app_data_root: PathBuf, health: Arc<dyn DiagnosticsEvidenceHealth>) -> Self {
        Self {
            app_data_root,
            health,
        }
    }

    pub fn run_at(&self, timestamp_unix_millis: u128) -> LogRetentionReport {
        let current_day = match days_since_epoch(timestamp_unix_millis) {
            Ok(day) => day,
            Err(_) => {
                self.health.record_debug_log_retention_failure();
                self.health.record_task_log_retention_failure();
                self.health.record_audit_log_retention_failure();
                return LogRetentionReport::default();
            }
        };
        let mut report = LogRetentionReport::default();

        match self.prune_debug_logs(current_day) {
            Ok(count) => report.debug_log_deleted_count = count,
            Err(_) => self.health.record_debug_log_retention_failure(),
        }

        match self.prune_task_logs(current_day) {
            Ok(count) => report.task_log_deleted_count = count,
            Err(_) => self.health.record_task_log_retention_failure(),
        }
        match self.prune_audit_logs(current_day) {
            Ok(count) => report.audit_log_deleted_count = count,
            Err(_) => self.health.record_audit_log_retention_failure(),
        }

        report
    }

    fn prune_debug_logs(&self, current_day: i64) -> Result<u64> {
        let cutoff_day = retention_cutoff_day(current_day, DEFAULT_DEBUG_LOG_RETENTION_DAYS)?;
        let Some(directory) =
            open_existing_log_directory(&self.app_data_root, "debug", "debug log directory")?
        else {
            return Ok(0);
        };
        prune_owned_files(&directory, |file_name, _metadata| {
            Ok(dated_log_day_from_file_name(file_name, "debug-")
                .is_some_and(|day| day < cutoff_day))
        })
    }

    fn prune_task_logs(&self, current_day: i64) -> Result<u64> {
        let cutoff_day = retention_cutoff_day(current_day, DEFAULT_TASK_LOG_RETENTION_DAYS)?;
        let cutoff_time = system_time_for_day(cutoff_day)?;
        let directory =
            open_or_create_log_directory(&self.app_data_root, "tasks", "task log directory")?;
        prune_owned_files(&directory, |file_name, metadata| {
            if !is_task_log_file_name(file_name) {
                return Ok(false);
            }
            Ok(metadata
                .modified()
                .context("failed to inspect task log modified time")?
                .into_std()
                < cutoff_time)
        })
    }

    fn prune_audit_logs(&self, current_day: i64) -> Result<u64> {
        let cutoff_day = retention_cutoff_day(current_day, DEFAULT_AUDIT_LOG_RETENTION_DAYS)?;
        let directory =
            open_or_create_log_directory(&self.app_data_root, "audit", "audit log directory")?;
        prune_owned_files(&directory, |file_name, _metadata| {
            Ok(audit_log_day_from_file_name(file_name).is_some_and(|day| day < cutoff_day))
        })
    }
}

fn prune_owned_files(
    directory: &Dir,
    should_delete: impl Fn(&str, &cap_std::fs::Metadata) -> Result<bool>,
) -> Result<u64> {
    let mut deleted_count = 0_u64;
    for entry in directory
        .entries()
        .context("failed to read managed log directory")?
    {
        let entry = entry.context("failed to read managed log directory entry")?;
        let name = entry.file_name();
        let Some(file_name) = name.to_str() else {
            continue;
        };
        let metadata = directory
            .symlink_metadata(&name)
            .context("failed to inspect managed log entry")?;
        if ensure_regular_file_metadata(&metadata, "managed log entry").is_err()
            || !should_delete(file_name, &metadata)?
        {
            continue;
        }

        let fingerprint = regular_file_fingerprint(&metadata, "expired managed log")?;
        remove_regular_file_if_unchanged(directory, &name, fingerprint, "expired managed log")?;
        deleted_count = deleted_count.saturating_add(1);
    }
    Ok(deleted_count)
}

fn days_since_epoch(timestamp_unix_millis: u128) -> Result<i64> {
    i64::try_from(timestamp_unix_millis / 86_400_000)
        .context("log retention timestamp is out of range")
}

fn retention_cutoff_day(current_day: i64, retention_days: i64) -> Result<i64> {
    if retention_days <= 0 {
        anyhow::bail!("log retention must be positive");
    }
    current_day
        .checked_sub(retention_days - 1)
        .context("log retention cutoff is out of range")
}

fn system_time_for_day(day: i64) -> Result<SystemTime> {
    let day = u64::try_from(day).context("task log retention predates Unix epoch")?;
    let seconds = day
        .checked_mul(86_400)
        .context("task log retention cutoff is out of range")?;
    SystemTime::UNIX_EPOCH
        .checked_add(Duration::from_secs(seconds))
        .context("task log retention cutoff is out of range")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagnosticsEvidenceHealthState;
    use hmm_ports::DiagnosticsEvidenceHealth;
    use std::fs::{self, File, FileTimes};
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    const MILLIS_PER_DAY: u128 = 86_400_000;

    fn set_modified_day(path: &Path, day: u64) {
        let file = File::options()
            .write(true)
            .open(path)
            .expect("open task log fixture");
        file.set_times(
            FileTimes::new()
                .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(day * 86_400)),
        )
        .expect("set task log modified time");
    }

    #[cfg(unix)]
    fn create_directory_link(link: &Path, target: &Path) {
        std::os::unix::fs::symlink(target, link).expect("create directory symlink");
    }

    #[cfg(windows)]
    fn create_directory_link(link: &Path, target: &Path) {
        let output = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link.to_str().expect("junction path"),
                target.to_str().expect("junction target"),
            ])
            .output()
            .expect("create directory junction");
        assert!(
            output.status.success(),
            "mklink failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    fn remove_directory_link(link: &Path) {
        fs::remove_file(link).expect("remove directory symlink");
    }

    #[cfg(windows)]
    fn remove_directory_link(link: &Path) {
        fs::remove_dir(link).expect("remove directory junction");
    }

    #[test]
    fn prunes_only_expired_owned_task_and_audit_logs() {
        let temp = tempfile::tempdir().expect("temp dir");
        let task_dir = temp.path().join("logs").join("tasks");
        let audit_dir = temp.path().join("logs").join("audit");
        fs::create_dir_all(&task_dir).expect("create task log dir");
        fs::create_dir_all(&audit_dir).expect("create audit log dir");

        let expired_task = task_dir.join("task-install-old.log");
        let retained_task = task_dir.join("task-install-boundary.log");
        let unknown_task = task_dir.join("notes.txt");
        fs::write(&expired_task, "old\n").expect("write expired task log");
        fs::write(&retained_task, "boundary\n").expect("write retained task log");
        fs::write(&unknown_task, "unknown\n").expect("write unknown task file");
        set_modified_day(&expired_task, 170);
        set_modified_day(&retained_task, 171);

        let expired_audit = audit_dir.join("audit-1970-04-21.log");
        let retained_audit = audit_dir.join("audit-1970-04-22.log");
        let invalid_audit = audit_dir.join("audit-1970-99-99.log");
        fs::write(&expired_audit, "old\n").expect("write expired audit log");
        fs::write(&retained_audit, "boundary\n").expect("write retained audit log");
        fs::write(&invalid_audit, "invalid\n").expect("write invalid audit file");

        let health = Arc::new(DiagnosticsEvidenceHealthState::default());
        let retention = FileSystemLogRetention::new(temp.path().to_path_buf(), health.clone());
        let report = retention.run_at(200 * MILLIS_PER_DAY);

        assert_eq!(report.task_log_deleted_count, 1);
        assert_eq!(report.audit_log_deleted_count, 1);
        assert!(!expired_task.exists());
        assert!(retained_task.exists());
        assert!(unknown_task.exists());
        assert!(!expired_audit.exists());
        assert!(retained_audit.exists());
        assert!(invalid_audit.exists());
        assert_eq!(health.snapshot().task_log_status, "ok");
        assert_eq!(health.snapshot().audit_log_status, "ok");
    }

    #[test]
    fn prunes_debug_logs_older_than_seven_utc_days_only() {
        let temp = tempfile::tempdir().expect("temp dir");
        let debug_dir = temp.path().join("logs").join("debug");
        fs::create_dir_all(&debug_dir).expect("create debug log dir");
        let expired = debug_dir.join("debug-1970-07-13.log");
        let boundary = debug_dir.join("debug-1970-07-14.log");
        let current = debug_dir.join("debug-1970-07-20.log");
        let invalid = debug_dir.join("debug-1970-99-99.log");
        let unknown = debug_dir.join("notes.log");
        for path in [&expired, &boundary, &current, &invalid, &unknown] {
            fs::write(path, "fixture\n").expect("write debug fixture");
        }

        let health = Arc::new(DiagnosticsEvidenceHealthState::default());
        let report = FileSystemLogRetention::new(temp.path().to_path_buf(), health.clone())
            .run_at(200 * MILLIS_PER_DAY);

        assert_eq!(report.debug_log_deleted_count, 1);
        assert!(!expired.exists());
        assert!(boundary.exists());
        assert!(current.exists());
        assert!(invalid.exists());
        assert!(unknown.exists());
        assert_eq!(health.snapshot().debug_log_status, "ok");
    }

    #[test]
    fn linked_task_directory_fails_closed_without_touching_outside_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside dir");
        let sentinel = outside.path().join("task-install-outside.log");
        fs::write(&sentinel, "outside\n").expect("write outside sentinel");
        fs::create_dir_all(temp.path().join("logs")).expect("create logs dir");
        let audit_dir = temp.path().join("logs").join("audit");
        fs::create_dir_all(&audit_dir).expect("create audit dir");
        let expired_audit = audit_dir.join("audit-1970-01-01.log");
        fs::write(&expired_audit, "expired\n").expect("write expired audit log");
        let task_link = temp.path().join("logs").join("tasks");
        create_directory_link(&task_link, outside.path());

        let health = Arc::new(DiagnosticsEvidenceHealthState::default());
        let retention = FileSystemLogRetention::new(temp.path().to_path_buf(), health.clone());
        let report = retention.run_at(200 * MILLIS_PER_DAY);

        assert_eq!(report.task_log_deleted_count, 0);
        assert_eq!(report.audit_log_deleted_count, 1);
        assert_eq!(fs::read_to_string(&sentinel).unwrap(), "outside\n");
        assert!(!expired_audit.exists());
        assert_eq!(
            health.snapshot().task_log_status,
            "task_log_retention_failed"
        );
        remove_directory_link(&task_link);
    }

    #[test]
    fn matching_link_entry_is_preserved_without_following_its_target() {
        let temp = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside dir");
        let sentinel = outside.path().join("sentinel.txt");
        fs::write(&sentinel, "outside\n").expect("write outside sentinel");
        let task_dir = temp.path().join("logs").join("tasks");
        fs::create_dir_all(&task_dir).expect("create task log dir");
        let linked_entry = task_dir.join("task-install-linked.log");
        create_directory_link(&linked_entry, outside.path());

        let health = Arc::new(DiagnosticsEvidenceHealthState::default());
        let retention = FileSystemLogRetention::new(temp.path().to_path_buf(), health.clone());
        let report = retention.run_at(200 * MILLIS_PER_DAY);

        assert_eq!(report.task_log_deleted_count, 0);
        assert_eq!(fs::read_to_string(&sentinel).unwrap(), "outside\n");
        assert_eq!(health.snapshot().task_log_status, "ok");
        remove_directory_link(&linked_entry);
    }

    #[test]
    fn linked_debug_directory_fails_closed_while_audit_cleanup_continues() {
        let temp = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside dir");
        let sentinel = outside.path().join("debug-1970-01-01.log");
        fs::write(&sentinel, "outside\n").expect("write outside sentinel");
        let logs_dir = temp.path().join("logs");
        fs::create_dir_all(&logs_dir).expect("create logs root");
        let debug_link = logs_dir.join("debug");
        create_directory_link(&debug_link, outside.path());
        let audit_dir = logs_dir.join("audit");
        fs::create_dir_all(&audit_dir).expect("create audit dir");
        let expired_audit = audit_dir.join("audit-1970-01-01.log");
        fs::write(&expired_audit, "expired\n").expect("write expired audit log");

        let health = Arc::new(DiagnosticsEvidenceHealthState::default());
        let retention = FileSystemLogRetention::new(temp.path().to_path_buf(), health.clone());
        let report = retention.run_at(200 * MILLIS_PER_DAY);

        assert_eq!(report.debug_log_deleted_count, 0);
        assert_eq!(report.audit_log_deleted_count, 1);
        assert_eq!(
            fs::read_to_string(&sentinel).expect("read outside sentinel"),
            "outside\n"
        );
        assert!(!expired_audit.exists());
        assert_eq!(
            health.snapshot().debug_log_status,
            "debug_log_retention_failed"
        );
        assert_eq!(health.snapshot().audit_log_status, "ok");
        remove_directory_link(&debug_link);
    }
}
