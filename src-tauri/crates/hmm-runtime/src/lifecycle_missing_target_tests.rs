use super::*;
use hmm_app::{InitialRetargetInstallStatusReader, ModLibraryProfileContext, ModLibraryQuery};
use hmm_core::{
    InstallRecoveryRecord, InstallRecoveryRecordEntry, InstallRecoveryRecordStatus,
    InstallTargetPath, PackageFileId,
};
use hmm_infra::JsonInstallRecoveryRecordRepository;
use hmm_ports::InstallRecoveryRecordRepository;

const TARGET: &str = "nativePC/models/player.mod3";
const ORIGINAL: &[u8] = b"original fixture file";

#[derive(Default)]
struct ProgressLog(std::sync::Mutex<Vec<TaskProgressEvent>>);
impl TaskProgressObserver for ProgressLog {
    type Error = std::convert::Infallible;
    fn observe(&self, event: &TaskProgressEvent) -> Result<(), Self::Error> {
        self.0.lock().unwrap().push(event.clone());
        Ok(())
    }
}

fn installed_fixture(with_backup: bool) -> (tempfile::TempDir, RuntimeEnvironment, PathBuf) {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let game_root = write_install_fixture(sandbox.path());
    if with_backup {
        fs::write(game_root.join(TARGET), ORIGINAL).unwrap();
    }
    fs::write(
        game_root.join("nativePC/foreign-sentinel.bin"),
        b"leave untouched",
    )
    .unwrap();
    let environment = RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).unwrap();
    let token = ReadOnlyInstallAutomation::from_environment(&environment)
        .unwrap()
        .plan_for_profile("mhw", "default", "mod-a")
        .unwrap()
        .plan_token
        .unwrap();
    let progress = ProgressLog::default();
    let outcome =
        CliLifecycleAutomation::prepare_install(&environment, "mhw", "default", "mod-a", &token)
            .unwrap()
            .run_install_with_observer(&progress);
    assert!(
        outcome.is_ok(),
        "install fixture failed: {outcome:?}; events: {:?}",
        progress.0.lock().unwrap()
    );
    assert_eq!(fs::read(game_root.join(TARGET)).unwrap(), b"fixture");
    fs::remove_file(game_root.join(TARGET)).unwrap();
    (sandbox, environment, game_root)
}

fn preview(environment: &RuntimeEnvironment) -> crate::InstallRecoveryPreviewSnapshot {
    ReadOnlyInstallAutomation::from_environment(environment)
        .unwrap()
        .recovery_preview(
            "mhw",
            "default",
            "mod-a",
            ReadOnlyInstallRecoveryAction::UninstallMissingTargets,
        )
        .unwrap()
}

fn prepared(environment: &RuntimeEnvironment, token: &str) -> CliLifecycleAutomation {
    CliLifecycleAutomation::prepare_recovery(
        environment,
        "mhw",
        "default",
        "mod-a",
        ReadOnlyInstallRecoveryAction::UninstallMissingTargets,
        token,
    )
    .unwrap()
}

fn library_status(runtime: &HmmRuntime) -> hmm_app::InstallManifestStatus {
    runtime
        .mod_library_query
        .query(ModLibraryQuery {
            profile_context: Some(ModLibraryProfileContext {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
            }),
            ..ModLibraryQuery::default()
        })
        .unwrap()
        .items[0]
        .install_summary
        .as_ref()
        .unwrap()
        .status
}

#[test]
fn missing_target_recovery_round_trip_unblocks_the_profile_and_refreshes_library_facts() {
    for with_backup in [false, true] {
        let (_sandbox, environment, game_root) = installed_fixture(with_backup);
        let read_only = ReadOnlyInstallAutomation::from_environment(&environment).unwrap();
        let scan = read_only
            .recovery_scan("mhw", "default", &["mod-a".to_owned()])
            .unwrap();
        assert_eq!(scan.items[0].status, "repair_required");
        assert!(
            !read_only
                .uninstall_preview("mhw", "default", "mod-a")
                .unwrap()
                .available
        );
        let plan = preview(&environment);
        assert_eq!(plan.availability, "available");
        assert_eq!(plan.missing_file_count, 1);
        assert_eq!(plan.remove_file_count, 0);
        assert_eq!(plan.restore_file_count, usize::from(with_backup));
        let operation = prepared(&environment, plan.plan_token.as_deref().unwrap());
        assert_eq!(
            operation
                .runtime
                .install_recovery_scanner
                .recovery_status(
                    &GameId::mhw(),
                    &ProfileId::new("default"),
                    &ModId::new("another-mod")
                )
                .unwrap(),
            InstallRecoveryStatus::RepairRequired
        );
        assert_eq!(
            library_status(&operation.runtime),
            hmm_app::InstallManifestStatus::Installed
        );
        let outcome = operation.run_recovery().unwrap();
        assert_eq!(
            outcome.events.last().unwrap().status,
            hmm_app::TaskStatus::Completed
        );
        assert_eq!(
            library_status(&operation.runtime),
            hmm_app::InstallManifestStatus::NotInstalled
        );
        assert_eq!(
            operation
                .runtime
                .install_recovery_scanner
                .recovery_status(
                    &GameId::mhw(),
                    &ProfileId::new("default"),
                    &ModId::new("another-mod")
                )
                .unwrap(),
            InstallRecoveryStatus::NotInstalled
        );
        if with_backup {
            assert_eq!(fs::read(game_root.join(TARGET)).unwrap(), ORIGINAL);
        } else {
            assert!(!game_root.join(TARGET).exists());
        }
        assert_eq!(
            fs::read(game_root.join("nativePC/foreign-sentinel.bin")).unwrap(),
            b"leave untouched"
        );
    }
}

