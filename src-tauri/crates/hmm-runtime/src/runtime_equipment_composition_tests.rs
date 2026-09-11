use super::*;

struct CompositionCase {
    sources: [&'static str; 2],
    shared_target: &'static str,
    split_target: &'static str,
    files: [(&'static str, &'static [u8]); 4],
    shared_paths: [&'static str; 4],
    split_paths: [&'static str; 4],
}

fn source_targets(
    state: &HmmRuntime,
    mod_id: &ModId,
    choices: &[(&str, &str)],
) -> EquipmentRetargetReinstallRequest {
    let profile_id = ProfileId::new("default");
    let configuration = state
        .replacement_workflow
        .equipment_configuration(&GameId::mhw(), mod_id, Some(&profile_id))
        .unwrap();
    assert_eq!(configuration.sources.len(), choices.len());
    let slots = configuration
        .sources
        .iter()
        .map(|item| {
            let (_, destination) = choices
                .iter()
                .find(|(source, _)| *source == item.source.internal_id())
                .expect("every fixture source has an explicit target");
            InitialRetargetSlotIntent::Retarget {
                source_id: item.source.id().clone(),
                target_id: target(destination, item.source.path_family()),
            }
        })
        .collect();
    EquipmentRetargetReinstallRequest {
        game_id: GameId::mhw(),
        profile_id,
        mod_id: mod_id.clone(),
        slots,
        layer: FileLayer::new("base", 0),
    }
}

fn assert_composition_roundtrip(case: CompositionCase) {
    let temp = tempfile::tempdir().unwrap();
    let app_data = temp.path().join("app-data");
    let game = temp.path().join("game");
    prepare_game_root(&game);
    for path in [case.shared_paths[0], case.split_paths[0]] {
        let path = game.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"original target contents").unwrap();
    }
    let baseline = snapshot_file_tree(&game);
    let companion = (
        "nativePC/sound/composition.bin",
        b"shared companion".as_slice(),
    );
    let mut files = case.files.to_vec();
    files.push(companion);
    let archive = temp.path().join("composition.zip");
    create_fixture_zip(&archive, &files);
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
    let shared = source_targets(
        &state,
        &mod_id,
        &case.sources.map(|source| (source, case.shared_target)),
    );
    let preview = state
        .initial_retarget_install_preflight
        .preview(initial_request(shared.clone()))
        .expect("disjoint parts may share an equipment target");
    assert_eq!(preview.planned.install_plan().actions.len(), 5);
    assert!(!preview.planned.install_plan().has_blocking_conflicts());
    assert_eq!(preview.planned.install_plan().replacement_bindings.len(), 2);
    install_equipment(&state, shared);
    let manifest = read_fixture_manifest(&app_data);
    assert_eq!(manifest.replacement_bindings.len(), 2);
    assert_eq!(manifest.entries.len(), 5);
    assert!(manifest
        .replacement_bindings
        .iter()
        .all(|binding| binding.target_internal_id() == case.shared_target));
    let expected = |paths: [&str; 4]| {
        let mut result = baseline.clone();
        for (path, (_, bytes)) in paths.into_iter().zip(case.files) {
            result.insert(path.to_owned(), bytes.to_vec());
        }
        result.insert(companion.0.to_owned(), companion.1.to_vec());
        result
    };
    assert_eq!(snapshot_file_tree(&game), expected(case.shared_paths));
    let summary = state
        .replacement_workflow
        .replacement_summary(
            hmm_app::AnalyzeImportedReplacementRequest {
                game_id: GameId::mhw(),
                mod_id: mod_id.clone(),
            },
            Some(&ProfileId::new("default")),
        )
        .unwrap();
    assert_eq!(summary.sources.len(), 2);
    assert_eq!(summary.installed_targets.unwrap().len(), 1);
    drop(state);

    let state = HmmRuntime::builder(app_data.clone())
        .with_install_manifest_repository(manifests.clone())
        .build()
        .unwrap();
    let split = source_targets(
        &state,
        &mod_id,
        &[
            (case.sources[0], case.split_target),
            (case.sources[1], case.shared_target),
        ],
    );
    let preview = state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(split.clone())
        .unwrap();
    assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
    assert_eq!(snapshot_file_tree(&game), expected(case.shared_paths));
    let request = StartEquipmentRetargetReinstallTaskRequest {
        selection: split,
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
    assert_eq!(snapshot_file_tree(&game), expected(case.split_paths));
    let after = read_fixture_manifest(&app_data);
    assert_eq!(after.replacement_bindings.len(), 2);
    for before in &manifest.replacement_bindings {
        let current = after
            .replacement_bindings
            .iter()
            .find(|binding| binding.binding().source_id() == before.binding().source_id())
            .unwrap();
        assert_eq!(before.binding_id(), current.binding_id());
        assert_eq!(before.revision_id(), current.revision_id());
    }

    // 再次合并验证重装入口也允许重复目标，并且清单失败能完整恢复拆分后的布局。
    let shared = source_targets(
        &state,
        &mod_id,
        &case.sources.map(|source| (source, case.shared_target)),
    );
    for fail_manifest in [true, false] {
        let preview = state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(shared.clone())
            .unwrap();
        assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
        let request = StartEquipmentRetargetReinstallTaskRequest {
            selection: shared.clone(),
            plan_token: preview.plan_token.unwrap(),
        };
        let task = state
            .reinstall_tasks
            .start_equipment_retarget_reinstall_task(request.clone())
            .unwrap();
        if fail_manifest {
            manifests.fail_next_save();
        }
        let result = state
            .reinstall_task_runner
            .run_equipment_retarget_reinstall_task(&task.task_id, request);
        if fail_manifest {
            let error = result.unwrap_err();
            assert_eq!(
                error.events.last().unwrap().error.as_deref(),
                Some("install_reinstall_failed:manifest")
            );
            assert_eq!(snapshot_file_tree(&game), expected(case.split_paths));
            assert_eq!(read_fixture_manifest(&app_data), after);
        } else {
            assert_eq!(
                result.unwrap().last().unwrap().status,
                TaskStatus::Completed
            );
            assert_eq!(snapshot_file_tree(&game), expected(case.shared_paths));
        }
        assert_no_retarget_staging(&app_data);
        assert_no_reinstall_recovery_transactions(&app_data);
    }
    let request = StartUninstallTaskRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        mod_id,
    };
    let task = state
        .uninstall_tasks
        .start_uninstall_task(request.clone())
        .unwrap();
    state
        .uninstall_task_runner
        .run_uninstall_task(&task.task_id, request)
        .unwrap();
    assert_eq!(snapshot_file_tree(&game), baseline);
    assert_no_retarget_staging(&app_data);
    assert_no_recovery_records(&app_data);
    assert_no_reinstall_recovery_transactions(&app_data);
}

#[test]
fn blade_and_sheath_can_share_a_target_then_move_one_source_after_restart() {
    assert_composition_roundtrip(CompositionCase {
        sources: ["swo035", "swo019"],
        shared_target: "swo001",
        split_target: "swo002",
        files: [
            ("nativePC/wp/swo/swo035/mod/swo035.mod3", b"blade model"),
            ("nativePC/wp/swo/swo035/mod/swo035.mrl3", b"blade material"),
            ("nativePC/wp/swo/swo019/mod/saya019.mod3", b"sheath model"),
            (
                "nativePC/wp/swo/swo019/mod/saya019.mrl3",
                b"sheath material",
            ),
        ],
        shared_paths: [
            "nativePC/wp/swo/swo001/mod/swo001.mod3",
            "nativePC/wp/swo/swo001/mod/swo001.mrl3",
            "nativePC/wp/swo/swo001/mod/saya001.mod3",
            "nativePC/wp/swo/swo001/mod/saya001.mrl3",
        ],
        split_paths: [
            "nativePC/wp/swo/swo002/mod/swo002.mod3",
            "nativePC/wp/swo/swo002/mod/swo002.mrl3",
            "nativePC/wp/swo/swo001/mod/saya001.mod3",
            "nativePC/wp/swo/swo001/mod/saya001.mrl3",
        ],
    });
}

#[test]
fn body_and_head_can_share_a_target_then_move_one_source_after_restart() {
    assert_composition_roundtrip(CompositionCase {
        sources: ["pl078_0000", "pl121_0000"],
        shared_target: "pl129_0000",
        split_target: "pl129_0010",
        files: [
            (
                "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3",
                b"body model",
            ),
            (
                "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mrl3",
                b"body material",
            ),
            (
                "nativePC/pl/f_equip/pl121_0000/helm/mod/f_helm121_0000.mod3",
                b"head model",
            ),
            (
                "nativePC/pl/f_equip/pl121_0000/helm/mod/f_helm121_0000.mrl3",
                b"head material",
            ),
        ],
        shared_paths: [
            "nativePC/pl/f_equip/pl129_0000/body/mod/f_body129_0000.mod3",
            "nativePC/pl/f_equip/pl129_0000/body/mod/f_body129_0000.mrl3",
            "nativePC/pl/f_equip/pl129_0000/helm/mod/f_helm129_0000.mod3",
            "nativePC/pl/f_equip/pl129_0000/helm/mod/f_helm129_0000.mrl3",
        ],
        split_paths: [
            "nativePC/pl/f_equip/pl129_0010/body/mod/f_body129_0010.mod3",
            "nativePC/pl/f_equip/pl129_0010/body/mod/f_body129_0010.mrl3",
            "nativePC/pl/f_equip/pl129_0000/helm/mod/f_helm129_0000.mod3",
            "nativePC/pl/f_equip/pl129_0000/helm/mod/f_helm129_0000.mrl3",
        ],
    });
}

#[test]
fn actual_same_file_collisions_are_visible_and_cannot_write_the_game() {
    for (files, choices) in [
        (
            [
                (
                    "nativePC/wp/one/one001/mod/one001.MOD3",
                    b"first".as_slice(),
                ),
                (
                    "nativePC/wp/one/one004/mod/one004.mod3",
                    b"second".as_slice(),
                ),
            ],
            [("one001", "one002"), ("one004", "one002")],
        ),
        (
            [
                (
                    "nativePC/pl/f_equip/pl078_0000/body/mod/f_é.mod3",
                    b"first".as_slice(),
                ),
                (
                    "nativePC/pl/f_equip/pl121_0000/body/mod/f_e\u{301}.mod3",
                    b"second".as_slice(),
                ),
            ],
            [("pl078_0000", "pl129_0000"), ("pl121_0000", "pl129_0000")],
        ),
        (
            [
                (
                    "nativePC/wp/one/one001/mod/one001.mod3",
                    b"first".as_slice(),
                ),
                (
                    "nativePC/wp/one/one004/mod/one004.mod3",
                    b"second".as_slice(),
                ),
            ],
            [("one001", "one002"), ("one004", "one002")],
        ),
        (
            [
                (
                    "nativePC/pl/f_equip/pl078_0000/body/mod/f_body.mod3",
                    b"first".as_slice(),
                ),
                (
                    "nativePC/pl/f_equip/pl121_0000/body/mod/f_body.mod3",
                    b"second".as_slice(),
                ),
            ],
            [("pl078_0000", "pl129_0000"), ("pl121_0000", "pl129_0000")],
        ),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        let game = temp.path().join("game");
        prepare_game_root(&game);
        let baseline = snapshot_file_tree(&game);
        let archive = temp.path().join("conflict.zip");
        create_fixture_zip(&archive, &files);
        let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
        state
            .game_setup
            .save_game_directory(GameId::mhw(), game.clone())
            .unwrap();
        let mod_id = import_equipment(&state, archive);
        let request = initial_request(source_targets(&state, &mod_id, &choices));
        let preview = state
            .initial_retarget_install_preflight
            .preview(request.clone())
            .unwrap();
        assert!(preview.planned.install_plan().has_blocking_conflicts());
        assert!(preview.planned.install_plan().actions.is_empty());
        assert_eq!(preview.planned.install_plan().replacement_bindings.len(), 2);
        let task = state
            .retarget_install_tasks
            .start_equipment_retarget_install_task(request.clone())
            .unwrap();
        let error = state
            .retarget_install_task_runner
            .run_equipment_retarget_install_task(&task.task_id, request)
            .unwrap_err();
        assert_eq!(
            error.events.last().unwrap().error.as_deref(),
            Some("install_retarget_failed:planning")
        );
        assert_eq!(snapshot_file_tree(&game), baseline);
        assert!(!app_data.join("install/manifests/default.json").exists());
        assert_no_retarget_staging(&app_data);
        assert_no_recovery_records(&app_data);

        let original_choices = choices.map(|(source, _)| (source, source));
        install_equipment(&state, source_targets(&state, &mod_id, &original_choices));
        let installed = snapshot_file_tree(&game);
        let manifest = read_fixture_manifest(&app_data);
        let blocked = state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(source_targets(&state, &mod_id, &choices))
            .expect("conflicting destinations must remain a reviewable preview");
        assert_eq!(blocked.status, ReinstallPreviewStatus::Blocked);
        assert!(blocked
            .blocking_reasons
            .iter()
            .any(|reason| reason.reason == hmm_app::ReinstallBlockingReason::PlanConflict));
        assert!(blocked.plan_token.is_none());
        assert_eq!(snapshot_file_tree(&game), installed);
        assert_eq!(read_fixture_manifest(&app_data), manifest);
        assert_no_retarget_staging(&app_data);
        assert_no_reinstall_recovery_transactions(&app_data);
    }
}

#[test]
fn separate_mods_can_share_equipment_without_claiming_each_others_files() {
    let temp = tempfile::tempdir().unwrap();
    let app_data = temp.path().join("app-data");
    let game = temp.path().join("game");
    prepare_game_root(&game);
    let baseline = snapshot_file_tree(&game);
    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game.clone())
        .unwrap();
    let mut mods = Vec::new();
    let mut expected = baseline.clone();
    for (name, source, source_path, target_path) in [
        (
            "blade",
            "swo035",
            "nativePC/wp/swo/swo035/mod/swo035.mod3",
            "nativePC/wp/swo/swo001/mod/swo001.mod3",
        ),
        (
            "sheath",
            "swo019",
            "nativePC/wp/swo/swo019/mod/saya019.mod3",
            "nativePC/wp/swo/swo001/mod/saya001.mod3",
        ),
    ] {
        let archive = temp.path().join(format!("{name}.zip"));
        create_fixture_zip(&archive, &[(source_path, name.as_bytes())]);
        let mod_id = import_equipment(&state, archive);
        install_equipment(
            &state,
            source_targets(&state, &mod_id, &[(source, "swo001")]),
        );
        mods.push((mod_id, target_path));
        expected.insert(target_path.to_owned(), name.as_bytes().to_vec());
        assert_eq!(snapshot_file_tree(&game), expected);
    }
    assert_eq!(
        read_fixture_manifest(&app_data).replacement_bindings.len(),
        2
    );
    for (mod_id, target_path) in mods {
        let request = StartUninstallTaskRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id,
        };
        let task = state
            .uninstall_tasks
            .start_uninstall_task(request.clone())
            .unwrap();
        state
            .uninstall_task_runner
            .run_uninstall_task(&task.task_id, request)
            .unwrap();
        expected.remove(target_path);
        assert_eq!(snapshot_file_tree(&game), expected);
    }
    assert_eq!(snapshot_file_tree(&game), baseline);
}

