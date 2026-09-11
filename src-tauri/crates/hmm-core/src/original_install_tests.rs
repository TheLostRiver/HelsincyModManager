use super::*;
use crate::{
    InstallFileProvider, InstallManifestEntry, ReinstallRecoveryTarget,
    ReinstallRecoveryTransaction, ReinstallRecoveryTransactionStatus,
    ReinstallRecoveryTransactionValidationError, ReinstallSnapshotCleanupOwner,
    ReinstallSnapshotPurpose, ReinstallSnapshotState, ReinstallTargetClass, ReplacementBinding,
    ReplacementBindingId, ReplacementSourceId, ReplacementTargetId, ReplacementTargetKind,
};

fn path(name: &str) -> InstallTargetPath {
    InstallTargetPath::parse(format!("content/{name}.bin"), ["content"]).unwrap()
}
fn summary(value: char) -> InstalledFileSummary {
    InstalledFileSummary {
        size_bytes: 1,
        sha256: value.to_string().repeat(64),
    }
}
fn binding(source: &str, target: &str) -> ReplacementBindingSnapshot {
    ReplacementBindingSnapshot::new(
        ReplacementBinding::new(
            ReplacementBindingId::parse(format!("binding-{source}")).unwrap(),
            ModId::new("mod"),
            ProfileId::new("profile"),
            ReplacementSourceId::parse(format!("source-{source}")).unwrap(),
            ReplacementTargetId::parse(format!("target-{target}")).unwrap(),
            0,
        )
        .unwrap(),
        Some(ModRevisionId::new("revision")),
        source,
        target,
        "fixture/weapon",
        "fixture/weapon",
        ReplacementTargetKind::parse("weapon").unwrap(),
    )
    .unwrap()
}

struct Facts {
    manifest: InstallManifest,
    plan: InstallPlan,
    sources: BTreeMap<PackageFileId, InstalledFileSummary>,
    game: BTreeMap<InstallTargetPath, InstalledFileSummary>,
}

impl Facts {
    fn new() -> Self {
        let entries = [("a", '1'), ("b", '2')]
            .map(|(name, value)| InstallManifestEntry {
                target_path: path(name),
                mod_id: ModId::new("mod"),
                revision_id: Some(ModRevisionId::new("revision")),
                package_file_id: PackageFileId::new(name),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: Some(summary(value)),
                adopted: false,
            })
            .to_vec();
        let plan = InstallPlan::from_providers(entries.iter().map(|entry| {
            InstallFileProvider::new(
                entry.mod_id.clone(),
                entry.package_file_id.clone(),
                entry.target_path.clone(),
                entry.layer.clone(),
            )
        }))
        .with_replacement_bindings(vec![binding("a", "a"), binding("b", "b")])
        .unwrap();
        let sources = entries
            .iter()
            .map(|entry| {
                (
                    entry.package_file_id.clone(),
                    entry.installed_file.clone().unwrap(),
                )
            })
            .collect();
        let game = entries
            .iter()
            .map(|entry| {
                (
                    entry.target_path.clone(),
                    entry.installed_file.clone().unwrap(),
                )
            })
            .collect();
        let mut manifest = InstallManifest::completed(ProfileId::new("profile"), entries);
        manifest.schema_version = 2;
        Self {
            manifest,
            plan,
            sources,
            game,
        }
    }
    fn verify(&self) -> Result<OriginalInstallEvidence, OriginalInstallEvidenceError> {
        OriginalInstallEvidence::verify(
            &self.manifest,
            &ModId::new("mod"),
            &ModRevisionId::new("revision"),
            &self.plan,
            &self.sources,
            &self.game,
        )
    }
}

#[test]
fn matching_complete_original_facts_allow_only_a_real_source_preserving_switch() {
    let facts = Facts::new();
    let before = facts.manifest.clone();
    let evidence = facts.verify().unwrap();
    assert!(evidence
        .allows_equipment_target_switch(&facts.manifest, &[binding("a", "c"), binding("b", "b")]));
    assert!(!evidence
        .allows_single_target_switch(&facts.manifest, &[binding("a", "c"), binding("b", "b")]));
    assert!(!evidence.allows_equipment_target_switch(&facts.manifest, evidence.bindings()));
    assert!(!evidence.allows_equipment_target_switch(
        &facts.manifest,
        &[binding("a", "c"), binding("other", "d")]
    ));
    assert_eq!(facts.manifest, before);
    assert_eq!(
        serde_json::from_value::<OriginalInstallEvidence>(serde_json::to_value(&evidence).unwrap())
            .unwrap(),
        evidence
    );
}

