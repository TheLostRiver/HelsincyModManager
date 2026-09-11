use super::*;
use hmm_app::{
    EquipmentRetargetReinstallTaskExecutor, ReinstallTaskExecutor, ReinstallTaskPrepared,
};
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn single_source_legacy_install_can_switch_without_weakening_the_single_source_gate() {
    for single in [true, false] {
        let fixture = LegacyFixture::with_files(
            true,
            if single {
                &EQUIPMENT_FILES[..3]
            } else {
                EQUIPMENT_FILES
            },
        );
        let request = RetargetReinstallRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: fixture.mod_id.clone(),
            target_id: target("one002", "wp/one"),
            layer: FileLayer::new("base", 0),
        };
        let preview = fixture
            .state
            .reinstall_executor
            .preview_retarget_reinstall(request.clone())
            .unwrap();
        if !single {
            assert_eq!(preview.status, ReinstallPreviewStatus::Blocked);
            assert!(read_fixture_manifest(&fixture.app_data)
                .replacement_bindings
                .is_empty());
            continue;
        }
        assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
        let request = StartRetargetReinstallTaskRequest {
            game_id: request.game_id,
            profile_id: request.profile_id,
            mod_id: request.mod_id,
            target_id: request.target_id,
            layer: request.layer,
            plan_token: preview.plan_token.unwrap(),
        };
        let task = fixture
            .state
            .reinstall_tasks
            .start_retarget_reinstall_task(request.clone())
            .unwrap();
        fixture
            .state
            .reinstall_task_runner
            .run_retarget_reinstall_task(&task.task_id, request)
            .unwrap();
        let installed = read_fixture_manifest(&fixture.app_data);
        assert_eq!(installed.replacement_bindings.len(), 1);
        assert_eq!(
            installed.replacement_bindings[0].target_internal_id(),
            "one002"
        );
    }
}

#[test]
fn commit_rechecks_the_original_package_after_candidate_staging() {
    let fixture = LegacyFixture::new(true);
    let before = snapshot_file_tree(&fixture.game);
    let prepared = fixture
        .state
        .reinstall_executor
        .prepare_equipment_retarget_reinstall(selection(
            &fixture.state,
            &fixture.mod_id,
            "one002",
            Some("pl129_0000"),
        ))
        .unwrap();
    let token = prepared.plan_token().to_owned();
    fs::write(
        fixture
            .package
            .join(fixture.manifest.entries[0].package_file_id.as_str()),
        b"changed after staging",
    )
    .unwrap();
    let error = fixture
        .state
        .reinstall_executor
        .commit(prepared, &token)
        .unwrap_err();
    assert_eq!(error, hmm_app::ReinstallCommitError::PreviewStale);
    assert_eq!(snapshot_file_tree(&fixture.game), before);
    assert_eq!(read_fixture_manifest(&fixture.app_data), fixture.manifest);
    assert_no_reinstall_recovery_transactions(&fixture.app_data);
}

struct CancelAfterOriginPreparation {
    task_manager: Arc<hmm_app::TaskManager>,
    phase: &'static str,
    cancelled: AtomicBool,
}

impl hmm_app::TaskProgressObserver for CancelAfterOriginPreparation {
    type Error = std::convert::Infallible;

