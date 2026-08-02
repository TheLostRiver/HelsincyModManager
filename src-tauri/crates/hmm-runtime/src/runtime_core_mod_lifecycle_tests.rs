use super::{retarget_reinstall_staging_root, HmmRuntime, RetargetStagingCleanup};
use hmm_app::{
    BuildImportedModInstallPlanRequest, InstallManifestQueryRequest, InstallManifestStatus,
    InstallRecoveryScanRequest, InstallRecoveryStatus, PreviewInitialRetargetInstallRequest,
    ReinstallPlanPreview, ReinstallPreviewRequest, ReinstallPreviewStatus, ReinstallTargetCounts,
    RetargetReinstallRequest, StartImportModRevisionTaskRequest, StartImportModTaskRequest,
    StartInstallTaskRequest, StartReinstallTaskRequest, StartRetargetInstallTaskRequest,
    StartRetargetReinstallTaskRequest, StartUninstallTaskRequest, TaskKind, TaskStatus,
};
use hmm_core::{
    FileLayer, GameDirectoryStatus, GameId, InstallManifest, ModId, ModRevisionId, ProfileId,
    ReplacementTargetId,
};
use hmm_infra::{FileSystemAuditLogWriter, JsonInstallManifestRepository};
use hmm_ports::{AuditLogEvent, AuditLogReadRequest, AuditLogReader, InstallManifestRepository};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use zip::write::SimpleFileOptions;

const OVERWRITTEN_TARGET: &str = "nativePC/lifecycle/overwritten.bin";
const BASELINE_BYTES: &[u8] = b"game-baseline-original\n";
const ARMOR_SOURCE_TARGET: &str = "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3";
const ARMOR_RETARGETED_TARGET: &str = "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3";
const ARMOR_SWITCH_TARGET: &str = "nativePC/pl/f_equip/pl129_0010/arm/mod/f_body.mod3";
const ARMOR_FIXTURE_BYTES: &[u8] = b"synthetic armor fixture\n";
const ARMOR_SWITCH_BASELINE_BYTES: &[u8] = b"pre-existing beta armor baseline\n";

const V1_FILES: &[(&str, &[u8])] = &[
    (OVERWRITTEN_TARGET, b"fixture-overwrite-v1\n"),
    ("nativePC/lifecycle/replaced.bin", b"fixture-replaced-v1\n"),
    ("nativePC/lifecycle/retained.bin", b"fixture-retained\n"),
    ("nativePC/lifecycle/stale.bin", b"fixture-stale-v1\n"),
];

#[test]
fn retarget_reinstall_staging_is_operation_scoped_and_cleanup_is_drop_safe() {
    let temp = tempfile::tempdir().expect("create staging cleanup temp root");
    let first = retarget_reinstall_staging_root(temp.path());
    let second = retarget_reinstall_staging_root(temp.path());
    assert_ne!(
        first, second,
        "each preview/start needs an isolated staging root"
    );

    fs::create_dir_all(&first).expect("create synthetic prepared staging");
    fs::write(first.join("prepared.bin"), b"prepared").expect("write synthetic staging file");
    let cleanup = RetargetStagingCleanup::armed(first.clone());
    drop(cleanup);

    assert!(
        !first.exists(),
        "dropping prepared state must discard staging"
    );
}

const V2_FILES: &[(&str, &[u8])] = &[
    ("nativePC/lifecycle/added-v2.bin", b"fixture-added-v2\n"),
    (OVERWRITTEN_TARGET, b"fixture-overwrite-v2\n"),
    ("nativePC/lifecycle/replaced.bin", b"fixture-replaced-v2\n"),
    ("nativePC/lifecycle/retained.bin", b"fixture-retained\n"),
];

#[test]
fn fixture_contract_covers_reinstall_target_classes() {
    let v1 = fixture_map(V1_FILES);
    let v2 = fixture_map(V2_FILES);
    let mut retained = Vec::new();
    let mut replaced = Vec::new();
    let mut stale = Vec::new();

    for (path, v1_bytes) in &v1 {
        match v2.get(path) {
            Some(v2_bytes) if v1_bytes == v2_bytes => retained.push(*path),
            Some(_) => replaced.push(*path),
            None => stale.push(*path),
        }
    }

    let added = v2
        .keys()
        .filter(|path| !v1.contains_key(*path))
        .copied()
        .collect::<Vec<_>>();

    assert_eq!(retained, ["nativePC/lifecycle/retained.bin"]);
    assert_eq!(
        replaced,
        [
            "nativePC/lifecycle/overwritten.bin",
            "nativePC/lifecycle/replaced.bin",
        ]
    );
    assert_eq!(added, ["nativePC/lifecycle/added-v2.bin"]);
    assert_eq!(stale, ["nativePC/lifecycle/stale.bin"]);
}

#[test]
fn audit_redaction_detects_windows_path_after_json_encoding() {
    let forbidden = r"C:\Users\fixture\game";
    let evidence = serde_json::json!({
        "events": [{ "safePath": forbidden }],
    });
    let serialized = serde_json::to_string(&evidence).expect("serialize audit regression fixture");

    assert!(!serialized.contains(forbidden));
    assert!(json_value_contains_forbidden(&evidence, forbidden));
}

#[test]
fn headless_composition_imports_v1_and_rebuilds_plan_after_restart() {
    let temp = tempfile::tempdir().expect("create lifecycle temp root");
    let app_data_dir = temp.path().join("app-data");
    let game_root = temp.path().join("game");
    let archive_path = temp.path().join("lifecycle-v1.zip");
    prepare_game_root(&game_root);
    create_fixture_zip(&archive_path, V1_FILES);

    let state = HmmRuntime::from_app_data_dir(app_data_dir.clone())
        .expect("compose headless state from temp AppData");
    let setup = state
        .game_setup
        .save_game_directory(GameId::mhw(), game_root.clone())
        .expect("save validated temp game directory");
    assert_eq!(setup.status, GameDirectoryStatus::Configured);

    let import_task = state
        .mod_import_tasks
        .start_import_mod_task(StartImportModTaskRequest {
            archive_path: archive_path.clone(),
        })
        .expect("register fixture import task");
    assert_eq!(import_task.kind, TaskKind::ModImport);
    assert_eq!(import_task.status, TaskStatus::Queued);

    let import_events = state
        .mod_import_task_runner
        .run_prepare_task(&import_task.task_id, archive_path)
        .expect("prepare and persist fixture import");
    assert!(import_events
        .iter()
        .all(|event| event.task_id == import_task.task_id));
    assert_eq!(
        import_events.last().map(|event| event.phase.as_str()),
        Some("mod_import.prepare.completed")
    );
    assert_eq!(
        state.task_manager.task_status(&import_task.task_id),
        Some(TaskStatus::Completed)
    );

    let mod_id = ModId::new(import_task.task_id.clone());
    let expected_targets = expected_v1_targets();
    assert_eq!(
        library_ids(&state).as_slice(),
        std::slice::from_ref(&import_task.task_id)
    );
    assert_eq!(build_target_paths(&state, mod_id.clone()), expected_targets);
    assert_game_root_unchanged(&game_root);

    drop(state);

    let restarted = HmmRuntime::from_app_data_dir(app_data_dir)
        .expect("recompose headless state from persisted AppData");
    let restarted_setup = restarted
        .game_setup
        .get_status(GameId::mhw())
        .expect("load persisted game setup");
    assert_eq!(restarted_setup.status, GameDirectoryStatus::Configured);
    assert_eq!(
        restarted_setup.instance.map(|instance| instance.root_dir),
        Some(game_root.clone())
    );
    assert_eq!(
        library_ids(&restarted).as_slice(),
        std::slice::from_ref(&import_task.task_id)
    );
    assert_eq!(build_target_paths(&restarted, mod_id), expected_targets);
    assert_eq!(
        restarted.task_manager.task_status(&import_task.task_id),
        None,
        "task state is transient; restart facts must come from repositories"
    );
    assert_game_root_unchanged(&game_root);
}

