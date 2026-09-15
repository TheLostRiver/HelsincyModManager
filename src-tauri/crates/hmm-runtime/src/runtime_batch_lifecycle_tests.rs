use super::*;
use crate::{BatchLifecycleAutomation, BatchLifecyclePlanRequest, RuntimeEnvironment};
use hmm_app::{ModLibraryFilter, ModLibraryProfileContext, ModLibraryQuery};
use hmm_core::{
    BatchAttemptStatus, BatchExecutionPolicy, BatchItemInput, BatchOperation, BatchPlanRequest,
    InstallBatchItemInput, ReinstallBatchItemInput, UninstallBatchItemInput,
    BATCH_PLAN_SCHEMA_VERSION,
};

const SECOND_TARGET: &str = "nativePC/lifecycle/second.bin";

fn assert_status_members(state: &HmmRuntime, expected: &[ModId], status: InstallManifestStatus) {
    let page = state
        .mod_library_query
        .query(ModLibraryQuery {
            profile_context: Some(ModLibraryProfileContext {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
            }),
            filter: ModLibraryFilter::Status(status),
            ..ModLibraryQuery::default()
        })
        .expect("query GUI projection without rebuilding its runtime");
    assert_eq!(page.library_total, 2);
    assert_eq!(page.matching_total, expected.len());
    let actual = page
        .items
        .iter()
        .map(|item| {
            assert_eq!(item.install_summary.as_ref().unwrap().status, status);
            item.item.id.clone()
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        actual,
        expected.iter().map(|id| id.as_str().to_owned()).collect()
    );
}

fn request(operation: BatchOperation, items: Vec<BatchItemInput>) -> BatchLifecyclePlanRequest {
    BatchPlanRequest {
        schema_version: BATCH_PLAN_SCHEMA_VERSION,
        operation,
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        execution_policy: BatchExecutionPolicy::StopOnFailure,
        items,
    }
    .into()
}

fn run_gui_batch(
    state: &HmmRuntime,
    environment: &RuntimeEnvironment,
    request: BatchLifecyclePlanRequest,
) -> (String, String, String) {
    let preview = BatchLifecycleAutomation::preview_request(environment, request.clone())
        .expect("preview batch");
    let (_, sealed) = BatchLifecycleAutomation::seal_request_with_database(
        environment,
        request,
        &preview.preview_token.expect("ready token"),
        state.database_handle(),
    )
    .expect("seal through the active GUI database");
    let (_, run) = BatchLifecycleAutomation::start_request_with_database(
        environment,
        &sealed.batch_id,
        &sealed.plan_token,
        state.database_handle(),
    )
    .expect("execute batch");
    assert_eq!(run.status, BatchAttemptStatus::Completed);
    let result = BatchLifecycleAutomation::result_with_database(
        environment,
        &sealed.batch_id,
        run.attempt_number,
        state.database_handle(),
    )
    .expect("read WAL-backed batch results");
    assert_eq!(result.status, BatchAttemptStatus::Completed);
    assert_eq!(result.summary.succeeded_count, 2);
    assert_eq!(result.summary.failed_count, 0);
    assert_eq!(result.items.len(), 2);
    (sealed.batch_id, sealed.plan_token, run.task_id)
}

#[test]
fn production_gui_batches_update_filters_and_restore_two_mods_without_restart() {
    let temp = tempfile::tempdir().expect("temporary GUI batch fixture");
    let app_data = temp.path().join("app-data");
    let game_root = temp.path().join("game");
    prepare_game_root(&game_root);
    let baseline = snapshot_file_tree(&game_root);
    let state = HmmRuntime::from_app_data_dir(app_data.clone()).expect("GUI runtime");
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game_root.clone())
        .unwrap();
    let environment = RuntimeEnvironment::production_with_app_data_root_for_tests(app_data.clone());
    let first_archive = temp.path().join("first.zip");
    let second_archive = temp.path().join("second.zip");
    create_fixture_zip(&first_archive, V1_FILES);
    create_fixture_zip(&second_archive, &[(SECOND_TARGET, b"second-v1")]);
    let (_, first, first_v1) = import_initial_fixture_revision(&state, &first_archive);
    let (_, second, second_v1) = import_initial_fixture_revision(&state, &second_archive);
    let mods = [first.clone(), second.clone()];
    assert_status_members(&state, &mods, InstallManifestStatus::NotInstalled);
    assert_status_members(&state, &[], InstallManifestStatus::Installed);

    let (batch_id, plan_token, task_id) = run_gui_batch(
        &state,
        &environment,
        request(
            BatchOperation::Install,
            [
                (first.clone(), first_v1.clone()),
                (second.clone(), second_v1.clone()),
            ]
            .into_iter()
            .map(|(mod_id, revision_id)| {
                BatchItemInput::Install(InstallBatchItemInput {
                    mod_id,
                    revision_id,
                    layer: FileLayer::new("base", 0),
                    replacement_binding_snapshot: None,
                })
            })
            .collect(),
        ),
    );
    assert_fixture_bytes(&game_root, V1_FILES);
    assert_eq!(
        fs::read(game_root.join(SECOND_TARGET)).unwrap(),
        b"second-v1"
    );
    assert_status_members(&state, &mods, InstallManifestStatus::Installed);
    assert_status_members(&state, &[], InstallManifestStatus::NotInstalled);
    let installed = snapshot_file_tree(&game_root);
    let manifest = read_fixture_manifest(&app_data);
    let (_, repeated) = BatchLifecycleAutomation::start_request_with_database(
        &environment,
        &batch_id,
        &plan_token,
        state.database_handle(),
    )
    .expect("repeated confirmation is idempotent");
    assert_eq!(repeated.task_id, task_id);
    assert_eq!(snapshot_file_tree(&game_root), installed);
    assert_eq!(read_fixture_manifest(&app_data), manifest);

    let first_update = temp.path().join("first-v2.zip");
    let second_update = temp.path().join("second-v2.zip");
    create_fixture_zip(&first_update, V2_FILES);
    create_fixture_zip(&second_update, &[(SECOND_TARGET, b"second-v2")]);
    let (_, first_v2) = import_candidate_fixture_revision(&state, &first_update, &first, &first_v1);
    let (_, second_v2) =
        import_candidate_fixture_revision(&state, &second_update, &second, &second_v1);
    assert_status_members(&state, &mods, InstallManifestStatus::Installed);
    run_gui_batch(
        &state,
        &environment,
        request(
            BatchOperation::Reinstall,
            [
                (first.clone(), first_v1, first_v2.clone()),
                (second.clone(), second_v1, second_v2.clone()),
            ]
            .into_iter()
            .map(|(mod_id, installed_revision_id, candidate_revision_id)| {
                BatchItemInput::Reinstall(ReinstallBatchItemInput {
                    intent: Default::default(),
                    mod_id,
                    installed_revision_id,
                    candidate_revision_id,
                    layer: FileLayer::new("base", 0),
                    replacement_binding_snapshot: None,
                })
            })
            .collect(),
        ),
    );
    assert_fixture_bytes(&game_root, V2_FILES);
    assert!(!game_root.join("nativePC/lifecycle/stale.bin").exists());
    assert_eq!(
        fs::read(game_root.join(SECOND_TARGET)).unwrap(),
        b"second-v2"
    );
    assert_status_members(&state, &mods, InstallManifestStatus::Installed);
    assert_status_members(&state, &[], InstallManifestStatus::NotInstalled);

    run_gui_batch(
        &state,
        &environment,
        request(
            BatchOperation::Uninstall,
            [(first, first_v2), (second, second_v2)]
                .into_iter()
                .map(|(mod_id, revision)| {
                    BatchItemInput::Uninstall(UninstallBatchItemInput {
                        mod_id,
                        expected_installed_revision_id: revision,
                    })
                })
                .collect(),
        ),
    );
    assert_status_members(&state, &[], InstallManifestStatus::Installed);
    assert_status_members(&state, &mods, InstallManifestStatus::NotInstalled);
    assert_eq!(snapshot_file_tree(&game_root), baseline);
    assert_no_recovery_records(&app_data);
    assert_no_reinstall_recovery_transactions(&app_data);
    assert!(app_data.join("secrets/batch-token-secret-v1").is_file());
    assert!(!app_data.join(crate::SANDBOX_MARKER_FILE_NAME).exists());
}