#[test]
fn changed_target_is_rejected_before_recovery_preparation() {
    let (_sandbox, environment, game_root) = installed_fixture(false);
    let token = preview(&environment).plan_token.unwrap();
    fs::write(game_root.join(TARGET), b"foreign replacement").unwrap();
    let rejected = CliLifecycleAutomation::prepare_recovery(
        &environment,
        "mhw",
        "default",
        "mod-a",
        ReadOnlyInstallRecoveryAction::UninstallMissingTargets,
        &token,
    );
    assert!(rejected.is_err());
    assert_eq!(
        fs::read(game_root.join(TARGET)).unwrap(),
        b"foreign replacement"
    );
}

#[test]
fn changing_the_configured_game_root_invalidates_a_missing_target_preview() {
    let (sandbox, environment, original_root) = installed_fixture(true);
    let token = preview(&environment).plan_token.unwrap();
    let different_root = sandbox.path().join("fixtures/games/different-game-root");
    fs::create_dir_all(&different_root).unwrap();
    fs::write(different_root.join("MonsterHunterWorld.exe"), b"fixture").unwrap();
    let config_path = sandbox.path().join("config/games.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    config["games"][0]["root_dir"] = serde_json::json!(different_root);
    fs::write(config_path, serde_json::to_vec(&config).unwrap()).unwrap();

    let rejected = CliLifecycleAutomation::prepare_recovery(
        &environment,
        "mhw",
        "default",
        "mod-a",
        ReadOnlyInstallRecoveryAction::UninstallMissingTargets,
        &token,
    );
    assert!(
        rejected.is_err(),
        "a preview for a different game root must not authorize backup restoration"
    );
    assert!(!different_root.join(TARGET).exists());
    assert!(!original_root.join(TARGET).exists());
}

#[test]
fn drift_after_preparation_is_rejected_inside_the_write_scope() {
    let (_sandbox, environment, game_root) = installed_fixture(false);
    let operation = prepared(
        &environment,
        preview(&environment).plan_token.as_deref().unwrap(),
    );
    fs::write(game_root.join(TARGET), b"foreign replacement").unwrap();
    assert!(operation.run_recovery().is_err());
    assert_eq!(
        fs::read(game_root.join(TARGET)).unwrap(),
        b"foreign replacement"
    );
    assert_eq!(
        library_status(&operation.runtime),
        hmm_app::InstallManifestStatus::Installed
    );
}

#[test]
fn another_mods_pending_install_record_blocks_missing_target_cleanup() {
    let (sandbox, environment, game_root) = installed_fixture(false);
    JsonInstallRecoveryRecordRepository::new(sandbox.path().join("install/recovery"))
        .save_record(&InstallRecoveryRecord {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("other-mod"),
            status: InstallRecoveryRecordStatus::Committing,
            entries: vec![InstallRecoveryRecordEntry {
                target_path: InstallTargetPath::parse("nativePC/other.mod3", ["nativePC"]).unwrap(),
                package_file_id: PackageFileId::new("other"),
                installed_file: Some(hmm_core::installed_file_summary(b"other")),
                backup_ref: None,
            }],
        })
        .unwrap();
    let result = preview(&environment);
    assert_eq!(result.availability, "blocked");
    assert!(result
        .blocking_reasons
        .iter()
        .any(|reason| reason.code == "recovery_pending"));
    assert!(result.plan_token.is_none());
    assert!(!game_root.join(TARGET).exists());
}
