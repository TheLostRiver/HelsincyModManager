use super::AppState;
use hmm_app::{
    BuildImportedModInstallPlanRequest, InstallManifestQueryRequest, InstallManifestStatus,
    InstallRecoveryScanRequest, InstallRecoveryStatus, StartImportModTaskRequest,
    StartInstallTaskRequest, StartUninstallTaskRequest, TaskKind, TaskStatus,
};
use hmm_core::{FileLayer, GameDirectoryStatus, GameId, InstallManifest, ModId, ProfileId};
use hmm_infra::FileSystemAuditLogWriter;
use hmm_ports::{AuditLogEvent, AuditLogReadRequest, AuditLogReader};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;

const OVERWRITTEN_TARGET: &str = "nativePC/lifecycle/overwritten.bin";
const BASELINE_BYTES: &[u8] = b"game-baseline-original\n";

const V1_FILES: &[(&str, &[u8])] = &[
    (OVERWRITTEN_TARGET, b"fixture-overwrite-v1\n"),
    ("nativePC/lifecycle/replaced.bin", b"fixture-replaced-v1\n"),
    ("nativePC/lifecycle/retained.bin", b"fixture-retained\n"),
    ("nativePC/lifecycle/stale.bin", b"fixture-stale-v1\n"),
];

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
fn headless_composition_imports_v1_and_rebuilds_plan_after_restart() {
    let temp = tempfile::tempdir().expect("create lifecycle temp root");
    let app_data_dir = temp.path().join("app-data");
    let game_root = temp.path().join("game");
    let archive_path = temp.path().join("lifecycle-v1.zip");
    prepare_game_root(&game_root);
    create_fixture_zip(&archive_path, V1_FILES);

    let state = AppState::from_app_data_dir(app_data_dir.clone())
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

    let restarted = AppState::from_app_data_dir(app_data_dir)
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
fn headless_composition_installs_restarts_uninstalls_and_restores_baseline() {
    let temp = tempfile::tempdir().expect("create lifecycle temp root");
    let app_data_dir = temp.path().join("app-data");
    let game_root = temp.path().join("game");
    let archive_path = temp.path().join("lifecycle-v1.zip");
    prepare_game_root(&game_root);
    create_fixture_zip(&archive_path, V1_FILES);
    let baseline = snapshot_file_tree(&game_root);

    let state = AppState::from_app_data_dir(app_data_dir.clone())
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
        .expect("install fixture through AppState composition");
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

    let restarted = AppState::from_app_data_dir(app_data_dir.clone())
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
        .expect("uninstall fixture through AppState composition");
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

    let restarted_after_uninstall = AppState::from_app_data_dir(app_data_dir.clone())
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

fn fixture_map(
    files: &'static [(&'static str, &'static [u8])],
) -> BTreeMap<&'static str, &'static [u8]> {
    files.iter().copied().collect()
}

fn prepare_game_root(game_root: &Path) {
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

fn library_ids(state: &AppState) -> Vec<String> {
    state
        .mod_library
        .get_mod_library()
        .expect("load persisted fixture library")
        .into_iter()
        .map(|item| item.id)
        .collect()
}

fn build_target_paths(state: &AppState, mod_id: ModId) -> Vec<String> {
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

fn read_install_audit_events(app_data_dir: &Path) -> Vec<AuditLogEvent> {
    FileSystemAuditLogWriter::new(app_data_dir.to_path_buf())
        .read_recent_sanitized(AuditLogReadRequest { max_events: 100 })
        .expect("read sanitized fixture audit events")
        .into_iter()
        .filter(|event| event.category == "install")
        .collect()
}
