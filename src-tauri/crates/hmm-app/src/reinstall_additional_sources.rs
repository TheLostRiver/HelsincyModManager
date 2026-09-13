use super::*;
use hmm_core::AdditionalSourcesEvidence;

impl ReinstallPreviewService {
    pub(super) fn verify_additional_sources(
        &self,
        game_id: &GameId,
        mod_id: &ModId,
        revision: &StoredModRevision,
        manifest: &InstallManifest,
    ) -> Result<Option<AdditionalSourcesEvidence>, ReinstallBlockingReason> {
        let installed = manifest
            .replacement_bindings
            .iter()
            .filter(|binding| binding.mod_id() == mod_id)
            .map(|binding| binding.binding().source_id())
            .collect::<BTreeSet<_>>();
        if installed.is_empty() {
            return Ok(None);
        }
        let first = manifest
            .entries
            .iter()
            .find(|entry| &entry.mod_id == mod_id)
            .ok_or(ReinstallBlockingReason::NotInstalled)?;
        let Some(inventory) = self
            .planner
            .original_source_inventory(ReinstallCandidatePlanRequest {
                game_id,
                profile_id: &manifest.profile_id,
                mod_id,
                candidate: revision,
                layer: &first.layer,
            })
            .map_err(|_| ReinstallBlockingReason::SourceUnavailable)?
        else {
            return Ok(None);
        };
        let extra = inventory
            .source_files
            .iter()
            .filter(|(id, _)| !installed.contains(id))
            .flat_map(|(_, files)| files)
            .collect::<BTreeSet<_>>();
        if extra.is_empty() {
            return Ok(None);
        }
        let mut original_files = BTreeMap::new();
        let mut actual_files = BTreeMap::new();
        for id in extra {
            let action = inventory
                .plan
                .actions
                .iter()
                .find(|action| &action.provider.package_file_id == id)
                .ok_or(ReinstallBlockingReason::OriginalInstallUnverified)?;
            let entry = manifest
                .entries
                .iter()
                .find(|entry| {
                    entry.target_path.windows_key() == action.target_path.windows_key()
                        && &entry.mod_id == mod_id
                        && &entry.package_file_id == id
                })
                .ok_or(ReinstallBlockingReason::OriginalInstallUnverified)?;
            if entry.adopted || entry.installed_file.is_none() {
                return Err(ReinstallBlockingReason::OriginalInstallUnverified);
            }
            let original = self
                .original_source
                .read_candidate_source_file(revision, id)
                .map_err(|_| ReinstallBlockingReason::SourceUnavailable)?;
            original_files.insert(id.clone(), summarize(&original));
            let current = self
                .game
                .read_game_file(&entry.target_path)
                .map_err(|_| ReinstallBlockingReason::TargetReadFailed)?
                .ok_or(ReinstallBlockingReason::TargetMissing)?;
            let current = summarize(&current);
            if entry.installed_file.as_ref() != Some(&current) {
                return Err(ReinstallBlockingReason::TargetChanged);
            }
            actual_files.insert(entry.target_path.clone(), current);
        }
        AdditionalSourcesEvidence::verify(
            manifest,
            mod_id,
            &revision.revision_id,
            &inventory.plan,
            &inventory.source_files,
            &original_files,
            &actual_files,
        )
        .map(Some)
        .map_err(|_| ReinstallBlockingReason::OriginalInstallUnverified)
    }
}
