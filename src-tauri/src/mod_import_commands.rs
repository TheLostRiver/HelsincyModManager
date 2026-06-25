use crate::dto::{
    AppSettingsDto, AuditLogDiagnosticsExportDto, CommandErrorDto, ModDetailDto, ModLibraryItemDto,
    PreviewImageCandidateListDto, PreviewImageDiagnosticsDto, PreviewImageDiagnosticsExportDto,
    PreviewImageDto, SupportDiagnosticsExportDto, TaskStartedDto,
};
use crate::state::AppState;
use crate::task_events::emit_task_progress;
use hmm_app::{StartImportModTaskRequest, TaskProgressEvent, TaskStarted};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};

const MOD_IMPORT_QUEUED_PHASE: &str = "mod_import.queued";

#[tauri::command]
pub fn start_import_mod_task(
    archive_path: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let archive_path = parse_archive_path(archive_path)?;

    let runner_archive_path = archive_path.clone();
    let task = state
        .mod_import_tasks
        .start_import_mod_task(StartImportModTaskRequest { archive_path })
        .map_err(CommandErrorDto::from_mod_import_task_error)?;

    emit_task_progress(&app_handle, queued_event_for_started_task(&task).into())?;
    spawn_prepare_runner(
        Arc::clone(&state.mod_import_task_runner),
        app_handle,
        task.task_id.clone(),
        runner_archive_path,
    );

    Ok(task.into())
}

#[tauri::command]
pub fn get_mod_library(
    state: State<'_, AppState>,
) -> Result<Vec<ModLibraryItemDto>, CommandErrorDto> {
    let items = state
        .mod_library
        .get_mod_library()
        .map_err(|_| mod_library_unavailable_error())?;

    Ok(items.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub fn get_mod_detail(
    mod_id: String,
    state: State<'_, AppState>,
) -> Result<Option<ModDetailDto>, CommandErrorDto> {
    let mod_id = parse_mod_id(mod_id)?;
    let detail = state
        .mod_library
        .get_mod_detail(&mod_id)
        .map_err(|_| mod_library_unavailable_error())?;

    Ok(detail.map(Into::into))
}

#[tauri::command]
pub fn get_preview_image_diagnostics(
    state: State<'_, AppState>,
) -> Result<PreviewImageDiagnosticsDto, CommandErrorDto> {
    let summary = state
        .mod_library
        .get_preview_image_diagnostics()
        .map_err(|_| mod_library_unavailable_error())?;

    Ok(summary.into())
}

#[tauri::command]
pub fn export_preview_image_diagnostics(
    state: State<'_, AppState>,
) -> Result<PreviewImageDiagnosticsExportDto, CommandErrorDto> {
    let export = state
        .preview_image_diagnostics_export
        .export_preview_image_diagnostics()
        .map_err(|_| preview_image_diagnostics_export_unavailable_error())?;

    Ok(export.into())
}

#[tauri::command]
pub fn export_audit_log_diagnostics(
    state: State<'_, AppState>,
) -> Result<AuditLogDiagnosticsExportDto, CommandErrorDto> {
    let export = state
        .audit_log_diagnostics_export
        .export_audit_log_diagnostics(hmm_app::MAX_AUDIT_LOG_DIAGNOSTIC_EVENTS)
        .map_err(|_| audit_log_diagnostics_export_unavailable_error())?;

    Ok(export.into())
}

#[tauri::command]
pub fn export_support_diagnostics(
    state: State<'_, AppState>,
) -> Result<SupportDiagnosticsExportDto, CommandErrorDto> {
    let export = state
        .support_diagnostics_export
        .export_support_diagnostics()
        .map_err(|_| support_diagnostics_export_unavailable_error())?;

    Ok(export.into())
}

#[tauri::command]
pub fn get_preview_image_candidates(
    mod_id: String,
    state: State<'_, AppState>,
) -> Result<Option<PreviewImageCandidateListDto>, CommandErrorDto> {
    let mod_id = parse_mod_id(mod_id)?;
    let candidates = state
        .preview_image_candidates
        .list_candidates(&mod_id)
        .map_err(|_| preview_image_candidates_unavailable_error())?;

    Ok(candidates.map(Into::into))
}

#[tauri::command]
pub fn select_preview_image_candidate(
    mod_id: String,
    candidate_index: i64,
    state: State<'_, AppState>,
) -> Result<Option<PreviewImageDto>, CommandErrorDto> {
    let mod_id = parse_mod_id(mod_id)?;
    let candidate_index = parse_candidate_index(candidate_index)?;
    let preview_image = state
        .preview_image_selection
        .select_candidate(&mod_id, candidate_index)
        .map_err(|_| preview_image_selection_unavailable_error())?;

    Ok(preview_image.map(Into::into))
}

#[tauri::command]
pub fn get_mod_detail_preview_image(
    mod_id: String,
    state: State<'_, AppState>,
) -> Result<Option<PreviewImageDto>, CommandErrorDto> {
    let mod_id = parse_mod_id(mod_id)?;
    let preview_image = state
        .preview_image_detail
        .get_detail_preview_image(&mod_id)
        .map_err(|_| preview_image_detail_unavailable_error())?;

    Ok(preview_image.map(Into::into))
}

#[tauri::command]
pub fn maintain_thumbnail_cache(state: State<'_, AppState>) -> Result<(), CommandErrorDto> {
    state.mod_import_task_runner.maintain_thumbnail_cache_now();
    Ok(())
}

#[tauri::command]
pub fn set_thumbnail_cache_settings(
    thumbnail_cache_max_bytes: Option<u64>,
    thumbnail_cache_max_age_days: Option<u32>,
    state: State<'_, AppState>,
) -> Result<AppSettingsDto, CommandErrorDto> {
    let settings = state
        .app_settings
        .update_thumbnail_cache_settings(thumbnail_cache_max_bytes, thumbnail_cache_max_age_days)
        .map_err(CommandErrorDto::from_app_settings_service_error)?;

    Ok(settings.into())
}

fn spawn_prepare_runner(
    runner: Arc<hmm_app::ModImportTaskRunner>,
    app_handle: AppHandle,
    task_id: String,
    archive_path: PathBuf,
) {
    std::thread::spawn(move || {
        let events = match runner.run_prepare_task(&task_id, archive_path) {
            Ok(events) => events,
            Err(error) => error.events,
        };

        for event in events {
            let _ = emit_task_progress(&app_handle, event.into());
        }
    });
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

fn parse_mod_id(value: String) -> Result<String, CommandErrorDto> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(CommandErrorDto {
            code: "mod_id_empty".to_owned(),
            message: "mod id cannot be empty".to_owned(),
        });
    }

    Ok(trimmed.to_owned())
}

fn parse_candidate_index(value: i64) -> Result<usize, CommandErrorDto> {
    usize::try_from(value).map_err(|_| CommandErrorDto {
        code: "preview_image_candidate_index_invalid".to_owned(),
        message: "preview image candidate index must be zero or greater".to_owned(),
    })
}

fn mod_library_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "mod_library_unavailable".to_owned(),
        message: "mod library is unavailable".to_owned(),
    }
}

