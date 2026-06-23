use crate::dto::{CommandErrorDto, TaskStartedDto};
use crate::state::AppState;
use crate::task_events::emit_task_progress;
use hmm_app::{TaskProgressEvent, TaskSnapshot, TaskStarted};
use tauri::{AppHandle, State};

const MOD_IMPORT_CANCELLED_PHASE: &str = "mod_import.cancelled";

#[tauri::command]
pub fn cancel_task(
    task_id: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let task_id = parse_task_id(task_id)?;
    let task = state
        .task_manager
        .cancel_task(&task_id)
        .map_err(CommandErrorDto::from_task_manager_error)?;

    emit_task_progress(&app_handle, cancelled_event_for_task(&task).into())?;

    Ok(TaskStarted {
        task_id: task.task_id,
        kind: task.kind,
        status: task.status,
    }
    .into())
}

fn cancelled_event_for_task(task: &TaskSnapshot) -> TaskProgressEvent {
    TaskProgressEvent::new(
        task.task_id.clone(),
        task.kind,
        task.status,
        MOD_IMPORT_CANCELLED_PHASE,
    )
}

fn parse_task_id(value: String) -> Result<String, CommandErrorDto> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(CommandErrorDto {
            code: "task_id_empty".to_owned(),
            message: "task id cannot be empty".to_owned(),
        });
    }

    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::TaskProgressEventDto;
    use hmm_app::{TaskKind, TaskStatus};
    use serde_json::Value;

    #[test]
    fn parse_task_id_rejects_empty_values() {
        let error = parse_task_id("  ".to_owned()).expect_err("empty task id rejected");

        assert_eq!(error.code, "task_id_empty");
    }

    #[test]
    fn cancelled_event_for_task_uses_registered_phase() {
        let task = TaskSnapshot {
            task_id: "mod-import-123".to_owned(),
            kind: TaskKind::ModImport,
            status: TaskStatus::Cancelled,
        };

        let dto: TaskProgressEventDto = cancelled_event_for_task(&task).into();
        let value: Value = serde_json::to_value(dto).expect("serialize event");

        assert_eq!(value["taskId"], "mod-import-123");
        assert_eq!(value["kind"], "mod_import");
        assert_eq!(value["status"], "cancelled");
        assert_eq!(value["phase"], MOD_IMPORT_CANCELLED_PHASE);
        assert!(value["current"].is_null());
        assert!(value["total"].is_null());
        assert!(value["message"].is_null());
        assert!(value["error"].is_null());
        assert!(value["resultRef"].is_null());
    }

    #[test]
    fn maps_task_manager_errors_to_command_error_codes() {
        let not_found = CommandErrorDto::from_task_manager_error(
            hmm_app::TaskManagerError::TaskNotFound("missing-task".to_owned()),
        );
        assert_eq!(not_found.code, "task_not_found");

        let cannot_cancel = CommandErrorDto::from_task_manager_error(
            hmm_app::TaskManagerError::TaskCannotBeCancelled {
                task_id: "mod-import-123".to_owned(),
                status: TaskStatus::Completed,
            },
        );
        assert_eq!(cannot_cancel.code, "task_cannot_be_cancelled");
    }
}
