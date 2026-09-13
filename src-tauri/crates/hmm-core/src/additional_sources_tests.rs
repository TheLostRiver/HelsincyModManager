use super::*;
use crate::{
    FileLayer, InstallFileProvider, InstallManifestEntry, ReinstallRecoveryTarget,
    ReinstallRecoveryTransaction, ReinstallRecoveryTransactionStatus, ReinstallSnapshotState,
    ReinstallTargetClass, ReplacementBinding, ReplacementBindingId, ReplacementTargetId,
    ReplacementTargetKind,
};

fn path(id: &str) -> InstallTargetPath {
    InstallTargetPath::parse(format!("content/{id}.bin"), ["content"]).unwrap()
}
fn summary() -> InstalledFileSummary {
    InstalledFileSummary {
        size_bytes: 1,
        sha256: "a".repeat(64),
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
        "fixture/equipment",
        "fixture/equipment",
        ReplacementTargetKind::parse("equipment").unwrap(),
    )
    .unwrap()
}

struct Fixture {
    manifest: InstallManifest,
    plan: InstallPlan,
    inventory: BTreeMap<ReplacementSourceId, BTreeSet<PackageFileId>>,
    originals: BTreeMap<PackageFileId, InstalledFileSummary>,
    actual: BTreeMap<InstallTargetPath, InstalledFileSummary>,
}
impl Fixture {
    fn new() -> Self {
        let entries = [("a", "c"), ("b", "b")]
            .map(|(file, target)| InstallManifestEntry {
                target_path: path(target),
                mod_id: ModId::new("mod"),
                revision_id: Some(ModRevisionId::new("revision")),
                package_file_id: PackageFileId::new(file),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: Some(summary()),
                adopted: false,
            })
            .to_vec();
        let mut manifest = InstallManifest::completed(ProfileId::new("profile"), entries);
        manifest.schema_version = 2;
        manifest.replacement_bindings = vec![binding("a", "c")];
        let plan = InstallPlan::from_providers(["a", "b"].map(|id| {
            InstallFileProvider::new(
                ModId::new("mod"),
                PackageFileId::new(id),
                path(id),
                FileLayer::new("base", 0),
            )
        }))
        .with_replacement_bindings(vec![binding("a", "a"), binding("b", "b")])
        .unwrap();
        Self {
            manifest,
            plan,
            inventory: ["a", "b"]
                .map(|id| {
                    (
                        ReplacementSourceId::parse(format!("source-{id}")).unwrap(),
                        BTreeSet::from([PackageFileId::new(id)]),
                    )
                })
                .into(),
            originals: BTreeMap::from([(PackageFileId::new("b"), summary())]),
            actual: BTreeMap::from([(path("b"), summary())]),
        }
    }
    fn verify(&self) -> Result<AdditionalSourcesEvidence, Error> {
        AdditionalSourcesEvidence::verify(
            &self.manifest,
            &ModId::new("mod"),
            &ModRevisionId::new("revision"),
            &self.plan,
            &self.inventory,
            &self.originals,
            &self.actual,
        )
    }
}

#[test]
fn additional_sources_preserve_existing_targets_and_do_not_authorize_metadata_only_writes() {
    let fixture = Fixture::new();
    let before = fixture.manifest.clone();
    let evidence = fixture.verify().unwrap();
    assert_eq!(evidence.bindings().count(), 1);
    assert_eq!(evidence.files().count(), 1);
    assert!(evidence.allows_equipment_target_switch(
        &fixture.manifest,
        &[binding("a", "c"), binding("b", "d")]
    ));
    assert!(evidence
        .allows_equipment_reapply(&fixture.manifest, &[binding("a", "c"), binding("b", "b")]));
    assert!(!evidence.allows_equipment_target_switch(
        &fixture.manifest,
        &[binding("a", "c"), binding("b", "b")]
    ));
    assert!(!evidence
        .allows_equipment_reapply(&fixture.manifest, &[binding("a", "a"), binding("b", "b")]));
    assert!(!crate::is_same_revision_equipment_target_switch(
        &fixture.manifest,
        &ModId::new("mod"),
        &ModRevisionId::new("revision"),
        &[binding("a", "c"), binding("b", "d")]
    ));
    assert_eq!(fixture.manifest, before);
}

