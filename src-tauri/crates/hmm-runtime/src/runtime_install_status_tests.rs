use super::*;
use hmm_app::{ModLibraryFilter, ModLibraryProfileContext, ModLibraryQuery};

fn assert_library_status(
    state: &HmmRuntime,
    mod_id: &ModId,
    profile_id: &ProfileId,
    expected: InstallManifestStatus,
) {
    for filter in [
        ModLibraryFilter::All,
        ModLibraryFilter::Status(InstallManifestStatus::Installed),
        ModLibraryFilter::Status(InstallManifestStatus::NotInstalled),
    ] {
        let matches = match filter {
            ModLibraryFilter::All => true,
            ModLibraryFilter::Status(status) => status == expected,
            ModLibraryFilter::Category(_) => unreachable!(),
        };
        let page = state
            .mod_library_query
            .query(ModLibraryQuery {
                profile_context: Some(ModLibraryProfileContext {
                    game_id: GameId::mhw(),
                    profile_id: profile_id.clone(),
                }),
                filter: filter.clone(),
                ..ModLibraryQuery::default()
            })
            .expect("query the existing runtime's status projection");
        assert_eq!(page.library_total, 1);
        assert_eq!(page.matching_total, usize::from(matches), "{filter:?}");
        assert_eq!(page.items.len(), usize::from(matches), "{filter:?}");
        if matches {
            assert_eq!(page.items[0].item.id, mod_id.as_str());
            assert_eq!(
                page.items[0]
                    .install_summary
                    .as_ref()
                    .map(|summary| summary.status),
                Some(expected),
                "card summary and filter membership must agree"
            );
        }
    }
}

fn setup(root: &Path, files: &[(&str, &[u8])]) -> (HmmRuntime, ModId, ProfileId, PathBuf) {
    let app_data = root.join("app-data");
    let game_root = root.join("game");
    let archive = root.join("fixture.zip");
    prepare_game_root(&game_root);
    create_fixture_zip(&archive, files);
    let state = HmmRuntime::from_app_data_dir(app_data).expect("compose temporary runtime");
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game_root.clone())
        .expect("configure artificial game directory");
    let (_, mod_id, _) = import_initial_fixture_revision(&state, &archive);
    (state, mod_id, ProfileId::new("default"), game_root)
}

#[test]
fn ordinary_install_refreshes_warmed_status_filters_without_restart() {
    let temp = tempfile::tempdir().expect("temporary install fixture");
    let (state, mod_id, profile_id, game_root) = setup(temp.path(), V1_FILES);
    assert_library_status(
        &state,
        &mod_id,
        &profile_id,
        InstallManifestStatus::NotInstalled,
    );

    let task_id = install_fixture_revision(&state, &mod_id, &profile_id);
    assert_eq!(
        state.task_manager.task_status(&task_id),
        Some(TaskStatus::Completed)
    );
    assert_fixture_bytes(&game_root, V1_FILES);
    assert_library_status(
        &state,
        &mod_id,
        &profile_id,
        InstallManifestStatus::Installed,
    );
}

#[test]
fn uninstall_refreshes_warmed_status_filters_without_restart() {
    let temp = tempfile::tempdir().expect("temporary uninstall fixture");
    let (state, mod_id, profile_id, game_root) = setup(temp.path(), V1_FILES);
    let baseline = snapshot_file_tree(&game_root);
    install_fixture_revision(&state, &mod_id, &profile_id);
    assert_library_status(
        &state,
        &mod_id,
        &profile_id,
        InstallManifestStatus::Installed,
    );

    let request = StartUninstallTaskRequest {
        game_id: GameId::mhw(),
        mod_id: mod_id.clone(),
        profile_id: profile_id.clone(),
    };
    let task = state
        .uninstall_tasks
        .start_uninstall_task(request.clone())
        .expect("register uninstall");
    state
        .uninstall_task_runner
        .run_uninstall_task(&task.task_id, request)
        .expect("run uninstall");
    assert_eq!(
        state.task_manager.task_status(&task.task_id),
        Some(TaskStatus::Completed)
    );
    assert_eq!(snapshot_file_tree(&game_root), baseline);
    assert_library_status(
        &state,
        &mod_id,
        &profile_id,
        InstallManifestStatus::NotInstalled,
    );
}

#[test]
fn retarget_install_and_switch_refresh_warmed_status_filters_without_restart() {
    let temp = tempfile::tempdir().expect("temporary retarget fixture");
    let (state, mod_id, profile_id, game_root) =
        setup(temp.path(), &[(ARMOR_SOURCE_TARGET, ARMOR_FIXTURE_BYTES)]);
    assert_library_status(
        &state,
        &mod_id,
        &profile_id,
        InstallManifestStatus::NotInstalled,
    );
    let initial = StartRetargetInstallTaskRequest {
        game_id: GameId::mhw(),
        profile_id: profile_id.clone(),
        mod_id: mod_id.clone(),
        target_id: ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target"),
        layer: FileLayer::new("base", 0),
    };
    let task = state
        .retarget_install_tasks
        .start_retarget_install_task(initial.clone())
        .expect("register retarget install");
    state
        .retarget_install_task_runner
        .run_retarget_install_task(&task.task_id, initial)
        .expect("run retarget install");
    assert_eq!(
        state.task_manager.task_status(&task.task_id),
        Some(TaskStatus::Completed)
    );
    assert_eq!(
        fs::read(game_root.join(ARMOR_RETARGETED_TARGET)).unwrap(),
        ARMOR_FIXTURE_BYTES
    );
    assert_library_status(
        &state,
        &mod_id,
        &profile_id,
        InstallManifestStatus::Installed,
    );

    let switch = RetargetReinstallRequest {
        game_id: GameId::mhw(),
        profile_id: profile_id.clone(),
        mod_id: mod_id.clone(),
        target_id: ReplacementTargetId::parse("mhw:armor:fatalis-beta").expect("switch target"),
        layer: FileLayer::new("base", 0),
    };
    let preview = state
        .reinstall_executor
        .preview_retarget_reinstall(switch.clone())
        .expect("preview target switch");
    let request = StartRetargetReinstallTaskRequest {
        game_id: switch.game_id,
        profile_id: switch.profile_id,
        mod_id: switch.mod_id,
        target_id: switch.target_id,
        layer: switch.layer,
        plan_token: preview.plan_token.expect("ready switch token"),
    };
    let task = state
        .reinstall_tasks
        .start_retarget_reinstall_task(request.clone())
        .expect("register target switch");
    state
        .reinstall_task_runner
        .run_retarget_reinstall_task(&task.task_id, request)
        .expect("run target switch");
    assert_eq!(
        state.task_manager.task_status(&task.task_id),
        Some(TaskStatus::Completed)
    );
    assert!(!game_root.join(ARMOR_SOURCE_TARGET).exists());
    assert!(!game_root.join(ARMOR_RETARGETED_TARGET).exists());
    assert_eq!(
        fs::read(game_root.join(ARMOR_SWITCH_TARGET)).unwrap(),
        ARMOR_FIXTURE_BYTES
    );
    assert_library_status(
        &state,
        &mod_id,
        &profile_id,
        InstallManifestStatus::Installed,
    );
}
