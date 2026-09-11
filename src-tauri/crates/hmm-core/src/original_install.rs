use crate::{
    is_same_revision_equipment_target_switch, is_same_revision_replacement_target_switch,
    FileLayer, InstallManifest, InstallManifestStatusConsumption, InstallPlan, InstallTargetPath,
    InstalledFileSummary, ModId, ModRevisionId, PackageFileId, ProfileId,
    ReplacementBindingSnapshot,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginalInstallFileEvidence {
    target_path: InstallTargetPath,
    package_file_id: PackageFileId,
    layer: FileLayer,
    summary: InstalledFileSummary,
}

impl OriginalInstallFileEvidence {
    pub fn package_file_id(&self) -> &PackageFileId {
        &self.package_file_id
    }
    pub fn summary(&self) -> &InstalledFileSummary {
        &self.summary
    }
}

/// 后端完成原包、清单和实际文件核对后产生的原位证据。不能作为独立写入授权。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginalInstallEvidence {
    schema_version: u32,
    profile_id: ProfileId,
    mod_id: ModId,
    revision_id: ModRevisionId,
    files: Vec<OriginalInstallFileEvidence>,
    bindings: Vec<ReplacementBindingSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum OriginalInstallEvidenceError {
    #[error("original installation metadata is not trusted")]
    Metadata,
    #[error("original installation layout does not match the managed files")]
    Layout,
    #[error("original source and installed file contents do not match")]
    Contents,
    #[error("original installation bindings are unavailable or invalid")]
    Bindings,
}

impl OriginalInstallEvidence {
    pub fn verify(
        manifest: &InstallManifest,
        mod_id: &ModId,
        revision_id: &ModRevisionId,
        original_plan: &InstallPlan,
        original_files: &BTreeMap<PackageFileId, InstalledFileSummary>,
        actual_files: &BTreeMap<InstallTargetPath, InstalledFileSummary>,
    ) -> Result<Self, OriginalInstallEvidenceError> {
        let entries = manifest
            .entries
            .iter()
            .filter(|entry| &entry.mod_id == mod_id)
            .collect::<Vec<_>>();
        if original_plan.has_blocking_conflicts()
            || original_plan.actions.len() != entries.len()
            || original_files.len() != entries.len()
            || actual_files.len() != entries.len()
        {
            return Err(OriginalInstallEvidenceError::Layout);
        }
        let mut plan_targets = BTreeMap::new();
        for action in &original_plan.actions {
            if &action.provider.mod_id != mod_id
                || action.target_path != action.provider.target_path
                || plan_targets
                    .insert(action.target_path.windows_key(), action)
                    .is_some()
            {
                return Err(OriginalInstallEvidenceError::Layout);
            }
        }
        let mut files = Vec::with_capacity(entries.len());
        for entry in entries {
            let action = plan_targets
                .remove(&entry.target_path.windows_key())
                .ok_or(OriginalInstallEvidenceError::Layout)?;
            if !action
                .target_path
                .as_str()
                .eq_ignore_ascii_case(entry.target_path.as_str())
                || action.provider.package_file_id != entry.package_file_id
                || action.provider.layer != entry.layer
            {
                return Err(OriginalInstallEvidenceError::Layout);
            }
            let expected = entry
                .installed_file
                .as_ref()
                .ok_or(OriginalInstallEvidenceError::Metadata)?;
            if original_files.get(&entry.package_file_id) != Some(expected)
                || actual_files.get(&entry.target_path) != Some(expected)
            {
                return Err(OriginalInstallEvidenceError::Contents);
            }
            files.push(OriginalInstallFileEvidence {
                target_path: entry.target_path.clone(),
                package_file_id: entry.package_file_id.clone(),
                layer: entry.layer.clone(),
                summary: expected.clone(),
            });
        }
        files.sort_by(|left, right| left.target_path.cmp(&right.target_path));
        let evidence = Self {
            schema_version: 1,
            profile_id: manifest.profile_id.clone(),
            mod_id: mod_id.clone(),
            revision_id: revision_id.clone(),
            files,
            bindings: original_plan.replacement_bindings.clone(),
        };
        evidence.validate(manifest)?;
        Ok(evidence)
    }

    pub fn validate(&self, manifest: &InstallManifest) -> Result<(), OriginalInstallEvidenceError> {
        if self.schema_version != 1
            || self.profile_id != manifest.profile_id
            || manifest.status.consumption() != InstallManifestStatusConsumption::TrustEntries
            || manifest.validate().is_err()
            || manifest
                .replacement_bindings
                .iter()
                .any(|binding| binding.mod_id() == &self.mod_id)
        {
            return Err(OriginalInstallEvidenceError::Metadata);
        }
        let entries = manifest
            .entries
            .iter()
            .filter(|entry| entry.mod_id == self.mod_id)
            .collect::<Vec<_>>();
        if entries.is_empty() || entries.len() != self.files.len() || self.bindings.is_empty() {
            return Err(OriginalInstallEvidenceError::Bindings);
        }
        let mut targets = BTreeMap::new();
        let mut file_ids = BTreeSet::new();
        for file in &self.files {
            if targets
                .insert(file.target_path.windows_key(), file)
                .is_some()
                || !file_ids.insert(&file.package_file_id)
                || file.summary.sha256.len() != 64
                || !file
                    .summary
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(OriginalInstallEvidenceError::Layout);
            }
        }
        for entry in &manifest.entries {
            if entry.mod_id != self.mod_id {
                if targets.contains_key(&entry.target_path.windows_key()) {
                    return Err(OriginalInstallEvidenceError::Metadata);
                }
                continue;
            }
            let file = targets
                .get(&entry.target_path.windows_key())
                .ok_or(OriginalInstallEvidenceError::Layout)?;
            if entry.adopted
                || entry
                    .revision_id
                    .as_ref()
                    .is_some_and(|revision| revision != &self.revision_id)
                || entry.target_path != file.target_path
                || entry.package_file_id != file.package_file_id
                || entry.layer != file.layer
                || entry.installed_file.as_ref() != Some(&file.summary)
            {
                return Err(OriginalInstallEvidenceError::Metadata);
            }
        }
        let mut bindings = BTreeSet::new();
        let mut sources = BTreeSet::new();
        for binding in &self.bindings {
            if binding.mod_id() != &self.mod_id
                || binding.profile_id() != &self.profile_id
                || binding.revision_id() != Some(&self.revision_id)
                || binding.binding().created_at_unix_millis() != 0
                || binding.source_internal_id() != binding.target_internal_id()
                || binding.source_path_family() != binding.target_path_family()
                || binding.adapter_facts().is_some()
                || !bindings.insert(binding.binding_id())
                || !sources.insert(binding.binding().source_id())
            {
                return Err(OriginalInstallEvidenceError::Bindings);
            }
        }
        Ok(())
    }

    pub fn mod_id(&self) -> &ModId {
        &self.mod_id
    }
    pub fn revision_id(&self) -> &ModRevisionId {
        &self.revision_id
    }
    pub fn files(&self) -> &[OriginalInstallFileEvidence] {
        &self.files
    }
    pub fn bindings(&self) -> &[ReplacementBindingSnapshot] {
        &self.bindings
    }

    pub fn allows_single_target_switch(
        &self,
        manifest: &InstallManifest,
        candidates: &[ReplacementBindingSnapshot],
    ) -> bool {
        self.allows_switch(
            manifest,
            candidates,
            is_same_revision_replacement_target_switch,
        )
    }

    pub fn allows_equipment_target_switch(
        &self,
        manifest: &InstallManifest,
        candidates: &[ReplacementBindingSnapshot],
    ) -> bool {
        self.allows_switch(
            manifest,
            candidates,
            is_same_revision_equipment_target_switch,
        )
    }

    fn allows_switch(
        &self,
        manifest: &InstallManifest,
        candidates: &[ReplacementBindingSnapshot],
        check: fn(&InstallManifest, &ModId, &ModRevisionId, &[ReplacementBindingSnapshot]) -> bool,
    ) -> bool {
        if self.validate(manifest).is_err() {
            return false;
        }
        // 此副本只用于来源连续性验证；事务和回滚必须保留未改动的原清单。
        let mut lineage = manifest.clone();
        lineage.replacement_bindings.extend(self.bindings.clone());
        check(&lineage, &self.mod_id, &self.revision_id, candidates)
    }
}

#[cfg(test)]
#[path = "original_install_tests.rs"]
mod tests;
