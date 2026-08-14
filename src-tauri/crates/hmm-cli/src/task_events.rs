use crate::contract::{CliTaskStatus, TaskEventEnvelope, TaskEventError, TaskEventType};
use hmm_ports::{TaskLogRecord, TaskLogWriter};
use hmm_runtime::{TaskKind, TaskProgressEvent, TaskProgressObserver, TaskStatus};
use serde_json::to_vec;
use std::fmt;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// JSONL observer used by long-running CLI commands.
///
/// The observer is deliberately transport-local: application runners only know the
/// `TaskProgressObserver` trait, while this type owns the CLI sequence and redaction
/// contract. Task Log is written before stdout so the persisted evidence has the same
/// event order as the Tauri bridge.
pub struct CliTaskProgressObserver<W> {
    command: &'static str,
    output: Arc<Mutex<W>>,
    task_log_writer: Arc<dyn TaskLogWriter>,
    sequence: AtomicU64,
    terminal_seen: AtomicBool,
    first_error: Mutex<Option<CliTaskProgressObserverError>>,
}

impl<W> CliTaskProgressObserver<W>
where
    W: Write + Send,
{
    pub fn new(
        command: &'static str,
        output: Arc<Mutex<W>>,
        task_log_writer: Arc<dyn TaskLogWriter>,
    ) -> Self {
        Self {
            command,
            output,
            task_log_writer,
            sequence: AtomicU64::new(0),
            terminal_seen: AtomicBool::new(false),
            first_error: Mutex::new(None),
        }
    }

    pub fn first_error(&self) -> Option<CliTaskProgressObserverError> {
        self.first_error.lock().ok().and_then(|error| *error)
    }

    fn build_envelope(&self, event: &TaskProgressEvent, sequence: u64) -> TaskEventEnvelope {
        let mut envelope = TaskEventEnvelope::new(
            event_type(event.status),
            self.command,
            sequence,
            event.task_id.clone(),
            event.status.into(),
            event.phase.clone(),
        );
        envelope.current = event.current;
        envelope.total = event.total;
        envelope.error = event_error_code(event).map(TaskEventError::new);
        envelope
    }

    fn record_task_log(
        &self,
        event: &TaskProgressEvent,
    ) -> Result<(), CliTaskProgressObserverError> {
        let timestamp_unix_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| CliTaskProgressObserverError::ClockUnavailable)?
            .as_millis();
        let record = TaskLogRecord {
            timestamp_unix_millis,
            task_id: event.task_id.clone(),
            kind: task_kind_code(event.kind).to_owned(),
            status: task_status_code(event.status).to_owned(),
            phase: event.phase.clone(),
            current: event.current,
            total: event.total,
            duration_ms: None,
            error_code: event_error_code(event),
        };
        self.task_log_writer
            .record(record)
            .map_err(|_| CliTaskProgressObserverError::TaskLogWrite)
    }

    fn write_jsonl(
        &self,
        envelope: &TaskEventEnvelope,
    ) -> Result<(), CliTaskProgressObserverError> {
        let mut output = self
            .output
            .lock()
            .map_err(|_| CliTaskProgressObserverError::OutputLock)?;
        let mut line = to_vec(envelope).map_err(|_| CliTaskProgressObserverError::Serialization)?;
        line.push(b'\n');
        output
            .write_all(&line)
            .and_then(|_| output.flush())
            .map_err(|_| CliTaskProgressObserverError::OutputWrite)
    }
}

impl<W> TaskProgressObserver for CliTaskProgressObserver<W>
where
    W: Write + Send,
{
    type Error = CliTaskProgressObserverError;

    fn observe(&self, event: &TaskProgressEvent) -> Result<(), Self::Error> {
        let result = (|| {
            validate_token("task_id", &event.task_id, true)?;
            validate_token("phase", &event.phase, false)?;
            self.accept_status(event.status)?;

            let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
            let envelope = self.build_envelope(event, sequence);

            let task_log_result = self.record_task_log(event);
            let output_result = self.write_jsonl(&envelope);

            match (task_log_result, output_result) {
                (Ok(()), Ok(())) => Ok(()),
                (Err(error), _) => Err(error),
                (_, Err(error)) => Err(error),
            }
        })();
        if let Err(error) = result {
            if let Ok(mut first_error) = self.first_error.lock() {
                first_error.get_or_insert(error);
            }
        }
        result
    }
}

impl<W> CliTaskProgressObserver<W> {
    fn accept_status(&self, status: TaskStatus) -> Result<(), CliTaskProgressObserverError> {
        if is_terminal(status) {
            return self
                .terminal_seen
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .map(|_| ())
                .map_err(|_| CliTaskProgressObserverError::DuplicateTerminal);
        }
        if self.terminal_seen.load(Ordering::Acquire) {
            return Err(CliTaskProgressObserverError::EventAfterTerminal);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliTaskProgressObserverError {
    ClockUnavailable,
    DuplicateTerminal,
    EventAfterTerminal,
    InvalidField,
    OutputLock,
    OutputWrite,
    Serialization,
    TaskLogWrite,
}

impl fmt::Display for CliTaskProgressObserverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ClockUnavailable => "task observer clock unavailable",
            Self::DuplicateTerminal => "task observer terminal event already emitted",
            Self::EventAfterTerminal => "task observer event follows terminal event",
            Self::InvalidField => "task observer field invalid",
            Self::OutputLock => "task observer output unavailable",
            Self::OutputWrite => "task observer output failed",
            Self::Serialization => "task observer serialization failed",
            Self::TaskLogWrite => "task observer task log failed",
        })
    }
}

