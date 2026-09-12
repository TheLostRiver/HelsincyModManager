use crate::dto::{CommandErrorDto, TaskStartedDto};
use crate::equipment_retarget_dto::*;
use crate::reinstall_commands::parse_plan_token;
use crate::reinstall_dto::ReinstallPlanPreviewDto;
use crate::replacement_commands::{
    analyze_request_from_dto, parse_game_id, parse_target_id,
    replacement_workflow_error_to_command_error, required_id,
};
use crate::replacement_dto::AnalyzeImportedModReplacementRequestDto;
use crate::state::AppState;
use crate::task_events::{
    emit_task_progress, TauriTaskProgressObserver, INSTALL_REINSTALL_QUEUED_PHASE,
};
use hmm_app::{
    EquipmentRetargetReinstallRequest, InitialRetargetSelection, InitialRetargetSlotIntent,
    PreviewInitialRetargetInstallRequest, ReplacementWorkflowError,
    StartEquipmentRetargetReinstallTaskRequest, TaskProgressEvent,
};
use hmm_core::{FileLayer, ModId, ProfileId, ReplacementSourceId};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn get_equipment_retarget_configuration(
    request: AnalyzeImportedModReplacementRequestDto,
    state: State<'_, AppState>,
) -> Result<EquipmentRetargetConfigurationDto, CommandErrorDto> {
    let (request, profile) = analyze_request_from_dto(request)?;
    let workflow = Arc::clone(&state.replacement_workflow);
    tauri::async_runtime::spawn_blocking(move || {
        workflow.equipment_configuration(&request.game_id, &request.mod_id, profile.as_ref())
    })
    .await
    .map_err(|_| unavailable())?
    .map(Into::into)
    .map_err(replacement_workflow_error_to_command_error)
}

#[tauri::command]
pub async fn preview_equipment_retarget_install(
    request: EquipmentRetargetSelectionRequestDto,
    state: State<'_, AppState>,
) -> Result<EquipmentRetargetInstallPreviewDto, EquipmentRetargetPreviewErrorDto> {
    let request = selection_from_dto(request)?;
    let preflight = Arc::clone(&state.initial_retarget_install_preflight);
    tauri::async_runtime::spawn_blocking(move || preflight.preview(request))
        .await
        .map_err(|_| unavailable())?
        .map(Into::into)
        .map_err(EquipmentRetargetPreviewErrorDto::from)
}

