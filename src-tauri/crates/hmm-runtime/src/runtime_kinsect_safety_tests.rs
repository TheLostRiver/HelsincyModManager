use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn overlapping_kinsect_files_block_initial_install_and_retarget_without_any_writes() {
    let f = Fixture::new(&[
        (MUS_MODEL, b"artificial first kinsect"),
        (
            "nativePC/wp/mus/mus002/mod/mus002.mod3",
            b"artificial second kinsect",
        ),
    ]);
    let changes = [("mus001", "mus003"), ("mus002", "mus003")];
    let request = initial_request(f.choices(&changes));
    let preview = f
        .state
        .initial_retarget_install_preflight
        .preview(request.clone())
        .unwrap();
    assert!(preview.planned.install_plan().has_blocking_conflicts());
    assert!(preview.planned.install_plan().actions.is_empty());
    let task = f
        .state
        .retarget_install_tasks
        .start_equipment_retarget_install_task(request.clone())
        .unwrap();
    let error = f
        .state
        .retarget_install_task_runner
        .run_equipment_retarget_install_task(&task.task_id, request)
        .unwrap_err();
    assert_eq!(
        error.events.last().unwrap().error.as_deref(),
        Some("install_retarget_failed:planning")
    );
    assert_eq!(snapshot_file_tree(&f.game), f.baseline);
    assert!(!f.app_data.join("install/manifests/default.json").exists());
    assert_no_retarget_staging(&f.app_data);

    f.install();
    let before = snapshot_file_tree(&f.game);
    let manifest = read_fixture_manifest(&f.app_data);
    let preview = f.preview(f.choices(&changes));
    assert_eq!(preview.status, ReinstallPreviewStatus::Blocked);
    assert!(preview
        .blocking_reasons
        .iter()
        .any(|reason| reason.reason == hmm_app::ReinstallBlockingReason::PlanConflict));
    assert!(preview.plan_token.is_none());
    assert_eq!(snapshot_file_tree(&f.game), before);
    assert_eq!(read_fixture_manifest(&f.app_data), manifest);
    assert_no_retarget_staging(&f.app_data);
    assert_no_reinstall_recovery_transactions(&f.app_data);
    f.uninstall();
}

#[test]
fn old_kinsect_sources_reject_missing_changed_adopted_and_foreign_files() {
    for mutation in [
        "source",
        "missing-source",
        "target",
        "missing-target",
        "adopted",
        "owner",
        "summary",
        "revision",
    ] {
        let f = Fixture::new(FILES);
        f.install();
        let mut manifest = f.legacy_bindings();
        let selection = f.choices(&[("mus001", "mus003")]);
        let entry = manifest
            .entries
            .iter_mut()
            .find(|entry| entry.target_path.as_str() == MUS_MODEL)
            .unwrap();
        match mutation {
            "source" => fs::write(
                f.package.join(entry.package_file_id.as_str()),
                b"changed source",
            )
            .unwrap(),
            "missing-source" => {
                fs::remove_file(f.package.join(entry.package_file_id.as_str())).unwrap()
            }
            "target" => fs::write(f.game.join(MUS_MODEL), b"changed target").unwrap(),
            "missing-target" => fs::remove_file(f.game.join(MUS_MODEL)).unwrap(),
            "adopted" => entry.adopted = true,
            "owner" => entry.mod_id = ModId::new("another-mod"),
            "summary" => entry.installed_file = None,
            "revision" => entry.revision_id = Some(ModRevisionId::new("other-revision")),
            _ => unreachable!(),
        }
        // 人工损坏清单用于验证读侧拒绝；生产 repository 不允许写入跨 revision 清单。
        let encoded = serde_json::to_vec(&manifest).unwrap();
        fs::write(f.app_data.join("install/manifests/default.json"), &encoded).unwrap();
        let before = snapshot_file_tree(&f.game);
        let result = f
            .state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(selection);
        assert!(
            !matches!(
                result,
                Ok(ReinstallPlanPreview {
                    status: ReinstallPreviewStatus::Ready,
                    ..
                })
            ),
            "{mutation}"
        );
        assert_eq!(snapshot_file_tree(&f.game), before);
        assert_eq!(
            fs::read(f.app_data.join("install/manifests/default.json")).unwrap(),
            encoded
        );
        assert_no_reinstall_recovery_transactions(&f.app_data);
        assert_no_retarget_staging(&f.app_data);
    }
}

