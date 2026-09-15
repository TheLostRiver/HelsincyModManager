use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn v3_resource_repair_manifest_failure_restores_all_bytes_bindings_and_backups() {
    for armor in [false, true] {
        let (fixture, _) = legacy_resources(armor);
        let before = snapshot_file_tree(&fixture.game);
        let manifest = read_fixture_manifest(&fixture.app_data);
        let prepared = fixture
            .state
            .reinstall_executor
            .prepare_equipment_retarget_reinstall(fixture.reapply())
            .unwrap();
        let token = prepared.plan_token().to_owned();
        fixture.manifests.fail_next_save();
        assert!(matches!(
            fixture.state.reinstall_executor.commit(prepared, &token),
            Err(hmm_app::ReinstallCommitError::RolledBack {
                failed_phase: hmm_app::ReinstallCommitPhase::Manifest,
                ..
            })
        ));
        assert_eq!(snapshot_file_tree(&fixture.game), before);
        assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
        assert_no_retarget_staging(&fixture.app_data);
        fixture.uninstall();
    }
}

#[test]
fn resource_repair_rechecks_original_material_after_staging_before_any_game_write() {
    let (fixture, paths) = legacy_resources(false);
    let before = snapshot_file_tree(&fixture.game);
    let manifest = read_fixture_manifest(&fixture.app_data);
    let prepared = fixture
        .state
        .reinstall_executor
        .prepare_equipment_retarget_reinstall(fixture.reapply())
        .unwrap();
    let token = prepared.plan_token().to_owned();
    let material = package_root(&fixture).join(&paths.original[1]);
    assert!(material.is_file());
    fs::write(
        material,
        single_texture_material("wp/two/two003/mod/changed"),
    )
    .unwrap();
    assert_eq!(
        fixture
            .state
            .reinstall_executor
            .commit(prepared, &token)
            .unwrap_err(),
        hmm_app::ReinstallCommitError::PreviewStale
    );
    assert_eq!(snapshot_file_tree(&fixture.game), before);
    assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
    assert_no_reinstall_recovery_transactions(&fixture.app_data);
    assert_no_retarget_staging(&fixture.app_data);
}

struct CrashBeforeManifest {
    repository: JsonInstallManifestRepository,
    crash: AtomicBool,
}

impl InstallManifestRepository for CrashBeforeManifest {
    fn load_manifest(&self, profile: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        self.repository.load_manifest(profile)
    }
    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        assert!(
            !self.crash.swap(false, Ordering::SeqCst),
            "synthetic crash before manifest"
        );
        self.repository.save_manifest(manifest)
    }
}

#[test]
fn interrupted_resource_repair_recovers_the_v3_materials_paths_and_manifest_after_restart() {
    for armor in [false, true] {
        let (mut fixture, _) = legacy_resources(armor);
        let before = snapshot_file_tree(&fixture.game);
        let manifest = read_fixture_manifest(&fixture.app_data);
        let crash = Arc::new(CrashBeforeManifest {
            repository: JsonInstallManifestRepository::new(
                fixture.app_data.join("install/manifests"),
            ),
            crash: AtomicBool::new(false),
        });
        fixture.state = HmmRuntime::builder(fixture.app_data.clone())
            .with_install_manifest_repository(crash.clone())
            .build()
            .unwrap();
        let prepared = fixture
            .state
            .reinstall_executor
            .prepare_equipment_retarget_reinstall(fixture.reapply())
            .unwrap();
        let token = prepared.plan_token().to_owned();
        crash.crash.store(true, Ordering::SeqCst);
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _ = fixture.state.reinstall_executor.commit(prepared, &token);
        }))
        .is_err());
        let transactions = fixture
            .state
            .reinstall_recovery_repository
            .list_transactions(&ProfileId::new("default"))
            .unwrap();
        assert_eq!(transactions.len(), 1);
        transactions[0].validate().unwrap();
        assert_eq!(transactions[0].pre_reinstall_manifest, manifest);
        assert_ne!(snapshot_file_tree(&fixture.game), before);
        let fixture = fixture.restart();
        let statuses = fixture
            .state
            .install_recovery_scanner
            .scan(
                GameId::mhw(),
                InstallRecoveryScanRequest {
                    profile_id: ProfileId::new("default"),
                    mod_ids: vec![fixture.mod_id.clone()],
                },
            )
            .unwrap();
        assert_eq!(statuses[0].status, InstallRecoveryStatus::RollbackRequired);
        let request = hmm_app::StartRecoveryActionTaskRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: fixture.mod_id.clone(),
            action_kind: hmm_app::InstallRecoveryActionKind::ReconcileReinstall,
            plan_token: None,
        };
        let task = fixture
            .state
            .recovery_action_tasks
            .start_recovery_action_task(request.clone())
            .unwrap();
        fixture
            .state
            .recovery_action_task_runner
            .run_recovery_action_task(&task.task_id, request)
            .unwrap();
        assert_eq!(snapshot_file_tree(&fixture.game), before);
        assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
        assert_no_retarget_staging(&fixture.app_data);
        fixture.uninstall();
    }
}

