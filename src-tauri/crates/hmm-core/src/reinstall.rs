use crate::{
    InstallFileProvider, InstallManifest, InstallManifestEntry, InstallTargetPath,
    InstalledFileSummary, ModId, ModRevisionId, PackageFileId, ReplacementBindingSnapshot,
    INSTALL_MANIFEST_SCHEMA_VERSION_V2,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinstallTargetState {
    pub target_path: InstallTargetPath,
    pub revision_id: ModRevisionId,
    pub providers: Vec<InstallFileProvider>,
    pub final_file: InstalledFileSummary,
}

impl ReinstallTargetState {
    pub fn new(
        target_path: InstallTargetPath,
        revision_id: ModRevisionId,
        providers: Vec<InstallFileProvider>,
        final_file: InstalledFileSummary,
    ) -> Self {
        Self {
            target_path,
            revision_id,
            providers,
            final_file,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReinstallTargetClass {
    Retained,
    Replaced,
    Added,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReinstallRecoveryTransactionStatus {
    Planned,
    Committing,
    Completed,
    RollbackRequired,
    RolledBack,
    RepairRequired,
}

impl ReinstallRecoveryTransactionStatus {
    pub fn can_transition_to(self, next: Self) -> bool {
        use ReinstallRecoveryTransactionStatus::{
            Committing, Completed, Planned, RepairRequired, RollbackRequired, RolledBack,
        };

        self == next
            || matches!(
                (self, next),
                (Planned, Committing)
                    | (Committing, Completed)
                    | (Committing, RollbackRequired)
                    | (Committing, RolledBack)
                    | (Committing, RepairRequired)
                    | (RollbackRequired, RolledBack)
                    | (RollbackRequired, RepairRequired)
            )
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReinstallRecoveryTransactionTransitionError {
    #[error("invalid reinstall recovery transaction transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: ReinstallRecoveryTransactionStatus,
        to: ReinstallRecoveryTransactionStatus,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReinstallSnapshotPurpose {
    TransactionRollback,
    OriginalBackupCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReinstallSnapshotCleanupOwner {
    Transaction,
    PromoteOnCommit,
    Manifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum ReinstallSnapshotState {
    NotRequired,
    PreStateAbsent,
    Stored {
        snapshot_ref: String,
        purpose: ReinstallSnapshotPurpose,
        cleanup_owner: ReinstallSnapshotCleanupOwner,
    },
    CleanupPending {
        snapshot_ref: String,
        purpose: ReinstallSnapshotPurpose,
    },
    Cleaned {
        purpose: ReinstallSnapshotPurpose,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReinstallRecoveryTarget {
    pub target_path: InstallTargetPath,
    pub class: ReinstallTargetClass,
    pub pre_state: Option<InstalledFileSummary>,
    pub candidate_state: Option<InstalledFileSummary>,
    pub snapshot: ReinstallSnapshotState,
    pub original_backup_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReinstallRecoveryTransaction {
    pub profile_id: crate::ProfileId,
    pub mod_id: ModId,
    pub old_revision_id: ModRevisionId,
    pub candidate_revision_id: ModRevisionId,
    pub plan_token: String,
    pub plan_hash: String,
    pub status: ReinstallRecoveryTransactionStatus,
    pub pre_reinstall_manifest: InstallManifest,
    #[serde(default)]
    pub candidate_replacement_bindings: Vec<ReplacementBindingSnapshot>,
    pub targets: Vec<ReinstallRecoveryTarget>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReinstallRecoveryTransactionValidationError {
    #[error("reinstall transaction profile does not match its pre-reinstall manifest")]
    ProfileMismatch,
    #[error("reinstall transaction old and candidate revisions must differ")]
    RevisionUnchanged,
    #[error("reinstall transaction plan identity cannot be empty")]
    EmptyPlanIdentity,
    #[error("reinstall transaction has no pre-reinstall entries for the requested Mod")]
    OldEntrySetEmpty,
    #[error("reinstall transaction contains a duplicate target")]
    DuplicateTarget { target_path: InstallTargetPath },
    #[error("reinstall transaction target facts do not match target class")]
    InvalidTargetFacts { target_path: InstallTargetPath },
    #[error("reinstall transaction snapshot ownership is invalid")]
    InvalidSnapshotOwnership { target_path: InstallTargetPath },
    #[error("reinstall transaction snapshot ref cannot be empty")]
    EmptySnapshotRef { target_path: InstallTargetPath },
    #[error("pre-reinstall manifest is invalid: {message}")]
    InvalidPreReinstallManifest { message: String },
    #[error("reinstall transaction candidate replacement binding is invalid")]
    InvalidCandidateReplacementBinding,
}

impl ReinstallRecoveryTransaction {
    pub fn validate(&self) -> Result<(), ReinstallRecoveryTransactionValidationError> {
        if self.profile_id != self.pre_reinstall_manifest.profile_id {
            return Err(ReinstallRecoveryTransactionValidationError::ProfileMismatch);
        }
        if self.old_revision_id == self.candidate_revision_id
            && !is_same_revision_replacement_target_switch(
                &self.pre_reinstall_manifest,
                &self.mod_id,
                &self.candidate_revision_id,
                &self.candidate_replacement_bindings,
            )
        {
            return Err(ReinstallRecoveryTransactionValidationError::RevisionUnchanged);
        }
        if self.plan_token.trim().is_empty() || self.plan_hash.trim().is_empty() {
            return Err(ReinstallRecoveryTransactionValidationError::EmptyPlanIdentity);
        }
        self.pre_reinstall_manifest.validate().map_err(|error| {
            ReinstallRecoveryTransactionValidationError::InvalidPreReinstallManifest {
                message: error.to_string(),
            }
        })?;
        if !self
            .pre_reinstall_manifest
            .entries
            .iter()
            .any(|entry| entry.mod_id == self.mod_id)
        {
            return Err(ReinstallRecoveryTransactionValidationError::OldEntrySetEmpty);
        }
        let mut candidate_binding_ids = BTreeSet::new();
        let mut candidate_binding_mods = BTreeSet::new();
        for snapshot in &self.candidate_replacement_bindings {
            if snapshot.mod_id() != &self.mod_id
                || snapshot.profile_id() != &self.profile_id
                || snapshot.revision_id() != Some(&self.candidate_revision_id)
                || !candidate_binding_ids.insert(snapshot.binding_id().clone())
                || !candidate_binding_mods.insert(snapshot.mod_id().clone())
            {
                return Err(
                    ReinstallRecoveryTransactionValidationError::InvalidCandidateReplacementBinding,
                );
            }
        }

        let mut targets = BTreeSet::new();
        for target in &self.targets {
            if !targets.insert(target.target_path.clone()) {
                return Err(
                    ReinstallRecoveryTransactionValidationError::DuplicateTarget {
                        target_path: target.target_path.clone(),
                    },
                );
            }

            let facts_valid = match target.class {
                ReinstallTargetClass::Retained | ReinstallTargetClass::Replaced => {
                    target.pre_state.is_some() && target.candidate_state.is_some()
                }
                ReinstallTargetClass::Added => target.candidate_state.is_some(),
                ReinstallTargetClass::Stale => {
                    target.pre_state.is_some() && target.candidate_state.is_none()
                }
            };
            if !facts_valid {
                return Err(
                    ReinstallRecoveryTransactionValidationError::InvalidTargetFacts {
                        target_path: target.target_path.clone(),
                    },
                );
            }

            let ownership_valid = match (&target.class, &target.snapshot) {
                (ReinstallTargetClass::Retained, ReinstallSnapshotState::NotRequired) => true,
                (ReinstallTargetClass::Added, ReinstallSnapshotState::PreStateAbsent)
                    if target.pre_state.is_none() =>
                {
                    true
                }
                (
                    ReinstallTargetClass::Added,
                    ReinstallSnapshotState::Stored {
                        purpose: ReinstallSnapshotPurpose::OriginalBackupCandidate,
                        cleanup_owner:
                            ReinstallSnapshotCleanupOwner::PromoteOnCommit
                            | ReinstallSnapshotCleanupOwner::Manifest,
                        ..
                    },
                ) if target.pre_state.is_some() => true,
                (
                    ReinstallTargetClass::Added,
                    ReinstallSnapshotState::CleanupPending {
                        purpose: ReinstallSnapshotPurpose::OriginalBackupCandidate,
                        ..
                    }
                    | ReinstallSnapshotState::Cleaned {
                        purpose: ReinstallSnapshotPurpose::OriginalBackupCandidate,
                    },
                ) if target.pre_state.is_some() => true,
                (
                    ReinstallTargetClass::Replaced | ReinstallTargetClass::Stale,
                    ReinstallSnapshotState::Stored {
                        purpose: ReinstallSnapshotPurpose::TransactionRollback,
                        cleanup_owner: ReinstallSnapshotCleanupOwner::Transaction,
                        ..
                    },
                ) => true,
                (
                    ReinstallTargetClass::Replaced | ReinstallTargetClass::Stale,
                    ReinstallSnapshotState::CleanupPending {
                        purpose: ReinstallSnapshotPurpose::TransactionRollback,
                        ..
                    }
                    | ReinstallSnapshotState::Cleaned {
                        purpose: ReinstallSnapshotPurpose::TransactionRollback,
                    },
                ) => true,
                _ => false,
            };
            if !ownership_valid {
                return Err(
                    ReinstallRecoveryTransactionValidationError::InvalidSnapshotOwnership {
                        target_path: target.target_path.clone(),
                    },
                );
            }

            if matches!(
                &target.snapshot,
                ReinstallSnapshotState::Stored { snapshot_ref, .. }
                    | ReinstallSnapshotState::CleanupPending { snapshot_ref, .. }
                    if snapshot_ref.trim().is_empty()
            ) {
                return Err(
                    ReinstallRecoveryTransactionValidationError::EmptySnapshotRef {
                        target_path: target.target_path.clone(),
                    },
                );
            }
        }

        Ok(())
    }

    pub fn transition_to(
        &mut self,
        next: ReinstallRecoveryTransactionStatus,
    ) -> Result<(), ReinstallRecoveryTransactionTransitionError> {
        if self.status.can_transition_to(next) {
            self.status = next;
            Ok(())
        } else {
            Err(
                ReinstallRecoveryTransactionTransitionError::InvalidTransition {
                    from: self.status,
                    to: next,
                },
            )
        }
    }
}

pub fn is_same_revision_replacement_target_switch(
    manifest: &InstallManifest,
    mod_id: &ModId,
    revision_id: &ModRevisionId,
    candidate_bindings: &[ReplacementBindingSnapshot],
) -> bool {
    let mut installed = manifest
        .replacement_bindings
        .iter()
        .filter(|snapshot| snapshot.mod_id() == mod_id);
    let (Some(installed), None) = (installed.next(), installed.next()) else {
        return false;
    };
    let [candidate] = candidate_bindings else {
        return false;
    };

    candidate.mod_id() == mod_id
        && candidate.profile_id() == &manifest.profile_id
        && candidate.revision_id() == Some(revision_id)
        && installed
            .revision_id()
            .is_none_or(|installed| installed == revision_id)
        && candidate.binding_id() == installed.binding_id()
        && candidate.binding().created_at_unix_millis()
            == installed.binding().created_at_unix_millis()
        && candidate.binding().source_id() == installed.binding().source_id()
        && candidate.source_internal_id() == installed.source_internal_id()
        && candidate.source_path_family() == installed.source_path_family()
        && candidate.target_path_family() == installed.target_path_family()
        && candidate.retarget_kind() == installed.retarget_kind()
        && candidate.binding().target_id() != installed.binding().target_id()
        && candidate.target_internal_id() != installed.target_internal_id()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinstallTargetClassification {
    pub target_path: InstallTargetPath,
    pub class: ReinstallTargetClass,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReinstallClassificationError {
    #[error("duplicate reinstall target state")]
    DuplicateTargetState { target_path: InstallTargetPath },
    #[error("reinstall target has no providers")]
    UnclassifiedTarget { target_path: InstallTargetPath },
    #[error("reinstall provider target does not match its target state")]
    ProviderTargetMismatch { target_path: InstallTargetPath },
    #[error("reinstall target is owned by another Mod")]
    CrossModTargetOwnership {
        target_path: InstallTargetPath,
        owner_mod_id: ModId,
    },
    #[error("reinstall provider stack contains a duplicate layer priority")]
    DuplicateLayerPriority {
        target_path: InstallTargetPath,
        priority: i32,
    },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReinstallManifestError {
    #[error("requested Mod has no installed manifest entries")]
    ModNotInstalled,
    #[error("candidate manifest entry set is empty")]
    CandidateEntriesEmpty,
    #[error("installed manifest entries mix legacy and revisioned facts")]
    MixedInstalledRevision,
    #[error("installed manifest entries contain multiple revisions")]
    MultipleInstalledRevisions,
    #[error("legacy installed revision has no provenance resolution")]
    LegacyRevisionUnresolved,
    #[error("legacy installed revision has ambiguous provenance")]
    LegacyRevisionAmbiguous,
    #[error("candidate manifest entry belongs to another Mod")]
    CandidateOwnerMismatch { owner_mod_id: ModId },
    #[error("candidate manifest entry is missing its revision")]
    CandidateRevisionMissing,
    #[error("candidate manifest entry revision does not match the requested candidate")]
    CandidateRevisionMismatch,
    #[error(
        "candidate replacement binding does not match the requested Mod, profile, or revision"
    )]
    CandidateReplacementBindingMismatch,
    #[error("reinstall target is also owned by another Mod")]
    CrossModTargetOwnership {
        target_path: InstallTargetPath,
        owner_mod_id: ModId,
    },
    #[error("manifest entry set contains a duplicate layer priority")]
    DuplicateLayerPriority {
        target_path: InstallTargetPath,
        priority: i32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ProviderSignature {
    priority: i32,
    layer_name: String,
    package_file_id: PackageFileId,
}

enum EntrySetRevision {
    Legacy,
    Revisioned(ModRevisionId),
}

pub fn classify_reinstall_targets<I, C>(
    requested_mod_id: &ModId,
    installed: I,
    candidate: C,
) -> Result<Vec<ReinstallTargetClassification>, ReinstallClassificationError>
where
    I: IntoIterator<Item = ReinstallTargetState>,
    C: IntoIterator<Item = ReinstallTargetState>,
{
    let installed = collect_target_states(requested_mod_id, installed)?;
    let candidate = collect_target_states(requested_mod_id, candidate)?;
    let target_paths = installed
        .keys()
        .chain(candidate.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    Ok(target_paths
        .into_iter()
        .map(|target_path| {
            let class = match (installed.get(&target_path), candidate.get(&target_path)) {
                (Some(installed), Some(candidate))
                    if provider_signature(installed) == provider_signature(candidate)
                        && installed.final_file == candidate.final_file =>
                {
                    ReinstallTargetClass::Retained
                }
                (Some(_), Some(_)) => ReinstallTargetClass::Replaced,
                (None, Some(_)) => ReinstallTargetClass::Added,
                (Some(_), None) => ReinstallTargetClass::Stale,
                (None, None) => unreachable!("target path comes from one of the state maps"),
            };
            ReinstallTargetClassification { target_path, class }
        })
        .collect())
}

pub fn resolve_installed_revision(
    manifest: &InstallManifest,
    requested_mod_id: &ModId,
    legacy_provenance: &[ModRevisionId],
) -> Result<ModRevisionId, ReinstallManifestError> {
    let installed_entries = entries_for_mod(manifest, requested_mod_id)?;
    match entry_set_revision(&installed_entries)? {
        EntrySetRevision::Revisioned(revision_id) => Ok(revision_id),
        EntrySetRevision::Legacy => {
            let revisions = legacy_provenance.iter().cloned().collect::<BTreeSet<_>>();
            match revisions.len() {
                0 => Err(ReinstallManifestError::LegacyRevisionUnresolved),
                1 => Ok(revisions
                    .into_iter()
                    .next()
                    .expect("one legacy provenance revision exists")),
                _ => Err(ReinstallManifestError::LegacyRevisionAmbiguous),
            }
        }
    }
}

pub fn replace_entries_for_mod(
    manifest: &InstallManifest,
    requested_mod_id: &ModId,
    legacy_provenance: &[ModRevisionId],
    candidate_revision_id: &ModRevisionId,
    candidate_entries: Vec<InstallManifestEntry>,
) -> Result<InstallManifest, ReinstallManifestError> {
    replace_entries_and_bindings_for_mod(
        manifest,
        requested_mod_id,
        legacy_provenance,
        candidate_revision_id,
        candidate_entries,
        Vec::new(),
    )
}

pub fn replace_entries_and_bindings_for_mod(
    manifest: &InstallManifest,
    requested_mod_id: &ModId,
    legacy_provenance: &[ModRevisionId],
    candidate_revision_id: &ModRevisionId,
    mut candidate_entries: Vec<InstallManifestEntry>,
    candidate_replacement_bindings: Vec<ReplacementBindingSnapshot>,
) -> Result<InstallManifest, ReinstallManifestError> {
    let installed_entries = entries_for_mod(manifest, requested_mod_id)?;
    resolve_installed_revision(manifest, requested_mod_id, legacy_provenance)?;

    if candidate_entries.is_empty() {
        return Err(ReinstallManifestError::CandidateEntriesEmpty);
    }

    for entry in &candidate_entries {
        if &entry.mod_id != requested_mod_id {
            return Err(ReinstallManifestError::CandidateOwnerMismatch {
                owner_mod_id: entry.mod_id.clone(),
            });
        }
        match &entry.revision_id {
            None => return Err(ReinstallManifestError::CandidateRevisionMissing),
            Some(revision_id) if revision_id != candidate_revision_id => {
                return Err(ReinstallManifestError::CandidateRevisionMismatch);
            }
            Some(_) => {}
        }
    }
    if candidate_replacement_bindings.iter().any(|snapshot| {
        snapshot.mod_id() != requested_mod_id
            || snapshot.profile_id() != &manifest.profile_id
            || snapshot.revision_id() != Some(candidate_revision_id)
    }) {
        return Err(ReinstallManifestError::CandidateReplacementBindingMismatch);
    }

    validate_entry_layer_priorities(installed_entries.iter().copied())?;
    validate_entry_layer_priorities(candidate_entries.iter())?;

    let protected_targets = installed_entries
        .iter()
        .map(|entry| entry.target_path.clone())
        .chain(
            candidate_entries
                .iter()
                .map(|entry| entry.target_path.clone()),
        )
        .collect::<BTreeSet<_>>();
    if let Some(entry) = manifest.entries.iter().find(|entry| {
        entry.mod_id != *requested_mod_id && protected_targets.contains(&entry.target_path)
    }) {
        return Err(ReinstallManifestError::CrossModTargetOwnership {
            target_path: entry.target_path.clone(),
            owner_mod_id: entry.mod_id.clone(),
        });
    }

    candidate_entries.sort_by(|left, right| {
        left.target_path
            .cmp(&right.target_path)
            .then_with(|| left.layer.priority.cmp(&right.layer.priority))
            .then_with(|| left.layer.name.cmp(&right.layer.name))
            .then_with(|| left.package_file_id.cmp(&right.package_file_id))
    });

    let mut updated = manifest.clone();
    updated.schema_version = INSTALL_MANIFEST_SCHEMA_VERSION_V2;
    updated
        .entries
        .retain(|entry| entry.mod_id != *requested_mod_id);
    updated.entries.extend(candidate_entries);
    updated
        .replacement_bindings
        .retain(|snapshot| snapshot.mod_id() != requested_mod_id);
    updated
        .replacement_bindings
        .extend(candidate_replacement_bindings);
    updated
        .validate()
        .map_err(|_| ReinstallManifestError::CandidateReplacementBindingMismatch)?;
    Ok(updated)
}

fn collect_target_states<I>(
    requested_mod_id: &ModId,
    states: I,
) -> Result<BTreeMap<InstallTargetPath, ReinstallTargetState>, ReinstallClassificationError>
where
    I: IntoIterator<Item = ReinstallTargetState>,
{
    let mut by_target = BTreeMap::new();
    for state in states {
        validate_target_state(requested_mod_id, &state)?;
        let target_path = state.target_path.clone();
        if by_target.insert(target_path.clone(), state).is_some() {
            return Err(ReinstallClassificationError::DuplicateTargetState { target_path });
        }
    }
    Ok(by_target)
}

fn validate_target_state(
    requested_mod_id: &ModId,
    state: &ReinstallTargetState,
) -> Result<(), ReinstallClassificationError> {
    if state.providers.is_empty() {
        return Err(ReinstallClassificationError::UnclassifiedTarget {
            target_path: state.target_path.clone(),
        });
    }

    let mut priorities = BTreeSet::new();
    for provider in &state.providers {
        if provider.target_path != state.target_path {
            return Err(ReinstallClassificationError::ProviderTargetMismatch {
                target_path: state.target_path.clone(),
            });
        }
        if &provider.mod_id != requested_mod_id {
            return Err(ReinstallClassificationError::CrossModTargetOwnership {
                target_path: state.target_path.clone(),
                owner_mod_id: provider.mod_id.clone(),
            });
        }
        if !priorities.insert(provider.layer.priority) {
            return Err(ReinstallClassificationError::DuplicateLayerPriority {
                target_path: state.target_path.clone(),
                priority: provider.layer.priority,
            });
        }
    }
    Ok(())
}

fn provider_signature(state: &ReinstallTargetState) -> Vec<ProviderSignature> {
    let mut signature = state
        .providers
        .iter()
        .map(|provider| ProviderSignature {
            priority: provider.layer.priority,
            layer_name: provider.layer.name.clone(),
            package_file_id: provider.package_file_id.clone(),
        })
        .collect::<Vec<_>>();
    signature.sort();
    signature
}

fn entries_for_mod<'a>(
    manifest: &'a InstallManifest,
    requested_mod_id: &ModId,
) -> Result<Vec<&'a InstallManifestEntry>, ReinstallManifestError> {
    let entries = manifest
        .entries
        .iter()
        .filter(|entry| entry.mod_id == *requested_mod_id)
        .collect::<Vec<_>>();
    if entries.is_empty() {
        Err(ReinstallManifestError::ModNotInstalled)
    } else {
        Ok(entries)
    }
}

fn entry_set_revision(
    entries: &[&InstallManifestEntry],
) -> Result<EntrySetRevision, ReinstallManifestError> {
    let has_legacy = entries.iter().any(|entry| entry.revision_id.is_none());
    let revisions = entries
        .iter()
        .filter_map(|entry| entry.revision_id.clone())
        .collect::<BTreeSet<_>>();

    if has_legacy && !revisions.is_empty() {
        return Err(ReinstallManifestError::MixedInstalledRevision);
    }
    if revisions.len() > 1 {
        return Err(ReinstallManifestError::MultipleInstalledRevisions);
    }
    Ok(match revisions.into_iter().next() {
        Some(revision_id) => EntrySetRevision::Revisioned(revision_id),
        None => EntrySetRevision::Legacy,
    })
}

fn validate_entry_layer_priorities<'a>(
    entries: impl IntoIterator<Item = &'a InstallManifestEntry>,
) -> Result<(), ReinstallManifestError> {
    let mut priorities_by_target = BTreeMap::<InstallTargetPath, BTreeSet<i32>>::new();
    for entry in entries {
        let priorities = priorities_by_target
            .entry(entry.target_path.clone())
            .or_default();
        if !priorities.insert(entry.layer.priority) {
            return Err(ReinstallManifestError::DuplicateLayerPriority {
                target_path: entry.target_path.clone(),
                priority: entry.layer.priority,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FileLayer, InstallFileProvider, InstallManifest, InstallManifestEntry,
        InstallManifestStatus, InstallTargetPath, InstalledFileSummary, ModId, ModRevisionId,
        PackageFileId, ProfileId, ReplacementBinding, ReplacementBindingId,
        ReplacementBindingSnapshot, ReplacementSourceId, ReplacementTargetId,
        ReplacementTargetKind,
    };
    use std::collections::BTreeMap;

    #[test]
    fn classifies_fixture_as_one_retained_two_replaced_one_added_one_stale() {
        let installed = vec![
            state(
                "retained.bin",
                "v1",
                vec![provider("retained.bin", "mod-a", "retained", "base", 0)],
                "same",
            ),
            state(
                "replaced.bin",
                "v1",
                vec![provider("replaced.bin", "mod-a", "replaced", "base", 0)],
                "replaced-v1",
            ),
            state(
                "overwritten.bin",
                "v1",
                vec![provider(
                    "overwritten.bin",
                    "mod-a",
                    "overwritten",
                    "base",
                    0,
                )],
                "overwritten-v1",
            ),
            state(
                "stale.bin",
                "v1",
                vec![provider("stale.bin", "mod-a", "stale", "base", 0)],
                "stale-v1",
            ),
        ];
        let candidate = vec![
            state(
                "retained.bin",
                "v2",
                vec![provider("retained.bin", "mod-a", "retained", "base", 0)],
                "same",
            ),
            state(
                "replaced.bin",
                "v2",
                vec![provider("replaced.bin", "mod-a", "replaced", "base", 0)],
                "replaced-v2",
            ),
            state(
                "overwritten.bin",
                "v2",
                vec![provider(
                    "overwritten.bin",
                    "mod-a",
                    "overwritten",
                    "base",
                    0,
                )],
                "overwritten-v2",
            ),
            state(
                "added-v2.bin",
                "v2",
                vec![provider("added-v2.bin", "mod-a", "added", "base", 0)],
                "added-v2",
            ),
        ];

        let classified = classify_reinstall_targets(&ModId::new("mod-a"), installed, candidate)
            .expect("fixture should classify");
        let by_target = classified
            .iter()
            .map(|target| (target.target_path.as_str(), target.class))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(
            by_target,
            BTreeMap::from([
                ("content/added-v2.bin", ReinstallTargetClass::Added),
                ("content/overwritten.bin", ReinstallTargetClass::Replaced,),
                ("content/replaced.bin", ReinstallTargetClass::Replaced,),
                ("content/retained.bin", ReinstallTargetClass::Retained,),
                ("content/stale.bin", ReinstallTargetClass::Stale),
            ])
        );
    }

    #[test]
    fn revision_change_alone_does_not_turn_identical_provider_bytes_into_replaced() {
        let installed = state(
            "same.bin",
            "v1",
            vec![provider("same.bin", "mod-a", "same", "base", 0)],
            "same",
        );
        let candidate = state(
            "same.bin",
            "v2",
            vec![provider("same.bin", "mod-a", "same", "base", 0)],
            "same",
        );

        let classified = classify_reinstall_targets(&ModId::new("mod-a"), [installed], [candidate])
            .expect("revision-only change should classify");

        assert_eq!(classified[0].class, ReinstallTargetClass::Retained);
    }

    #[test]
    fn provider_or_layer_change_is_replaced_even_when_final_bytes_match() {
        let installed = vec![
            state(
                "provider.bin",
                "v1",
                vec![provider("provider.bin", "mod-a", "old", "base", 0)],
                "same",
            ),
            state(
                "layer.bin",
                "v1",
                vec![provider("layer.bin", "mod-a", "same", "base", 0)],
                "same",
            ),
        ];
        let candidate = vec![
            state(
                "provider.bin",
                "v2",
                vec![provider("provider.bin", "mod-a", "new", "base", 0)],
                "same",
            ),
            state(
                "layer.bin",
                "v2",
                vec![provider("layer.bin", "mod-a", "same", "overlay", 10)],
                "same",
            ),
        ];

        let classified = classify_reinstall_targets(&ModId::new("mod-a"), installed, candidate)
            .expect("provider changes should classify");

        assert!(classified
            .iter()
            .all(|target| target.class == ReinstallTargetClass::Replaced));
    }

    #[test]
    fn classification_rejects_duplicate_or_unclassified_target_facts() {
        let duplicate = state(
            "duplicate.bin",
            "v1",
            vec![provider("duplicate.bin", "mod-a", "duplicate", "base", 0)],
            "duplicate",
        );

        let duplicate_error = classify_reinstall_targets(
            &ModId::new("mod-a"),
            [duplicate.clone(), duplicate],
            Vec::<ReinstallTargetState>::new(),
        )
        .expect_err("duplicate target facts must be rejected");
        assert!(matches!(
            duplicate_error,
            ReinstallClassificationError::DuplicateTargetState { .. }
        ));

        let unclassified = ReinstallTargetState::new(
            target("empty.bin"),
            ModRevisionId::new("v1"),
            Vec::new(),
            summary("empty"),
        );
        let unclassified_error = classify_reinstall_targets(
            &ModId::new("mod-a"),
            [unclassified],
            Vec::<ReinstallTargetState>::new(),
        )
        .expect_err("targets without providers must be rejected");
        assert!(matches!(
            unclassified_error,
            ReinstallClassificationError::UnclassifiedTarget { .. }
        ));

        let target_mismatch = state(
            "state.bin",
            "v1",
            vec![provider("provider.bin", "mod-a", "mismatch", "base", 0)],
            "mismatch",
        );
        let target_mismatch_error = classify_reinstall_targets(
            &ModId::new("mod-a"),
            [target_mismatch],
            Vec::<ReinstallTargetState>::new(),
        )
        .expect_err("provider targets must match their target state");
        assert!(matches!(
            target_mismatch_error,
            ReinstallClassificationError::ProviderTargetMismatch { .. }
        ));

        let duplicate_priority = state(
            "priority.bin",
            "v1",
            vec![
                provider("priority.bin", "mod-a", "base-a", "base", 0),
                provider("priority.bin", "mod-a", "base-b", "overlay", 0),
            ],
            "priority",
        );
        let duplicate_priority_error = classify_reinstall_targets(
            &ModId::new("mod-a"),
            [duplicate_priority],
            Vec::<ReinstallTargetState>::new(),
        )
        .expect_err("duplicate layer priorities must remain blocking conflicts");
        assert!(matches!(
            duplicate_priority_error,
            ReinstallClassificationError::DuplicateLayerPriority { priority: 0, .. }
        ));
    }

    #[test]
    fn classification_rejects_cross_mod_target_ownership() {
        let candidate = state(
            "owned.bin",
            "v2",
            vec![provider("owned.bin", "mod-b", "owned", "base", 0)],
            "owned",
        );

        let error = classify_reinstall_targets(
            &ModId::new("mod-a"),
            Vec::<ReinstallTargetState>::new(),
            [candidate],
        )
        .expect_err("another Mod owner must fail closed");

        assert!(matches!(
            error,
            ReinstallClassificationError::CrossModTargetOwnership { owner_mod_id, .. }
                if owner_mod_id == ModId::new("mod-b")
        ));
    }

    #[test]
    fn replace_entries_for_mod_removes_only_requested_mod_and_all_stale_entries() {
        let other_entry = entry("other.bin", "mod-b", "other", Some("other-v1"));
        let manifest = manifest(vec![
            entry("retained.bin", "mod-a", "retained-v1", Some("v1")),
            entry("stale.bin", "mod-a", "stale-v1", Some("v1")),
            other_entry.clone(),
        ]);
        let candidate_entries = vec![
            entry("retained.bin", "mod-a", "retained-v2", Some("v2")),
            entry("added.bin", "mod-a", "added-v2", Some("v2")),
        ];

        let replaced = replace_entries_for_mod(
            &manifest,
            &ModId::new("mod-a"),
            &[],
            &ModRevisionId::new("v2"),
            candidate_entries,
        )
        .expect("single-Mod entry set should be replaced");

        assert_eq!(manifest.entries.len(), 3, "input manifest stays immutable");
        assert_eq!(replaced.manifest_id, manifest.manifest_id);
        assert_eq!(
            replaced.schema_version,
            crate::INSTALL_MANIFEST_SCHEMA_VERSION_V2
        );
        assert_eq!(replaced.schema_migration, manifest.schema_migration);
        assert_eq!(replaced.backend, manifest.backend);
        assert_eq!(replaced.status, manifest.status);
        assert_eq!(replaced.created_at, manifest.created_at);
        assert_eq!(replaced.completed_at, manifest.completed_at);
        assert_eq!(replaced.plan_hash, manifest.plan_hash);
        assert!(replaced.entries.contains(&other_entry));
        assert!(!replaced
            .entries
            .iter()
            .any(|entry| entry.target_path == target("stale.bin")));
        assert_eq!(
            replaced
                .entries
                .iter()
                .filter(|entry| entry.mod_id == ModId::new("mod-a"))
                .map(|entry| entry.revision_id.as_ref().map(ModRevisionId::as_str))
                .collect::<Vec<_>>(),
            [Some("v2"), Some("v2")]
        );

        let serialized = serde_json::to_string(&replaced).expect("serialize v2 manifest");
        let reloaded: InstallManifest =
            serde_json::from_str(&serialized).expect("reload v2 manifest");
        assert_eq!(reloaded, replaced);
    }

    #[test]
    fn replace_entries_for_mod_rejects_mixed_revision_or_other_owner() {
        let mixed_manifest = manifest(vec![
            entry("old-a.bin", "mod-a", "old-a", None),
            entry("old-b.bin", "mod-a", "old-b", Some("v1")),
        ]);
        let candidate = vec![entry("candidate.bin", "mod-a", "candidate", Some("v2"))];

        let mixed_error = replace_entries_for_mod(
            &mixed_manifest,
            &ModId::new("mod-a"),
            &[],
            &ModRevisionId::new("v2"),
            candidate.clone(),
        )
        .expect_err("mixed legacy/new installed entries must be rejected");
        assert_eq!(mixed_error, ReinstallManifestError::MixedInstalledRevision);

        let multiple_revision_manifest = manifest(vec![
            entry("old-a.bin", "mod-a", "old-a", Some("v1")),
            entry("old-b.bin", "mod-a", "old-b", Some("v0")),
        ]);
        let multiple_revision_error = replace_entries_for_mod(
            &multiple_revision_manifest,
            &ModId::new("mod-a"),
            &[],
            &ModRevisionId::new("v2"),
            candidate.clone(),
        )
        .expect_err("installed entries from multiple revisions must be rejected");
        assert_eq!(
            multiple_revision_error,
            ReinstallManifestError::MultipleInstalledRevisions
        );

        let clean_manifest = manifest(vec![entry("old.bin", "mod-a", "old", Some("v1"))]);
        let owner_error = replace_entries_for_mod(
            &clean_manifest,
            &ModId::new("mod-a"),
            &[],
            &ModRevisionId::new("v2"),
            vec![entry("candidate.bin", "mod-b", "candidate", Some("v2"))],
        )
        .expect_err("candidate entries must belong to the requested Mod");
        assert!(matches!(
            owner_error,
            ReinstallManifestError::CandidateOwnerMismatch { owner_mod_id }
                if owner_mod_id == ModId::new("mod-b")
        ));

        let shared_target_manifest = manifest(vec![
            entry("shared.bin", "mod-a", "old", Some("v1")),
            entry("shared.bin", "mod-b", "other", Some("other-v1")),
        ]);
        let shared_error = replace_entries_for_mod(
            &shared_target_manifest,
            &ModId::new("mod-a"),
            &[],
            &ModRevisionId::new("v2"),
            candidate.clone(),
        )
        .expect_err("cross-Mod target ownership must fail closed");
        assert!(matches!(
            shared_error,
            ReinstallManifestError::CrossModTargetOwnership { .. }
        ));

        let candidate_target_manifest = manifest(vec![
            entry("old.bin", "mod-a", "old", Some("v1")),
            entry("candidate.bin", "mod-b", "other", Some("other-v1")),
        ]);
        let candidate_target_error = replace_entries_for_mod(
            &candidate_target_manifest,
            &ModId::new("mod-a"),
            &[],
            &ModRevisionId::new("v2"),
            candidate,
        )
        .expect_err("candidate targets owned by another Mod must fail closed");
        assert!(matches!(
            candidate_target_error,
            ReinstallManifestError::CrossModTargetOwnership { .. }
        ));
    }

    #[test]
    fn replace_entries_for_mod_rejects_missing_or_mismatched_candidate_revision() {
        let installed = manifest(vec![entry("old.bin", "mod-a", "old", Some("v1"))]);

        let missing_error = replace_entries_for_mod(
            &installed,
            &ModId::new("mod-a"),
            &[],
            &ModRevisionId::new("v2"),
            vec![entry("candidate.bin", "mod-a", "candidate", None)],
        )
        .expect_err("candidate entries without a revision must be rejected");
        assert_eq!(
            missing_error,
            ReinstallManifestError::CandidateRevisionMissing
        );

        let mismatch_error = replace_entries_for_mod(
            &installed,
            &ModId::new("mod-a"),
            &[],
            &ModRevisionId::new("v2"),
            vec![entry("candidate.bin", "mod-a", "candidate", Some("v3"))],
        )
        .expect_err("candidate entries from another revision must be rejected");
        assert_eq!(
            mismatch_error,
            ReinstallManifestError::CandidateRevisionMismatch
        );
    }

    #[test]
    fn legacy_entry_set_requires_one_provenance_resolved_revision() {
        let legacy = manifest(vec![
            entry("legacy-a.bin", "mod-a", "legacy-a", None),
            entry("legacy-b.bin", "mod-a", "legacy-b", None),
        ]);

        assert_eq!(
            resolve_installed_revision(&legacy, &ModId::new("mod-a"), &[]),
            Err(ReinstallManifestError::LegacyRevisionUnresolved)
        );
        assert_eq!(
            resolve_installed_revision(
                &legacy,
                &ModId::new("mod-a"),
                &[ModRevisionId::new("v1"), ModRevisionId::new("v2")],
            ),
            Err(ReinstallManifestError::LegacyRevisionAmbiguous)
        );
        assert_eq!(
            resolve_installed_revision(&legacy, &ModId::new("mod-a"), &[ModRevisionId::new("v1")],),
            Ok(ModRevisionId::new("v1"))
        );

        let candidate = vec![entry("legacy-a.bin", "mod-a", "candidate", Some("v2"))];
        assert_eq!(
            replace_entries_for_mod(
                &legacy,
                &ModId::new("mod-a"),
                &[],
                &ModRevisionId::new("v2"),
                candidate.clone(),
            ),
            Err(ReinstallManifestError::LegacyRevisionUnresolved)
        );
        assert_eq!(
            replace_entries_for_mod(
                &legacy,
                &ModId::new("mod-a"),
                &[ModRevisionId::new("v1"), ModRevisionId::new("v2")],
                &ModRevisionId::new("v2"),
                candidate.clone(),
            ),
            Err(ReinstallManifestError::LegacyRevisionAmbiguous)
        );
        let replaced = replace_entries_for_mod(
            &legacy,
            &ModId::new("mod-a"),
            &[ModRevisionId::new("v1")],
            &ModRevisionId::new("v2"),
            candidate,
        )
        .expect("one provenance-resolved legacy revision should allow replacement");
        assert!(replaced
            .entries
            .iter()
            .all(|entry| { entry.revision_id.as_ref() == Some(&ModRevisionId::new("v2")) }));

        let revisioned = manifest(vec![entry(
            "revisioned.bin",
            "mod-a",
            "revisioned",
            Some("v2"),
        )]);
        assert_eq!(
            resolve_installed_revision(&revisioned, &ModId::new("mod-a"), &[]),
            Ok(ModRevisionId::new("v2"))
        );
    }

    #[test]
    fn recovery_transaction_round_trips_all_target_classes_and_snapshot_ownership() {
        let transaction = recovery_transaction(ReinstallRecoveryTransactionStatus::Planned);

        let serialized = serde_json::to_string(&transaction).expect("serialize transaction");
        let reloaded: ReinstallRecoveryTransaction =
            serde_json::from_str(&serialized).expect("reload transaction");

        assert_eq!(reloaded, transaction);
        assert!(serialized.contains("\"class\":\"retained\""));
        assert!(serialized.contains("\"class\":\"replaced\""));
        assert!(serialized.contains("\"class\":\"added\""));
        assert!(serialized.contains("\"class\":\"stale\""));
        assert!(serialized.contains("\"cleanup_owner\":\"promote_on_commit\""));
        assert_eq!(reloaded.pre_reinstall_manifest.entries.len(), 3);
    }

    #[test]
    fn recovery_transaction_defaults_legacy_bindings_and_rejects_wrong_candidate_ownership() {
        let transaction = recovery_transaction(ReinstallRecoveryTransactionStatus::Planned);
        let mut legacy = serde_json::to_value(&transaction).expect("serialize transaction");
        legacy
            .as_object_mut()
            .expect("transaction object")
            .remove("candidate_replacement_bindings");
        let legacy: ReinstallRecoveryTransaction =
            serde_json::from_value(legacy).expect("legacy transaction");
        assert!(legacy.candidate_replacement_bindings.is_empty());

        for (mod_id, profile_id, revision_id) in [
            ("mod-b", "default", "v2"),
            ("mod-a", "other-profile", "v2"),
            ("mod-a", "default", "v3"),
        ] {
            let mut invalid = transaction.clone();
            invalid.candidate_replacement_bindings =
                vec![replacement_snapshot(mod_id, profile_id, revision_id)];
            assert_eq!(
                invalid.validate(),
                Err(
                    ReinstallRecoveryTransactionValidationError::InvalidCandidateReplacementBinding
                )
            );
        }
    }

    #[test]
    fn recovery_transaction_allows_only_a_proven_same_revision_replacement_target_switch() {
        let mut transaction = recovery_transaction(ReinstallRecoveryTransactionStatus::Planned);
        transaction.old_revision_id = ModRevisionId::new("v1");
        transaction.candidate_revision_id = ModRevisionId::new("v1");
        transaction.pre_reinstall_manifest.schema_version =
            crate::INSTALL_MANIFEST_SCHEMA_VERSION_V2;
        transaction.pre_reinstall_manifest.replacement_bindings =
            vec![replacement_snapshot_for_target(
                "binding-v2",
                "mod-a",
                "default",
                None,
                "mhw:armor:guardian-alpha",
                "pl121_0000",
            )];
        transaction.candidate_replacement_bindings = vec![replacement_snapshot_for_target(
            "binding-v2",
            "mod-a",
            "default",
            Some("v1"),
            "mhw:armor:fatalis-alpha",
            "pl129_0000",
        )];

        transaction
            .validate()
            .expect("same revision is valid when the persisted binding switches target");

        let mut unchanged = transaction.clone();
        unchanged.candidate_replacement_bindings = vec![replacement_snapshot_for_target(
            "binding-v2",
            "mod-a",
            "default",
            Some("v1"),
            "mhw:armor:guardian-alpha",
            "pl121_0000",
        )];
        assert_eq!(
            unchanged.validate(),
            Err(ReinstallRecoveryTransactionValidationError::RevisionUnchanged)
        );

        let mut unrelated_binding = transaction;
        let candidate = replacement_snapshot_for_target(
            "binding-other",
            "mod-a",
            "default",
            Some("v1"),
            "mhw:armor:fatalis-alpha",
            "pl129_0000",
        );
        unrelated_binding.candidate_replacement_bindings = vec![candidate];
        assert_eq!(
            unrelated_binding.validate(),
            Err(ReinstallRecoveryTransactionValidationError::RevisionUnchanged)
        );

        let mut changed_target_family =
            recovery_transaction(ReinstallRecoveryTransactionStatus::Planned);
        changed_target_family.old_revision_id = ModRevisionId::new("v1");
        changed_target_family.candidate_revision_id = ModRevisionId::new("v1");
        changed_target_family.pre_reinstall_manifest.schema_version =
            crate::INSTALL_MANIFEST_SCHEMA_VERSION_V2;
        changed_target_family
            .pre_reinstall_manifest
            .replacement_bindings = vec![replacement_snapshot_for_target(
            "binding-v2",
            "mod-a",
            "default",
            None,
            "mhw:armor:guardian-alpha",
            "pl121_0000",
        )];
        let mut candidate = replacement_snapshot_for_target(
            "binding-v2",
            "mod-a",
            "default",
            Some("v1"),
            "mhw:armor:fatalis-alpha",
            "pl129_0000",
        );
        candidate = ReplacementBindingSnapshot::new(
            candidate.binding().clone(),
            candidate.revision_id().cloned(),
            candidate.source_internal_id(),
            candidate.target_internal_id(),
            candidate.source_path_family(),
            "pl/m_equip",
            candidate.retarget_kind().clone(),
        )
        .expect("changed target family snapshot");
        changed_target_family.candidate_replacement_bindings = vec![candidate];
        assert_eq!(
            changed_target_family.validate(),
            Err(ReinstallRecoveryTransactionValidationError::RevisionUnchanged)
        );
    }

    #[test]
    fn recovery_transaction_round_trips_durable_snapshot_cleanup_progress() {
        let mut transaction = recovery_transaction(ReinstallRecoveryTransactionStatus::Planned);
        transaction.targets[1].snapshot = ReinstallSnapshotState::CleanupPending {
            snapshot_ref: "snapshot-replaced".to_owned(),
            purpose: ReinstallSnapshotPurpose::TransactionRollback,
        };
        transaction.targets[2].snapshot = ReinstallSnapshotState::Cleaned {
            purpose: ReinstallSnapshotPurpose::OriginalBackupCandidate,
        };

        transaction.validate().expect("valid cleanup progress");
        let serialized = serde_json::to_string(&transaction).expect("serialize transaction");
        let reloaded: ReinstallRecoveryTransaction =
            serde_json::from_str(&serialized).expect("reload transaction");

        assert_eq!(reloaded, transaction);
        assert!(serialized.contains("\"state\":\"cleanup_pending\""));
        assert!(serialized.contains("\"state\":\"cleaned\""));
    }

    #[test]
    fn recovery_transaction_rejects_invalid_status_transitions() {
        let mut transaction = recovery_transaction(ReinstallRecoveryTransactionStatus::Planned);

        let error = transaction
            .transition_to(ReinstallRecoveryTransactionStatus::Completed)
            .expect_err("planned cannot skip committing");
        assert_eq!(
            error,
            ReinstallRecoveryTransactionTransitionError::InvalidTransition {
                from: ReinstallRecoveryTransactionStatus::Planned,
                to: ReinstallRecoveryTransactionStatus::Completed,
            }
        );

        transaction
            .transition_to(ReinstallRecoveryTransactionStatus::Committing)
            .expect("planned can enter committing");
        transaction
            .transition_to(ReinstallRecoveryTransactionStatus::RollbackRequired)
            .expect("committing can require rollback");
        transaction
            .transition_to(ReinstallRecoveryTransactionStatus::RolledBack)
            .expect("rollback can complete");
        assert!(transaction
            .transition_to(ReinstallRecoveryTransactionStatus::Committing)
            .is_err());
    }

    fn state(
        path: &str,
        revision_id: &str,
        providers: Vec<InstallFileProvider>,
        file_hash: &str,
    ) -> ReinstallTargetState {
        ReinstallTargetState::new(
            target(path),
            ModRevisionId::new(revision_id),
            providers,
            summary(file_hash),
        )
    }

    fn provider(
        path: &str,
        mod_id: &str,
        package_file_id: &str,
        layer_name: &str,
        priority: i32,
    ) -> InstallFileProvider {
        InstallFileProvider::new(
            ModId::new(mod_id),
            PackageFileId::new(package_file_id),
            target(path),
            FileLayer::new(layer_name, priority),
        )
    }

    fn entry(
        path: &str,
        mod_id: &str,
        package_file_id: &str,
        revision_id: Option<&str>,
    ) -> InstallManifestEntry {
        InstallManifestEntry {
            target_path: target(path),
            mod_id: ModId::new(mod_id),
            revision_id: revision_id.map(ModRevisionId::new),
            package_file_id: PackageFileId::new(package_file_id),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: Some(summary(package_file_id)),
        }
    }

    fn manifest(entries: Vec<InstallManifestEntry>) -> InstallManifest {
        InstallManifest {
            profile_id: ProfileId::new("default"),
            manifest_id: "manifest-1".to_owned(),
            schema_version: 1,
            schema_migration: Some("legacy-compatible".to_owned()),
            backend: Some("install_plan".to_owned()),
            status: InstallManifestStatus::Completed,
            created_at: Some("2026-07-14T00:00:00Z".to_owned()),
            completed_at: Some("2026-07-14T00:00:01Z".to_owned()),
            plan_hash: Some("plan-v1".to_owned()),
            entries,
            replacement_bindings: Vec::new(),
        }
    }

    fn target(path: &str) -> InstallTargetPath {
        InstallTargetPath::parse(format!("content/{path}"), ["content"])
            .expect("test target should be valid")
    }

    fn summary(hash: &str) -> InstalledFileSummary {
        InstalledFileSummary {
            size_bytes: hash.len() as u64,
            sha256: hash.to_owned(),
        }
    }

    fn recovery_transaction(
        status: ReinstallRecoveryTransactionStatus,
    ) -> ReinstallRecoveryTransaction {
        let old_manifest = manifest(vec![
            entry("retained.bin", "mod-a", "retained-v1", None),
            entry("replaced.bin", "mod-a", "replaced-v1", None),
            entry("stale.bin", "mod-a", "stale-v1", None),
        ]);
        let target =
            |path: &str,
             class: ReinstallTargetClass,
             pre_state: Option<InstalledFileSummary>,
             candidate_state: Option<InstalledFileSummary>,
             snapshot: ReinstallSnapshotState,
             original_backup_ref: Option<&str>| ReinstallRecoveryTarget {
                target_path: target(path),
                class,
                pre_state,
                candidate_state,
                snapshot,
                original_backup_ref: original_backup_ref.map(str::to_owned),
            };

        ReinstallRecoveryTransaction {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            old_revision_id: ModRevisionId::new("v1"),
            candidate_revision_id: ModRevisionId::new("v2"),
            plan_token: "preview-token".to_owned(),
            plan_hash: "sha256:plan".to_owned(),
            status,
            pre_reinstall_manifest: old_manifest,
            candidate_replacement_bindings: Vec::new(),
            targets: vec![
                target(
                    "retained.bin",
                    ReinstallTargetClass::Retained,
                    Some(summary("same")),
                    Some(summary("same")),
                    ReinstallSnapshotState::NotRequired,
                    Some("original-retained"),
                ),
                target(
                    "replaced.bin",
                    ReinstallTargetClass::Replaced,
                    Some(summary("old")),
                    Some(summary("new")),
                    ReinstallSnapshotState::Stored {
                        snapshot_ref: "snapshot-replaced".to_owned(),
                        purpose: ReinstallSnapshotPurpose::TransactionRollback,
                        cleanup_owner: ReinstallSnapshotCleanupOwner::Transaction,
                    },
                    Some("original-replaced"),
                ),
                target(
                    "added.bin",
                    ReinstallTargetClass::Added,
                    Some(summary("unmanaged")),
                    Some(summary("candidate")),
                    ReinstallSnapshotState::Stored {
                        snapshot_ref: "snapshot-added".to_owned(),
                        purpose: ReinstallSnapshotPurpose::OriginalBackupCandidate,
                        cleanup_owner: ReinstallSnapshotCleanupOwner::PromoteOnCommit,
                    },
                    None,
                ),
                target(
                    "stale.bin",
                    ReinstallTargetClass::Stale,
                    Some(summary("stale")),
                    None,
                    ReinstallSnapshotState::Stored {
                        snapshot_ref: "snapshot-stale".to_owned(),
                        purpose: ReinstallSnapshotPurpose::TransactionRollback,
                        cleanup_owner: ReinstallSnapshotCleanupOwner::Transaction,
                    },
                    Some("original-stale"),
                ),
            ],
        }
    }

    fn replacement_snapshot(
        mod_id: &str,
        profile_id: &str,
        revision_id: &str,
    ) -> ReplacementBindingSnapshot {
        replacement_snapshot_for_target(
            "binding-v2",
            mod_id,
            profile_id,
            Some(revision_id),
            "mhw:armor:fatalis-alpha",
            "pl129_0000",
        )
    }

    fn replacement_snapshot_for_target(
        binding_id: &str,
        mod_id: &str,
        profile_id: &str,
        revision_id: Option<&str>,
        target_id: &str,
        target_internal_id: &str,
    ) -> ReplacementBindingSnapshot {
        ReplacementBindingSnapshot::new(
            ReplacementBinding::new(
                ReplacementBindingId::parse(binding_id).expect("binding id"),
                ModId::new(mod_id),
                ProfileId::new(profile_id),
                ReplacementSourceId::parse("mhw:armor:f_equip:pl121_0000").expect("source id"),
                ReplacementTargetId::parse(target_id).expect("target id"),
                42,
            )
            .expect("binding"),
            revision_id.map(ModRevisionId::new),
            "pl121_0000",
            target_internal_id,
            "pl/f_equip",
            "pl/f_equip",
            ReplacementTargetKind::parse("armor").expect("replacement kind"),
        )
        .expect("replacement snapshot")
    }
}
