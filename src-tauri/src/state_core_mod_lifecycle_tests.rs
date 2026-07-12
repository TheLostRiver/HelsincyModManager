use super::AppState;
use hmm_app::{
    BuildImportedModInstallPlanRequest, StartImportModTaskRequest, TaskKind, TaskStatus,
};
use hmm_core::{FileLayer, GameDirectoryStatus, GameId, ModId};
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
