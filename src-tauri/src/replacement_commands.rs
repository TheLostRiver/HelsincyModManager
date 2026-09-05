use std::sync::Arc;

use hmm_app::{
    AnalyzeImportedReplacementRequest, InitialRetargetInstallPreflight, InitialRetargetSelection,
    PreviewInitialRetargetInstallRequest, ReinstallTaskService, ReplacementServiceError,
    ReplacementWorkflowError, RetargetInstallTaskService, RetargetReinstallRequest,
    StartRetargetInstallTaskRequest, StartRetargetReinstallTaskRequest, TaskProgressEvent,
    TaskStarted,
};
use hmm_core::{
    FileLayer, GameId, ModId, ProfileId, ReplacementAnalysis, ReplacementTarget,
    ReplacementTargetId, ReplacementWarning,
};
use tauri::{AppHandle, State};

use crate::dto::{
    AnalyzeImportedModReplacementRequestDto, CommandErrorDto, InitialRetargetInstallPreviewDto,
    ListReplacementTargetsRequestDto, PreviewInitialRetargetInstallRequestDto,
    PreviewRetargetReinstallRequestDto, ReplacementAnalysisDto, ReplacementSourceDto,
    ReplacementTargetDto, ReplacementWarningDto, RetargetActionPreviewDto,
    StartRetargetInstallTaskRequestDto, StartRetargetReinstallTaskRequestDto, TaskStartedDto,
};
use crate::reinstall_commands::{parse_plan_token, preview_error_to_command_error};
use crate::reinstall_dto::ReinstallPlanPreviewDto;
use crate::replacement_dto::{
    ListReplacementTargetOccupancyRequestDto, ReplacementTargetOccupancyDto,
};
use crate::state::{AppState, ConfiguredRetargetReinstallError};
use crate::task_events::{emit_task_progress, INSTALL_REINSTALL_QUEUED_PHASE};

const RETARGET_QUEUED_PHASE: &str = "install.retarget.queued";

#[tauri::command]
pub fn list_replacement_targets(
    request: ListReplacementTargetsRequestDto,
    state: State<'_, AppState>,
) -> Result<Vec<ReplacementTargetDto>, CommandErrorDto> {
    let game_id = parse_game_id(request.game_id)?;
    let mod_id = ModId::new(required_id(
        request.mod_id,
        "replacement_mod_id_invalid",
        "Mod id is required",
    )?);
    state
        .replacement_workflow
        .list_compatible_targets(&game_id, &mod_id, request.query.as_deref())
        .map(|targets| targets.into_iter().map(Into::into).collect())
        .map_err(replacement_workflow_error_to_command_error)
}

#[tauri::command]
pub fn analyze_imported_mod_replacement(
    request: AnalyzeImportedModReplacementRequestDto,
    state: State<'_, AppState>,
) -> Result<ReplacementAnalysisDto, CommandErrorDto> {
    let (request, profile_id) = analyze_request_from_dto(request)?;
    let mod_id = request.mod_id.clone();
    let analysis = state
        .replacement_workflow
        .analyze_imported_mod(request)
        .map_err(replacement_workflow_error_to_command_error)?;
    let installed_target_id = profile_id
        .map(|profile_id| {
            state
                .install_manifest_query
                .query_installed_replacement_target(&profile_id, &mod_id)
        })
        .transpose()
        .map_err(|_| CommandErrorDto {
            code: "replacement_install_state_unavailable".to_owned(),
            message: "replacement install state is unavailable".to_owned(),
        })?
        .flatten();
    Ok(replacement_analysis_to_dto(analysis, installed_target_id))
}

