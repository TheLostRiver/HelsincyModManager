use super::{
    inventory::is_texture, material_table::MAX_MATERIAL_BYTES, material_transform, path_strategy,
};
use hmm_core::{InstallTargetPath, RetargetPlan};
use hmm_ports::{ReplacementAdapterError, ReplacementAdapterResult, ReplacementAssetContentReader};
use std::collections::{BTreeMap, BTreeSet};

fn moved_textures(plans: &[RetargetPlan]) -> BTreeMap<String, InstallTargetPath> {
    plans
        .iter()
        .flat_map(|plan| plan.actions())
        .filter(|action| {
            is_texture(action.source_relative_path())
                && action.source_relative_path().windows_key()
                    != action.target_relative_path().windows_key()
        })
        .map(|action| {
            (
                action.source_relative_path().windows_key(),
                action.target_relative_path().clone(),
            )
        })
        .collect()
}

fn is_material(path: &InstallTargetPath) -> bool {
    path.as_str()
        .trim_end_matches(['.', ' '])
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("mrl3"))
}

pub(super) fn requires_content(plans: &[RetargetPlan]) -> bool {
    !moved_textures(plans).is_empty()
        && plans
            .iter()
            .flat_map(|plan| plan.actions())
            .any(|action| is_material(action.source_relative_path()))
}

pub(super) fn complete(
    plans: Vec<RetargetPlan>,
    reader: &dyn ReplacementAssetContentReader,
) -> ReplacementAdapterResult<Vec<RetargetPlan>> {
    let mut sources = BTreeSet::new();
    let mut files = BTreeSet::new();
    for plan in &plans {
        if !sources.insert(plan.source().id())
            || plan
                .actions()
                .iter()
                .any(|action| !files.insert(action.source_relative_path().windows_key()))
        {
            return Err(ReplacementAdapterError::InvalidRetargetPlan);
        }
    }
    let destinations = moved_textures(&plans);
    if destinations.is_empty() {
        return Ok(plans);
    }
    plans
        .into_iter()
        .map(|plan| {
            let mut actions = Vec::with_capacity(plan.actions().len());
            for action in plan.actions() {
                let mut action = action.clone();
                if is_material(action.source_relative_path()) {
                    let bytes =
                        reader.read_asset_content(action.package_file_id(), MAX_MATERIAL_BYTES)?;
                    let transform =
                        material_transform::invocation(&bytes, &destinations).map_err(|code| {
                            ReplacementAdapterError::SourceAnalysisRejected {
                                source_id: plan.source().id().clone(),
                                code,
                            }
                        })?;
                    if let Some(transform) = transform {
                        action = action.with_content_transform(transform);
                    }
                }
                actions.push(action);
            }
            let updated = RetargetPlan::new(
                plan.binding().clone(),
                plan.source().clone(),
                actions,
                plan.warnings().to_vec(),
            )
            .and_then(|updated| updated.with_policy_exclusions(plan.policy_exclusions().to_vec()))
            .and_then(|updated| updated.with_file_effects(plan.file_effects().to_vec()))
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?;
            path_strategy::with_facts(
                updated,
                plan.adapter_facts()
                    .map_or(0, |facts| facts.excluded_file_count()),
            )
        })
        .collect()
}
