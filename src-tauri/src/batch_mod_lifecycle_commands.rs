use crate::batch_mod_lifecycle_dto::{
    BatchModLifecycleActionSummaryDto, BatchModLifecycleCapabilityDto,
    BatchModLifecycleItemInputDto, BatchModLifecycleLayerDto, BatchModLifecycleOperationDto,
    BatchModLifecyclePreviewDto, BatchModLifecyclePreviewStatusDto,
    BatchModLifecycleReasonSummaryDto, BatchModLifecycleRequestDto, BatchModLifecycleResultItemDto,
    BatchModLifecycleResultPageDto, BatchModLifecycleResultSummaryDto, BatchModLifecycleSealDto,
    BatchModLifecycleSealStatusDto, BatchModLifecycleStartedDto,
};
use crate::dto::CommandErrorDto;
use crate::state::AppState;
use crate::task_events::emit_task_progress;
use hmm_app::{TaskKind, TaskProgressEvent, TaskStarted, TaskStatus};
use hmm_core::{
    BatchItemInput, BatchOperation, BatchPlanRequest, BatchResultSummary, FileLayer, ModId,
    ModRevisionId, ProfileId, ReplacementTargetId, DEFAULT_BATCH_MAX_ITEMS,
};
use hmm_runtime::{SandboxBatchInstallAutomation, SandboxBatchPlanRequest};
use std::collections::BTreeMap;
use tauri::{AppHandle, State};

pub const BATCH_MOD_LIFECYCLE_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_BATCH_MOD_LIFECYCLE_RESULT_LIMIT: usize = 50;
pub const MAX_BATCH_MOD_LIFECYCLE_RESULT_LIMIT: usize = 100;
const BATCH_OPAQUE_ID_MAX_LENGTH: usize = 160;
const BATCH_TARGET_ID_MAX_LENGTH: usize = 256;

#[tauri::command]
pub fn get_batch_mod_lifecycle_capability(
    state: State<'_, AppState>,
) -> BatchModLifecycleCapabilityDto {
    project_batch_capability(state.batch_sandbox_environment())
}

fn project_batch_capability(
    environment: Option<&hmm_runtime::RuntimeEnvironment>,
) -> BatchModLifecycleCapabilityDto {
    match environment {
        Some(_) => BatchModLifecycleCapabilityDto {
            preview_available: true,
            write_available: true,
            unavailable_reason_code: None,
        },
        None => BatchModLifecycleCapabilityDto {
            preview_available: false,
            write_available: false,
            unavailable_reason_code: Some("sandbox_batch_production_forbidden".to_owned()),
        },
    }
}

#[tauri::command]
pub fn preview_batch_mod_lifecycle(
    request: BatchModLifecycleRequestDto,
    state: State<'_, AppState>,
) -> Result<BatchModLifecyclePreviewDto, CommandErrorDto> {
    let environment = state
        .batch_sandbox_environment()
        .ok_or_else(batch_sandbox_unavailable_error)?;
    let request = parse_batch_plan_request(request)?;
    let preview = SandboxBatchInstallAutomation::preview_request(environment, request)
        .map_err(batch_automation_error)?;
    Ok(project_preview(preview))
}

#[tauri::command]
pub fn seal_batch_mod_lifecycle(
    request: BatchModLifecycleRequestDto,
    preview_token: String,
    state: State<'_, AppState>,
) -> Result<BatchModLifecycleSealDto, CommandErrorDto> {
    let environment = state
        .batch_sandbox_environment()
        .ok_or_else(batch_sandbox_unavailable_error)?;
    let request = parse_batch_plan_request(request)?;
    let sealed = match state.batch_sandbox_database() {
        Some(database) => SandboxBatchInstallAutomation::seal_request_with_database(
            environment,
            request,
            &preview_token,
            database,
        ),
        None => SandboxBatchInstallAutomation::seal_request(environment, request, &preview_token),
    }
    .map_err(batch_automation_error)?
    .1;
    let expires_at_unix_millis =
        u64::try_from(sealed.expires_at_unix_millis).map_err(|_| batch_internal_error())?;
    Ok(BatchModLifecycleSealDto {
        batch_id: sealed.batch_id,
        status: BatchModLifecycleSealStatusDto::Sealed,
        operation: sealed.operation,
        execution_policy: sealed.execution_policy,
        expires_at_unix_millis,
        plan_token: sealed.plan_token,
    })
}