/// 列出该 profile 下**其他 Mod** 已占用的替换目标，供前端提示占用方并禁用写入。
///
/// 这是纯展示查询，不承担门禁职责：清单不可信或读取失败时返回空列表
/// （fail-open），前端因此不提示、不禁用，但硬门禁仍在预览、任务期计划构建
/// 和 commit 三层，冲突写入照样被拦。
#[tauri::command]
pub fn list_replacement_target_occupancy(
    request: ListReplacementTargetOccupancyRequestDto,
    state: State<'_, AppState>,
) -> Result<Vec<ReplacementTargetOccupancyDto>, CommandErrorDto> {
    // game_id 只用于确认该游戏支持替换目标；占用事实按 profile 判定。
    let _game_id = parse_game_id(request.game_id.clone())?;
    let (profile_id, mod_id) = occupancy_request_from_dto(request)?;

    Ok(state
        .replacement_occupancy
        .list_occupancy(&profile_id, &mod_id)
        .into_iter()
        .map(|occupancy| ReplacementTargetOccupancyDto {
            target_id: occupancy.target_id.as_str().to_owned(),
            mod_id: occupancy.mod_id.as_str().to_owned(),
            display_name: occupancy.display_name,
        })
        .collect())
}

#[tauri::command]
pub fn preview_initial_retarget_install(
    request: PreviewInitialRetargetInstallRequestDto,
    state: State<'_, AppState>,
) -> Result<InitialRetargetInstallPreviewDto, CommandErrorDto> {
    let request = preview_request_from_dto(request)?;
    state
        .initial_retarget_install_preflight
        .preview(request)
        .map(Into::into)
        .map_err(replacement_workflow_error_to_command_error)
}

#[tauri::command]
pub fn preview_retarget_reinstall(
    request: PreviewRetargetReinstallRequestDto,
    state: State<'_, AppState>,
) -> Result<ReinstallPlanPreviewDto, CommandErrorDto> {
    let preview = state
        .reinstall_executor
        .preview_retarget_reinstall(retarget_reinstall_request_from_dto(request)?)
        .map_err(retarget_reinstall_error_to_command_error)?;
    ReinstallPlanPreviewDto::try_from(preview).map_err(|_| CommandErrorDto {
        code: "replacement_reinstall_preview_unavailable".to_owned(),
        message: "replacement reinstall preview is unavailable".to_owned(),
    })
}

#[tauri::command]
pub fn start_retarget_install_task(
    request: StartRetargetInstallTaskRequestDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let request = start_request_from_dto(request)?;
    let runner_request = request.clone();
    let task = queue_retarget_install_task(&state.retarget_install_tasks, request)?;
    let _ = emit_task_progress(&app_handle, queued_event(&task));
    spawn_runner(
        Arc::clone(&state.retarget_install_task_runner),
        app_handle,
        task.task_id.clone(),
        runner_request,
    );
    Ok(task.into())
}

#[tauri::command]
pub fn start_retarget_reinstall_task(
    request: StartRetargetReinstallTaskRequestDto,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let request = start_retarget_reinstall_request_from_dto(request)?;
    let runner_request = request.clone();
    let task = queue_retarget_reinstall_task(&state.reinstall_tasks, request)?;
    let _ = emit_task_progress(
        &app_handle,
        TaskProgressEvent::new(
            task.task_id.clone(),
            task.kind,
            task.status,
            INSTALL_REINSTALL_QUEUED_PHASE,
        ),
    );
    spawn_retarget_reinstall_runner(
        Arc::clone(&state.reinstall_task_runner),
        app_handle,
        task.task_id.clone(),
        runner_request,
    );
    Ok(task.into())
}

fn queue_retarget_reinstall_task(
    task_service: &ReinstallTaskService,
    request: StartRetargetReinstallTaskRequest,
) -> Result<TaskStarted, CommandErrorDto> {
    task_service
        .start_retarget_reinstall_task(request)
        .map_err(CommandErrorDto::from_task_manager_error)
}

fn spawn_retarget_reinstall_runner(
    runner: Arc<hmm_app::ReinstallTaskRunner<crate::state::ConfiguredReinstallExecutor>>,
    app_handle: AppHandle,
    task_id: String,
    request: StartRetargetReinstallTaskRequest,
) {
    std::thread::spawn(move || {
        let events = match runner.run_retarget_reinstall_task(&task_id, request) {
            Ok(events) => events,
            Err(error) => error.events,
        };
        for event in events {
            let _ = emit_task_progress(&app_handle, event);
        }
    });
}

fn queue_retarget_install_task(
    task_service: &RetargetInstallTaskService,
    request: StartRetargetInstallTaskRequest,
) -> Result<TaskStarted, CommandErrorDto> {
    task_service
        .start_retarget_install_task(request)
        .map_err(CommandErrorDto::from_task_manager_error)
}

