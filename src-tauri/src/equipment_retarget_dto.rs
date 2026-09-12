use crate::dto::{GamePrerequisiteDecisionDto, InstallPlanPreviewDto};
use crate::replacement_dto::{
    ReplacementAnalysisDto, ReplacementSourceDto, ReplacementTargetDto, ReplacementWarningDto,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 装备预览错误可指向一个稳定来源；不扩张通用错误 DTO，也不暴露资源路径。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentRetargetPreviewErrorDto {
    #[serde(flatten)]
    pub error: crate::dto::CommandErrorDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
}

impl From<crate::dto::CommandErrorDto> for EquipmentRetargetPreviewErrorDto {
    fn from(error: crate::dto::CommandErrorDto) -> Self {
        Self {
            error,
            source_id: None,
        }
    }
}

fn rejected_source(error: &hmm_app::ReplacementWorkflowError) -> Option<String> {
    match error {
        hmm_app::ReplacementWorkflowError::Analysis(hmm_app::ReplacementServiceError::Adapter(
            hmm_ports::ReplacementAdapterError::SourceAnalysisRejected { source_id, .. },
        )) => Some(source_id.as_str().to_owned()),
        _ => None,
    }
}

impl From<hmm_app::ReplacementWorkflowError> for EquipmentRetargetPreviewErrorDto {
    fn from(error: hmm_app::ReplacementWorkflowError) -> Self {
        let source_id = rejected_source(&error);
        Self {
            error: crate::replacement_commands::replacement_workflow_error_to_command_error(error),
            source_id,
        }
    }
}

impl From<hmm_runtime::ConfiguredRetargetReinstallError> for EquipmentRetargetPreviewErrorDto {
    fn from(error: hmm_runtime::ConfiguredRetargetReinstallError) -> Self {
        let source_id = match &error {
            hmm_runtime::ConfiguredRetargetReinstallError::Replacement(error) => {
                rejected_source(error)
            }
            _ => None,
        };
        Self {
            error: crate::replacement_commands::retarget_reinstall_error_to_command_error(error),
            source_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentReapplyRequestDto {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartEquipmentReapplyRequestDto {
    pub selection: EquipmentReapplyRequestDto,
    pub plan_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum EquipmentSlotIntentDto {
    Keep {
        #[serde(rename = "sourceId")]
        source_id: String,
    },
    Retarget {
        #[serde(rename = "sourceId")]
        source_id: String,
        #[serde(rename = "targetId")]
        target_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentRetargetSelectionRequestDto {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub slots: Vec<EquipmentSlotIntentDto>,
    pub layer_name: String,
    pub layer_priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartEquipmentRetargetReinstallRequestDto {
    pub selection: EquipmentRetargetSelectionRequestDto,
    pub plan_token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentSourceConfigurationDto {
    pub source: ReplacementSourceDto,
    pub original_target_id: Option<String>,
    pub targets: Vec<ReplacementTargetDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentRetargetConfigurationDto {
    pub game_id: String,
    pub mod_id: String,
    pub sources: Vec<EquipmentSourceConfigurationDto>,
    pub installed_targets: Option<BTreeMap<String, String>>,
    pub warnings: Vec<ReplacementWarningDto>,
}

impl From<hmm_app::EquipmentRetargetConfiguration> for EquipmentRetargetConfigurationDto {
    fn from(configuration: hmm_app::EquipmentRetargetConfiguration) -> Self {
        Self {
            game_id: configuration.game_id.as_str().to_owned(),
            mod_id: configuration.mod_id.as_str().to_owned(),
            sources: configuration
                .sources
                .into_iter()
                .map(|item| EquipmentSourceConfigurationDto {
                    source: ReplacementSourceDto {
                        id: item.source.id().as_str().to_owned(),
                        source_type: item.source.source_type().as_str().to_owned(),
                        internal_id: item.source.internal_id().to_owned(),
                        supported: item.source.is_supported(),
                        display_names: item.display_names,
                    },
                    original_target_id: item
                        .original_target_id
                        .map(|target| target.as_str().to_owned()),
                    targets: item.targets.into_iter().map(Into::into).collect(),
                })
                .collect(),
            installed_targets: configuration.installed_targets.map(|targets| {
                targets
                    .into_iter()
                    .map(|(source, target)| {
                        (source.as_str().to_owned(), target.as_str().to_owned())
                    })
                    .collect()
            }),
            warnings: configuration
                .analysis
                .warnings()
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentRetargetInstallPreviewDto {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub file_effects: Vec<crate::retarget_file_dto::RetargetFilePreviewDto>,
    pub analysis: ReplacementAnalysisDto,
    pub targets: Vec<ReplacementTargetDto>,
    pub warnings: Vec<ReplacementWarningDto>,
    pub install_plan: InstallPlanPreviewDto,
    pub prerequisite_decision: GamePrerequisiteDecisionDto,
}

impl From<hmm_app::InitialRetargetInstallPreflight> for EquipmentRetargetInstallPreviewDto {
    fn from(preflight: hmm_app::InitialRetargetInstallPreflight) -> Self {
        Self {
            file_effects: preflight
                .planned
                .file_effects()
                .into_iter()
                .map(Into::into)
                .collect(),
            analysis: preflight.planned.analysis().clone().into(),
            targets: preflight
                .planned
                .targets()
                .iter()
                .cloned()
                .map(Into::into)
                .collect(),
            warnings: preflight
                .planned
                .retarget_plans()
                .iter()
                .flat_map(|plan| plan.warnings())
                .copied()
                .map(Into::into)
                .collect(),
            install_plan: preflight.planned.install_plan().clone().into(),
            prerequisite_decision: preflight.prerequisite_decision.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reapply_requests_accept_only_scope_and_a_start_token() {
        let scope = json!({"gameId":"mhw", "profileId":"default", "modId":"fixture-mod"});
        serde_json::from_value::<EquipmentReapplyRequestDto>(scope.clone()).unwrap();
        for field in [
            "slots",
            "targetId",
            "revisionId",
            "layer",
            "layerName",
            "intent",
            "originalInstallEvidence",
            "fileEffects",
            "gameRoot",
        ] {
            let mut value = scope.clone();
            value[field] = json!("caller-supplied");
            assert!(
                serde_json::from_value::<EquipmentReapplyRequestDto>(value).is_err(),
                "accepted {field}"
            );
            for nested in [false, true] {
                let mut start = json!({"selection":scope, "planToken":"fixture-token"});
                if nested {
                    start["selection"][field] = json!("caller-supplied");
                } else {
                    start[field] = json!("caller-supplied");
                }
                assert!(serde_json::from_value::<StartEquipmentReapplyRequestDto>(start).is_err());
            }
        }
        serde_json::from_value::<StartEquipmentReapplyRequestDto>(
            json!({"selection":scope, "planToken":"fixture-token"}),
        )
        .unwrap();
    }

    #[test]
    fn equipment_preview_errors_identify_only_the_rejected_source() {
        let error =
            hmm_app::ReplacementWorkflowError::Analysis(hmm_app::ReplacementServiceError::Adapter(
                hmm_ports::ReplacementAdapterError::SourceAnalysisRejected {
                    source_id: hmm_core::ReplacementSourceId::parse("fixture-source").unwrap(),
                    code: "weapon_no_relocatable_resources",
                },
            ));
        let initial =
            serde_json::to_value(EquipmentRetargetPreviewErrorDto::from(error.clone())).unwrap();
        let reinstall = serde_json::to_value(EquipmentRetargetPreviewErrorDto::from(
            hmm_runtime::ConfiguredRetargetReinstallError::Replacement(error),
        ))
        .unwrap();
        assert_eq!(
            initial,
            json!({
                "code": "weapon_no_relocatable_resources",
                "message": "replacement analysis is unavailable",
                "sourceId": "fixture-source",
            })
        );
        assert_eq!(reinstall, initial);
        let generic = serde_json::to_value(EquipmentRetargetPreviewErrorDto::from(
            hmm_app::ReplacementWorkflowError::PlanUnavailable,
        ))
        .unwrap();
        assert!(generic.get("sourceId").is_none());
    }

    #[test]
    fn slots_accept_only_stable_ids_and_an_explicit_action() {
        let keep: EquipmentSlotIntentDto =
            serde_json::from_value(json!({"action":"keep","sourceId":"source-a"})).unwrap();
        assert_eq!(
            keep,
            EquipmentSlotIntentDto::Keep {
                source_id: "source-a".into()
            }
        );
        for value in [
            json!({"action":"keep","sourceId":"source-a","targetId":"target-b"}),
            json!({"action":"retarget","sourceId":"source-a"}),
            json!({"action":"retarget","sourceId":"source-a","targetId":"target-b","path":"nativePC/custom"}),
            json!({"action":"delete","sourceId":"source-a"}),
        ] {
            assert!(serde_json::from_value::<EquipmentSlotIntentDto>(value).is_err());
        }
    }

    #[test]
    fn equipment_requests_reject_paths_and_caller_supplied_installation_facts() {
        let request = json!({"gameId":"mhw","profileId":"default","modId":"mod",
            "slots":[{"action":"keep","sourceId":"source"}],"layerName":"base","layerPriority":0});
        assert!(
            serde_json::from_value::<EquipmentRetargetSelectionRequestDto>(request.clone()).is_ok()
        );
        for key in [
            "sourcePath",
            "targetPath",
            "gameRoot",
            "bindingId",
            "revisionId",
            "packageId",
            "originalInstallEvidence",
            "original_install_evidence",
            "policyExclusions",
            "attachmentCounts",
            "retainedAttachments",
        ] {
            let mut injected = request.clone();
            injected[key] = json!("not-accepted");
            assert!(
                serde_json::from_value::<EquipmentRetargetSelectionRequestDto>(injected).is_err(),
                "accepted {key}"
            );
        }
        let start = json!({"selection": request, "planToken": "fixture-preview"});
        serde_json::from_value::<StartEquipmentRetargetReinstallRequestDto>(start.clone()).unwrap();
        for field in ["originalInstallEvidence", "original_install_evidence"] {
            for nested in [false, true] {
                let mut injected = start.clone();
                let object = if nested {
                    &mut injected["selection"]
                } else {
                    &mut injected
                };
                object[field] = json!({"forged": true});
                assert!(
                    serde_json::from_value::<StartEquipmentRetargetReinstallRequestDto>(injected)
                        .unwrap_err()
                        .to_string()
                        .contains("unknown field")
                );
            }
        }
    }

    #[test]
    fn original_only_identity_never_projects_a_fabricated_equipment_name() {
        use hmm_ports::ReplacementCatalogProvider;
        let source = hmm_core::ReplacementSource::new(
            hmm_core::ReplacementSourceId::parse("fixture-source").unwrap(),
            hmm_core::GameId::mhw(),
            hmm_core::ReplacementTargetKind::parse("weapon").unwrap(),
            "one999",
            "wp/one",
            true,
        )
        .unwrap();
        let original = hmm_games_mhw::MhwReplacementCatalog
            .original_target_for_source(&source)
            .unwrap();
        let dto = ReplacementTargetDto::from(original);
        assert_eq!(dto.internal_id, "one999");
        assert!(dto.display_names.is_empty());
        assert!(dto.aliases.is_empty());
    }
}
