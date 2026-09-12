use super::*;

impl ReinstallPreviewService {
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
                if !retained_attachments.contains(&source.provider.package_file_id) {
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
        prepared.plan_token = format!("{:x}", hasher.finalize());
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
