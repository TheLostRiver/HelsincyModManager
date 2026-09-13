use crate::{
    InstallManifest, InstallManifestStatusConsumption, InstallPlan, InstallTargetPath,
    InstalledFileSummary, ModId, ModRevisionId, OriginalInstallEvidenceError as Error,
    OriginalInstallFileEvidence, PackageFileId, ProfileId, ReplacementBindingSnapshot,
    ReplacementSourceId,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdditionalSource {
    binding: ReplacementBindingSnapshot,
    files: Vec<OriginalInstallFileEvidence>,
}

#[cfg(test)]
#[path = "additional_sources_tests.rs"]
mod tests;

/// 新 adapter 识别出的原位来源。已有绑定完全保留；原位文件须由原包、所有权和实际摘要共同证明。
/// 只用于同版本的来源连续性校验，不单独授权写入，也不用于完全无绑定的旧安装。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdditionalSourcesEvidence {
    schema_version: u32,
    profile_id: ProfileId,
    mod_id: ModId,
    revision_id: ModRevisionId,
    installed_bindings: Vec<ReplacementBindingSnapshot>,
    sources: Vec<AdditionalSource>,
}

impl AdditionalSourcesEvidence {
    pub fn verify(
        manifest: &InstallManifest,
        mod_id: &ModId,
        revision_id: &ModRevisionId,
        original_plan: &InstallPlan,
        source_files: &BTreeMap<ReplacementSourceId, BTreeSet<PackageFileId>>,
        original_files: &BTreeMap<PackageFileId, InstalledFileSummary>,
        actual_files: &BTreeMap<InstallTargetPath, InstalledFileSummary>,
    ) -> Result<Self, Error> {
        let mut installed_bindings = manifest
            .replacement_bindings
            .iter()
            .filter(|binding| binding.mod_id() == mod_id)
            .cloned()
            .collect::<Vec<_>>();
        installed_bindings.sort_by(|a, b| a.binding_id().cmp(b.binding_id()));
        if installed_bindings.is_empty()
            || original_plan.has_blocking_conflicts()
            || source_files.len() != original_plan.replacement_bindings.len()
            || original_plan
                .validate_replacement_bindings_for_profile_and_revision(
                    &manifest.profile_id,
                    Some(revision_id),
                )
                .is_err()
            || original_plan
                .replacement_bindings
                .iter()
                .any(|binding| binding.mod_id() != mod_id)
        {
            return Err(Error::Bindings);
        }
        let mut inventory_ids = BTreeSet::new();
        for files in source_files.values() {
            if files.is_empty() || files.iter().any(|file| !inventory_ids.insert(file)) {
                return Err(Error::Layout);
            }
        }
        let mut actions = BTreeMap::new();
        for action in &original_plan.actions {
            if action.provider.mod_id != *mod_id
                || action.provider.target_path != action.target_path
                || actions
                    .insert(&action.provider.package_file_id, action)
                    .is_some()
            {
                return Err(Error::Layout);
            }
        }
        if inventory_ids.iter().any(|id| !actions.contains_key(*id)) {
            return Err(Error::Layout);
        }
        for installed in &installed_bindings {
            if !original_plan.replacement_bindings.iter().any(|original| {
                original.binding().source_id() == installed.binding().source_id()
                    && original.source_internal_id() == installed.source_internal_id()
                    && original.source_path_family() == installed.source_path_family()
                    && original.retarget_kind() == installed.retarget_kind()
            }) {
                return Err(Error::Bindings);
            }
        }
        let mut sources = Vec::new();
        for binding in &original_plan.replacement_bindings {
            let ids = source_files
                .get(binding.binding().source_id())
                .ok_or(Error::Bindings)?;
            if installed_bindings
                .iter()
                .any(|old| old.binding().source_id() == binding.binding().source_id())
            {
                continue;
            }
            let mut files = Vec::new();
            for id in ids {
                let action = actions.get(id).ok_or(Error::Layout)?;
                let entry = manifest
                    .entries
                    .iter()
                    .find(|entry| {
                        entry.target_path.windows_key() == action.target_path.windows_key()
                    })
                    .ok_or(Error::Layout)?;
                if entry.mod_id != *mod_id || entry.package_file_id != *id {
                    return Err(Error::Metadata);
                }
                let summary = entry.installed_file.as_ref().ok_or(Error::Metadata)?;
                if original_files.get(id) != Some(summary)
                    || actual_files.get(&entry.target_path) != Some(summary)
                {
                    return Err(Error::Contents);
                }
                files.push(OriginalInstallFileEvidence {
                    target_path: entry.target_path.clone(),
                    package_file_id: id.clone(),
                    layer: entry.layer.clone(),
                    summary: summary.clone(),
                });
            }
            sources.push(AdditionalSource {
                binding: binding.clone(),
                files,
            });
        }
        sources.sort_by(|a, b| a.binding.binding_id().cmp(b.binding.binding_id()));
        let evidence = Self {
            schema_version: 1,
            profile_id: manifest.profile_id.clone(),
            mod_id: mod_id.clone(),
            revision_id: revision_id.clone(),
            installed_bindings,
            sources,
        };
        let file_count = evidence.files().count();
        if file_count != original_files.len() || file_count != actual_files.len() {
            return Err(Error::Layout);
        }
        evidence.validate(manifest)?;
        Ok(evidence)
    }

