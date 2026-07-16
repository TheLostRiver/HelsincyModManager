use crate::dto::InstallPlanPreviewDto;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListReplacementTargetsRequestDto {
    pub game_id: String,
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalyzeImportedModReplacementRequestDto {
    pub game_id: String,
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
    pub metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplacementSourceDto {
    pub id: String,
    pub source_type: String,
    pub internal_id: String,
    pub path_family: String,
    pub supported: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplacementWarningDto {
    NoSupportedAssets,
    MultipleSources,
    UnsupportedSource,
    SourceMatchesTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplacementAnalysisDto {
    pub game_id: String,
    pub retargetable: bool,
    pub matched_asset_count: usize,
    pub sources: Vec<ReplacementSourceDto>,
    pub warnings: Vec<ReplacementWarningDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetargetActionPreviewDto {
    pub source_relative_path: String,
    pub target_relative_path: String,
    pub source_internal_id: String,
    pub target_internal_id: String,
    pub source_path_family: String,
    pub target_path_family: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialRetargetInstallPreviewDto {
    pub analysis: ReplacementAnalysisDto,
    pub target: ReplacementTargetDto,
    pub actions: Vec<RetargetActionPreviewDto>,
    pub warnings: Vec<ReplacementWarningDto>,
    pub install_plan: InstallPlanPreviewDto,
}

#[cfg(test)]
mod replacement_dto_tests {
    use super::*;
    use serde_json::json;

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
    fn replacement_target_and_analysis_serialize_camel_case_with_snake_case_warnings() {
        let target = ReplacementTargetDto {
            id: "mhw:armor:fatalis-alpha".to_owned(),
            game_id: "mhw".to_owned(),
            target_type: "armor".to_owned(),
            display_name: "【精英‧龙α】服装".to_owned(),
            secondary_name: Some("Fatalis Alpha +".to_owned()),
            aliases: vec!["黑龙".to_owned()],
            internal_id: "pl129_0000".to_owned(),
            metadata: BTreeMap::from([("rank".to_owned(), json!("master"))]),
        };
        let analysis = ReplacementAnalysisDto {
            game_id: "mhw".to_owned(),
            retargetable: true,
            matched_asset_count: 1,
            sources: vec![ReplacementSourceDto {
                id: "mhw:armor:f_equip:pl121_0000".to_owned(),
                source_type: "armor".to_owned(),
                internal_id: "pl121_0000".to_owned(),
                path_family: "pl/f_equip".to_owned(),
                supported: true,
            }],
            warnings: vec![ReplacementWarningDto::SourceMatchesTarget],
        };

        let target_value = serde_json::to_value(target).expect("serialize target");
        let analysis_value = serde_json::to_value(analysis).expect("serialize analysis");
        assert_eq!(target_value["gameId"], "mhw");
        assert_eq!(target_value["secondaryName"], "Fatalis Alpha +");
        assert_eq!(analysis_value["matchedAssetCount"], 1);
        assert_eq!(analysis_value["warnings"][0], "source_matches_target");
        assert!(!target_value.to_string().contains("nativePC"));
    }
}
