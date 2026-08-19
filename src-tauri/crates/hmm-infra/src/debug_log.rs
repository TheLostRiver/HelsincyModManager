use crate::managed_log::{
    dated_log_file_name, open_append_regular_file, open_or_create_log_directory,
};
use anyhow::{Context, Result};
use hmm_ports::{DebugLogControl, DiagnosticsEvidenceHealth};
use serde::Serialize;
use std::ffi::OsStr;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const MILLIS_PER_DAY: u128 = 86_400_000;
const MAX_CODE_LENGTH: usize = 96;
const MAX_ID_LENGTH: usize = 160;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugLogWriteOutcome {
    Disabled,
    Written,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugLogEvent {
    event: String,
    component: Option<String>,
    operation: Option<String>,
    result: Option<String>,
    error_code: Option<String>,
    task_id: Option<String>,
    item_count: Option<u64>,
    duration_ms: Option<u64>,
}

impl DebugLogEvent {
    pub fn new(event: impl Into<String>) -> Self {
        Self {
            event: event.into(),
            component: None,
            operation: None,
            result: None,
            error_code: None,
            task_id: None,
            item_count: None,
            duration_ms: None,
        }
    }

    pub fn with_component(mut self, value: impl Into<String>) -> Self {
        self.component = Some(value.into());
        self
    }

    pub fn with_operation(mut self, value: impl Into<String>) -> Self {
        self.operation = Some(value.into());
        self
    }

    pub fn with_result(mut self, value: impl Into<String>) -> Self {
        self.result = Some(value.into());
        self
    }

    pub fn with_error_code(mut self, value: impl Into<String>) -> Self {
        self.error_code = Some(value.into());
        self
    }

    pub fn with_task_id(mut self, value: impl Into<String>) -> Self {
        self.task_id = Some(value.into());
        self
    }

    pub fn with_item_count(mut self, value: u64) -> Self {
        self.item_count = Some(value);
        self
    }

    pub fn with_duration_ms(mut self, value: u64) -> Self {
        self.duration_ms = Some(value);
        self
    }

    fn validate(&self) -> Result<()> {
        validate_code("event", &self.event)?;
        for (label, value) in [
            ("component", self.component.as_deref()),
            ("operation", self.operation.as_deref()),
            ("result", self.result.as_deref()),
            ("error_code", self.error_code.as_deref()),
        ] {
            if let Some(value) = value {
                validate_code(label, value)?;
            }
        }
        if let Some(task_id) = self.task_id.as_deref() {
            validate_id("task_id", task_id)?;
        }
        Ok(())
    }
}

pub struct DebugLogController {
    app_data_root: PathBuf,
    enabled: AtomicBool,
    health: Arc<dyn DiagnosticsEvidenceHealth>,
}

impl DebugLogController {
    pub fn new(
        app_data_root: PathBuf,
        enabled: bool,
        health: Arc<dyn DiagnosticsEvidenceHealth>,
    ) -> Self {
        Self {
            app_data_root,
            enabled: AtomicBool::new(enabled),
            health,
        }
    }

    pub fn record(&self, event: DebugLogEvent) -> Result<DebugLogWriteOutcome> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("debug log clock unavailable")?
            .as_millis();
        self.record_at(event, timestamp)
    }

    fn record_at(
        &self,
        event: DebugLogEvent,
        timestamp_unix_millis: u128,
    ) -> Result<DebugLogWriteOutcome> {
        if !self.is_enabled() {
            return Ok(DebugLogWriteOutcome::Disabled);
        }
        if let Err(error) = event.validate() {
            self.health.record_debug_log_event_rejected();
            return Err(error);
        }
        if let Err(error) = self.write_validated(event, timestamp_unix_millis) {
            self.health.record_debug_log_write_failure();
            return Err(error);
        }
        Ok(DebugLogWriteOutcome::Written)
    }

    fn write_validated(&self, event: DebugLogEvent, timestamp_unix_millis: u128) -> Result<()> {
        let day = i64::try_from(timestamp_unix_millis / MILLIS_PER_DAY)
            .context("debug log timestamp is out of range")?;
        let file_name = dated_log_file_name("debug-", day)?;
        let directory =
            open_or_create_log_directory(&self.app_data_root, "debug", "debug log directory")?;
        let mut file =
            open_append_regular_file(&directory, OsStr::new(&file_name), "debug log file")?;
        let record = DebugLogRecord {
            schema_version: 1,
            timestamp_unix_millis,
            event: event.event,
            component: event.component,
            operation: event.operation,
            result: event.result,
            error_code: event.error_code,
            task_id: event.task_id,
            item_count: event.item_count,
            duration_ms: event.duration_ms,
        };
        let serialized = serde_json::to_vec(&record).context("failed to serialize debug log")?;
        file.write_all(&serialized)
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_data())
            .context("failed to write debug log")
    }
}

