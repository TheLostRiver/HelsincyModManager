use super::*;
use hmm_core::{
    ReplacementBinding, ReplacementBindingId, ReplacementBindingSnapshot, ReplacementSourceId,
    ReplacementTargetId, ReplacementTargetKind,
};

#[test]
fn scan_marks_reinstall_repair_required_when_candidate_binding_snapshot_differs() {
    let (_old_manifest, mut candidate_manifest, transaction, target) = reinstall_recovery_fixture();
    candidate_manifest.replacement_bindings = vec![reinstall_replacement_snapshot(
        "binding-drifted",
        "mhw:armor:alatreon-alpha",
        "pl127_0000",
        "candidate-v2",
    )];
    let game_files = Arc::new(FakeGameFiles::default());
    game_files
        .files
        .lock()
        .expect("game files lock")
        .insert(target.as_str().to_owned(), b"candidate-v2".to_vec());
    let transactions = Arc::new(FakeReinstallTransactions::default());
    transactions.insert(transaction);
    let snapshots = Arc::new(FakeReinstallSnapshots::default());
    snapshots
        .snapshots
        .lock()
        .expect("snapshots lock")
        .insert("snapshot-v1".to_owned(), b"installed-v1".to_vec());
    let service = InstallRecoveryScanService::new(
        game_files,
        Arc::new(FakeBackups::default()),
        Arc::new(FakeManifests {
            manifest: Some(candidate_manifest),
        }),
    )
    .with_reinstall_recovery_transactions(transactions, snapshots);

    let summaries = service
        .scan(InstallRecoveryScanRequest {
            profile_id: ProfileId::new("default"),
            mod_ids: vec![ModId::new("mod-a")],
        })
        .expect("scan returns fail-closed status");

    assert_eq!(summaries[0].status, InstallRecoveryStatus::RepairRequired);
}

pub(super) fn reinstall_recovery_fixture() -> (
    InstallManifest,
    InstallManifest,
    ReinstallRecoveryTransaction,
    InstallTargetPath,
) {
    let profile_id = ProfileId::new("default");
    let mod_id = ModId::new("mod-a");
    let old_revision_id = ModRevisionId::new("installed-v1");
    let candidate_revision_id = ModRevisionId::new("candidate-v2");
    let target =
        InstallTargetPath::parse("nativePC/reinstall.bin", ["nativePC"]).expect("reinstall target");
    let mut old_manifest = InstallManifest::completed(
        profile_id.clone(),
        vec![InstallManifestEntry {
            target_path: target.clone(),
            mod_id: mod_id.clone(),
            revision_id: Some(old_revision_id.clone()),
            package_file_id: PackageFileId::new("old/reinstall.bin"),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: Some(installed_file_summary(b"installed-v1")),
        }],
    );
    old_manifest.schema_version = hmm_core::INSTALL_MANIFEST_SCHEMA_VERSION_V2;
    old_manifest.plan_hash = Some("old-plan-hash".to_owned());
    old_manifest.replacement_bindings = vec![reinstall_replacement_snapshot(
        "binding-v1",
        "mhw:armor:guardian-alpha",
        "pl121_0000",
        "installed-v1",
    )];

    let mut candidate_manifest = old_manifest.clone();
    candidate_manifest.entries[0].revision_id = Some(candidate_revision_id.clone());
    candidate_manifest.entries[0].package_file_id = PackageFileId::new("new/reinstall.bin");
    candidate_manifest.entries[0].installed_file = Some(installed_file_summary(b"candidate-v2"));
    candidate_manifest.plan_hash = Some("candidate-plan-hash".to_owned());
    let candidate_replacement_bindings = vec![reinstall_replacement_snapshot(
        "binding-v2",
        "mhw:armor:fatalis-alpha",
        "pl129_0000",
        "candidate-v2",
    )];
    candidate_manifest.replacement_bindings = candidate_replacement_bindings.clone();

    let transaction = ReinstallRecoveryTransaction {
        profile_id,
        mod_id,
        old_revision_id,
        candidate_revision_id,
        plan_token: "opaque-plan-token".to_owned(),
        plan_hash: "candidate-plan-hash".to_owned(),
        status: ReinstallRecoveryTransactionStatus::Committing,
        pre_reinstall_manifest: old_manifest.clone(),
        candidate_replacement_bindings,
        targets: vec![ReinstallRecoveryTarget {
            target_path: target.clone(),
            class: ReinstallTargetClass::Replaced,
            pre_state: Some(installed_file_summary(b"installed-v1")),
            candidate_state: Some(installed_file_summary(b"candidate-v2")),
            snapshot: ReinstallSnapshotState::Stored {
                snapshot_ref: "snapshot-v1".to_owned(),
                purpose: ReinstallSnapshotPurpose::TransactionRollback,
                cleanup_owner: ReinstallSnapshotCleanupOwner::Transaction,
            },
            original_backup_ref: None,
        }],
    };
    transaction.validate().expect("valid reinstall transaction");

    (old_manifest, candidate_manifest, transaction, target)
}

fn reinstall_replacement_snapshot(
    binding_id: &str,
    target_id: &str,
    target_internal_id: &str,
    revision_id: &str,
) -> ReplacementBindingSnapshot {
    ReplacementBindingSnapshot::new(
        ReplacementBinding::new(
            ReplacementBindingId::parse(binding_id).expect("binding id"),
            ModId::new("mod-a"),
            ProfileId::new("default"),
            ReplacementSourceId::parse("mhw:armor:f_equip:pl121_0000").expect("source id"),
            ReplacementTargetId::parse(target_id).expect("target id"),
            42,
        )
        .expect("binding"),
        Some(ModRevisionId::new(revision_id)),
        "pl121_0000",
        target_internal_id,
        "pl/f_equip",
        "pl/f_equip",
        ReplacementTargetKind::parse("armor").expect("replacement kind"),
    )
    .expect("replacement snapshot")
}
