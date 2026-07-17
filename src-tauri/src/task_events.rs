use crate::dto::{CommandErrorDto, TaskKindDto, TaskProgressEventDto, TaskStatusDto};
use crate::state::AppState;
use hmm_infra::{emit_safe_app_log, AppLogEvent};
use hmm_ports::TaskLogRecord;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

pub const TASK_PROGRESS_EVENT_NAME: &str = "hmm://task-progress";
pub const INSTALL_REINSTALL_QUEUED_PHASE: &str = "install.reinstall.queued";

pub fn emit_task_progress(
    app_handle: &AppHandle,
    event: TaskProgressEventDto,
) -> Result<(), CommandErrorDto> {
    record_task_log(app_handle, &event);
    if let Some(registration) = queued_task_registration_event(&event) {
        emit_safe_app_log(registration);
    }
    let failure_event = AppLogEvent::warning("task.progress_emit_failed")
        .with_task_id(event.task_id.clone())
        .with_task_kind(task_kind_code(event.kind))
        .with_task_status(task_status_code(event.status))
        .with_phase(event.phase.clone())
        .with_error_code("task_progress_emit_failed");
    app_handle
        .emit(TASK_PROGRESS_EVENT_NAME, event)
        .map_err(|_| {
            emit_safe_app_log(failure_event);
            CommandErrorDto {
                code: "task_progress_emit_failed".to_owned(),
                message: "failed to emit task progress event".to_owned(),
            }
        })
}

fn record_task_log(app_handle: &AppHandle, event: &TaskProgressEventDto) {
    let Ok(timestamp_unix_millis) = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
    else {
        return;
    };
    let error_code = event
        .error
        .as_deref()
        .filter(|value| is_stable_code(value))
        .map(str::to_owned);
    let record = TaskLogRecord {
        timestamp_unix_millis,
        task_id: event.task_id.clone(),
        kind: task_kind_code(event.kind).to_owned(),
        status: task_status_code(event.status).to_owned(),
        phase: event.phase.clone(),
        current: event.current,
        total: event.total,
        duration_ms: None,
        error_code,
    };
    let _ = app_handle
        .state::<AppState>()
        .task_log_writer
        .record(record);
}

fn is_stable_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
}

fn queued_task_registration_event(event: &TaskProgressEventDto) -> Option<AppLogEvent> {
    (event.status == TaskStatusDto::Queued).then(|| {
        AppLogEvent::info("task.registered")
            .with_task_id(event.task_id.clone())
            .with_task_kind(task_kind_code(event.kind))
            .with_task_status(task_status_code(event.status))
            .with_phase(event.phase.clone())
    })
}

fn task_kind_code(kind: TaskKindDto) -> &'static str {
    match kind {
        TaskKindDto::ModImport => "mod_import",
        TaskKindDto::Install => "install",
        TaskKindDto::SaveBackup => "save_backup",
    }
}

fn task_status_code(status: TaskStatusDto) -> &'static str {
    match status {
        TaskStatusDto::Queued => "queued",
        TaskStatusDto::Running => "running",
        TaskStatusDto::Completed => "completed",
        TaskStatusDto::Failed => "failed",
        TaskStatusDto::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress_event(status: TaskStatusDto) -> TaskProgressEventDto {
        TaskProgressEventDto {
            task_id: "install-123".to_owned(),
            kind: TaskKindDto::Install,
            status,
            phase: "install.queued".to_owned(),
            current: None,
            total: None,
            message: Some("not logged".to_owned()),
            error: Some("C:/Users/Alice must not be logged".to_owned()),
            result_ref: Some("not-logged".to_owned()),
        }
    }

    #[test]
    fn queued_event_builds_task_registration_from_allowlisted_fields_only() {
        assert_eq!(
            queued_task_registration_event(&progress_event(TaskStatusDto::Queued)),
            Some(
                AppLogEvent::info("task.registered")
                    .with_task_id("install-123")
                    .with_task_kind("install")
                    .with_task_status("queued")
                    .with_phase("install.queued")
            )
        );
    }

    #[test]
    fn non_queued_event_does_not_create_a_registration_log() {
        assert!(queued_task_registration_event(&progress_event(TaskStatusDto::Running)).is_none());
    }
}