impl std::error::Error for CliTaskProgressObserverError {}

fn event_type(status: TaskStatus) -> TaskEventType {
    match status {
        TaskStatus::Queued => TaskEventType::Started,
        TaskStatus::Running => TaskEventType::Progress,
        TaskStatus::Completed => TaskEventType::Completed,
        TaskStatus::Failed => TaskEventType::Failed,
        TaskStatus::Cancelled => TaskEventType::Cancelled,
    }
}

fn is_terminal(status: TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
    )
}

fn task_kind_code(kind: TaskKind) -> &'static str {
    match kind {
        TaskKind::ModImport => "mod_import",
        TaskKind::Install => "install",
        TaskKind::SaveBackup => "save_backup",
        TaskKind::SaveRestore => "save_restore",
    }
}

fn task_status_code(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Queued => "queued",
        TaskStatus::Running => "running",
        TaskStatus::Completed => "completed",
        TaskStatus::Failed => "failed",
        TaskStatus::Cancelled => "cancelled",
    }
}

impl From<TaskStatus> for CliTaskStatus {
    fn from(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Queued => Self::Queued,
            TaskStatus::Running => Self::Running,
            TaskStatus::Completed => Self::Completed,
            TaskStatus::Failed => Self::Failed,
            TaskStatus::Cancelled => Self::Cancelled,
        }
    }
}

