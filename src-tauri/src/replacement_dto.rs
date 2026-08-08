use crate::dto::{GamePrerequisiteDecisionDto, InstallPlanPreviewDto};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListReplacementTargetsRequestDto {
    pub game_id: String,
    pub mod_id: String,
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalyzeImportedModReplacementRequestDto {
    pub game_id: String,
    #[serde(default)]
    pub profile_id: Option<String>,
    pub mod_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewInitialRetargetInstallRequestDto {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub target_id: String,
    pub layer_name: String,
    pub layer_priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartRetargetInstallTaskRequestDto {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub target_id: String,
    pub layer_name: String,
    pub layer_priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewRetargetReinstallRequestDto {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub target_id: String,
    pub layer_name: String,
    pub layer_priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartRetargetReinstallTaskRequestDto {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub target_id: String,
    pub layer_name: String,
    pub layer_priority: i32,
    pub plan_token: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplacementTargetDto {
    pub id: String,
    pub game_id: String,
    pub target_type: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary_name: Option<String>,
    pub aliases: Vec<String>,
    pub internal_id: String,
    pub catalog_scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplacementSourceDto {
    pub id: String,
    pub source_type: String,
    pub internal_id: String,
    pub supported: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplacementWarningDto {
    NoSupportedAssets,
    MultipleSources,
    UnsupportedSource,
    SourceMatchesTarget,
    WeaponPartialPartSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplacementAnalysisDto {
    pub game_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_target_id: Option<String>,
    pub retargetable: bool,
    pub matched_asset_count: usize,
    pub sources: Vec<ReplacementSourceDto>,
    pub warnings: Vec<ReplacementWarningDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetargetActionPreviewDto {
    pub source_internal_id: String,
    pub target_internal_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialRetargetInstallPreviewDto {
    pub analysis: ReplacementAnalysisDto,
    pub target: ReplacementTargetDto,
    pub actions: Vec<RetargetActionPreviewDto>,
    pub warnings: Vec<ReplacementWarningDto>,
    pub install_plan: InstallPlanPreviewDto,
    pub prerequisite_decision: GamePrerequisiteDecisionDto,
}

#[cfg(test)]
mod replacement_dto_tests {
    use super::*;
    use crate::dto::{GamePrerequisiteDecisionCodeDto, GamePrerequisiteDecisionStatusDto};
    use serde_json::json;

    #[test]
    fn target_list_request_requires_mod_identity_and_rejects_backend_paths() {
        let request: ListReplacementTargetsRequestDto = serde_json::from_value(json!({
            "gameId": "mhw",
            "modId": "weapon-mod",
            "query": "one"
        }))
        .expect("deserialize target list request");
        assert_eq!(request.mod_id, "weapon-mod");

        assert!(
            serde_json::from_value::<ListReplacementTargetsRequestDto>(json!({
                "gameId": "mhw",
                "modId": "weapon-mod",
                "sandboxPath": "C:\\private\\package"
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<ListReplacementTargetsRequestDto>(json!({
                "gameId": "mhw"
            }))
            .is_err()
        );
    }

    #[test]
    fn initial_retarget_requests_deserialize_stable_ids_without_backend_paths() {
        let preview: PreviewInitialRetargetInstallRequestDto = serde_json::from_value(json!({
            "gameId": "mhw",
            "profileId": "profile-a",
            "modId": "mod-a",
            "targetId": "mhw:armor:fatalis-alpha",
            "layerName": "base",
            "layerPriority": 0
        }))
        .expect("deserialize preview request");
        let start: StartRetargetInstallTaskRequestDto = serde_json::from_value(json!({
            "gameId": "mhw",
            "profileId": "profile-a",
            "modId": "mod-a",
            "targetId": "mhw:armor:fatalis-alpha",
            "layerName": "base",
            "layerPriority": 0
        }))
        .expect("deserialize start request");

        assert_eq!(preview.mod_id, "mod-a");
        assert_eq!(start.target_id, "mhw:armor:fatalis-alpha");
        let serialized = serde_json::to_value(start).expect("serialize request shape");
        for forbidden in [
            "packageId",
            "revisionId",
            "sourceId",
            "bindingId",
            "sandboxPath",
            "stagingPath",
            "targetPath",
        ] {
            assert!(
                serialized.get(forbidden).is_none(),
                "forbidden field: {forbidden}"
            );
        }
    }

    #[test]
    fn retarget_reinstall_requests_keep_revision_and_paths_backend_owned() {
        let preview: PreviewRetargetReinstallRequestDto = serde_json::from_value(json!({
            "gameId": "mhw",
            "profileId": "profile-a",
            "modId": "mod-a",
            "targetId": "mhw:armor:fatalis-alpha",
            "layerName": "base",
            "layerPriority": 0
        }))
        .expect("deserialize preview request");
        let start: StartRetargetReinstallTaskRequestDto = serde_json::from_value(json!({
            "gameId": "mhw",
            "profileId": "profile-a",
            "modId": "mod-a",
            "targetId": "mhw:armor:fatalis-alpha",
            "layerName": "base",
            "layerPriority": 0,
            "planToken": "reinstall-preview-v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        }))
        .expect("deserialize start request");

        assert_eq!(preview.target_id, "mhw:armor:fatalis-alpha");
        let serialized = serde_json::to_value(start).expect("serialize request shape");
        for forbidden in [
            "packageId",
            "revisionId",
            "sourceId",
            "bindingId",
            "sandboxPath",
            "stagingPath",
            "targetPath",
        ] {
            assert!(
                serialized.get(forbidden).is_none(),
                "forbidden field: {forbidden}"
            );
        }
        assert!(serialized["planToken"]
            .as_str()
            .expect("plan token")
            .starts_with("reinstall-preview-v1:"));
    }

    #[test]
    fn replacement_target_and_analysis_serialize_camel_case_with_snake_case_warnings() {
        let target = ReplacementTargetDto {
            id: "mhw:armor:fatalis-alpha".to_owned(),
            game_id: "mhw".to_owned(),
            target_type: "armor".to_owned(),
            display_name: "【精英‧龙α】服装".to_owned(),
            secondary_name: Some("Fatalis Alpha +".to_owned()),
            aliases: vec!["黑龙".to_owned()],
            internal_id: "pl129_0000".to_owned(),
            catalog_scope: "production".to_owned(),
        };
        let analysis = ReplacementAnalysisDto {
            game_id: "mhw".to_owned(),
            installed_target_id: Some("mhw:armor:fatalis-beta".to_owned()),
            retargetable: true,
            matched_asset_count: 1,
            sources: vec![ReplacementSourceDto {
                id: "mhw:armor:f_equip:pl121_0000".to_owned(),
                source_type: "armor".to_owned(),
                internal_id: "pl121_0000".to_owned(),
                supported: true,
            }],
            warnings: vec![ReplacementWarningDto::SourceMatchesTarget],
        };

        let target_value = serde_json::to_value(target).expect("serialize target");
        let analysis_value = serde_json::to_value(analysis).expect("serialize analysis");
        let action_value = serde_json::to_value(RetargetActionPreviewDto {
            source_internal_id: "one001".to_owned(),
            target_internal_id: "one002".to_owned(),
        })
        .expect("serialize action");
        assert_eq!(target_value["gameId"], "mhw");
        assert_eq!(target_value["secondaryName"], "Fatalis Alpha +");
        assert_eq!(target_value["catalogScope"], "production");
        assert!(target_value.get("metadata").is_none());
        assert_eq!(analysis_value["matchedAssetCount"], 1);
        assert_eq!(
            analysis_value["installedTargetId"],
            "mhw:armor:fatalis-beta"
        );
        assert_eq!(analysis_value["warnings"][0], "source_matches_target");
        assert!(!target_value.to_string().contains("nativePC"));
        assert_eq!(action_value["sourceInternalId"], "one001");
        assert_eq!(action_value["targetInternalId"], "one002");
        for forbidden in [
            "pathFamily",
            "sourceRelativePath",
            "targetRelativePath",
            "sourcePathFamily",
            "targetPathFamily",
        ] {
            assert!(analysis_value.get(forbidden).is_none());
            assert!(action_value.get(forbidden).is_none());
        }
    }

    #[test]
    fn replacement_analysis_request_accepts_only_stable_profile_and_mod_identity() {
        let request: AnalyzeImportedModReplacementRequestDto = serde_json::from_value(json!({
            "gameId": "mhw",
            "profileId": "profile-a",
            "modId": "mod-a"
        }))
        .expect("deserialize replacement analysis request");

        assert_eq!(request.profile_id.as_deref(), Some("profile-a"));
        let serialized = serde_json::to_value(request).expect("serialize request shape");
        for forbidden in [
            "packageId",
            "revisionId",
            "sourceId",
            "bindingId",
            "sandboxPath",
            "stagingPath",
            "targetPath",
        ] {
            assert!(
                serialized.get(forbidden).is_none(),
                "forbidden field: {forbidden}"
            );
        }

        let without_profile: AnalyzeImportedModReplacementRequestDto =
            serde_json::from_value(json!({
                "gameId": "mhw",
                "modId": "mod-a"
            }))
            .expect("profile is optional while setup is unavailable");
        assert_eq!(without_profile.profile_id, None);
    }

    #[test]
    fn initial_retarget_preview_serializes_prerequisite_decision_at_the_preflight_boundary() {
        let dto = InitialRetargetInstallPreviewDto {
            analysis: ReplacementAnalysisDto {
                game_id: "mhw".to_owned(),
                installed_target_id: None,
                retargetable: true,
                matched_asset_count: 1,
                sources: Vec::new(),
                warnings: Vec::new(),
            },
            target: ReplacementTargetDto {
                id: "mhw:armor:fatalis-alpha".to_owned(),
                game_id: "mhw".to_owned(),
                target_type: "armor".to_owned(),
                display_name: "Target".to_owned(),
                secondary_name: None,
                aliases: Vec::new(),
                internal_id: "pl129_0000".to_owned(),
                catalog_scope: "production".to_owned(),
            },
            actions: Vec::new(),
            warnings: Vec::new(),
            install_plan: InstallPlanPreviewDto {
                actions: Vec::new(),
                conflicts: Vec::new(),
                has_blocking_conflicts: false,
            },
            prerequisite_decision: GamePrerequisiteDecisionDto {
                status: GamePrerequisiteDecisionStatusDto::Warning,
                rules_version: Some(1),
                codes: vec![GamePrerequisiteDecisionCodeDto::SignatureUnverified],
            },
        };

        let value = serde_json::to_value(dto).expect("serialize initial retarget preview");
        assert_eq!(value["prerequisiteDecision"]["status"], "warning");
        assert_eq!(value["prerequisiteDecision"]["rulesVersion"], 1);
        assert_eq!(
            value["prerequisiteDecision"]["codes"],
            json!(["signature_unverified"])
        );
        assert!(
            value["installPlan"].get("prerequisiteDecision").is_none(),
            "generic nested InstallPlan must not fabricate lifecycle prerequisite facts"
        );
        assert!(!value.to_string().contains(r"C:\Users\fixture"));
    }
}