#[test]
fn headless_composition_retargets_staging_commits_and_persists_binding_snapshot() {
    let temp = tempfile::tempdir().expect("create retarget lifecycle temp root");
    let app_data_dir = temp.path().join("app-data");
    let game_root = temp.path().join("game");
    let archive_path = temp.path().join("armor-v1.zip");
    prepare_game_root(&game_root);
    create_fixture_zip(&archive_path, &[(ARMOR_SOURCE_TARGET, ARMOR_FIXTURE_BYTES)]);

    let state = HmmRuntime::from_app_data_dir(app_data_dir.clone())
        .expect("compose headless state from temp AppData");
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game_root.clone())
        .expect("save validated temp game directory");
    let import_task = state
        .mod_import_tasks
        .start_import_mod_task(StartImportModTaskRequest {
            archive_path: archive_path.clone(),
        })
        .expect("register armor fixture import");
    state
        .mod_import_task_runner
        .run_prepare_task(&import_task.task_id, archive_path)
        .expect("prepare armor fixture import");

    let profile_id = ProfileId::new("default");
    let mod_id = ModId::new(import_task.task_id);
    let request = StartRetargetInstallTaskRequest {
        game_id: GameId::mhw(),
        profile_id: profile_id.clone(),
        mod_id: mod_id.clone(),
        target_id: ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target id"),
        layer: FileLayer::new("base", 0),
    };
    let task = state
        .retarget_install_tasks
        .start_retarget_install_task(request.clone())
        .expect("register retarget install task");
    let events = state
        .retarget_install_task_runner
        .run_retarget_install_task(&task.task_id, request)
        .expect("run retarget install task");

    assert_eq!(
        events
            .iter()
            .map(|event| event.phase.as_str())
            .collect::<Vec<_>>(),
        [
            "install.retarget.plan.building",
            "install.retarget.commit.processing",
            "install.retarget.completed",
        ]
    );
    assert_eq!(
        fs::read(game_root.join(ARMOR_RETARGETED_TARGET)).expect("read retargeted armor file"),
        ARMOR_FIXTURE_BYTES
    );
    assert!(!game_root.join(ARMOR_SOURCE_TARGET).exists());

    let manifest =
        JsonInstallManifestRepository::new(app_data_dir.join("install").join("manifests"))
            .load_manifest(&profile_id)
            .expect("load retarget manifest")
            .expect("retarget manifest exists");
    assert_eq!(manifest.entries.len(), 1);
    assert_eq!(
        manifest.entries[0].target_path.as_str(),
        ARMOR_RETARGETED_TARGET
    );
    assert_eq!(manifest.replacement_bindings.len(), 1);
    let snapshot = &manifest.replacement_bindings[0];
    assert_eq!(snapshot.mod_id(), &mod_id);
    assert_eq!(
        snapshot.binding().target_id().as_str(),
        "mhw:armor:fatalis-alpha"
    );
    assert_eq!(snapshot.source_internal_id(), "pl121_0000");
    assert_eq!(snapshot.target_internal_id(), "pl129_0000");
    let manifest_revision = manifest.entries[0]
        .revision_id
        .clone()
        .expect("retarget manifest records imported revision");
    assert_eq!(snapshot.revision_id(), Some(&manifest_revision));

    let staging_parent = app_data_dir.join("install").join("retarget-staging");
    assert!(
        !staging_parent.exists()
            || fs::read_dir(staging_parent)
                .expect("read retarget staging parent")
                .next()
                .is_none(),
        "successful commit must clean its temporary retarget staging"
    );
}

#[test]
fn headless_composition_blocks_initial_retarget_when_required_prerequisites_are_missing() {
    let temp = tempfile::tempdir().expect("create blocked retarget temp root");
    let app_data_dir = temp.path().join("app-data");
    let game_root = temp.path().join("game");
    let archive_path = temp.path().join("armor-v1.zip");
    prepare_game_root_without_prerequisites(&game_root);
    let baseline = snapshot_file_tree(&game_root);
    create_fixture_zip(&archive_path, &[(ARMOR_SOURCE_TARGET, ARMOR_FIXTURE_BYTES)]);

    let state = HmmRuntime::from_app_data_dir(app_data_dir.clone())
        .expect("compose headless state from temp AppData");
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game_root.clone())
        .expect("save validated temp game directory");
    let import_task = state
        .mod_import_tasks
        .start_import_mod_task(StartImportModTaskRequest {
            archive_path: archive_path.clone(),
        })
        .expect("register armor fixture import");
    state
        .mod_import_task_runner
        .run_prepare_task(&import_task.task_id, archive_path)
        .expect("prepare armor fixture import");

    let profile_id = ProfileId::new("default");
    let mod_id = ModId::new(import_task.task_id);
    let preview_request = PreviewInitialRetargetInstallRequest {
        game_id: GameId::mhw(),
        profile_id: profile_id.clone(),
        mod_id: mod_id.clone(),
        target_id: ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target id"),
        layer: FileLayer::new("base", 0),
    };
    let preflight = state
        .initial_retarget_install_preflight
        .preview(preview_request.clone())
        .expect("preview blocked retarget plan");
    assert_eq!(
        preflight.prerequisite_decision.status,
        hmm_app::GamePrerequisiteDecisionStatus::Blocked
    );
    assert_eq!(
        preflight.prerequisite_decision.codes,
        vec![hmm_app::GamePrerequisiteDecisionCode::MissingRequiredFile]
    );

    let request = StartRetargetInstallTaskRequest {
        game_id: preview_request.game_id,
        profile_id: preview_request.profile_id,
        mod_id: preview_request.mod_id,
        target_id: preview_request.target_id,
        layer: preview_request.layer,
    };
    let task = state
        .retarget_install_tasks
        .start_retarget_install_task(request.clone())
        .expect("register blocked retarget task");
    let error = state
        .retarget_install_task_runner
        .run_retarget_install_task(&task.task_id, request)
        .expect_err("missing prerequisites must block retarget install");

    assert_eq!(
        error.events.last().and_then(|event| event.error.as_deref()),
        Some("install_retarget_failed:prerequisite")
    );
    assert_eq!(snapshot_file_tree(&game_root), baseline);
    assert_no_retarget_staging(&app_data_dir);
    assert!(
        JsonInstallManifestRepository::new(app_data_dir.join("install").join("manifests"))
            .load_manifest(&profile_id)
            .expect("load blocked retarget manifest")
            .is_none()
    );
}

