use anyhow::{Context, Result};
use hmm_ports::{AuditLogEvent, AuditLogReadRequest, AuditLogReader, AuditLogWriter};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const AUDIT_LOG_DIR: &str = "audit";
const MILLIS_PER_DAY: u128 = 86_400_000;

pub struct FileSystemAuditLogWriter {
    app_data_root: PathBuf,
    write_lock: Mutex<()>,
}

impl FileSystemAuditLogWriter {
    pub fn new(app_data_root: PathBuf) -> Self {
        Self {
            app_data_root,
            write_lock: Mutex::new(()),
        }
    }

    fn audit_dir(&self) -> PathBuf {
        self.app_data_root.join("logs").join(AUDIT_LOG_DIR)
    }

    fn audit_path_for_event(&self, event: &AuditLogEvent) -> Result<PathBuf> {
        Ok(self
            .audit_dir()
            .join(audit_log_file_name(event.timestamp_unix_millis)?))
    }
}

impl AuditLogWriter for FileSystemAuditLogWriter {
    fn record(&self, event: AuditLogEvent) -> Result<()> {
        validate_audit_event(&event)?;

        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("audit log write lock poisoned"))?;
        let audit_dir = self.audit_dir();
        fs::create_dir_all(&audit_dir).context("failed to create audit log directory")?;
        let audit_path = self.audit_path_for_event(&event)?;
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&audit_path)
            .context("failed to open audit log")?;
        let serialized =
            serde_json::to_string(&event).context("failed to serialize audit log event")?;
        file.write_all(serialized.as_bytes())
            .context("failed to write audit log event")?;
        file.write_all(b"\n")
            .context("failed to write audit log event")?;
        file.sync_all().context("failed to sync audit log")?;
        open_directory_for_sync(&audit_dir)
            .and_then(|directory| directory.sync_all())
            .context("failed to sync audit log directory")?;

        Ok(())
    }
}

impl AuditLogReader for FileSystemAuditLogWriter {
    fn read_recent_sanitized(&self, request: AuditLogReadRequest) -> Result<Vec<AuditLogEvent>> {
        if request.max_events == 0 {
            return Ok(Vec::new());
        }

        let audit_dir = self.audit_dir();
        let directory_entries = match fs::read_dir(&audit_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error).context("failed to read audit log directory"),
        };
        let mut audit_paths = Vec::new();
        for entry in directory_entries {
            let entry = entry.context("failed to read audit log directory entry")?;
            if !entry
                .file_type()
                .context("failed to inspect audit log entry")?
                .is_file()
            {
                continue;
            }

            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            if is_audit_log_file_name(file_name) {
                audit_paths.push(entry.path());
            }
        }
        audit_paths.sort();

        let mut events = Vec::new();
        for audit_path in audit_paths {
            let file = File::open(&audit_path).context("failed to open audit log")?;
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
                    events.remove(0);
                }
                events.push(event);
            }
        }

        Ok(events)
    }
}

fn is_audit_log_file_name(file_name: &str) -> bool {
    let bytes = file_name.as_bytes();
    bytes.len() == "audit-1970-01-01.log".len()
        && file_name.starts_with("audit-")
        && file_name.ends_with(".log")
        && bytes[6..10].iter().all(u8::is_ascii_digit)
        && bytes[10] == b'-'
        && bytes[11..13].iter().all(u8::is_ascii_digit)
        && bytes[13] == b'-'
        && bytes[14..16].iter().all(u8::is_ascii_digit)
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
    if value.is_empty() || value.chars().any(char::is_control) {
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
fn open_directory_for_sync(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;

    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

#[cfg(not(windows))]
fn open_directory_for_sync(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_ports::{AuditLogReadRequest, AuditLogReader};
    use std::collections::BTreeMap;

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
}