    pub fn validate(&self, manifest: &InstallManifest) -> Result<(), Error> {
        if self.schema_version != 1
            || self.profile_id != manifest.profile_id
            || manifest.validate().is_err()
            || manifest.status.consumption() != InstallManifestStatusConsumption::TrustEntries
            || self.sources.is_empty()
            || self.installed_bindings.is_empty()
        {
            return Err(Error::Metadata);
        }
        let mut installed = manifest
            .replacement_bindings
            .iter()
            .filter(|binding| binding.mod_id() == &self.mod_id)
            .cloned()
            .collect::<Vec<_>>();
        installed.sort_by(|a, b| a.binding_id().cmp(b.binding_id()));
        if installed != self.installed_bindings
            || installed.iter().any(|binding| {
                binding.profile_id() != &self.profile_id
                    || binding.revision_id() != Some(&self.revision_id)
            })
        {
            return Err(Error::Bindings);
        }
        // 同 revision 必须有明确证据；不从当前显示版本或未绑定旧记录猜测。
        if manifest
            .entries
            .iter()
            .filter(|entry| entry.mod_id == self.mod_id)
            .any(|entry| entry.revision_id.as_ref() != Some(&self.revision_id))
        {
            return Err(Error::Metadata);
        }
        let mut binding_ids = manifest
            .replacement_bindings
            .iter()
            .map(|b| b.binding_id())
            .collect::<BTreeSet<_>>();
        let mut source_ids = installed
            .iter()
            .map(|b| b.binding().source_id())
            .collect::<BTreeSet<_>>();
        let mut paths = BTreeSet::new();
        let mut file_ids = BTreeSet::new();
        for source in &self.sources {
            let binding = &source.binding;
            if binding.mod_id() != &self.mod_id
                || binding.profile_id() != &self.profile_id
                || binding.revision_id() != Some(&self.revision_id)
                || binding.binding().created_at_unix_millis() != 0
                || binding.source_internal_id() != binding.target_internal_id()
                || binding.source_path_family() != binding.target_path_family()
                || binding.adapter_facts().is_some()
                || source.files.is_empty()
                || !binding_ids.insert(binding.binding_id())
                || !source_ids.insert(binding.binding().source_id())
            {
                return Err(Error::Bindings);
            }
            for file in &source.files {
                if !paths.insert(file.target_path.windows_key())
                    || !file_ids.insert(&file.package_file_id)
                    || file.summary.sha256.len() != 64
                    || !file.summary.sha256.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Err(Error::Layout);
                }
                let entries = manifest
                    .entries
                    .iter()
                    .filter(|entry| {
                        entry.target_path.windows_key() == file.target_path.windows_key()
                    })
                    .collect::<Vec<_>>();
                if entries.len() != 1 {
                    return Err(Error::Layout);
                }
                let entry = entries[0];
                if entry.mod_id != self.mod_id
                    || entry.adopted
                    || entry.target_path != file.target_path
                    || entry.package_file_id != file.package_file_id
                    || entry.layer != file.layer
                    || manifest
                        .entries
                        .iter()
                        .filter(|entry| {
                            entry.mod_id == self.mod_id
                                && entry.package_file_id == file.package_file_id
                        })
                        .count()
                        != 1
                    || entry.installed_file.as_ref() != Some(&file.summary)
                {
                    return Err(Error::Metadata);
                }
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
    pub fn files(&self) -> impl Iterator<Item = &OriginalInstallFileEvidence> {
        self.sources.iter().flat_map(|source| &source.files)
    }
    pub fn bindings(&self) -> impl Iterator<Item = &ReplacementBindingSnapshot> {
        self.sources.iter().map(|source| &source.binding)
    }

    pub fn allows_equipment_target_switch(
        &self,
        manifest: &InstallManifest,
        candidates: &[ReplacementBindingSnapshot],
    ) -> bool {
        self.allows(
            manifest,
            candidates,
            crate::is_same_revision_equipment_target_switch,
        )
    }
    pub fn allows_equipment_reapply(
        &self,
        manifest: &InstallManifest,
        candidates: &[ReplacementBindingSnapshot],
    ) -> bool {
        self.allows(
            manifest,
            candidates,
            crate::is_same_revision_equipment_reapply,
        )
    }
    fn allows(
        &self,
        manifest: &InstallManifest,
        candidates: &[ReplacementBindingSnapshot],
        check: fn(&InstallManifest, &ModId, &ModRevisionId, &[ReplacementBindingSnapshot]) -> bool,
    ) -> bool {
        if self.validate(manifest).is_err() {
            return false;
        }
        let mut lineage = manifest.clone();
        lineage
            .replacement_bindings
            .extend(self.bindings().cloned());
        check(&lineage, &self.mod_id, &self.revision_id, candidates)
    }
}
