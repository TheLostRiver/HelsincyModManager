use super::*;
use hmm_app::{
    EquipmentRetargetReinstallTaskExecutor, ReinstallTaskExecutor, ReinstallTaskPrepared,
};
use hmm_core::PackageFileId;

#[test]
fn selection_rejects_stale_inventory_foreign_ids_and_scope_drift_without_writes() {
    let fixture = AutomationFixture::new();
    let scope = fixture.scope();
    let inventory = fixture
        .state
        .plugin_selection
        .inventory(&scope)
        .unwrap()
        .unwrap();
    let id = inventory.candidates[0].package_file_id.clone();
    for ids in [
        vec![PackageFileId::new("foreign")],
        vec![id.clone(), id.clone()],
    ] {
        assert!(fixture
            .state
            .plugin_selection
            .select(&scope, &inventory.selection.inventory_id(), &ids)
            .is_err());
    }
    let other = hmm_core::PluginSelectionScope {
        profile_id: ProfileId::new("other"),
        ..scope.clone()
    };
    assert!(fixture
        .state
        .plugin_selection
        .select(
            &other,
            &inventory.selection.inventory_id(),
            std::slice::from_ref(&id)
        )
        .is_err());
    fixture.choose(false);
    assert!(
        fixture
            .state
            .plugin_selection
            .inventory(&other)
            .unwrap()
            .unwrap()
            .candidates[0]
            .selected
    );
    let mut bytes = fixture.dll.clone();
    bytes[400] = 7;
    fs::write(fixture.package().join(PLUGIN), bytes).unwrap();
    assert!(fixture
        .state
        .plugin_selection
        .select(&scope, &inventory.selection.inventory_id(), &[id])
        .is_err());
    assert!(
        fixture
            .state
            .plugin_selection
            .inventory(&scope)
            .unwrap()
            .unwrap()
            .confirmation_required
    );
    assert!(!fixture.game.join(PLUGIN).exists());
}

#[test]
fn ordinary_install_reports_plugin_inventory_failures_without_writes() {
    let fixture = AutomationFixture::new();
    fixture.choose(true);
    let files = fs::read_dir(fixture.app_data.join("install/plugin-selections"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 1);
    fs::write(&files[0], b"invalid preference fixture").unwrap();
    let before = snapshot_file_tree(&fixture.game);
    let request = StartInstallTaskRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        mod_id: fixture.mod_id.clone(),
        layer: FileLayer::new("base", 0),
    };
    let task = fixture
        .state
        .install_tasks
        .start_install_task(request.clone())
        .unwrap();
    let error = fixture
        .state
        .install_task_runner
        .run_install_task(&task.task_id, request)
        .unwrap_err();
    assert_eq!(
        error.events.last().unwrap().error.as_deref(),
        Some("install_failed:plugin_selection_unavailable")
    );
    assert_eq!(snapshot_file_tree(&fixture.game), before);
}