#[tauri::command]
pub async fn start_batch_mod_lifecycle(
    batch_id: String,
    plan_token: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<BatchModLifecycleStartedDto, CommandErrorDto> {
    let environment = state
        .batch_sandbox_environment()
        .ok_or_else(batch_sandbox_unavailable_error)?
        .clone();
    let database = state.batch_sandbox_database();
    let (operation, run) = tauri::async_runtime::spawn_blocking(move || match database {
        Some(database) => SandboxBatchInstallAutomation::start_request_with_database(
            &environment,
            &batch_id,
            &plan_token,
            database,
        ),
        None => SandboxBatchInstallAutomation::start_request(&environment, &batch_id, &plan_token),
    })
    .await
    .map_err(|_| batch_internal_error())?
    .map_err(batch_automation_error)?;
    let _ = emit_task_progress(&app_handle, batch_terminal_event(operation, &run));
    Ok(project_started(run))
}

#[tauri::command]
pub fn get_batch_mod_lifecycle_result(
    batch_id: String,
    attempt_number: u32,
    cursor: Option<String>,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<BatchModLifecycleResultPageDto, CommandErrorDto> {
    let environment = state
        .batch_sandbox_environment()
        .ok_or_else(batch_sandbox_unavailable_error)?;
    let offset = parse_result_cursor(cursor)?;
    let limit = parse_result_limit(limit)?;
    let snapshot = match state.batch_sandbox_database() {
        Some(database) => SandboxBatchInstallAutomation::result_with_database(
            environment,
            &batch_id,
            attempt_number,
            database,
        ),
        None => SandboxBatchInstallAutomation::result(environment, &batch_id, attempt_number),
    }
    .map_err(batch_automation_error)?;
    Ok(project_result_page(snapshot, offset, limit))
}

#[tauri::command]
pub async fn retry_batch_mod_lifecycle(
    batch_id: String,
    expected_attempt_number: u32,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<BatchModLifecycleStartedDto, CommandErrorDto> {
    let environment = state
        .batch_sandbox_environment()
        .ok_or_else(batch_sandbox_unavailable_error)?
        .clone();
    let database = state.batch_sandbox_database();
    let (operation, _retry, run) = tauri::async_runtime::spawn_blocking(move || match database {
        Some(database) => SandboxBatchInstallAutomation::retry_with_operation_with_database(
            &environment,
            &batch_id,
            expected_attempt_number,
            database,
        ),
        None => SandboxBatchInstallAutomation::retry_with_operation(
            &environment,
            &batch_id,
            expected_attempt_number,
        ),
    })
    .await
    .map_err(|_| batch_internal_error())?
    .map_err(batch_automation_error)?;
    let _ = emit_task_progress(&app_handle, batch_terminal_event(operation, &run));
    Ok(project_started(run))
}

// ===== Input conversion =====

fn parse_batch_plan_request(
    request: BatchModLifecycleRequestDto,
) -> Result<SandboxBatchPlanRequest, CommandErrorDto> {
    if request.schema_version != BATCH_MOD_LIFECYCLE_SCHEMA_VERSION {
        return Err(batch_input_invalid_error());
    }
    if request.items.is_empty() {
        return Err(batch_input_invalid_error());
    }
    if request.items.len() > DEFAULT_BATCH_MAX_ITEMS {
        return Err(batch_input_invalid_error());
    }
    let operation = BatchOperation::from(request.operation);
    let game_id =
        hmm_core::GameId::parse(&request.game_id).map_err(|_| batch_input_invalid_error())?;
    let profile_id = ProfileId::new(parse_opaque_id(&request.profile_id)?);
    let mut replacement_targets = BTreeMap::new();
    for target in &request.replacement_targets {
        if operation != BatchOperation::Reinstall {
            return Err(batch_input_invalid_error());
        }
        let mod_id = ModId::new(parse_opaque_id(&target.mod_id)?);
        let target_id = parse_replacement_target_id(&target.target_id)?;
        if replacement_targets.insert(mod_id, target_id).is_some() {
            return Err(batch_input_invalid_error());
        }
    }
    let mut items = Vec::with_capacity(request.items.len());
    for item in request.items {
        if item.operation() != request_operation_for(operation) {
            return Err(batch_input_invalid_error());
        }
        items.push(match item {
            BatchModLifecycleItemInputDto::Install {
                mod_id,
                revision_id,
                layer,
            } => BatchItemInput::Install(hmm_core::InstallBatchItemInput {
                mod_id: ModId::new(parse_opaque_id(&mod_id)?),
                revision_id: ModRevisionId::new(parse_opaque_id(&revision_id)?),
                layer: file_layer(layer)?,
                replacement_binding_snapshot: None,
            }),
            BatchModLifecycleItemInputDto::Uninstall {
                mod_id,
                expected_installed_revision_id,
            } => BatchItemInput::Uninstall(hmm_core::UninstallBatchItemInput {
                mod_id: ModId::new(parse_opaque_id(&mod_id)?),
                expected_installed_revision_id: ModRevisionId::new(parse_opaque_id(
                    &expected_installed_revision_id,
                )?),
            }),
            BatchModLifecycleItemInputDto::Reinstall {
                mod_id,
                installed_revision_id,
                candidate_revision_id,
                layer,
            } => BatchItemInput::Reinstall(hmm_core::ReinstallBatchItemInput {
                mod_id: ModId::new(parse_opaque_id(&mod_id)?),
                installed_revision_id: ModRevisionId::new(parse_opaque_id(&installed_revision_id)?),
                candidate_revision_id: ModRevisionId::new(parse_opaque_id(&candidate_revision_id)?),
                layer: file_layer(layer)?,
                replacement_binding_snapshot: None,
            }),
        });
    }
    Ok(SandboxBatchPlanRequest {
        plan: BatchPlanRequest {
            schema_version: BATCH_MOD_LIFECYCLE_SCHEMA_VERSION,
            operation,
            game_id,
            profile_id,
            execution_policy: request.execution_policy.into(),
            items,
        },
        replacement_targets,
    })
}

fn request_operation_for(operation: BatchOperation) -> BatchModLifecycleOperationDto {
    match operation {
        BatchOperation::Install => BatchModLifecycleOperationDto::Install,
        BatchOperation::Uninstall => BatchModLifecycleOperationDto::Uninstall,
        BatchOperation::Reinstall => BatchModLifecycleOperationDto::Reinstall,
    }
}

fn file_layer(layer: BatchModLifecycleLayerDto) -> Result<FileLayer, CommandErrorDto> {
    let name = layer.name.trim();
    if name.is_empty() || name.len() > BATCH_OPAQUE_ID_MAX_LENGTH {
        return Err(batch_input_invalid_error());
    }
    Ok(FileLayer::new(name, layer.priority))
}

fn parse_opaque_id(value: &str) -> Result<String, CommandErrorDto> {
    // Explicit transport contract shared with the CLI batch parser
    // (`parse_batch_id_component`): controlled ids are ASCII alphanumeric plus `-`/`_`. The
    // backend accepts nothing broader; non-ASCII ids and path-like values are rejected here.
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > BATCH_OPAQUE_ID_MAX_LENGTH
        || !trimmed
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(batch_input_invalid_error());
    }
    Ok(trimmed.to_owned())
}

fn parse_replacement_target_id(value: &str) -> Result<ReplacementTargetId, CommandErrorDto> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > BATCH_TARGET_ID_MAX_LENGTH
        || !trimmed.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, ':' | '-' | '_' | '.')
        })
    {
        return Err(batch_input_invalid_error());
    }
    ReplacementTargetId::parse(trimmed.to_owned()).map_err(|_| batch_input_invalid_error())
}

