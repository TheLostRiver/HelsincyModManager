use super::*;
use hmm_core::PluginFileChoiceKind;

#[path = "runtime_plugin_batch_tests.rs"]
mod batch;
#[path = "runtime_plugin_selection_recovery_tests.rs"]
mod recovery;
#[path = "runtime_plugin_selection_safety_tests.rs"]
mod safety;

const PLUGIN: &str = "nativePC/plugins/selection-fixture.dll";
const TOOL: &str = "nativePC/tools/converter-fixture.exe";
const MODEL: &str = "nativePC/wp/one/one001/mod/one001.mod3";

pub(super) fn synthetic_dll() -> Vec<u8> {
    crate::plugin_test_fixture::X64_DLL.to_vec()
}

pub(super) fn confirm_supported_plugins(
    state: &HmmRuntime,
    mod_id: &ModId,
    revision: Option<ModRevisionId>,
) {
    let scope = state
        .plugin_selection
        .resolve_scope(
            GameId::mhw(),
            ProfileId::new("default"),
            mod_id.clone(),
            revision,
        )
        .unwrap();
    if let Some(inventory) = state.plugin_selection.inventory(&scope).unwrap() {
        let included = inventory
            .candidates
            .iter()
            .filter(|file| file.selected)
            .map(|file| file.package_file_id.clone())
            .collect::<Vec<_>>();
        state
            .plugin_selection
            .select(&scope, &inventory.selection.inventory_id(), &included)
            .unwrap();
    }
}

fn reapply(state: &HmmRuntime, mod_id: &ModId) {
    let selection = EquipmentRetargetReinstallRequest::reapply(
        GameId::mhw(),
        ProfileId::new("default"),
        mod_id.clone(),
    );
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
}