#[test]
fn prepared_kinsect_switch_rechecks_original_bytes_targets_bindings_and_token() {
    for mutation in ["source", "target", "binding", "token"] {
        let f = Fixture::new(FILES);
        f.install();
        let mut manifest = f.legacy_bindings();
        let prepared = f
            .state
            .reinstall_executor
            .prepare_equipment_retarget_reinstall(f.choices(&[("mus001", "mus003")]))
            .unwrap();
        let mut token = prepared.plan_token().to_owned();
        match mutation {
            "source" => fs::write(f.package.join(MUS_MODEL), b"changed after staging").unwrap(),
            "target" => fs::write(f.game.join(MUS_MODEL), b"changed after preview").unwrap(),
            "binding" => {
                manifest.replacement_bindings.pop();
                f.manifests.save_manifest(&manifest).unwrap();
            }
            "token" => token.push_str("-stale"),
            _ => unreachable!(),
        }
        let before = snapshot_file_tree(&f.game);
        assert!(
            f.state.reinstall_executor.commit(prepared, &token).is_err(),
            "{mutation}"
        );
        assert_eq!(snapshot_file_tree(&f.game), before);
        assert_eq!(read_fixture_manifest(&f.app_data), manifest);
        assert_no_reinstall_recovery_transactions(&f.app_data);
        assert_no_retarget_staging(&f.app_data);
    }
}

#[test]
fn failed_kinsect_commit_restores_the_exact_old_binding_set_and_backup_baseline() {
    let f = Fixture::new(FILES);
    f.install();
    let manifest = f.legacy_bindings();
    let before = snapshot_file_tree(&f.game);
    let prepared = f
        .state
        .reinstall_executor
        .prepare_equipment_retarget_reinstall(f.choices(&[("mus001", "mus003")]))
        .unwrap();
    let token = prepared.plan_token().to_owned();
    f.manifests.fail_next_save();
    assert!(f.state.reinstall_executor.commit(prepared, &token).is_err());
    assert_eq!(read_fixture_manifest(&f.app_data), manifest);
    assert_eq!(snapshot_file_tree(&f.game), before);
    assert_no_reinstall_recovery_transactions(&f.app_data);
    f.uninstall();
}

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
fn cancellation_does_not_persist_discovered_kinsect_bindings() {
    for phase in [
        "install.reinstall.preflight.processing",
        "install.reinstall.commit.processing",
    ] {
        let f = Fixture::new(FILES);
        f.install();
        f.legacy_bindings();
        let before = snapshot_file_tree(&f.game);
        let path = f.app_data.join("install/manifests/default.json");
        let manifest = fs::read(&path).unwrap();
        let request = f.request(f.choices(&[("mus001", "mus003")]));
        let task = f
            .state
            .reinstall_tasks
            .start_equipment_retarget_reinstall_task(request.clone())
            .unwrap();
        let observer = CancelAtPhase {
            tasks: f.state.task_manager.clone(),
            phase,
            cancelled: AtomicBool::new(false),
        };
        f.state
            .reinstall_task_runner
            .run_equipment_retarget_reinstall_task_with_observer(&task.task_id, request, &observer)
            .unwrap();
        assert!(observer.cancelled.load(Ordering::SeqCst));
        assert_eq!(
            f.state.task_manager.task_status(&task.task_id),
            Some(TaskStatus::Cancelled)
        );
        assert_eq!(fs::read(path).unwrap(), manifest);
        assert_eq!(snapshot_file_tree(&f.game), before);
        assert_no_reinstall_recovery_transactions(&f.app_data);
    }
}

