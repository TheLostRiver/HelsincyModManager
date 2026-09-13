use hmm_core::{
    GameId, InstallTargetPath, InstalledFileSummary, ModId, ModRevisionId, PackageFileId,
    PluginFileChoice, PluginFileChoiceKind, PluginSelectionScope, PluginSelectionSnapshot,
    ProfileId,
};

fn scope() -> PluginSelectionScope {
    PluginSelectionScope {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("profile"),
        mod_id: ModId::new("mod"),
        revision_id: ModRevisionId::new("revision"),
    }
}

fn file() -> PluginFileChoice {
    PluginFileChoice {
        package_file_id: PackageFileId::new("package-plugin"),
        target_path: InstallTargetPath::parse("content/plugin.bin", ["content"]).unwrap(),
        source_file: InstalledFileSummary {
            size_bytes: 512,
            sha256: "a".repeat(64),
        },
        choice: PluginFileChoiceKind::Include,
        excluded_by_package_selection: false,
    }
}

fn snapshot(scope: PluginSelectionScope, files: Vec<PluginFileChoice>) -> PluginSelectionSnapshot {
    PluginSelectionSnapshot::new(scope, "fixture.plugin", 1, files).unwrap()
}

#[test]
fn choices_round_trip_but_inventory_identity_binds_scope_source_and_policy() {
    let original = snapshot(scope(), vec![file()]);
    let json = serde_json::to_value(&original).unwrap();
    assert_eq!(
        serde_json::from_value::<PluginSelectionSnapshot>(json).unwrap(),
        original
    );
    let mut excluded = file();
    excluded.choice = PluginFileChoiceKind::Exclude;
    assert_eq!(
        snapshot(scope(), vec![excluded]).inventory_id(),
        original.inventory_id()
    );
    for kind in [
        "profile",
        "mod",
        "revision",
        "bytes",
        "size",
        "path",
        "package-file",
        "policy",
    ] {
        let mut changed_scope = scope();
        let mut changed_file = file();
        let mut policy_version = 1;
        match kind {
            "profile" => changed_scope.profile_id = ProfileId::new("another"),
            "mod" => changed_scope.mod_id = ModId::new("another"),
            "revision" => changed_scope.revision_id = ModRevisionId::new("another"),
            "bytes" => changed_file.source_file.sha256 = "b".repeat(64),
            "size" => changed_file.source_file.size_bytes += 1,
            "path" => {
                changed_file.target_path =
                    InstallTargetPath::parse("content/other.bin", ["content"]).unwrap()
            }
            "package-file" => changed_file.package_file_id = PackageFileId::new("other"),
            "policy" => policy_version = 2,
            _ => unreachable!(),
        }
        let changed = PluginSelectionSnapshot::new(
            changed_scope,
            "fixture.plugin",
            policy_version,
            vec![changed_file],
        )
        .unwrap();
        assert_ne!(
            changed.inventory_id(),
            original.inventory_id(),
            "unbound {kind}"
        );
    }
}

#[test]
fn serialized_selection_rejects_unsafe_facts_duplicate_files_and_unknown_schema() {
    let value = serde_json::to_value(snapshot(scope(), vec![file()])).unwrap();
    for kind in [
        "schema",
        "digest",
        "absolute",
        "traversal",
        "duplicates",
        "package-exclusion",
        "extra-field",
    ] {
        let mut changed = value.clone();
        match kind {
            "schema" => changed["schema_version"] = 2.into(),
            "digest" => changed["files"][0]["source_file"]["sha256"] = "not-a-digest".into(),
            "absolute" => changed["files"][0]["target_path"] = "/outside/plugin.bin".into(),
            "traversal" => changed["files"][0]["target_path"] = "content/../outside.bin".into(),
            "duplicates" => {
                let duplicate = changed["files"][0].clone();
                changed["files"].as_array_mut().unwrap().push(duplicate);
            }
            "package-exclusion" => {
                changed["files"][0]["excluded_by_package_selection"] = true.into()
            }
            "extra-field" => changed["caller_approved"] = true.into(),
            _ => unreachable!(),
        }
        assert!(
            serde_json::from_value::<PluginSelectionSnapshot>(changed).is_err(),
            "accepted {kind}"
        );
    }
    let mut same_target = file();
    same_target.package_file_id = PackageFileId::new("other");
    same_target.target_path = InstallTargetPath::parse("content/PLUGIN.bin.", ["content"]).unwrap();
    assert!(
        PluginSelectionSnapshot::new(scope(), "fixture.plugin", 1, vec![file(), same_target])
            .is_err()
    );
}