fn spawn_runner(
    runner: Arc<hmm_app::RetargetInstallTaskRunner>,
    app_handle: AppHandle,
    task_id: String,
    request: StartRetargetInstallTaskRequest,
) {
    std::thread::spawn(move || {
        let events = match runner.run_retarget_install_task(&task_id, request) {
            Ok(events) => events,
            Err(error) => error.events,
        };
        for event in events {
            let _ = emit_task_progress(&app_handle, event);
        }
    });
}

fn queued_event(task: &TaskStarted) -> TaskProgressEvent {
    TaskProgressEvent::new(
        task.task_id.clone(),
        task.kind,
        task.status,
        RETARGET_QUEUED_PHASE,
    )
}

fn analyze_request_from_dto(
    request: AnalyzeImportedModReplacementRequestDto,
) -> Result<(AnalyzeImportedReplacementRequest, Option<ProfileId>), CommandErrorDto> {
    let profile_id = request
        .profile_id
        .map(|profile_id| {
            required_id(
                profile_id,
                "replacement_profile_id_invalid",
                "profile id is required",
            )
            .map(ProfileId::new)
        })
        .transpose()?;
    Ok((
        AnalyzeImportedReplacementRequest {
            game_id: parse_game_id(request.game_id)?,
            mod_id: ModId::new(required_id(
                request.mod_id,
                "replacement_mod_id_invalid",
                "Mod id is required",
            )?),
        },
        profile_id,
    ))
}

