use super::*;

impl ReinstallPreviewService {
    /// 候选可由只读内存转换提供；原包校验继续使用独立的 original_source。
    pub fn with_candidate_source(
        mut self,
        source: Arc<dyn ReinstallCandidateSourceReader>,
    ) -> Self {
        self.source = source;
        self
    }

    pub fn prepare_unbound_plugin_reapply(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
        layer: &FileLayer,
        workflow: &crate::ReplacementWorkflowService,
    ) -> Result<Option<ReinstallPreparation>, ReinstallPreviewError> {
        let Some(manifest) = self
            .manifests
            .load_manifest(profile_id)
            .map_err(|_| ReinstallPreviewError::ManifestUnavailable)?
        else {
            return Ok(None);
        };
        if manifest
            .replacement_bindings
            .iter()
            .any(|binding| binding.mod_id() == mod_id)
        {
            return Ok(None);
        }
        let Some(revision) = manifest
            .entries
            .iter()
            .find(|entry| &entry.mod_id == mod_id)
            .and_then(|entry| entry.revision_id.as_ref())
        else {
            return Ok(None);
        };
        let candidate = self
            .catalog
            .get_revision(revision)
            .map_err(|_| ReinstallPreviewError::CatalogUnavailable)?
            .filter(|candidate| &candidate.mod_id == mod_id)
            .ok_or(ReinstallPreviewError::CandidatePlanUnavailable)?;
        let mut plan = self
            .planner
            .build_candidate_plan(ReinstallCandidatePlanRequest {
                game_id,
                profile_id,
                mod_id,
                candidate: &candidate,
                layer,
            })
            .map_err(|_| ReinstallPreviewError::CandidatePlanUnavailable)?;
        if plan.plugin_selections.is_empty() || !plan.replacement_bindings.is_empty() {
            return Ok(None);
        }
        for action in &mut plan.actions {
            if let Some(entry) = manifest.entries.iter().find(|entry| {
                &entry.mod_id == mod_id
                    && entry.package_file_id == action.provider.package_file_id
                    && entry.target_path.windows_key() == action.target_path.windows_key()
            }) {
                action.provider.layer = entry.layer.clone();
            }
        }
        let plugin_effects = workflow.plugin_file_effects(&plan);
        let ids = plugin_effects
            .iter()
            .map(|effect| &effect.package_file_id)
            .collect::<BTreeSet<_>>();
        let mut effects = plan
            .actions
            .iter()
            .filter(|action| !ids.contains(&action.provider.package_file_id))
            .map(|action| hmm_core::RetargetFileEffect {
                package_file_id: action.provider.package_file_id.clone(),
                source_id: None,
                source_path: action.target_path.clone(),
                target_path: Some(action.target_path.clone()),
                disposition: hmm_core::RetargetFileDisposition::PackageCompanion,
                reason: hmm_core::RetargetFileReason::PackageResource,
            })
            .collect::<Vec<_>>();
        effects.extend(plugin_effects);
        self.prepare_equipment_with_intent(
            ReinstallPreviewRequest {
                game_id: game_id.clone(),
                profile_id: profile_id.clone(),
                mod_id: mod_id.clone(),
                candidate_revision_id: revision.clone(),
                layer: layer.clone(),
            },
            plan,
            None,
            None,
            hmm_core::ReinstallIntent::ReapplyEquipmentTargets,
        )
        .and_then(|prepared| prepared.with_file_effects(effects))
        .and_then(ReinstallPreparation::with_untransformed_reapply_sources)
        .map(Some)
    }

    pub fn prepare_equipment_with_intent(
        &self,
        request: ReinstallPreviewRequest,
        candidate_plan: InstallPlan,
        original_install_evidence: Option<OriginalInstallEvidence>,
        policy_exclusions: Option<Vec<hmm_core::RetargetPolicyExcludedFile>>,
        intent: hmm_core::ReinstallIntent,
    ) -> Result<ReinstallPreparation, ReinstallPreviewError> {
        self.prepare_with_candidate_plan(
            request,
            Some(candidate_plan),
            match intent {
                hmm_core::ReinstallIntent::Standard => ReplacementSwitchMode::Equipment,
                hmm_core::ReinstallIntent::ReapplyEquipmentTargets => {
                    ReplacementSwitchMode::Reapply
                }
            },
            original_install_evidence,
            policy_exclusions.as_deref(),
        )
    }
}