#[test]
fn plan_and_manifest_choices_match_actual_file_ownership_and_survive_legacy_reads() {
    use hmm_core::{
        FileLayer, InstallFileProvider, InstallManifest, InstallManifestEntry, InstallPlan,
    };
    let selected = snapshot(scope(), vec![file()]);
    let mut plan = InstallPlan::from_providers([InstallFileProvider::new(
        scope().mod_id,
        file().package_file_id,
        file().target_path,
        FileLayer::new("base", 0),
    )]);
    plan.plugin_selections = vec![selected.clone()];
    plan.validate_plugin_selections(&GameId::mhw(), &scope().profile_id)
        .unwrap();
    assert!(plan
        .validate_plugin_selections(&GameId::mhw(), &ProfileId::new("other"))
        .is_err());
    let mut wrong_target = plan.clone();
    wrong_target.actions[0].target_path =
        InstallTargetPath::parse("content/other.bin", ["content"]).unwrap();
    assert!(wrong_target
        .validate_plugin_selections(&GameId::mhw(), &scope().profile_id)
        .is_err());
    let entry = InstallManifestEntry {
        target_path: file().target_path,
        mod_id: scope().mod_id,
        revision_id: Some(scope().revision_id),
        package_file_id: file().package_file_id,
        layer: FileLayer::new("base", 0),
        backup_ref: None,
        installed_file: Some(file().source_file),
        adopted: false,
    };
    let mut manifest = InstallManifest::completed(scope().profile_id, vec![entry]);
    manifest.schema_version = 2;
    manifest.plugin_selections = vec![selected];
    manifest.validate().unwrap();
    let json = serde_json::to_value(&manifest).unwrap();
    assert_eq!(
        serde_json::from_value::<InstallManifest>(json.clone()).unwrap(),
        manifest
    );
    for key in ["entries", "profile", "source", "revision"] {
        let mut changed = json.clone();
        match key {
            "entries" => changed["entries"] = serde_json::json!([]),
            "profile" => changed["plugin_selections"][0]["scope"]["profile_id"] = "other".into(),
            "source" => changed["entries"][0]["installed_file"]["sha256"] = "b".repeat(64).into(),
            "revision" => changed["plugin_selections"][0]["scope"]["revision_id"] = "other".into(),
            _ => unreachable!(),
        }
        assert!(
            serde_json::from_value::<InstallManifest>(changed).is_err(),
            "accepted inconsistent {key}"
        );
    }
    let mut legacy = json;
    legacy.as_object_mut().unwrap().remove("plugin_selections");
    assert!(serde_json::from_value::<InstallManifest>(legacy)
        .unwrap()
        .plugin_selections
        .is_empty());
}

#[test]
fn removing_all_files_requires_complete_same_revision_plugin_ownership() {
    use hmm_core::{
        is_complete_plugin_removal, replace_entries_bindings_and_plugins_for_mod, FileLayer,
        InstallManifest, InstallManifestEntry,
    };
    let entry = InstallManifestEntry {
        target_path: file().target_path,
        mod_id: scope().mod_id,
        revision_id: Some(scope().revision_id),
        package_file_id: file().package_file_id,
        layer: FileLayer::new("base", 0),
        backup_ref: None,
        installed_file: Some(file().source_file),
        adopted: false,
    };
    let mut excluded = file();
    excluded.choice = PluginFileChoiceKind::Exclude;
    let selection = snapshot(scope(), vec![excluded]);
    let manifest = InstallManifest::completed(scope().profile_id, vec![entry]);
    let remove = |manifest: &InstallManifest, selection: PluginSelectionSnapshot| {
        replace_entries_bindings_and_plugins_for_mod(
            manifest,
            &scope().mod_id,
            &[],
            &scope().revision_id,
            vec![],
            vec![],
            vec![selection],
        )
    };
    assert!(is_complete_plugin_removal(
        &manifest,
        &scope().mod_id,
        &scope().revision_id,
        &[],
        std::slice::from_ref(&selection)
    ));
    let removed = remove(&manifest, selection.clone()).unwrap();
    assert!(removed.entries.is_empty());
    assert!(removed.plugin_selections.is_empty());
    for problem in [
        "non-plugin",
        "adopted",
        "digest",
        "missing-summary",
        "path",
        "revision",
        "include",
        "scope",
        "empty-current",
    ] {
        let mut current = manifest.clone();
        let mut selected = selection.clone();
        match problem {
            "non-plugin" => {
                let mut unrelated = current.entries[0].clone();
                unrelated.package_file_id = PackageFileId::new("unrelated");
                unrelated.target_path =
                    InstallTargetPath::parse("content/unrelated.bin", ["content"]).unwrap();
                current.entries.push(unrelated);
            }
            "adopted" => current.entries[0].adopted = true,
            "digest" => current.entries[0].installed_file.as_mut().unwrap().sha256 = "b".repeat(64),
            "missing-summary" => current.entries[0].installed_file = None,
            "path" => {
                current.entries[0].target_path =
                    InstallTargetPath::parse("content/another.bin", ["content"]).unwrap()
            }
            "revision" => {
                current.entries[0].revision_id = Some(ModRevisionId::new("another-revision"))
            }
            "include" => selected = snapshot(scope(), vec![file()]),
            "scope" => {
                let mut other = scope();
                other.profile_id = ProfileId::new("other");
                selected = snapshot(other, selected.files().to_vec());
            }
            "empty-current" => current.entries.clear(),
            _ => unreachable!(),
        }
        assert!(
            !is_complete_plugin_removal(
                &current,
                &scope().mod_id,
                &scope().revision_id,
                &[],
                std::slice::from_ref(&selected)
            ),
            "accepted {problem}"
        );
        assert!(
            remove(&current, selected).is_err(),
            "removed unverified {problem}"
        );
    }
}