// ===== Pagination =====

fn parse_result_cursor(value: Option<String>) -> Result<usize, CommandErrorDto> {
    let Some(value) = value else {
        return Ok(0);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(batch_input_invalid_error());
    }
    trimmed.parse().map_err(|_| batch_input_invalid_error())
}

fn parse_result_limit(value: Option<i64>) -> Result<usize, CommandErrorDto> {
    let limit = value.unwrap_or(DEFAULT_BATCH_MOD_LIFECYCLE_RESULT_LIMIT as i64);
    if !(1..=MAX_BATCH_MOD_LIFECYCLE_RESULT_LIMIT as i64).contains(&limit) {
        return Err(batch_input_invalid_error());
    }
    usize::try_from(limit).map_err(|_| batch_input_invalid_error())
}

// ===== Projection =====

fn project_preview(preview: hmm_app::BatchPlanPreview) -> BatchModLifecyclePreviewDto {
    let plan = &preview.plan;
    let ready_item_count = plan.items.iter().filter(|item| item.is_ready()).count();
    let blocked_item_count = plan.items.len().saturating_sub(ready_item_count);
    let mut action_summary = BatchModLifecycleActionSummaryDto {
        actions: 0,
        retained: 0,
        replaced: 0,
        added: 0,
        stale: 0,
    };
    let item_reason_codes = plan
        .items
        .iter()
        .flat_map(|item| item.blocking_reasons.iter().cloned());
    for item in &plan.items {
        action_summary.actions += item.action_summary.actions;
        action_summary.retained += item.action_summary.retained;
        action_summary.replaced += item.action_summary.replaced;
        action_summary.added += item.action_summary.added;
        action_summary.stale += item.action_summary.stale;
    }
    BatchModLifecyclePreviewDto {
        status: if plan.is_ready() {
            BatchModLifecyclePreviewStatusDto::Ready
        } else {
            BatchModLifecyclePreviewStatusDto::Blocked
        },
        operation: plan.operation,
        execution_policy: plan.execution_policy,
        item_reasons: aggregate_reasons(item_reason_codes),
        global_reasons: plan
            .global_blocking_reasons
            .iter()
            .map(|reason| BatchModLifecycleReasonSummaryDto {
                code: reason.code.clone(),
                count: reason.count,
            })
            .collect(),
        action_summary,
        ready_item_count,
        blocked_item_count,
        preview_token: preview.preview_token,
    }
}

