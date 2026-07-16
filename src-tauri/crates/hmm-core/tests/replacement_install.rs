use hmm_core::{
    replace_entries_and_bindings_for_mod, FileLayer, GameId, InstallManifest, InstallManifestEntry,
    InstallTargetPath, ModId, ModRevisionId, PackageFileId, ProfileId, ReplacementBinding,
    ReplacementBindingId, ReplacementBindingSnapshot, ReplacementSource, ReplacementSourceId,
    ReplacementTargetId, ReplacementTargetKind, RetargetAction, RetargetPlan,
    INSTALL_MANIFEST_SCHEMA_VERSION_V2,
};

fn retarget_plan(mod_id: &str, profile_id: &str, binding_id: &str) -> RetargetPlan {
    let source_id = ReplacementSourceId::parse("mhw:armor:f_equip:pl121_0000").expect("source id");
    let source = ReplacementSource::new(
        source_id.clone(),
        GameId::mhw(),
        ReplacementTargetKind::parse("armor").expect("target kind"),
        "pl121_0000",
        "pl/f_equip",
        true,
    )
    .expect("source");
    let binding = ReplacementBinding::new(
        ReplacementBindingId::parse(binding_id).expect("binding id"),
        ModId::new(mod_id),
        ProfileId::new(profile_id),
        source_id.clone(),
        ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target id"),
        42,
    )
    .expect("binding");
    let action = RetargetAction::new(
        PackageFileId::new(format!("{mod_id}-body")),
        InstallTargetPath::parse(
            "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
            ["nativePC"],
        )
        .expect("source path"),
        InstallTargetPath::parse(
            "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
            ["nativePC"],
        )
        .expect("target path"),
        source_id,
        "pl121_0000",
        "pl129_0000",
        "pl/f_equip",
        "pl/f_equip",
    )
    .expect("action");

    RetargetPlan::new(binding, source, vec![action], Vec::new()).expect("retarget plan")
}

fn entry(mod_id: &str, revision_id: Option<&str>, package_file_id: &str) -> InstallManifestEntry {
    InstallManifestEntry {
        target_path: InstallTargetPath::parse(
            format!("nativePC/{mod_id}/{package_file_id}.bin"),
            ["nativePC"],
        )
        .expect("target path"),
        mod_id: ModId::new(mod_id),
        revision_id: revision_id.map(ModRevisionId::new),
        package_file_id: PackageFileId::new(package_file_id),
        layer: FileLayer::new("default", 0),
        backup_ref: None,
        installed_file: None,
    }
}

#[test]
fn binding_snapshot_round_trips_and_legacy_manifest_defaults_to_empty() {
    let snapshot = ReplacementBindingSnapshot::from_retarget_plan(
        &retarget_plan("mod-a", "profile-a", "binding-a"),
        None,
    );
    let mut manifest = InstallManifest::completed(
        ProfileId::new("profile-a"),
        vec![entry("mod-a", None, "body")],
    );
    manifest.replacement_bindings = vec![snapshot.clone()];

    let value = serde_json::to_value(&manifest).expect("serialize manifest");
    assert_eq!(
        value["replacement_bindings"][0]["source_path_family"],
        "pl/f_equip"
    );
    assert_eq!(
        value["replacement_bindings"][0]["target_path_family"],
        "pl/f_equip"
    );
    let decoded: InstallManifest = serde_json::from_value(value.clone()).expect("round trip");
    assert_eq!(decoded.replacement_bindings, vec![snapshot]);

    let mut legacy = value;
    legacy
        .as_object_mut()
        .expect("manifest object")
        .remove("replacement_bindings");
    let legacy: InstallManifest = serde_json::from_value(legacy).expect("legacy manifest");
    assert!(legacy.replacement_bindings.is_empty());
}

#[test]
fn manifest_rejects_binding_with_wrong_profile_or_revision() {
    let snapshot = ReplacementBindingSnapshot::from_retarget_plan(
        &retarget_plan("mod-a", "profile-a", "binding-a"),
        Some(ModRevisionId::new("revision-v2")),
    );
    let mut manifest = InstallManifest::completed(
        ProfileId::new("profile-b"),
        vec![entry("mod-a", Some("revision-v3"), "body")],
    );
    manifest.schema_version = INSTALL_MANIFEST_SCHEMA_VERSION_V2;
    manifest.replacement_bindings = vec![snapshot];

    assert!(manifest.validate().is_err());
}

#[test]
fn reinstall_replaces_only_the_requested_mod_binding_snapshot() {
    let old_snapshot = ReplacementBindingSnapshot::from_retarget_plan(
        &retarget_plan("mod-a", "profile-a", "binding-old"),
        None,
    );
    let other_snapshot = ReplacementBindingSnapshot::from_retarget_plan(
        &retarget_plan("mod-b", "profile-a", "binding-b"),
        None,
    );
    let candidate_snapshot = ReplacementBindingSnapshot::from_retarget_plan(
        &retarget_plan("mod-a", "profile-a", "binding-new"),
        Some(ModRevisionId::new("revision-v2")),
    );
    let mut manifest = InstallManifest::completed(
        ProfileId::new("profile-a"),
        vec![
            entry("mod-a", None, "old-body"),
            entry("mod-b", None, "other-body"),
        ],
    );
    manifest.replacement_bindings = vec![old_snapshot, other_snapshot.clone()];

    let updated = replace_entries_and_bindings_for_mod(
        &manifest,
        &ModId::new("mod-a"),
        &[ModRevisionId::new("revision-v1")],
        &ModRevisionId::new("revision-v2"),
        vec![entry("mod-a", Some("revision-v2"), "new-body")],
        vec![candidate_snapshot.clone()],
    )
    .expect("replace Mod entry set and binding");

    assert_eq!(
        updated.replacement_bindings,
        vec![other_snapshot, candidate_snapshot]
    );
    assert!(updated.validate().is_ok());
}
