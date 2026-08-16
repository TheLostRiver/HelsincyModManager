use super::*;
use hmm_core::{
    ContentTransformerIdentity, ModRevisionId, ReplacementAdapterFacts, ReplacementBinding,
    ReplacementBindingId, ReplacementBindingSnapshot, ReplacementSourceId, ReplacementTargetId,
    ReplacementTargetKind,
};

fn transformer_facts(transformer_version: u32, file_count: u32) -> ReplacementAdapterFacts {
    ReplacementAdapterFacts::new(
        1,
        "mhw.weapon",
        "mrl3-texture-path",
        1,
        "a".repeat(64),
        "b".repeat(64),
        "c".repeat(64),
    )
    .expect("adapter facts")
    .with_transformers(
        vec![ContentTransformerIdentity::new(
            "mhw.weapon.mrl3-texture-path.v1",
            transformer_version,
        )
        .expect("transformer identity")],
        1,
        file_count,
    )
    .expect("transformer facts")
}

pub(super) fn replacement_snapshot(
    mod_id: &str,
    profile_id: &str,
    binding_id: &str,
    target_id: &str,
    revision_id: Option<&str>,
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
        "pl129_0000",
        "pl/f_equip",
        "pl/f_equip",
        ReplacementTargetKind::parse("armor").expect("kind"),
    )
    .expect("snapshot")
}

#[test]
fn commit_persists_replacement_snapshot_and_includes_it_in_plan_hash() {
    let target =
        InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"]).expect("target");
    let provider = InstallFileProvider::new(
        ModId::new("mod-a"),
        PackageFileId::new("nativePC/models/player.mod3"),
        target,
        FileLayer::new("base", 0),
    );
    let first = InstallPlan::from_providers([provider.clone()])
        .with_replacement_bindings(vec![replacement_snapshot(
            "mod-a",
            "default",
            "binding-a",
            "mhw:armor:fatalis-alpha",
            None,
        )])
        .expect("first plan");
    let second = InstallPlan::from_providers([provider])
        .with_replacement_bindings(vec![replacement_snapshot(
            "mod-a",
            "default",
            "binding-b",
            "mhw:armor:alatreon-alpha",
            None,
        )])
        .expect("second plan");
    let with_adapter_facts = InstallPlan::from_providers([InstallFileProvider::new(
        ModId::new("mod-a"),
        PackageFileId::new("nativePC/models/player.mod3"),
        InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"]).expect("target"),
        FileLayer::new("base", 0),
    )])
    .with_replacement_bindings(vec![replacement_snapshot(
        "mod-a",
        "default",
        "binding-a",
        "mhw:armor:fatalis-alpha",
        None,
    )
    .with_adapter_facts(transformer_facts(1, 1))])
    .expect("adapter facts plan");
    let with_changed_transformer = InstallPlan::from_providers([InstallFileProvider::new(
        ModId::new("mod-a"),
        PackageFileId::new("nativePC/models/player.mod3"),
        InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"]).expect("target"),
        FileLayer::new("base", 0),
    )])
    .with_replacement_bindings(vec![replacement_snapshot(
        "mod-a",
        "default",
        "binding-a",
        "mhw:armor:fatalis-alpha",
        None,
    )
    .with_adapter_facts(transformer_facts(2, 1))])
    .expect("changed transformer plan");

    let first_manifest = commit_plan_for_hash_test(first);
    let second_manifest = commit_plan_for_hash_test(second);
    let adapter_facts_manifest = commit_plan_for_hash_test(with_adapter_facts);
    let changed_transformer_manifest = commit_plan_for_hash_test(with_changed_transformer);

    assert_eq!(first_manifest.replacement_bindings.len(), 1);
    assert_eq!(
        first_manifest.replacement_bindings[0]
            .binding()
            .target_id()
            .as_str(),
        "mhw:armor:fatalis-alpha"
    );
    assert_ne!(first_manifest.plan_hash, second_manifest.plan_hash);
    assert_ne!(first_manifest.plan_hash, adapter_facts_manifest.plan_hash);
    assert_ne!(
        adapter_facts_manifest.plan_hash,
        changed_transformer_manifest.plan_hash
    );
    assert!(adapter_facts_manifest.replacement_bindings[0]
        .adapter_facts()
        .is_some());
}

#[test]
fn commit_rejects_binding_profile_mismatch_before_any_io() {
    let plan = InstallPlan::from_providers([InstallFileProvider::new(
        ModId::new("mod-a"),
        PackageFileId::new("nativePC/models/player.mod3"),
        InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"]).expect("target"),
        FileLayer::new("base", 0),
    )])
    .with_replacement_bindings(vec![replacement_snapshot(
        "mod-a",
        "other-profile",
        "binding-a",
        "mhw:armor:fatalis-alpha",
        None,
    )])
    .expect("owned binding");
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
        "nativePC/models/player.mod3",
        b"new player".as_slice(),
    )]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::default());
    let manifests = Arc::new(RecordingInstallManifestRepository::default());
    let service = InstallCommitService::new(
        source_files.clone(),
        game_files.clone(),
        Arc::new(RecordingInstallBackupStore::default()),
        manifests.clone(),
    );

    let error = service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect_err("profile mismatch");

    assert_eq!(error, InstallCommitError::PlanHasInvalidReplacementBindings);
    assert!(source_files.read_requests().is_empty());
    assert!(game_files.write_requests().is_empty());
    assert_eq!(manifests.save_attempts(), 0);
}

#[test]
fn commit_rejects_binding_revision_mismatch_before_any_io() {
    let snapshot = replacement_snapshot(
        "mod-a",
        "default",
        "binding-a",
        "mhw:armor:fatalis-alpha",
        Some("revision-v2"),
    );
    let plan = InstallPlan::from_providers([InstallFileProvider::new(
        ModId::new("mod-a"),
        PackageFileId::new("nativePC/models/player.mod3"),
        InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"]).expect("target"),
        FileLayer::new("base", 0),
    )])
    .with_replacement_bindings(vec![snapshot])
    .expect("owned binding");
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
        "nativePC/models/player.mod3",
        b"new player".as_slice(),
    )]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::default());
    let manifests = Arc::new(RecordingInstallManifestRepository::default());
    let service = InstallCommitService::new(
        source_files.clone(),
        game_files.clone(),
        Arc::new(RecordingInstallBackupStore::default()),
        manifests.clone(),
    );

    let error = service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect_err("revision mismatch");

    assert_eq!(error, InstallCommitError::PlanHasInvalidReplacementBindings);
    assert!(source_files.read_requests().is_empty());
    assert!(game_files.write_requests().is_empty());
    assert_eq!(manifests.save_attempts(), 0);
}