#[test]
fn production_gui_batch_partial_failure_retries_only_the_failed_mod_and_updates_filters() {
    let temp = tempfile::tempdir().expect("temporary partial batch fixture");
    let app_data = temp.path().join("app-data");
    let game_root = temp.path().join("game");
    prepare_game_root(&game_root);
    let baseline = snapshot_file_tree(&game_root);
    let state = HmmRuntime::from_app_data_dir(app_data.clone()).expect("GUI runtime");
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game_root.clone())
        .unwrap();
    let environment = RuntimeEnvironment::production_with_app_data_root_for_tests(app_data.clone());
    let first_archive = temp.path().join("first.zip");
    let second_archive = temp.path().join("second.zip");
    create_fixture_zip(&first_archive, V1_FILES);
    create_fixture_zip(&second_archive, &[(SECOND_TARGET, b"second")]);
    let (_, first, first_revision) = import_initial_fixture_revision(&state, &first_archive);
    let (_, second, second_revision) = import_initial_fixture_revision(&state, &second_archive);
    let mods = [first.clone(), second.clone()];
    assert_status_members(&state, &mods, InstallManifestStatus::NotInstalled);
    let mut install = request(
        BatchOperation::Install,
        [
            (first.clone(), first_revision.clone()),
            (second.clone(), second_revision.clone()),
        ]
        .into_iter()
        .map(|(mod_id, revision_id)| {
            BatchItemInput::Install(InstallBatchItemInput {
                mod_id,
                revision_id,
                layer: FileLayer::new("base", 0),
                replacement_binding_snapshot: None,
            })
        })
        .collect(),
    );
    install.plan.execution_policy = BatchExecutionPolicy::ContinueOnItemFailure;
    let preview = BatchLifecycleAutomation::preview_request(&environment, install.clone()).unwrap();
    let (_, sealed) = BatchLifecycleAutomation::seal_request_with_database(
        &environment,
        install,
        &preview.preview_token.expect("ready token"),
        state.database_handle(),
    )
    .unwrap();

    // Make only the second target unwritable after preview without changing the sealed input.
    fs::create_dir(game_root.join(SECOND_TARGET)).expect("inject a target directory collision");
    let (_, run) = BatchLifecycleAutomation::start_request_with_database(
        &environment,
        &sealed.batch_id,
        &sealed.plan_token,
        state.database_handle(),
    )
    .expect("partial failure remains a readable batch result");
    let result = BatchLifecycleAutomation::result_with_database(
        &environment,
        &sealed.batch_id,
        run.attempt_number,
        state.database_handle(),
    )
    .unwrap();
    assert_eq!(result.status, BatchAttemptStatus::CompletedWithErrors);
    assert_eq!(result.summary.succeeded_count, 1);
    assert_eq!(result.summary.failed_count, 1);
    let failed = result
        .items
        .iter()
        .find(|item| item.mod_id == second)
        .unwrap();
    assert!(
        failed.retryable,
        "an unwritten target can be retried after correcting the collision"
    );
    assert_fixture_bytes(&game_root, V1_FILES);
    assert_status_members(
        &state,
        std::slice::from_ref(&first),
        InstallManifestStatus::Installed,
    );
    assert_status_members(
        &state,
        std::slice::from_ref(&second),
        InstallManifestStatus::NotInstalled,
    );
    let first_manifest = read_fixture_manifest(&app_data);

    fs::remove_dir(game_root.join(SECOND_TARGET))
        .expect("remove injected empty collision directory");
    let (_, _, retried) = BatchLifecycleAutomation::retry_with_operation_with_database(
        &environment,
        &sealed.batch_id,
        0,
        state.database_handle(),
    )
    .expect("retry failed item using the GUI journal");
    assert_eq!(retried.status, BatchAttemptStatus::Completed);
    assert_eq!(retried.attempt_number, 1);
    let retry_result = BatchLifecycleAutomation::result_with_database(
        &environment,
        &sealed.batch_id,
        1,
        state.database_handle(),
    )
    .unwrap();
    assert_eq!(
        retry_result.items.len(),
        1,
        "successful Mod must not be executed again"
    );
    assert_eq!(retry_result.items[0].mod_id, second);
    assert_eq!(retry_result.summary.succeeded_count, 1);
    let manifest = read_fixture_manifest(&app_data);
    assert!(first_manifest
        .entries
        .iter()
        .all(|entry| manifest.entries.contains(entry)));
    assert_status_members(&state, &mods, InstallManifestStatus::Installed);
    assert_status_members(&state, &[], InstallManifestStatus::NotInstalled);
    assert_eq!(fs::read(game_root.join(SECOND_TARGET)).unwrap(), b"second");

    run_gui_batch(
        &state,
        &environment,
        request(
            BatchOperation::Uninstall,
            [(first, first_revision), (second, second_revision)]
                .into_iter()
                .map(|(mod_id, revision)| {
                    BatchItemInput::Uninstall(UninstallBatchItemInput {
                        mod_id,
                        expected_installed_revision_id: revision,
                    })
                })
                .collect(),
        ),
    );
    assert_eq!(snapshot_file_tree(&game_root), baseline);
    assert_status_members(&state, &mods, InstallManifestStatus::NotInstalled);
}