/// 占用查询只接受稳定身份：profile 与 Mod 都必填，游戏 id 单独校验。
fn occupancy_request_from_dto(
    request: ListReplacementTargetOccupancyRequestDto,
) -> Result<(ProfileId, ModId), CommandErrorDto> {
    Ok((
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

fn preview_request_from_dto(
    request: PreviewInitialRetargetInstallRequestDto,
) -> Result<PreviewInitialRetargetInstallRequest, CommandErrorDto> {
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
        selection: InitialRetargetSelection::SoleSource {
            target_id: parse_target_id(request.target_id)?,
        },
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

fn start_request_from_dto(
    request: StartRetargetInstallTaskRequestDto,
) -> Result<StartRetargetInstallTaskRequest, CommandErrorDto> {
    let preview = preview_request_from_dto(PreviewInitialRetargetInstallRequestDto {
        game_id: request.game_id,
        profile_id: request.profile_id,
        mod_id: request.mod_id,
        target_id: request.target_id,
        layer_name: request.layer_name,
        layer_priority: request.layer_priority,
    })?;
    Ok(StartRetargetInstallTaskRequest {
        game_id: preview.game_id,
        profile_id: preview.profile_id,
        mod_id: preview.mod_id,
        // `preview_request_from_dto` 只会构造 `SoleSource`——前端目前发的就是单目标。
        // 逐槽位意图（D2 三态）的 DTO 是 `#349` 切片④ 的事。
        target_id: preview
            .selection
            .sole_target_id()
            .cloned()
            .ok_or_else(|| CommandErrorDto {
                code: "replacement_target_id_invalid".to_owned(),
                message: "a retarget install task needs exactly one target".to_owned(),
            })?,
        layer: preview.layer,
    })
}

fn retarget_reinstall_request_from_dto(
    request: PreviewRetargetReinstallRequestDto,
) -> Result<RetargetReinstallRequest, CommandErrorDto> {
    Ok(RetargetReinstallRequest {
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
        target_id: parse_target_id(request.target_id)?,
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

fn start_retarget_reinstall_request_from_dto(
    request: StartRetargetReinstallTaskRequestDto,
) -> Result<StartRetargetReinstallTaskRequest, CommandErrorDto> {
    let plan_token = parse_plan_token(request.plan_token)?;
    let preview = retarget_reinstall_request_from_dto(PreviewRetargetReinstallRequestDto {
        game_id: request.game_id,
        profile_id: request.profile_id,
        mod_id: request.mod_id,
        target_id: request.target_id,
        layer_name: request.layer_name,
        layer_priority: request.layer_priority,
    })?;
    Ok(StartRetargetReinstallTaskRequest {
        game_id: preview.game_id,
        profile_id: preview.profile_id,
        mod_id: preview.mod_id,
        target_id: preview.target_id,
        layer: preview.layer,
        plan_token,
    })
}

fn parse_game_id(value: String) -> Result<GameId, CommandErrorDto> {
    GameId::parse(value).map_err(|_| CommandErrorDto {
        code: "replacement_unsupported_game".to_owned(),
        message: "replacement is unsupported for this game".to_owned(),
    })
}

fn parse_target_id(value: String) -> Result<ReplacementTargetId, CommandErrorDto> {
    ReplacementTargetId::parse(value).map_err(|_| CommandErrorDto {
        code: "replacement_target_id_invalid".to_owned(),
        message: "replacement target id is invalid".to_owned(),
    })
}

fn required_id(
    value: String,
    code: &'static str,
    message: &'static str,
) -> Result<String, CommandErrorDto> {
    let value = value.trim();
    if value.is_empty() {
        Err(CommandErrorDto {
            code: code.to_owned(),
            message: message.to_owned(),
        })
    } else {
        Ok(value.to_owned())
    }
}

fn replacement_analysis_to_dto(
    analysis: ReplacementAnalysis,
    installed_target_id: Option<ReplacementTargetId>,
) -> ReplacementAnalysisDto {
    let mut response: ReplacementAnalysisDto = analysis.into();
    response.installed_target_id =
        installed_target_id.map(|target_id| target_id.as_str().to_owned());
    response
}

fn replacement_workflow_error_to_command_error(error: ReplacementWorkflowError) -> CommandErrorDto {
    let (code, message) = match error {
        ReplacementWorkflowError::UnsupportedGame => (
            "replacement_unsupported_game",
            "replacement is unsupported for this game",
        ),
        ReplacementWorkflowError::CatalogUnavailable => (
            "replacement_target_catalog_unavailable",
            "replacement target catalog is unavailable",
        ),
        ReplacementWorkflowError::TargetNotFound => (
            "replacement_target_not_found",
            "replacement target was not found",
        ),
        ReplacementWorkflowError::ModRepositoryUnavailable
        | ReplacementWorkflowError::RevisionNotFound => (
            "replacement_package_unavailable",
            "imported Mod package is unavailable",
        ),
        // `#349` 切片③b：逐槽位意图的两档错误。前端目前只发单目标，构造不出它们，
        // 但错误码要先有——切片④ 的 UI 会直接用上。
        ReplacementWorkflowError::DuplicateSlotIntent => (
            "replacement_duplicate_slot_intent",
            "one replacement source was given two slot intents",
        ),
        ReplacementWorkflowError::DuplicateSlotTarget => (
            "replacement_duplicate_slot_target",
            "two replacement sources aim at one target",
        ),
        ReplacementWorkflowError::KeepInPlaceUnavailable => (
            "replacement_keep_in_place_unavailable",
            "this replacement source cannot be kept in place",
        ),
        ReplacementWorkflowError::ModNotFound => {
            ("replacement_mod_not_found", "imported Mod was not found")
        }
        ReplacementWorkflowError::SandboxUnavailable
        | ReplacementWorkflowError::PackageFilesUnavailable => (
            "replacement_package_unavailable",
            "imported Mod package is unavailable",
        ),
        ReplacementWorkflowError::SourceNotRetargetable => (
            "replacement_source_not_retargetable",
            "imported Mod does not contain one supported replacement source",
        ),
        ReplacementWorkflowError::InstallStatusUnavailable => (
            "replacement_install_state_unavailable",
            "replacement install state is unavailable",
        ),
        ReplacementWorkflowError::InitialInstallBlocked { status: _ } => (
            "replacement_initial_install_blocked",
            "initial replacement install is not allowed in the current state",
        ),
        ReplacementWorkflowError::BindingUnavailable
        | ReplacementWorkflowError::PlanUnavailable => (
            "replacement_preview_unavailable",
            "replacement preview is unavailable",
        ),
        ReplacementWorkflowError::InstallManifestUnavailable => (
            "replacement_install_manifest_unavailable",
            "install manifest is unavailable; the write admission cannot be established",
        ),
        ReplacementWorkflowError::InstalledBindingUnavailable => (
            "replacement_installed_binding_unavailable",
            "installed replacement binding is unavailable",
        ),
        ReplacementWorkflowError::TargetAlreadySelected => (
            "replacement_target_already_selected",
            "replacement target is already selected",
        ),
        ReplacementWorkflowError::Analysis(error) => return analysis_error_to_command_error(error),
    };
    CommandErrorDto {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

fn retarget_reinstall_error_to_command_error(
    error: ConfiguredRetargetReinstallError,
) -> CommandErrorDto {
    match error {
        ConfiguredRetargetReinstallError::Reinstall(error) => preview_error_to_command_error(error),
        ConfiguredRetargetReinstallError::Replacement(error) => {
            replacement_workflow_error_to_command_error(error)
        }
    }
}

fn analysis_error_to_command_error(error: ReplacementServiceError) -> CommandErrorDto {
    let code = match error {
        ReplacementServiceError::UnsupportedGame => "replacement_unsupported_game",
        ReplacementServiceError::Adapter(
            hmm_ports::ReplacementAdapterError::UnrecognizedSourceSlot
            | hmm_ports::ReplacementAdapterError::AmbiguousSourceSlot
            | hmm_ports::ReplacementAdapterError::SourceBindingMismatch,
        ) => "replacement_source_not_retargetable",
        // #356：认得出源槽位、但本游戏当前没有可选目标（防具 catalog 只覆盖女装）。
        // 与上面那组分开是因为玩家的下一步不同：那组是「这个包不行」，这条是
        // 「包没问题，是我们还没覆盖到」，让玩家去换包是错的引导。
        ReplacementServiceError::Adapter(
            hmm_ports::ReplacementAdapterError::SourceHasNoAvailableTargets,
        ) => "replacement_source_has_no_targets",
        ReplacementServiceError::Adapter(
            hmm_ports::ReplacementAdapterError::TargetCatalogMissing { .. }
            | hmm_ports::ReplacementAdapterError::TargetCatalogUnavailable,
        ) => "replacement_target_catalog_unavailable",
        ReplacementServiceError::Adapter(
            hmm_ports::ReplacementAdapterError::SourceContentUnavailable,
        ) => "weapon_source_content_unavailable",
        ReplacementServiceError::Adapter(
            hmm_ports::ReplacementAdapterError::AnalysisRejected { code },
        ) => code,
        ReplacementServiceError::Adapter(_) => "replacement_analysis_unavailable",
    };
    CommandErrorDto {
        code: code.to_owned(),
        message: "replacement analysis is unavailable".to_owned(),
    }
}

impl From<ReplacementTarget> for ReplacementTargetDto {
    fn from(target: ReplacementTarget) -> Self {
        // I18N-08：不再按固定 locale 投影，DTO 携带全语言名称表。
        // LocalizedText 构造时已拒绝空表（EmptyLocalizedText），此处必然非空。
        let display_names: std::collections::BTreeMap<String, String> =
            target.display_name().clone().into();
        Self {
            id: target.id().as_str().to_owned(),
            game_id: target.game_id().as_str().to_owned(),
            target_type: target.target_type().as_str().to_owned(),
            display_names,
            aliases: target.aliases().to_vec(),
            aliases_by_locale: target.localized_aliases().cloned(),
            internal_id: target.internal_id().to_owned(),
        }
    }
}

impl From<ReplacementAnalysis> for ReplacementAnalysisDto {
    fn from(analysis: ReplacementAnalysis) -> Self {
        Self {
            game_id: analysis.game_id().as_str().to_owned(),
            installed_target_id: None,
            retargetable: analysis.is_retargetable(),
            matched_asset_count: analysis.matched_asset_count(),
            sources: analysis
                .sources()
                .iter()
                .map(|source| ReplacementSourceDto {
                    id: source.id().as_str().to_owned(),
                    source_type: source.source_type().as_str().to_owned(),
                    internal_id: source.internal_id().to_owned(),
                    supported: source.is_supported(),
                })
                .collect(),
            warnings: analysis
                .warnings()
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<ReplacementWarning> for ReplacementWarningDto {
    fn from(warning: ReplacementWarning) -> Self {
        match warning {
            ReplacementWarning::NoSupportedAssets => Self::NoSupportedAssets,
            ReplacementWarning::MultipleSources => Self::MultipleSources,
            ReplacementWarning::UnsupportedSource => Self::UnsupportedSource,
            ReplacementWarning::SourceMatchesTarget => Self::SourceMatchesTarget,
            ReplacementWarning::WeaponPartialPartSet => Self::WeaponPartialPartSet,
        }
    }
}

impl From<InitialRetargetInstallPreflight> for InitialRetargetInstallPreviewDto {
    fn from(preflight: InitialRetargetInstallPreflight) -> Self {
        let planned = preflight.planned;
        Self {
            analysis: planned.analysis().clone().into(),
            // 单目标预览：前端契约仍是单个 target/actions/warnings。多槽位预览的 DTO
            // 是 `#349` 切片④ 的事，这里取第一个（`SoleSource` 下恰好只有一个）。
            target: planned
                .targets()
                .first()
                .cloned()
                .expect("a planned retarget install always carries at least one target")
                .into(),
            actions: planned
                .retarget_plans()
                .iter()
                .flat_map(|plan| plan.actions())
                .map(|action| RetargetActionPreviewDto {
                    source_internal_id: action.source_internal_id().to_owned(),
                    target_internal_id: action.target_internal_id().to_owned(),
                })
                .collect(),
            warnings: planned
                .retarget_plans()
                .iter()
                .flat_map(|plan| plan.warnings())
                .copied()
                .map(Into::into)
                .collect(),
            install_plan: planned.install_plan().clone().into(),
            prerequisite_decision: preflight.prerequisite_decision.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{LocalizedText, ReplacementTargetKind};
    use serde_json::json;
    use std::collections::BTreeMap;

    /// WR-05 起 catalogScope 概念随 developer seed 退役：DTO 不得再出现该字段，
    /// 防止后端悄悄把 scope 元数据带回前端契约。
    #[test]
    fn target_projection_does_not_carry_catalog_scope() {
        let target = ReplacementTarget::new(
            ReplacementTargetId::parse("mhw:weapon:scope-one001").expect("target id"),
            GameId::mhw(),
            ReplacementTargetKind::parse("weapon").expect("weapon kind"),
            LocalizedText::new(BTreeMap::from([(
                "en".to_owned(),
                "Artificial weapon".to_owned(),
            )]))
            .expect("display name"),
            Vec::new(),
            "one001",
            BTreeMap::from([("catalog_scope".to_owned(), json!("developer_sandbox"))]),
        )
        .expect("replacement target");

        let value = serde_json::to_value(ReplacementTargetDto::from(target)).expect("dto json");
        assert!(
            value.get("catalogScope").is_none(),
            "DTO 不应再投影 catalogScope"
        );
    }

    /// #274：按语言分组的别名原样透传；来源没给（铠甲）时 DTO 省略键，不伪造空对象。
    #[test]
    fn target_projection_passes_localized_aliases_through_and_omits_them_when_absent() {
        let build = || {
            ReplacementTarget::new(
                ReplacementTargetId::parse("mhw:weapon:two029").expect("target id"),
                GameId::mhw(),
                ReplacementTargetKind::parse("weapon").expect("weapon kind"),
                LocalizedText::new(BTreeMap::from([
                    ("zh_cn".to_owned(), "黑龙刃".to_owned()),
                    ("en".to_owned(), "Fatalis Blade".to_owned()),
                ]))
                .expect("display name"),
                vec!["Black Fatalis Blade".to_owned(), "黑龙玄刃".to_owned()],
                "two029",
                BTreeMap::new(),
            )
            .expect("replacement target")
        };

        let without = serde_json::to_value(ReplacementTargetDto::from(build())).expect("dto json");
        assert!(without.get("aliasesByLocale").is_none());
        assert_eq!(
            without["aliases"],
            json!(["Black Fatalis Blade", "黑龙玄刃"])
        );

        let localized = build()
            .with_localized_aliases(BTreeMap::from([
                ("zh_cn".to_owned(), vec!["黑龙玄刃".to_owned()]),
                ("en".to_owned(), vec!["Black Fatalis Blade".to_owned()]),
            ]))
            .expect("localized aliases");
        let with = serde_json::to_value(ReplacementTargetDto::from(localized)).expect("dto json");
        assert_eq!(
            with["aliasesByLocale"],
            json!({ "en": ["Black Fatalis Blade"], "zh_cn": ["黑龙玄刃"] })
        );
        assert_eq!(with["aliases"], json!(["Black Fatalis Blade", "黑龙玄刃"]));
    }

    #[test]
    fn request_mapping_rejects_paths_and_accepts_only_stable_ids() {
        let request: PreviewInitialRetargetInstallRequestDto = serde_json::from_value(json!({
            "gameId": "mhw",
            "profileId": "profile-a",
            "modId": "mod-a",
            "targetId": "mhw:armor:fatalis-alpha",
            "layerName": "base",
            "layerPriority": 0
        }))
        .expect("deserialize stable ids");
        let mapped = preview_request_from_dto(request).expect("map stable ids");
        assert_eq!(mapped.mod_id.as_str(), "mod-a");
        assert_eq!(
            mapped
                .selection
                .sole_target_id()
                .expect("single-target selection")
                .as_str(),
            "mhw:armor:fatalis-alpha"
        );

        let forbidden = serde_json::from_value::<PreviewInitialRetargetInstallRequestDto>(json!({
            "gameId": "mhw",
            "profileId": "profile-a",
            "modId": "mod-a",
            "targetId": "mhw:armor:fatalis-alpha",
            "layerName": "base",
            "layerPriority": 0,
            "sandboxPath": "C:/forbidden"
        }));
        assert!(forbidden.is_err());
    }

    #[test]
    fn analysis_request_mapping_keeps_profile_identity_optional_and_backend_owned() {
        let (request, profile_id) =
            analyze_request_from_dto(AnalyzeImportedModReplacementRequestDto {
                game_id: "mhw".to_owned(),
                profile_id: Some("profile-a".to_owned()),
                mod_id: "mod-a".to_owned(),
            })
            .expect("map analysis request");

        assert_eq!(request.mod_id.as_str(), "mod-a");
        assert_eq!(profile_id.expect("profile id").as_str(), "profile-a");
    }

    #[test]
    fn occupancy_request_mapping_requires_profile_and_rejects_backend_paths() {
        let (profile_id, mod_id) =
            occupancy_request_from_dto(ListReplacementTargetOccupancyRequestDto {
                game_id: "mhw".to_owned(),
                profile_id: "profile-a".to_owned(),
                mod_id: "mod-a".to_owned(),
            })
            .expect("map occupancy request");
        assert_eq!(profile_id.as_str(), "profile-a");
        assert_eq!(mod_id.as_str(), "mod-a");

        let without_profile =
            occupancy_request_from_dto(ListReplacementTargetOccupancyRequestDto {
                game_id: "mhw".to_owned(),
                profile_id: "   ".to_owned(),
                mod_id: "mod-a".to_owned(),
            })
            .expect_err("occupancy is a per-profile query");
        assert_eq!(without_profile.code, "replacement_profile_id_invalid");

        let without_mod = occupancy_request_from_dto(ListReplacementTargetOccupancyRequestDto {
            game_id: "mhw".to_owned(),
            profile_id: "profile-a".to_owned(),
            mod_id: String::new(),
        })
        .expect_err("mod identity is required");
        assert_eq!(without_mod.code, "replacement_mod_id_invalid");
    }

    #[test]
    fn analysis_response_maps_only_the_stable_installed_target_id() {
        let analysis = ReplacementAnalysis::new(GameId::mhw(), Vec::new(), 0, Vec::new())
            .expect("replacement analysis");
        let response = replacement_analysis_to_dto(
            analysis,
            Some(
                ReplacementTargetId::parse("mhw:armor:fatalis-beta")
                    .expect("replacement target id"),
            ),
        );

        assert_eq!(
            response.installed_target_id.as_deref(),
            Some("mhw:armor:fatalis-beta")
        );
    }

    #[test]
    fn workflow_errors_map_to_stable_codes_without_status_details() {
        let blocked = replacement_workflow_error_to_command_error(
            ReplacementWorkflowError::InitialInstallBlocked {
                status: hmm_app::InstallRecoveryStatus::RollbackRequired,
            },
        );
        assert_eq!(blocked.code, "replacement_initial_install_blocked");
        assert!(!blocked.message.contains("RollbackRequired"));

        let unchanged = replacement_workflow_error_to_command_error(
            ReplacementWorkflowError::TargetAlreadySelected,
        );
        assert_eq!(unchanged.code, "replacement_target_already_selected");
        assert!(!unchanged.message.contains("mhw:armor"));
    }

    #[test]
    fn start_task_queueing_does_not_require_a_workflow_preview() {
        let task_manager = Arc::new(hmm_app::TaskManager::new());
        let task_service = hmm_app::RetargetInstallTaskService::new(task_manager);
        let request = start_request_from_dto(StartRetargetInstallTaskRequestDto {
            game_id: "mhw".to_owned(),
            profile_id: "profile-a".to_owned(),
            mod_id: "mod-a".to_owned(),
            target_id: "mhw:armor:fatalis-alpha".to_owned(),
            layer_name: "base".to_owned(),
            layer_priority: 0,
        })
        .expect("valid controlled request");

        let task = queue_retarget_install_task(&task_service, request).expect("queue task");

        assert_eq!(task.kind, hmm_app::TaskKind::Install);
        assert_eq!(task.status, hmm_app::TaskStatus::Queued);
    }

    #[test]
    fn retarget_reinstall_mapping_keeps_revision_and_paths_backend_owned() {
        let preview = retarget_reinstall_request_from_dto(PreviewRetargetReinstallRequestDto {
            game_id: "mhw".to_owned(),
            profile_id: "profile-a".to_owned(),
            mod_id: "mod-a".to_owned(),
            target_id: "mhw:armor:fatalis-beta".to_owned(),
            layer_name: "base".to_owned(),
            layer_priority: 0,
        })
        .expect("map controlled target-switch preview");
        assert_eq!(preview.mod_id.as_str(), "mod-a");
        assert_eq!(preview.target_id.as_str(), "mhw:armor:fatalis-beta");

        let start =
            start_retarget_reinstall_request_from_dto(StartRetargetReinstallTaskRequestDto {
                game_id: "mhw".to_owned(),
                profile_id: "profile-a".to_owned(),
                mod_id: "mod-a".to_owned(),
                target_id: "mhw:armor:fatalis-beta".to_owned(),
                layer_name: "base".to_owned(),
                layer_priority: 0,
                plan_token: format!("reinstall-preview-v1:{}", "a".repeat(64)),
            })
            .expect("map controlled target-switch start");
        assert_eq!(start.target_id.as_str(), "mhw:armor:fatalis-beta");
        assert!(start.plan_token.starts_with("reinstall-preview-v1:"));

        let invalid =
            start_retarget_reinstall_request_from_dto(StartRetargetReinstallTaskRequestDto {
                game_id: "mhw".to_owned(),
                profile_id: "profile-a".to_owned(),
                mod_id: "mod-a".to_owned(),
                target_id: "mhw:armor:fatalis-beta".to_owned(),
                layer_name: "base".to_owned(),
                layer_priority: 0,
                plan_token: "not-a-plan-token".to_owned(),
            })
            .expect_err("target switch must consume a validated preview token");
        assert_eq!(invalid.code, "plan_token_invalid");
    }

    #[test]
    fn retarget_reinstall_queueing_uses_the_existing_install_task_shape() {
        let task_manager = Arc::new(hmm_app::TaskManager::new());
        let task_service = hmm_app::ReinstallTaskService::new(task_manager);
        let request =
            start_retarget_reinstall_request_from_dto(StartRetargetReinstallTaskRequestDto {
                game_id: "mhw".to_owned(),
                profile_id: "profile-a".to_owned(),
                mod_id: "mod-a".to_owned(),
                target_id: "mhw:armor:fatalis-beta".to_owned(),
                layer_name: "base".to_owned(),
                layer_priority: 0,
                plan_token: format!("reinstall-preview-v1:{}", "b".repeat(64)),
            })
            .expect("valid controlled target-switch request");

        let task = queue_retarget_reinstall_task(&task_service, request).expect("queue task");

        assert_eq!(task.kind, hmm_app::TaskKind::Install);
        assert_eq!(task.status, hmm_app::TaskStatus::Queued);
    }
}
