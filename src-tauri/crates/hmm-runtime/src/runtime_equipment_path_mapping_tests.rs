use super::*;

#[path = "runtime_equipment_path_upgrade_tests.rs"]
mod upgrade;

const FILES: &[(&str, &[u8])] = &[
    (
        "nativePC/wp/two/two028/mod/two028/two028.mod3",
        b"first nested model",
    ),
    (
        "nativePC/wp/two/two028/mod/two028/custom.mrl3",
        b"first author material",
    ),
    (
        "nativePC/wp/two/two028/mod/two028/skin.tex",
        b"unchanged texture",
    ),
    (
        "nativePC/wp/two/two020/mod/two020/two020.mod3",
        b"second nested model",
    ),
    (
        "nativePC/wp/two/two020/mod/two020/custom.mrl3",
        b"second author material",
    ),
];
const PLUGIN: &str = "nativePC/plugins/nested-fixture.dll";
const PLUGIN_BYTES: &[u8] = b"inert installed attachment";
const COMPANION: &str = "nativePC/sound/nested-fixture.bin";
const COMPANION_BYTES: &[u8] = b"package companion";

struct Fixture {
    _temp: tempfile::TempDir,
    state: HmmRuntime,
    app_data: PathBuf,
    game: PathBuf,
    mod_id: ModId,
    files: Vec<(String, Vec<u8>)>,
    baseline: std::collections::BTreeMap<String, Vec<u8>>,
    manifests: Arc<FailNextManifestSaveRepository>,
}

impl Fixture {
    fn new(files: &[(&str, &[u8])]) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        let game = temp.path().join("game");
        prepare_game_root(&game);
        fs::create_dir_all(game.join("nativePC/plugins")).unwrap();
        fs::write(game.join(PLUGIN), b"baseline attachment").unwrap();
        let baseline = snapshot_file_tree(&game);
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
        let mut all = files.to_vec();
        all.extend([(PLUGIN, PLUGIN_BYTES), (COMPANION, COMPANION_BYTES)]);
        let archive = temp.path().join("nested-paths.zip");
        create_fixture_zip(&archive, &all);
        let mod_id = import_equipment(&state, archive);
        Self {
            _temp: temp,
            state,
            app_data,
            game,
            mod_id,
            baseline,
            manifests,
            files: files
                .iter()
                .map(|(path, bytes)| ((*path).to_owned(), bytes.to_vec()))
                .collect(),
        }
    }

    fn choices(&self, destinations: &[(&str, &str)]) -> EquipmentRetargetReinstallRequest {
        let configuration = self
            .state
            .replacement_workflow
            .equipment_configuration(
                &GameId::mhw(),
                &self.mod_id,
                Some(&ProfileId::new("default")),
            )
            .unwrap();
        assert_eq!(configuration.sources.len(), destinations.len());
        let slots = configuration
            .sources
            .iter()
            .map(|item| {
                let (_, destination) = destinations
                    .iter()
                    .find(|(source, _)| *source == item.source.internal_id())
                    .unwrap();
                if *destination == item.source.internal_id() {
                    InitialRetargetSlotIntent::KeepInPlace {
                        source_id: item.source.id().clone(),
                    }
                } else {
                    InitialRetargetSlotIntent::Retarget {
                        source_id: item.source.id().clone(),
                        target_id: target(destination, item.source.path_family()),
                    }
                }
            })
            .collect();
        EquipmentRetargetReinstallRequest {
            intent: Default::default(),
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: self.mod_id.clone(),
            slots,
            layer: FileLayer::new("base", 0),
        }
    }

    fn reapply(&self) -> EquipmentRetargetReinstallRequest {
        EquipmentRetargetReinstallRequest::reapply(
            GameId::mhw(),
            ProfileId::new("default"),
            self.mod_id.clone(),
        )
    }

    fn run(&self, selection: EquipmentRetargetReinstallRequest) {
        let before = snapshot_file_tree(&self.game);
        let preview = self
            .state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(selection.clone())
            .unwrap();
        assert_eq!(
            preview.status,
            ReinstallPreviewStatus::Ready,
            "{:?}",
            preview.blocking_reasons
        );
        assert_eq!(
            snapshot_file_tree(&self.game),
            before,
            "preview cannot change the game"
        );
        let request = StartEquipmentRetargetReinstallTaskRequest {
            selection,
            plan_token: preview.plan_token.unwrap(),
        };
        let task = self
            .state
            .reinstall_tasks
            .start_equipment_retarget_reinstall_task(request.clone())
            .unwrap();
        let events = self
            .state
            .reinstall_task_runner
            .run_equipment_retarget_reinstall_task(&task.task_id, request)
            .unwrap();
        assert_eq!(events.last().unwrap().status, TaskStatus::Completed);
        assert_no_retarget_staging(&self.app_data);
        assert_no_reinstall_recovery_transactions(&self.app_data);
    }

    fn restart(self) -> Self {
        let Self {
            _temp,
            state,
            app_data,
            game,
            mod_id,
            files,
            baseline,
            manifests,
        } = self;
        drop(state);
        let state = HmmRuntime::builder(app_data.clone())
            .with_install_manifest_repository(manifests.clone())
            .build()
            .unwrap();
        Self {
            _temp,
            state,
            app_data,
            game,
            mod_id,
            files,
            baseline,
            manifests,
        }
    }

    fn assert_layout(&self, paths: &[&str]) {
        assert_eq!(paths.len(), self.files.len());
        let mut expected = self.baseline.clone();
        for (path, (_, bytes)) in paths.iter().zip(&self.files) {
            expected.insert((*path).to_owned(), bytes.clone());
        }
        expected.insert(PLUGIN.to_owned(), PLUGIN_BYTES.to_vec());
        expected.insert(COMPANION.to_owned(), COMPANION_BYTES.to_vec());
        assert_eq!(snapshot_file_tree(&self.game), expected);
    }

    fn uninstall(&self) {
        let request = StartUninstallTaskRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: self.mod_id.clone(),
        };
        let task = self
            .state
            .uninstall_tasks
            .start_uninstall_task(request.clone())
            .unwrap();
        self.state
            .uninstall_task_runner
            .run_uninstall_task(&task.task_id, request)
            .unwrap();
        assert_eq!(snapshot_file_tree(&self.game), self.baseline);
        assert_no_recovery_records(&self.app_data);
        assert_no_reinstall_recovery_transactions(&self.app_data);
    }
}