#[test]
fn selected_plugin_survives_changing_one_source_and_is_not_reported_as_excluded() {
    let fixture = AutomationFixture::with_equipment(true);
    fixture.choose(true);
    let request = selection(&fixture.state, &fixture.mod_id, "one002", None);
    let planned = fixture
        .state
        .initial_retarget_install_preflight
        .preview(initial_request(request.clone()))
        .unwrap()
        .planned;
    assert!(!planned
        .warnings()
        .contains(&hmm_core::ReplacementWarning::PolicyExcludedResources));
    assert!(planned.file_effects().iter().any(|effect| effect.reason
        == hmm_core::RetargetFileReason::PluginSelected
        && effect.target_path.is_some()));
    install_equipment(&fixture.state, request);
    let before = read_fixture_manifest(&fixture.app_data);
    let unchanged = before
        .replacement_bindings
        .iter()
        .filter(|binding| binding.source_internal_id() != "one001")
        .cloned()
        .collect::<Vec<_>>();
    let selected = selection(&fixture.state, &fixture.mod_id, "one003", None);
    let preview = fixture
        .state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(selected.clone())
        .unwrap();
    assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
    assert_eq!(preview.attachment_counts.excluded, 0);
    let request = StartEquipmentRetargetReinstallTaskRequest {
        selection: selected,
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
    let after = read_fixture_manifest(&fixture.app_data);
    assert_eq!(after.plugin_selections, before.plugin_selections);
    assert_eq!(
        after
            .replacement_bindings
            .iter()
            .filter(|binding| binding.source_internal_id() != "one001")
            .cloned()
            .collect::<Vec<_>>(),
        unchanged
    );
    assert_eq!(fs::read(fixture.game.join(PLUGIN)).unwrap(), fixture.dll);
}

#[test]
fn untracked_missing_plugin_requires_selection_but_tracked_missing_or_changed_files_block() {
    for kind in ["untracked", "missing", "changed"] {
        let fixture = AutomationFixture::new();
        fixture.install();
        let mut manifest = read_fixture_manifest(&fixture.app_data);
        if kind == "untracked" {
            manifest
                .entries
                .retain(|entry| entry.target_path.as_str() != PLUGIN);
            manifest.plugin_selections.clear();
            fixture.manifests.save_manifest(&manifest).unwrap();
            fs::remove_dir_all(fixture.app_data.join("install/plugin-selections")).unwrap();
        }
        if kind == "changed" {
            fs::write(fixture.game.join(PLUGIN), b"external edit").unwrap();
        } else {
            fs::remove_file(fixture.game.join(PLUGIN)).unwrap();
        }
        let before = snapshot_file_tree(&fixture.game);
        let preview = fixture
            .state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(fixture.reapply_request())
            .unwrap();
        if kind == "untracked" {
            assert_eq!(preview.status, ReinstallPreviewStatus::NoChanges);
            assert!(
                !fixture
                    .state
                    .plugin_selection
                    .inventory(&fixture.scope())
                    .unwrap()
                    .unwrap()
                    .candidates[0]
                    .selected
            );
            fixture.choose(true);
            reapply(&fixture.state, &fixture.mod_id);
            assert_eq!(fs::read(fixture.game.join(PLUGIN)).unwrap(), fixture.dll);
        } else {
            assert_eq!(preview.status, ReinstallPreviewStatus::Blocked, "{kind}");
            assert_eq!(snapshot_file_tree(&fixture.game), before);
            assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
        }
    }
}

#[test]
fn reapply_rechecks_excluded_plugin_bytes_game_bytes_choice_and_token() {
    for kind in ["source", "game", "choice", "token"] {
        let fixture = AutomationFixture::new();
        fixture.install();
        fixture.choose(false);
        let prepared = fixture
            .state
            .reinstall_executor
            .prepare_equipment_retarget_reinstall(fixture.reapply_request())
            .unwrap();
        let mut token = prepared.plan_token().to_owned();
        match kind {
            "source" => {
                let mut bytes = fixture.dll.clone();
                bytes[401] = 1;
                fs::write(fixture.package().join(PLUGIN), bytes).unwrap();
            }
            "game" => fs::write(fixture.game.join(PLUGIN), b"external edit").unwrap(),
            "choice" => fixture.choose(true),
            "token" => token.push_str("-stale"),
            _ => unreachable!(),
        }
        let before = snapshot_file_tree(&fixture.game);
        let manifest = read_fixture_manifest(&fixture.app_data);
        assert!(
            fixture
                .state
                .reinstall_executor
                .commit(prepared, &token)
                .is_err(),
            "accepted {kind}"
        );
        assert_eq!(snapshot_file_tree(&fixture.game), before);
        assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
    }
}

#[test]
fn plugin_removal_manifest_failure_restores_files_and_applied_choices() {
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
    fixture.manifests.fail_next_save();
    assert!(fixture
        .state
        .reinstall_task_runner
        .run_equipment_retarget_reinstall_task(&task.task_id, request)
        .is_err());
    assert_eq!(snapshot_file_tree(&fixture.game), before);
    assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
    assert!(
        !fixture
            .state
            .plugin_selection
            .inventory(&fixture.scope())
            .unwrap()
            .unwrap()
            .candidates[0]
            .selected,
        "pending preference stays pending"
    );
    assert_no_reinstall_recovery_transactions(&fixture.app_data);
}

#[test]
fn candidate_revision_has_its_own_choices_and_cli_token_approves_only_that_preview() {
    let fixture = AutomationFixture::new();
    fixture.install();
    let old_scope = fixture.scope();
    let mut next = fixture.dll.clone();
    next[402] = 2;
    let archive = fixture._temp.path().join("candidate.zip");
    create_fixture_zip(&archive, &[(MODEL, b"candidate model"), (PLUGIN, &next)]);
    let (_, candidate) = import_candidate_fixture_revision(
        &fixture.state,
        &archive,
        &fixture.mod_id,
        &old_scope.revision_id,
    );
    assert_eq!(
        fixture.scope(),
        old_scope,
        "default choices follow the installed revision"
    );
    let candidate_scope = hmm_core::PluginSelectionScope {
        revision_id: candidate.clone(),
        ..old_scope
    };
    assert!(
        fixture
            .state
            .plugin_selection
            .inventory(&candidate_scope)
            .unwrap()
            .unwrap()
            .confirmation_required
    );
    let reader = crate::ReadOnlyInstallAutomation::from_environment(&fixture.environment).unwrap();
    let preview = reader
        .reinstall_preview(
            "mhw",
            "default",
            fixture.mod_id.as_str(),
            candidate.as_str(),
        )
        .unwrap();
    let token = preview.plan_token.unwrap();
    assert!(crate::CliLifecycleAutomation::prepare_reinstall(
        &fixture.environment,
        "mhw",
        "default",
        fixture.mod_id.as_str(),
        candidate.as_str(),
        &(token.clone() + "bad")
    )
    .is_err());
    assert!(
        fixture
            .state
            .plugin_selection
            .inventory(&candidate_scope)
            .unwrap()
            .unwrap()
            .confirmation_required
    );
    let prepared = crate::CliLifecycleAutomation::prepare_reinstall(
        &fixture.environment,
        "mhw",
        "default",
        fixture.mod_id.as_str(),
        candidate.as_str(),
        &token,
    )
    .unwrap();
    assert_eq!(
        prepared
            .run_reinstall()
            .unwrap()
            .events
            .last()
            .unwrap()
            .status,
        TaskStatus::Completed
    );
    assert_eq!(fs::read(fixture.game.join(PLUGIN)).unwrap(), next);
    assert_eq!(
        read_fixture_manifest(&fixture.app_data).plugin_selections[0].scope(),
        &candidate_scope
    );
}

#[test]
fn another_mods_plugin_conflicts_are_blocked_and_its_applied_choices_are_preserved() {
    for conflict in [false, true] {
        let fixture = AutomationFixture::new();
        fixture.install();
        let old = read_fixture_manifest(&fixture.app_data);
        let before = snapshot_file_tree(&fixture.game);
        let archive = fixture._temp.path().join("other-mod.zip");
        let plugin_path = if conflict {
            PLUGIN
        } else {
            "nativePC/plugins/other-fixture.dll"
        };
        create_fixture_zip(
            &archive,
            &[
                ("nativePC/sound/other-fixture.bin", b"second package"),
                (plugin_path, &fixture.dll),
            ],
        );
        let other_mod = import_equipment(&fixture.state, archive);
        let scope = fixture
            .state
            .plugin_selection
            .resolve_scope(
                GameId::mhw(),
                ProfileId::new("default"),
                other_mod.clone(),
                None,
            )
            .unwrap();
        let inventory = fixture
            .state
            .plugin_selection
            .inventory(&scope)
            .unwrap()
            .unwrap();
        let ids = inventory
            .candidates
            .iter()
            .filter(|file| file.selectable)
            .map(|file| file.package_file_id.clone())
            .collect::<Vec<_>>();
        fixture
            .state
            .plugin_selection
            .select(&scope, &inventory.selection.inventory_id(), &ids)
            .unwrap();
        let request = StartInstallTaskRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: other_mod,
            layer: FileLayer::new("base", 0),
        };
        let task = fixture
            .state
            .install_tasks
            .start_install_task(request.clone())
            .unwrap();
        let result = fixture
            .state
            .install_task_runner
            .run_install_task(&task.task_id, request);
        if conflict {
            assert!(result.is_err());
            assert_eq!(snapshot_file_tree(&fixture.game), before);
            assert_eq!(read_fixture_manifest(&fixture.app_data), old);
        } else {
            result.unwrap();
            let current = read_fixture_manifest(&fixture.app_data);
            assert_eq!(current.plugin_selections.len(), 2);
            fixture.choose(false);
            reapply(&fixture.state, &fixture.mod_id);
            let after = read_fixture_manifest(&fixture.app_data);
            assert_eq!(
                after
                    .plugin_selections
                    .iter()
                    .find(|selection| selection.scope() == &scope),
                current
                    .plugin_selections
                    .iter()
                    .find(|selection| selection.scope() == &scope)
            );
            assert_eq!(
                fs::read(fixture.game.join(plugin_path)).unwrap(),
                fixture.dll
            );
        }
    }
}