#[test]
fn headless_composition_switches_retarget_with_true_reinstall_and_uninstalls_to_baseline() {
    let temp = tempfile::tempdir().expect("create retarget reinstall temp root");
    let app_data_dir = temp.path().join("app-data");
    let game_root = temp.path().join("game");
    let archive_path = temp.path().join("armor-v1.zip");
    prepare_game_root(&game_root);
    let switch_target = game_root.join(ARMOR_SWITCH_TARGET);
    fs::create_dir_all(switch_target.parent().expect("switch target parent"))
        .expect("create switch target parent");
    fs::write(&switch_target, ARMOR_SWITCH_BASELINE_BYTES)
        .expect("write pre-existing switch target baseline");
    let baseline = snapshot_file_tree(&game_root);
    create_fixture_zip(&archive_path, &[(ARMOR_SOURCE_TARGET, ARMOR_FIXTURE_BYTES)]);

    let state = HmmRuntime::from_app_data_dir(app_data_dir.clone())
        .expect("compose headless state from temp AppData");
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game_root.clone())
        .expect("save validated temp game directory");
    let import_task = state
        .mod_import_tasks
        .start_import_mod_task(StartImportModTaskRequest {
            archive_path: archive_path.clone(),
        })
        .expect("register armor fixture import");
    state
        .mod_import_task_runner
        .run_prepare_task(&import_task.task_id, archive_path)
        .expect("prepare armor fixture import");

    let profile_id = ProfileId::new("default");
    let mod_id = ModId::new(import_task.task_id);
    let initial = StartRetargetInstallTaskRequest {
        game_id: GameId::mhw(),
        profile_id: profile_id.clone(),
        mod_id: mod_id.clone(),
        target_id: ReplacementTargetId::parse("mhw:armor:fatalis-alpha")
            .expect("initial target id"),
        layer: FileLayer::new("base", 0),
    };
    let initial_task = state
        .retarget_install_tasks
        .start_retarget_install_task(initial.clone())
        .expect("register initial retarget task");
    state
        .retarget_install_task_runner
        .run_retarget_install_task(&initial_task.task_id, initial)
        .expect("run initial retarget install");

    let switch = RetargetReinstallRequest {
        game_id: GameId::mhw(),
        profile_id: profile_id.clone(),
        mod_id: mod_id.clone(),
        target_id: ReplacementTargetId::parse("mhw:armor:fatalis-beta").expect("switch target id"),
        layer: FileLayer::new("base", 0),
    };
    let preview = state
        .reinstall_executor
        .preview_retarget_reinstall(switch.clone())
        .expect("preview retarget reinstall");
    assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
    assert_eq!(
        preview.counts,
        ReinstallTargetCounts {
            retained: 0,
            replaced: 0,
            added: 1,
            stale: 1,
        }
    );
    assert_no_retarget_staging(&app_data_dir);
    let plan_token = preview.plan_token.expect("ready retarget reinstall token");
    let request = StartRetargetReinstallTaskRequest {
        game_id: switch.game_id,
        profile_id: switch.profile_id,
        mod_id: switch.mod_id,
        target_id: switch.target_id,
        layer: switch.layer,
        plan_token,
    };
    let reinstall_task = state
        .reinstall_tasks
        .start_retarget_reinstall_task(request.clone())
        .expect("register retarget reinstall task");
    let events = state
        .reinstall_task_runner
        .run_retarget_reinstall_task(&reinstall_task.task_id, request)
        .expect("run retarget reinstall task");
    assert_eq!(
        events
            .iter()
            .map(|event| event.phase.as_str())
            .collect::<Vec<_>>(),
        [
            "install.reinstall.plan.building",
            "install.reinstall.preflight.processing",
            "install.reinstall.commit.processing",
            "install.reinstall.completed",
        ]
    );
    assert!(!game_root.join(ARMOR_RETARGETED_TARGET).exists());
    assert_eq!(
        fs::read(game_root.join(ARMOR_SWITCH_TARGET)).expect("read switched armor target"),
        ARMOR_FIXTURE_BYTES
    );
    assert!(!game_root.join(ARMOR_SOURCE_TARGET).exists());
    assert_no_recovery_records(&app_data_dir);
    assert_no_reinstall_recovery_transactions(&app_data_dir);
    assert_no_retarget_staging(&app_data_dir);

    let switched_manifest = read_fixture_manifest(&app_data_dir);
    assert_eq!(switched_manifest.entries.len(), 1);
    assert_eq!(
        switched_manifest.entries[0].target_path.as_str(),
        ARMOR_SWITCH_TARGET
    );
    assert!(switched_manifest.entries[0].revision_id.is_some());
    assert_eq!(switched_manifest.replacement_bindings.len(), 1);
    let switched_binding = &switched_manifest.replacement_bindings[0];
    assert_eq!(
        switched_binding.binding().target_id().as_str(),
        "mhw:armor:fatalis-beta"
    );
    assert_eq!(switched_binding.target_internal_id(), "pl129_0010");
    assert_eq!(
        switched_binding.revision_id(),
        switched_manifest.entries[0].revision_id.as_ref()
    );

    drop(state);
    let restarted = HmmRuntime::from_app_data_dir(app_data_dir.clone())
        .expect("restart from switched retarget manifest");
    assert_eq!(read_fixture_manifest(&app_data_dir), switched_manifest);
    let uninstall = StartUninstallTaskRequest {
        game_id: GameId::mhw(),
        mod_id: mod_id.clone(),
        profile_id: profile_id.clone(),
    };
    let uninstall_task = restarted
        .uninstall_tasks
        .start_uninstall_task(uninstall.clone())
        .expect("register retarget uninstall task");
    restarted
        .uninstall_task_runner
        .run_uninstall_task(&uninstall_task.task_id, uninstall)
        .expect("uninstall switched retarget");
    assert_eq!(snapshot_file_tree(&game_root), baseline);
    assert!(!game_root.join(ARMOR_RETARGETED_TARGET).exists());
    assert!(!game_root.join(ARMOR_SOURCE_TARGET).exists());
    assert_no_recovery_records(&app_data_dir);
    assert_no_reinstall_recovery_transactions(&app_data_dir);

    drop(restarted);
    let restarted_uninstalled = HmmRuntime::from_app_data_dir(app_data_dir.clone())
        .expect("restart after retarget uninstall");
    let summaries = restarted_uninstalled
        .install_recovery_scanner
        .scan(
            GameId::mhw(),
            InstallRecoveryScanRequest {
                profile_id,
                mod_ids: vec![mod_id],
            },
        )
        .expect("scan final retarget uninstall status");
    assert_eq!(summaries[0].status, InstallRecoveryStatus::NotInstalled);

    let audit_events = read_install_audit_events(&app_data_dir);
    let switch_audit = audit_events
        .iter()
        .find(|event| {
            event.operation == "reinstall_mod"
                && event.fields.get("target_id").map(String::as_str)
                    == Some("mhw:armor:fatalis-beta")
        })
        .expect("retarget reinstall audit event");
    assert_eq!(switch_audit.result, "success");
    assert_eq!(switch_audit.fields["added_count"], "1");
    assert_eq!(switch_audit.fields["stale_count"], "1");
}