struct CrashBeforeSave {
    inner: JsonInstallManifestRepository,
    crash: AtomicBool,
}
impl InstallManifestRepository for CrashBeforeSave {
    fn load_manifest(&self, profile: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        self.inner.load_manifest(profile)
    }
    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        assert!(
            !self.crash.swap(false, Ordering::SeqCst),
            "simulated interrupted kinsect commit"
        );
        self.inner.save_manifest(manifest)
    }
}

#[test]
fn restart_recovery_restores_old_bindings_from_persisted_additional_source_evidence() {
    let mut f = Fixture::new(FILES);
    f.install();
    let manifest = f.legacy_bindings();
    let before = snapshot_file_tree(&f.game);
    let crash = Arc::new(CrashBeforeSave {
        inner: JsonInstallManifestRepository::new(f.app_data.join("install/manifests")),
        crash: AtomicBool::new(false),
    });
    f.state = HmmRuntime::builder(f.app_data.clone())
        .with_install_manifest_repository(crash.clone())
        .build()
        .unwrap();
    let request = f.request(f.choices(&[("mus001", "mus003")]));
    let task = f
        .state
        .reinstall_tasks
        .start_equipment_retarget_reinstall_task(request.clone())
        .unwrap();
    crash.crash.store(true, Ordering::SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = f
            .state
            .reinstall_task_runner
            .run_equipment_retarget_reinstall_task(&task.task_id, request);
    }))
    .is_err());
    let pending = f
        .state
        .reinstall_recovery_repository
        .list_transactions(&ProfileId::new("default"))
        .unwrap();
    assert_eq!(pending.len(), 1);
    pending[0].validate().unwrap();
    assert!(pending[0].additional_sources_evidence.is_some());
    assert!(pending[0].original_install_evidence.is_none());
    assert_eq!(pending[0].pre_reinstall_manifest, manifest);
    assert_ne!(snapshot_file_tree(&f.game), before);
    f.state = HmmRuntime::from_app_data_dir(f.app_data.clone()).unwrap();
    let request = hmm_app::StartRecoveryActionTaskRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        mod_id: f.mod_id.clone(),
        action_kind: hmm_app::InstallRecoveryActionKind::ReconcileReinstall,
        plan_token: None,
    };
    let task = f
        .state
        .recovery_action_tasks
        .start_recovery_action_task(request.clone())
        .unwrap();
    let events = f
        .state
        .recovery_action_task_runner
        .run_recovery_action_task(&task.task_id, request)
        .unwrap();
    assert_eq!(events.last().unwrap().status, TaskStatus::Completed);
    assert_eq!(snapshot_file_tree(&f.game), before);
    assert_eq!(read_fixture_manifest(&f.app_data), manifest);
    assert_no_reinstall_recovery_transactions(&f.app_data);
    f.uninstall();
}

#[test]
fn reapply_can_update_an_old_glaive_layout_and_add_original_kinsect_bindings_atomically() {
    let f = Fixture::new(FILES);
    f.install();
    f.run(f.choices(&[("rod001", "rod002")]));
    let mut manifest = f.legacy_bindings();
    let current = "nativePC/wp/rod/rod002/mod/rod002/rod002.mod3";
    let legacy = "nativePC/wp/rod/rod002/mod/rod001/rod002.mod3";
    fs::create_dir_all(f.game.join(legacy).parent().unwrap()).unwrap();
    fs::rename(f.game.join(current), f.game.join(legacy)).unwrap();
    manifest
        .entries
        .iter_mut()
        .find(|entry| entry.target_path.as_str() == current)
        .unwrap()
        .target_path = hmm_core::InstallTargetPath::parse(legacy, ["nativePC"]).unwrap();
    f.manifests.save_manifest(&manifest).unwrap();
    f.run(f.reapply());
    let after = read_fixture_manifest(&f.app_data);
    assert_eq!(after.replacement_bindings.len(), 4);
    assert_eq!(fs::read(f.game.join(current)).unwrap(), FILES[0].1);
    assert!(!f.game.join(legacy).exists());
    assert_eq!(fs::read(f.game.join(MUS_MODEL)).unwrap(), FILES[1].1);
    assert_eq!(after.plugin_selections, manifest.plugin_selections);
    f.uninstall();
}
