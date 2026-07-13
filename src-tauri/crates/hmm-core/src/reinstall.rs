use crate::{
    InstallFileProvider, InstallManifest, InstallManifestEntry, InstallTargetPath,
    InstalledFileSummary, ModId, ModRevisionId, PackageFileId,
};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReinstallTargetClass {
    Retained,
    Replaced,
    Added,
    Stale,
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
    mut candidate_entries: Vec<InstallManifestEntry>,
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
    updated
        .entries
        .retain(|entry| entry.mod_id != *requested_mod_id);
    updated.entries.extend(candidate_entries);
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
        PackageFileId, ProfileId,
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
        assert_eq!(replaced.schema_version, manifest.schema_version);
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
}