fn aggregate_reasons(
    codes: impl Iterator<Item = String>,
) -> Vec<BatchModLifecycleReasonSummaryDto> {
    let mut counts = BTreeMap::<String, usize>::new();
    for code in codes {
        *counts.entry(code).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(code, count)| BatchModLifecycleReasonSummaryDto { code, count })
        .collect()
}

fn project_started(run: hmm_app::BatchInstallRunResult) -> BatchModLifecycleStartedDto {
    let (status, _) = batch_terminal_mapping(run.status);
    BatchModLifecycleStartedDto {
        task: TaskStarted {
            task_id: run.task_id,
            kind: TaskKind::Install,
            status,
        }
        .into(),
        batch_id: run.batch_id.as_str().to_owned(),
        attempt_number: run.attempt_number,
    }
}

fn batch_terminal_event(
    operation: BatchOperation,
    run: &hmm_app::BatchInstallRunResult,
) -> TaskProgressEvent {
    let (status, terminal) = batch_terminal_mapping(run.status);
    let mut event = TaskProgressEvent::new(
        run.task_id.clone(),
        TaskKind::Install,
        status,
        format!("install.batch.{}.{}", operation.as_str(), terminal),
    );
    event.result_ref = Some(run.batch_id.as_str().to_owned());
    event
}

fn batch_terminal_mapping(status: hmm_core::BatchAttemptStatus) -> (TaskStatus, &'static str) {
    use hmm_core::BatchAttemptStatus;
    // The current execution model is synchronous: `start`/`retry` return only after the attempt
    // is terminal, so the non-terminal arms below are defensive and unreachable today. They map
    // to `failed` so an unexpected non-terminal result can never be presented as success; when
    // T13-07 introduces asynchronous execution, queued/running attempts must emit their own
    // progress events instead of a terminal mapping.
    match status {
        BatchAttemptStatus::Completed => (TaskStatus::Completed, "completed"),
        BatchAttemptStatus::CompletedWithErrors => (TaskStatus::Completed, "completed_with_errors"),
        BatchAttemptStatus::Blocked => (TaskStatus::Failed, "failed"),
        BatchAttemptStatus::Cancelled => (TaskStatus::Cancelled, "cancelled"),
        BatchAttemptStatus::RecoveryRequired | BatchAttemptStatus::Interrupted => {
            (TaskStatus::Failed, "recovery_required")
        }
        BatchAttemptStatus::Failed => (TaskStatus::Failed, "failed"),
        BatchAttemptStatus::Sealed
        | BatchAttemptStatus::Queued
        | BatchAttemptStatus::Running
        | BatchAttemptStatus::Stopping => (TaskStatus::Failed, "failed"),
    }
}

