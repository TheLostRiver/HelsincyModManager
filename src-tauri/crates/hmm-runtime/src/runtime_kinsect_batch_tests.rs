use super::*;
use hmm_core::{
    BatchExecutionPolicy, BatchItemInput, BatchOperation, BatchPlanRequest,
    ReinstallBatchItemInput, ReinstallIntent, BATCH_PLAN_SCHEMA_VERSION,
};

fn batch_request(f: &Fixture) -> crate::BatchLifecyclePlanRequest {
    let revision = read_fixture_manifest(&f.app_data).entries[0]
        .revision_id
        .clone()
        .unwrap();
    crate::BatchLifecyclePlanRequest {
        plan: BatchPlanRequest {
            schema_version: BATCH_PLAN_SCHEMA_VERSION,
            operation: BatchOperation::Reinstall,
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            execution_policy: BatchExecutionPolicy::StopOnFailure,
            items: vec![BatchItemInput::Reinstall(ReinstallBatchItemInput {
                intent: ReinstallIntent::ReapplyEquipmentTargets,
                mod_id: f.mod_id.clone(),
                installed_revision_id: revision.clone(),
                candidate_revision_id: revision,
                layer: FileLayer::new("ignored-caller-layer", 123),
                replacement_binding_snapshot: None,
            })],
        },
        replacement_targets: BTreeMap::new(),
    }
}

#[test]
fn batch_reapply_uses_the_same_kinsect_evidence_and_only_saves_bindings_with_file_changes() {
    for changed in [false, true] {
        let f = Fixture::new(FILES);
        f.install();
        f.run(f.choices(&[("rod001", "rod002")]));
        f.legacy_bindings();
        if changed {
            f.old_glaive_layout();
        }
        let manifest_path = f.app_data.join("install/manifests/default.json");
        let before_manifest = fs::read(&manifest_path).unwrap();
        let before = snapshot_file_tree(&f.game);
        let environment = crate::RuntimeEnvironment::sandbox(f.app_data.clone()).unwrap();
        let request = batch_request(&f);
        let preview =
            crate::BatchLifecycleAutomation::preview_request(&environment, request.clone())
                .unwrap();
        assert_eq!(preview.plan.status(), hmm_core::BatchPlanStatus::Ready);
        assert_eq!(
            preview.plan.items[0]
                .warning_codes
                .iter()
                .any(|code| code == "reapply_no_changes"),
            !changed
        );
        assert_eq!(snapshot_file_tree(&f.game), before);
        assert_eq!(fs::read(&manifest_path).unwrap(), before_manifest);
        let database = f.state.database_handle();
        let (_, sealed) = crate::BatchLifecycleAutomation::seal_request_with_database(
            &environment,
            request,
            preview.preview_token.as_deref().unwrap(),
            database.clone(),
        )
        .unwrap();
        let (_, result) = crate::BatchLifecycleAutomation::start_request_with_database(
            &environment,
            &sealed.batch_id,
            &sealed.plan_token,
            database,
        )
        .unwrap();
        assert_eq!(
            result.status,
            hmm_core::BatchAttemptStatus::Completed,
            "{result:?}"
        );
        if changed {
            assert_eq!(
                read_fixture_manifest(&f.app_data)
                    .replacement_bindings
                    .len(),
                4
            );
            assert!(f
                .game
                .join("nativePC/wp/rod/rod002/mod/rod002/rod002.mod3")
                .is_file());
            assert!(!f
                .game
                .join("nativePC/wp/rod/rod002/mod/rod001/rod002.mod3")
                .exists());
        } else {
            assert_eq!(fs::read(&manifest_path).unwrap(), before_manifest);
            assert_eq!(snapshot_file_tree(&f.game), before);
        }
        assert_eq!(fs::read(f.game.join(MUS_MODEL)).unwrap(), FILES[1].1);
        assert_eq!(fs::read(f.game.join(FILES[7].0)).unwrap(), FILES[7].1);
        f.uninstall();
    }
}

#[test]
fn batch_preview_cannot_be_sealed_after_a_new_kinsect_source_changes() {
    let f = Fixture::new(FILES);
    f.install();
    f.run(f.choices(&[("rod001", "rod002")]));
    f.legacy_bindings();
    f.old_glaive_layout();
    let environment = crate::RuntimeEnvironment::sandbox(f.app_data.clone()).unwrap();
    let request = batch_request(&f);
    let preview =
        crate::BatchLifecycleAutomation::preview_request(&environment, request.clone()).unwrap();
    let before = snapshot_file_tree(&f.game);
    let manifest = read_fixture_manifest(&f.app_data);
    fs::write(
        f.package.join(MUS_MODEL),
        b"changed source after batch preview",
    )
    .unwrap();
    assert!(crate::BatchLifecycleAutomation::seal_request_with_database(
        &environment,
        request,
        preview.preview_token.as_deref().unwrap(),
        f.state.database_handle()
    )
    .is_err());
    assert_eq!(snapshot_file_tree(&f.game), before);
    assert_eq!(read_fixture_manifest(&f.app_data), manifest);
    assert_no_reinstall_recovery_transactions(&f.app_data);
}
