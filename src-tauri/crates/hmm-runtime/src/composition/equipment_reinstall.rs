use super::*;
use hmm_app::{
    EquipmentRetargetReinstallRequest, EquipmentRetargetReinstallTaskExecutor,
    InstalledEquipmentReinstallResolution, PreviewEquipmentRetargetReinstallRequest,
};

impl ConfiguredReinstallExecutor {
    pub fn preview_equipment_retarget_reinstall(
        &self,
        request: EquipmentRetargetReinstallRequest,
    ) -> Result<ReinstallPlanPreview, ConfiguredRetargetReinstallError> {
        self.prepare_equipment_selection(request)
            .map(|prepared| prepared.preparation.into_preview())
    }

    fn prepare_equipment_selection(
        &self,
        request: EquipmentRetargetReinstallRequest,
    ) -> Result<ConfiguredRetargetReinstallPreparation, ConfiguredRetargetReinstallError> {
        let services = self
            .services_for(&request.game_id)
            .map_err(ConfiguredRetargetReinstallError::Reinstall)?;
        let context = services
            .preview
            .resolve_installed_equipment_context(
                &request.game_id,
                &request.profile_id,
                &request.mod_id,
            )
            .map_err(ConfiguredRetargetReinstallError::Reinstall)?;
        let context = match context {
            InstalledEquipmentReinstallResolution::Ready(context) => context,
            InstalledEquipmentReinstallResolution::Blocked(preview) => {
                return Ok(ConfiguredRetargetReinstallPreparation {
                    preparation: ReinstallPreparation::Blocked(preview),
                    game_instance: services.game_instance,
                    source: Arc::clone(&self.source),
                    staging_cleanup: RetargetStagingCleanup::default(),
                })
            }
        };
        let planned = self
            .replacement_workflow
            .preview_equipment_reinstall(PreviewEquipmentRetargetReinstallRequest {
                selection: request.clone(),
                installed_revision_id: context.installed_revision_id.clone(),
                installed_bindings: context.installed_bindings,
            })
            .map_err(ConfiguredRetargetReinstallError::Replacement)?;
        let source_root = self
            .sandbox_locator
            .sandbox_root_for_package(planned.package_id())
            .map_err(|_| {
                ConfiguredRetargetReinstallError::Replacement(
                    ReplacementWorkflowError::SandboxUnavailable,
                )
            })?;
        let staging_root = retarget_reinstall_staging_root(&self.app_data_dir);
        let materializer = FileSystemRetargetStagingMaterializer::new_with_registry(
            staging_root.clone(),
            Arc::new(FileSystemInstallSourceFileReader::new(source_root)),
            Arc::clone(&self.content_transformers),
        );
        let plan = self
            .replacement_workflow
            .materialize_equipment_reinstall(&materializer, planned)
            .map_err(ConfiguredRetargetReinstallError::Replacement)?;
        let staging_cleanup = RetargetStagingCleanup::armed(staging_root.clone());
        let reader = RetargetStagingInstallSourceFileReader::from_install_plan(staging_root, &plan)
            .map_err(|_| {
                ConfiguredRetargetReinstallError::Replacement(
                    ReplacementWorkflowError::PlanUnavailable,
                )
            })?;
        let source: Arc<dyn ReinstallCandidateSourceReader> =
            Arc::new(RetargetStagingReinstallCandidateSourceReader { reader });
        let preparation = self
            .services_for_game_instance_with_source(
                services.game_instance.clone(),
                Arc::clone(&source),
            )
            .preview
            .prepare_equipment_target_switch(
                ReinstallPreviewRequest {
                    game_id: request.game_id,
                    profile_id: request.profile_id,
                    mod_id: request.mod_id,
                    candidate_revision_id: context.installed_revision_id,
                    layer: request.layer,
                },
                plan,
            )
            .map_err(ConfiguredRetargetReinstallError::Reinstall)?;
        Ok(ConfiguredRetargetReinstallPreparation {
            preparation,
            game_instance: services.game_instance,
            source,
            staging_cleanup,
        })
    }
}

impl EquipmentRetargetReinstallTaskExecutor for ConfiguredReinstallExecutor {
    fn prepare_equipment_retarget_reinstall(
        &self,
        request: EquipmentRetargetReinstallRequest,
    ) -> Result<Self::Prepared, ReinstallTaskPrepareError> {
        let fallback = ReinstallTaskAuditContext {
            previous_revision_id: None,
            candidate_revision_id: hmm_core::ModRevisionId::new("unresolved"),
            counts: ReinstallTargetCounts::default(),
            adapter_facts: None,
        };
        let prepared = match self.prepare_equipment_selection(request) {
            Ok(prepared) => prepared,
            Err(ConfiguredRetargetReinstallError::Replacement(_)) => {
                return Err(ReinstallTaskPrepareError::Planning(fallback))
            }
            Err(ConfiguredRetargetReinstallError::Reinstall(
                ReinstallPreviewError::CatalogUnavailable
                | ReinstallPreviewError::CandidatePlanUnavailable,
            )) => return Err(ReinstallTaskPrepareError::Planning(fallback)),
            Err(ConfiguredRetargetReinstallError::Reinstall(
                ReinstallPreviewError::ManifestUnavailable
                | ReinstallPreviewError::RecoveryUnavailable,
            )) => return Err(ReinstallTaskPrepareError::Preflight(fallback)),
        };
        match prepared.preparation {
            ReinstallPreparation::Ready(reinstall) => Ok(ConfiguredPreparedReinstall {
                prepared: *reinstall,
                game_instance: prepared.game_instance,
                source: prepared.source,
                staging_cleanup: prepared.staging_cleanup,
            }),
            ReinstallPreparation::Blocked(preview) => Err(ReinstallTaskPrepareError::Preflight(
                ReinstallTaskAuditContext {
                    previous_revision_id: preview
                        .installed_revision
                        .as_ref()
                        .map(|revision| revision.revision_id.clone()),
                    candidate_revision_id: preview
                        .candidate_revision
                        .map(|revision| revision.revision_id)
                        .or_else(|| {
                            preview
                                .installed_revision
                                .map(|revision| revision.revision_id)
                        })
                        .unwrap_or(fallback.candidate_revision_id),
                    counts: preview.counts,
                    adapter_facts: None,
                },
            )),
        }
    }
}
