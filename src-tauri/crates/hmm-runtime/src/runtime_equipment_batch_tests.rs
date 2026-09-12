use super::*;
use crate::lifecycle_automation::write_install_fixture;
use hmm_core::{
    BatchExecutionPolicy, BatchItemInput, BatchPlanRequest, FileLayer, GameId,
    InstallBatchItemInput, InstallManifest, ModId, ModRevisionId, ProfileId,
    BATCH_PLAN_SCHEMA_VERSION,
};
use std::collections::BTreeMap;

const EXTRA_FILES: &[(&str, &[u8])] = &[
    ("nativePC/wp/one/one001/mod/one001.mod3", b"batch weapon"),
    (
        "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
        b"batch armor",
    ),
    (
        "nativePC/plugins/fixture_support.dll",
        b"inert batch plugin",
    ),
];

fn fixture() -> (tempfile::TempDir, RuntimeEnvironment, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let game = write_install_fixture(temp.path());
    for (relative, bytes) in EXTRA_FILES {
        let file = temp
            .path()
            .join("mod-import/sandboxes/package-a")
            .join(relative);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, bytes).unwrap();
    }
    let environment = RuntimeEnvironment::sandbox(temp.path().to_path_buf()).unwrap();
    (temp, environment, game)
}

fn request() -> BatchLifecyclePlanRequest {
    BatchLifecyclePlanRequest {
        plan: BatchPlanRequest {
            schema_version: BATCH_PLAN_SCHEMA_VERSION,
            operation: BatchOperation::Install,
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            execution_policy: BatchExecutionPolicy::StopOnFailure,
            items: vec![BatchItemInput::Install(InstallBatchItemInput {
                mod_id: ModId::new("mod-a"),
                revision_id: ModRevisionId::new("package-a"),
                layer: FileLayer::new("base", 0),
                replacement_binding_snapshot: None,
            })],
        },
        replacement_targets: BTreeMap::new(),
    }
}

#[test]
fn multi_source_batch_preview_keeps_complete_facts_and_normal_file_policy() {
    let (temp, environment, game) = fixture();
    let read_only = ReadOnlyInstallAutomation::from_environment(&environment).unwrap();
    let (_, _, mod_id, revision, plan, prerequisite) = read_only
        .build_install_plan_for_revision(
            "mhw",
            "default",
            "mod-a",
            "package-a",
            &FileLayer::new("base", 0),
        )
        .unwrap();
    assert_eq!(plan.actions.len(), 4);
    assert_eq!(plan.replacement_bindings.len(), 2);
    let complete = facts_for_install(&mod_id, &revision, &plan, &prerequisite);
    let mut incomplete = plan;
    incomplete.replacement_bindings.pop();
    assert_ne!(
        complete.fact_digest,
        facts_for_install(&mod_id, &revision, &incomplete, &prerequisite).fact_digest
    );
    let preview = BatchLifecycleAutomation::preview_request(&environment, request()).unwrap();
    assert_eq!(preview.plan.status(), hmm_core::BatchPlanStatus::Ready);
    let BatchItemInput::Install(input) = &preview.plan.items[0].input_snapshot else {
        panic!("install input")
    };
    assert!(
        input.replacement_binding_snapshot.is_none(),
        "one source must not represent the entire set"
    );
    BatchLifecycleAutomation::seal_request(
        &environment,
        request(),
        preview.preview_token.as_deref().unwrap(),
    )
    .unwrap();
    assert!(!temp.path().join("install/manifests/default.json").exists());
    for (path, _) in EXTRA_FILES {
        assert!(!game.join(path).exists());
    }
}

#[test]
fn multi_source_batch_preview_rejects_changed_source_facts_before_sealing() {
    let (temp, environment, game) = fixture();
    let preview = BatchLifecycleAutomation::preview_request(&environment, request()).unwrap();
    let file = temp
        .path()
        .join("mod-import/sandboxes/package-a/nativePC/wp/one/one004/mod/one004.mod3");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, b"new source").unwrap();
    let error = BatchLifecycleAutomation::seal_request(
        &environment,
        request(),
        preview.preview_token.as_deref().unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.code(), "batch_plan_stale");
    assert!(!temp.path().join("install/manifests/default.json").exists());
    for (path, _) in EXTRA_FILES {
        assert!(!game.join(path).exists());
    }
}

#[test]
fn multi_source_batch_install_records_all_sources_and_can_switch_after_restart() {
    let (temp, environment, game) = fixture();
    let preview = BatchLifecycleAutomation::preview_request(&environment, request()).unwrap();
    let (_, sealed) = BatchLifecycleAutomation::seal_request(
        &environment,
        request(),
        preview.preview_token.as_deref().unwrap(),
    )
    .unwrap();
    let (_, run) =
        BatchLifecycleAutomation::start_request(&environment, &sealed.batch_id, &sealed.plan_token)
            .unwrap();
    assert_eq!(run.status, BatchAttemptStatus::Completed);
    let manifest: InstallManifest = serde_json::from_slice(
        &fs::read(temp.path().join("install/manifests/default.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest.entries.len(), 4);
    assert_eq!(manifest.replacement_bindings.len(), 2);
    for (path, bytes) in EXTRA_FILES {
        assert_eq!(fs::read(game.join(path)).unwrap(), *bytes);
    }

    let state = HmmRuntime::from_app_data_dir(temp.path().to_path_buf()).unwrap();
    let mod_id = ModId::new("mod-a");
    let profile = ProfileId::new("default");
    let configuration = state
        .replacement_workflow
        .equipment_configuration(&GameId::mhw(), &mod_id, Some(&profile))
        .unwrap();
    assert_eq!(configuration.installed_targets.unwrap().len(), 2);
    let slots = configuration
        .sources
        .iter()
        .map(|item| {
            if item.source.internal_id() == "one001" {
                hmm_app::InitialRetargetSlotIntent::Retarget {
                    source_id: item.source.id().clone(),
                    target_id: item
                        .targets
                        .iter()
                        .find(|target| target.internal_id() == "one002")
                        .unwrap()
                        .id()
                        .clone(),
                }
            } else {
                hmm_app::InitialRetargetSlotIntent::KeepInPlace {
                    source_id: item.source.id().clone(),
                }
            }
        })
        .collect();
    let selection = hmm_app::EquipmentRetargetReinstallRequest {
        game_id: GameId::mhw(),
        profile_id: profile,
        mod_id,
        slots,
        layer: FileLayer::new("base", 0),
    };
    let preview = state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(selection.clone())
        .unwrap();
    assert_eq!(preview.status, hmm_app::ReinstallPreviewStatus::Ready);
    let request = hmm_app::StartEquipmentRetargetReinstallTaskRequest {
        selection,
        plan_token: preview.plan_token.unwrap(),
    };
    let task = state
        .reinstall_tasks
        .start_equipment_retarget_reinstall_task(request.clone())
        .unwrap();
    state
        .reinstall_task_runner
        .run_equipment_retarget_reinstall_task(&task.task_id, request)
        .unwrap();
    assert_eq!(
        fs::read(game.join("nativePC/wp/one/one002/mod/one002.mod3")).unwrap(),
        b"batch weapon"
    );
    assert_eq!(
        fs::read(game.join(EXTRA_FILES[1].0)).unwrap(),
        EXTRA_FILES[1].1
    );
    assert_eq!(
        fs::read(game.join(EXTRA_FILES[2].0)).unwrap(),
        EXTRA_FILES[2].1
    );
    assert_eq!(
        fs::read(game.join("nativePC/models/player.mod3")).unwrap(),
        b"fixture"
    );
}