#[test]
fn headless_composition_installs_restarts_uninstalls_and_restores_baseline() {
    let temp = tempfile::tempdir().expect("create lifecycle temp root");
    let app_data_dir = temp.path().join("app-data");
    let game_root = temp.path().join("game");
    let archive_path = temp.path().join("lifecycle-v1.zip");
    prepare_game_root(&game_root);
    create_fixture_zip(&archive_path, V1_FILES);
    let baseline = snapshot_file_tree(&game_root);

    let state = HmmRuntime::from_app_data_dir(app_data_dir.clone())
        .expect("compose headless state from temp AppData");
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game_root.clone())
        .expect("save validated temp game directory");

    let import_task = state
        .mod_import_tasks
        .start_import_mod_task(StartImportModTaskRequest {
            archive_path: archive_path.clone(),
        })
        .expect("register fixture import task");
    state
        .mod_import_task_runner
        .run_prepare_task(&import_task.task_id, archive_path)
        .expect("prepare and persist fixture import");

    let profile_id = ProfileId::new("default");
    let mod_id = ModId::new(import_task.task_id);
    let install_request = StartInstallTaskRequest {
        game_id: GameId::mhw(),
        mod_id: mod_id.clone(),
        profile_id: profile_id.clone(),
        layer: FileLayer::new("base", 0),
    };
    let install_task = state
        .install_tasks
        .start_install_task(install_request.clone())
        .expect("register fixture install task");
    assert_eq!(install_task.kind, TaskKind::Install);
    assert_eq!(install_task.status, TaskStatus::Queued);

    let install_events = state
        .install_task_runner
        .run_install_task(&install_task.task_id, install_request)
        .expect("install fixture through HmmRuntime composition");
    assert!(install_events
        .iter()
        .all(|event| event.task_id == install_task.task_id));
    assert_eq!(
        install_events
            .iter()
            .map(|event| event.phase.as_str())
            .collect::<Vec<_>>(),
        [
            "install.plan.building",
            "install.commit.processing",
            "install.completed",
        ]
    );
    assert_eq!(
        state.task_manager.task_status(&install_task.task_id),
        Some(TaskStatus::Completed)
    );

    for (logical_path, expected_bytes) in V1_FILES {
        assert_eq!(
            fs::read(game_root.join(logical_path)).expect("read installed fixture target"),
            *expected_bytes,
            "installed bytes differ for {logical_path}"
        );
    }

    let manifest_path = app_data_dir
        .join("install")
        .join("manifests")
        .join("default.json");
    let manifest: InstallManifest = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("read persisted fixture manifest"),
    )
    .expect("deserialize persisted fixture manifest");
    assert_eq!(manifest.entries.len(), V1_FILES.len());
    assert_eq!(
        manifest
            .entries
            .iter()
            .filter(|entry| entry.backup_ref.is_some())
            .count(),
        1
    );
    assert!(manifest.entries.iter().any(|entry| {
        entry.target_path.as_str() == OVERWRITTEN_TARGET && entry.backup_ref.is_some()
    }));
    assert_no_recovery_records(&app_data_dir);

    drop(state);

    let restarted = HmmRuntime::from_app_data_dir(app_data_dir.clone())
        .expect("recompose installed fixture state from persisted AppData");
    let manifest_statuses = restarted
        .install_manifest_query
        .query_statuses(InstallManifestQueryRequest {
            profile_id: profile_id.clone(),
            mod_ids: vec![mod_id.clone()],
        })
        .expect("query installed fixture manifest status");
    assert_eq!(manifest_statuses.len(), 1);
    assert_eq!(
        manifest_statuses[0].status,
        InstallManifestStatus::Installed
    );
    assert_eq!(manifest_statuses[0].managed_file_count, V1_FILES.len());
    assert_eq!(manifest_statuses[0].backup_count, 1);

    let recovery_statuses = restarted
        .install_recovery_scanner
        .scan(
            GameId::mhw(),
            InstallRecoveryScanRequest {
                profile_id: profile_id.clone(),
                mod_ids: vec![mod_id.clone()],
            },
        )
        .expect("scan installed fixture recovery status");
    assert_eq!(recovery_statuses.len(), 1);
    assert_eq!(
        recovery_statuses[0].status,
        InstallRecoveryStatus::Completed
    );
    assert_eq!(recovery_statuses[0].managed_file_count, V1_FILES.len());
    assert_eq!(recovery_statuses[0].backup_count, 1);
    assert_eq!(recovery_statuses[0].issue_count, 0);

    let uninstall_request = StartUninstallTaskRequest {
        game_id: GameId::mhw(),
        mod_id: mod_id.clone(),
        profile_id: profile_id.clone(),
    };
    let uninstall_task = restarted
        .uninstall_tasks
        .start_uninstall_task(uninstall_request.clone())
        .expect("register fixture uninstall task");
    assert_eq!(uninstall_task.kind, TaskKind::Install);
    assert_eq!(uninstall_task.status, TaskStatus::Queued);

    let uninstall_events = restarted
        .uninstall_task_runner
        .run_uninstall_task(&uninstall_task.task_id, uninstall_request)
        .expect("uninstall fixture through HmmRuntime composition");
    assert!(uninstall_events
        .iter()
        .all(|event| event.task_id == uninstall_task.task_id));
    assert_eq!(
        uninstall_events
            .iter()
            .map(|event| event.phase.as_str())
            .collect::<Vec<_>>(),
        [
            "install.uninstall.processing",
            "install.uninstall.completed",
        ]
    );
    assert_eq!(snapshot_file_tree(&game_root), baseline);
    assert_no_recovery_records(&app_data_dir);

    let install_audit_events = read_install_audit_events(&app_data_dir);
    assert_eq!(install_audit_events.len(), 2);
    let install_audit = install_audit_events
        .iter()
        .find(|event| event.operation == "commit_imported_mod")
        .expect("install success audit event");
    assert_eq!(install_audit.result, "success");
    assert_eq!(
        &install_audit.fields,
        &BTreeMap::from([
            ("action_count".to_owned(), V1_FILES.len().to_string()),
            ("game_id".to_owned(), "mhw".to_owned()),
            ("mod_id".to_owned(), mod_id.as_str().to_owned()),
            ("profile_id".to_owned(), "default".to_owned()),
            ("task_id".to_owned(), install_task.task_id.clone()),
        ])
    );

    let uninstall_audit = install_audit_events
        .iter()
        .find(|event| event.operation == "uninstall_mod")
        .expect("uninstall success audit event");
    assert_eq!(uninstall_audit.result, "success");
    assert_eq!(
        &uninstall_audit.fields,
        &BTreeMap::from([
            ("game_id".to_owned(), "mhw".to_owned()),
            ("mod_id".to_owned(), mod_id.as_str().to_owned()),
            ("profile_id".to_owned(), "default".to_owned()),
            ("removed_file_count".to_owned(), "3".to_owned()),
            ("restored_file_count".to_owned(), "1".to_owned()),
            ("task_id".to_owned(), uninstall_task.task_id.clone()),
        ])
    );

    let serialized_public_evidence =
        serde_json::to_string(&install_audit_events).expect("serialize public audit evidence");
    let pending_backup_ref = manifest
        .entries
        .iter()
        .find_map(|entry| entry.backup_ref.as_deref())
        .expect("fixture overwrite backup ref");
    for forbidden in [
        game_root.to_string_lossy().into_owned(),
        app_data_dir.to_string_lossy().into_owned(),
        manifest_path.to_string_lossy().into_owned(),
        pending_backup_ref.to_owned(),
        OVERWRITTEN_TARGET.to_owned(),
        "sandbox".to_owned(),
        "installedFile".to_owned(),
    ] {
        assert!(
            !serialized_public_evidence.contains(&forbidden),
            "public lifecycle evidence must not expose {forbidden}"
        );
    }

    drop(restarted);

    let restarted_after_uninstall = HmmRuntime::from_app_data_dir(app_data_dir.clone())
        .expect("recompose uninstalled fixture state from persisted AppData");
    let manifest_statuses = restarted_after_uninstall
        .install_manifest_query
        .query_statuses(InstallManifestQueryRequest {
            profile_id: profile_id.clone(),
            mod_ids: vec![mod_id.clone()],
        })
        .expect("query uninstalled fixture manifest status");
    assert_eq!(manifest_statuses.len(), 1);
    assert_eq!(
        manifest_statuses[0].status,
        InstallManifestStatus::NotInstalled
    );
    assert_eq!(manifest_statuses[0].managed_file_count, 0);
    assert_eq!(manifest_statuses[0].backup_count, 0);

    let recovery_statuses = restarted_after_uninstall
        .install_recovery_scanner
        .scan(
            GameId::mhw(),
            InstallRecoveryScanRequest {
                profile_id,
                mod_ids: vec![mod_id],
            },
        )
        .expect("scan uninstalled fixture recovery status");
    assert_eq!(recovery_statuses.len(), 1);
    assert_eq!(
        recovery_statuses[0].status,
        InstallRecoveryStatus::NotInstalled
    );
    assert_eq!(recovery_statuses[0].managed_file_count, 0);
    assert_eq!(recovery_statuses[0].backup_count, 0);
    assert_eq!(recovery_statuses[0].issue_count, 0);
    assert_eq!(snapshot_file_tree(&game_root), baseline);
    assert_no_recovery_records(&app_data_dir);
}