impl DebugLogControl for DebugLogController {
    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }
}

#[derive(Serialize)]
struct DebugLogRecord {
    schema_version: u8,
    timestamp_unix_millis: u128,
    event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    component: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    item_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
}

fn validate_code(label: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_CODE_LENGTH
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.-".contains(&byte)
        })
        || contains_sensitive_text(value)
    {
        anyhow::bail!("debug log event contains invalid {label}");
    }
    Ok(())
}

fn validate_id(label: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_ID_LENGTH
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
        || contains_sensitive_text(value)
    {
        anyhow::bail!("debug log event contains invalid {label}");
    }
    Ok(())
}

fn contains_sensitive_text(value: &str) -> bool {
    if value.chars().any(char::is_control) {
        return true;
    }
    let lower = value.to_ascii_lowercase();
    [
        "token",
        "cookie",
        "api_key",
        "api-key",
        "apikey",
        "authorization",
        "password",
        "secret",
        "steamid",
        "steam_id",
        "c:/",
        "c:\\",
        "\\users\\",
        "/users/",
        "/home/",
        "/root/",
        "appdata\\",
    ]
    .iter()
    .any(|snippet| lower.contains(snippet))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagnosticsEvidenceHealthState;
    use hmm_ports::DiagnosticsEvidenceHealth;
    use std::fs;
    use std::sync::Arc;

    const DAY_MILLIS: u128 = 86_400_000;

    #[test]
    fn disabled_debug_log_does_not_create_storage() {
        let temp = tempfile::tempdir().expect("temp dir");
        let health = Arc::new(DiagnosticsEvidenceHealthState::default());
        let logger = DebugLogController::new(temp.path().to_path_buf(), false, health);

        let outcome = logger
            .record_at(DebugLogEvent::new("runtime.initialized"), DAY_MILLIS)
            .expect("disabled write is a no-op");

        assert_eq!(outcome, DebugLogWriteOutcome::Disabled);
        assert!(!temp.path().join("logs").exists());
    }

    #[test]
    fn enabled_debug_log_writes_only_the_bounded_json_schema() {
        let temp = tempfile::tempdir().expect("temp dir");
        let health = Arc::new(DiagnosticsEvidenceHealthState::default());
        let logger = DebugLogController::new(temp.path().to_path_buf(), true, health.clone());

        let outcome = logger
            .record_at(
                DebugLogEvent::new("runtime.initialized")
                    .with_component("runtime")
                    .with_operation("composition")
                    .with_result("success")
                    .with_task_id("worker-42")
                    .with_item_count(3)
                    .with_duration_ms(7),
                DAY_MILLIS,
            )
            .expect("debug event written");

        assert_eq!(outcome, DebugLogWriteOutcome::Written);
        let text = fs::read_to_string(temp.path().join("logs/debug/debug-1970-01-02.log"))
            .expect("read debug log");
        let value: serde_json::Value = serde_json::from_str(text.trim()).expect("valid jsonl");
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["timestamp_unix_millis"], DAY_MILLIS as u64);
        assert_eq!(value["event"], "runtime.initialized");
        assert_eq!(value["component"], "runtime");
        assert_eq!(value["operation"], "composition");
        assert_eq!(value["result"], "success");
        assert_eq!(value["task_id"], "worker-42");
        assert_eq!(value["item_count"], 3);
        assert_eq!(value["duration_ms"], 7);
        assert_eq!(value.as_object().expect("object").len(), 9);
        assert_eq!(health.snapshot().debug_log_status, "ok");
    }

    #[test]
    fn path_like_and_free_text_values_are_rejected_without_writing() {
        let temp = tempfile::tempdir().expect("temp dir");
        let health = Arc::new(DiagnosticsEvidenceHealthState::default());
        let logger = DebugLogController::new(temp.path().to_path_buf(), true, health.clone());

        for event in [
            DebugLogEvent::new("runtime initialized with details"),
            DebugLogEvent::new("runtime.initialized").with_error_code("C:/Users/Player/raw.log"),
            DebugLogEvent::new("runtime.initialized").with_task_id("../outside"),
        ] {
            assert!(logger.record_at(event, DAY_MILLIS).is_err());
        }

        assert!(!temp.path().join("logs/debug/debug-1970-01-02.log").exists());
        assert_eq!(
            health.snapshot().debug_log_status,
            "debug_log_event_rejected"
        );
    }
}
