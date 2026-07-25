use crate::dto::CommandErrorDto;
use crate::external_import_dto::{
    ExternalImportBatchResultPageDto, ExternalImportBatchStartedDto, ExternalImportPreviewPageDto,
    ExternalImportScanStartedDto, ExternalImportSelectionDto,
    ExternalImportSelectionMutationInputDto, ExternalImportSelectionMutationResultDto,
    ExternalImportSourceDto,
};
use crate::state::AppState;
use crate::task_events::emit_task_progress;
use hmm_app::{
    ExternalImportBatchError, ExternalImportBatchLaunch, ExternalImportBatchService,
    ExternalImportScanError, ExternalImportScanLaunch, ExternalImportScanService, TaskManager,
    TaskProgressEvent, TaskStatus, DEFAULT_EXTERNAL_IMPORT_PREVIEW_LIMIT,
    DEFAULT_EXTERNAL_IMPORT_RESULT_LIMIT, EXTERNAL_IMPORT_BATCH_CANCELLED_PHASE,
    EXTERNAL_IMPORT_BATCH_FAILED_PHASE, EXTERNAL_IMPORT_BATCH_QUEUED_PHASE,
    EXTERNAL_IMPORT_SCAN_CANCELLED_PHASE, EXTERNAL_IMPORT_SCAN_FAILED_PHASE,
    EXTERNAL_IMPORT_SCAN_QUEUED_PHASE, MAX_EXTERNAL_IMPORT_PREVIEW_LIMIT,
    MAX_EXTERNAL_IMPORT_RESULT_LIMIT,
};
use hmm_core::{
    ExternalImportBatchId, ExternalImportCandidateId, ExternalImportSelectionDecision,
    ExternalImportSelectionId, ExternalImportSelectionMutation, ExternalImportSourceId,
    EXTERNAL_IMPORT_SELECTION_MUTATION_MAX_ITEMS,
};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

const EXTERNAL_IMPORT_ID_MAX_LENGTH: usize = 160;