#[test]
fn batch_revision_upgrade_confirms_only_the_sealed_candidate_plugin_choices() {
    let fixture = AutomationFixture::with_equipment(true);
    fixture.install();
    let old_scope = fixture.scope();
    let before = snapshot_file_tree(&fixture.game);
    let mut next = fixture.dll.clone();
    next[402] = 3;
    let archive = fixture._temp.path().join("batch-candidate.zip");
    let mut files = EQUIPMENT_FILES.to_vec();
    files.push((PLUGIN, &next));
    create_fixture_zip(&archive, &files);
    let (_, candidate) = import_candidate_fixture_revision(
        &fixture.state,
        &archive,
        &fixture.mod_id,
        &old_scope.revision_id,
    );
    let candidate_scope = hmm_core::PluginSelectionScope {
        revision_id: candidate.clone(),
        ..old_scope.clone()
    };
    let request = crate::BatchLifecyclePlanRequest {
        plan: hmm_core::BatchPlanRequest {
            schema_version: hmm_core::BATCH_PLAN_SCHEMA_VERSION,
            operation: hmm_core::BatchOperation::Reinstall,
            game_id: GameId::mhw(),
            profile_id: old_scope.profile_id,
            execution_policy: hmm_core::BatchExecutionPolicy::StopOnFailure,
            items: vec![hmm_core::BatchItemInput::Reinstall(
                hmm_core::ReinstallBatchItemInput {
                    mod_id: fixture.mod_id.clone(),
                    installed_revision_id: old_scope.revision_id,
                    candidate_revision_id: candidate,
                    layer: FileLayer::new("base", 0),
                    replacement_binding_snapshot: None,
                    intent: hmm_core::ReinstallIntent::Standard,
                },
            )],
        },
        replacement_targets: std::collections::BTreeMap::new(),
    };
    let preview =
        crate::BatchLifecycleAutomation::preview_request(&fixture.environment, request.clone())
            .unwrap();
    assert_eq!(
        preview.plan.status(),
        hmm_core::BatchPlanStatus::Ready,
        "{preview:?}"
    );
    let database = fixture.state.database_handle();
    let (_, sealed) = crate::BatchLifecycleAutomation::seal_request_with_database(
        &fixture.environment,
        request,
        preview.preview_token.as_deref().unwrap(),
        database.clone(),
    )
    .unwrap();
    assert!(
        fixture
            .state
            .plugin_selection
            .inventory(&candidate_scope)
            .unwrap()
            .unwrap()
            .confirmation_required
    );
    assert!(
        crate::BatchLifecycleAutomation::start_request_with_database(
            &fixture.environment,
            &sealed.batch_id,
            &(sealed.plan_token.clone() + "bad"),
            database.clone()
        )
        .is_err()
    );
    assert_eq!(snapshot_file_tree(&fixture.game), before);
    assert!(
        fixture
            .state
            .plugin_selection
            .inventory(&candidate_scope)
            .unwrap()
            .unwrap()
            .confirmation_required
    );
    let (_, result) = crate::BatchLifecycleAutomation::start_request_with_database(
        &fixture.environment,
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
    assert_eq!(fs::read(fixture.game.join(PLUGIN)).unwrap(), next);
    let manifest = read_fixture_manifest(&fixture.app_data);
    assert!(manifest.replacement_bindings.len() > 1);
    assert!(manifest
        .replacement_bindings
        .iter()
        .all(hmm_app::is_identity_replacement_binding));
    assert_eq!(manifest.plugin_selections[0].scope(), &candidate_scope);
}

#[test]
fn sealed_batch_reapply_can_remove_the_last_owned_plugin_and_restore_its_backup() {
    let fixture = AutomationFixture::new();
    let archive = fixture._temp.path().join("plugin-only.zip");
    create_fixture_zip(&archive, &[(PLUGIN, &fixture.dll)]);
    let mod_id = import_equipment(&fixture.state, archive);
    fs::write(fixture.game.join(PLUGIN), b"preexisting plugin fixture").unwrap();
    let baseline = snapshot_file_tree(&fixture.game);
    let scope = fixture
        .state
        .plugin_selection
        .resolve_scope(
            GameId::mhw(),
            ProfileId::new("default"),
            mod_id.clone(),
            None,
        )
        .unwrap();
    let inventory = fixture
        .state
        .plugin_selection
        .inventory(&scope)
        .unwrap()
        .unwrap();
    let ids = inventory
        .candidates
        .iter()
        .map(|file| file.package_file_id.clone())
        .collect::<Vec<_>>();
    fixture
        .state
        .plugin_selection
        .select(&scope, &inventory.selection.inventory_id(), &ids)
        .unwrap();
    install_fixture_revision(&fixture.state, &mod_id, &ProfileId::new("default"));
    let inventory = fixture
        .state
        .plugin_selection
        .inventory(&scope)
        .unwrap()
        .unwrap();
    fixture
        .state
        .plugin_selection
        .select(&scope, &inventory.selection.inventory_id(), &[])
        .unwrap();
    let request = crate::BatchLifecyclePlanRequest {
        plan: hmm_core::BatchPlanRequest {
            schema_version: hmm_core::BATCH_PLAN_SCHEMA_VERSION,
            operation: hmm_core::BatchOperation::Reinstall,
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            execution_policy: hmm_core::BatchExecutionPolicy::StopOnFailure,
            items: vec![hmm_core::BatchItemInput::Reinstall(
                hmm_core::ReinstallBatchItemInput {
                    mod_id: mod_id.clone(),
                    installed_revision_id: scope.revision_id.clone(),
                    candidate_revision_id: scope.revision_id,
                    layer: FileLayer::new("base", 0),
                    replacement_binding_snapshot: None,
                    intent: hmm_core::ReinstallIntent::ReapplyEquipmentTargets,
                },
            )],
        },
        replacement_targets: std::collections::BTreeMap::new(),
    };
    let preview =
        crate::BatchLifecycleAutomation::preview_request(&fixture.environment, request.clone())
            .unwrap();
    assert_eq!(
        preview.plan.status(),
        hmm_core::BatchPlanStatus::Ready,
        "{preview:?}"
    );
    let database = fixture.state.database_handle();
    let (_, sealed) = crate::BatchLifecycleAutomation::seal_request_with_database(
        &fixture.environment,
        request,
        preview.preview_token.as_deref().unwrap(),
        database.clone(),
    )
    .unwrap();
    let (_, result) = crate::BatchLifecycleAutomation::start_request_with_database(
        &fixture.environment,
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
    assert_eq!(snapshot_file_tree(&fixture.game), baseline);
    let manifest = read_fixture_manifest(&fixture.app_data);
    assert!(manifest.entries.iter().all(|entry| entry.mod_id != mod_id));
    assert!(manifest.plugin_selections.is_empty());
    assert_no_reinstall_recovery_transactions(&fixture.app_data);
}
