use super::*;
use hmm_app::EquipmentRetargetReinstallRequest;

impl ConfiguredReinstallExecutor {
    pub(super) fn prepare_unbound_plugin_reapply(
        &self,
        request: &EquipmentRetargetReinstallRequest,
    ) -> Result<Option<ConfiguredRetargetReinstallPreparation>, ConfiguredRetargetReinstallError>
    {
        if !request.slots.is_empty() {
            return Err(ConfiguredRetargetReinstallError::Replacement(
                ReplacementWorkflowError::SourceNotRetargetable,
            ));
        }
        let services = self
            .services_for(&request.game_id)
            .map_err(ConfiguredRetargetReinstallError::Reinstall)?;
        let preparation = services
            .preview
            .prepare_unbound_plugin_reapply(
                &request.game_id,
                &request.profile_id,
                &request.mod_id,
                &request.layer,
                &self.replacement_workflow,
            )
            .map_err(ConfiguredRetargetReinstallError::Reinstall)?;
        Ok(
            preparation.map(|preparation| ConfiguredRetargetReinstallPreparation {
                preparation,
                game_instance: services.game_instance,
                source: Arc::clone(&self.source),
                staging_cleanup: RetargetStagingCleanup::default(),
            }),
        )
    }
}