#[test]
fn file_contents_and_inventory_must_match_all_three_sources_of_facts() {
    for change in [
        "source",
        "game",
        "forged-summary",
        "missing-source",
        "extra-game",
        "file-id",
        "path",
        "layer",
    ] {
        let mut facts = Facts::new();
        match change {
            "source" => {
                facts.sources.insert(PackageFileId::new("a"), summary('3'));
            }
            "game" => {
                facts.game.insert(path("a"), summary('3'));
            }
            "forged-summary" => {
                facts.manifest.entries[0].installed_file = Some(summary('3'));
                facts.game.insert(path("a"), summary('3'));
            }
            "missing-source" => {
                facts.sources.remove(&PackageFileId::new("a"));
            }
            "extra-game" => {
                facts.game.insert(path("extra"), summary('3'));
            }
            "file-id" => {
                facts.plan.actions[0].provider.package_file_id = PackageFileId::new("another");
            }
            "path" => {
                facts.plan.actions[0].target_path = path("another");
            }
            "layer" => {
                facts.plan.actions[0].provider.layer.priority = 2;
            }
            _ => unreachable!(),
        }
        assert!(facts.verify().is_err(), "accepted {change}");
    }
}

#[test]
fn persisted_evidence_cannot_override_existing_ownership_or_binding_metadata() {
    let facts = Facts::new();
    let evidence = facts.verify().unwrap();
    for change in [
        "profile",
        "revision",
        "adopted",
        "bindings",
        "summary",
        "foreign-owner",
    ] {
        let mut manifest = facts.manifest.clone();
        match change {
            "profile" => {
                manifest.profile_id = ProfileId::new("another");
            }
            "revision" => {
                manifest.entries[0].revision_id = Some(ModRevisionId::new("another"));
            }
            "adopted" => {
                manifest.entries[0].adopted = true;
            }
            "bindings" => {
                manifest.replacement_bindings = vec![binding("a", "a")];
            }
            "summary" => {
                manifest.entries[0].installed_file = None;
            }
            "foreign-owner" => {
                let mut foreign = manifest.entries[0].clone();
                foreign.mod_id = ModId::new("another");
                foreign.target_path =
                    InstallTargetPath::parse("content/A.bin", ["content"]).unwrap();
                manifest.entries.push(foreign);
            }
            _ => unreachable!(),
        }
        assert!(evidence.validate(&manifest).is_err(), "accepted {change}");
    }
}

#[test]
fn recovery_requires_the_original_evidence_and_retains_the_unmodified_manifest() {
    let facts = Facts::new();
    let evidence = facts.verify().unwrap();
    let mut transaction = ReinstallRecoveryTransaction {
        profile_id: ProfileId::new("profile"),
        mod_id: ModId::new("mod"),
        old_revision_id: ModRevisionId::new("revision"),
        candidate_revision_id: ModRevisionId::new("revision"),
        plan_token: "preview".into(),
        plan_hash: "hash".into(),
        status: ReinstallRecoveryTransactionStatus::Planned,
        pre_reinstall_manifest: facts.manifest.clone(),
        original_install_evidence: Some(evidence),
        candidate_replacement_bindings: vec![binding("a", "c"), binding("b", "b")],
        targets: vec![
            ReinstallRecoveryTarget {
                target_path: path("a"),
                class: ReinstallTargetClass::Stale,
                pre_state: Some(summary('1')),
                candidate_state: None,
                snapshot: ReinstallSnapshotState::Stored {
                    snapshot_ref: "rollback-a".into(),
                    purpose: ReinstallSnapshotPurpose::TransactionRollback,
                    cleanup_owner: ReinstallSnapshotCleanupOwner::Transaction,
                },
                original_backup_ref: None,
            },
            ReinstallRecoveryTarget {
                target_path: path("b"),
                class: ReinstallTargetClass::Retained,
                pre_state: Some(summary('2')),
                candidate_state: Some(summary('2')),
                snapshot: ReinstallSnapshotState::NotRequired,
                original_backup_ref: None,
            },
            ReinstallRecoveryTarget {
                target_path: path("c"),
                class: ReinstallTargetClass::Added,
                pre_state: None,
                candidate_state: Some(summary('1')),
                snapshot: ReinstallSnapshotState::PreStateAbsent,
                original_backup_ref: None,
            },
        ],
    };
    transaction.validate().unwrap();
    assert_eq!(transaction.pre_reinstall_manifest, facts.manifest);
    let loaded: ReinstallRecoveryTransaction =
        serde_json::from_value(serde_json::to_value(&transaction).unwrap()).unwrap();
    loaded.validate().unwrap();
    transaction.original_install_evidence = None;
    assert_eq!(
        transaction.validate(),
        Err(ReinstallRecoveryTransactionValidationError::RevisionUnchanged)
    );
}
