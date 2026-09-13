use super::*;
use hmm_core::{PluginSelectionScope, PluginSelectionSnapshot, RetargetFileEffect};

impl ReplacementWorkflowService {
    pub fn plugin_file_effects(&self, plan: &InstallPlan) -> Vec<RetargetFileEffect> {
        self.plugin_selection
            .as_ref()
            .map(|service| service.file_effects(&plan.plugin_selections))
            .unwrap_or_default()
    }
    pub fn with_plugin_selection(mut self, service: Arc<crate::PluginSelectionService>) -> Self {
        self.plugin_selection = Some(service);
        self
    }

    pub fn apply_plugin_selection(
        &self,
        scope: PluginSelectionScope,
        layer: &FileLayer,
        preserve_layers: bool,
        plan: &mut InstallPlan,
    ) -> Result<Vec<RetargetFileEffect>, ReplacementWorkflowError> {
        let Some(service) = &self.plugin_selection else {
            return Ok(Vec::new());
        };
        service.apply_to_plan(&scope, layer, preserve_layers, plan)?;
        Ok(service.file_effects(&plan.plugin_selections))
    }

    pub fn ensure_plugin_choices_confirmed(
        &self,
        plan: &InstallPlan,
    ) -> Result<(), ReplacementWorkflowError> {
        self.ensure_plugin_snapshots_confirmed(&plan.plugin_selections)
    }

    pub fn ensure_plugin_snapshots_confirmed(
        &self,
        selections: &[PluginSelectionSnapshot],
    ) -> Result<(), ReplacementWorkflowError> {
        if let Some(service) = &self.plugin_selection {
            for selection in selections {
                service.ensure_confirmed(selection)?;
            }
        } else if !selections.is_empty() {
            return Err(crate::PluginSelectionServiceError::Unavailable.into());
        }
        Ok(())
    }

    pub fn record_approved_plugin_choices(
        &self,
        plan: &InstallPlan,
    ) -> Result<(), ReplacementWorkflowError> {
        if let Some(service) = &self.plugin_selection {
            service.record_approved_selections(&plan.plugin_selections)?;
        }
        Ok(())
    }

    pub(super) fn restore_planned_plugins(
        &self,
        selections: &[PluginSelectionSnapshot],
        layer: &FileLayer,
        preserve_layers: bool,
        plan: &mut InstallPlan,
    ) -> Result<(), ReplacementWorkflowError> {
        if let Some(service) = &self.plugin_selection {
            service.apply_planned_selections(selections, layer, preserve_layers, plan)?;
        } else if !selections.is_empty() {
            return Err(crate::PluginSelectionServiceError::Unavailable.into());
        }
        Ok(())
    }
}

pub(super) fn merge_file_effects(
    base: Vec<RetargetFileEffect>,
    plugins: &[RetargetFileEffect],
) -> Vec<RetargetFileEffect> {
    if plugins.is_empty() {
        return base;
    }
    let mut effects = base
        .into_iter()
        .map(|effect| (effect.package_file_id.clone(), effect))
        .collect::<std::collections::BTreeMap<_, _>>();
    for effect in plugins {
        effects.insert(effect.package_file_id.clone(), effect.clone());
    }
    effects.into_values().collect()
}
