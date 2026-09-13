use super::*;
use hmm_core::{
    FileLayer, InstallFileProvider, InstallPlan, RetargetFileDisposition, RetargetFileEffect,
    RetargetFileReason,
};

impl PluginSelectionService {
    pub fn has_policy_files(&self, plan: &InstallPlan, game_id: &GameId) -> bool {
        self.sources
            .policies
            .iter()
            .filter(|policy| policy.game_id() == *game_id)
            .any(|policy| {
                plan.actions
                    .iter()
                    .map(|action| &action.provider)
                    .chain(
                        plan.conflicts
                            .iter()
                            .flat_map(|conflict| &conflict.providers),
                    )
                    .any(|provider| {
                        policy.is_candidate(&provider.target_path)
                            || policy.is_excluded_attachment(&provider.target_path)
                    })
            })
    }

    pub fn validate_plan_coverage(
        &self,
        plan: &InstallPlan,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<()> {
        plan.validate_plugin_selections(game_id, profile_id)
            .map_err(|_| PluginSelectionServiceError::InvalidSelection)?;
        let Some(policy) = self
            .sources
            .policies
            .iter()
            .find(|policy| policy.game_id() == *game_id)
        else {
            return if plan.plugin_selections.is_empty() {
                Ok(())
            } else {
                Err(PluginSelectionServiceError::InvalidSelection)
            };
        };
        for action in &plan.actions {
            if !policy.is_candidate(&action.target_path)
                && !policy.is_excluded_attachment(&action.target_path)
            {
                continue;
            }
            let covered = plan
                .plugin_selections
                .iter()
                .filter(|selection| selection.scope().mod_id == action.provider.mod_id)
                .flat_map(|selection| selection.files())
                .any(|file| {
                    file.package_file_id == action.provider.package_file_id
                        && file.choice.is_included()
                        && file
                            .target_path
                            .as_str()
                            .eq_ignore_ascii_case(action.target_path.as_str())
                });
            if !covered {
                return Err(PluginSelectionServiceError::InvalidSelection);
            }
        }
        Ok(())
    }

    /// 所有入口共用的文件策略投影；调用方继续执行最终跨 Mod 冲突检查和原有事务。
    pub fn apply_to_plan(
        &self,
        scope: &PluginSelectionScope,
        layer: &FileLayer,
        preserve_layers: bool,
        plan: &mut InstallPlan,
    ) -> Result<Option<PluginInventory>> {
        let inventory = self.inventory(scope)?;
        if let Some(inventory) = &inventory {
            let manifest = self.manifest(&scope.profile_id)?;
            apply_selection(
                &inventory.selection,
                layer,
                preserve_layers.then_some(manifest.as_ref()).flatten(),
                plan,
            )?;
        } else {
            plan.plugin_selections
                .retain(|selection| selection.scope().mod_id != scope.mod_id);
        }
        Ok(inventory)
    }

    /// 暂存重建仅携带已经盘点的选择事实，不再次改变选择，也不从缺失的暂存猜原包来源。
    pub fn apply_planned_selections(
        &self,
        selections: &[PluginSelectionSnapshot],
        layer: &FileLayer,
        preserve_layers: bool,
        plan: &mut InstallPlan,
    ) -> Result<()> {
        for selection in selections {
            let manifest = self.manifest(&selection.scope().profile_id)?;
            apply_selection(
                selection,
                layer,
                preserve_layers.then_some(manifest.as_ref()).flatten(),
                plan,
            )?;
        }
        Ok(())
    }

    pub fn file_effects(&self, selections: &[PluginSelectionSnapshot]) -> Vec<RetargetFileEffect> {
        selections
            .iter()
            .flat_map(|selection| {
                selection.files().iter().map(|file| {
                    let is_plugin = self.sources.policies.iter().any(|policy| {
                        policy.game_id() == selection.scope().game_id
                            && policy.is_candidate(&file.target_path)
                    });
                    RetargetFileEffect {
                        package_file_id: file.package_file_id.clone(),
                        source_id: None,
                        source_path: file.target_path.clone(),
                        target_path: file.choice.is_included().then(|| file.target_path.clone()),
                        disposition: match file.choice {
                            PluginFileChoiceKind::Include => {
                                RetargetFileDisposition::PackageCompanion
                            }
                            PluginFileChoiceKind::RetainInstalled => {
                                RetargetFileDisposition::InstalledAttachmentRetained
                            }
                            PluginFileChoiceKind::Exclude if is_plugin => {
                                RetargetFileDisposition::PluginCandidate
                            }
                            PluginFileChoiceKind::Exclude => {
                                RetargetFileDisposition::PolicyExcluded
                            }
                        },
                        reason: match file.choice {
                            PluginFileChoiceKind::Include => RetargetFileReason::PluginSelected,
                            PluginFileChoiceKind::RetainInstalled => {
                                RetargetFileReason::InstalledAttachment
                            }
                            PluginFileChoiceKind::Exclude if is_plugin => {
                                RetargetFileReason::PluginNotIncluded
                            }
                            PluginFileChoiceKind::Exclude => RetargetFileReason::ExecutablePolicy,
                        },
                    }
                })
            })
            .collect()
    }
}

fn apply_selection(
    selection: &PluginSelectionSnapshot,
    layer: &FileLayer,
    manifest: Option<&InstallManifest>,
    plan: &mut InstallPlan,
) -> Result<()> {
    let mod_id = &selection.scope().mod_id;
    let choices = selection
        .files()
        .iter()
        .map(|file| (&file.package_file_id, file))
        .collect::<std::collections::BTreeMap<_, _>>();
    let is_selected_file = |provider: &InstallFileProvider| {
        provider.mod_id == *mod_id && choices.contains_key(&provider.package_file_id)
    };
    let mut providers = plan
        .actions
        .iter()
        .filter(|action| !is_selected_file(&action.provider))
        .map(|action| action.provider.clone())
        .collect::<Vec<_>>();
    let mut kept_conflicts = Vec::new();
    let mut conflicting_ids = BTreeSet::new();
    for conflict in &plan.conflicts {
        let mut retained = conflict.clone();
        retained.providers.retain(|provider| {
            if provider.mod_id != *mod_id {
                return true;
            }
            choices
                .get(&provider.package_file_id)
                .is_none_or(|file| file.choice.is_included())
        });
        if retained.providers.len() >= 2 {
            conflicting_ids.extend(
                retained
                    .providers
                    .iter()
                    .filter(|provider| is_selected_file(provider))
                    .map(|provider| provider.package_file_id.clone()),
            );
            kept_conflicts.push(retained);
        } else if let Some(provider) = retained
            .providers
            .first()
            .filter(|provider| provider.mod_id == *mod_id && !is_selected_file(provider))
        {
            providers.push(provider.clone());
        }
    }
    for file in selection
        .files()
        .iter()
        .filter(|file| file.choice.is_included())
    {
        if conflicting_ids.contains(&file.package_file_id) {
            continue;
        }
        let existing_layer = manifest
            .and_then(|manifest| {
                manifest.entries.iter().find(|entry| {
                    entry.mod_id == *mod_id
                        && entry.package_file_id == file.package_file_id
                        && entry
                            .revision_id
                            .as_ref()
                            .is_none_or(|revision| revision == &selection.scope().revision_id)
                })
            })
            .map(|entry| &entry.layer);
        providers.push(InstallFileProvider::new(
            mod_id.clone(),
            file.package_file_id.clone(),
            file.target_path.clone(),
            existing_layer.unwrap_or(layer).clone(),
        ));
    }
    let mut rebuilt = InstallPlan::from_providers(providers);
    rebuilt.conflicts.extend(kept_conflicts);
    rebuilt.replacement_bindings = plan.replacement_bindings.clone();
    rebuilt.plugin_selections = plan
        .plugin_selections
        .iter()
        .filter(|other| other.scope().mod_id != *mod_id)
        .cloned()
        .collect();
    rebuilt.plugin_selections.push(selection.clone());
    rebuilt
        .validate_plugin_selections(&selection.scope().game_id, &selection.scope().profile_id)
        .map_err(|_| PluginSelectionServiceError::InvalidSelection)?;
    *plan = rebuilt;
    Ok(())
}
