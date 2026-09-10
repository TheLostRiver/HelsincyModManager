use crate::diagnostics_dto::DiagnosticsPageSnapshotDto;
use crate::dto::{
    AppSettingsDto, AuditLogDiagnosticsExportDto, CommandErrorDto, ModDependencyGraphDto,
    ModDetailDto, ModLibraryItemDto, PreviewImageCandidateListDto, PreviewImageDiagnosticsDto,
    PreviewImageDiagnosticsExportDto, PreviewImageDto, SupportDiagnosticsExportDto, TaskStartedDto,
};
use crate::reinstall_dto::StartImportModRevisionTaskRequestDto;
use crate::state::AppState;
use crate::task_events::emit_task_progress;
use hmm_app::{
    StartImportModRevisionTaskRequest, StartImportModTaskRequest, TaskProgressEvent, TaskStarted,
};
use hmm_core::ModId;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};

const MOD_IMPORT_QUEUED_PHASE: &str = "mod_import.queued";
const MAX_DROPPED_ARCHIVES: usize = 100;

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

    emit_task_progress(&app_handle, queued_event_for_started_task(&task))?;
    spawn_prepare_runner(
        Arc::clone(&state.mod_import_task_runner),
        app_handle,
        task.task_id.clone(),
        runner_archive_path,
    );

    Ok(task.into())
}

