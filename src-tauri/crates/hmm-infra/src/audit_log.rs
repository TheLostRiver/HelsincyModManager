use crate::controlled_fs::ensure_regular_file_metadata;
use crate::managed_log::{
    dated_log_day_from_file_name, open_append_regular_file, open_existing_log_directory,
    open_or_create_log_directory, open_read_log_file,
};
use anyhow::{Context, Result};
use cap_std::fs::Dir;
use hmm_ports::{
    AuditLogEvent, AuditLogReadRequest, AuditLogReader, AuditLogWriter, AuditWriteFailurePolicy,
    DiagnosticsEvidenceHealth,
};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const AUDIT_LOG_DIR: &str = "audit";
const MILLIS_PER_DAY: u128 = 86_400_000;

pub struct FileSystemAuditLogWriter {
    app_data_root: PathBuf,
    write_lock: Mutex<()>,
    audit_dir: Mutex<Option<Dir>>,
    health: Option<Arc<dyn DiagnosticsEvidenceHealth>>,
}

pub struct FileSystemAuditLogReader {
    app_data_root: PathBuf,
}

impl FileSystemAuditLogReader {
    pub fn new(app_data_root: PathBuf) -> Self {
        Self { app_data_root }
    }
}

impl FileSystemAuditLogWriter {
    pub fn new(app_data_root: PathBuf) -> Self {
        Self {
            app_data_root,
            write_lock: Mutex::new(()),
            audit_dir: Mutex::new(None),
            health: None,
        }
    }

    pub fn with_health(app_data_root: PathBuf, health: Arc<dyn DiagnosticsEvidenceHealth>) -> Self {
        Self {
            app_data_root,
            write_lock: Mutex::new(()),
            audit_dir: Mutex::new(None),
            health: Some(health),
        }
    }

    fn audit_log_directory(&self) -> Result<Dir> {
        let mut directory = self
            .audit_dir
            .lock()
            .map_err(|_| anyhow::anyhow!("audit log directory state unavailable"))?;
        if directory.is_none() {
            *directory = Some(open_or_create_log_directory(
                &self.app_data_root,
                AUDIT_LOG_DIR,
                "audit log directory",
            )?);
        }
        directory
            .as_ref()
            .context("audit log directory unavailable")?
            .try_clone()
            .context("failed to clone audit log directory handle")
    }
}

impl AuditLogWriter for FileSystemAuditLogWriter {
    fn record(&self, event: AuditLogEvent) -> Result<()> {
        let after_commit = event.result == "success";
        self.record_observed(event, after_commit)
    }

    fn record_with_policy(
        &self,
        event: AuditLogEvent,
        policy: AuditWriteFailurePolicy,
    ) -> Result<()> {
        self.record_observed(event, policy == AuditWriteFailurePolicy::ReportAfterCommit)
    }
}

impl FileSystemAuditLogWriter {
    fn record_observed(&self, event: AuditLogEvent, after_commit: bool) -> Result<()> {
        let result = self.record_inner(event);
        if result.is_err() {
            if let Some(health) = &self.health {
                health.record_audit_write_failure(after_commit);
            }
        }
        result
    }

    fn record_inner(&self, event: AuditLogEvent) -> Result<()> {
        validate_audit_event(&event)?;

        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("audit log write lock poisoned"))?;
        let audit_dir = self.audit_log_directory()?;
        let file_name = audit_log_file_name(event.timestamp_unix_millis)?;
        let mut file = open_append_regular_file(&audit_dir, file_name.as_ref(), "audit log")?;
        let serialized =
            serde_json::to_string(&event).context("failed to serialize audit log event")?;
        file.write_all(serialized.as_bytes())
            .context("failed to write audit log event")?;
        file.write_all(b"\n")
            .context("failed to write audit log event")?;
        file.sync_all().context("failed to sync audit log")?;
        sync_directory(&audit_dir).context("failed to sync audit log directory")?;

        Ok(())
    }
}

impl AuditLogReader for FileSystemAuditLogWriter {
    fn read_recent_sanitized(&self, request: AuditLogReadRequest) -> Result<Vec<AuditLogEvent>> {
        read_recent_sanitized(&self.app_data_root, request)
    }
}

impl AuditLogReader for FileSystemAuditLogReader {
    fn read_recent_sanitized(&self, request: AuditLogReadRequest) -> Result<Vec<AuditLogEvent>> {
        read_recent_sanitized(&self.app_data_root, request)
    }
}

