use super::*;

fn assert_normal_install_tracks_all_sources(files: &[(&str, &[u8])], source_count: usize) {
    let temp = tempfile::tempdir().unwrap();
    let app_data = temp.path().join("app-data");
    let game = temp.path().join("game");
    prepare_game_root(&game);
    let baseline = snapshot_file_tree(&game);
    let archive = temp.path().join("normal-equipment.zip");
    create_fixture_zip(&archive, files);
    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game.clone())
        .unwrap();
    let mod_id = import_equipment(&state, archive);
    install_fixture_revision(&state, &mod_id, &ProfileId::new("default"));
    let manifest = read_fixture_manifest(&app_data);
    let mut expected = baseline.clone();
    for (path, bytes) in files {
        expected.insert((*path).to_owned(), bytes.to_vec());
    }
    assert_eq!(snapshot_file_tree(&game), expected);
    assert_eq!(manifest.entries.len(), files.len());
    assert_eq!(
        manifest.replacement_bindings.len(),
        source_count,
        "normal installation must record every recognized equipment source"
    );
    assert!(manifest
        .replacement_bindings
        .iter()
        .all(hmm_app::is_identity_replacement_binding));
    assert!(manifest
        .replacement_bindings
        .iter()
        .all(|binding| binding.adapter_facts().is_none()));
    let installed_revision = manifest.entries[0].revision_id.clone().unwrap();
    let candidate = temp.path().join("candidate.zip");
    let candidate_files = files
        .iter()
        .map(|(path, _)| (*path, b"new revision content".as_slice()))
        .collect::<Vec<_>>();
    create_fixture_zip(&candidate, &candidate_files);
    let (_, candidate_revision) =
        import_candidate_fixture_revision(&state, &candidate, &mod_id, &installed_revision);
    assert_ne!(candidate_revision, installed_revision);
    drop(state);

    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
    let configuration = state
        .replacement_workflow
        .equipment_configuration(&GameId::mhw(), &mod_id, Some(&ProfileId::new("default")))
        .unwrap();
    assert_eq!(configuration.sources.len(), source_count);
    assert_eq!(configuration.installed_targets.unwrap().len(), source_count);
    let moving_weapon = configuration
        .sources
        .iter()
        .any(|item| item.source.internal_id() == "one001");
    let slots = configuration
        .sources
        .iter()
        .map(|item| {
            let destination = match item.source.internal_id() {
                "one001" => Some("one002"),
                "pl121_0000" if !moving_weapon => Some("pl129_0000"),
                _ => None,
            };
            match destination {
                Some(destination) => InitialRetargetSlotIntent::Retarget {
                    source_id: item.source.id().clone(),
                    target_id: target(destination, item.source.path_family()),
                },
                None => InitialRetargetSlotIntent::KeepInPlace {
                    source_id: item.source.id().clone(),
                },
            }
        })
        .collect();
    let selection = EquipmentRetargetReinstallRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        mod_id: mod_id.clone(),
        slots,
        layer: FileLayer::new("base", 0),
    };
    let preview = state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(selection.clone())
        .unwrap();
    assert_eq!(
        preview.status,
        ReinstallPreviewStatus::Ready,
        "{:?}",
        preview.blocking_reasons
    );
    assert_eq!(snapshot_file_tree(&game), expected);
    let request = StartEquipmentRetargetReinstallTaskRequest {
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
    let mut switched = baseline.clone();
    for (path, bytes) in files {
        let destination = match *path {
            "nativePC/wp/one/one001/mod/one001.mod3" => "nativePC/wp/one/one002/mod/one002.mod3",
            "nativePC/wp/one/one001/mod/one001.mrl3" => "nativePC/wp/one/one002/mod/one002.mrl3",
            "nativePC/wp/one/one001/mod/ya001.mod3" => "nativePC/wp/one/one002/mod/ya002.mod3",
            ARMOR_SOURCE_TARGET if !moving_weapon => ARMOR_RETARGETED_TARGET,
            // 重定向保留既有排除政策；初始普通安装已在上面验证完整包含此文件。
            "nativePC/plugins/fixture_support.dll" => continue,
            path => path,
        };
        switched.insert(destination.to_owned(), bytes.to_vec());
    }
    assert_eq!(
        snapshot_file_tree(&game),
        switched,
        "target switch must use the installed revision"
    );
    let after = read_fixture_manifest(&app_data);
    assert!(after
        .entries
        .iter()
        .all(|entry| entry.revision_id.as_ref() == Some(&installed_revision)));
    assert_eq!(after.replacement_bindings.len(), source_count);
    for before in &manifest.replacement_bindings {
        let current = after
            .replacement_bindings
            .iter()
            .find(|item| item.binding().source_id() == before.binding().source_id())
            .unwrap();
        assert_eq!(before.binding_id(), current.binding_id());
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
    assert_no_reinstall_recovery_transactions(&app_data);
}

#[test]
fn ordinary_install_records_both_weapon_sources_without_changing_files() {
    assert_normal_install_tracks_all_sources(
        &[
            ("nativePC/wp/one/one001/mod/one001.mod3", b"first weapon"),
            ("nativePC/wp/one/one004/mod/one004.mod3", b"second weapon"),
            ("nativePC/sound/normal.bin", b"companion"),
        ],
        2,
    );
}

#[test]
fn ordinary_install_records_both_armor_sources_without_changing_files() {
    assert_normal_install_tracks_all_sources(
        &[
            (ARMOR_SOURCE_TARGET, ARMOR_FIXTURE_BYTES),
            (
                "nativePC/pl/f_equip/pl078_0000/body/mod/f_body.mod3",
                b"second armor",
            ),
            ("nativePC/sound/normal.bin", b"companion"),
        ],
        2,
    );
}

#[test]
fn ordinary_install_tracks_mixed_and_unknown_sources_without_dropping_plugins() {
    let mut files = EQUIPMENT_FILES.to_vec();
    files.push((
        "nativePC/plugins/fixture_support.dll",
        b"inert synthetic plugin bytes",
    ));
    assert_normal_install_tracks_all_sources(&files, 3);
}

#[test]
fn canonical_plan_records_actual_selected_sources_without_changing_file_policy() {
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
    let mut files = EQUIPMENT_FILES.to_vec();
    files.push(("nativePC/plugins/fixture_support.dll", b"inert plugin"));
    let archive = temp.path().join("selection.zip");
    create_fixture_zip(&archive, &files);
    let (_, mod_id, revision) = import_initial_fixture_revision(&state, &archive);
    let plain = state
        .install_preflight
        .preview_revision(
            &GameId::mhw(),
            &mod_id,
            &revision,
            &FileLayer::new("base", 0),
        )
        .unwrap()
        .plan;
    let bind = |plan| {
        state.replacement_workflow.bind_canonical_install_sources(
            &GameId::mhw(),
            &ProfileId::new("default"),
            &mod_id,
            &revision,
            plan,
        )
    };
    let bound = bind(plain.clone()).unwrap();
    assert_eq!(bound.actions, plain.actions);
    assert_eq!(bound.conflicts, plain.conflicts);
    assert_eq!(bound.replacement_bindings.len(), 3);
    assert!(bound
        .replacement_bindings
        .iter()
        .all(|binding| binding.adapter_facts().is_none()));
    let mut selected = plain.clone();
    selected
        .actions
        .retain(|action| !action.target_path.as_str().starts_with("nativePC/wp/one/"));
    let selected = bind(selected).unwrap();
    assert_eq!(
        selected.replacement_bindings.len(),
        2,
        "unselected equipment cannot gain an installed binding"
    );
    assert_eq!(
        bind(bound).unwrap_err(),
        ReplacementWorkflowError::BindingUnavailable
    );
    let mut wrong_owner = plain;
    wrong_owner.actions[0].provider.mod_id = ModId::new("different-mod");
    assert_eq!(
        bind(wrong_owner).unwrap_err(),
        ReplacementWorkflowError::PlanUnavailable
    );
    assert_eq!(snapshot_file_tree(&game), baseline);
    assert!(!app_data.join("install/manifests/default.json").exists());
    assert_no_retarget_staging(&app_data);
}

#[test]
fn ordinary_revision_reinstall_records_every_new_source_and_preserves_all_files() {
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
    let archive = temp.path().join("original.zip");
    create_fixture_zip(&archive, EQUIPMENT_FILES);
    let (_, mod_id, revision) = import_initial_fixture_revision(&state, &archive);
    install_fixture_revision(&state, &mod_id, &ProfileId::new("default"));
    let candidate_files: &[(&str, &[u8])] = &[
        ("nativePC/wp/one/one004/mod/one004.mod3", b"new weapon"),
        (
            "nativePC/pl/f_equip/pl078_0000/body/mod/f_body.mod3",
            b"new armor",
        ),
        (
            "nativePC/plugins/fixture_support.dll",
            b"unchanged normal install policy",
        ),
    ];
    let candidate = temp.path().join("updated.zip");
    create_fixture_zip(&candidate, candidate_files);
    let (_, next_revision) =
        import_candidate_fixture_revision(&state, &candidate, &mod_id, &revision);
    let profile = ProfileId::new("default");
    let preview = preview_fixture_reinstall(&state, &profile, &mod_id, &next_revision);
    assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
    let request = StartReinstallTaskRequest {
        game_id: GameId::mhw(),
        profile_id: profile.clone(),
        mod_id: mod_id.clone(),
        candidate_revision_id: next_revision.clone(),
        layer: FileLayer::new("base", 0),
        plan_token: preview.plan_token.unwrap(),
    };
    let task = state
        .reinstall_tasks
        .start_reinstall_task(request.clone())
        .unwrap();
    state
        .reinstall_task_runner
        .run_reinstall_task(&task.task_id, request)
        .unwrap();
    let mut expected = baseline.clone();
    for (path, bytes) in candidate_files {
        expected.insert((*path).to_owned(), bytes.to_vec());
    }
    assert_eq!(snapshot_file_tree(&game), expected);
    let manifest = read_fixture_manifest(&app_data);
    assert_eq!(manifest.entries.len(), 3);
    assert_eq!(manifest.replacement_bindings.len(), 2);
    assert!(manifest
        .replacement_bindings
        .iter()
        .all(|binding| binding.revision_id() == Some(&next_revision)));
    let request = StartUninstallTaskRequest {
        game_id: GameId::mhw(),
        profile_id: profile,
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
}

#[test]
fn switching_another_source_keeps_an_unchanged_weapon_with_uppercase_directories() {
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
    let weapon = "nativePC/WP/ONE/ONE001/mod/one001.mod3";
    let files: &[(&str, &[u8])] = &[
        (weapon, b"uppercase weapon"),
        (ARMOR_SOURCE_TARGET, ARMOR_FIXTURE_BYTES),
    ];
    let archive = temp.path().join("uppercase.zip");
    create_fixture_zip(&archive, files);
    let mod_id = import_equipment(&state, archive);
    install_fixture_revision(&state, &mod_id, &ProfileId::new("default"));
    let selection = selection(&state, &mod_id, "one001", Some("pl129_0000"));
    let preview = state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(selection.clone())
        .unwrap();
    assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
    assert_eq!(
        preview.counts.retained, 1,
        "unchanged weapon must not become added plus stale"
    );
    let request = StartEquipmentRetargetReinstallTaskRequest {
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
    let mut expected = baseline;
    expected.insert(weapon.to_owned(), b"uppercase weapon".to_vec());
    expected.insert(
        ARMOR_RETARGETED_TARGET.to_owned(),
        ARMOR_FIXTURE_BYTES.to_vec(),
    );
    assert_eq!(snapshot_file_tree(&game), expected);
    assert_no_reinstall_recovery_transactions(&app_data);
}