#[test]
fn nested_weapon_sources_swap_chain_and_move_independently_from_original_bytes() {
    let mut fixture = Fixture::new(FILES);
    install_fixture_revision(&fixture.state, &fixture.mod_id, &ProfileId::new("default"));
    for (choices, paths) in [
        (
            [("two028", "two020"), ("two020", "two028")],
            [
                "nativePC/wp/two/two020/mod/two020/two020.mod3",
                "nativePC/wp/two/two020/mod/two020/custom.mrl3",
                "nativePC/wp/two/two028/mod/two028/skin.tex",
                "nativePC/wp/two/two028/mod/two028/two028.mod3",
                "nativePC/wp/two/two028/mod/two028/custom.mrl3",
            ],
        ),
        (
            [("two028", "two020"), ("two020", "two029")],
            [
                "nativePC/wp/two/two020/mod/two020/two020.mod3",
                "nativePC/wp/two/two020/mod/two020/custom.mrl3",
                "nativePC/wp/two/two028/mod/two028/skin.tex",
                "nativePC/wp/two/two029/mod/two029/two029.mod3",
                "nativePC/wp/two/two029/mod/two029/custom.mrl3",
            ],
        ),
        (
            [("two028", "two003"), ("two020", "two029")],
            [
                "nativePC/wp/two/two003/mod/two003/two003.mod3",
                "nativePC/wp/two/two003/mod/two003/custom.mrl3",
                "nativePC/wp/two/two028/mod/two028/skin.tex",
                "nativePC/wp/two/two029/mod/two029/two029.mod3",
                "nativePC/wp/two/two029/mod/two029/custom.mrl3",
            ],
        ),
    ] {
        fixture.run(fixture.choices(&choices));
        fixture.assert_layout(&paths);
        fixture = fixture.restart();
        fixture.assert_layout(&paths);
        assert_eq!(
            read_fixture_manifest(&fixture.app_data)
                .replacement_bindings
                .len(),
            2
        );
    }
    fixture.uninstall();
}