#[test]
fn plugin_choices_survive_install_restart_remove_reinclude_and_uninstall() {
    let temp = tempfile::tempdir().unwrap();
    let app_data = temp.path().join("app-data");
    let game = temp.path().join("game");
    prepare_game_root(&game);
    fs::write(game.join(PLUGIN), b"original external plugin bytes").unwrap();
    let baseline = snapshot_file_tree(&game);
    let dll = synthetic_dll();
    let archive = temp.path().join("plugins.zip");
    create_fixture_zip(
        &archive,
        &[
            (MODEL, b"synthetic weapon"),
            (PLUGIN, &dll),
            (TOOL, b"inert author tool"),
        ],
    );
    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game.clone())
        .unwrap();
    let mod_id = import_equipment(&state, archive);
    let scope = state
        .plugin_selection
        .resolve_scope(
            GameId::mhw(),
            ProfileId::new("default"),
            mod_id.clone(),
            None,
        )
        .unwrap();
    let inventory = state.plugin_selection.inventory(&scope).unwrap().unwrap();
    assert!(inventory.confirmation_required);
    let plugin_id = inventory
        .candidates
        .iter()
        .find(|file| file.target_path.as_str() == PLUGIN)
        .unwrap()
        .package_file_id
        .clone();
    assert!(
        inventory
            .candidates
            .iter()
            .find(|file| file.package_file_id == plugin_id)
            .unwrap()
            .selected
    );
    assert!(
        !inventory
            .candidates
            .iter()
            .find(|file| file.target_path.as_str() == TOOL)
            .unwrap()
            .selectable
    );
    assert_eq!(snapshot_file_tree(&game), baseline);
    assert!(!app_data.join("install/plugin-selections").exists());

    let request = StartInstallTaskRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        mod_id: mod_id.clone(),
        layer: FileLayer::new("base", 0),
    };
    let task = state
        .install_tasks
        .start_install_task(request.clone())
        .unwrap();
    let error = state
        .install_task_runner
        .run_install_task(&task.task_id, request)
        .unwrap_err();
    assert_eq!(
        error.events.last().unwrap().error.as_deref(),
        Some("install_failed:plugin_selection_required")
    );
    assert_eq!(snapshot_file_tree(&game), baseline);
    state
        .plugin_selection
        .select(
            &scope,
            &inventory.selection.inventory_id(),
            std::slice::from_ref(&plugin_id),
        )
        .unwrap();
    install_fixture_revision(&state, &mod_id, &ProfileId::new("default"));
    assert_eq!(fs::read(game.join(PLUGIN)).unwrap(), dll);
    assert!(!game.join(TOOL).exists());
    let manifest = read_fixture_manifest(&app_data);
    assert_eq!(manifest.plugin_selections.len(), 1);
    let applied = manifest.plugin_selections[0]
        .files()
        .iter()
        .find(|file| file.package_file_id == plugin_id)
        .unwrap();
    assert_eq!(applied.choice, PluginFileChoiceKind::Include);
    drop(state);

    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
    let inventory = state.plugin_selection.inventory(&scope).unwrap().unwrap();
    assert!(!inventory.confirmation_required);
    assert!(
        inventory
            .candidates
            .iter()
            .find(|file| file.package_file_id == plugin_id)
            .unwrap()
            .selected
    );
    let noop = state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(EquipmentRetargetReinstallRequest::reapply(
            GameId::mhw(),
            ProfileId::new("default"),
            mod_id.clone(),
        ))
        .unwrap();
    assert_eq!(noop.status, ReinstallPreviewStatus::NoChanges);
    state
        .plugin_selection
        .select(&scope, &inventory.selection.inventory_id(), &[])
        .unwrap();
    reapply(&state, &mod_id);
    assert_eq!(
        fs::read(game.join(PLUGIN)).unwrap(),
        b"original external plugin bytes"
    );
    let manifest = read_fixture_manifest(&app_data);
    assert!(!manifest
        .entries
        .iter()
        .any(|entry| entry.target_path.as_str() == PLUGIN));
    assert_eq!(
        manifest.plugin_selections[0]
            .files()
            .iter()
            .find(|file| file.package_file_id == plugin_id)
            .unwrap()
            .choice,
        PluginFileChoiceKind::Exclude
    );

    let inventory = state.plugin_selection.inventory(&scope).unwrap().unwrap();
    state
        .plugin_selection
        .select(&scope, &inventory.selection.inventory_id(), &[plugin_id])
        .unwrap();
    reapply(&state, &mod_id);
    assert_eq!(fs::read(game.join(PLUGIN)).unwrap(), dll);
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
    assert!(read_fixture_manifest(&app_data)
        .plugin_selections
        .is_empty());
    assert_no_reinstall_recovery_transactions(&app_data);
    assert_no_retarget_staging(&app_data);
}

struct AutomationFixture {
    _temp: tempfile::TempDir,
    state: HmmRuntime,
    environment: crate::RuntimeEnvironment,
    app_data: PathBuf,
    game: PathBuf,
    mod_id: ModId,
    dll: Vec<u8>,
    manifests: Arc<FailNextManifestSaveRepository>,
}

impl AutomationFixture {
    fn new() -> Self {
        Self::with_equipment(false)
    }

    fn with_equipment(multiple: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        fs::create_dir_all(&app_data).unwrap();
        fs::write(
            app_data.join(crate::SANDBOX_MARKER_FILE_NAME),
            crate::SANDBOX_MARKER_SCHEMA,
        )
        .unwrap();
        let game = app_data.join("fixtures/games/mhw-minimal");
        prepare_game_root(&game);
        let dll = synthetic_dll();
        let archive = temp.path().join("plugins.zip");
        let mut files = if multiple {
            EQUIPMENT_FILES.to_vec()
        } else {
            vec![(MODEL, b"synthetic weapon".as_slice())]
        };
        files.push((PLUGIN, &dll));
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
        let environment = crate::RuntimeEnvironment::sandbox(app_data.clone()).unwrap();
        Self {
            _temp: temp,
            state,
            environment,
            app_data,
            game,
            mod_id,
            dll,
            manifests,
        }
    }

    fn scope(&self) -> hmm_core::PluginSelectionScope {
        self.state
            .plugin_selection
            .resolve_scope(
                GameId::mhw(),
                ProfileId::new("default"),
                self.mod_id.clone(),
                None,
            )
            .unwrap()
    }

