use super::*;
#[path = "runtime_equipment_reapply_safety_tests.rs"]
mod safety;

const MODEL: &str = "nativePC/wp/one/one001/mod/one001.mod3";
const OLD_MODEL: &str = "nativePC/wp/one/one001/legacy/one001.mod3";

fn reapply(fixture: &Fixture) -> EquipmentRetargetReinstallRequest {
    EquipmentRetargetReinstallRequest::reapply(
        GameId::mhw(),
        ProfileId::new("default"),
        fixture.mod_id.clone(),
    )
}

fn old_layout_fixture(single: bool) -> Fixture {
    let fixture = Fixture::new(single, true);
    move_to_old_layout(&fixture);
    fixture
}

fn move_to_old_layout(fixture: &Fixture) {
    fs::create_dir_all(fixture.game.join("nativePC/wp/one/one001/legacy")).unwrap();
    fs::rename(fixture.game.join(MODEL), fixture.game.join(OLD_MODEL)).unwrap();
    let mut manifest = read_fixture_manifest(&fixture.app_data);
    let entry = manifest
        .entries
        .iter_mut()
        .find(|entry| entry.target_path.as_str() == MODEL)
        .unwrap();
    entry.target_path = InstallTargetPath::parse(OLD_MODEL, ["nativePC"]).unwrap();
    fixture.manifests.save_manifest(&manifest).unwrap();
}

#[test]
fn current_rules_with_no_file_changes_do_not_write_or_backfill_legacy_bindings() {
    for single in [true, false] {
        for legacy in [false, true] {
            let fixture = Fixture::new(single, true);
            if legacy {
                let mut manifest = read_fixture_manifest(&fixture.app_data);
                manifest.replacement_bindings.clear();
                manifest.schema_version = 1;
                for entry in &mut manifest.entries {
                    entry.revision_id = None;
                }
                fixture.manifests.save_manifest(&manifest).unwrap();
            }
            let before = snapshot_file_tree(&fixture.game);
            let manifest =
                fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap();
            let preview = fixture
                .state
                .reinstall_executor
                .preview_equipment_retarget_reinstall(reapply(&fixture))
                .unwrap();
            assert_eq!(preview.status, ReinstallPreviewStatus::NoChanges);
            assert!(preview.plan_token.is_none());
            assert!(preview.blocking_reasons.is_empty());
            assert!(!preview.file_effects.is_empty());
            assert!(preview
                .file_effects
                .iter()
                .all(|file| file.change == Some(hmm_core::ReinstallTargetClass::Retained)));
            for path in [PLUGIN, TOOL] {
                let file = preview
                    .file_effects
                    .iter()
                    .find(|file| file.effect.source_path.as_str() == path)
                    .unwrap();
                assert_eq!(
                    file.effect.disposition,
                    hmm_core::RetargetFileDisposition::InstalledAttachmentRetained
                );
                assert_eq!(file.installed_path, file.effect.target_path);
            }
            assert_eq!(snapshot_file_tree(&fixture.game), before);
            assert_eq!(
                fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap(),
                manifest
            );
            assert_no_reinstall_recovery_transactions(&fixture.app_data);
            assert_no_retarget_staging(&fixture.app_data);
        }
    }
}

