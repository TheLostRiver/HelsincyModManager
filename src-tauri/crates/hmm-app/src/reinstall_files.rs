use super::*;
use hmm_core::{RetargetFileDisposition, RetargetFileEffect, RetargetFileReason};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RetargetFilePreview {
    pub effect: RetargetFileEffect,
    pub installed_path: Option<InstallTargetPath>,
    pub change: Option<ReinstallTargetClass>,
}

impl From<RetargetFileEffect> for RetargetFilePreview {
    fn from(effect: RetargetFileEffect) -> Self {
        Self {
            effect,
            installed_path: None,
            change: None,
        }
    }
}

impl ReinstallPreparation {
    pub fn with_file_effects(
        mut self,
        effects: Vec<RetargetFileEffect>,
    ) -> Result<Self, ReinstallPreviewError> {
        let Self::Ready(prepared) = &mut self else {
            return Ok(self);
        };
        if effects.is_empty() {
            return Ok(self);
        }
        let mut ids = BTreeSet::new();
        let mut previews = Vec::with_capacity(effects.len());
        for mut effect in effects {
            if !ids.insert(effect.package_file_id.clone()) {
                return Err(ReinstallPreviewError::CandidatePlanUnavailable);
            }
            let installed_path = prepared
                .old_manifest
                .entries
                .iter()
                .find(|entry| {
                    entry.mod_id == prepared.request.mod_id
                        && entry.package_file_id == effect.package_file_id
                })
                .map(|entry| entry.target_path.clone());
            let candidate = prepared
                .source_files
                .iter()
                .find(|source| source.provider.package_file_id == effect.package_file_id);
            let change = if let Some(source) = candidate {
                if effect.target_path.as_ref().is_some_and(|target| {
                    target.windows_key() != source.provider.target_path.windows_key()
                }) {
                    return Err(ReinstallPreviewError::CandidatePlanUnavailable);
                }
                let target = prepared
                    .targets
                    .iter()
                    .find(|target| target.target_path == source.provider.target_path)
                    .ok_or(ReinstallPreviewError::CandidatePlanUnavailable)?;
                if effect.target_path.is_none() {
                    if target.class != ReinstallTargetClass::Retained
                        || installed_path.as_ref() != Some(&source.provider.target_path)
                        || effect.source_path.windows_key()
                            != source.provider.target_path.windows_key()
                    {
                        return Err(ReinstallPreviewError::CandidatePlanUnavailable);
                    }
                    effect.disposition = RetargetFileDisposition::InstalledAttachmentRetained;
                    effect.reason = RetargetFileReason::InstalledAttachment;
                }
                effect.target_path = Some(source.provider.target_path.clone());
                Some(target.class)
            } else {
                if effect.target_path.is_some() {
                    return Err(ReinstallPreviewError::CandidatePlanUnavailable);
                }
                None
            };
            previews.push(RetargetFilePreview {
                effect,
                installed_path,
                change,
            });
        }
        if prepared
            .source_files
            .iter()
            .any(|source| !ids.contains(&source.provider.package_file_id))
        {
            return Err(ReinstallPreviewError::CandidatePlanUnavailable);
        }
        previews.sort_by(|left, right| {
            left.effect
                .source_path
                .cmp(&right.effect.source_path)
                .then_with(|| {
                    left.effect
                        .package_file_id
                        .cmp(&right.effect.package_file_id)
                })
        });
        let serialized = serde_json::to_vec(&previews)
            .map_err(|_| ReinstallPreviewError::CandidatePlanUnavailable)?;
        let mut hasher = Sha256::new();
        hash_field(&mut hasher, "retarget-file-effects-v1");
        hash_field(&mut hasher, &prepared.plan_token);
        hasher.update(serialized);
        prepared.plan_token = format!("{:x}", hasher.finalize());
        prepared.plan_hash = prepared.plan_token.clone();
        prepared.file_effects = previews;
        Ok(self)
    }
}
