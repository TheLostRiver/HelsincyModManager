use super::*;

impl ReplacementWorkflowService {
    /// 替换目标页面按已记录的安装版本展示来源；此查询不校验或改写游戏文件。
    pub(super) fn resolve_profiled_replacement(
        &self,
        game_id: &GameId,
        mod_id: &ModId,
        profile_id: Option<&ProfileId>,
    ) -> Result<ResolvedImportedReplacement, ReplacementWorkflowError> {
        if let Some(profile_id) = profile_id {
            let manifest = self
                .install_manifests
                .load_manifest(profile_id)
                .map_err(|_| ReplacementWorkflowError::InstallManifestUnavailable)?;
            if let Some(manifest) = manifest {
                if manifest.profile_id != *profile_id
                    || manifest.validate().is_err()
                    || manifest.status.consumption()
                        != InstallManifestStatusConsumption::TrustEntries
                {
                    return Err(ReplacementWorkflowError::InstallManifestUnavailable);
                }
                let entries = manifest
                    .entries
                    .iter()
                    .filter(|entry| &entry.mod_id == mod_id)
                    .collect::<Vec<_>>();
                if !entries.is_empty() {
                    let revisions = manifest
                        .replacement_bindings
                        .iter()
                        .filter(|binding| binding.mod_id() == mod_id)
                        .filter_map(|binding| binding.revision_id().cloned())
                        .chain(entries.iter().filter_map(|entry| entry.revision_id.clone()))
                        .collect::<BTreeSet<_>>();
                    if revisions.len() == 1 {
                        return self.resolve_imported_revision(
                            game_id,
                            mod_id,
                            revisions.first().expect("one revision"),
                        );
                    }
                    if revisions.is_empty() {
                        let known = self
                            .result_repository
                            .list_revisions(mod_id)
                            .map_err(|_| ReplacementWorkflowError::ModRepositoryUnavailable)?;
                        if let [revision] = known.as_slice() {
                            if revision.mod_id == *mod_id {
                                return self.resolve_imported_revision(
                                    game_id,
                                    mod_id,
                                    &revision.revision_id,
                                );
                            }
                        }
                    }
                }
            }
        }
        self.resolve_imported_replacement(game_id, mod_id)
    }

    pub fn analyze_imported_mod_in_profile(
        &self,
        request: AnalyzeImportedReplacementRequest,
        profile_id: Option<&ProfileId>,
    ) -> Result<ReplacementAnalysis, ReplacementWorkflowError> {
        self.resolve_profiled_replacement(&request.game_id, &request.mod_id, profile_id)
            .map(|resolved| resolved.analysis)
    }
}
