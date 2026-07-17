use anyhow::{Context, Result};
use hmm_ports::{DiagnosticsEvidenceHealth, TaskLogRecord, TaskLogWriter};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct FileSystemTaskLogWriter {
    root: PathBuf,
    health: Arc<dyn DiagnosticsEvidenceHealth>,
    started_at: Mutex<HashMap<String, u128>>,
}

impl FileSystemTaskLogWriter {
    pub fn new(root: PathBuf, health: Arc<dyn DiagnosticsEvidenceHealth>) -> Self {
        Self {
            root,
            health,
            started_at: Mutex::new(HashMap::new()),
        }
    }

    fn write_record(&self, mut record: TaskLogRecord) -> Result<()> {
        validate_token("task_id", &record.task_id, true)?;
        validate_token("kind", &record.kind, false)?;
        validate_token("status", &record.status, false)?;
        validate_token("phase", &record.phase, false)?;
        if let Some(code) = record.error_code.as_deref() {
            validate_error_code(code)?;
        }
        let mut starts = self
            .started_at
            .lock()
            .map_err(|_| anyhow::anyhow!("task log state unavailable"))?;
        let started_at = starts
            .entry(record.task_id.clone())
            .or_insert(record.timestamp_unix_millis);
        record.duration_ms =
            u64::try_from(record.timestamp_unix_millis.saturating_sub(*started_at)).ok();
        if matches!(record.status.as_str(), "completed" | "failed" | "cancelled") {
            starts.remove(&record.task_id);
        }
        drop(starts);

        let dir = prepare_task_log_directory(&self.root)?;
        let path = dir.join(format!("task-{}.log", record.task_id));
        if path
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            anyhow::bail!("task log path is a symlink");
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .context("failed to open task log")?;
        serde_json::to_writer(&mut file, &SerializableTaskLogRecord::from(record))
            .context("failed to serialize task log")?;
        file.write_all(b"\n")
            .and_then(|_| file.sync_data())
            .context("failed to write task log")
    }
}

fn prepare_task_log_directory(root: &std::path::Path) -> Result<PathBuf> {
    let logs = root.join("logs");
    reject_symlink_if_present(&logs)?;
    fs::create_dir_all(&logs).context("failed to prepare logs directory")?;
    let tasks = logs.join("tasks");
    reject_symlink_if_present(&tasks)?;
    fs::create_dir_all(&tasks).context("failed to prepare task log directory")?;
    Ok(tasks)
}

fn reject_symlink_if_present(path: &std::path::Path) -> Result<()> {
    match path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() => anyhow::bail!("task log directory is a symlink"),
        Ok(metadata) if !metadata.is_dir() => anyhow::bail!("task log directory is not a directory"),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("failed to inspect task log directory"),
    }
}

fn validate_error_code(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 96
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
    {
        anyhow::bail!("invalid error_code");
    }
    Ok(())
}

impl TaskLogWriter for FileSystemTaskLogWriter {
    fn record(&self, record: TaskLogRecord) -> Result<()> {
        self.write_record(record).inspect_err(|_| {
            self.health
                .record_task_log_write_failure("task_log_write_failed");
        })
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializableTaskLogRecord {
    schema_version: u8,
    timestamp_unix_millis: u128,
    task_id: String,
    kind: String,
    status: String,
    phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    current: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
}

impl From<TaskLogRecord> for SerializableTaskLogRecord {
    fn from(value: TaskLogRecord) -> Self {
        Self {
            schema_version: 1,
            timestamp_unix_millis: value.timestamp_unix_millis,
            task_id: value.task_id,
            kind: value.kind,
            status: value.status,
            phase: value.phase,
            current: value.current,
            total: value.total,
            duration_ms: value.duration_ms,
            error_code: value.error_code,
        }
    }
}

fn validate_token(name: &str, value: &str, allow_dash: bool) -> Result<()> {
    if value.is_empty()
        || value.len() > 160
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'_' | b'.')
                || (allow_dash && byte == b'-')
        })
    {
        anyhow::bail!("invalid {name}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagnosticsEvidenceHealthState;
    use hmm_ports::DiagnosticsEvidenceHealth;

    fn event(task_id: &str, status: &str, timestamp: u128) -> TaskLogRecord {
        TaskLogRecord {
            timestamp_unix_millis: timestamp,
            task_id: task_id.to_owned(),
            kind: "install".to_owned(),
            status: status.to_owned(),
            phase: format!("install.{status}"),
            current: Some(1),
            total: Some(2),
            duration_ms: None,
            error_code: None,
        }
    }

    #[test]
    fn writes_each_task_to_its_own_allowlisted_jsonl_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let health: Arc<dyn DiagnosticsEvidenceHealth> =
            Arc::new(DiagnosticsEvidenceHealthState::default());
        let writer = FileSystemTaskLogWriter::new(temp.path().to_path_buf(), health);
        writer
            .record(event("install-1", "running", 100))
            .expect("first task log");
        writer
            .record(event("install-2", "completed", 130))
            .expect("second task log");

        let first = fs::read_to_string(temp.path().join("logs/tasks/task-install-1.log"))
            .expect("first file");
        let second = fs::read_to_string(temp.path().join("logs/tasks/task-install-2.log"))
            .expect("second file");
        assert!(first.contains("\"taskId\":\"install-1\""));
        assert!(!first.contains("install-2"));
        assert!(second.contains("\"durationMs\":0"));
        assert!(!first.contains("message"));
        assert!(!first.contains("resultRef"));
    }

    #[test]
    fn invalid_identity_degrades_health_without_creating_a_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let health = Arc::new(DiagnosticsEvidenceHealthState::default());
        let writer = FileSystemTaskLogWriter::new(temp.path().to_path_buf(), health.clone());
        assert!(writer.record(event("bad/path", "running", 100)).is_err());
        assert_eq!(health.snapshot().task_log_status, "task_log_write_failed");
        assert!(!temp.path().join("logs/tasks").exists());
    }
}
