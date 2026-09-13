use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

struct CancelPluginApply {
    tasks: Arc<hmm_app::TaskManager>,
    phase: &'static str,
}
impl hmm_app::TaskProgressObserver for CancelPluginApply {
    type Error = std::convert::Infallible;
    fn observe(&self, event: &hmm_app::TaskProgressEvent) -> Result<(), Self::Error> {
        if event.phase == self.phase {
            self.tasks.cancel_task(&event.task_id).unwrap();
        }
        Ok(())
    }
}

#[test]
fn cancelling_plugin_changes_at_each_safe_barrier_keeps_the_applied_snapshot() {
    for phase in [
        "install.reinstall.preflight.processing",
        "install.reinstall.commit.processing",
    ] {
        let fixture = AutomationFixture::new();
        fixture.install();
        fixture.choose(false);
        let before = snapshot_file_tree(&fixture.game);
        let manifest = read_fixture_manifest(&fixture.app_data);
        let preview = fixture
            .state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(fixture.reapply_request())
            .unwrap();
        let request = StartEquipmentRetargetReinstallTaskRequest {
            selection: fixture.reapply_request(),
            plan_token: preview.plan_token.unwrap(),
        };
        let task = fixture
            .state
            .reinstall_tasks
            .start_equipment_retarget_reinstall_task(request.clone())
            .unwrap();
        let observer = CancelPluginApply {
            tasks: fixture.state.task_manager.clone(),
            phase,
        };
        fixture
            .state
            .reinstall_task_runner
            .run_equipment_retarget_reinstall_task_with_observer(&task.task_id, request, &observer)
            .unwrap();
        assert_eq!(
            fixture.state.task_manager.task_status(&task.task_id),
            Some(TaskStatus::Cancelled)
        );
        assert_eq!(snapshot_file_tree(&fixture.game), before);
        assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
    }
}

struct CrashPluginManifest {
    delegate: JsonInstallManifestRepository,
    crash: AtomicBool,
}
impl InstallManifestRepository for CrashPluginManifest {
    fn load_manifest(&self, profile: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        self.delegate.load_manifest(profile)
    }
    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        assert!(
            !self.crash.swap(false, Ordering::SeqCst),
            "simulated plugin manifest interruption"
        );
        self.delegate.save_manifest(manifest)
    }
}

#[test]
fn restart_recovery_restores_removed_plugin_and_original_applied_choices() {
    let fixture = AutomationFixture::new();
    fixture.install();
    fixture.choose(false);
    let before = snapshot_file_tree(&fixture.game);
    let manifest = read_fixture_manifest(&fixture.app_data);
    let AutomationFixture {
        _temp,
        app_data,
        game,
        mod_id,
        state,
        ..
    } = fixture;
    drop(state);
    let repository = Arc::new(CrashPluginManifest {
        delegate: JsonInstallManifestRepository::new(app_data.join("install/manifests")),
        crash: AtomicBool::new(false),
    });
    let state = HmmRuntime::builder(app_data.clone())
        .with_install_manifest_repository(repository.clone())
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
    repository.crash.store(true, Ordering::SeqCst);
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
    transactions[0].validate().unwrap();
    assert_eq!(transactions[0].pre_reinstall_manifest, manifest);
    assert_eq!(
        transactions[0].candidate_plugin_selections[0].files()[0].choice,
        PluginFileChoiceKind::Exclude
    );
    assert_ne!(snapshot_file_tree(&game), before);
    drop(state);
    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
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
    assert_eq!(read_fixture_manifest(&app_data), manifest);
    assert_no_reinstall_recovery_transactions(&app_data);
}