#[tauri::command]
pub async fn select_external_import_source(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<Option<ExternalImportSourceDto>, CommandErrorDto> {
    let source_registry = Arc::clone(&state.external_import.source_registry);
    let Some(root_directory) = pick_external_import_source_directory(&app_handle).await? else {
        return Ok(None);
    };
    let source = source_registry
        .register_directory(root_directory)
        .map_err(|_| external_import_source_unavailable_error())?;

    Ok(Some(source.into()))
}

#[tauri::command]
pub fn start_external_import_scan(
    source_id: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<ExternalImportScanStartedDto, CommandErrorDto> {
    let source_id = ExternalImportSourceId::new(parse_external_import_id(
        source_id,
        "external_import_source_id_invalid",
        "external import source id is invalid",
    )?);
    let launch = state
        .external_import
        .scans
        .start_scan(source_id)
        .map_err(external_import_scan_error)?;
    let response = ExternalImportScanStartedDto::from(&launch);

    if let Err(error) = emit_task_progress(&app_handle, queued_event_for_scan(&launch).into()) {
        let _ = state.external_import.scans.abort_queued_scan(&launch);
        return Err(error);
    }
    spawn_external_import_scan_runner(
        Arc::clone(&state.external_import.scans),
        Arc::clone(&state.task_manager),
        app_handle,
        launch,
    );

    Ok(response)
}

#[tauri::command]
pub fn get_external_import_preview(
    batch_id: String,
    selection_id: Option<String>,
    cursor: Option<String>,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<ExternalImportPreviewPageDto, CommandErrorDto> {
    let batch_id = ExternalImportBatchId::new(parse_external_import_id(
        batch_id,
        "external_import_batch_id_invalid",
        "external import batch id is invalid",
    )?);
    let selection_id = selection_id
        .map(|selection_id| {
            parse_external_import_id(
                selection_id,
                "external_import_selection_id_invalid",
                "external import selection id is invalid",
            )
            .map(ExternalImportSelectionId::new)
        })
        .transpose()?;
    let offset = parse_external_import_cursor(cursor)?;
    let limit = parse_external_import_preview_limit(limit)?;
    let page = state
        .external_import
        .batches
        .get_preview(&batch_id, selection_id.as_ref(), offset, limit)
        .map_err(external_import_batch_error)?;

    Ok(page.into())
}

#[tauri::command]
pub fn create_external_import_selection(
    batch_id: String,
    state: State<'_, AppState>,
) -> Result<ExternalImportSelectionDto, CommandErrorDto> {
    let batch_id = ExternalImportBatchId::new(parse_external_import_id(
        batch_id,
        "external_import_batch_id_invalid",
        "external import batch id is invalid",
    )?);
    state
        .external_import
        .batches
        .create_selection(&batch_id)
        .map(Into::into)
        .map_err(external_import_batch_error)
}

#[tauri::command]
pub fn update_external_import_selection(
    selection_id: String,
    expected_revision: u64,
    entries: Vec<ExternalImportSelectionMutationInputDto>,
    state: State<'_, AppState>,
) -> Result<ExternalImportSelectionMutationResultDto, CommandErrorDto> {
    let selection_id = ExternalImportSelectionId::new(parse_external_import_id(
        selection_id,
        "external_import_selection_id_invalid",
        "external import selection id is invalid",
    )?);
    let mutations = parse_selection_mutations(entries)?;
    state
        .external_import
        .batches
        .update_selection(&selection_id, expected_revision, &mutations)
        .map(Into::into)
        .map_err(external_import_batch_error)
}

#[tauri::command]
pub fn select_all_external_import_candidates(
    selection_id: String,
    expected_revision: u64,
    state: State<'_, AppState>,
) -> Result<ExternalImportSelectionMutationResultDto, CommandErrorDto> {
    let selection_id = ExternalImportSelectionId::new(parse_external_import_id(
        selection_id,
        "external_import_selection_id_invalid",
        "external import selection id is invalid",
    )?);
    state
        .external_import
        .batches
        .select_all_ready(&selection_id, expected_revision)
        .map(Into::into)
        .map_err(external_import_batch_error)
}

#[tauri::command]
pub fn start_external_import_batch(
    batch_id: String,
    selection_id: String,
    expected_revision: u64,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<ExternalImportBatchStartedDto, CommandErrorDto> {
    let batch_id = ExternalImportBatchId::new(parse_external_import_id(
        batch_id,
        "external_import_batch_id_invalid",
        "external import batch id is invalid",
    )?);
    let selection_id = ExternalImportSelectionId::new(parse_external_import_id(
        selection_id,
        "external_import_selection_id_invalid",
        "external import selection id is invalid",
    )?);
    let service = Arc::clone(&state.external_import.batches);
    let launch = service
        .start_import(&batch_id, &selection_id, expected_revision)
        .map_err(external_import_batch_error)?;
    launch_external_import_batch(service, Arc::clone(&state.task_manager), app_handle, launch)
}

#[tauri::command]
pub fn retry_external_import_batch(
    batch_id: String,
    selection_id: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<ExternalImportBatchStartedDto, CommandErrorDto> {
    let batch_id = ExternalImportBatchId::new(parse_external_import_id(
        batch_id,
        "external_import_batch_id_invalid",
        "external import batch id is invalid",
    )?);
    let selection_id = ExternalImportSelectionId::new(parse_external_import_id(
        selection_id,
        "external_import_selection_id_invalid",
        "external import selection id is invalid",
    )?);
    let service = Arc::clone(&state.external_import.batches);
    let launch = service
        .retry_import(&batch_id, &selection_id)
        .map_err(external_import_batch_error)?;
    launch_external_import_batch(service, Arc::clone(&state.task_manager), app_handle, launch)
}

#[tauri::command]
pub fn get_external_import_batch_result(
    batch_id: String,
    cursor: Option<String>,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<ExternalImportBatchResultPageDto, CommandErrorDto> {
    let batch_id = ExternalImportBatchId::new(parse_external_import_id(
        batch_id,
        "external_import_batch_id_invalid",
        "external import batch id is invalid",
    )?);
    let offset = parse_external_import_result_cursor(cursor)?;
    let limit = parse_external_import_result_limit(limit)?;
    state
        .external_import
        .batches
        .get_results(&batch_id, offset, limit)
        .map(Into::into)
        .map_err(external_import_batch_error)
}

async fn pick_external_import_source_directory(
    app_handle: &AppHandle,
) -> Result<Option<PathBuf>, CommandErrorDto> {
    let (sender, mut receiver) = tauri::async_runtime::channel(1);
    app_handle.dialog().file().pick_folder(move |selection| {
        let selected_path = selection
            .map(|file_path| file_path.into_path().map_err(|_| ()))
            .transpose();
        let _ = sender.try_send(selected_path);
    });

    receiver
        .recv()
        .await
        .ok_or_else(external_import_source_picker_unavailable_error)?
        .map_err(|_| external_import_source_picker_unavailable_error())
}

fn spawn_external_import_scan_runner(
    service: Arc<ExternalImportScanService>,
    task_manager: Arc<TaskManager>,
    app_handle: AppHandle,
    launch: ExternalImportScanLaunch,
) {
    std::thread::spawn(move || {
        let task_id = launch.task.task_id.clone();
        let task_kind = launch.task.kind;
        let batch_id = launch.batch_id.as_str().to_owned();
        let events = match service.run_scan(launch) {
            Ok(events) => events,
            Err(error) => vec![fallback_scan_terminal_event(
                &task_manager,
                task_id,
                task_kind,
                batch_id,
                error,
            )],
        };

        for event in events {
            let _ = emit_task_progress(&app_handle, event.into());
        }
    });
}

fn launch_external_import_batch(
    service: Arc<ExternalImportBatchService>,
    task_manager: Arc<TaskManager>,
    app_handle: AppHandle,
    launch: ExternalImportBatchLaunch,
) -> Result<ExternalImportBatchStartedDto, CommandErrorDto> {
    let response = ExternalImportBatchStartedDto::from(&launch);
    if let Err(error) = emit_task_progress(&app_handle, queued_event_for_batch(&launch).into()) {
        let _ = service.abort_queued_import(&launch);
        return Err(error);
    }
    spawn_external_import_batch_runner(service, task_manager, app_handle, launch);
    Ok(response)
}

fn spawn_external_import_batch_runner(
    service: Arc<ExternalImportBatchService>,
    task_manager: Arc<TaskManager>,
    app_handle: AppHandle,
    launch: ExternalImportBatchLaunch,
) {
    std::thread::spawn(move || {
        let task_id = launch.task.task_id.clone();
        let task_kind = launch.task.kind;
        let batch_id = launch.batch_id.as_str().to_owned();
        let events = match service.run_import(launch.clone()) {
            Ok(events) => events,
            Err(error) => match service.recover_unhandled_import_failure(&launch, error) {
                Ok(event) => vec![event],
                Err(_) => vec![fallback_batch_terminal_event(
                    &task_manager,
                    task_id,
                    task_kind,
                    batch_id,
                    error,
                )],
            },
        };

        for event in events {
            let _ = emit_task_progress(&app_handle, event.into());
        }
    });
}

fn queued_event_for_scan(launch: &ExternalImportScanLaunch) -> TaskProgressEvent {
    queued_event_for_task(
        &launch.task,
        launch.batch_id.as_str(),
        EXTERNAL_IMPORT_SCAN_QUEUED_PHASE,
    )
}

fn queued_event_for_batch(launch: &ExternalImportBatchLaunch) -> TaskProgressEvent {
    queued_event_for_task(
        &launch.task,
        launch.batch_id.as_str(),
        EXTERNAL_IMPORT_BATCH_QUEUED_PHASE,
    )
}

fn queued_event_for_task(
    task: &hmm_app::TaskStarted,
    batch_id: &str,
    phase: &'static str,
) -> TaskProgressEvent {
    let mut event = TaskProgressEvent::new(task.task_id.clone(), task.kind, task.status, phase);
    event.result_ref = Some(batch_id.to_owned());
    event
}

fn fallback_batch_terminal_event(
    task_manager: &TaskManager,
    task_id: String,
    task_kind: hmm_app::TaskKind,
    batch_id: String,
    error: ExternalImportBatchError,
) -> TaskProgressEvent {
    if matches!(
        task_manager.task_status(&task_id),
        Some(TaskStatus::Queued | TaskStatus::Running)
    ) {
        let _ = task_manager.fail_task(&task_id);
    }
    let status = task_manager
        .task_status(&task_id)
        .unwrap_or(TaskStatus::Failed);
    let mut event = TaskProgressEvent::new(
        task_id,
        task_kind,
        status,
        if status == TaskStatus::Cancelled {
            EXTERNAL_IMPORT_BATCH_CANCELLED_PHASE
        } else {
            EXTERNAL_IMPORT_BATCH_FAILED_PHASE
        },
    );
    event.result_ref = Some(batch_id);
    if status != TaskStatus::Cancelled {
        event.error = Some(error.error_code().to_owned());
    }
    event
}

fn fallback_scan_terminal_event(
    task_manager: &TaskManager,
    task_id: String,
    task_kind: hmm_app::TaskKind,
    batch_id: String,
    error: ExternalImportScanError,
) -> TaskProgressEvent {
    if matches!(
        task_manager.task_status(&task_id),
        Some(TaskStatus::Queued | TaskStatus::Running)
    ) {
        let _ = task_manager.fail_task(&task_id);
    }
    let status = task_manager
        .task_status(&task_id)
        .unwrap_or(TaskStatus::Failed);
    let mut event = TaskProgressEvent::new(
        task_id,
        task_kind,
        status,
        if status == TaskStatus::Cancelled {
            EXTERNAL_IMPORT_SCAN_CANCELLED_PHASE
        } else {
            EXTERNAL_IMPORT_SCAN_FAILED_PHASE
        },
    );
    event.result_ref = Some(batch_id);
    if status != TaskStatus::Cancelled {
        event.error = Some(error.error_code().to_owned());
    }
    event
}

fn parse_external_import_id(
    value: String,
    code: &'static str,
    message: &'static str,
) -> Result<String, CommandErrorDto> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > EXTERNAL_IMPORT_ID_MAX_LENGTH
        || !trimmed
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(CommandErrorDto {
            code: code.to_owned(),
            message: message.to_owned(),
        });
    }

    Ok(trimmed.to_owned())
}

fn parse_external_import_cursor(value: Option<String>) -> Result<usize, CommandErrorDto> {
    let Some(value) = value else {
        return Ok(0);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(external_import_preview_cursor_invalid_error());
    }

    trimmed
        .parse()
        .map_err(|_| external_import_preview_cursor_invalid_error())
}

fn parse_external_import_preview_limit(value: Option<i64>) -> Result<usize, CommandErrorDto> {
    let limit = value.unwrap_or(DEFAULT_EXTERNAL_IMPORT_PREVIEW_LIMIT as i64);
    if !(1..=MAX_EXTERNAL_IMPORT_PREVIEW_LIMIT as i64).contains(&limit) {
        return Err(external_import_preview_limit_invalid_error());
    }

    usize::try_from(limit).map_err(|_| external_import_preview_limit_invalid_error())
}

fn parse_external_import_result_cursor(value: Option<String>) -> Result<usize, CommandErrorDto> {
    let Some(value) = value else {
        return Ok(0);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(external_import_result_cursor_invalid_error());
    }

    trimmed
        .parse()
        .map_err(|_| external_import_result_cursor_invalid_error())
}

fn parse_external_import_result_limit(value: Option<i64>) -> Result<usize, CommandErrorDto> {
    let limit = value.unwrap_or(DEFAULT_EXTERNAL_IMPORT_RESULT_LIMIT as i64);
    if !(1..=MAX_EXTERNAL_IMPORT_RESULT_LIMIT as i64).contains(&limit) {
        return Err(external_import_result_limit_invalid_error());
    }

    usize::try_from(limit).map_err(|_| external_import_result_limit_invalid_error())
}

fn parse_selection_mutations(
    entries: Vec<ExternalImportSelectionMutationInputDto>,
) -> Result<Vec<ExternalImportSelectionMutation>, CommandErrorDto> {
    if entries.len() > EXTERNAL_IMPORT_SELECTION_MUTATION_MAX_ITEMS {
        return Err(CommandErrorDto {
            code: "selection_mutation_limit_exceeded".to_owned(),
            message: "external import selection mutation limit exceeded".to_owned(),
        });
    }

    entries
        .into_iter()
        .map(|entry| {
            let candidate_id = ExternalImportCandidateId::new(parse_external_import_id(
                entry.candidate_id,
                "external_import_candidate_id_invalid",
                "external import candidate id is invalid",
            )?);
            let decision = entry
                .decision
                .map(|decision| {
                    Ok(ExternalImportSelectionDecision {
                        conflict_resolution: decision.conflict_resolution.map(Into::into),
                        category_id: decision
                            .category_id
                            .map(|category_id| {
                                parse_external_import_id(
                                    category_id,
                                    "external_import_category_id_invalid",
                                    "external import category id is invalid",
                                )
                            })
                            .transpose()?,
                    })
                })
                .transpose()?;
            Ok(ExternalImportSelectionMutation {
                candidate_id,
                selected: entry.selected,
                decision,
            })
        })
        .collect()
}

fn external_import_scan_error(error: ExternalImportScanError) -> CommandErrorDto {
    let message = match error {
        ExternalImportScanError::SourceUnavailable => "external import source is unavailable",
        ExternalImportScanError::TaskUnavailable => "external import task is unavailable",
        ExternalImportScanError::BatchUnavailable => "external import batch is unavailable",
        ExternalImportScanError::ScanFailed => "external import scan failed",
        ExternalImportScanError::PreviewRequestInvalid => {
            "external import preview request is invalid"
        }
        ExternalImportScanError::ClockUnavailable => "external import clock is unavailable",
    };
    CommandErrorDto {
        code: error.error_code().to_owned(),
        message: message.to_owned(),
    }
}

fn external_import_batch_error(error: ExternalImportBatchError) -> CommandErrorDto {
    let message = match error {
        ExternalImportBatchError::SourceUnavailable => "external import source is unavailable",
        ExternalImportBatchError::TaskUnavailable => "external import task is unavailable",
        ExternalImportBatchError::BatchUnavailable => "external import batch is unavailable",
        ExternalImportBatchError::SelectionUnavailable => {
            "external import selection is unavailable"
        }
        ExternalImportBatchError::Selection(_) => "external import selection is invalid",
        ExternalImportBatchError::BatchNotStartable => "external import batch is not startable",
        ExternalImportBatchError::CatalogUnavailable => "external import catalog is unavailable",
        ExternalImportBatchError::CategoryUnavailable => "external import category is unavailable",
        ExternalImportBatchError::PreviewPageInvalid => {
            "external import preview request is invalid"
        }
        ExternalImportBatchError::ResultPageInvalid => "external import result request is invalid",
        ExternalImportBatchError::ClockUnavailable => "external import clock is unavailable",
    };
    CommandErrorDto {
        code: error.error_code().to_owned(),
        message: message.to_owned(),
    }
}

fn external_import_source_picker_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "external_import_source_picker_unavailable".to_owned(),
        message: "external import source picker is unavailable".to_owned(),
    }
}

fn external_import_source_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "external_import_source_unavailable".to_owned(),
        message: "external import source is unavailable".to_owned(),
    }
}