#[test]
fn readonly_batch_reapply_and_commit_agree_on_v3_material_migration_facts() {
    use hmm_core::{
        BatchExecutionPolicy, BatchItemInput, BatchOperation, BatchPlanRequest,
        ReinstallBatchItemInput, BATCH_PLAN_SCHEMA_VERSION,
    };
    let (fixture, paths) = legacy_resources(false);
    let manifest = read_fixture_manifest(&fixture.app_data);
    let before = snapshot_file_tree(&fixture.game);
    let revision = manifest
        .entries
        .iter()
        .find(|entry| entry.mod_id == fixture.mod_id)
        .unwrap()
        .revision_id
        .clone()
        .unwrap();
    let request = crate::BatchLifecyclePlanRequest {
        plan: BatchPlanRequest {
            schema_version: BATCH_PLAN_SCHEMA_VERSION,
            operation: BatchOperation::Reinstall,
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            execution_policy: BatchExecutionPolicy::StopOnFailure,
            items: vec![BatchItemInput::Reinstall(ReinstallBatchItemInput {
                intent: hmm_core::ReinstallIntent::ReapplyEquipmentTargets,
                mod_id: fixture.mod_id.clone(),
                installed_revision_id: revision.clone(),
                candidate_revision_id: revision,
                layer: FileLayer::new("base", 0),
                replacement_binding_snapshot: None,
            })],
        },
        replacement_targets: Default::default(),
    };
    let environment = crate::RuntimeEnvironment::sandbox(fixture.app_data.clone()).unwrap();
    let preview =
        crate::BatchLifecycleAutomation::preview_request(&environment, request.clone()).unwrap();
    assert_eq!(preview.plan.status(), hmm_core::BatchPlanStatus::Ready);
    assert_eq!(snapshot_file_tree(&fixture.game), before);
    assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
    assert_no_retarget_staging(&fixture.app_data);
    let database = fixture.state.database_handle();
    let (_, sealed) = crate::BatchLifecycleAutomation::seal_request_with_database(
        &environment,
        request,
        preview.preview_token.as_deref().unwrap(),
        Arc::clone(&database),
    )
    .unwrap();
    let (_, result) = crate::BatchLifecycleAutomation::start_request_with_database(
        &environment,
        &sealed.batch_id,
        &sealed.plan_token,
        database,
    )
    .unwrap();
    assert_eq!(result.status, hmm_core::BatchAttemptStatus::Completed);
    assert_eq!(snapshot_file_tree(&fixture.game), paths.expected(&fixture));
    assert_no_reinstall_recovery_transactions(&fixture.app_data);
    assert_no_retarget_staging(&fixture.app_data);
    fixture.uninstall();
}