#[test]
fn reapply_current_rules_updates_files_and_preserves_all_equipment_targets_and_attachments() {
    for single in [true, false] {
        let fixture = old_layout_fixture(single);
        let before = read_fixture_manifest(&fixture.app_data);
        let preview = fixture
            .state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(reapply(&fixture))
            .unwrap();
        assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
        assert_eq!((preview.counts.added, preview.counts.stale), (1, 1));
        let request = StartEquipmentRetargetReinstallTaskRequest {
            selection: reapply(&fixture),
            plan_token: preview.plan_token.unwrap(),
        };
        let task = fixture
            .state
            .reinstall_tasks
            .start_equipment_retarget_reinstall_task(request.clone())
            .unwrap();
        fixture
            .state
            .reinstall_task_runner
            .run_equipment_retarget_reinstall_task(&task.task_id, request)
            .unwrap();
        assert_eq!(
            fs::read(fixture.game.join(MODEL)).unwrap(),
            EQUIPMENT_FILES[0].1
        );
        assert!(!fixture.game.join(OLD_MODEL).exists());
        let after = read_fixture_manifest(&fixture.app_data);
        for previous in &before.replacement_bindings {
            let kept = after
                .replacement_bindings
                .iter()
                .find(|binding| binding.binding_id() == previous.binding_id())
                .unwrap();
            assert_eq!(kept.binding(), previous.binding());
            assert_eq!(kept.target_internal_id(), previous.target_internal_id());
        }
        for path in [PLUGIN, TOOL] {
            assert_eq!(
                after
                    .entries
                    .iter()
                    .find(|entry| entry.target_path.as_str() == path),
                before
                    .entries
                    .iter()
                    .find(|entry| entry.target_path.as_str() == path)
            );
        }
        let next = fixture
            .state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(reapply(&fixture))
            .unwrap();
        assert_eq!(next.status, ReinstallPreviewStatus::NoChanges);
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
        assert_no_retarget_staging(&fixture.app_data);
    }
}

#[test]
fn reapply_rejects_supplied_target_choices_and_changed_managed_files() {
    let fixture = old_layout_fixture(false);
    let before = snapshot_file_tree(&fixture.game);
    let manifest = read_fixture_manifest(&fixture.app_data);
    let mut request = reapply(&fixture);
    request.slots = fixture.request().slots;
    assert!(fixture
        .state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(request)
        .is_err());
    assert_eq!(snapshot_file_tree(&fixture.game), before);
    fs::write(fixture.game.join(OLD_MODEL), b"externally changed fixture").unwrap();
    let changed = snapshot_file_tree(&fixture.game);
    let preview = fixture
        .state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(reapply(&fixture))
        .unwrap();
    assert_eq!(preview.status, ReinstallPreviewStatus::Blocked);
    assert!(preview
        .blocking_reasons
        .iter()
        .any(|reason| reason.reason == hmm_app::ReinstallBlockingReason::TargetChanged));
    assert_eq!(snapshot_file_tree(&fixture.game), changed);
    assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
    assert_no_retarget_staging(&fixture.app_data);
}

#[test]
fn reapply_manifest_failure_restores_the_previous_layout_and_original_metadata() {
    let fixture = old_layout_fixture(false);
    let before = snapshot_file_tree(&fixture.game);
    let manifest = read_fixture_manifest(&fixture.app_data);
    let prepared = fixture
        .state
        .reinstall_executor
        .prepare_equipment_retarget_reinstall(reapply(&fixture))
        .unwrap();
    let token = prepared.plan_token().to_owned();
    fixture.manifests.fail_next_save();
    assert!(fixture
        .state
        .reinstall_executor
        .commit(prepared, &token)
        .is_err());
    assert_eq!(snapshot_file_tree(&fixture.game), before);
    assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
    assert_no_reinstall_recovery_transactions(&fixture.app_data);
    assert_no_retarget_staging(&fixture.app_data);
}