    fn observe(&self, event: &hmm_app::TaskProgressEvent) -> Result<(), Self::Error> {
        if event.phase == self.phase {
            self.task_manager.cancel_task(&event.task_id).unwrap();
            self.cancelled.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

#[test]
fn cancelled_origin_recovery_keeps_game_files_and_the_exact_unbound_manifest() {
    for phase in [
        "install.reinstall.preflight.processing",
        "install.reinstall.commit.processing",
    ] {
        let fixture = LegacyFixture::new(false);
        let before = snapshot_file_tree(&fixture.game);
        let manifest_path = fixture.app_data.join("install/manifests/default.json");
        let manifest_before = fs::read(&manifest_path).unwrap();
        let request = switch_request(&fixture);
        let task = fixture
            .state
            .reinstall_tasks
            .start_equipment_retarget_reinstall_task(request.clone())
            .unwrap();
        let observer = CancelAfterOriginPreparation {
            task_manager: Arc::clone(&fixture.state.task_manager),
            phase,
            cancelled: AtomicBool::new(false),
        };
        let events = fixture
            .state
            .reinstall_task_runner
            .run_equipment_retarget_reinstall_task_with_observer(&task.task_id, request, &observer)
            .unwrap();
        assert!(observer.cancelled.load(Ordering::SeqCst));
        assert!(events.iter().any(|event| event.phase == phase));
        assert_eq!(
            fixture.state.task_manager.task_status(&task.task_id),
            Some(TaskStatus::Cancelled)
        );
        assert_eq!(snapshot_file_tree(&fixture.game), before);
        assert_eq!(fs::read(manifest_path).unwrap(), manifest_before);
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
        assert_no_retarget_staging(&fixture.app_data);
    }
}

struct CrashBeforeManifestSave {
    inner: JsonInstallManifestRepository,
    crash: AtomicBool,
}

impl InstallManifestRepository for CrashBeforeManifestSave {
    fn load_manifest(&self, profile: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        self.inner.load_manifest(profile)
    }
    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        assert!(
            !self.crash.swap(false, Ordering::SeqCst),
            "simulated stop before final manifest save"
        );
        self.inner.save_manifest(manifest)
    }
}

#[test]
fn restart_recovery_uses_persisted_origin_evidence_and_restores_the_unbound_manifest() {
    let LegacyFixture {
        _temp,
        state,
        app_data,
        game,
        mod_id,
        manifest,
        ..
    } = LegacyFixture::new(false);
    let before = snapshot_file_tree(&game);
    drop(state);
    let manifests = Arc::new(CrashBeforeManifestSave {
        inner: JsonInstallManifestRepository::new(app_data.join("install/manifests")),
        crash: AtomicBool::new(false),
    });
    let state = HmmRuntime::builder(app_data.clone())
        .with_install_manifest_repository(manifests.clone())
        .build()
        .unwrap();
    let selection = selection(&state, &mod_id, "one002", Some("pl129_0000"));
    let preview = state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(selection.clone())
        .unwrap();
    let request = StartEquipmentRetargetReinstallTaskRequest {
        selection,
        plan_token: preview.plan_token.unwrap(),
    };
    let task = state
        .reinstall_tasks
        .start_equipment_retarget_reinstall_task(request.clone())
        .unwrap();
    manifests.crash.store(true, Ordering::SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = state
            .reinstall_task_runner
            .run_equipment_retarget_reinstall_task(&task.task_id, request);
    }))
    .is_err());
    let pending = state
        .reinstall_recovery_repository
        .list_transactions(&ProfileId::new("default"))
        .unwrap();
    assert_eq!(pending.len(), 1);
    pending[0].validate().unwrap();
    assert!(pending[0].original_install_evidence.is_some());
    assert_eq!(pending[0].pre_reinstall_manifest, manifest);
    assert_ne!(
        snapshot_file_tree(&game),
        before,
        "interruption happens after player-file mutations"
    );
    drop(state);

    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
    let recovered = state
        .install_recovery_scanner
        .scan(
            GameId::mhw(),
            InstallRecoveryScanRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![mod_id.clone()],
            },
        )
        .unwrap();
    assert_eq!(recovered[0].status, InstallRecoveryStatus::RollbackRequired);
    let request = hmm_app::StartRecoveryActionTaskRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        mod_id,
        action_kind: hmm_app::InstallRecoveryActionKind::ReconcileReinstall,
        plan_token: None,
    };
    let task = state
        .recovery_action_tasks
        .start_recovery_action_task(request.clone())
        .unwrap();
    let events = state
        .recovery_action_task_runner
        .run_recovery_action_task(&task.task_id, request)
        .unwrap();
    assert_eq!(events.last().unwrap().status, TaskStatus::Completed);
    assert_eq!(snapshot_file_tree(&game), before);
    assert_eq!(read_fixture_manifest(&app_data), manifest);
    assert_no_reinstall_recovery_transactions(&app_data);
}