impl ReinstallPreparation {
    pub fn with_reapply_source_fingerprints(
        mut self,
        mut summaries: BTreeMap<PackageFileId, InstalledFileSummary>,
    ) -> Result<Self, ReinstallPreviewError> {
        let Self::Ready(prepared) = &mut self else {
            return Ok(self);
        };
        if prepared.intent != hmm_core::ReinstallIntent::ReapplyEquipmentTargets {
            return Err(ReinstallPreviewError::CandidatePlanUnavailable);
        }
        let ids = prepared
            .source_files
            .iter()
            .map(|source| &source.provider.package_file_id)
            .collect::<BTreeSet<_>>();
        if ids.len() != prepared.source_files.len() || summaries.keys().any(|id| !ids.contains(id))
        {
            return Err(ReinstallPreviewError::CandidatePlanUnavailable);
        }
        let retained_attachments = prepared
            .file_effects
            .iter()
            .filter(|file| {
                file.effect.disposition
                    == hmm_core::RetargetFileDisposition::InstalledAttachmentRetained
            })
            .map(|file| &file.effect.package_file_id)
            .collect::<BTreeSet<_>>();
        for source in &prepared.source_files {
            if !summaries.contains_key(&source.provider.package_file_id) {
                let selected_plugin = prepared
                    .candidate_plugin_selections
                    .iter()
                    .flat_map(|selection| selection.files())
                    .any(|file| {
                        file.choice.is_included()
                            && file.package_file_id == source.provider.package_file_id
                            && file.source_file == source.summary
                    });
                if !retained_attachments.contains(&source.provider.package_file_id)
                    && !selected_plugin
                {
                    return Err(ReinstallPreviewError::CandidatePlanUnavailable);
                }
                summaries.insert(
                    source.provider.package_file_id.clone(),
                    source.summary.clone(),
                );
            }
        }
        let serialized = serde_json::to_vec(&summaries)
            .map_err(|_| ReinstallPreviewError::CandidatePlanUnavailable)?;
        let mut hasher = Sha256::new();
        hash_field(&mut hasher, "equipment-reapply-source-v1");
        hash_field(&mut hasher, &prepared.plan_token);
        hasher.update(serialized);
        prepared.plan_token = finalize_plan_token(hasher);
        prepared.plan_hash = prepared.plan_token.clone();
        prepared.reapply_source_fingerprints = summaries;
        Ok(self)
    }

    /// 仅供确认没有内容转换的只读计划：候选字节就是读取到的原始字节。
    pub fn with_untransformed_reapply_sources(self) -> Result<Self, ReinstallPreviewError> {
        let summaries = match &self {
            Self::Ready(prepared) => prepared
                .source_files
                .iter()
                .map(|source| {
                    (
                        source.provider.package_file_id.clone(),
                        source.summary.clone(),
                    )
                })
                .collect(),
            Self::Blocked(_) => BTreeMap::new(),
        };
        self.with_reapply_source_fingerprints(summaries)
    }
}

pub(super) fn preserve_installed_layers(
    request: &mut ReinstallPreviewRequest,
    manifest: &InstallManifest,
    plan: &mut InstallPlan,
) -> Result<(), ReinstallBlockingReason> {
    let entries = manifest
        .entries
        .iter()
        .filter(|entry| entry.mod_id == request.mod_id)
        .collect::<Vec<_>>();
    let first = entries
        .first()
        .ok_or(ReinstallBlockingReason::NotInstalled)?;
    let common_layer = entries
        .iter()
        .all(|entry| entry.layer == first.layer)
        .then_some(&first.layer);
    let mut layers = BTreeMap::new();
    for entry in &entries {
        if entry.adopted
            || layers
                .insert(&entry.package_file_id, &entry.layer)
                .is_some()
        {
            return Err(ReinstallBlockingReason::ManifestStateUnsafe);
        }
    }
    for action in &mut plan.actions {
        action.provider.layer = layers
            .get(&action.provider.package_file_id)
            .copied()
            .or(common_layer)
            .ok_or(ReinstallBlockingReason::CandidateNotReady)?
            .clone();
    }
    request.layer = first.layer.clone();
    Ok(())
}