#[test]
fn additional_sources_require_original_bytes_current_ownership_and_complete_inventory() {
    for mutation in [
        "original",
        "actual",
        "missing",
        "owner",
        "adopted",
        "revision",
        "summary",
        "path",
        "inventory",
        "bindings",
    ] {
        let mut f = Fixture::new();
        match mutation {
            "original" => {
                f.originals
                    .get_mut(&PackageFileId::new("b"))
                    .unwrap()
                    .size_bytes = 2
            }
            "actual" => f.actual.get_mut(&path("b")).unwrap().size_bytes = 2,
            "missing" => {
                f.originals.clear();
            }
            "owner" => f.manifest.entries[1].mod_id = ModId::new("foreign"),
            "adopted" => f.manifest.entries[1].adopted = true,
            "revision" => f.manifest.entries[1].revision_id = Some(ModRevisionId::new("other")),
            "summary" => f.manifest.entries[1].installed_file = None,
            "path" => f.manifest.entries[1].target_path = path("other"),
            "inventory" => {
                f.inventory
                    .get_mut(&ReplacementSourceId::parse("source-b").unwrap())
                    .unwrap()
                    .insert(PackageFileId::new("a"));
            }
            "bindings" => f.manifest.replacement_bindings.clear(),
            _ => unreachable!(),
        }
        assert!(f.verify().is_err(), "{mutation}");
    }
}

#[test]
fn persisted_additional_sources_reject_changed_baseline_bindings_and_forged_new_targets() {
    let fixture = Fixture::new();
    let evidence = fixture.verify().unwrap();
    let encoded = serde_json::to_value(&evidence).unwrap();
    let loaded: AdditionalSourcesEvidence = serde_json::from_value(encoded.clone()).unwrap();
    loaded.validate(&fixture.manifest).unwrap();
    let mut changed = fixture.manifest.clone();
    changed.replacement_bindings[0] = binding("a", "d");
    assert!(loaded.validate(&changed).is_err());
    for field in [
        "target_internal_id",
        "source_path_family",
        "target_path_family",
    ] {
        let mut forged = encoded.clone();
        forged["sources"][0]["binding"][field] = serde_json::json!("different");
        let forged: AdditionalSourcesEvidence = serde_json::from_value(forged).unwrap();
        assert!(forged.validate(&fixture.manifest).is_err(), "{field}");
    }
}

#[test]
fn recovery_persists_additional_evidence_and_keeps_the_exact_previous_manifest() {
    let fixture = Fixture::new();
    let mut transaction = ReinstallRecoveryTransaction {
        profile_id: ProfileId::new("profile"),
        mod_id: ModId::new("mod"),
        old_revision_id: ModRevisionId::new("revision"),
        candidate_revision_id: ModRevisionId::new("revision"),
        intent: crate::ReinstallIntent::Standard,
        plan_token: "token".into(),
        plan_hash: "hash".into(),
        status: ReinstallRecoveryTransactionStatus::Planned,
        pre_reinstall_manifest: fixture.manifest.clone(),
        original_install_evidence: None,
        source_evidence_version: 1,
        additional_sources_evidence: Some(fixture.verify().unwrap()),
        candidate_replacement_bindings: vec![binding("a", "c"), binding("b", "d")],
        candidate_plugin_selections: vec![],
        targets: vec![
            ReinstallRecoveryTarget {
                target_path: path("c"),
                class: ReinstallTargetClass::Retained,
                pre_state: Some(summary()),
                candidate_state: Some(summary()),
                snapshot: ReinstallSnapshotState::NotRequired,
                original_backup_ref: None,
            },
            ReinstallRecoveryTarget {
                target_path: path("b"),
                class: ReinstallTargetClass::Stale,
                pre_state: Some(summary()),
                candidate_state: None,
                snapshot: ReinstallSnapshotState::Stored {
                    snapshot_ref: "rollback-b".into(),
                    purpose: crate::ReinstallSnapshotPurpose::TransactionRollback,
                    cleanup_owner: crate::ReinstallSnapshotCleanupOwner::Transaction,
                },
                original_backup_ref: None,
            },
            ReinstallRecoveryTarget {
                target_path: path("d"),
                class: ReinstallTargetClass::Added,
                pre_state: None,
                candidate_state: Some(summary()),
                snapshot: ReinstallSnapshotState::PreStateAbsent,
                original_backup_ref: None,
            },
        ],
    };
    transaction.validate().unwrap();
    let loaded: ReinstallRecoveryTransaction =
        serde_json::from_value(serde_json::to_value(&transaction).unwrap()).unwrap();
    loaded.validate().unwrap();
    assert_eq!(loaded.pre_reinstall_manifest, fixture.manifest);
    transaction.additional_sources_evidence = None;
    assert!(transaction.validate().is_err());
    // 升级不能把旧版本已经开始的多源事务变成不可恢复。
    transaction.source_evidence_version = 0;
    let mut encoded = serde_json::to_value(transaction).unwrap();
    encoded
        .as_object_mut()
        .unwrap()
        .remove("source_evidence_version");
    let legacy: ReinstallRecoveryTransaction = serde_json::from_value(encoded).unwrap();
    legacy.validate().unwrap();
    assert_eq!(legacy.pre_reinstall_manifest, fixture.manifest);
}