fn read_recent_sanitized(
    app_data_root: &Path,
    request: AuditLogReadRequest,
) -> Result<Vec<AuditLogEvent>> {
    if request.max_events == 0 {
        return Ok(Vec::new());
    }

    let Some(audit_dir) =
        open_existing_log_directory(app_data_root, AUDIT_LOG_DIR, "audit log directory")?
    else {
        return Ok(Vec::new());
    };
    let mut audit_files = Vec::new();
    for entry in audit_dir
        .entries()
        .context("failed to read audit log directory")?
    {
        let entry = entry.context("failed to read audit log directory entry")?;
        let file_name = entry.file_name();
        let metadata = audit_dir
            .symlink_metadata(&file_name)
            .context("failed to inspect audit log entry")?;
        if ensure_regular_file_metadata(&metadata, "audit log entry").is_err() {
            continue;
        }
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        if is_audit_log_file_name(file_name) {
            audit_files.push((file_name.to_owned(), entry.file_name()));
        }
    }
    audit_files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut events = VecDeque::new();
    for (_, file_name) in audit_files {
        let file = open_read_log_file(&audit_dir, &file_name, "audit log")?;
        for line in BufReader::new(file).lines() {
            let line = line.context("failed to read audit log event")?;
            if line.trim().is_empty() {
                continue;
            }

            let Ok(event) = serde_json::from_str::<AuditLogEvent>(&line) else {
                continue;
            };
            if validate_audit_event(&event).is_err() {
                continue;
            }
            if events.len() == request.max_events {
                events.pop_front();
            }
            events.push_back(event);
        }
    }

    Ok(events.into_iter().collect())
}

fn is_audit_log_file_name(file_name: &str) -> bool {
    audit_log_day_from_file_name(file_name).is_some()
}

pub(crate) fn audit_log_day_from_file_name(file_name: &str) -> Option<i64> {
    dated_log_day_from_file_name(file_name, "audit-")
}

fn validate_audit_event(event: &AuditLogEvent) -> Result<()> {
    validate_audit_value("category", &event.category)?;
    validate_audit_value("operation", &event.operation)?;
    validate_audit_value("result", &event.result)?;

    for (key, value) in &event.fields {
        validate_audit_key(key)?;
        validate_audit_value(key, value)?;
    }

    Ok(())
}

fn validate_audit_key(key: &str) -> Result<()> {
    if key.is_empty()
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        anyhow::bail!("audit log event contains invalid field key");
    }

    const FORBIDDEN_KEYS: &[&str] = &[
        "raw_path",
        "raw_steam_id",
        "raw_token",
        "raw_cookie",
        "raw_save_content",
        "raw_mod_content",
        "token",
        "cookie",
        "api_key",
    ];
    if FORBIDDEN_KEYS.contains(&key) {
        anyhow::bail!("audit log event contains forbidden sensitive field");
    }

    Ok(())
}

fn validate_audit_value(_label: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.chars().any(char::is_control)
        || value.contains('/')
        || value.contains('\\')
    {
        anyhow::bail!("audit log event contains invalid field value");
    }

    let lower = value.to_ascii_lowercase();
    const FORBIDDEN_SNIPPETS: &[&str] = &[
        "thumbnail://",
        "thumbnailurl",
        "contenthash",
        "raw_path",
        "raw_mod_content",
        "raw_save_content",
        "token",
        "cookie",
        "api_key",
        "sandbox",
        "c:/",
        "c:\\",
        "\\users\\",
        "/users/",
    ];

    if FORBIDDEN_SNIPPETS
        .iter()
        .any(|snippet| lower.contains(snippet))
    {
        anyhow::bail!("audit log event contains forbidden sensitive field");
    }

    Ok(())
}