#[test]
fn headless_composition_reinstalls_v1_to_v2_and_restores_baseline() {
    let temp = tempfile::tempdir().expect("create reinstall lifecycle temp root");
    let app_data_dir = temp.path().join("app-data");
    let game_root = temp.path().join("game");
    let v1_archive_path = temp.path().join("lifecycle-v1.zip");
    let v2_archive_path = temp.path().join("lifecycle-v2.zip");
    prepare_game_root(&game_root);
    create_fixture_zip(&v1_archive_path, V1_FILES);
    create_fixture_zip(&v2_archive_path, V2_FILES);
    let baseline = snapshot_file_tree(&game_root);

    let state = HmmRuntime::from_app_data_dir(app_data_dir.clone())
        .expect("compose reinstall lifecycle state from temp AppData");
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game_root.clone())
        .expect("save validated temp game directory");

    let (v1_import_task_id, mod_id, v1_revision_id) =
        import_initial_fixture_revision(&state, &v1_archive_path);
    let profile_id = ProfileId::new("default");
    let install_task_id = install_fixture_revision(&state, &mod_id, &profile_id);
    let v1_manifest = read_fixture_manifest(&app_data_dir);
    assert_manifest_entries(&v1_manifest, &mod_id, None, V1_FILES);
    let original_backup_ref = original_overwrite_backup_ref(&v1_manifest, &mod_id);
    assert_no_recovery_records(&app_data_dir);

    drop(state);

    let restarted_v1 = HmmRuntime::from_app_data_dir(app_data_dir.clone())
        .expect("recompose installed v1 state from persisted AppData");
    assert_manifest_status(
        &restarted_v1,
        &profile_id,
        &mod_id,
        InstallManifestStatus::Installed,
        V1_FILES.len(),
        1,
    );
    let restarted_v1_revisions = restarted_v1
        .mod_library
        .get_mod_revisions(&mod_id)
        .expect("query persisted v1 revision catalog")
        .expect("logical Mod remains after restart");
    assert_eq!(
        restarted_v1_revisions.revision_ids.as_slice(),
        std::slice::from_ref(&v1_revision_id)
    );
    assert_eq!(
        restarted_v1.task_manager.task_status(&v1_import_task_id),
        None
    );
    assert_eq!(
        restarted_v1.task_manager.task_status(&install_task_id),
        None
    );

    let (v2_import_task_id, v2_revision_id) = import_candidate_fixture_revision(
        &restarted_v1,
        &v2_archive_path,
        &mod_id,
        &v1_revision_id,
    );
    assert_eq!(library_ids(&restarted_v1), [mod_id.as_str().to_owned()]);
    let revisions = restarted_v1
        .mod_library
        .get_mod_revisions(&mod_id)
        .expect("query v1 and v2 revision catalog")
        .expect("logical Mod exists after revision import");
    assert_eq!(revisions.mod_id, mod_id);
    assert_eq!(revisions.origin_revision_id, v1_revision_id);
    assert_eq!(revisions.revision_ids.len(), 2);
    assert!(revisions.revision_ids.contains(&v1_revision_id));
    assert!(revisions.revision_ids.contains(&v2_revision_id));

    let preview = preview_fixture_reinstall(&restarted_v1, &profile_id, &mod_id, &v2_revision_id);
    assert_ready_preview(&preview, &v1_revision_id, &v2_revision_id);
    let plan_token = preview
        .plan_token
        .expect("ready preview exposes opaque token");
    let reinstall_request = StartReinstallTaskRequest {
        game_id: GameId::mhw(),
        profile_id: profile_id.clone(),
        mod_id: mod_id.clone(),
        candidate_revision_id: v2_revision_id.clone(),
        layer: FileLayer::new("base", 0),
        plan_token,
    };
    let reinstall_task = restarted_v1
        .reinstall_tasks
        .start_reinstall_task(reinstall_request.clone())
        .expect("register v1 to v2 reinstall task");
    let reinstall_events = restarted_v1
        .reinstall_task_runner
        .run_reinstall_task(&reinstall_task.task_id, reinstall_request)
        .expect("reinstall v2 through HmmRuntime composition");
    assert_eq!(
        reinstall_events
            .iter()
            .map(|event| event.phase.as_str())
            .collect::<Vec<_>>(),
        [
            "install.reinstall.plan.building",
            "install.reinstall.preflight.processing",
            "install.reinstall.commit.processing",
            "install.reinstall.completed",
        ]
    );
    assert_eq!(
        restarted_v1
            .task_manager
            .task_status(&reinstall_task.task_id),
        Some(TaskStatus::Completed)
    );
    assert_fixture_bytes(&game_root, V2_FILES);
    assert!(!game_root.join("nativePC/lifecycle/stale.bin").exists());

    let v2_manifest = read_fixture_manifest(&app_data_dir);
    assert_manifest_entries(&v2_manifest, &mod_id, Some(&v2_revision_id), V2_FILES);
    assert_eq!(
        original_overwrite_backup_ref(&v2_manifest, &mod_id),
        original_backup_ref,
        "reinstall must preserve the pre-v1 original backup"
    );
    assert_eq!(
        v2_manifest
            .entries
            .iter()
            .filter(|entry| entry.mod_id == mod_id && entry.backup_ref.is_some())
            .count(),
        1
    );
    assert_no_recovery_records(&app_data_dir);

    let reinstall_audit_events = read_install_audit_events(&app_data_dir);
    let reinstall_audit = reinstall_audit_events
        .iter()
        .find(|event| event.operation == "reinstall_mod")
        .expect("reinstall success audit event");
    assert_eq!(reinstall_audit.result, "success");
    assert_eq!(
        reinstall_audit.fields["previous_revision_id"],
        v1_revision_id.as_str()
    );
    assert_eq!(
        reinstall_audit.fields["candidate_revision_id"],
        v2_revision_id.as_str()
    );
    assert_eq!(reinstall_audit.fields["retained_count"], "1");
    assert_eq!(reinstall_audit.fields["replaced_count"], "2");
    assert_eq!(reinstall_audit.fields["added_count"], "1");
    assert_eq!(reinstall_audit.fields["stale_count"], "1");
    assert_audit_evidence_redacted(
        &reinstall_audit_events,
        &game_root,
        &app_data_dir,
        &original_backup_ref,
    );

    drop(restarted_v1);

    let restarted_v2 = HmmRuntime::from_app_data_dir(app_data_dir.clone())
        .expect("recompose installed v2 state from persisted AppData");
    assert_manifest_status(
        &restarted_v2,
        &profile_id,
        &mod_id,
        InstallManifestStatus::Installed,
        V2_FILES.len(),
        1,
    );
    let reverse_preview =
        preview_fixture_reinstall(&restarted_v2, &profile_id, &mod_id, &v1_revision_id);
    assert_ready_preview(&reverse_preview, &v2_revision_id, &v1_revision_id);
    for transient_task_id in [
        &v1_import_task_id,
        &install_task_id,
        &v2_import_task_id,
        &reinstall_task.task_id,
    ] {
        assert_eq!(
            restarted_v2.task_manager.task_status(transient_task_id),
            None,
            "restart facts must not depend on transient TaskManager state"
        );
    }

    let uninstall_request = StartUninstallTaskRequest {
        game_id: GameId::mhw(),
        mod_id: mod_id.clone(),
        profile_id: profile_id.clone(),
    };
    let uninstall_task = restarted_v2
        .uninstall_tasks
        .start_uninstall_task(uninstall_request.clone())
        .expect("register v2 uninstall task");
    restarted_v2
        .uninstall_task_runner
        .run_uninstall_task(&uninstall_task.task_id, uninstall_request)
        .expect("uninstall v2 through HmmRuntime composition");
    assert_eq!(snapshot_file_tree(&game_root), baseline);
    assert!(!game_root.join("nativePC/lifecycle/added-v2.bin").exists());
    assert_eq!(
        fs::read(game_root.join(OVERWRITTEN_TARGET))
            .expect("read restored overwritten baseline target"),
        BASELINE_BYTES
    );
    assert_no_recovery_records(&app_data_dir);

    drop(restarted_v2);

    let restarted_uninstalled = HmmRuntime::from_app_data_dir(app_data_dir.clone())
        .expect("recompose uninstalled v2 state from persisted AppData");
    assert_manifest_status(
        &restarted_uninstalled,
        &profile_id,
        &mod_id,
        InstallManifestStatus::NotInstalled,
        0,
        0,
    );
    let recovery_statuses = restarted_uninstalled
        .install_recovery_scanner
        .scan(
            GameId::mhw(),
            InstallRecoveryScanRequest {
                profile_id,
                mod_ids: vec![mod_id],
            },
        )
        .expect("scan final uninstalled recovery status");
    assert_eq!(recovery_statuses.len(), 1);
    assert_eq!(
        recovery_statuses[0].status,
        InstallRecoveryStatus::NotInstalled
    );
    assert_eq!(recovery_statuses[0].issue_count, 0);
    assert_eq!(snapshot_file_tree(&game_root), baseline);
    assert_no_recovery_records(&app_data_dir);
}

