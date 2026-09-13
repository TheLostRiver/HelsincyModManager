use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginSelectionQueryDto {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub revision_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetPluginSelectionDto {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub revision_id: String,
    pub inventory_id: String,
    pub selected_file_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInventoryDto {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub revision_id: String,
    pub inventory_id: String,
    pub confirmation_required: bool,
    pub files: Vec<PluginCandidateDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCandidateDto {
    pub file_id: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub check: &'static str,
    pub selected: bool,
    pub selectable: bool,
    pub managed: bool,
    pub retain_only: bool,
    pub excluded_by_package: bool,
}

impl From<hmm_app::PluginInventory> for PluginInventoryDto {
    fn from(inventory: hmm_app::PluginInventory) -> Self {
        let scope = inventory.selection.scope();
        Self {
            game_id: scope.game_id.as_str().to_owned(),
            profile_id: scope.profile_id.as_str().to_owned(),
            mod_id: scope.mod_id.as_str().to_owned(),
            revision_id: scope.revision_id.as_str().to_owned(),
            inventory_id: inventory.selection.inventory_id(),
            confirmation_required: inventory.confirmation_required,
            files: inventory
                .candidates
                .into_iter()
                .map(|candidate| PluginCandidateDto {
                    file_id: candidate.package_file_id.as_str().to_owned(),
                    relative_path: candidate.target_path.as_str().to_owned(),
                    size_bytes: candidate.size_bytes,
                    check: candidate.check.code(),
                    selected: candidate.selected,
                    selectable: candidate.selectable,
                    managed: candidate.installed,
                    retain_only: candidate.retain_only,
                    excluded_by_package: candidate.excluded_by_package,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plugin_requests_accept_only_scope_inventory_and_file_id_selection() {
        let query = json!({"gameId":"mhw","profileId":"profile","modId":"mod"});
        serde_json::from_value::<PluginSelectionQueryDto>(query.clone()).unwrap();
        let set = json!({"gameId":"mhw","profileId":"profile","modId":"mod","revisionId":"revision",
            "inventoryId":"inventory","selectedFileIds":["file"]});
        serde_json::from_value::<SetPluginSelectionDto>(set.clone()).unwrap();
        for key in [
            "gameRoot",
            "sourcePath",
            "targetPath",
            "bytes",
            "sha256",
            "confirmed",
            "force",
            "selectionSnapshot",
        ] {
            let mut value = set.clone();
            value[key] = json!("not-accepted");
            assert!(serde_json::from_value::<SetPluginSelectionDto>(value).is_err());
            let mut value = query.clone();
            value[key] = json!("not-accepted");
            assert!(serde_json::from_value::<PluginSelectionQueryDto>(value).is_err());
        }
    }
}