fn sanitize_error_code(value: &str) -> Option<String> {
    (value.len() <= 96
        && !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-')))
    .then(|| value.to_owned())
}

fn event_error_code(event: &TaskProgressEvent) -> Option<String> {
    (event.status == TaskStatus::Failed)
        .then(|| event.error.as_deref().and_then(sanitize_error_code))
        .flatten()
}

fn validate_token(
    _name: &'static str,
    value: &str,
    allow_dash: bool,
) -> Result<(), CliTaskProgressObserverError> {
    if value.is_empty()
        || value.len() > 160
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'_' | b'.')
                || (allow_dash && byte == b'-')
        })
    {
        return Err(CliTaskProgressObserverError::InvalidField);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::CLI_SCHEMA_VERSION;
    use hmm_ports::PortResult;
    use serde_json::Value;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingTaskLogWriter {
        records: Mutex<Vec<TaskLogRecord>>,
        order: Option<Arc<Mutex<Vec<&'static str>>>>,
    }

    impl TaskLogWriter for RecordingTaskLogWriter {
        fn record(&self, record: TaskLogRecord) -> PortResult<()> {
            if let Some(order) = &self.order {
                order.lock().expect("order lock").push("task-log");
            }
            self.records.lock().expect("record lock").push(record);
            Ok(())
        }
    }

    struct OrderingWriter {
        output: Vec<u8>,
        order: Arc<Mutex<Vec<&'static str>>>,
    }

    struct FailingTaskLogWriter;

    impl TaskLogWriter for FailingTaskLogWriter {
        fn record(&self, _record: TaskLogRecord) -> PortResult<()> {
            Err(std::io::Error::other("fixture task log failure").into())
        }
    }

    impl Write for OrderingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.order.lock().expect("order lock").push("stdout");
            self.output.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn event(status: TaskStatus) -> TaskProgressEvent {
        let mut event = TaskProgressEvent::new(
            "install-opaque-id",
            TaskKind::Install,
            status,
            "install.plan.building",
        );
        event.current = Some(2);
        event.total = Some(3);
        event.message = Some("C:/Users/Alice/private message".to_owned());
        event.result_ref = Some("C:/Users/Alice/result.json".to_owned());
        event
    }

    #[test]
    fn emits_stable_jsonl_sequence_and_drops_sensitive_fields() {
        let output = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Arc::new(RecordingTaskLogWriter::default());
        let observer =
            CliTaskProgressObserver::new("install.apply", output.clone(), writer.clone());

        let mut failed = event(TaskStatus::Failed);
        failed.error = Some("install_failed:install.plan.building".to_owned());
        observer
            .observe(&event(TaskStatus::Queued))
            .expect("queued");
        observer
            .observe(&event(TaskStatus::Running))
            .expect("running");
        observer.observe(&failed).expect("failed");

        let output = output.lock().expect("output lock").clone();
        let lines = String::from_utf8(output)
            .expect("utf8")
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("json line"))
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["sequence"], 0);
        assert_eq!(lines[1]["sequence"], 1);
        assert_eq!(lines[2]["sequence"], 2);
        assert_eq!(lines[0]["type"], "started");
        assert_eq!(lines[2]["type"], "failed");
        assert_eq!(
            lines[2]["error"]["code"],
            "install_failed:install.plan.building"
        );
        for line in &lines {
            assert_eq!(line["schemaVersion"], CLI_SCHEMA_VERSION);
            assert!(!line.to_string().contains("Alice"));
            assert!(!line.to_string().contains("private message"));
            assert!(!line.to_string().contains("result.json"));
            assert!(line.get("result").is_none());
        }

        let records = writer.records.lock().expect("records lock");
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].task_id, "install-opaque-id");
        assert_eq!(
            records[2].error_code.as_deref(),
            Some("install_failed:install.plan.building")
        );
    }

    #[test]
    fn writes_task_log_before_stdout() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let output = Arc::new(Mutex::new(OrderingWriter {
            output: Vec::new(),
            order: order.clone(),
        }));
        let writer = Arc::new(RecordingTaskLogWriter {
            records: Mutex::new(Vec::new()),
            order: Some(order.clone()),
        });
        let observer = CliTaskProgressObserver::new("install.apply", output, writer);

        observer
            .observe(&event(TaskStatus::Running))
            .expect("running");

        assert_eq!(
            *order.lock().expect("order lock"),
            vec!["task-log", "stdout"]
        );
    }

    #[test]
    fn unsafe_error_is_omitted_instead_of_echoed() {
        let output = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Arc::new(RecordingTaskLogWriter::default());
        let observer = CliTaskProgressObserver::new("install.apply", output.clone(), writer);
        let mut event = event(TaskStatus::Failed);
        event.error = Some("C:/Users/Alice/secret".to_owned());

        observer
            .observe(&event)
            .expect("unsafe error is best effort");

        let output = String::from_utf8(output.lock().expect("output lock").clone()).expect("utf8");
        let value: Value = serde_json::from_str(output.trim()).expect("json");
        assert!(value.get("error").is_none());
        assert!(!output.contains("Alice"));
        assert!(!output.contains("secret"));
    }

    #[test]
    fn invalid_identity_fails_without_writing_jsonl() {
        let output = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Arc::new(RecordingTaskLogWriter::default());
        let observer = CliTaskProgressObserver::new("install.apply", output.clone(), writer);
        let mut invalid_event = event(TaskStatus::Running);
        invalid_event.task_id = "../private".to_owned();

        assert_eq!(
            observer.observe(&invalid_event),
            Err(CliTaskProgressObserverError::InvalidField)
        );
        assert!(output.lock().expect("output lock").is_empty());

        observer
            .observe(&event(TaskStatus::Running))
            .expect("first valid event");
        let output = String::from_utf8(output.lock().expect("output lock").clone()).expect("utf8");
        let value: Value = serde_json::from_str(output.trim()).expect("json");
        assert_eq!(value["sequence"], 0);
    }

    #[test]
    fn task_log_failure_is_queryable_after_jsonl_is_flushed() {
        let output = Arc::new(Mutex::new(Vec::<u8>::new()));
        let observer = CliTaskProgressObserver::new(
            "install.apply",
            output.clone(),
            Arc::new(FailingTaskLogWriter),
        );

        assert_eq!(
            observer.observe(&event(TaskStatus::Running)),
            Err(CliTaskProgressObserverError::TaskLogWrite)
        );
        assert_eq!(
            observer.first_error(),
            Some(CliTaskProgressObserverError::TaskLogWrite)
        );
        assert_eq!(
            String::from_utf8(output.lock().expect("output lock").clone())
                .expect("utf8")
                .lines()
                .count(),
            1
        );
    }

    #[test]
    fn duplicate_terminal_and_events_after_terminal_are_rejected() {
        let output = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Arc::new(RecordingTaskLogWriter::default());
        let observer = CliTaskProgressObserver::new("install.apply", output.clone(), writer);

        observer
            .observe(&event(TaskStatus::Completed))
            .expect("first terminal");
        assert_eq!(
            observer.observe(&event(TaskStatus::Failed)),
            Err(CliTaskProgressObserverError::DuplicateTerminal)
        );
        assert_eq!(
            observer.observe(&event(TaskStatus::Running)),
            Err(CliTaskProgressObserverError::EventAfterTerminal)
        );
        assert_eq!(
            String::from_utf8(output.lock().expect("output lock").clone())
                .expect("utf8")
                .lines()
                .count(),
            1
        );
    }

    #[test]
    fn status_mapping_is_stable() {
        let statuses = [
            (
                TaskStatus::Queued,
                TaskEventType::Started,
                CliTaskStatus::Queued,
            ),
            (
                TaskStatus::Running,
                TaskEventType::Progress,
                CliTaskStatus::Running,
            ),
            (
                TaskStatus::Completed,
                TaskEventType::Completed,
                CliTaskStatus::Completed,
            ),
            (
                TaskStatus::Failed,
                TaskEventType::Failed,
                CliTaskStatus::Failed,
            ),
            (
                TaskStatus::Cancelled,
                TaskEventType::Cancelled,
                CliTaskStatus::Cancelled,
            ),
        ];
        for (status, event_kind, cli_status) in statuses {
            assert_eq!(event_type(status), event_kind);
            assert_eq!(CliTaskStatus::from(status), cli_status);
        }
    }
}