#[test]
fn headless_composition_rolls_back_v1_when_reinstall_manifest_save_fails() {
    let temp = tempfile::tempdir().expect("create reinstall rollback temp root");
    let app_data_dir = temp.path().join("app-data");
    let game_root = temp.path().join("game");
    let v1_archive_path = temp.path().join("lifecycle-v1.zip");
    let v2_archive_path = temp.path().join("lifecycle-v2.zip");
    prepare_game_root(&game_root);
    create_fixture_zip(&v1_archive_path, V1_FILES);
    create_fixture_zip(&v2_archive_path, V2_FILES);

    let manifest_repository = Arc::new(FailNextManifestSaveRepository::new(
        app_data_dir.join("install").join("manifests"),
    ));
    let state = HmmRuntime::builder(app_data_dir.clone())
        .with_install_manifest_repository(manifest_repository.clone())
        .build()
        .expect("compose fault-injected runtime from temp AppData");
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game_root.clone())
        .expect("save validated temp game directory");

    let (v1_import_task_id, mod_id, v1_revision_id) =
        import_initial_fixture_revision(&state, &v1_archive_path);
    let profile_id = ProfileId::new("default");
    let install_task_id = install_fixture_revision(&state, &mod_id, &profile_id);
    let v1_manifest = read_fixture_manifest(&app_data_dir);
    let v1_game_snapshot = snapshot_file_tree(&game_root);
    let original_backup_ref = original_overwrite_backup_ref(&v1_manifest, &mod_id);
    let (v2_import_task_id, v2_revision_id) =
        import_candidate_fixture_revision(&state, &v2_archive_path, &mod_id, &v1_revision_id);
    let preview = preview_fixture_reinstall(&state, &profile_id, &mod_id, &v2_revision_id);
    assert_ready_preview(&preview, &v1_revision_id, &v2_revision_id);
    let reinstall_request = StartReinstallTaskRequest {
        game_id: GameId::mhw(),
        profile_id: profile_id.clone(),
        mod_id: mod_id.clone(),
        candidate_revision_id: v2_revision_id.clone(),
        layer: FileLayer::new("base", 0),
        plan_token: preview
            .plan_token
            .expect("ready preview exposes opaque token"),
    };
    let reinstall_task = state
        .reinstall_tasks
        .start_reinstall_task(reinstall_request.clone())
        .expect("register fault-injected reinstall task");
    manifest_repository.fail_next_save();
    let failure = state
        .reinstall_task_runner
        .run_reinstall_task(&reinstall_task.task_id, reinstall_request)
        .expect_err("manifest save failure must fail the reinstall task");
    assert_eq!(
        failure
            .events
            .iter()
            .map(|event| event.phase.as_str())
            .collect::<Vec<_>>(),
        [
            "install.reinstall.plan.building",
            "install.reinstall.preflight.processing",
            "install.reinstall.commit.processing",
            "install.reinstall.rollback.processing",
            "install.reinstall.failed",
        ]
    );
    assert_eq!(
        failure
            .events
            .last()
            .and_then(|event| event.error.as_deref()),
        Some("install_reinstall_failed:manifest")
    );
    assert_eq!(
        state.task_manager.task_status(&reinstall_task.task_id),
        Some(TaskStatus::Failed)
    );
    assert_eq!(snapshot_file_tree(&game_root), v1_game_snapshot);
    assert_eq!(read_fixture_manifest(&app_data_dir), v1_manifest);
    assert_manifest_entries(&v1_manifest, &mod_id, None, V1_FILES);
    assert_no_recovery_records(&app_data_dir);
    assert!(state
        .reinstall_recovery_repository
        .list_transactions(&profile_id)
        .expect("query active reinstall recovery transactions")
        .is_empty());

    let audit_events = read_install_audit_events(&app_data_dir);
    let failed_reinstall_audit = audit_events
        .iter()
        .find(|event| event.operation == "reinstall_mod")
        .expect("reinstall failure audit event");
    assert_eq!(failed_reinstall_audit.result, "failure");
    assert_eq!(
        failed_reinstall_audit.fields["error_code"],
        "install_reinstall_failed:manifest"
    );
    assert_eq!(
        failed_reinstall_audit.fields["rollback_result"],
        "rolled_back"
    );
    assert_audit_evidence_redacted(
        &audit_events,
        &game_root,
        &app_data_dir,
        &original_backup_ref,
    );

    drop(state);

    let restarted_v1 = HmmRuntime::from_app_data_dir(app_data_dir.clone())
        .expect("recompose rolled-back v1 state from persisted AppData");
    assert_manifest_status(
        &restarted_v1,
        &profile_id,
        &mod_id,
        InstallManifestStatus::Installed,
        V1_FILES.len(),
        1,
    );
    assert_fixture_bytes(&game_root, V1_FILES);
    assert!(!game_root.join("nativePC/lifecycle/added-v2.bin").exists());
    let retry_preview =
        preview_fixture_reinstall(&restarted_v1, &profile_id, &mod_id, &v2_revision_id);
    assert_ready_preview(&retry_preview, &v1_revision_id, &v2_revision_id);
    let recovery_statuses = restarted_v1
        .install_recovery_scanner
        .scan(
            GameId::mhw(),
            InstallRecoveryScanRequest {
                profile_id: profile_id.clone(),
                mod_ids: vec![mod_id.clone()],
            },
        )
        .expect("scan rolled-back v1 recovery status");
    assert_eq!(recovery_statuses.len(), 1);
    assert_eq!(
        recovery_statuses[0].status,
        InstallRecoveryStatus::Completed
    );
    assert_eq!(recovery_statuses[0].managed_file_count, V1_FILES.len());
    assert_eq!(recovery_statuses[0].backup_count, 1);
    assert_eq!(recovery_statuses[0].issue_count, 0);
    assert!(restarted_v1
        .reinstall_recovery_repository
        .list_transactions(&profile_id)
        .expect("query restarted reinstall recovery transactions")
        .is_empty());
    for transient_task_id in [
        &v1_import_task_id,
        &install_task_id,
        &v2_import_task_id,
        &reinstall_task.task_id,
    ] {
        assert_eq!(
            restarted_v1.task_manager.task_status(transient_task_id),
            None
        );
    }
    assert_no_recovery_records(&app_data_dir);
}

