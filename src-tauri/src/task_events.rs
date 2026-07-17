use crate::dto::{CommandErrorDto, TaskKindDto, TaskProgressEventDto, TaskStatusDto};
use hmm_infra::{emit_safe_app_log, AppLogEvent};
use tauri::{AppHandle, Emitter};

pub const TASK_PROGRESS_EVENT_NAME: &str = "hmm://task-progress";
pub const INSTALL_REINSTALL_QUEUED_PHASE: &str = "install.reinstall.queued";

pub fn emit_task_progress(
    app_handle: &AppHandle,
    event: TaskProgressEventDto,
) -> Result<(), CommandErrorDto> {
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