#[tauri::command]
pub async fn preview_equipment_retarget_reinstall(
    request: EquipmentRetargetSelectionRequestDto,
    state: State<'_, AppState>,
) -> Result<ReinstallPlanPreviewDto, EquipmentRetargetPreviewErrorDto> {
    let request = reinstall_selection_from_dto(request)?;
    let executor = Arc::clone(&state.reinstall_executor);
    let preview = tauri::async_runtime::spawn_blocking(move || {
        executor.preview_equipment_retarget_reinstall(request)
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(EquipmentRetargetPreviewErrorDto::from)?;
    ReinstallPlanPreviewDto::try_from(preview).map_err(|_| unavailable().into())
}

#[tauri::command]
pub fn start_equipment_retarget_install_task(
    request: EquipmentRetargetSelectionRequestDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let request = selection_from_dto(request)?;
    let task = state
        .retarget_install_tasks
        .start_equipment_retarget_install_task(request.clone())
        .map_err(CommandErrorDto::from_task_manager_error)?;
    let _ = emit_task_progress(
        &app_handle,
        TaskProgressEvent::new(
            task.task_id.clone(),
            task.kind,
            task.status,
            "install.retarget.queued",
        ),
    );
    let runner = Arc::clone(&state.retarget_install_task_runner);
    let task_id = task.task_id.clone();
    std::thread::spawn(move || {
        let events = runner
            .run_equipment_retarget_install_task(&task_id, request)
            .unwrap_or_else(|error| error.events);
        for event in events {
            let _ = emit_task_progress(&app_handle, event);
        }
    });
    Ok(task.into())
}

#[tauri::command]
pub fn start_equipment_retarget_reinstall_task(
    request: StartEquipmentRetargetReinstallRequestDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let request = StartEquipmentRetargetReinstallTaskRequest {
        selection: reinstall_selection_from_dto(request.selection)?,
        plan_token: parse_plan_token(request.plan_token)?,
    };
    queue_equipment_reinstall(request, &state, app_handle)
}

#[tauri::command]
pub async fn preview_equipment_reapply(
    request: EquipmentReapplyRequestDto,
    state: State<'_, AppState>,
) -> Result<ReinstallPlanPreviewDto, EquipmentRetargetPreviewErrorDto> {
    let request = reapply_selection_from_dto(request)?;
    let executor = Arc::clone(&state.reinstall_executor);
    let preview = tauri::async_runtime::spawn_blocking(move || {
        executor.preview_equipment_retarget_reinstall(request)
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(EquipmentRetargetPreviewErrorDto::from)?;
    ReinstallPlanPreviewDto::try_from(preview).map_err(|_| unavailable().into())
}

#[tauri::command]
pub fn start_equipment_reapply_task(
    request: StartEquipmentReapplyRequestDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let request = StartEquipmentRetargetReinstallTaskRequest {
        selection: reapply_selection_from_dto(request.selection)?,
        plan_token: parse_plan_token(request.plan_token)?,
    };
    queue_equipment_reinstall(request, &state, app_handle)
}

fn reapply_selection_from_dto(
    request: EquipmentReapplyRequestDto,
) -> Result<EquipmentRetargetReinstallRequest, CommandErrorDto> {
    Ok(EquipmentRetargetReinstallRequest::reapply(
        parse_game_id(request.game_id)?,
        ProfileId::new(required_id(
            request.profile_id,
            "replacement_profile_id_invalid",
            "profile id is required",
        )?),
        ModId::new(required_id(
            request.mod_id,
            "replacement_mod_id_invalid",
            "Mod id is required",
        )?),
    ))
}

fn queue_equipment_reinstall(
    request: StartEquipmentRetargetReinstallTaskRequest,
    state: &AppState,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let task = state
        .reinstall_tasks
        .start_equipment_retarget_reinstall_task(request.clone())
        .map_err(CommandErrorDto::from_task_manager_error)?;
    let _ = emit_task_progress(
        &app_handle,
        TaskProgressEvent::new(
            task.task_id.clone(),
            task.kind,
            task.status,
            INSTALL_REINSTALL_QUEUED_PHASE,
        ),
    );
    let runner = Arc::clone(&state.reinstall_task_runner);
    let task_id = task.task_id.clone();
    std::thread::spawn(move || {
        let observer = TauriTaskProgressObserver::new(&app_handle);
        let _ = runner
            .run_equipment_retarget_reinstall_task_with_observer(&task_id, request, &observer);
    });
    Ok(task.into())
}

fn selection_from_dto(
    request: EquipmentRetargetSelectionRequestDto,
) -> Result<PreviewInitialRetargetInstallRequest, CommandErrorDto> {
    if request.slots.is_empty() {
        return Err(unavailable());
    }
    let slots = request
        .slots
        .into_iter()
        .map(|slot| {
            let parse_source = |value| ReplacementSourceId::parse(value).map_err(|_| unavailable());
            match slot {
                EquipmentSlotIntentDto::Keep { source_id } => {
                    Ok(InitialRetargetSlotIntent::KeepInPlace {
                        source_id: parse_source(source_id)?,
                    })
                }
                EquipmentSlotIntentDto::Retarget {
                    source_id,
                    target_id,
                } => Ok(InitialRetargetSlotIntent::Retarget {
                    source_id: parse_source(source_id)?,
                    target_id: parse_target_id(target_id)?,
                }),
            }
        })
        .collect::<Result<Vec<_>, CommandErrorDto>>()?;
    Ok(PreviewInitialRetargetInstallRequest {
        game_id: parse_game_id(request.game_id)?,
        profile_id: ProfileId::new(required_id(
            request.profile_id,
            "replacement_profile_id_invalid",
            "profile id is required",
        )?),
        mod_id: ModId::new(required_id(
            request.mod_id,
            "replacement_mod_id_invalid",
            "Mod id is required",
        )?),
        selection: InitialRetargetSelection::PerSlot(slots),
        layer: FileLayer::new(
            required_id(
                request.layer_name,
                "replacement_layer_invalid",
                "layer name is required",
            )?,
            request.layer_priority,
        ),
    })
}

fn reinstall_selection_from_dto(
    request: EquipmentRetargetSelectionRequestDto,
) -> Result<EquipmentRetargetReinstallRequest, CommandErrorDto> {
    let request = selection_from_dto(request)?;
    let InitialRetargetSelection::PerSlot(slots) = request.selection else {
        return Err(unavailable());
    };
    Ok(EquipmentRetargetReinstallRequest {
        intent: Default::default(),
        game_id: request.game_id,
        profile_id: request.profile_id,
        mod_id: request.mod_id,
        slots,
        layer: request.layer,
    })
}

fn unavailable() -> CommandErrorDto {
    replacement_workflow_error_to_command_error(ReplacementWorkflowError::PlanUnavailable)
}
