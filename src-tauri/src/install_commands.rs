use crate::dto::{
    CommandErrorDto, InstallPlanPreviewDto, PreviewImportedModInstallPlanRequestDto,
    PreviewInstallPlanFileInputDto, PreviewInstallPlanRequestDto,
};
use crate::state::AppState;
use hmm_app::{
    BuildImportedModInstallPlanRequest, BuildInstallPlanRequest, InstallPlanFile,
    InstallPlanningError,
};
use hmm_core::{FileLayer, GameId, InstallTargetPathError, ModId, PackageFileId};
use tauri::State;

#[tauri::command]
pub fn preview_install_plan(
    request: PreviewInstallPlanRequestDto,
    state: State<'_, AppState>,
) -> Result<InstallPlanPreviewDto, CommandErrorDto> {
    let request = build_install_plan_request_from_dto(request);
    let plan = state
        .install_planning
        .build_plan(request)
        .map_err(install_planning_error_to_command_error)?;

    Ok(plan.into())
}

#[tauri::command]
pub fn preview_imported_mod_install_plan(
    request: PreviewImportedModInstallPlanRequestDto,
    state: State<'_, AppState>,
) -> Result<InstallPlanPreviewDto, CommandErrorDto> {
    let request = imported_mod_install_plan_request_from_dto(request)?;
    let plan = state
        .install_planning
        .build_plan_from_imported_mod(request)
        .map_err(install_planning_error_to_command_error)?;

    Ok(plan.into())
}

fn build_install_plan_request_from_dto(
    request: PreviewInstallPlanRequestDto,
) -> BuildInstallPlanRequest {
    BuildInstallPlanRequest {
        allowed_target_roots: request.allowed_target_roots,
        files: request
            .files
            .into_iter()
            .map(install_plan_file_from_dto)
            .collect(),
    }
}

fn imported_mod_install_plan_request_from_dto(
    request: PreviewImportedModInstallPlanRequestDto,
) -> Result<BuildImportedModInstallPlanRequest, CommandErrorDto> {
    let game_id = GameId::parse(request.game_id).map_err(|_| CommandErrorDto {
        code: "game_id_invalid".to_owned(),
        message: "game id is invalid".to_owned(),
    })?;

    Ok(BuildImportedModInstallPlanRequest {
        game_id,
        mod_id: ModId::new(request.mod_id),
        layer: FileLayer::new(request.layer_name, request.layer_priority),
    })
}

fn install_plan_file_from_dto(file: PreviewInstallPlanFileInputDto) -> InstallPlanFile {
    InstallPlanFile {
        mod_id: ModId::new(file.mod_id),
        package_file_id: PackageFileId::new(file.package_file_id),
        target_path: file.target_path,
        layer: FileLayer::new(file.layer_name, file.layer_priority),
    }
}

fn install_planning_error_to_command_error(error: InstallPlanningError) -> CommandErrorDto {
    match error {
        InstallPlanningError::InvalidTargetPath {
            package_file_id: _,
            source,
        } => CommandErrorDto {
            code: install_target_path_error_code(source).to_owned(),
            message: "install target path is invalid".to_owned(),
        },
        InstallPlanningError::ImportedModSourcesUnavailable => CommandErrorDto {
            code: "install_planning_sources_unavailable".to_owned(),
            message: "install planning sources are unavailable".to_owned(),
        },
        InstallPlanningError::GameAdapterNotFound { game_id: _ } => CommandErrorDto {
            code: "install_planning_game_adapter_not_found".to_owned(),
            message: "game adapter is unavailable for install planning".to_owned(),
        },
        InstallPlanningError::ImportedModNotFound { mod_id: _ } => CommandErrorDto {
            code: "install_planning_imported_mod_not_found".to_owned(),
            message: "imported mod was not found".to_owned(),
        },
        InstallPlanningError::ImportedModAnalysisUnavailable => CommandErrorDto {
            code: "install_planning_imported_mod_analysis_unavailable".to_owned(),
            message: "imported mod analysis is unavailable".to_owned(),
        },
        InstallPlanningError::ImportedModSandboxUnavailable => CommandErrorDto {
            code: "install_planning_imported_mod_sandbox_unavailable".to_owned(),
            message: "imported mod sandbox is unavailable".to_owned(),
        },
        InstallPlanningError::ImportedModFileScanUnavailable => CommandErrorDto {
            code: "install_planning_imported_mod_file_scan_unavailable".to_owned(),
            message: "imported mod files are unavailable".to_owned(),
        },
    }
}