fn audit_log_file_name(timestamp_unix_millis: u128) -> Result<String> {
    let days_since_epoch = timestamp_unix_millis / MILLIS_PER_DAY;
    let days_since_epoch =
        i64::try_from(days_since_epoch).context("audit log timestamp is out of supported range")?;
    let (year, month, day) = civil_from_days(days_since_epoch);

    Ok(format!("audit-{year:04}-{month:02}-{day:02}.log"))
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

#[cfg(windows)]
fn sync_directory(directory: &Dir) -> std::io::Result<()> {
    match directory
        .try_clone()
        .and_then(|directory| directory.into_std_file().sync_all())
    {
        Ok(()) => Ok(()),
        Err(error) if is_windows_directory_sync_capability_error(&error) => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(not(windows))]
fn sync_directory(directory: &Dir) -> std::io::Result<()> {
    directory
        .try_clone()
        .and_then(|directory| directory.into_std_file().sync_all())
}

#[cfg(windows)]
fn is_windows_directory_sync_capability_error(error: &std::io::Error) -> bool {
    // Windows-backed mapped folders can persist the file but reject directory handles or flushes.
    const ERROR_INVALID_FUNCTION: i32 = 1;
    const ERROR_ACCESS_DENIED: i32 = 5;
    const ERROR_INVALID_HANDLE: i32 = 6;
    const ERROR_NOT_SUPPORTED: i32 = 50;
    const ERROR_INVALID_PARAMETER: i32 = 87;

    matches!(
        error.raw_os_error(),
        Some(
            ERROR_INVALID_FUNCTION
                | ERROR_ACCESS_DENIED
                | ERROR_INVALID_HANDLE
                | ERROR_NOT_SUPPORTED
                | ERROR_INVALID_PARAMETER
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagnosticsEvidenceHealthState;
    use hmm_ports::{AuditLogReadRequest, AuditLogReader, DiagnosticsEvidenceHealth};
    use std::collections::BTreeMap;
    use std::fs::{self, OpenOptions};

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
        assert!(output.status.success(), "mklink failed");
    }

    #[cfg(unix)]
    fn remove_directory_link(link: &Path) {
        fs::remove_file(link).expect("remove directory symlink");
    }

    #[cfg(windows)]
    fn remove_directory_link(link: &Path) {
        fs::remove_dir(link).expect("remove directory junction");
    }

    #[cfg(windows)]
    #[test]
    fn mapped_folder_directory_sync_capability_errors_are_non_fatal() {
        for raw_os_error in [1, 5, 6, 50, 87] {
            assert!(is_windows_directory_sync_capability_error(
                &std::io::Error::from_raw_os_error(raw_os_error)
            ));
        }
        assert!(!is_windows_directory_sync_capability_error(
            &std::io::Error::from_raw_os_error(112)
        ));
    }

    #[test]
    fn explicit_post_commit_policy_reports_stable_health_without_retrying_player_writes() {
        let temp = tempfile::tempdir().expect("temp dir");
        let health = Arc::new(DiagnosticsEvidenceHealthState::default());
        let writer =
            FileSystemAuditLogWriter::with_health(temp.path().to_path_buf(), health.clone());
        let event = AuditLogEvent {
            timestamp_unix_millis: 42,
            category: "install".to_owned(),
            operation: "install_mod".to_owned(),
            result: "success".to_owned(),
            fields: BTreeMap::from([("raw_path".to_owned(), "forbidden".to_owned())]),
        };

        assert!(writer
            .record_with_policy(event, AuditWriteFailurePolicy::ReportAfterCommit)
            .is_err());
        let snapshot = health.snapshot();
        assert_eq!(snapshot.audit_log_status, "audit_write_failed_after_commit");
        assert_eq!(snapshot.audit_write_failure_count, 1);
        assert_eq!(snapshot.audit_write_failure_after_commit_count, 1);
        assert!(!temp.path().join("logs/audit").exists());
    }

    #[test]
    fn linked_audit_directory_is_rejected_for_writes_and_reads() {
        let temp = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside dir");
        fs::create_dir_all(temp.path().join("logs")).expect("create logs dir");
        let audit_link = temp.path().join("logs").join(AUDIT_LOG_DIR);
        create_directory_link(&audit_link, outside.path());
        let outside_log = outside.path().join("audit-1970-01-01.log");
        fs::write(&outside_log, "outside\n").expect("write outside sentinel");
        let health = Arc::new(DiagnosticsEvidenceHealthState::default());
        let writer =
            FileSystemAuditLogWriter::with_health(temp.path().to_path_buf(), health.clone());
        let event = AuditLogEvent {
            timestamp_unix_millis: 42,
            category: "install".to_owned(),
            operation: "install_mod".to_owned(),
            result: "success".to_owned(),
            fields: BTreeMap::new(),
        };

        assert!(writer.record(event).is_err());
        assert_eq!(
            health.snapshot().audit_log_status,
            "audit_write_failed_after_commit"
        );
        assert!(writer
            .read_recent_sanitized(AuditLogReadRequest { max_events: 10 })
            .is_err());
        assert_eq!(fs::read_to_string(&outside_log).unwrap(), "outside\n");
        remove_directory_link(&audit_link);
    }

    #[test]
    fn audit_log_writer_appends_jsonl_inside_app_data_without_returning_path() {
        let temp = tempfile::tempdir().expect("temp dir");
        let writer = FileSystemAuditLogWriter::new(temp.path().to_path_buf());
        let event = AuditLogEvent {
            timestamp_unix_millis: 42,
            category: "diagnostic_export".to_owned(),
            operation: "export_preview_image_diagnostics".to_owned(),
            result: "success".to_owned(),
            fields: BTreeMap::from([
                (
                    "export_id".to_owned(),
                    "preview-image-diagnostics-42.zip".to_owned(),
                ),
                (
                    "file_name".to_owned(),
                    "preview-image-diagnostics-42.zip".to_owned(),
                ),
                ("size_bytes".to_owned(), "4096".to_owned()),
            ]),
        };

        writer.record(event).expect("record audit event");

        let audit_path = temp
            .path()
            .join("logs")
            .join("audit")
            .join("audit-1970-01-01.log");
        let content = fs::read_to_string(audit_path).expect("audit log content");
        let value: serde_json::Value =
            serde_json::from_str(content.trim()).expect("audit json line");
        assert_eq!(value["category"], "diagnostic_export");
        assert_eq!(value["operation"], "export_preview_image_diagnostics");
        assert_eq!(value["result"], "success");
        assert_eq!(
            value["fields"]["fileName"],
            serde_json::Value::Null,
            "fields must keep snake_case keys"
        );
        assert_eq!(
            value["fields"]["file_name"],
            "preview-image-diagnostics-42.zip"
        );
        assert!(!content.contains(temp.path().to_string_lossy().as_ref()));
        assert!(!content.contains("thumbnail://"));
        assert!(!content.contains("contentHash"));
        assert!(!content.contains("sandbox"));
        assert!(!content.contains("C:/"));
    }

    #[test]
    fn audit_log_writer_rejects_forbidden_sensitive_field_names() {
        let temp = tempfile::tempdir().expect("temp dir");
        let writer = FileSystemAuditLogWriter::new(temp.path().to_path_buf());
        let event = AuditLogEvent {
            timestamp_unix_millis: 42,
            category: "diagnostic_export".to_owned(),
            operation: "export_preview_image_diagnostics".to_owned(),
            result: "success".to_owned(),
            fields: BTreeMap::from([("raw_path".to_owned(), "mod.zip".to_owned())]),
        };

        let error = writer.record(event).expect_err("sensitive field rejected");

        assert!(!error.to_string().contains("mod.zip"));
    }

    #[test]
    fn audit_log_file_name_uses_utc_calendar_date() {
        assert_eq!(
            audit_log_file_name(42).expect("epoch day"),
            "audit-1970-01-01.log"
        );
        assert_eq!(
            audit_log_file_name(86_400_000).expect("next day"),
            "audit-1970-01-02.log"
        );
        assert_eq!(
            audit_log_file_name(1_704_067_200_000).expect("2024 leap year date"),
            "audit-2024-01-01.log"
        );
    }

    #[test]
    fn audit_log_file_name_parser_rejects_dates_the_writer_cannot_own() {
        assert_eq!(
            audit_log_day_from_file_name("audit-1970-01-01.log"),
            Some(0)
        );
        assert_eq!(
            audit_log_day_from_file_name("audit-2024-02-29.log"),
            Some(19_782)
        );
        for invalid in [
            "audit-1969-12-31.log",
            "audit-2023-02-29.log",
            "audit-2024-13-01.log",
            "audit-2024-00-01.log",
            "audit-2024-01-00.log",
            "audit-2024-01-32.log",
            "audit-2024-1-01.log",
            "notes.log",
        ] {
            assert_eq!(audit_log_day_from_file_name(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn audit_log_reader_returns_recent_sanitized_events_without_paths() {
        let temp = tempfile::tempdir().expect("temp dir");
        let writer = FileSystemAuditLogWriter::new(temp.path().to_path_buf());
        writer
            .record(AuditLogEvent {
                timestamp_unix_millis: 42,
                category: "diagnostic_export".to_owned(),
                operation: "export_preview_image_diagnostics".to_owned(),
                result: "success".to_owned(),
                fields: BTreeMap::from([(
                    "file_name".to_owned(),
                    "preview-image-diagnostics-42.zip".to_owned(),
                )]),
            })
            .expect("record first audit event");
        writer
            .record(AuditLogEvent {
                timestamp_unix_millis: 86_400_000,
                category: "diagnostic_export".to_owned(),
                operation: "export_preview_image_diagnostics".to_owned(),
                result: "failure".to_owned(),
                fields: BTreeMap::from([
                    (
                        "file_name".to_owned(),
                        "preview-image-diagnostics-86400000.zip".to_owned(),
                    ),
                    (
                        "error_code".to_owned(),
                        "diagnostic_package_export_failed".to_owned(),
                    ),
                ]),
            })
            .expect("record second audit event");

        let events = writer
            .read_recent_sanitized(AuditLogReadRequest { max_events: 1 })
            .expect("read recent audit event");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].timestamp_unix_millis, 86_400_000);
        assert_eq!(events[0].result, "failure");
        assert_eq!(
            events[0].fields["error_code"],
            "diagnostic_package_export_failed"
        );
        let serialized = serde_json::to_string(&events).expect("serialize audit events");
        assert!(!serialized.contains(temp.path().to_string_lossy().as_ref()));
        assert!(!serialized.contains("raw_path"));
        assert!(!serialized.contains("thumbnail://"));
        assert!(!serialized.contains("contentHash"));
        assert!(!serialized.contains("sandbox"));
        assert!(!serialized.contains("C:/"));
    }

    #[test]
    fn audit_log_reader_skips_corrupted_or_unsanitized_events() {
        let temp = tempfile::tempdir().expect("temp dir");
        let writer = FileSystemAuditLogWriter::new(temp.path().to_path_buf());
        writer
            .record(AuditLogEvent {
                timestamp_unix_millis: 42,
                category: "diagnostic_export".to_owned(),
                operation: "export_preview_image_diagnostics".to_owned(),
                result: "success".to_owned(),
                fields: BTreeMap::from([(
                    "file_name".to_owned(),
                    "preview-image-diagnostics-42.zip".to_owned(),
                )]),
            })
            .expect("record valid audit event");

        let audit_path = temp
            .path()
            .join("logs")
            .join("audit")
            .join("audit-1970-01-01.log");
        let mut file = OpenOptions::new()
            .append(true)
            .open(&audit_path)
            .expect("open audit log for fixture append");
        writeln!(file, "{{not-json").expect("write corrupted audit line");
        writeln!(
            file,
            "{}",
            serde_json::to_string(&AuditLogEvent {
                timestamp_unix_millis: 43,
                category: "diagnostic_export".to_owned(),
                operation: "export_audit_log_diagnostics".to_owned(),
                result: "failure".to_owned(),
                fields: BTreeMap::from([(
                    "raw_path".to_owned(),
                    "C:/Users/Player/raw_path/audit.log".to_owned(),
                )]),
            })
            .expect("serialize unsafe fixture event")
        )
        .expect("write unsafe audit line");

        let events = writer
            .read_recent_sanitized(AuditLogReadRequest { max_events: 10 })
            .expect("read only sanitized audit events");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].timestamp_unix_millis, 42);
        assert_eq!(
            events[0].fields["file_name"],
            "preview-image-diagnostics-42.zip"
        );
        let serialized = serde_json::to_string(&events).expect("serialize audit events");
        assert!(!serialized.contains("C:/Users/Player"));
        assert!(!serialized.contains("raw_path"));
    }

    #[test]
    fn audit_log_reader_returns_empty_when_max_events_is_zero() {
        let temp = tempfile::tempdir().expect("temp dir");
        let writer = FileSystemAuditLogWriter::new(temp.path().to_path_buf());
        writer
            .record(AuditLogEvent {
                timestamp_unix_millis: 42,
                category: "diagnostic_export".to_owned(),
                operation: "export_preview_image_diagnostics".to_owned(),
                result: "success".to_owned(),
                fields: BTreeMap::from([(
                    "file_name".to_owned(),
                    "preview-image-diagnostics-42.zip".to_owned(),
                )]),
            })
            .expect("record audit event");

        let events = writer
            .read_recent_sanitized(AuditLogReadRequest { max_events: 0 })
            .expect("read zero audit events");

        assert!(events.is_empty());
    }

    #[test]
    fn audit_log_reader_skips_generic_path_like_values() {
        let temp = tempfile::tempdir().expect("temp dir");
        let writer = FileSystemAuditLogWriter::new(temp.path().to_path_buf());
        let audit_dir = temp.path().join("logs").join("audit");
        fs::create_dir_all(&audit_dir).expect("create audit dir");
        let audit_path = audit_dir.join("audit-1970-01-01.log");
        fs::write(
            &audit_path,
            serde_json::to_string(&AuditLogEvent {
                timestamp_unix_millis: 42,
                category: "diagnostic_export".to_owned(),
                operation: "export_preview_image_diagnostics".to_owned(),
                result: "failure".to_owned(),
                fields: BTreeMap::from([(
                    "file_name".to_owned(),
                    "D:\\Games\\MonsterHunterWorld\\nativePC\\mod.bin".to_owned(),
                )]),
            })
            .expect("serialize path-like audit event"),
        )
        .expect("write path-like audit event");

        let events = writer
            .read_recent_sanitized(AuditLogReadRequest { max_events: 10 })
            .expect("read sanitized audit events");

        assert!(events.is_empty());
    }
}