fn preview_image_candidates_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "preview_image_candidates_unavailable".to_owned(),
        message: "preview image candidates are unavailable".to_owned(),
    }
}

fn preview_image_selection_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "preview_image_selection_unavailable".to_owned(),
        message: "preview image selection is unavailable".to_owned(),
    }
}

fn preview_image_detail_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "preview_image_detail_unavailable".to_owned(),
        message: "preview image detail preview is unavailable".to_owned(),
    }
}

fn preview_image_diagnostics_export_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "preview_image_diagnostics_export_unavailable".to_owned(),
        message: "preview image diagnostics export is unavailable".to_owned(),
    }
}

fn audit_log_diagnostics_export_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "audit_log_diagnostics_export_unavailable".to_owned(),
        message: "audit log diagnostics export is unavailable".to_owned(),
    }
}

fn support_diagnostics_export_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "support_diagnostics_export_unavailable".to_owned(),
        message: "support diagnostics export is unavailable".to_owned(),
    }
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
    fn parse_mod_id_rejects_empty_values() {
        let error = parse_mod_id("  ".to_owned()).expect_err("empty id rejected");

        assert_eq!(error.code, "mod_id_empty");
    }

    #[test]
    fn parse_mod_id_trims_values() {
        let mod_id = parse_mod_id("  pkg-1  ".to_owned()).expect("id accepted");

        assert_eq!(mod_id, "pkg-1");
    }

    #[test]
    fn parse_candidate_index_rejects_negative_values() {
        let error = parse_candidate_index(-1).expect_err("negative index rejected");

        assert_eq!(error.code, "preview_image_candidate_index_invalid");
    }

    #[test]
    fn mod_library_unavailable_error_uses_stable_code_without_paths() {
        let error = mod_library_unavailable_error();

        assert_eq!(error.code, "mod_library_unavailable");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
    }

    #[test]
    fn preview_image_candidates_unavailable_error_uses_stable_code_without_paths() {
        let error = preview_image_candidates_unavailable_error();

        assert_eq!(error.code, "preview_image_candidates_unavailable");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
    }

    #[test]
    fn preview_image_selection_unavailable_error_uses_stable_code_without_paths() {
        let error = preview_image_selection_unavailable_error();

        assert_eq!(error.code, "preview_image_selection_unavailable");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
    }

    #[test]
    fn preview_image_detail_unavailable_error_uses_stable_code_without_paths() {
        let error = preview_image_detail_unavailable_error();

        assert_eq!(error.code, "preview_image_detail_unavailable");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
    }

    #[test]
    fn preview_image_diagnostics_export_unavailable_error_uses_stable_code_without_paths() {
        let error = preview_image_diagnostics_export_unavailable_error();

        assert_eq!(error.code, "preview_image_diagnostics_export_unavailable");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
    }

    #[test]
    fn audit_log_diagnostics_export_unavailable_error_uses_stable_code_without_paths() {
        let error = audit_log_diagnostics_export_unavailable_error();

        assert_eq!(error.code, "audit_log_diagnostics_export_unavailable");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
    }

    #[test]
    fn support_diagnostics_export_unavailable_error_uses_stable_code_without_paths() {
        let error = support_diagnostics_export_unavailable_error();

        assert_eq!(error.code, "support_diagnostics_export_unavailable");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
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
