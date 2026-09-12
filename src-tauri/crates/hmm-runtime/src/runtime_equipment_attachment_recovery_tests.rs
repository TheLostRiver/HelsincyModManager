use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

struct CancelAtPhase {
    tasks: Arc<hmm_app::TaskManager>,
    phase: &'static str,
    cancelled: AtomicBool,
}

impl hmm_app::TaskProgressObserver for CancelAtPhase {
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
fn cancelled_switch_keeps_attachments_and_the_original_manifest_untouched() {
    for phase in [
        "install.reinstall.preflight.processing",
        "install.reinstall.commit.processing",
    ] {
        let fixture = Fixture::new(false, true);
        let before = snapshot_file_tree(&fixture.game);
        let manifest_path = fixture.app_data.join("install/manifests/default.json");
        let before_json = fs::read(&manifest_path).unwrap();
        let request = StartEquipmentRetargetReinstallTaskRequest {
            selection: fixture.request(),
            plan_token: fixture.preview().plan_token.unwrap(),
        };
        let task = fixture
            .state
            .reinstall_tasks
            .start_equipment_retarget_reinstall_task(request.clone())
            .unwrap();
        let observer = CancelAtPhase {
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
        assert_eq!(fs::read(&manifest_path).unwrap(), before_json);
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
        assert_no_retarget_staging(&fixture.app_data);
    }
}

struct CrashBeforeSave {
    repository: JsonInstallManifestRepository,
    crash: AtomicBool,
}

impl InstallManifestRepository for CrashBeforeSave {
    fn load_manifest(&self, profile: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        self.repository.load_manifest(profile)
    }
    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        assert!(
            !self.crash.swap(false, Ordering::SeqCst),
            "simulated interruption before manifest save"
        );
        self.repository.save_manifest(manifest)
    }
}

#[test]
fn restart_recovery_keeps_retained_attachment_evidence_and_restores_equipment() {
    let Fixture {
        _temp,
        state,
        app_data,
        game,
        mod_id,
        ..
    } = Fixture::new(false, true);
    let before = snapshot_file_tree(&game);
    let before_manifest = read_fixture_manifest(&app_data);
    drop(state);
    let manifests = Arc::new(CrashBeforeSave {
        repository: JsonInstallManifestRepository::new(app_data.join("install/manifests")),
        crash: AtomicBool::new(false),
    });
    let state = HmmRuntime::builder(app_data.clone())
        .with_install_manifest_repository(manifests.clone())
        .build()
        .unwrap();
    let selection = selection(&state, &mod_id, "one002", None);
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
    let transaction = &transactions[0];
    transaction.validate().unwrap();
    assert_eq!(transaction.pre_reinstall_manifest, before_manifest);
    for path in [PLUGIN, TOOL] {
        let fact = transaction
            .targets
            .iter()
            .find(|item| item.target_path.as_str() == path)
            .unwrap();
        assert_eq!(fact.class, hmm_core::ReinstallTargetClass::Retained);
        assert_eq!(fact.pre_state, fact.candidate_state);
        assert_eq!(fs::read(game.join(path)).unwrap(), before[path]);
    }
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

#[test]
fn revision_reinstall_still_removes_files_removed_from_the_new_package() {
    let fixture = Fixture::new(false, true);
    let old = read_fixture_manifest(&fixture.app_data).entries[0]
        .revision_id
        .clone()
        .unwrap();
    let archive = fixture
        ._temp
        .path()
        .join("revision-without-attachments.zip");
    create_fixture_zip(&archive, EQUIPMENT_FILES);
    let (_, revision) =
        import_candidate_fixture_revision(&fixture.state, &archive, &fixture.mod_id, &old);
    let preview_request = hmm_app::ReinstallPreviewRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        mod_id: fixture.mod_id.clone(),
        candidate_revision_id: revision,
        layer: FileLayer::new("base", 0),
    };
    let preview = fixture
        .state
        .reinstall_executor
        .preview(preview_request.clone())
        .unwrap();
    assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
    assert!(preview.attachment_counts.is_empty());
    let request = hmm_app::StartReinstallTaskRequest {
        game_id: preview_request.game_id,
        profile_id: preview_request.profile_id,
        mod_id: preview_request.mod_id,
        candidate_revision_id: preview_request.candidate_revision_id,
        layer: preview_request.layer,
        plan_token: preview.plan_token.unwrap(),
    };
    let task = fixture
        .state
        .reinstall_tasks
        .start_reinstall_task(request.clone())
        .unwrap();
    fixture
        .state
        .reinstall_task_runner
        .run_reinstall_task(&task.task_id, request)
        .unwrap();
    assert_eq!(
        fs::read(fixture.game.join(PLUGIN)).unwrap(),
        b"original external file"
    );
    assert!(!fixture.game.join(TOOL).exists());
    assert!(read_fixture_manifest(&fixture.app_data)
        .entries
        .iter()
        .all(|item| ![PLUGIN, TOOL].contains(&item.target_path.as_str())));
}
