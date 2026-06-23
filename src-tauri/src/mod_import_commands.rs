use crate::dto::{CommandErrorDto, TaskStartedDto};
use crate::state::AppState;
use crate::task_events::emit_task_progress;
use hmm_app::{StartImportModTaskRequest, TaskProgressEvent, TaskStarted};
use std::path::PathBuf;
use tauri::{AppHandle, State};

const MOD_IMPORT_QUEUED_PHASE: &str = "mod_import.queued";

#[tauri::command]
pub fn start_import_mod_task(
    archive_path: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let archive_path = parse_archive_path(archive_path)?;

    let task = state
        .mod_import_tasks
        .start_import_mod_task(StartImportModTaskRequest { archive_path })
        .map_err(CommandErrorDto::from_mod_import_task_error)?;

    emit_task_progress(&app_handle, queued_event_for_started_task(&task).into())?;

    Ok(task.into())
}

fn queued_event_for_started_task(task: &TaskStarted) -> TaskProgressEvent {
    TaskProgressEvent::new(
        task.task_id.clone(),
        task.kind,
        task.status,
        MOD_IMPORT_QUEUED_PHASE,
    )
}

fn parse_archive_path(value: String) -> Result<PathBuf, CommandErrorDto> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(CommandErrorDto {
            code: "archive_path_empty".to_owned(),
            message: "archive path cannot be empty".to_owned(),
        });
    }

    let archive_path = PathBuf::from(trimmed);
    if !archive_path.is_absolute() {
        return Err(CommandErrorDto {
            code: "archive_path_not_absolute".to_owned(),
            message: "archive path must be an absolute path".to_owned(),
        });
    }

    Ok(archive_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::TaskProgressEventDto;
    use hmm_app::{TaskKind, TaskStatus};
    use serde_json::Value;

    #[test]
    fn parse_archive_path_rejects_empty_paths() {
        let error = parse_archive_path("  ".to_owned()).expect_err("empty path rejected");

        assert_eq!(error.code, "archive_path_empty");
    }

    #[test]
    fn parse_archive_path_rejects_relative_paths() {
        let error =
            parse_archive_path("mods/sample.zip".to_owned()).expect_err("relative path rejected");

        assert_eq!(error.code, "archive_path_not_absolute");
    }

    #[test]
    fn queued_event_for_started_task_uses_registered_phase() {
        let task = TaskStarted {
            task_id: "mod-import-123".to_owned(),
            kind: TaskKind::ModImport,
            status: TaskStatus::Queued,
        };

        let dto: TaskProgressEventDto = queued_event_for_started_task(&task).into();
        let value: Value = serde_json::to_value(dto).expect("serialize event");

        assert_eq!(value["taskId"], "mod-import-123");
        assert_eq!(value["kind"], "mod_import");
        assert_eq!(value["status"], "queued");
        assert_eq!(value["phase"], MOD_IMPORT_QUEUED_PHASE);
        assert!(value["current"].is_null());
        assert!(value["total"].is_null());
        assert!(value["message"].is_null());
        assert!(value["error"].is_null());
        assert!(value["resultRef"].is_null());
    }
}