    fn choose(&self, included: bool) {
        let scope = self.scope();
        let inventory = self
            .state
            .plugin_selection
            .inventory(&scope)
            .unwrap()
            .unwrap();
        let ids = inventory
            .candidates
            .iter()
            .filter(|file| included && file.selectable)
            .map(|file| file.package_file_id.clone())
            .collect::<Vec<_>>();
        self.state
            .plugin_selection
            .select(&scope, &inventory.selection.inventory_id(), &ids)
            .unwrap();
    }

    fn install(&self) {
        self.choose(true);
        install_fixture_revision(&self.state, &self.mod_id, &ProfileId::new("default"));
    }

    fn reapply_request(&self) -> EquipmentRetargetReinstallRequest {
        EquipmentRetargetReinstallRequest::reapply(
            GameId::mhw(),
            ProfileId::new("default"),
            self.mod_id.clone(),
        )
    }

    fn package(&self) -> PathBuf {
        let summary = self
            .state
            .replacement_workflow
            .replacement_summary(
                hmm_app::AnalyzeImportedReplacementRequest {
                    game_id: GameId::mhw(),
                    mod_id: self.mod_id.clone(),
                },
                None,
            )
            .unwrap();
        self.app_data
            .join("mod-import/sandboxes")
            .join(summary.package_id)
    }
}

#[test]
fn plugin_install_tokens_authorize_the_exact_cli_preview_without_prior_preferences() {
    let fixture = AutomationFixture::new();
    let reader = crate::ReadOnlyInstallAutomation::from_environment(&fixture.environment).unwrap();
    let preview = reader
        .plan_for_profile("mhw", "default", fixture.mod_id.as_str())
        .unwrap();
    let revision = fixture
        .state
        .replacement_workflow
        .current_install_revision(&fixture.mod_id)
        .unwrap();
    let (_, _, _, default_plan, _) = reader
        .build_install_plan("mhw", "default", fixture.mod_id.as_str())
        .unwrap();
    let (_, _, _, _, pinned_plan, _) = reader
        .build_install_plan_for_revision(
            "mhw",
            "default",
            fixture.mod_id.as_str(),
            revision.as_str(),
            &FileLayer::new("base", 0),
        )
        .unwrap();
    assert_eq!(
        default_plan, pinned_plan,
        "CLI preview must include the same canonical bindings as execution"
    );
    assert_eq!(preview.action_count, 2);
    assert!(!fixture.app_data.join("install/plugin-selections").exists());
    let prepared = crate::CliLifecycleAutomation::prepare_install(
        &fixture.environment,
        "mhw",
        "default",
        fixture.mod_id.as_str(),
        preview.plan_token.as_deref().unwrap(),
    )
    .unwrap();
    let outcome = prepared.run_install().unwrap();
    assert_eq!(outcome.events.last().unwrap().status, TaskStatus::Completed);
    assert_eq!(fs::read(fixture.game.join(PLUGIN)).unwrap(), fixture.dll);
    assert_eq!(
        read_fixture_manifest(&fixture.app_data)
            .plugin_selections
            .len(),
        1
    );
}

