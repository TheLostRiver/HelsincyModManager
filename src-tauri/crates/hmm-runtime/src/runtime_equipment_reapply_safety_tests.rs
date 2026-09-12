use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn author_original_reapply_still_requires_a_consistent_catalog_target_identity() {
    let fixture = old_layout_fixture(false);
    let mut manifest = serde_json::to_value(read_fixture_manifest(&fixture.app_data)).unwrap();
    let binding = manifest["replacement_bindings"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|binding| binding["source_internal_id"] == "one001")
        .unwrap();
    binding["binding"]["target_id"] = serde_json::json!(target("one002", "wp/one").as_str());
    let manifest: InstallManifest = serde_json::from_value(manifest).unwrap();
    fixture.manifests.save_manifest(&manifest).unwrap();
    let before = snapshot_file_tree(&fixture.game);
    assert!(fixture
        .state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(reapply(&fixture))
        .is_err());
    assert_eq!(snapshot_file_tree(&fixture.game), before);
    assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
    assert_no_retarget_staging(&fixture.app_data);
}

#[test]
fn prepared_reapply_rejects_source_target_and_token_drift_before_writes() {
    for kind in ["source", "source_missing", "target", "token"] {
        let fixture = old_layout_fixture(false);
        let manifest = read_fixture_manifest(&fixture.app_data);
        let prepared = fixture
            .state
            .reinstall_executor
            .prepare_equipment_retarget_reinstall(reapply(&fixture))
            .unwrap();
        let mut token = prepared.plan_token().to_owned();
        match kind {
            "source" => fs::write(fixture.package.join(MODEL), b"modified source fixture").unwrap(),
            "source_missing" => fs::remove_file(fixture.package.join(MODEL)).unwrap(),
            "target" => {
                fs::write(fixture.game.join(OLD_MODEL), b"modified target fixture").unwrap()
            }
            "token" => token.push_str("-stale"),
            _ => unreachable!(),
        }
        let before = snapshot_file_tree(&fixture.game);
        assert!(
            fixture
                .state
                .reinstall_executor
                .commit(prepared, &token)
                .is_err(),
            "accepted {kind} drift after preparation"
        );
        assert_eq!(snapshot_file_tree(&fixture.game), before);
        assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
        assert_no_retarget_staging(&fixture.app_data);
    }
}

#[test]
fn a_noop_reapply_cannot_be_forced_through_the_commit_service() {
    let fixture = Fixture::new(false, true);
    let before = snapshot_file_tree(&fixture.game);
    let manifest = fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap();
    let prepared = fixture
        .state
        .reinstall_executor
        .prepare_equipment_retarget_reinstall(reapply(&fixture))
        .unwrap();
    let token = prepared.plan_token().to_owned();
    assert!(fixture
        .state
        .reinstall_executor
        .commit(prepared, &token)
        .is_err());
    assert_eq!(snapshot_file_tree(&fixture.game), before);
    assert_eq!(
        fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap(),
        manifest
    );
    assert_no_reinstall_recovery_transactions(&fixture.app_data);
}

struct CancelReapply {
    tasks: Arc<hmm_app::TaskManager>,
    phase: &'static str,
    cancelled: AtomicBool,
}

impl hmm_app::TaskProgressObserver for CancelReapply {
    type Error = std::convert::Infallible;
    fn observe(&self, event: &hmm_app::TaskProgressEvent) -> Result<(), Self::Error> {
        if event.phase == self.phase {
            self.tasks.cancel_task(&event.task_id).unwrap();
            self.cancelled.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

#[test]
fn reapply_cancellation_at_preflight_and_commit_barriers_keeps_the_original_layout() {
    for phase in [
        "install.reinstall.preflight.processing",
        "install.reinstall.commit.processing",
    ] {
        let fixture = old_layout_fixture(false);
        let before = snapshot_file_tree(&fixture.game);
        let manifest = fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap();
        let selection = reapply(&fixture);
        let preview = fixture
            .state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(selection.clone())
            .unwrap();
        let request = StartEquipmentRetargetReinstallTaskRequest {
            selection,
            plan_token: preview.plan_token.unwrap(),
        };
        let task = fixture
            .state
            .reinstall_tasks
            .start_equipment_retarget_reinstall_task(request.clone())
            .unwrap();
        let observer = CancelReapply {
            tasks: fixture.state.task_manager.clone(),
            phase,
            cancelled: AtomicBool::new(false),
        };
        fixture
            .state
            .reinstall_task_runner
            .run_equipment_retarget_reinstall_task_with_observer(&task.task_id, request, &observer)
            .unwrap();
        assert!(observer.cancelled.load(Ordering::SeqCst));
        assert_eq!(
            fixture.state.task_manager.task_status(&task.task_id),
            Some(TaskStatus::Cancelled)
        );
        assert_eq!(snapshot_file_tree(&fixture.game), before);
        assert_eq!(
            fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap(),
            manifest
        );
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
        assert_no_retarget_staging(&fixture.app_data);
    }
}

struct CrashBeforeReapplySave {
    repository: JsonInstallManifestRepository,
    crash: AtomicBool,
}

impl InstallManifestRepository for CrashBeforeReapplySave {
    fn load_manifest(&self, profile: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        self.repository.load_manifest(profile)
    }
    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        assert!(
            !self.crash.swap(false, Ordering::SeqCst),
            "simulated reapply interruption before manifest save"
        );
        self.repository.save_manifest(manifest)
    }
}

#[test]
fn restart_recovery_reads_the_reapply_intent_and_restores_the_previous_layout() {
    let Fixture {
        _temp,
        state,
        app_data,
        game,
        mod_id,
        ..
    } = old_layout_fixture(false);
    let before = snapshot_file_tree(&game);
    let before_manifest = read_fixture_manifest(&app_data);
    drop(state);
    let manifests = Arc::new(CrashBeforeReapplySave {
        repository: JsonInstallManifestRepository::new(app_data.join("install/manifests")),
        crash: AtomicBool::new(false),
    });
    let state = HmmRuntime::builder(app_data.clone())
        .with_install_manifest_repository(manifests.clone())
        .build()
        .unwrap();
    let selection = EquipmentRetargetReinstallRequest::reapply(
        GameId::mhw(),
        ProfileId::new("default"),
        mod_id.clone(),
    );
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
    let transactions = state
        .reinstall_recovery_repository
        .list_transactions(&ProfileId::new("default"))
        .unwrap();
    assert_eq!(transactions.len(), 1);
    assert_eq!(
        transactions[0].intent,
        hmm_core::ReinstallIntent::ReapplyEquipmentTargets
    );
    transactions[0].validate().unwrap();
    assert_ne!(snapshot_file_tree(&game), before);
    drop(state);
    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
    let statuses = state
        .install_recovery_scanner
        .scan(
            GameId::mhw(),
            InstallRecoveryScanRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![mod_id.clone()],
            },
        )
        .unwrap();
    assert_eq!(statuses[0].status, InstallRecoveryStatus::RollbackRequired);
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
    state
        .recovery_action_task_runner
        .run_recovery_action_task(&task.task_id, request)
        .unwrap();
    assert_eq!(snapshot_file_tree(&game), before);
    assert_eq!(read_fixture_manifest(&app_data), before_manifest);
    assert_no_reinstall_recovery_transactions(&app_data);
}