#[test]
fn nested_shared_targets_reject_real_and_windows_equivalent_collisions_without_writes() {
    for second_name in ["custom.mod3", "CUSTOM.MOD3", "ｃustom.mod3"] {
        let second = format!("nativePC/wp/two/two020/mod/two020/{second_name}");
        let fixture = Fixture::new(&[
            ("nativePC/wp/two/two028/mod/two028/custom.mod3", b"first"),
            (&second, b"second"),
        ]);
        let selection = fixture.choices(&[("two028", "two029"), ("two020", "two029")]);
        let request = initial_request(selection.clone());
        let preview = fixture
            .state
            .initial_retarget_install_preflight
            .preview(request.clone())
            .unwrap();
        assert!(
            preview.planned.install_plan().has_blocking_conflicts(),
            "accepted {second_name}"
        );
        let task = fixture
            .state
            .retarget_install_tasks
            .start_equipment_retarget_install_task(request.clone())
            .unwrap();
        assert!(fixture
            .state
            .retarget_install_task_runner
            .run_equipment_retarget_install_task(&task.task_id, request)
            .is_err());
        assert_eq!(snapshot_file_tree(&fixture.game), fixture.baseline);
        assert_no_retarget_staging(&fixture.app_data);
        let keep = initial_request(fixture.choices(&[("two028", "two028"), ("two020", "two020")]));
        let task = fixture
            .state
            .retarget_install_tasks
            .start_equipment_retarget_install_task(keep.clone())
            .unwrap();
        let installed = fixture
            .state
            .retarget_install_task_runner
            .run_equipment_retarget_install_task(&task.task_id, keep);
        assert!(
            installed.is_ok(),
            "original install for {second_name}: {installed:?}"
        );
        let before = snapshot_file_tree(&fixture.game);
        let manifest = read_fixture_manifest(&fixture.app_data);
        let preview = fixture
            .state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(selection)
            .unwrap();
        assert_eq!(preview.status, ReinstallPreviewStatus::Blocked);
        assert!(preview.plan_token.is_none());
        assert_eq!(snapshot_file_tree(&fixture.game), before);
        assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
        assert_no_retarget_staging(&fixture.app_data);
    }
}

#[test]
fn an_unmovable_source_is_identified_and_can_stay_while_other_sources_move() {
    let fixture = Fixture::new(&[
        (
            "nativePC/wp/two/two028/mod/two028/custom.mod3",
            b"movable author model",
        ),
        (
            "nativePC/wp/two/two020/mod/custom.mod3",
            b"original-only model",
        ),
    ]);
    let request = fixture.choices(&[("two028", "two029"), ("two020", "two003")]);
    let blocked_source = request
        .slots
        .iter()
        .find_map(|slot| match slot {
            InitialRetargetSlotIntent::Retarget {
                source_id,
                target_id,
            } if *target_id == target("two003", "wp/two") => Some(source_id.clone()),
            _ => None,
        })
        .unwrap();
    let error = fixture
        .state
        .initial_retarget_install_preflight
        .preview(initial_request(request))
        .unwrap_err();
    assert_eq!(
        error,
        ReplacementWorkflowError::Analysis(hmm_app::ReplacementServiceError::Adapter(
            hmm_ports::ReplacementAdapterError::SourceAnalysisRejected {
                source_id: blocked_source,
                code: "weapon_no_relocatable_resources",
            },
        ))
    );
    assert_eq!(snapshot_file_tree(&fixture.game), fixture.baseline);
    let selection = fixture.choices(&[("two028", "two029"), ("two020", "two020")]);
    install_equipment(&fixture.state, selection);
    assert_eq!(
        fs::read(
            fixture
                .game
                .join("nativePC/wp/two/two029/mod/two029/custom.mod3")
        )
        .unwrap(),
        b"movable author model"
    );
    assert_eq!(
        fs::read(fixture.game.join("nativePC/wp/two/two020/mod/custom.mod3")).unwrap(),
        b"original-only model"
    );
    assert_eq!(
        read_fixture_manifest(&fixture.app_data)
            .replacement_bindings
            .len(),
        2
    );
    fixture.uninstall();
}
