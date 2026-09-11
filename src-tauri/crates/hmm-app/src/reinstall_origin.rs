use super::*;
use hmm_core::{OriginalInstallEvidence, OriginalInstallEvidenceError};

impl ReinstallPreviewService {
    pub fn with_original_install_source(
        mut self,
        source: Arc<dyn ReinstallCandidateSourceReader>,
    ) -> Self {
        self.original_source = source;
        self
    }

    pub(super) fn verify_original_install(
        &self,
        game_id: &GameId,
        mod_id: &ModId,
        revision: &StoredModRevision,
        manifest: &InstallManifest,
    ) -> Result<OriginalInstallEvidence, ReinstallBlockingReason> {
        let entries = manifest
            .entries
            .iter()
            .filter(|entry| &entry.mod_id == mod_id)
            .collect::<Vec<_>>();
        let first = entries
            .first()
            .ok_or(ReinstallBlockingReason::NotInstalled)?;
        if entries.iter().any(|entry| entry.revision_id.is_none()) {
            let known = self
                .catalog
                .list_revisions(mod_id)
                .map_err(|_| ReinstallBlockingReason::InstalledRevisionUnknown)?;
            if known.len() != 1
                || known[0].revision_id != revision.revision_id
                || known[0].mod_id != *mod_id
            {
                return Err(ReinstallBlockingReason::InstalledRevisionUnknown);
            }
        }
        if entries
            .iter()
            .any(|entry| entry.adopted || entry.installed_file.is_none())
        {
            return Err(ReinstallBlockingReason::OriginalInstallUnverified);
        }
        let original_plan = self
            .planner
            .build_candidate_plan(ReinstallCandidatePlanRequest {
                game_id,
                profile_id: &manifest.profile_id,
                mod_id,
                candidate: revision,
                layer: &first.layer,
            })
            .map_err(|_| ReinstallBlockingReason::SourceUnavailable)?;
        if original_plan.replacement_bindings.is_empty() {
            return Err(ReinstallBlockingReason::CandidateNotReady);
        }
        let mut original_files = BTreeMap::new();
        for action in &original_plan.actions {
            let bytes = self
                .original_source
                .read_candidate_source_file(revision, &action.provider.package_file_id)
                .map_err(|_| ReinstallBlockingReason::SourceUnavailable)?;
            original_files.insert(action.provider.package_file_id.clone(), summarize(&bytes));
        }
        let mut actual_files = BTreeMap::new();
        for entry in entries {
            let bytes = self
                .game
                .read_game_file(&entry.target_path)
                .map_err(|_| ReinstallBlockingReason::TargetReadFailed)?
                .ok_or(ReinstallBlockingReason::TargetMissing)?;
            let current = summarize(&bytes);
            if entry.installed_file.as_ref() != Some(&current) {
                return Err(ReinstallBlockingReason::TargetChanged);
            }
            actual_files.insert(entry.target_path.clone(), current);
        }
        OriginalInstallEvidence::verify(
            manifest,
            mod_id,
            &revision.revision_id,
            &original_plan,
            &original_files,
            &actual_files,
        )
        .map_err(|error| match error {
            OriginalInstallEvidenceError::Bindings => ReinstallBlockingReason::CandidateNotReady,
            _ => ReinstallBlockingReason::OriginalInstallUnverified,
        })
    }

    pub fn prepare_replacement_target_switch_with_origin(
        &self,
        request: ReinstallPreviewRequest,
        plan: InstallPlan,
        origin: Option<OriginalInstallEvidence>,
    ) -> Result<ReinstallPreparation, ReinstallPreviewError> {
        self.prepare_with_candidate_plan(
            request,
            Some(plan),
            ReplacementSwitchMode::SingleSource,
            origin,
        )
    }

    pub fn prepare_equipment_target_switch_with_origin(
        &self,
        request: ReinstallPreviewRequest,
        plan: InstallPlan,
        origin: Option<OriginalInstallEvidence>,
    ) -> Result<ReinstallPreparation, ReinstallPreviewError> {
        self.prepare_with_candidate_plan(
            request,
            Some(plan),
            ReplacementSwitchMode::Equipment,
            origin,
        )
    }
}