fn external_import_preview_cursor_invalid_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "external_import_preview_cursor_invalid".to_owned(),
        message: "external import preview cursor is invalid".to_owned(),
    }
}

fn external_import_preview_limit_invalid_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "external_import_preview_limit_invalid".to_owned(),
        message: "external import preview limit is invalid".to_owned(),
    }
}

fn external_import_result_cursor_invalid_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "external_import_result_cursor_invalid".to_owned(),
        message: "external import result cursor is invalid".to_owned(),
    }
}

fn external_import_result_limit_invalid_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "external_import_result_limit_invalid".to_owned(),
        message: "external import result limit is invalid".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_app::{TaskKind, TaskStarted};
    use serde_json::Value;

    #[test]
    fn opaque_id_parser_rejects_paths_and_trims_valid_ids() {
        let error = parse_external_import_id(
            "C:/private/source".to_owned(),
            "external_import_source_id_invalid",
            "external import source id is invalid",
        )
        .expect_err("paths are not opaque source ids");
        assert_eq!(error.code, "external_import_source_id_invalid");
        assert!(!error.message.contains(':'));

        let id = parse_external_import_id(
            " external-import-source-123 ".to_owned(),
            "external_import_source_id_invalid",
            "external import source id is invalid",
        )
        .expect("opaque id is accepted");
        assert_eq!(id, "external-import-source-123");
    }

    #[test]
    fn preview_cursor_and_limit_enforce_the_documented_bounds() {
        assert_eq!(
            parse_external_import_cursor(None).expect("default cursor"),
            0
        );
        assert_eq!(
            parse_external_import_cursor(Some("50".to_owned())).expect("numeric cursor"),
            50
        );
        assert_eq!(
            parse_external_import_preview_limit(None).expect("default page size"),
            DEFAULT_EXTERNAL_IMPORT_PREVIEW_LIMIT
        );
        assert_eq!(
            parse_external_import_preview_limit(Some(100)).expect("maximum page size"),
            MAX_EXTERNAL_IMPORT_PREVIEW_LIMIT
        );
        assert_eq!(
            parse_external_import_cursor(Some("../private".to_owned()))
                .expect_err("path-like cursor is rejected")
                .code,
            "external_import_preview_cursor_invalid"
        );
        assert_eq!(
            parse_external_import_preview_limit(Some(101))
                .expect_err("oversized page is rejected")
                .code,
            "external_import_preview_limit_invalid"
        );
    }

    #[test]
    fn result_cursor_and_limit_enforce_the_documented_bounds() {
        assert_eq!(
            parse_external_import_result_cursor(None).expect("default result cursor"),
            0
        );
        assert_eq!(
            parse_external_import_result_cursor(Some("50".to_owned()))
                .expect("numeric result cursor"),
            50
        );
        assert_eq!(
            parse_external_import_result_limit(None).expect("default result page size"),
            DEFAULT_EXTERNAL_IMPORT_RESULT_LIMIT
        );
        assert_eq!(
            parse_external_import_result_limit(Some(100)).expect("maximum result page size"),
            MAX_EXTERNAL_IMPORT_RESULT_LIMIT
        );
        assert_eq!(
            parse_external_import_result_cursor(Some("../private".to_owned()))
                .expect_err("path-like result cursor is rejected")
                .code,
            "external_import_result_cursor_invalid"
        );
        assert_eq!(
            parse_external_import_result_limit(Some(101))
                .expect_err("oversized result page is rejected")
                .code,
            "external_import_result_limit_invalid"
        );
    }

    #[test]
    fn selection_mutation_parser_rejects_more_than_two_hundred_entries() {
        let entries = (0..=EXTERNAL_IMPORT_SELECTION_MUTATION_MAX_ITEMS)
            .map(|index| ExternalImportSelectionMutationInputDto {
                candidate_id: format!("external-import-candidate-{index}"),
                selected: true,
                decision: None,
            })
            .collect();

        assert_eq!(
            parse_selection_mutations(entries)
                .expect_err("oversized mutation is rejected before service dispatch")
                .code,
            "selection_mutation_limit_exceeded"
        );
    }

    #[test]
    fn queued_scan_event_contains_only_task_and_batch_identity() {
        let task = TaskStarted {
            task_id: "mod-import-123".to_owned(),
            kind: TaskKind::ModImport,
            status: TaskStatus::Queued,
        };
        let value: Value = serde_json::to_value(crate::dto::TaskProgressEventDto::from(
            queued_event_for_task(
                &task,
                "external-import-batch-123",
                EXTERNAL_IMPORT_SCAN_QUEUED_PHASE,
            ),
        ))
        .expect("serialize queued event");

        assert_eq!(value["taskId"], "mod-import-123");
        assert_eq!(value["resultRef"], "external-import-batch-123");
        assert_eq!(value["phase"], EXTERNAL_IMPORT_SCAN_QUEUED_PHASE);
        assert!(!value.to_string().contains("source"));
    }

    #[test]
    fn scan_errors_are_stable_and_redacted() {
        let error = external_import_scan_error(ExternalImportScanError::SourceUnavailable);

        assert_eq!(error.code, "external_import_source_unavailable");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
    }
}
