use super::*;
use hmm_app::{
    EquipmentRetargetReinstallRequest, InitialRetargetSlotIntent,
    PreviewEquipmentRetargetReinstallRequest, ReplacementWorkflowError,
    StartEquipmentRetargetReinstallTaskRequest,
};
use hmm_games_mhw::MhwReplacementCatalog;

#[path = "runtime_equipment_attachment_tests.rs"]
mod attachments;
#[path = "runtime_equipment_canonical_tests.rs"]
mod canonical;
#[path = "runtime_equipment_composition_tests.rs"]
mod composition;
#[path = "runtime_equipment_origin_tests.rs"]
mod origin;
#[path = "runtime_equipment_path_mapping_tests.rs"]
mod paths;

const EQUIPMENT_FILES: &[(&str, &[u8])] = &[
    (
        "nativePC/wp/one/one001/mod/one001.mod3",
        b"synthetic model, deliberately not a binary parser fixture",
    ),
    (
        "nativePC/wp/one/one001/mod/one001.mrl3",
        b"synthetic unchanged references: wp/one/one001/mod/one001_BML",
    ),
    (
        "nativePC/wp/one/one001/mod/one001_BML.tex",
        b"synthetic texture",
    ),
    (
        "nativePC/wp/one/one001/mod/custom.mod3",
        b"unmapped model kept intact",
    ),
    (
        "nativePC/wp/one/one001/mod/ya001.mod3",
        b"unpaired accessory",
    ),
    (ARMOR_SOURCE_TARGET, ARMOR_FIXTURE_BYTES),
    (
        "nativePC/pl/f_equip/pl999_0000/arm/mod/f_custom.mod3",
        b"unknown catalog source",
    ),
    (
        "nativePC/sound/author/resources.bin",
        b"companion outside equipment trees",
    ),
];

fn target(internal: &str, family: &str) -> ReplacementTargetId {
    MhwReplacementCatalog
        .replacement_catalog()
        .unwrap()
        .targets()
        .iter()
        .find(|target| {
            target.internal_id() == internal
                && target
                    .metadata()
                    .get("path_family")
                    .and_then(|value| value.as_str())
                    == Some(family)
        })
        .unwrap_or_else(|| panic!("missing synthetic fixture target {internal}"))
        .id()
        .clone()
}

fn import_equipment(state: &HmmRuntime, archive: PathBuf) -> ModId {
    let task = state
        .mod_import_tasks
        .start_import_mod_task(StartImportModTaskRequest {
            archive_path: archive.clone(),
        })
        .unwrap();
    state
        .mod_import_task_runner
        .run_prepare_task(&task.task_id, archive)
        .unwrap();
    ModId::new(task.task_id)
}

fn selection(
    state: &HmmRuntime,
    mod_id: &ModId,
    weapon: &str,
    armor: Option<&str>,
) -> EquipmentRetargetReinstallRequest {
    let profile_id = ProfileId::new("default");
    let config = state
        .replacement_workflow
        .equipment_configuration(&GameId::mhw(), mod_id, Some(&profile_id))
        .unwrap();
    let slots = config
        .sources
        .iter()
        .map(|item| {
            let target_id = match item.source.internal_id() {
                "one001" => Some(target(weapon, "wp/one")),
                "pl121_0000" => armor.map(|slot| target(slot, "pl/f_equip")),
                _ => None,
            };
            match target_id {
                Some(target_id) => InitialRetargetSlotIntent::Retarget {
                    source_id: item.source.id().clone(),
                    target_id,
                },
                None => InitialRetargetSlotIntent::KeepInPlace {
                    source_id: item.source.id().clone(),
                },
            }
        })
        .collect();
    EquipmentRetargetReinstallRequest {
        intent: Default::default(),
        game_id: GameId::mhw(),
        profile_id,
        mod_id: mod_id.clone(),
        slots,
        layer: FileLayer::new("base", 0),
    }
}

fn initial_request(
    selection: EquipmentRetargetReinstallRequest,
) -> PreviewInitialRetargetInstallRequest {
    PreviewInitialRetargetInstallRequest {
        game_id: selection.game_id,
        profile_id: selection.profile_id,
        mod_id: selection.mod_id,
        selection: InitialRetargetSelection::PerSlot(selection.slots),
        layer: selection.layer,
    }
}

