use crate::dto::{GamePrerequisiteDecisionDto, InstallPlanPreviewDto};
use crate::replacement_dto::{
    ReplacementAnalysisDto, ReplacementSourceDto, ReplacementTargetDto, ReplacementWarningDto,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    pub analysis: ReplacementAnalysisDto,
    pub targets: Vec<ReplacementTargetDto>,
    pub warnings: Vec<ReplacementWarningDto>,
    pub install_plan: InstallPlanPreviewDto,
    pub prerequisite_decision: GamePrerequisiteDecisionDto,
}

impl From<hmm_app::InitialRetargetInstallPreflight> for EquipmentRetargetInstallPreviewDto {
    fn from(preflight: hmm_app::InitialRetargetInstallPreflight) -> Self {
        Self {
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
        ] {
            let mut injected = request.clone();
            injected[key] = json!("not-accepted");
            assert!(
                serde_json::from_value::<EquipmentRetargetSelectionRequestDto>(injected).is_err(),
                "accepted {key}"
            );
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
