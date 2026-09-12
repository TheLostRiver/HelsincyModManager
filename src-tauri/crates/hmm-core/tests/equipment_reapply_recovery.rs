use hmm_core::{
    FileLayer, InstallManifest, InstallManifestEntry, InstallManifestStatus, InstallTargetPath,
    InstalledFileSummary, ModId, ModRevisionId, PackageFileId, ProfileId,
    ReinstallRecoveryTransaction, ReplacementBinding, ReplacementBindingId,
    ReplacementBindingSnapshot, ReplacementSourceId, ReplacementTargetId, ReplacementTargetKind,
};
use serde_json::{json, Value};

fn binding(source: &str) -> ReplacementBindingSnapshot {
    ReplacementBindingSnapshot::new(
        ReplacementBinding::new(
            ReplacementBindingId::parse(format!("binding-{source}")).unwrap(),
            ModId::new("mod"),
            ProfileId::new("profile"),
            ReplacementSourceId::parse(format!("source-{source}")).unwrap(),
            ReplacementTargetId::parse(format!("target-{source}")).unwrap(),
            1,
        )
        .unwrap(),
        Some(ModRevisionId::new("r1")),
        source,
        source,
        "fixture/equipment",
        "fixture/equipment",
        ReplacementTargetKind::parse("weapon").unwrap(),
    )
    .unwrap()
}

fn summary(value: &str) -> InstalledFileSummary {
    InstalledFileSummary {
        size_bytes: 1,
        sha256: value.repeat(64),
    }
}

fn transaction(source_count: usize) -> Value {
    let sources = &['a', 'b'][..source_count];
    let manifest = InstallManifest {
        profile_id: ProfileId::new("profile"),
        manifest_id: "fixture-manifest".into(),
        schema_version: 2,
        schema_migration: None,
        backend: None,
        status: InstallManifestStatus::Completed,
        created_at: None,
        completed_at: None,
        plan_hash: None,
        entries: sources
            .iter()
            .map(|source| InstallManifestEntry {
                target_path: InstallTargetPath::parse(
                    format!("content/{source}/resource.bin"),
                    ["content"],
                )
                .unwrap(),
                mod_id: ModId::new("mod"),
                revision_id: Some(ModRevisionId::new("r1")),
                package_file_id: PackageFileId::new(format!("file-{source}")),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: Some(summary("a")),
                adopted: false,
            })
            .collect(),
        replacement_bindings: sources
            .iter()
            .map(|source| binding(&source.to_string()))
            .collect(),
    };
    let targets = manifest
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            json!({
                "target_path": entry.target_path,
                "class": if index == 0 { "replaced" } else { "retained" },
                "pre_state": summary("a"),
                "candidate_state": summary(if index == 0 { "b" } else { "a" }),
                "snapshot": if index == 0 {
                    json!({"state":"stored", "snapshot_ref":"fixture-rollback",
                        "purpose":"transaction_rollback", "cleanup_owner":"transaction"})
                } else { json!({"state":"not_required"}) },
                "original_backup_ref": null
            })
        })
        .collect::<Vec<_>>();
    json!({
        "profile_id":"profile", "mod_id":"mod", "old_revision_id":"r1",
        "candidate_revision_id":"r1", "intent":"reapply_equipment_targets",
        "plan_token":"fixture-token", "plan_hash":"fixture-plan", "status":"committing",
        "candidate_replacement_bindings":manifest.replacement_bindings,
        "pre_reinstall_manifest":manifest, "targets":targets
    })
}

fn accepts(value: Value) -> bool {
    serde_json::from_value::<ReinstallRecoveryTransaction>(value)
        .is_ok_and(|transaction| transaction.validate().is_ok())
}

#[test]
fn explicit_reapply_recovers_file_changes_at_one_or_multiple_unchanged_targets() {
    for count in [1, 2] {
        assert!(accepts(transaction(count)), "source count: {count}");
    }
}

#[test]
fn reapply_cannot_change_revision_targets_or_source_provenance() {
    for field in [
        "candidate_revision_id",
        "target",
        "source",
        "missing_source",
        "extra_source",
    ] {
        let mut value = transaction(2);
        match field {
            "candidate_revision_id" => {
                value[field] = json!("r2");
                for binding in value["candidate_replacement_bindings"]
                    .as_array_mut()
                    .unwrap()
                {
                    binding["revision_id"] = json!("r2");
                }
            }
            "target" => {
                value["candidate_replacement_bindings"][0]["target_internal_id"] = json!("c");
                value["candidate_replacement_bindings"][0]["binding"]["target_id"] =
                    json!("target-c");
            }
            "source" => {
                value["candidate_replacement_bindings"][0]["source_internal_id"] = json!("c")
            }
            "missing_source" => {
                value["candidate_replacement_bindings"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
            "extra_source" => value["candidate_replacement_bindings"]
                .as_array_mut()
                .unwrap()
                .push(json!(binding("c"))),
            _ => unreachable!(),
        }
        assert!(!accepts(value), "accepted reapply change: {field}");
    }
}

#[test]
fn reapply_noop_cannot_become_a_transaction_and_unknown_intent_is_rejected() {
    let mut value = transaction(1);
    value["targets"][0]["class"] = json!("retained");
    value["targets"][0]["candidate_state"] = json!(summary("a"));
    value["targets"][0]["snapshot"] = json!({"state":"not_required"});
    assert!(!accepts(value));
    let mut value = transaction(1);
    value["intent"] = json!("force");
    assert!(serde_json::from_value::<ReinstallRecoveryTransaction>(value).is_err());
}

#[test]
fn old_transactions_keep_existing_revision_and_target_switch_semantics() {
    let mut value = transaction(1);
    value.as_object_mut().unwrap().remove("intent");
    assert!(
        !accepts(value.clone()),
        "ordinary same-target switch remains invalid"
    );
    value["candidate_revision_id"] = json!("r2");
    value["candidate_replacement_bindings"][0]["revision_id"] = json!("r2");
    assert!(accepts(value.clone()));
    let decoded: ReinstallRecoveryTransaction = serde_json::from_value(value).unwrap();
    assert!(serde_json::to_value(decoded)
        .unwrap()
        .get("intent")
        .is_none());
    let mut value = transaction(1);
    value.as_object_mut().unwrap().remove("intent");
    value["candidate_replacement_bindings"][0]["target_internal_id"] = json!("b");
    value["candidate_replacement_bindings"][0]["binding"]["target_id"] = json!("target-b");
    assert!(accepts(value));
}

#[test]
fn rollback_can_prune_restored_targets_but_still_preserves_reapply_identity() {
    let mut value = transaction(2);
    value["targets"] = json!([]);
    for status in ["planned", "committing"] {
        value["status"] = json!(status);
        assert!(!accepts(value.clone()));
    }
    for status in ["rollback_required", "rolled_back", "repair_required"] {
        value["status"] = json!(status);
        assert!(accepts(value.clone()));
    }
    value["candidate_replacement_bindings"][0]["target_internal_id"] = json!("c");
    value["candidate_replacement_bindings"][0]["binding"]["target_id"] = json!("target-c");
    assert!(!accepts(value));
}