#[tauri::command]
pub fn start_import_mod_revision_task(
    request: StartImportModRevisionTaskRequestDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let archive_path = parse_archive_path(request.archive_path)?;
    let mod_id = ModId::new(parse_mod_id(request.mod_id)?);
    let runner_archive_path = archive_path.clone();
    let runner_mod_id = mod_id.clone();
    let task = state
        .mod_import_tasks
        .start_import_mod_revision_task(StartImportModRevisionTaskRequest {
            archive_path,
            mod_id,
        })
        .map_err(CommandErrorDto::from_mod_import_task_error)?;

    let _ = emit_task_progress(&app_handle, queued_event_for_started_task(&task));
    spawn_prepare_revision_runner(
        Arc::clone(&state.mod_import_task_runner),
        app_handle,
        task.task_id.clone(),
        runner_archive_path,
        runner_mod_id,
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
pub fn get_mod_dependency_graph(
    state: State<'_, AppState>,
) -> Result<ModDependencyGraphDto, CommandErrorDto> {
    let graph = state
        .mod_dependency_graph
        .get_mod_dependency_graph()
        .map_err(|_| mod_dependency_graph_unavailable_error())?;

    Ok(graph.into())
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
pub fn get_diagnostics_page_snapshot(
    state: State<'_, AppState>,
) -> Result<DiagnosticsPageSnapshotDto, CommandErrorDto> {
    Ok(state.support_diagnostics_export.read_page_snapshot().into())
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
pub fn get_thumbnail_cache_settings(
    state: State<'_, AppState>,
) -> Result<AppSettingsDto, CommandErrorDto> {
    let settings = state
        .app_settings
        .get_settings()
        .map_err(CommandErrorDto::from_app_settings_service_error)?;

    Ok(settings.into())
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
            let _ = emit_task_progress(&app_handle, event);
        }
    });
}

fn spawn_prepare_revision_runner(
    runner: Arc<hmm_app::ModImportTaskRunner>,
    app_handle: AppHandle,
    task_id: String,
    archive_path: PathBuf,
    mod_id: ModId,
) {
    std::thread::spawn(move || {
        let events = match runner.run_prepare_revision_task(&task_id, archive_path, mod_id) {
            Ok(events) => events,
            Err(error) => error.events,
        };

        for event in events {
            let _ = emit_task_progress(&app_handle, event);
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

/// 一次拖拽里单个文件的预检结果（T22，#366）。
///
/// `errorCode` 为 `null` 表示可导入；否则是**与导入失败同一套**的语义码
/// （`mod_import_unsupported_archive_format` / `mod_import_not_an_archive` /
/// `mod_import_archive_encrypted` / `mod_import_archive_multi_volume` /
/// `mod_import_prepare_failed`）。前端因此不必维护第二张映射表。
///
/// **只投影语义码，不带任何底层错误文本**——脱敏口径与既有失败事件一致。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DroppedArchivePreviewDto {
    /// 原样回传，供前端与自己那份列表对齐。
    pub archive_path: String,
    /// 展示用的文件名。后端算好，免得前端各写一份跨平台的路径切分。
    pub file_name: String,
    /// 文件字节数。`None` = 读不到（文件已被移走、权限不足等）。
    ///
    /// 读不到**不构成不可导入**：真正的判据是能不能打开归档，那由 `error_code` 说了算。
    /// 这里只是给玩家一个「我拖的是不是那个包」的旁证，所以缺了就不显示。
    pub size_bytes: Option<u64>,
    pub error_code: Option<String>,
    /// 内容层警示码。**与 `errorCode` 不是一类**：这条只警示，玩家可以覆盖。
    ///
    /// `errorCode` 非空时恒为 `None`——读都读不了的包，谈不上「里面有没有内容目录」。
    pub warning_code: Option<String>,
}

/// 拖拽进来的文件逐个预检。**只读归档头，不解包、不写任何东西。**
///
/// 逐个独立判定：一个文件坏了不影响其余的结论——这正是「清单」要表达的东西。
#[tauri::command]
pub async fn preview_dropped_mod_archives(
    archive_paths: Vec<String>,
) -> Result<Vec<DroppedArchivePreviewDto>, CommandErrorDto> {
    dispatch_archive_preview(archive_paths, probe_dropped_archives).await
}

async fn dispatch_archive_preview<F>(
    archive_paths: Vec<String>,
    probe: F,
) -> Result<Vec<DroppedArchivePreviewDto>, CommandErrorDto>
where
    F: FnOnce(Vec<PathBuf>) -> Vec<DroppedArchivePreviewDto> + Send + 'static,
{
    if archive_paths.len() > MAX_DROPPED_ARCHIVES {
        return Err(CommandErrorDto {
            code: "mod_import_preview_limit_exceeded".to_owned(),
            message: "too many archives in one preview request".to_owned(),
        });
    }
    let paths = archive_paths
        .into_iter()
        .map(parse_archive_path)
        .collect::<Result<Vec<_>, _>>()?;
    // Archive I/O and the RAR session lock must never block the WebView callback.
    tauri::async_runtime::spawn_blocking(move || probe(paths))
        .await
        .map_err(|_| CommandErrorDto {
            code: "mod_import_prepare_failed".to_owned(),
            message: "archive preview is unavailable".to_owned(),
        })
}

fn probe_dropped_archives(archive_paths: Vec<PathBuf>) -> Vec<DroppedArchivePreviewDto> {
    archive_paths
        .into_iter()
        .map(|archive_path| {
            let file_name = archive_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            // 大小失败不升级成命令失败，也不改判可导入性：见 `size_bytes` 的说明。
            let size_bytes = std::fs::metadata(&archive_path).ok().map(|meta| meta.len());
            let (error_code, warning_code) = match hmm_infra::probe_mod_archive(&archive_path) {
                // 读得了，但按目录结构看装不出东西：只警示，不否决。
                Ok(probe) if !probe.declares_game_content_root => (
                    None,
                    Some(hmm_ports::MOD_IMPORT_ARCHIVE_NO_GAME_CONTENT_CODE.to_owned()),
                ),
                Ok(_) => (None, None),
                Err(error) => (Some(error.code().to_owned()), None),
            };
            DroppedArchivePreviewDto {
                archive_path: archive_path.to_string_lossy().into_owned(),
                file_name,
                size_bytes,
                error_code,
                warning_code,
            }
        })
        .collect()
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

fn mod_dependency_graph_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "mod_dependency_graph_unavailable".to_owned(),
        message: "mod dependency graph is unavailable".to_owned(),
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
    fn archive_preview_runs_on_a_blocking_worker() {
        let caller = std::thread::current().id();
        let result =
            tauri::async_runtime::block_on(dispatch_archive_preview(Vec::new(), move |_| {
                assert_ne!(std::thread::current().id(), caller);
                Vec::new()
            }));
        assert!(result.expect("worker completes").is_empty());
    }

    #[test]
    fn archive_preview_rejects_excess_paths_before_work() {
        let result = tauri::async_runtime::block_on(dispatch_archive_preview(
            vec![String::new(); MAX_DROPPED_ARCHIVES + 1],
            |_| panic!("oversized input must not reach the worker"),
        ));
        assert_eq!(
            result.expect_err("reject oversized request").code,
            "mod_import_preview_limit_exceeded"
        );
    }

    #[test]
    fn archive_preview_validates_all_paths_before_work() {
        let result = tauri::async_runtime::block_on(dispatch_archive_preview(
            vec!["relative.zip".to_owned()],
            |_| panic!("invalid paths must not reach the worker"),
        ));
        assert_eq!(
            result.expect_err("reject relative path").code,
            "archive_path_not_absolute"
        );
    }

    #[test]
    fn archive_preview_worker_failure_has_a_stable_error() {
        let result = tauri::async_runtime::block_on(dispatch_archive_preview(Vec::new(), |_| {
            panic!("synthetic worker failure")
        }));
        let error = result.expect_err("worker panic is reported");
        assert_eq!(error.code, "mod_import_prepare_failed");
        assert!(!error.message.contains("synthetic"));
    }

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
    fn mod_dependency_graph_unavailable_error_uses_stable_code_without_paths() {
        let error = mod_dependency_graph_unavailable_error();

        assert_eq!(error.code, "mod_dependency_graph_unavailable");
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