fn install_target_path_error_code(error: InstallTargetPathError) -> &'static str {
    match error {
        InstallTargetPathError::Empty => "install_target_path_empty",
        InstallTargetPathError::Absolute => "install_target_path_absolute",
        InstallTargetPathError::ParentTraversal => "install_target_path_parent_traversal",
        InstallTargetPathError::WindowsDrivePrefix => "install_target_path_windows_drive_prefix",
        InstallTargetPathError::InvalidSegment => "install_target_path_invalid_segment",
        InstallTargetPathError::TargetRootNotAllowed { .. } => "install_target_root_not_allowed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{
        InstallPlanPreviewDto, PreviewImportedModInstallPlanRequestDto,
        PreviewInstallPlanFileInputDto, PreviewInstallPlanRequestDto,
    };
    use hmm_core::{
        FileLayer, InstallAction, InstallConflict, InstallFileProvider, InstallPlan,
        InstallTargetPath, InstallTargetPathError, ModId, PackageFileId,
    };
    use serde_json::{json, Value};

    #[test]
    fn preview_install_plan_request_deserializes_camel_case_fields() {
        let value = json!({
            "allowedTargetRoots": ["content"],
            "files": [{
                "modId": "mod-a",
                "packageFileId": "file-a",
                "targetPath": "content/models/player.mod3",
                "layerName": "base",
                "layerPriority": 10
            }]
        });

        let request: PreviewInstallPlanRequestDto =
            serde_json::from_value(value).expect("request should deserialize");

        assert_eq!(request.allowed_target_roots, vec!["content"]);
        assert_eq!(request.files[0].mod_id, "mod-a");
        assert_eq!(request.files[0].package_file_id, "file-a");
        assert_eq!(request.files[0].target_path, "content/models/player.mod3");
        assert_eq!(request.files[0].layer_name, "base");
        assert_eq!(request.files[0].layer_priority, 10);
    }

    #[test]
    fn preview_imported_mod_install_plan_request_deserializes_without_paths() {
        let value = json!({
            "gameId": "mhw",
            "modId": "mod-a",
            "layerName": "base",
            "layerPriority": 10
        });

        let request: PreviewImportedModInstallPlanRequestDto =
            serde_json::from_value(value).expect("request should deserialize");
        let app_request = imported_mod_install_plan_request_from_dto(request)
            .expect("valid ids should map to app request");

        assert_eq!(app_request.game_id.as_str(), "mhw");
        assert_eq!(app_request.mod_id.as_str(), "mod-a");
        assert_eq!(app_request.layer.name, "base");
        assert_eq!(app_request.layer.priority, 10);
    }

    #[test]
    fn install_plan_preview_serializes_actions_and_conflicts() {
        let action_target =
            InstallTargetPath::parse("content/models/player.mod3", ["content"]).expect("target");
        let conflict_target =
            InstallTargetPath::parse("content/models/weapon.mod3", ["content"]).expect("target");
        let provider_a = provider("mod-a", "file-a", action_target.clone(), "base", 10);
        let provider_b = provider("mod-b", "file-b", conflict_target.clone(), "base", 0);
        let provider_c = provider("mod-c", "file-c", conflict_target.clone(), "base", 0);
        let plan = InstallPlan {
            actions: vec![InstallAction {
                target_path: action_target,
                provider: provider_a,
            }],
            conflicts: vec![InstallConflict {
                target_path: conflict_target,
                providers: vec![provider_b, provider_c],
            }],
        };

        let dto: InstallPlanPreviewDto = plan.into();
        let value: Value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["hasBlockingConflicts"], true);
        assert_eq!(
            value["actions"][0]["targetPath"],
            "content/models/player.mod3"
        );
        assert_eq!(value["actions"][0]["modId"], "mod-a");
        assert_eq!(value["actions"][0]["packageFileId"], "file-a");
        assert_eq!(value["actions"][0]["layerName"], "base");
        assert_eq!(value["actions"][0]["layerPriority"], 10);
        assert_eq!(
            value["conflicts"][0]["targetPath"],
            "content/models/weapon.mod3"
        );
        assert_eq!(value["conflicts"][0]["providers"][0]["modId"], "mod-b");
        assert_eq!(
            value["conflicts"][0]["providers"][1]["packageFileId"],
            "file-c"
        );
    }

    #[test]
    fn invalid_target_path_error_uses_stable_code_without_paths() {
        let error = install_planning_error_to_command_error(
            hmm_app::InstallPlanningError::InvalidTargetPath {
                package_file_id: PackageFileId::new("file-a"),
                source: InstallTargetPathError::ParentTraversal,
            },
        );

        assert_eq!(error.code, "install_target_path_parent_traversal");
        assert!(!error.message.contains("../"));
        assert!(!error.message.contains('\\'));
    }

    #[test]
    fn file_input_maps_to_app_request_file() {
        let input = PreviewInstallPlanFileInputDto {
            mod_id: "mod-a".to_owned(),
            package_file_id: "file-a".to_owned(),
            target_path: "content/models/player.mod3".to_owned(),
            layer_name: "base".to_owned(),
            layer_priority: 10,
        };

        let file = install_plan_file_from_dto(input);

        assert_eq!(file.mod_id.as_str(), "mod-a");
        assert_eq!(file.package_file_id.as_str(), "file-a");
        assert_eq!(file.target_path, "content/models/player.mod3");
        assert_eq!(file.layer.name, "base");
        assert_eq!(file.layer.priority, 10);
    }

    fn provider(
        mod_id: &str,
        package_file_id: &str,
        target_path: InstallTargetPath,
        layer_name: &str,
        layer_priority: i32,
    ) -> InstallFileProvider {
        InstallFileProvider::new(
            ModId::new(mod_id),
            PackageFileId::new(package_file_id),
            target_path,
            FileLayer::new(layer_name, layer_priority),
        )
    }
}
