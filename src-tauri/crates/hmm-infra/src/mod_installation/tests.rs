use super::*;
use hmm_core::{GameDirectoryStatus, GameId};
use std::fs;

fn game(root: PathBuf) -> GameInstance {
    GameInstance {
        id: "mhw-default".into(),
        game_id: GameId::parse("mhw").unwrap(),
        display_name: "Fixture".into(),
        root_dir: root,
        status: GameDirectoryStatus::Configured,
        configured_at_unix_millis: 0,
    }
}

fn legacy_manifest(app: &std::path::Path, namespace: &str) -> PathBuf {
    let root = app.join("install/manifests");
    fs::create_dir_all(&root).unwrap();
    let path = root.join(format!("{namespace}.json"));
    // A non-terminal empty manifest is still recovery evidence and must retain its identity.
    fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "profile_id":namespace, "entries":[], "status":"repair_required"
        }))
        .unwrap(),
    )
    .unwrap();
    path
}

#[test]
fn directories_have_distinct_persistent_scopes_even_when_instance_ids_are_equal() {
    let data = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let repository = JsonModInstallationScopeRepository::new(data.path().to_path_buf());
    let first = repository
        .resolve_scope(&game(a.path().to_path_buf()))
        .unwrap();
    let second = repository
        .resolve_scope(&game(b.path().to_path_buf()))
        .unwrap();
    assert_ne!(first.scope_id, second.scope_id);
    assert_ne!(first.installation_id, second.installation_id);
    let restarted = JsonModInstallationScopeRepository::new(data.path().to_path_buf());
    assert_eq!(
        first,
        restarted
            .resolve_scope(&game(a.path().to_path_buf()))
            .unwrap()
    );
    assert_eq!(
        second,
        restarted
            .resolve_scope(&game(b.path().to_path_buf()))
            .unwrap()
    );
    assert_eq!(restarted.list_scope_ids().unwrap().len(), 2);
    assert_eq!(fs::read_dir(a.path()).unwrap().count(), 0);
    assert_eq!(fs::read_dir(b.path()).unwrap().count(), 0);
}

#[test]
fn a_unique_legacy_scope_survives_without_any_save_profile_database() {
    let data = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let manifest = legacy_manifest(data.path(), "former-account-b");
    let original = fs::read(&manifest).unwrap();
    let repository = JsonModInstallationScopeRepository::new(data.path().to_path_buf());
    let context = repository
        .resolve_scope(&game(root.path().to_path_buf()))
        .unwrap();
    assert_eq!(context.scope_id.as_str(), "former-account-b");
    assert_eq!(fs::read(manifest).unwrap(), original);
    assert!(!data.path().join("hmm.db").exists());
    assert_eq!(repository.list_scope_ids().unwrap(), vec![context.scope_id]);
}

#[test]
fn multiple_legacy_namespaces_are_preserved_and_never_silently_selected_or_merged() {
    let data = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let a = legacy_manifest(data.path(), "account-a");
    let b = legacy_manifest(data.path(), "account-b");
    let originals = (fs::read(&a).unwrap(), fs::read(&b).unwrap());
    let repository = JsonModInstallationScopeRepository::new(data.path().to_path_buf());
    assert_eq!(
        repository.resolve_scope(&game(root.path().to_path_buf())),
        Err(ModInstallationScopeError::LegacyAmbiguous)
    );
    assert!(!data.path().join("install").join(REGISTRY).exists());
    assert_eq!((fs::read(a).unwrap(), fs::read(b).unwrap()), originals);
    assert_eq!(repository.list_scope_ids().unwrap().len(), 2);
}

#[test]
fn orphaned_recovery_and_pending_choices_remain_in_the_installation_index() {
    let data = tempfile::tempdir().unwrap();
    for (directory, namespace) in [
        ("recovery", "orphan-a"),
        ("reinstall-recovery", "orphan-b"),
        ("replacement-selections", "orphan-c"),
        ("plugin-selections", "orphan-d"),
    ] {
        let root = data.path().join("install").join(directory);
        fs::create_dir_all(&root).unwrap();
        let value = if directory == "plugin-selections" {
            serde_json::json!({"scope":{"profile_id":namespace}})
        } else {
            serde_json::json!({"profile_id":namespace})
        };
        fs::write(root.join("fixture.json"), value.to_string()).unwrap();
    }
    let repository = JsonModInstallationScopeRepository::new(data.path().to_path_buf());
    assert_eq!(
        repository
            .list_scope_ids()
            .unwrap()
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        vec!["orphan-a", "orphan-b", "orphan-c", "orphan-d"]
    );
}