#[test]
fn batch_reapply_seals_the_intent_and_skips_noop_game_writes() {
    use hmm_core::{
        BatchExecutionPolicy, BatchItemInput, BatchOperation, BatchPlanRequest,
        ReinstallBatchItemInput, ReinstallIntent, BATCH_PLAN_SCHEMA_VERSION,
    };
    for changed in [false, true] {
        let fixture = Fixture::with_layout(false, true, true);
        if changed {
            move_to_old_layout(&fixture);
        }
        let before = snapshot_file_tree(&fixture.game);
        let manifest_bytes =
            fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap();
        let manifest = read_fixture_manifest(&fixture.app_data);
        let revision = manifest.entries[0].revision_id.clone().unwrap();
        let request = crate::BatchLifecyclePlanRequest {
            plan: BatchPlanRequest {
                schema_version: BATCH_PLAN_SCHEMA_VERSION,
                operation: BatchOperation::Reinstall,
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                execution_policy: BatchExecutionPolicy::StopOnFailure,
                items: vec![BatchItemInput::Reinstall(ReinstallBatchItemInput {
                    intent: ReinstallIntent::ReapplyEquipmentTargets,
                    mod_id: fixture.mod_id.clone(),
                    installed_revision_id: revision.clone(),
                    candidate_revision_id: revision,
                    layer: FileLayer::new("caller-layer-must-not-change-files", 123),
                    replacement_binding_snapshot: None,
                })],
            },
            replacement_targets: std::collections::BTreeMap::new(),
        };
        let environment = crate::RuntimeEnvironment::sandbox(fixture.app_data.clone()).unwrap();
        let preview =
            crate::BatchLifecycleAutomation::preview_request(&environment, request.clone())
                .unwrap();
        assert_eq!(preview.plan.status(), hmm_core::BatchPlanStatus::Ready);
        let normalized = hmm_core::NormalizedBatchPlanRequest {
            schema_version: BATCH_PLAN_SCHEMA_VERSION,
            operation: preview.plan.operation,
            game_id: preview.plan.game_id.clone(),
            profile_id: preview.plan.profile_id.clone(),
            execution_policy: preview.plan.execution_policy,
            items: preview
                .plan
                .items
                .iter()
                .map(|item| item.input_snapshot.clone())
                .collect(),
        };
        let fresh = crate::ReadOnlyInstallAutomation::from_environment(&environment)
            .unwrap()
            .read_batch_reinstall_facts(&normalized, preview.plan.environment_digest.clone())
            .unwrap();
        assert_eq!(
            fresh.items[0].warning_codes, preview.plan.items[0].warning_codes,
            "warning ordering must agree with execution facts"
        );
        assert_eq!(
            fresh.items[0].target_claims, preview.plan.items[0].target_claims,
            "target ordering must agree with execution facts"
        );
        assert_eq!(
            fresh.items[0].fact_digest,
            preview.plan.items[0].fact_digest
        );
        assert_eq!(
            preview.plan.items[0]
                .warning_codes
                .iter()
                .any(|code| code == "reapply_no_changes"),
            !changed
        );
        let mut tampered = preview.plan.clone();
        let BatchItemInput::Reinstall(input) = &mut tampered.items[0].input_snapshot else {
            panic!("reinstall input")
        };
        assert_eq!(input.layer, manifest.entries[0].layer);
        input.intent = ReinstallIntent::Standard;
        assert!(
            tampered.validate_integrity().is_err(),
            "changing intent invalidates the sealed batch facts"
        );
        let mut injected = request.clone();
        injected
            .replacement_targets
            .insert(fixture.mod_id.clone(), target("one002", "wp/one"));
        assert!(crate::BatchLifecycleAutomation::preview_request(&environment, injected).is_err());
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
            Arc::clone(&database),
        )
        .unwrap();
        let snapshot = crate::BatchLifecycleAutomation::result_with_database(
            &environment,
            &sealed.batch_id,
            result.attempt_number,
            database,
        )
        .unwrap();
        assert_eq!(
            result.status,
            hmm_core::BatchAttemptStatus::Completed,
            "changed={changed}, result={snapshot:?}"
        );
        assert_eq!(fs::read(fixture.game.join(PLUGIN)).unwrap(), PLUGIN_BYTES);
        assert_eq!(fs::read(fixture.game.join(TOOL)).unwrap(), TOOL_BYTES);
        if changed {
            assert_eq!(
                fs::read(fixture.game.join(MODEL)).unwrap(),
                EQUIPMENT_FILES[0].1
            );
            assert!(!fixture.game.join(OLD_MODEL).exists());
        } else {
            assert_eq!(snapshot_file_tree(&fixture.game), before);
            assert_eq!(
                fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap(),
                manifest_bytes
            );
        }
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
        assert_no_retarget_staging(&fixture.app_data);
    }
}