fn project_result_page(
    snapshot: hmm_runtime::BatchAttemptSnapshot,
    offset: usize,
    limit: usize,
) -> BatchModLifecycleResultPageDto {
    let mut items = snapshot
        .items
        .iter()
        .map(project_result_item)
        .collect::<Vec<_>>();
    items.sort_by_key(|item| item.ordinal);
    let total = items.len();
    let page = items
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    let next_cursor = (offset + page.len() < total).then(|| (offset + page.len()).to_string());
    BatchModLifecycleResultPageDto {
        batch_id: snapshot.batch_id,
        attempt_number: snapshot.attempt_number,
        status: snapshot.status,
        task_id: snapshot.task_id,
        evidence_health_degraded: snapshot.evidence_health_degraded,
        summary: project_result_summary(&snapshot.summary),
        items: page,
        next_cursor,
    }
}

fn project_result_summary(summary: &BatchResultSummary) -> BatchModLifecycleResultSummaryDto {
    BatchModLifecycleResultSummaryDto {
        item_count: summary.item_count,
        succeeded_count: summary.succeeded_count,
        blocked_count: summary.blocked_count,
        failed_count: summary.failed_count,
        cancelled_count: summary.cancelled_count,
        skipped_count: summary.skipped_count,
        recovery_required_count: summary.recovery_required_count,
    }
}

fn project_result_item(result: &hmm_core::BatchItemResult) -> BatchModLifecycleResultItemDto {
    BatchModLifecycleResultItemDto {
        item_id: result.item_id.as_str().to_owned(),
        ordinal: result.ordinal,
        mod_id: result.mod_id.as_str().to_owned(),
        status: result.status,
        reason_code: result.reason_code.clone(),
        retryable: result.retryable,
    }
}

// ===== Errors =====

fn batch_sandbox_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "sandbox_batch_production_forbidden".to_owned(),
        message: "batch mod lifecycle requires a sandbox environment".to_owned(),
    }
}

fn batch_input_invalid_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "batch_input_invalid".to_owned(),
        message: "batch mod lifecycle request is invalid".to_owned(),
    }
}

fn batch_internal_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "batch_internal_error".to_owned(),
        message: "batch mod lifecycle operation failed".to_owned(),
    }
}

fn batch_automation_error(error: hmm_runtime::SandboxBatchAutomationError) -> CommandErrorDto {
    CommandErrorDto {
        code: error.code().to_owned(),
        message: batch_error_message(error.code()),
    }
}