#[test]
fn corrupt_or_mismatched_installation_records_fail_closed() {
    let data = tempfile::tempdir().unwrap();
    let game_root = tempfile::tempdir().unwrap();
    let manifest = legacy_manifest(data.path(), "first");
    fs::write(&manifest, br#"{"profile_id":"second","entries":[]}"#).unwrap();
    let repository = JsonModInstallationScopeRepository::new(data.path().to_path_buf());
    assert_eq!(
        repository.resolve_scope(&game(game_root.path().to_path_buf())),
        Err(ModInstallationScopeError::Unavailable)
    );
    assert!(repository.list_scope_ids().is_err());
    assert_eq!(
        fs::read(&manifest).unwrap(),
        br#"{"profile_id":"second","entries":[]}"#
    );
}

#[test]
fn an_empty_completed_legacy_manifest_does_not_override_existing_installation_evidence() {
    let data = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let empty = legacy_manifest(data.path(), "default");
    fs::write(empty, br#"{"profile_id":"default","entries":[]}"#).unwrap();
    legacy_manifest(data.path(), "old-install");
    let repository = JsonModInstallationScopeRepository::new(data.path().to_path_buf());
    assert_eq!(
        repository
            .resolve_scope(&game(root.path().to_path_buf()))
            .unwrap()
            .scope_id
            .as_str(),
        "old-install"
    );
}

#[cfg(windows)]
#[test]
fn directory_spelling_and_case_do_not_create_another_installation() {
    let data = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let repository = JsonModInstallationScopeRepository::new(data.path().to_path_buf());
    let first = repository
        .resolve_scope(&game(root.path().to_path_buf()))
        .unwrap();
    let alternate = PathBuf::from(
        root.path()
            .to_string_lossy()
            .to_uppercase()
            .replace('\\', "/"),
    );
    assert_eq!(first, repository.resolve_scope(&game(alternate)).unwrap());
}

#[test]
fn inspection_does_not_create_installation_metadata_or_locks() {
    let data = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let repository = JsonModInstallationScopeRepository::new(data.path().to_path_buf());
    let inspected = repository
        .inspect_scope(&game(root.path().to_path_buf()))
        .unwrap();
    assert_eq!(fs::read_dir(data.path()).unwrap().count(), 0);
    assert_eq!(
        repository
            .resolve_scope(&game(root.path().to_path_buf()))
            .unwrap(),
        inspected
    );
}

#[test]
fn missing_primary_registry_does_not_rebind_a_namespace_to_another_directory() {
    let data = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let repository = JsonModInstallationScopeRepository::new(data.path().to_path_buf());
    let first = repository
        .resolve_scope(&game(a.path().to_path_buf()))
        .unwrap();
    fs::remove_file(data.path().join("install").join(REGISTRY)).unwrap();
    let restarted = JsonModInstallationScopeRepository::new(data.path().to_path_buf());
    let second = restarted
        .resolve_scope(&game(b.path().to_path_buf()))
        .unwrap();
    assert_ne!(first.scope_id, second.scope_id);
    assert_eq!(
        restarted
            .resolve_scope(&game(a.path().to_path_buf()))
            .unwrap(),
        first
    );
}

#[test]
fn conflicting_registry_snapshots_fail_without_rewriting_either_snapshot() {
    let data = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let repository = JsonModInstallationScopeRepository::new(data.path().to_path_buf());
    repository
        .resolve_scope(&game(root.path().to_path_buf()))
        .unwrap();
    let next = data.path().join("install").join(REGISTRY_NEXT);
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&next).unwrap()).unwrap();
    value["installations"][0]["scope_id"] = "unrelated-namespace".into();
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(&next, &bytes).unwrap();
    assert_eq!(
        repository.resolve_scope(&game(root.path().to_path_buf())),
        Err(ModInstallationScopeError::Unavailable)
    );
    assert_eq!(fs::read(next).unwrap(), bytes);
}

#[test]
fn an_unregistered_generated_namespace_cannot_be_claimed_by_another_directory() {
    let data = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let key = installation_key(&game(a.path().to_path_buf())).unwrap();
    legacy_manifest(data.path(), &key);
    let repository = JsonModInstallationScopeRepository::new(data.path().to_path_buf());
    assert_eq!(
        repository.resolve_scope(&game(b.path().to_path_buf())),
        Err(ModInstallationScopeError::LegacyAmbiguous)
    );
    assert_eq!(
        repository
            .resolve_scope(&game(a.path().to_path_buf()))
            .unwrap()
            .scope_id
            .as_str(),
        key
    );
}

#[cfg(windows)]
#[test]
fn a_junction_installation_root_is_rejected_without_writing_outside_app_data() {
    let data = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let junction = data.path().join("install");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(outside.path())
        .output()
        .unwrap()
        .status;
    assert!(status.success());
    let repository = JsonModInstallationScopeRepository::new(data.path().to_path_buf());
    assert_eq!(
        repository.resolve_scope(&game(root.path().to_path_buf())),
        Err(ModInstallationScopeError::Unavailable)
    );
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}
