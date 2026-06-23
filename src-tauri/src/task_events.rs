use crate::dto::{CommandErrorDto, TaskProgressEventDto};
use tauri::{AppHandle, Emitter};

pub const TASK_PROGRESS_EVENT_NAME: &str = "hmm://task-progress";

pub fn emit_task_progress(
    app_handle: &AppHandle,
    event: TaskProgressEventDto,
) -> Result<(), CommandErrorDto> {
    app_handle
        .emit(TASK_PROGRESS_EVENT_NAME, event)
        .map_err(|error| CommandErrorDto {
            code: "task_progress_emit_failed".to_owned(),
            message: format!("failed to emit task progress event: {error}"),
        })
}
