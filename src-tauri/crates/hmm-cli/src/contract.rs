use serde::Serialize;
use serde_json::Value;

pub const CLI_SCHEMA_VERSION: &str = "hmm.cli/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum CliExitCode {
    Success = 0,
    Usage = 2,
    Rejected = 3,
    ControlledFailure = 4,
    PartialSuccess = 5,
    RuntimeUnavailable = 6,
    Cancelled = 130,
}

impl CliExitCode {
    pub const fn get(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CliErrorCategory {
    UserActionRequired,
    Recoverable,
    RollbackSucceeded,
    RollbackFailed,
    DataSafetyRisk,
    InternalBug,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliErrorEnvelope {
    pub code: &'static str,
    pub category: CliErrorCategory,
    pub retryable: bool,
}

impl CliErrorEnvelope {
    pub const fn new(code: &'static str, category: CliErrorCategory, retryable: bool) -> Self {
        Self {
            code,
            category,
            retryable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEnvelope<T> {
    pub schema_version: &'static str,
    pub command: &'static str,
    pub ok: bool,
    pub task_id: Option<String>,
    pub result: Option<T>,
    pub error: Option<CliErrorEnvelope>,
}

impl<T> CommandEnvelope<T> {
    pub const fn success(command: &'static str, result: T) -> Self {
        Self {
            schema_version: CLI_SCHEMA_VERSION,
            command,
            ok: true,
            task_id: None,
            result: Some(result),
            error: None,
        }
    }

    pub const fn failure(command: &'static str, error: CliErrorEnvelope) -> Self {
        Self {
            schema_version: CLI_SCHEMA_VERSION,
            command,
            ok: false,
            task_id: None,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskEventType {
    Started,
    Progress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CliTaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskEventEnvelope {
    pub schema_version: &'static str,
    #[serde(rename = "type")]
    pub event_type: TaskEventType,
    pub command: &'static str,
    pub sequence: u64,
    pub task_id: String,
    pub status: CliTaskStatus,
    pub phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TaskEventError>,
}

impl TaskEventEnvelope {
    pub fn new(
        event_type: TaskEventType,
        command: &'static str,
        sequence: u64,
        task_id: impl Into<String>,
        status: CliTaskStatus,
        phase: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: CLI_SCHEMA_VERSION,
            event_type,
            command,
            sequence,
            task_id: task_id.into(),
            status,
            phase: phase.into(),
            current: None,
            total: None,
            result: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskEventError {
    pub code: String,
}

impl TaskEventError {
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ExampleResult {
        production_writes_allowed: bool,
    }

    #[test]
    fn success_envelope_has_stable_schema_and_null_error_fields() {
        let envelope = CommandEnvelope::success(
            "runtime.status",
            ExampleResult {
                production_writes_allowed: false,
            },
        );
        let value = serde_json::to_value(envelope).expect("serialize envelope");

        assert_eq!(value["schemaVersion"], CLI_SCHEMA_VERSION);
        assert_eq!(value["command"], "runtime.status");
        assert_eq!(value["ok"], true);
        assert_eq!(value["taskId"], Value::Null);
        assert_eq!(value["result"]["productionWritesAllowed"], false);
        assert_eq!(value["error"], Value::Null);
    }

    #[test]
    fn failure_envelope_uses_stable_category_without_message() {
        let envelope = CommandEnvelope::<Value>::failure(
            "runtime.status",
            CliErrorEnvelope::new(
                "sandbox_data_dir_required",
                CliErrorCategory::UserActionRequired,
                false,
            ),
        );
        let value = serde_json::to_value(envelope).expect("serialize envelope");

        assert_eq!(value["ok"], false);
        assert_eq!(value["result"], Value::Null);
        assert_eq!(value["error"]["code"], "sandbox_data_dir_required");
        assert_eq!(value["error"]["category"], "user_action_required");
        assert!(value["error"].get("message").is_none());
    }

    #[test]
    fn task_event_omits_free_text_and_absent_optional_fields() {
        let event = TaskEventEnvelope::new(
            TaskEventType::Progress,
            "install.apply",
            1,
            "install-opaque-id",
            CliTaskStatus::Running,
            "install.plan.building",
        );
        let value = serde_json::to_value(event).expect("serialize task event");

        assert_eq!(value["schemaVersion"], CLI_SCHEMA_VERSION);
        assert_eq!(value["type"], "progress");
        assert_eq!(value["taskId"], "install-opaque-id");
        assert!(value.get("message").is_none());
        assert!(value.get("resultRef").is_none());
        assert!(value.get("current").is_none());
        assert!(value.get("error").is_none());
    }

    #[test]
    fn exit_codes_match_the_documented_script_categories() {
        assert_eq!(CliExitCode::Success.get(), 0);
        assert_eq!(CliExitCode::Usage.get(), 2);
        assert_eq!(CliExitCode::Rejected.get(), 3);
        assert_eq!(CliExitCode::ControlledFailure.get(), 4);
        assert_eq!(CliExitCode::PartialSuccess.get(), 5);
        assert_eq!(CliExitCode::RuntimeUnavailable.get(), 6);
        assert_eq!(CliExitCode::Cancelled.get(), 130);
    }
}