fn install_equipment(state: &HmmRuntime, selection: EquipmentRetargetReinstallRequest) {
    let request = initial_request(selection);
    let task = state
        .retarget_install_tasks
        .start_equipment_retarget_install_task(request.clone())
        .unwrap();
    let events = state
        .retarget_install_task_runner
        .run_equipment_retarget_install_task(&task.task_id, request)
        .unwrap();
    assert_eq!(events.last().unwrap().status, TaskStatus::Completed);
}

#[test]
fn mixed_equipment_installs_switches_after_restart_and_uninstalls_to_the_exact_baseline() {
    let temp = tempfile::tempdir().unwrap();
    let app_data = temp.path().join("app-data");
    let game = temp.path().join("game");
    prepare_game_root(&game);
    for (path, bytes) in [
        (
            "nativePC/wp/one/one002/mod/one002.mod3",
            b"original first target".as_slice(),
        ),
        (
            "nativePC/wp/one/one004/mod/one004.mod3",
            b"original second target".as_slice(),
        ),
        (ARMOR_RETARGETED_TARGET, b"original armor".as_slice()),
    ] {
        let path = game.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    let baseline = snapshot_file_tree(&game);
    let archive = temp.path().join("equipment.zip");
    create_fixture_zip(&archive, EQUIPMENT_FILES);
    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game.clone())
        .unwrap();
    let mod_id = import_equipment(&state, archive);
    let initial = selection(&state, &mod_id, "one002", None);
    assert_eq!(
        initial.slots.len(),
        3,
        "known armor, unknown armor, and weapon are separate sources"
    );
    let preview = state
        .initial_retarget_install_preflight
        .preview(initial_request(initial.clone()))
        .unwrap();
    assert_eq!(
        preview.planned.install_plan().actions.len(),
        EQUIPMENT_FILES.len()
    );
    assert!(!preview.planned.install_plan().has_blocking_conflicts());
    install_equipment(&state, initial);
    let manifest = read_fixture_manifest(&app_data);
    assert_eq!(manifest.replacement_bindings.len(), 3);
    assert_eq!(manifest.entries.len(), EQUIPMENT_FILES.len());
    let mut installed = baseline.clone();
    for (source, bytes) in EQUIPMENT_FILES {
        let destination = match *source {
            "nativePC/wp/one/one001/mod/one001.mod3" => "nativePC/wp/one/one002/mod/one002.mod3",
            "nativePC/wp/one/one001/mod/one001.mrl3" => "nativePC/wp/one/one002/mod/one002.mrl3",
            "nativePC/wp/one/one001/mod/ya001.mod3" => "nativePC/wp/one/one002/mod/ya002.mod3",
            _ => source,
        };
        installed.insert(destination.to_owned(), bytes.to_vec());
    }
    assert_eq!(
        snapshot_file_tree(&game),
        installed,
        "every installed byte and retained path must match"
    );
    assert_no_retarget_staging(&app_data);
    drop(state);

    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
    let switch = selection(&state, &mod_id, "one004", Some("pl129_0000"));
    let context = state
        .replacement_workflow
        .equipment_configuration(&GameId::mhw(), &mod_id, Some(&ProfileId::new("default")))
        .unwrap();
    assert_eq!(
        context.installed_targets.unwrap().len(),
        3,
        "unknown names do not invalidate installed facts"
    );
    let preview = state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(switch.clone())
        .unwrap();
    assert_eq!(
        preview.status,
        ReinstallPreviewStatus::Ready,
        "{:?}",
        preview.blocking_reasons
    );
    assert_eq!(
        snapshot_file_tree(&game),
        installed,
        "preview must not write the game"
    );
    assert_no_retarget_staging(&app_data);
    let request = StartEquipmentRetargetReinstallTaskRequest {
        selection: switch,
        plan_token: preview.plan_token.unwrap(),
    };
    let task = state
        .reinstall_tasks
        .start_equipment_retarget_reinstall_task(request.clone())
        .unwrap();
    let events = state
        .reinstall_task_runner
        .run_equipment_retarget_reinstall_task(&task.task_id, request)
        .unwrap();
    assert_eq!(events.last().unwrap().status, TaskStatus::Completed);
    let mut switched = baseline.clone();
    for (source, bytes) in EQUIPMENT_FILES {
        let destination = match *source {
            "nativePC/wp/one/one001/mod/one001.mod3" => "nativePC/wp/one/one004/mod/one004.mod3",
            "nativePC/wp/one/one001/mod/one001.mrl3" => "nativePC/wp/one/one004/mod/one004.mrl3",
            "nativePC/wp/one/one001/mod/ya001.mod3" => "nativePC/wp/one/one004/mod/ya004.mod3",
            ARMOR_SOURCE_TARGET => ARMOR_RETARGETED_TARGET,
            _ => source,
        };
        switched.insert(destination.to_owned(), bytes.to_vec());
    }
    assert_eq!(snapshot_file_tree(&game), switched);
    let switched_manifest = read_fixture_manifest(&app_data);
    assert_eq!(switched_manifest.entries.len(), EQUIPMENT_FILES.len());
    assert_eq!(switched_manifest.replacement_bindings.len(), 3);
    for before in &manifest.replacement_bindings {
        let after = switched_manifest
            .replacement_bindings
            .iter()
            .find(|binding| binding.binding().source_id() == before.binding().source_id())
            .unwrap();
        assert_eq!(before.binding_id(), after.binding_id());
        assert_eq!(before.revision_id(), after.revision_id());
    }
    assert_no_retarget_staging(&app_data);
    assert_no_reinstall_recovery_transactions(&app_data);
    let uninstall = StartUninstallTaskRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        mod_id,
    };
    let task = state
        .uninstall_tasks
        .start_uninstall_task(uninstall.clone())
        .unwrap();
    state
        .uninstall_task_runner
        .run_uninstall_task(&task.task_id, uninstall)
        .unwrap();
    assert_eq!(
        snapshot_file_tree(&game),
        baseline,
        "uninstall must restore both overwritten targets and unknown files"
    );
    assert_no_recovery_records(&app_data);
}