fn fixture_map(
    files: &'static [(&'static str, &'static [u8])],
) -> BTreeMap<&'static str, &'static [u8]> {
    files.iter().copied().collect()
}

fn import_initial_fixture_revision(
    state: &HmmRuntime,
    archive_path: &Path,
) -> (String, ModId, ModRevisionId) {
    let task = state
        .mod_import_tasks
        .start_import_mod_task(StartImportModTaskRequest {
            archive_path: archive_path.to_path_buf(),
        })
        .expect("register initial fixture import task");
    state
        .mod_import_task_runner
        .run_prepare_task(&task.task_id, archive_path.to_path_buf())
        .expect("prepare and persist initial fixture revision");
    let mod_id = ModId::new(task.task_id.clone());
    let revisions = state
        .mod_library
        .get_mod_revisions(&mod_id)
        .expect("query initial fixture revision")
        .expect("initial logical Mod exists");
    assert_eq!(
        revisions.revision_ids.as_slice(),
        std::slice::from_ref(&revisions.origin_revision_id)
    );
    (task.task_id, mod_id, revisions.origin_revision_id)
}

fn import_candidate_fixture_revision(
    state: &HmmRuntime,
    archive_path: &Path,
    mod_id: &ModId,
    previous_revision_id: &ModRevisionId,
) -> (String, ModRevisionId) {
    let task = state
        .mod_import_tasks
        .start_import_mod_revision_task(StartImportModRevisionTaskRequest {
            archive_path: archive_path.to_path_buf(),
            mod_id: mod_id.clone(),
        })
        .expect("register candidate revision import task");
    state
        .mod_import_task_runner
        .run_prepare_revision_task(&task.task_id, archive_path.to_path_buf(), mod_id.clone())
        .expect("prepare and persist candidate fixture revision");
    let revisions = state
        .mod_library
        .get_mod_revisions(mod_id)
        .expect("query candidate fixture revisions")
        .expect("logical Mod exists after revision import");
    let candidates = revisions
        .revision_ids
        .iter()
        .filter(|revision_id| *revision_id != previous_revision_id)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        candidates.len(),
        1,
        "revision import must append exactly one candidate"
    );
    (task.task_id, candidates[0].clone())
}

fn install_fixture_revision(state: &HmmRuntime, mod_id: &ModId, profile_id: &ProfileId) -> String {
    let request = StartInstallTaskRequest {
        game_id: GameId::mhw(),
        mod_id: mod_id.clone(),
        profile_id: profile_id.clone(),
        layer: FileLayer::new("base", 0),
    };
    let task = state
        .install_tasks
        .start_install_task(request.clone())
        .expect("register initial fixture install task");
    state
        .install_task_runner
        .run_install_task(&task.task_id, request)
        .expect("install initial fixture revision");
    task.task_id
}

fn preview_fixture_reinstall(
    state: &HmmRuntime,
    profile_id: &ProfileId,
    mod_id: &ModId,
    candidate_revision_id: &ModRevisionId,
) -> ReinstallPlanPreview {
    state
        .reinstall_executor
        .preview(ReinstallPreviewRequest {
            game_id: GameId::mhw(),
            profile_id: profile_id.clone(),
            mod_id: mod_id.clone(),
            candidate_revision_id: candidate_revision_id.clone(),
            layer: FileLayer::new("base", 0),
        })
        .expect("preview fixture reinstall through HmmRuntime composition")
}

fn assert_ready_preview(
    preview: &ReinstallPlanPreview,
    installed_revision_id: &ModRevisionId,
    candidate_revision_id: &ModRevisionId,
) {
    assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
    assert_eq!(
        preview
            .installed_revision
            .as_ref()
            .map(|revision| &revision.revision_id),
        Some(installed_revision_id)
    );
    assert_eq!(
        preview
            .candidate_revision
            .as_ref()
            .map(|revision| &revision.revision_id),
        Some(candidate_revision_id)
    );
    assert_eq!(
        preview.counts,
        ReinstallTargetCounts {
            retained: 1,
            replaced: 2,
            added: 1,
            stale: 1,
        }
    );
    assert!(preview.blocking_reasons.is_empty());
    assert!(preview.plan_token.is_some());
}

fn read_fixture_manifest(app_data_dir: &Path) -> InstallManifest {
    serde_json::from_str(
        &fs::read_to_string(
            app_data_dir
                .join("install")
                .join("manifests")
                .join("default.json"),
        )
        .expect("read persisted fixture manifest"),
    )
    .expect("deserialize persisted fixture manifest")
}

