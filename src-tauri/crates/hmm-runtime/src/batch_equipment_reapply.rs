use super::*;
use hmm_app::{
    EquipmentRetargetReinstallRequest, InstalledEquipmentReinstallResolution,
    PreviewEquipmentRetargetReinstallRequest,
};

impl ReadOnlyInstallAutomation {
    pub(crate) fn resolve_batch_equipment_reapply(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        input: &ReinstallBatchItemInput,
    ) -> anyhow::Result<FileLayer> {
        anyhow::ensure!(
            input.intent == hmm_core::ReinstallIntent::ReapplyEquipmentTargets
                && input.installed_revision_id == input.candidate_revision_id
                && input.replacement_binding_snapshot.is_none(),
            "invalid equipment reapply identity"
        );
        let preview = self.reinstall_preview_service(game_id)?;
        let InstalledEquipmentReinstallResolution::Ready(context) =
            preview.resolve_installed_equipment_context(game_id, profile_id, &input.mod_id)?
        else {
            anyhow::bail!("installed equipment context is unavailable");
        };
        anyhow::ensure!(
            context.installed_revision_id == input.installed_revision_id,
            "installed revision changed"
        );
        let planned = self.replacement_workflow.preview_equipment_reinstall(
            PreviewEquipmentRetargetReinstallRequest {
                selection: EquipmentRetargetReinstallRequest::reapply(
                    game_id.clone(),
                    profile_id.clone(),
                    input.mod_id.clone(),
                ),
                installed_revision_id: context.installed_revision_id,
                installed_bindings: context.installed_bindings,
            },
        )?;
        Ok(planned.layer().clone())
    }
}

impl ReadOnlyBatchReinstallItemFactsReader {
    pub(super) fn read_reapply_facts(
        &self,
        request: &BatchReinstallItemFactsRequest,
    ) -> anyhow::Result<BatchItemFacts> {
        anyhow::ensure!(
            request.input.installed_revision_id == request.input.candidate_revision_id
                && request.input.replacement_binding_snapshot.is_none(),
            "invalid equipment reapply identity"
        );
        let context = self.preview.resolve_installed_equipment_context(
            &request.game_id,
            &request.profile_id,
            &request.input.mod_id,
        )?;
        let context = match context {
            InstalledEquipmentReinstallResolution::Ready(context)
                if context.installed_revision_id == request.input.installed_revision_id =>
            {
                context
            }
            InstalledEquipmentReinstallResolution::Ready(_) => {
                return ReinstallPreviewBatchItemFactsReader::facts_from_preparation(
                    request,
                    ReinstallPreparation::Blocked(blocked_reinstall_preview(
                        &self.preview,
                        request,
                        ReinstallBlockingReason::InstalledRevisionUnknown,
                    )),
                )
            }
            InstalledEquipmentReinstallResolution::Blocked(preview) => {
                return ReinstallPreviewBatchItemFactsReader::facts_from_preparation(
                    request,
                    ReinstallPreparation::Blocked(preview),
                )
            }
        };
        let planned = self.replacement_workflow.preview_equipment_reinstall(
            PreviewEquipmentRetargetReinstallRequest {
                selection: EquipmentRetargetReinstallRequest::reapply(
                    request.game_id.clone(),
                    request.profile_id.clone(),
                    request.input.mod_id.clone(),
                ),
                installed_revision_id: context.installed_revision_id.clone(),
                installed_bindings: context.installed_bindings,
            },
        )?;
        anyhow::ensure!(
            planned
                .retarget_plans()
                .iter()
                .flat_map(|plan| plan.actions())
                .all(|action| action.content_transform().is_none()),
            "read-only reapply requires untransformed source facts"
        );
        let preparation = self
            .preview
            .prepare_equipment_with_intent(
                ReinstallPreviewRequest {
                    game_id: request.game_id.clone(),
                    profile_id: request.profile_id.clone(),
                    mod_id: request.input.mod_id.clone(),
                    candidate_revision_id: context.installed_revision_id,
                    layer: planned.layer().clone(),
                },
                planned.install_plan().clone(),
                context.original_install_evidence,
                planned.policy_exclusions(),
                request.input.intent,
            )?
            .with_file_effects(planned.file_effects())?
            .with_untransformed_reapply_sources()?;
        ReinstallPreviewBatchItemFactsReader::facts_from_preparation(request, preparation)
    }
}
