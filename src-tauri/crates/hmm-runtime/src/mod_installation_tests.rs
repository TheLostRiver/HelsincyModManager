use crate::{CliLifecycleAutomation, HmmRuntime, ReadOnlyInstallAutomation, RuntimeEnvironment};
use hmm_core::{GameId, ModId, ProfileId};
use hmm_ports::GameConfigRepository;
use sha2::{Digest, Sha256};
use std::fs;

#[derive(Default)]
struct Events(std::sync::Mutex<Vec<(String, Option<String>)>>);

impl hmm_app::TaskProgressObserver for Events {
    type Error = std::convert::Infallible;
    fn observe(&self, event: &hmm_app::TaskProgressEvent) -> Result<(), Self::Error> {
        self.0
            .lock()
            .unwrap()
            .push((event.phase.clone(), event.error.clone()));
        Ok(())
    }
}

#[test]
fn mod_installation_rejects_a_prepared_write_after_the_game_directory_changes() {
    use hmm_app::{InstallWriteAdmission, InstallWriteAdmissionError};

    let data = tempfile::tempdir().unwrap();
    let first_root = crate::lifecycle_automation::write_install_fixture(data.path());
    let second_root = data.path().join("fixtures/games/mhw-second");
    crate::lifecycle_automation::write_production_style_fixture(data.path(), &second_root);
    let first_target = first_root.join("nativePC/models/player.mod3");
    let second_target = second_root.join("nativePC/models/player.mod3");
    fs::write(&first_target, b"first-original").unwrap();
    fs::write(&second_target, b"second-original").unwrap();
    let games = hmm_infra::JsonGameConfigRepository::new(data.path().join("config/games.json"));
    let mut game = games.load_game_instance(&GameId::mhw()).unwrap().unwrap();
    game.root_dir = first_root;
    games.save_game_instance(&game).unwrap();

    let runtime = HmmRuntime::builder(data.path().to_path_buf())
        .build()
        .unwrap();
    let first = runtime
        .mod_installation_scope
        .context(&GameId::mhw())
        .unwrap();
    let environment = RuntimeEnvironment::sandbox(data.path().to_path_buf()).unwrap();
    let preview = ReadOnlyInstallAutomation::from_environment(&environment)
        .unwrap()
        .plan("mhw", "mod-a")
        .unwrap();
    let prepared = CliLifecycleAutomation::prepare_install(
        &environment,
        "mhw",
        "auto",
        "mod-a",
        &preview.plan_token.unwrap(),
    )
    .unwrap();

    game.root_dir = second_root;
    games.save_game_instance(&game).unwrap();
    assert_eq!(
        runtime
            .mod_installation_scope
            .ensure_write_allowed(&GameId::mhw(), &first.scope_id),
        Err(InstallWriteAdmissionError::SafetyRejected),
    );
    assert!(prepared.run_install().is_err());
    assert_eq!(fs::read(&first_target).unwrap(), b"first-original");
    assert_eq!(fs::read(&second_target).unwrap(), b"second-original");
    assert!(!data.path().join("install/manifests/default.json").exists());
    let second = runtime
        .mod_installation_scope
        .context(&GameId::mhw())
        .unwrap();
    assert_ne!(first.scope_id, second.scope_id);
    runtime
        .mod_installation_scope
        .ensure_write_allowed(&GameId::mhw(), &second.scope_id)
        .unwrap();
}

#[test]
fn mod_installation_survives_save_profile_switch_delete_restart_and_uninstall() {
    let data = tempfile::tempdir().unwrap();
    let game_root = crate::lifecycle_automation::write_install_fixture(data.path());
    let environment = RuntimeEnvironment::sandbox(data.path().to_path_buf()).unwrap();
    let original = b"original-target";
    let target = game_root.join("nativePC/models/player.mod3");
    fs::write(&target, original).unwrap();
    // Existing installation data belongs to a namespace whose save profile no longer exists.
    fs::write(game_root.join("nativePC/models/legacy.mod3"), b"legacy").unwrap();
    let manifests = data.path().join("install/manifests");
    fs::create_dir_all(&manifests).unwrap();
    let manifest = manifests.join("former-account.json");
    fs::write(&manifest, serde_json::json!({
        "profile_id": "former-account", "entries": [{
            "target_path": "nativePC/models/legacy.mod3", "mod_id": "legacy-mod",
            "package_file_id": "legacy-file", "layer": {"name":"base", "priority":0},
            "backup_ref": null,
            "installed_file": {"size_bytes":6, "sha256":format!("{:x}", Sha256::digest(b"legacy"))}
        }]
    }).to_string()).unwrap();

    let preview = ReadOnlyInstallAutomation::from_environment(&environment)
        .unwrap()
        .plan("mhw", "mod-a")
        .unwrap();
    assert_eq!(preview.profile_id, "former-account");
    assert!(
        !data
            .path()
            .join("install/installation-scopes.json")
            .exists(),
        "a read-only preview must not register the scope"
    );
    let install = CliLifecycleAutomation::prepare_install(
        &environment,
        "mhw",
        "auto",
        "mod-a",
        &preview.plan_token.unwrap(),
    )
    .unwrap();
    let events = Events::default();
    let outcome = install.run_install_with_observer(&events);
    assert!(
        outcome.is_ok(),
        "{outcome:?}; events: {:?}",
        events.0.lock().unwrap()
    );
    drop(install);
    assert_eq!(fs::read(&target).unwrap(), b"fixture");
    let installed_manifest = fs::read(&manifest).unwrap();

    let runtime = HmmRuntime::builder(data.path().to_path_buf())
        .build()
        .unwrap();
    let context = runtime
        .mod_installation_scope
        .context(&GameId::mhw())
        .unwrap();
    assert_eq!(context.scope_id.as_str(), "former-account");
    let account = runtime
        .profiles
        .create_profile(hmm_app::CreateProfileRequest {
            name: "Save account B".into(),
            description: None,
        })
        .unwrap();
    runtime.profiles.set_active_profile(&account).unwrap();
    assert_eq!(runtime.profiles.get_active_profile().unwrap().id, account);
    assert_eq!(
        runtime
            .mod_installation_scope
            .context(&GameId::mhw())
            .unwrap(),
        context
    );
    assert_eq!(
        runtime
            .mod_installation_scope
            .require_current(&GameId::mhw(), &ProfileId::new(&account)),
        Err(hmm_ports::ModInstallationScopeError::Mismatch)
    );
    runtime.profiles.set_active_profile("default").unwrap();
    runtime.profiles.delete_profile(&account).unwrap();
    assert_eq!(fs::read(&manifest).unwrap(), installed_manifest);
    assert!(matches!(
        runtime.mod_deletion.delete_mod(&ModId::new("mod-a")),
        Err(hmm_app::ModDeletionError::BlockedInstalled { .. })
    ));
    drop(runtime);

    let preview = ReadOnlyInstallAutomation::from_environment(&environment)
        .unwrap()
        .uninstall_preview("mhw", "auto", "mod-a")
        .unwrap();
    assert!(preview.available);
    assert_eq!(preview.profile_id, "former-account");
    let uninstall = CliLifecycleAutomation::prepare_uninstall(
        &environment,
        "mhw",
        "auto",
        "mod-a",
        &preview.plan_token.unwrap(),
    )
    .unwrap();
    uninstall.run_uninstall().unwrap();
    assert_eq!(fs::read(target).unwrap(), original);
    assert_eq!(
        fs::read(game_root.join("nativePC/models/legacy.mod3")).unwrap(),
        b"legacy"
    );
}