fn assert_manifest_entries(
    manifest: &InstallManifest,
    mod_id: &ModId,
    revision_id: Option<&ModRevisionId>,
    files: &[(&str, &[u8])],
) {
    let matching_entries = manifest
        .entries
        .iter()
        .filter(|entry| entry.mod_id == *mod_id)
        .collect::<Vec<_>>();
    assert_eq!(
        matching_entries.len(),
        files.len(),
        "manifest must contain exactly the expected entry set"
    );
    let actual = matching_entries
        .into_iter()
        .map(|entry| {
            (
                entry.target_path.as_str().to_owned(),
                entry
                    .revision_id
                    .as_ref()
                    .map(|revision| revision.as_str().to_owned()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let expected = files
        .iter()
        .map(|(path, _)| {
            (
                (*path).to_owned(),
                revision_id.map(|revision| revision.as_str().to_owned()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual, expected);
}

fn original_overwrite_backup_ref(manifest: &InstallManifest, mod_id: &ModId) -> String {
    manifest
        .entries
        .iter()
        .find(|entry| entry.mod_id == *mod_id && entry.target_path.as_str() == OVERWRITTEN_TARGET)
        .and_then(|entry| entry.backup_ref.clone())
        .expect("overwritten target retains original backup ref")
}

fn assert_manifest_status(
    state: &HmmRuntime,
    profile_id: &ProfileId,
    mod_id: &ModId,
    expected_status: InstallManifestStatus,
    expected_managed_file_count: usize,
    expected_backup_count: usize,
) {
    let statuses = state
        .install_manifest_query
        .query_statuses(InstallManifestQueryRequest {
            profile_id: profile_id.clone(),
            mod_ids: vec![mod_id.clone()],
        })
        .expect("query fixture manifest status");
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].status, expected_status);
    assert_eq!(statuses[0].managed_file_count, expected_managed_file_count);
    assert_eq!(statuses[0].backup_count, expected_backup_count);
}

fn assert_fixture_bytes(game_root: &Path, files: &[(&str, &[u8])]) {
    for (logical_path, expected_bytes) in files {
        assert_eq!(
            fs::read(game_root.join(logical_path)).expect("read fixture target"),
            *expected_bytes,
            "fixture bytes differ for {logical_path}"
        );
    }
}

fn assert_audit_evidence_redacted(
    events: &[AuditLogEvent],
    game_root: &Path,
    app_data_dir: &Path,
    backup_ref: &str,
) {
    let evidence = serde_json::to_value(events).expect("serialize public audit evidence");
    for forbidden in [
        game_root.to_string_lossy().into_owned(),
        app_data_dir.to_string_lossy().into_owned(),
        backup_ref.to_owned(),
        OVERWRITTEN_TARGET.to_owned(),
        "sandbox".to_owned(),
        "installedFile".to_owned(),
    ] {
        assert!(
            !json_value_contains_forbidden(&evidence, &forbidden),
            "public lifecycle evidence must not expose {forbidden}"
        );
    }
}

fn json_value_contains_forbidden(value: &serde_json::Value, forbidden: &str) -> bool {
    match value {
        serde_json::Value::String(text) => text.contains(forbidden),
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| json_value_contains_forbidden(value, forbidden)),
        serde_json::Value::Object(fields) => fields.iter().any(|(key, value)| {
            key.contains(forbidden) || json_value_contains_forbidden(value, forbidden)
        }),
        _ => false,
    }
}

struct FailNextManifestSaveRepository {
    delegate: JsonInstallManifestRepository,
    fail_next_save: AtomicBool,
}

impl FailNextManifestSaveRepository {
    fn new(manifest_root: PathBuf) -> Self {
        Self {
            delegate: JsonInstallManifestRepository::new(manifest_root),
            fail_next_save: AtomicBool::new(false),
        }
    }

    fn fail_next_save(&self) {
        self.fail_next_save.store(true, Ordering::SeqCst);
    }
}

impl InstallManifestRepository for FailNextManifestSaveRepository {
    fn load_manifest(&self, profile_id: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        self.delegate.load_manifest(profile_id)
    }

    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        if self.fail_next_save.swap(false, Ordering::SeqCst) {
            anyhow::bail!("injected manifest save failure");
        }
        self.delegate.save_manifest(manifest)
    }
}

fn prepare_game_root(game_root: &Path) {
    prepare_game_root_without_prerequisites(game_root);
    let plugin_root = game_root.join("nativePC/plugins");
    fs::create_dir_all(&plugin_root).expect("create prerequisite plugin fixture");
    for relative_path in [
        "dinput8.dll",
        "loader.dll",
        "nativePC/plugins/MonsterLoader.dll",
        "nativePC/plugins/QuestLoader.dll",
        "nativePC/plugins/!CRCBypass.dll",
    ] {
        fs::write(
            game_root.join(relative_path),
            b"synthetic prerequisite fixture\n",
        )
        .expect("write synthetic prerequisite fixture");
    }
    fs::write(
        game_root.join("loader-config.json"),
        br#"{"enablePluginLoader":true}"#,
    )
    .expect("write synthetic prerequisite config");
}

fn prepare_game_root_without_prerequisites(game_root: &Path) {
    fs::create_dir_all(game_root.join("nativePC/lifecycle")).expect("create temp game root");
    fs::write(
        game_root.join("MonsterHunterWorld.exe"),
        b"fixture executable\n",
    )
    .expect("write synthetic game executable");
    fs::write(game_root.join(OVERWRITTEN_TARGET), BASELINE_BYTES)
        .expect("write original game baseline");
}

fn create_fixture_zip(path: &Path, files: &[(&str, &[u8])]) {
    let file = File::create(path).expect("create synthetic fixture archive");
    let mut archive = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    for (logical_path, bytes) in files {
        archive
            .start_file(*logical_path, options)
            .expect("start synthetic fixture entry");
        archive
            .write_all(bytes)
            .expect("write synthetic fixture entry");
    }

    archive.finish().expect("finish synthetic fixture archive");
}

fn library_ids(state: &HmmRuntime) -> Vec<String> {
    state
        .mod_library
        .get_mod_library()
        .expect("load persisted fixture library")
        .into_iter()
        .map(|item| item.id)
        .collect()
}

fn build_target_paths(state: &HmmRuntime, mod_id: ModId) -> Vec<String> {
    let plan = state
        .install_planning
        .build_plan_from_imported_mod(BuildImportedModInstallPlanRequest {
            game_id: GameId::mhw(),
            mod_id,
            layer: FileLayer::new("base", 0),
        })
        .expect("build fixture InstallPlan from persisted import");

    assert!(plan.conflicts.is_empty());
    plan.actions
        .into_iter()
        .map(|action| action.target_path.as_str().to_owned())
        .collect()
}

fn expected_v1_targets() -> Vec<String> {
    V1_FILES
        .iter()
        .map(|(path, _)| (*path).to_owned())
        .collect()
}

fn assert_game_root_unchanged(game_root: &Path) {
    assert_eq!(
        fs::read(game_root.join(OVERWRITTEN_TARGET)).expect("read original baseline"),
        BASELINE_BYTES
    );

    for (logical_path, _) in V1_FILES {
        if *logical_path != OVERWRITTEN_TARGET {
            assert!(
                !game_root.join(logical_path).exists(),
                "CL0 planning harness must not write {logical_path}"
            );
        }
    }
}

fn snapshot_file_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn collect(root: &Path, current: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = fs::read_dir(current)
            .expect("read fixture directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read fixture directory entries");
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type().expect("inspect fixture entry type");
            assert!(
                !file_type.is_symlink(),
                "fixture tree must not contain links"
            );
            if file_type.is_dir() {
                collect(root, &path, files);
            } else if file_type.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .expect("fixture entry stays below root")
                    .to_string_lossy()
                    .replace('\\', "/");
                files.insert(relative, fs::read(path).expect("read fixture file bytes"));
            }
        }
    }

    let mut files = BTreeMap::new();
    collect(root, root, &mut files);
    files
}

fn assert_no_recovery_records(app_data_dir: &Path) {
    let recovery_root = app_data_dir.join("install").join("recovery");
    if recovery_root.exists() {
        assert_eq!(
            fs::read_dir(recovery_root)
                .expect("read fixture recovery directory")
                .count(),
            0,
            "fixture recovery records must be cleared"
        );
    }
}

fn assert_no_reinstall_recovery_transactions(app_data_dir: &Path) {
    let recovery_root = app_data_dir.join("install").join("reinstall-recovery");
    if recovery_root.exists() {
        assert_eq!(
            fs::read_dir(recovery_root)
                .expect("read reinstall recovery directory")
                .count(),
            0,
            "reinstall recovery transactions must be cleared"
        );
    }
}

fn assert_no_retarget_staging(app_data_dir: &Path) {
    let staging_root = app_data_dir.join("install").join("retarget-staging");
    if staging_root.exists() {
        assert_eq!(
            fs::read_dir(staging_root)
                .expect("read retarget staging directory")
                .count(),
            0,
            "retarget reinstall staging must be discarded"
        );
    }
}

fn read_install_audit_events(app_data_dir: &Path) -> Vec<AuditLogEvent> {
    FileSystemAuditLogWriter::new(app_data_dir.to_path_buf())
        .read_recent_sanitized(AuditLogReadRequest { max_events: 100 })
        .expect("read sanitized fixture audit events")
        .into_iter()
        .filter(|event| event.category == "install")
        .collect()
}