fn batch_error_message(code: &str) -> String {
    let message = match code {
        "sandbox_batch_production_forbidden" => {
            "batch mod lifecycle requires a sandbox environment"
        }
        "batch_input_invalid" => "batch mod lifecycle request is invalid",
        "batch_duplicate_item" => "batch contains a duplicate mod",
        "batch_resource_limit_exceeded" => "batch resource limit exceeded",
        "batch_global_target_conflict" => "batch targets conflict across items",
        "batch_plan_blocked" => "batch plan is blocked",
        "batch_plan_stale" => "batch plan is stale",
        "batch_plan_expired" => "batch plan token expired",
        "batch_token_invalid" => "batch token is invalid",
        "batch_retry_unavailable" => "batch retry is unavailable",
        "batch_attempt_stale" => "batch attempt is stale",
        "batch_id_invalid" => "batch id is invalid",
        "batch_operation_mismatch" => "batch operation mismatch",
        "batch_admission_rejected" => "batch admission was rejected",
        "batch_attempt_reconciliation_required" => "batch scope requires reconciliation",
        "sandbox_data_dir_required" => "sandbox data directory is required",
        "batch_token_unavailable" => "batch token service is unavailable",
        "batch_unavailable" => "batch is unavailable",
        "batch_journal_unavailable" => "batch journal is unavailable",
        "batch_result_unavailable" => "batch result is unavailable",
        "batch_task_unavailable" => "batch task is unavailable",
        "batch_evidence_unavailable" => "batch evidence is unavailable",
        "batch_write_admission_unavailable" => "batch write admission is unavailable",
        "batch_runtime_unavailable" => "batch runtime is unavailable",
        _ => "batch mod lifecycle operation failed",
    };
    message.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_request_dto() -> BatchModLifecycleRequestDto {
        serde_json::from_value(json!({
            "schemaVersion": 1,
            "operation": "install",
            "gameId": "mhw",
            "profileId": "default",
            "executionPolicy": "stop_on_failure",
            "items": [{
                "operation": "install",
                "modId": "mod-a",
                "revisionId": "rev-1",
                "layer": { "name": "base", "priority": 0 }
            }]
        }))
        .expect("sample request deserializes")
    }

    #[test]
    fn request_parser_accepts_well_formed_install_request() {
        let request =
            parse_batch_plan_request(sample_request_dto()).expect("valid request is accepted");
        assert_eq!(request.plan.operation, BatchOperation::Install);
        assert_eq!(request.plan.schema_version, 1);
        assert_eq!(request.plan.items.len(), 1);
        assert!(request.replacement_targets.is_empty());
    }

    #[test]
    fn request_parser_rejects_unknown_schema_version() {
        let mut request = sample_request_dto();
        request.schema_version = 2;
        let error = parse_batch_plan_request(request).expect_err("unknown schema version");
        assert_eq!(error.code, "batch_input_invalid");
    }

    #[test]
    fn request_parser_rejects_empty_items_and_oversized_batches() {
        let mut request = sample_request_dto();
        request.items.clear();
        assert_eq!(
            parse_batch_plan_request(request)
                .expect_err("empty items rejected")
                .code,
            "batch_input_invalid"
        );

        let mut request = sample_request_dto();
        request.items = (0..=DEFAULT_BATCH_MAX_ITEMS)
            .map(|index| BatchModLifecycleItemInputDto::Install {
                mod_id: format!("mod-{index}"),
                revision_id: "rev-1".to_owned(),
                layer: BatchModLifecycleLayerDto {
                    name: "base".to_owned(),
                    priority: 0,
                },
            })
            .collect();
        assert_eq!(
            parse_batch_plan_request(request)
                .expect_err("oversized batch rejected")
                .code,
            "batch_input_invalid"
        );
    }

    #[test]
    fn request_parser_rejects_operation_tag_mismatch() {
        let mut request = sample_request_dto();
        request.items = vec![BatchModLifecycleItemInputDto::Uninstall {
            mod_id: "mod-a".to_owned(),
            expected_installed_revision_id: "rev-1".to_owned(),
        }];
        let error = parse_batch_plan_request(request).expect_err("tag mismatch rejected");
        assert_eq!(error.code, "batch_input_invalid");
    }

    #[test]
    fn request_parser_rejects_path_like_identifiers() {
        let mut request = sample_request_dto();
        request.items = vec![BatchModLifecycleItemInputDto::Install {
            mod_id: "C:/private/mod".to_owned(),
            revision_id: "rev-1".to_owned(),
            layer: BatchModLifecycleLayerDto {
                name: "base".to_owned(),
                priority: 0,
            },
        }];
        let error = parse_batch_plan_request(request).expect_err("path-like id rejected");
        assert_eq!(error.code, "batch_input_invalid");
    }

    #[test]
    fn request_parser_rejects_replacement_targets_outside_reinstall() {
        let mut request = sample_request_dto();
        request.replacement_targets = vec![
            crate::batch_mod_lifecycle_dto::BatchModLifecycleReplacementTargetDto {
                mod_id: "mod-a".to_owned(),
                target_id: "target-1".to_owned(),
            },
        ];
        let error = parse_batch_plan_request(request).expect_err("targets outside reinstall");
        assert_eq!(error.code, "batch_input_invalid");
    }

    #[test]
    fn result_cursor_and_limit_enforce_documented_bounds() {
        assert_eq!(parse_result_cursor(None).expect("default cursor"), 0);
        assert_eq!(
            parse_result_cursor(Some("25".to_owned())).expect("numeric cursor"),
            25
        );
        assert_eq!(
            parse_result_limit(None).expect("default page size"),
            DEFAULT_BATCH_MOD_LIFECYCLE_RESULT_LIMIT
        );
        assert_eq!(
            parse_result_limit(Some(100)).expect("maximum page size"),
            MAX_BATCH_MOD_LIFECYCLE_RESULT_LIMIT
        );
        assert_eq!(
            parse_result_cursor(Some("../private".to_owned()))
                .expect_err("path-like cursor rejected")
                .code,
            "batch_input_invalid"
        );
        assert_eq!(
            parse_result_limit(Some(101))
                .expect_err("oversized page rejected")
                .code,
            "batch_input_invalid"
        );
    }

    #[test]
    fn result_page_slices_by_offset_and_limit_and_emits_next_cursor() {
        let snapshot = hmm_runtime::BatchAttemptSnapshot {
            batch_id: "batch-1".to_owned(),
            operation: BatchOperation::Install,
            attempt_number: 0,
            status: hmm_core::BatchAttemptStatus::Completed,
            task_id: Some("task-1".to_owned()),
            evidence_health_degraded: false,
            summary: BatchResultSummary {
                item_count: 3,
                ..BatchResultSummary::default()
            },
            items: (0..3)
                .map(|ordinal| hmm_core::BatchItemResult {
                    batch_id: hmm_core::BatchId::new("batch-1"),
                    attempt_number: 0,
                    item_id: hmm_core::BatchItemId::new(format!("item-{ordinal}")),
                    ordinal,
                    mod_id: ModId::new(format!("mod-{ordinal}")),
                    status: hmm_core::BatchItemStatus::Succeeded,
                    reason_code: None,
                    retryable: false,
                })
                .collect(),
        };

        let first = project_result_page(snapshot.clone(), 0, 2);
        assert_eq!(first.items.len(), 2);
        assert_eq!(first.items[0].item_id, "item-0");
        assert_eq!(first.next_cursor.as_deref(), Some("2"));
        assert_eq!(first.summary.item_count, 3);

        let second = project_result_page(snapshot, 2, 2);
        assert_eq!(second.items.len(), 1);
        assert_eq!(second.items[0].item_id, "item-2");
        assert!(second.next_cursor.is_none());
    }

    #[test]
    fn terminal_event_carries_task_id_phase_and_batch_ref() {
        let run = hmm_app::BatchInstallRunResult {
            task_id: "batch-task-1".to_owned(),
            batch_id: hmm_core::BatchId::new("batch-1"),
            attempt_number: 0,
            status: hmm_core::BatchAttemptStatus::CompletedWithErrors,
            summary: BatchResultSummary::default(),
        };
        let event = batch_terminal_event(BatchOperation::Install, &run);
        assert_eq!(event.task_id, "batch-task-1");
        assert_eq!(event.kind, TaskKind::Install);
        assert_eq!(event.status, TaskStatus::Completed);
        assert_eq!(event.phase, "install.batch.install.completed_with_errors");
        assert_eq!(event.result_ref.as_deref(), Some("batch-1"));
    }

    #[test]
    fn terminal_event_uses_operation_specific_phase() {
        let run = hmm_app::BatchInstallRunResult {
            task_id: "batch-task-2".to_owned(),
            batch_id: hmm_core::BatchId::new("batch-2"),
            attempt_number: 1,
            status: hmm_core::BatchAttemptStatus::Cancelled,
            summary: BatchResultSummary::default(),
        };
        let event = batch_terminal_event(BatchOperation::Uninstall, &run);
        assert_eq!(event.phase, "install.batch.uninstall.cancelled");
        assert_eq!(event.status, TaskStatus::Cancelled);
    }

    #[test]
    fn error_mapping_uses_stable_codes_and_redacted_messages() {
        let stale = batch_error_message("batch_plan_stale");
        assert!(!stale.contains(':'));
        assert!(!stale.contains('\\'));
        assert!(batch_error_message("batch_plan_stale") == batch_error_message("batch_plan_stale"));

        let forbidden = batch_sandbox_unavailable_error();
        assert_eq!(forbidden.code, "sandbox_batch_production_forbidden");
        assert!(!forbidden.message.contains(':'));
    }

    #[test]
    fn automation_error_maps_code_verbatim_and_redacted_message() {
        let error = batch_error_message("batch_plan_stale");
        assert_eq!(error, "batch plan is stale");
        let fallback = batch_error_message("unregistered_code");
        assert_eq!(fallback, "batch mod lifecycle operation failed");
    }

    #[test]
    fn capability_projection_disables_batch_entry_points_outside_sandbox() {
        let production = project_batch_capability(None);
        assert!(!production.preview_available);
        assert!(!production.write_available);
        assert_eq!(
            production.unavailable_reason_code.as_deref(),
            Some("sandbox_batch_production_forbidden")
        );
    }

    #[test]
    fn capability_projection_enables_batch_entry_points_for_valid_sandbox_environment() {
        let temp = tempfile::tempdir().expect("temp sandbox root");
        let environment = hmm_runtime::RuntimeEnvironment::sandbox(temp.path().to_path_buf())
            .expect("absolute temp path is a valid sandbox root");
        let capability = project_batch_capability(Some(&environment));

        assert!(capability.preview_available);
        assert!(capability.write_available);
        assert_eq!(capability.unavailable_reason_code, None);
    }
}