#[test]
fn another_mods_file_with_different_case_blocks_before_any_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let app_data = temp.path().join("app-data");
    let game = temp.path().join("game");
    prepare_game_root(&game);
    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game.clone())
        .unwrap();
    let owner_archive = temp.path().join("owner.zip");
    create_fixture_zip(
        &owner_archive,
        &[("nativePC/wp/one/one001/mod/one001.MOD3", b"owner bytes")],
    );
    let owner = import_equipment(&state, owner_archive);
    install_equipment(
        &state,
        source_targets(&state, &owner, &[("one001", "one002")]),
    );
    let archive = temp.path().join("candidate.zip");
    create_fixture_zip(
        &archive,
        &[("nativePC/wp/one/one004/mod/one004.mod3", b"candidate bytes")],
    );
    let candidate = import_equipment(&state, archive);
    let request = initial_request(source_targets(&state, &candidate, &[("one004", "one002")]));
    let before = snapshot_file_tree(&game);
    let manifest = read_fixture_manifest(&app_data);
    let preview = state
        .initial_retarget_install_preflight
        .preview(request.clone())
        .unwrap();
    assert!(preview.planned.install_plan().has_blocking_conflicts());
    let task = state
        .retarget_install_tasks
        .start_equipment_retarget_install_task(request.clone())
        .unwrap();
    assert!(state
        .retarget_install_task_runner
        .run_equipment_retarget_install_task(&task.task_id, request)
        .is_err());
    assert_eq!(snapshot_file_tree(&game), before);
    assert_eq!(read_fixture_manifest(&app_data), manifest);
    assert_no_retarget_staging(&app_data);
}