#[test]
fn plugin_install_tokens_authorize_the_exact_batch_plan_without_prior_preferences() {
    let fixture = AutomationFixture::new();
    let scope = fixture
        .state
        .plugin_selection
        .resolve_scope(
            GameId::mhw(),
            ProfileId::new("default"),
            fixture.mod_id.clone(),
            None,
        )
        .unwrap();
    let request = crate::BatchLifecyclePlanRequest {
        plan: hmm_core::BatchPlanRequest {
            schema_version: hmm_core::BATCH_PLAN_SCHEMA_VERSION,
            operation: hmm_core::BatchOperation::Install,
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            execution_policy: hmm_core::BatchExecutionPolicy::StopOnFailure,
            items: vec![hmm_core::BatchItemInput::Install(
                hmm_core::InstallBatchItemInput {
                    mod_id: fixture.mod_id.clone(),
                    revision_id: scope.revision_id,
                    layer: FileLayer::new("base", 0),
                    replacement_binding_snapshot: None,
                },
            )],
        },
        replacement_targets: std::collections::BTreeMap::new(),
    };
    let preview =
        crate::BatchLifecycleAutomation::preview_request(&fixture.environment, request.clone())
            .unwrap();
    assert_eq!(preview.plan.status(), hmm_core::BatchPlanStatus::Ready);
    let database = fixture.state.database_handle();
    let (_, sealed) = crate::BatchLifecycleAutomation::seal_request_with_database(
        &fixture.environment,
        request,
        preview.preview_token.as_deref().unwrap(),
        Arc::clone(&database),
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
    assert_eq!(fs::read(fixture.game.join(PLUGIN)).unwrap(), fixture.dll);
    assert_eq!(
        read_fixture_manifest(&fixture.app_data)
            .plugin_selections
            .len(),
        1
    );
}

#[test]
fn plugin_only_mod_can_apply_choices_but_cannot_change_other_files_with_reapply() {
    const SECOND: &str = "nativePC/plugins/second-fixture.dll";
    const CONFIG: &str = "nativePC/plugins/fixture-settings.ini";
    let temp = tempfile::tempdir().unwrap();
    let app_data = temp.path().join("app-data");
    let game = temp.path().join("game");
    prepare_game_root(&game);
    let dll = synthetic_dll();
    let archive = temp.path().join("plugin-only.zip");
    create_fixture_zip(
        &archive,
        &[
            (PLUGIN, &dll),
            (SECOND, &dll),
            (CONFIG, b"original settings"),
        ],
    );
    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
    state
        .game_setup
        .save_game_directory(GameId::mhw(), game.clone())
        .unwrap();
    let mod_id = import_equipment(&state, archive);
    let scope = state
        .plugin_selection
        .resolve_scope(
            GameId::mhw(),
            ProfileId::new("default"),
            mod_id.clone(),
            None,
        )
        .unwrap();
    let inventory = state.plugin_selection.inventory(&scope).unwrap().unwrap();
    let selected = inventory
        .candidates
        .iter()
        .map(|file| file.package_file_id.clone())
        .collect::<Vec<_>>();
    state
        .plugin_selection
        .select(&scope, &inventory.selection.inventory_id(), &selected)
        .unwrap();
    install_fixture_revision(&state, &mod_id, &ProfileId::new("default"));
    assert!(read_fixture_manifest(&app_data)
        .replacement_bindings
        .is_empty());
    let inventory = state.plugin_selection.inventory(&scope).unwrap().unwrap();
    let keep = inventory
        .candidates
        .iter()
        .find(|file| file.target_path.as_str() == PLUGIN)
        .unwrap()
        .package_file_id
        .clone();
    state
        .plugin_selection
        .select(&scope, &inventory.selection.inventory_id(), &[keep])
        .unwrap();
    reapply(&state, &mod_id);
    assert!(game.join(PLUGIN).is_file());
    assert!(!game.join(SECOND).exists());
    assert_eq!(fs::read(game.join(CONFIG)).unwrap(), b"original settings");
    let before = snapshot_file_tree(&game);
    let manifest = read_fixture_manifest(&app_data);
    assert!(manifest.replacement_bindings.is_empty());
    let summary = state
        .replacement_workflow
        .replacement_summary(
            hmm_app::AnalyzeImportedReplacementRequest {
                game_id: GameId::mhw(),
                mod_id: mod_id.clone(),
            },
            None,
        )
        .unwrap();
    fs::write(
        app_data
            .join("mod-import/sandboxes")
            .join(summary.package_id)
            .join(CONFIG),
        b"changed unrelated settings",
    )
    .unwrap();
    let preview = state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(EquipmentRetargetReinstallRequest::reapply(
            GameId::mhw(),
            ProfileId::new("default"),
            mod_id,
        ))
        .unwrap();
    assert_eq!(preview.status, ReinstallPreviewStatus::Blocked);
    assert_eq!(snapshot_file_tree(&game), before);
    assert_eq!(read_fixture_manifest(&app_data), manifest);
}