#[test]
fn equipment_switch_rejects_incomplete_duplicate_and_stale_selections_without_game_writes() {
    let temp = tempfile::tempdir().unwrap();
    let app_data = temp.path().join("app-data");
    let game = temp.path().join("game");
    prepare_game_root(&game);
    let archive = temp.path().join("equipment.zip");
    create_fixture_zip(&archive, EQUIPMENT_FILES);
    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game.clone())
        .unwrap();
    let mod_id = import_equipment(&state, archive);
    install_equipment(&state, selection(&state, &mod_id, "one002", None));
    let before = snapshot_file_tree(&game);
    let manifest = read_fixture_manifest(&app_data);
    let valid = selection(&state, &mod_id, "one004", None);
    let preview_request = |selection| PreviewEquipmentRetargetReinstallRequest {
        selection,
        installed_revision_id: manifest.entries[0].revision_id.clone().unwrap(),
        installed_bindings: manifest.replacement_bindings.clone(),
    };
    let mut incomplete = valid.clone();
    incomplete.slots.pop();
    assert_eq!(
        state
            .replacement_workflow
            .preview_equipment_reinstall(preview_request(incomplete))
            .unwrap_err(),
        ReplacementWorkflowError::SourceNotRetargetable
    );
    let mut duplicate = valid.clone();
    duplicate.slots[1] = duplicate.slots[0].clone();
    assert_eq!(
        state
            .replacement_workflow
            .preview_equipment_reinstall(preview_request(duplicate))
            .unwrap_err(),
        ReplacementWorkflowError::DuplicateSlotIntent
    );
    let unchanged = selection(&state, &mod_id, "one002", None);
    assert_eq!(
        state
            .replacement_workflow
            .preview_equipment_reinstall(preview_request(unchanged))
            .unwrap_err(),
        ReplacementWorkflowError::TargetAlreadySelected
    );
    let preview = state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(valid.clone())
        .unwrap();
    assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
    let mut stale = valid;
    if let Some(InitialRetargetSlotIntent::Retarget { target_id, .. }) = stale
        .slots
        .iter_mut()
        .find(|slot| matches!(slot, InitialRetargetSlotIntent::Retarget { .. }))
    {
        *target_id = target("one005", "wp/one");
    }
    let request = StartEquipmentRetargetReinstallTaskRequest {
        selection: stale,
        plan_token: preview.plan_token.unwrap(),
    };
    let task = state
        .reinstall_tasks
        .start_equipment_retarget_reinstall_task(request.clone())
        .unwrap();
    let error = state
        .reinstall_task_runner
        .run_equipment_retarget_reinstall_task(&task.task_id, request)
        .unwrap_err();
    assert_eq!(
        error.events.last().unwrap().error.as_deref(),
        Some("install_reinstall_failed:preflight")
    );
    assert!(!error
        .events
        .iter()
        .any(|event| event.phase == "install.reinstall.rollback.processing"));
    assert_eq!(snapshot_file_tree(&game), before);
    assert_eq!(read_fixture_manifest(&app_data), manifest);
    assert_no_retarget_staging(&app_data);
    assert_no_recovery_records(&app_data);
    assert_no_reinstall_recovery_transactions(&app_data);
}

