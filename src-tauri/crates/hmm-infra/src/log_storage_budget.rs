use crate::audit_log::audit_log_day_from_file_name;
use crate::controlled_fs::ensure_regular_file_metadata;
use crate::managed_log::{
    dated_log_day_from_file_name, is_task_log_file_name, open_existing_log_directory,
    regular_file_fingerprint, remove_regular_file_if_unchanged, RegularFileFingerprint,
};
use anyhow::{Context, Result};
use hmm_ports::DiagnosticsEvidenceHealth;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

pub const DEFAULT_LOG_STORAGE_MAX_BYTES: u64 = 128 * 1024 * 1024;
pub const LOG_STORAGE_AUDIT_RESERVE_BYTES: u64 = 16 * 1024;
pub const MIN_AUDIT_LOG_RETENTION_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStorageBudgetOutcome {
    WithinBudget,
    ReducedToBudget,
    Unsatisfied,
    Failed,
}

impl LogStorageBudgetOutcome {
    pub fn code(self) -> &'static str {
        match self {
            Self::WithinBudget => "within_budget",
            Self::ReducedToBudget => "reduced_to_budget",
            Self::Unsatisfied => "unsatisfied",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogStorageBudgetReport {
    pub outcome: LogStorageBudgetOutcome,
    pub max_bytes: u64,
    pub cleanup_target_bytes: u64,
    pub owned_bytes_before: u64,
    pub owned_bytes_after: u64,
    pub deleted_file_count: u64,
    pub deleted_bytes: u64,
    pub failed_category_count: u64,
}

pub struct FileSystemLogStorageBudget {
    app_data_root: PathBuf,
    health: Arc<dyn DiagnosticsEvidenceHealth>,
    audit_reserve_bytes: u64,
}

impl FileSystemLogStorageBudget {
    pub fn new(app_data_root: PathBuf, health: Arc<dyn DiagnosticsEvidenceHealth>) -> Self {
        Self {
            app_data_root,
            health,
            audit_reserve_bytes: LOG_STORAGE_AUDIT_RESERVE_BYTES,
        }
    }

    pub fn run_at(
        &self,
        timestamp_unix_millis: u128,
        max_bytes: u64,
        maintenance_audit_required: bool,
    ) -> LogStorageBudgetReport {
        let cleanup_target_bytes = max_bytes.saturating_sub(self.audit_reserve_bytes);
        let current_day = match i64::try_from(timestamp_unix_millis / 86_400_000) {
            Ok(day) if max_bytes > self.audit_reserve_bytes => day,
            _ => return self.failed_report(max_bytes, cleanup_target_bytes),
        };
        let audit_cutoff_day = match current_day.checked_sub(MIN_AUDIT_LOG_RETENTION_DAYS - 1) {
            Some(day) => day,
            None => return self.failed_report(max_bytes, cleanup_target_bytes),
        };
        let mut candidates = Vec::new();
        let mut owned_bytes = 0_u64;
        let mut failed_categories = BTreeSet::new();

        for category in ManagedLogCategory::ALL {
            match scan_category(&self.app_data_root, category, current_day, audit_cutoff_day) {
                Ok(scan) => {
                    let Some(total) = owned_bytes.checked_add(scan.owned_bytes) else {
                        failed_categories.insert(category);
                        continue;
                    };
                    owned_bytes = total;
                    candidates.extend(scan.candidates);
                }
                Err(_) => {
                    failed_categories.insert(category);
                }
            }
        }

        let owned_bytes_before = owned_bytes;
        let reserve_for_maintenance_audit = maintenance_audit_required
            || !failed_categories.is_empty()
            || owned_bytes_before > max_bytes;
        let deletion_target_bytes = if reserve_for_maintenance_audit {
            cleanup_target_bytes
        } else {
            max_bytes
        };
        candidates.sort_by(candidate_order);
        let mut deleted_file_count = 0_u64;
        let mut deleted_bytes = 0_u64;

        if owned_bytes > deletion_target_bytes {
            for candidate in candidates {
                if owned_bytes <= deletion_target_bytes {
                    break;
                }
                if failed_categories.contains(&candidate.category) {
                    continue;
                }
                match delete_candidate(&self.app_data_root, &candidate) {
                    Ok(()) => {
                        owned_bytes = owned_bytes.saturating_sub(candidate.fingerprint.len());
                        deleted_bytes = deleted_bytes.saturating_add(candidate.fingerprint.len());
                        deleted_file_count = deleted_file_count.saturating_add(1);
                    }
                    Err(_) => {
                        failed_categories.insert(candidate.category);
                    }
                }
            }
        }

        let outcome = if !failed_categories.is_empty() {
            self.health.record_log_storage_budget_failure();
            LogStorageBudgetOutcome::Failed
        } else if owned_bytes <= deletion_target_bytes && deleted_file_count == 0 {
            LogStorageBudgetOutcome::WithinBudget
        } else if owned_bytes <= deletion_target_bytes {
            LogStorageBudgetOutcome::ReducedToBudget
        } else {
            self.health.record_log_storage_budget_unsatisfied();
            LogStorageBudgetOutcome::Unsatisfied
        };

        LogStorageBudgetReport {
            outcome,
            max_bytes,
            cleanup_target_bytes,
            owned_bytes_before,
            owned_bytes_after: owned_bytes,
            deleted_file_count,
            deleted_bytes,
            failed_category_count: failed_categories.len() as u64,
        }
    }

    fn failed_report(&self, max_bytes: u64, cleanup_target_bytes: u64) -> LogStorageBudgetReport {
        self.health.record_log_storage_budget_failure();
        LogStorageBudgetReport {
            outcome: LogStorageBudgetOutcome::Failed,
            max_bytes,
            cleanup_target_bytes,
            owned_bytes_before: 0,
            owned_bytes_after: 0,
            deleted_file_count: 0,
            deleted_bytes: 0,
            failed_category_count: ManagedLogCategory::ALL.len() as u64,
        }
    }

    #[cfg(test)]
    fn with_audit_reserve_bytes(mut self, audit_reserve_bytes: u64) -> Self {
        self.audit_reserve_bytes = audit_reserve_bytes;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ManagedLogCategory {
    Debug,
    Task,
    App,
    Audit,
}

impl ManagedLogCategory {
    const ALL: [Self; 4] = [Self::Debug, Self::Task, Self::App, Self::Audit];

    fn directory_name(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Task => "tasks",
            Self::App => "app",
            Self::Audit => "audit",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Debug => "debug log directory",
            Self::Task => "task log directory",
            Self::App => "app log directory",
            Self::Audit => "audit log directory",
        }
    }

    fn priority(self) -> u8 {
        match self {
            Self::Debug | Self::Task => 0,
            Self::App => 1,
            Self::Audit => 2,
        }
    }
}

struct CategoryScan {
    owned_bytes: u64,
    candidates: Vec<LogStorageCandidate>,
}

struct LogStorageCandidate {
    category: ManagedLogCategory,
    name: OsString,
    sort_key: SystemTime,
    fingerprint: RegularFileFingerprint,
}

fn scan_category(
    app_data_root: &Path,
    category: ManagedLogCategory,
    current_day: i64,
    audit_cutoff_day: i64,
) -> Result<CategoryScan> {
    let Some(directory) =
        open_existing_log_directory(app_data_root, category.directory_name(), category.label())?
    else {
        return Ok(CategoryScan {
            owned_bytes: 0,
            candidates: Vec::new(),
        });
    };
    let mut owned_bytes = 0_u64;
    let mut candidates = Vec::new();

    for entry in directory
        .entries()
        .with_context(|| format!("failed to read {}", category.label()))?
    {
        let entry = entry.with_context(|| format!("failed to read {} entry", category.label()))?;
        let name = entry.file_name();
        let metadata = directory
            .symlink_metadata(&name)
            .with_context(|| format!("failed to inspect {} entry", category.label()))?;
        if ensure_regular_file_metadata(&metadata, "managed log entry").is_err() {
            continue;
        }
        let Some(file_name) = name.to_str() else {
            continue;
        };
        let Some(classification) = classify_owned_log(category, file_name, &metadata)? else {
            continue;
        };
        let fingerprint = regular_file_fingerprint(&metadata, "managed log entry")?;
        owned_bytes = owned_bytes
            .checked_add(fingerprint.len())
            .context("managed log byte count overflowed")?;
        if classification.is_deletable(current_day, audit_cutoff_day) {
            candidates.push(LogStorageCandidate {
                category,
                name,
                sort_key: classification.sort_key(),
                fingerprint,
            });
        }
    }

    Ok(CategoryScan {
        owned_bytes,
        candidates,
    })
}

enum OwnedLogClassification {
    Dated { day: i64, protect_days: i64 },
    Task { modified: SystemTime },
}

impl OwnedLogClassification {
    fn is_deletable(&self, current_day: i64, audit_cutoff_day: i64) -> bool {
        match self {
            Self::Dated {
                day,
                protect_days: 1,
            } => *day < current_day,
            Self::Dated { day, .. } => *day < audit_cutoff_day,
            Self::Task { .. } => true,
        }
    }

    fn sort_key(&self) -> SystemTime {
        match self {
            Self::Dated { day, .. } => SystemTime::UNIX_EPOCH
                .checked_add(std::time::Duration::from_secs(
                    (*day as u64).saturating_mul(86_400),
                ))
                .unwrap_or(SystemTime::UNIX_EPOCH),
            Self::Task { modified } => *modified,
        }
    }
}

fn classify_owned_log(
    category: ManagedLogCategory,
    file_name: &str,
    metadata: &cap_std::fs::Metadata,
) -> Result<Option<OwnedLogClassification>> {
    let classification = match category {
        ManagedLogCategory::Debug => dated_log_day_from_file_name(file_name, "debug-").map(|day| {
            OwnedLogClassification::Dated {
                day,
                protect_days: 1,
            }
        }),
        ManagedLogCategory::Task if is_task_log_file_name(file_name) => {
            Some(OwnedLogClassification::Task {
                modified: metadata
                    .modified()
                    .context("failed to inspect task log modified time")?
                    .into_std(),
            })
        }
        ManagedLogCategory::Task => None,
        ManagedLogCategory::App => dated_log_day_from_file_name(file_name, "app-").map(|day| {
            OwnedLogClassification::Dated {
                day,
                protect_days: 1,
            }
        }),
        ManagedLogCategory::Audit => {
            audit_log_day_from_file_name(file_name).map(|day| OwnedLogClassification::Dated {
                day,
                protect_days: MIN_AUDIT_LOG_RETENTION_DAYS,
            })
        }
    };
    Ok(classification)
}

fn candidate_order(left: &LogStorageCandidate, right: &LogStorageCandidate) -> Ordering {
    left.category
        .priority()
        .cmp(&right.category.priority())
        .then_with(|| left.sort_key.cmp(&right.sort_key))
        .then_with(|| left.category.cmp(&right.category))
        .then_with(|| left.name.cmp(&right.name))
}

fn delete_candidate(app_data_root: &Path, candidate: &LogStorageCandidate) -> Result<()> {
    let directory = open_existing_log_directory(
        app_data_root,
        candidate.category.directory_name(),
        candidate.category.label(),
    )?
    .context("managed log directory disappeared before deletion")?;
    remove_regular_file_if_unchanged(
        &directory,
        &candidate.name,
        candidate.fingerprint,
        "managed log candidate",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagnosticsEvidenceHealthState;
    use std::fs::{self, File, FileTimes};
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    const CURRENT_DAY: u64 = 100;
    const CURRENT_MILLIS: u128 = CURRENT_DAY as u128 * 86_400_000;

    fn service(root: &Path) -> FileSystemLogStorageBudget {
        FileSystemLogStorageBudget::new(
            root.to_path_buf(),
            Arc::new(DiagnosticsEvidenceHealthState::default()),
        )
        .with_audit_reserve_bytes(0)
    }

    fn write_sized(path: &Path, size: usize) {
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture parent");
        fs::write(path, vec![b'x'; size]).expect("write fixture");
    }

    fn set_modified_day(path: &Path, day: u64) {
        let file = File::options()
            .write(true)
            .open(path)
            .expect("open fixture");
        file.set_times(
            FileTimes::new()
                .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(day * 86_400)),
        )
        .expect("set fixture modified time");
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
    fn deletes_oldest_debug_and_task_before_app_and_audit() {
        let temp = tempfile::tempdir().expect("temp dir");
        let debug = temp.path().join("logs/debug/debug-1970-01-01.log");
        let task = temp.path().join("logs/tasks/task-install-old.log");
        let app = temp.path().join("logs/app/app-1970-04-10.log");
        let audit = temp.path().join("logs/audit/audit-1970-03-12.log");
        for path in [&debug, &task, &app, &audit] {
            write_sized(path, 60);
        }
        set_modified_day(&task, 1);

        let report = service(temp.path()).run_at(CURRENT_MILLIS, 120, false);

        assert_eq!(report.outcome, LogStorageBudgetOutcome::ReducedToBudget);
        assert!(!debug.exists());
        assert!(!task.exists());
        assert!(app.exists());
        assert!(audit.exists());
        assert_eq!(report.deleted_file_count, 2);
        assert_eq!(report.deleted_bytes, 120);
    }

    #[test]
    fn deletes_app_only_after_debug_and_task_then_uses_old_audit_last() {
        let temp = tempfile::tempdir().expect("temp dir");
        let debug = temp.path().join("logs/debug/debug-1970-01-01.log");
        let app = temp.path().join("logs/app/app-1970-04-10.log");
        let audit = temp.path().join("logs/audit/audit-1970-03-12.log");
        for path in [&debug, &app, &audit] {
            write_sized(path, 50);
        }

        let report = service(temp.path()).run_at(CURRENT_MILLIS, 50, false);

        assert_eq!(report.outcome, LogStorageBudgetOutcome::ReducedToBudget);
        assert!(!debug.exists());
        assert!(!app.exists());
        assert!(audit.exists());
    }

    #[test]
    fn preserves_recent_audit_and_reports_unsatisfied_budget() {
        let temp = tempfile::tempdir().expect("temp dir");
        let protected = temp.path().join("logs/audit/audit-1970-03-14.log");
        write_sized(&protected, 120);
        let health = Arc::new(DiagnosticsEvidenceHealthState::default());
        let budget = FileSystemLogStorageBudget::new(temp.path().to_path_buf(), health.clone())
            .with_audit_reserve_bytes(0);

        let report = budget.run_at(CURRENT_MILLIS, 100, false);

        assert_eq!(report.outcome, LogStorageBudgetOutcome::Unsatisfied);
        assert_eq!(report.owned_bytes_after, 120);
        assert!(protected.exists());
        assert_eq!(
            health.snapshot().log_storage_status,
            "log_storage_budget_unsatisfied"
        );
        assert_eq!(health.snapshot().log_storage_unsatisfied_count, 1);
    }

    #[test]
    fn deletes_audit_older_than_hard_floor_but_preserves_boundary() {
        let temp = tempfile::tempdir().expect("temp dir");
        let expired = temp.path().join("logs/audit/audit-1970-03-12.log");
        let boundary = temp.path().join("logs/audit/audit-1970-03-13.log");
        write_sized(&expired, 80);
        write_sized(&boundary, 50);

        let report = service(temp.path()).run_at(CURRENT_MILLIS, 60, false);

        assert_eq!(report.outcome, LogStorageBudgetOutcome::ReducedToBudget);
        assert!(!expired.exists());
        assert!(boundary.exists());
    }

    #[test]
    fn protects_current_day_app_and_debug_even_when_budget_cannot_converge() {
        let temp = tempfile::tempdir().expect("temp dir");
        let app = temp.path().join("logs/app/app-1970-04-11.log");
        let debug = temp.path().join("logs/debug/debug-1970-04-11.log");
        let task = temp.path().join("logs/tasks/task-install-old.log");
        write_sized(&app, 70);
        write_sized(&debug, 70);
        write_sized(&task, 20);
        set_modified_day(&task, 1);

        let report = service(temp.path()).run_at(CURRENT_MILLIS, 100, false);

        assert_eq!(report.outcome, LogStorageBudgetOutcome::Unsatisfied);
        assert!(app.exists());
        assert!(debug.exists());
        assert!(!task.exists());
    }

    #[test]
    fn ignores_unknown_invalid_and_non_regular_entries() {
        let temp = tempfile::tempdir().expect("temp dir");
        write_sized(&temp.path().join("logs/app/notes.txt"), 200);
        write_sized(&temp.path().join("logs/app/app-1970-99-99.log"), 200);
        fs::create_dir_all(temp.path().join("logs/tasks/task-not-a-file.log"))
            .expect("create non regular fixture");

        let report = service(temp.path()).run_at(CURRENT_MILLIS, 1, false);

        assert_eq!(report.outcome, LogStorageBudgetOutcome::WithinBudget);
        assert_eq!(report.owned_bytes_before, 0);
        assert!(temp.path().join("logs/app/notes.txt").exists());
        assert!(temp.path().join("logs/app/app-1970-99-99.log").exists());
        assert!(temp.path().join("logs/tasks/task-not-a-file.log").exists());
    }

    #[test]
    fn category_failure_does_not_follow_junction_and_other_categories_still_clean() {
        let temp = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside dir");
        let sentinel = outside.path().join("task-install-outside.log");
        write_sized(&sentinel, 80);
        fs::create_dir_all(temp.path().join("logs")).expect("create logs root");
        let task_link = temp.path().join("logs").join("tasks");
        create_directory_link(&task_link, outside.path());
        let app = temp.path().join("logs/app/app-1970-04-10.log");
        write_sized(&app, 80);

        let report = service(temp.path()).run_at(CURRENT_MILLIS, 40, false);

        assert_eq!(report.outcome, LogStorageBudgetOutcome::Failed);
        assert_eq!(report.failed_category_count, 1);
        assert!(!app.exists());
        assert_eq!(fs::read(&sentinel).expect("read sentinel").len(), 80);
        remove_directory_link(&task_link);
    }

    #[test]
    fn deletes_an_oversized_single_owned_candidate() {
        let temp = tempfile::tempdir().expect("temp dir");
        let task = temp.path().join("logs/tasks/task-install-huge.log");
        write_sized(&task, 512);
        set_modified_day(&task, 1);

        let report = service(temp.path()).run_at(CURRENT_MILLIS, 100, false);

        assert_eq!(report.outcome, LogStorageBudgetOutcome::ReducedToBudget);
        assert_eq!(report.deleted_bytes, 512);
        assert!(!task.exists());
    }

    #[test]
    fn reserves_headroom_for_the_single_maintenance_audit_event() {
        let temp = tempfile::tempdir().expect("temp dir");
        let current_app = temp.path().join("logs/app/app-1970-04-11.log");
        let task = temp.path().join("logs/tasks/task-install-reserve.log");
        write_sized(&current_app, 85);
        write_sized(&task, 10);
        set_modified_day(&task, 1);
        let budget = FileSystemLogStorageBudget::new(
            temp.path().to_path_buf(),
            Arc::new(DiagnosticsEvidenceHealthState::default()),
        )
        .with_audit_reserve_bytes(10);

        let report = budget.run_at(CURRENT_MILLIS, 100, true);

        assert_eq!(report.cleanup_target_bytes, 90);
        assert_eq!(report.owned_bytes_after, 85);
        assert_eq!(report.outcome, LogStorageBudgetOutcome::ReducedToBudget);
        assert!(current_app.exists());
        assert!(!task.exists());
    }

    #[test]
    fn reports_unsatisfied_when_required_audit_headroom_is_protected() {
        let temp = tempfile::tempdir().expect("temp dir");
        let current_app = temp.path().join("logs/app/app-1970-04-11.log");
        write_sized(&current_app, 95);
        let health = Arc::new(DiagnosticsEvidenceHealthState::default());
        let budget = FileSystemLogStorageBudget::new(temp.path().to_path_buf(), health.clone())
            .with_audit_reserve_bytes(10);

        let report = budget.run_at(CURRENT_MILLIS, 100, true);

        assert_eq!(report.outcome, LogStorageBudgetOutcome::Unsatisfied);
        assert_eq!(report.owned_bytes_after, 95);
        assert!(current_app.exists());
        assert_eq!(
            health.snapshot().log_storage_status,
            "log_storage_budget_unsatisfied"
        );
    }

    #[test]
    fn directory_drift_to_a_junction_is_rejected_before_candidate_deletion() {
        let temp = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside dir");
        let logs_dir = temp.path().join("logs");
        let task_dir = logs_dir.join("tasks");
        let moved_task_dir = logs_dir.join("tasks-before-drift");
        let file_name = "task-install-drift.log";
        let candidate_path = task_dir.join(file_name);
        let sentinel = outside.path().join(file_name);
        write_sized(&candidate_path, 80);
        write_sized(&sentinel, 80);
        set_modified_day(&candidate_path, 1);
        let mut scan = scan_category(
            temp.path(),
            ManagedLogCategory::Task,
            CURRENT_DAY as i64,
            CURRENT_DAY as i64 - (MIN_AUDIT_LOG_RETENTION_DAYS - 1),
        )
        .expect("scan task category");
        let candidate = scan.candidates.pop().expect("task candidate");
        fs::rename(&task_dir, &moved_task_dir).expect("move scanned task directory");
        create_directory_link(&task_dir, outside.path());

        let error = delete_candidate(temp.path(), &candidate)
            .expect_err("drifted category must fail closed");

        assert!(error
            .to_string()
            .contains("failed to open task log directory"));
        assert!(moved_task_dir.join(file_name).exists());
        assert_eq!(fs::read(&sentinel).expect("read sentinel").len(), 80);
        remove_directory_link(&task_dir);
    }
}