#[test]
fn equipment_switch_rolls_back_every_source_when_the_final_manifest_cannot_be_saved() {
    let temp = tempfile::tempdir().unwrap();
    let app_data = temp.path().join("app-data");
    let game = temp.path().join("game");
    prepare_game_root(&game);
    let archive = temp.path().join("equipment.zip");
    create_fixture_zip(&archive, EQUIPMENT_FILES);
    let manifests = Arc::new(FailNextManifestSaveRepository::new(
        app_data.join("install/manifests"),
    ));
    let state = HmmRuntime::builder(app_data.clone())
        .with_install_manifest_repository(manifests.clone())
        .build()
        .unwrap();
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game.clone())
        .unwrap();
    let mod_id = import_equipment(&state, archive);
    install_equipment(&state, selection(&state, &mod_id, "one002", None));
    let before = snapshot_file_tree(&game);
    let manifest = read_fixture_manifest(&app_data);
    let selection = selection(&state, &mod_id, "one004", Some("pl129_0000"));
    let preview = state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(selection.clone())
        .unwrap();
    assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
    let request = StartEquipmentRetargetReinstallTaskRequest {
        selection,
        plan_token: preview.plan_token.unwrap(),
    };
    let task = state
        .reinstall_tasks
        .start_equipment_retarget_reinstall_task(request.clone())
        .unwrap();
    manifests.fail_next_save();
    let failure = state
        .reinstall_task_runner
        .run_equipment_retarget_reinstall_task(&task.task_id, request)
        .unwrap_err();
    assert_eq!(
        failure.events.last().unwrap().error.as_deref(),
        Some("install_reinstall_failed:manifest")
    );
    assert!(failure
        .events
        .iter()
        .any(|event| event.phase == "install.reinstall.rollback.processing"));
    assert_eq!(snapshot_file_tree(&game), before);
    assert_eq!(read_fixture_manifest(&app_data), manifest);
    assert_no_retarget_staging(&app_data);
    assert_no_reinstall_recovery_transactions(&app_data);
}

#[test]
fn equipment_initial_install_and_switch_keep_cross_mod_conflicts_after_staging() {
    let temp = tempfile::tempdir().unwrap();
    let app_data = temp.path().join("app-data");
    let game = temp.path().join("game");
    prepare_game_root(&game);
    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game.clone())
        .unwrap();
    let other_archive = temp.path().join("other.zip");
    create_fixture_zip(
        &other_archive,
        &[(
            "nativePC/wp/one/one004/mod/one004.mod3",
            b"another Mod owns this target",
        )],
    );
    let other = import_equipment(&state, other_archive);
    install_fixture_revision(&state, &other, &ProfileId::new("default"));
    let archive = temp.path().join("equipment.zip");
    create_fixture_zip(&archive, EQUIPMENT_FILES);
    let mod_id = import_equipment(&state, archive);
    let blocked = initial_request(selection(&state, &mod_id, "one004", None));
    let before = snapshot_file_tree(&game);
    let preview = state
        .initial_retarget_install_preflight
        .preview(blocked.clone())
        .unwrap();
    assert!(preview.planned.install_plan().has_blocking_conflicts());
    let task = state
        .retarget_install_tasks
        .start_equipment_retarget_install_task(blocked.clone())
        .unwrap();
    let error = state
        .retarget_install_task_runner
        .run_equipment_retarget_install_task(&task.task_id, blocked)
        .unwrap_err();
    assert_eq!(
        error.events.last().unwrap().error.as_deref(),
        Some("install_retarget_failed:planning")
    );
    assert_eq!(snapshot_file_tree(&game), before);
    assert_no_retarget_staging(&app_data);
    install_equipment(&state, selection(&state, &mod_id, "one002", None));
    let before = snapshot_file_tree(&game);
    let preview = state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(selection(&state, &mod_id, "one004", None))
        .unwrap();
    assert_eq!(preview.status, ReinstallPreviewStatus::Blocked);
    assert_eq!(snapshot_file_tree(&game), before);
    assert_no_retarget_staging(&app_data);
}
