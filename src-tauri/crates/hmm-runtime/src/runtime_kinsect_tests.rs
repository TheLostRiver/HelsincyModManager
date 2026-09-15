use super::*;
use hmm_app::{
    EquipmentRetargetReinstallTaskExecutor, ReinstallTaskExecutor, ReinstallTaskPrepared,
};

#[path = "runtime_kinsect_batch_tests.rs"]
mod batch;
#[path = "runtime_kinsect_safety_tests.rs"]
mod safety;

const MUS_MODEL: &str = "nativePC/wp/mus/mus001/mod/mus001.mod3";
const ROD_MODEL: &str = "nativePC/wp/rod/rod001/mod/rod001/rod001.mod3";
const FILES: &[(&str, &[u8])] = &[
    (ROD_MODEL, b"artificial glaive"),
    (MUS_MODEL, b"artificial kinsect one"),
    (
        "nativePC/wp/mus/mus001/mod/mus001.mrl3",
        &single_texture_material("wp/mus/mus001/mod/mus001_BML"),
    ),
    (
        "nativePC/wp/mus/mus001/mod/mus001_BML.tex",
        b"artificial texture",
    ),
    (
        "nativePC/wp/mus/mus002/epv/mus002.epv3",
        b"artificial kinsect two effect",
    ),
    (ARMOR_SOURCE_TARGET, ARMOR_FIXTURE_BYTES),
    ("nativePC/sound/kinsect-fixture.bin", b"companion"),
    (
        "nativePC/plugins/kinsect-fixture.dll",
        &crate::plugin_test_fixture::X64_DLL,
    ),
];

struct Fixture {
    _temp: tempfile::TempDir,
    state: HmmRuntime,
    app_data: PathBuf,
    game: PathBuf,
    package: PathBuf,
    mod_id: ModId,
    manifests: Arc<FailNextManifestSaveRepository>,
    baseline: BTreeMap<String, Vec<u8>>,
}

impl Fixture {
    fn new(files: &[(&str, &[u8])]) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        let game = app_data.join("fixtures/games/mhw-minimal");
        fs::create_dir_all(&app_data).unwrap();
        fs::write(
            app_data.join(crate::SANDBOX_MARKER_FILE_NAME),
            crate::SANDBOX_MARKER_SCHEMA,
        )
        .unwrap();
        prepare_game_root(&game);
        let baseline_file = game.join("nativePC/wp/mus/mus003/mod/mus003.mod3");
        fs::create_dir_all(baseline_file.parent().unwrap()).unwrap();
        fs::write(baseline_file, b"existing destination baseline").unwrap();
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
        let archive = temp.path().join("kinsects.zip");
        create_fixture_zip(&archive, files);
        let mod_id = import_equipment(&state, archive);
        plugins::confirm_supported_plugins(&state, &mod_id, None);
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
        let package = app_data
            .join("mod-import/sandboxes")
            .join(summary.package_id);
        Self {
            _temp: temp,
            state,
            app_data,
            game,
            package,
            mod_id,
            manifests,
            baseline,
        }
    }

    fn install(&self) {
        install_fixture_revision(&self.state, &self.mod_id, &ProfileId::new("default"));
    }

    fn choices(&self, changes: &[(&str, &str)]) -> EquipmentRetargetReinstallRequest {
        let configuration = self
            .state
            .replacement_workflow
            .equipment_configuration(
                &GameId::mhw(),
                &self.mod_id,
                Some(&ProfileId::new("default")),
            )
            .unwrap();
        let slots = configuration
            .sources
            .iter()
            .map(|source| {
                let selected = changes
                    .iter()
                    .find(|(from, _)| *from == source.source.internal_id())
                    .map(|(_, to)| target(to, source.source.path_family()))
                    .or_else(|| {
                        configuration
                            .installed_targets
                            .as_ref()?
                            .get(source.source.id())
                            .cloned()
                    });
                match selected {
                    Some(target_id) if Some(&target_id) != source.original_target_id.as_ref() => {
                        InitialRetargetSlotIntent::Retarget {
                            source_id: source.source.id().clone(),
                            target_id,
                        }
                    }
                    _ => InitialRetargetSlotIntent::KeepInPlace {
                        source_id: source.source.id().clone(),
                    },
                }
            })
            .collect();
        EquipmentRetargetReinstallRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: self.mod_id.clone(),
            slots,
            layer: FileLayer::new("base", 0),
            intent: Default::default(),
        }
    }

    fn reapply(&self) -> EquipmentRetargetReinstallRequest {
        EquipmentRetargetReinstallRequest::reapply(
            GameId::mhw(),
            ProfileId::new("default"),
            self.mod_id.clone(),
        )
    }

    fn preview(&self, selection: EquipmentRetargetReinstallRequest) -> ReinstallPlanPreview {
        self.state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(selection)
            .unwrap()
    }

    fn request(
        &self,
        selection: EquipmentRetargetReinstallRequest,
    ) -> StartEquipmentRetargetReinstallTaskRequest {
        let before = snapshot_file_tree(&self.game);
        let preview = self.preview(selection.clone());
        assert_eq!(
            preview.status,
            ReinstallPreviewStatus::Ready,
            "{:?}",
            preview.blocking_reasons
        );
        assert_eq!(snapshot_file_tree(&self.game), before);
        StartEquipmentRetargetReinstallTaskRequest {
            selection,
            plan_token: preview.plan_token.unwrap(),
        }
    }

    fn run(&self, selection: EquipmentRetargetReinstallRequest) {
        let request = self.request(selection);
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
        assert_no_reinstall_recovery_transactions(&self.app_data);
        assert_no_retarget_staging(&self.app_data);
    }

    fn legacy_bindings(&self) -> InstallManifest {
        let mut manifest = read_fixture_manifest(&self.app_data);
        manifest
            .replacement_bindings
            .retain(|binding| binding.retarget_kind().as_str() != "kinsect");
        self.manifests.save_manifest(&manifest).unwrap();
        manifest
    }

    fn old_glaive_layout(&self) {
        let mut manifest = read_fixture_manifest(&self.app_data);
        let current = "nativePC/wp/rod/rod002/mod/rod002/rod002.mod3";
        let legacy = "nativePC/wp/rod/rod002/mod/rod001/rod002.mod3";
        fs::create_dir_all(self.game.join(legacy).parent().unwrap()).unwrap();
        fs::rename(self.game.join(current), self.game.join(legacy)).unwrap();
        manifest
            .entries
            .iter_mut()
            .find(|entry| entry.target_path.as_str() == current)
            .unwrap()
            .target_path = hmm_core::InstallTargetPath::parse(legacy, ["nativePC"]).unwrap();
        self.manifests.save_manifest(&manifest).unwrap();
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
    }
}

#[test]
fn independent_kinsect_installs_retargets_and_uninstalls_to_the_backup_baseline() {
    let f = Fixture::new(&FILES[1..4]);
    f.install();
    assert_eq!(
        read_fixture_manifest(&f.app_data)
            .replacement_bindings
            .len(),
        1
    );
    let original = snapshot_file_tree(&f.package);
    f.run(f.choices(&[("mus001", "mus003")]));
    assert_eq!(
        fs::read(f.game.join("nativePC/wp/mus/mus003/mod/mus003.mod3")).unwrap(),
        FILES[1].1
    );
    assert!(!f.game.join(MUS_MODEL).exists());
    assert!(!f.game.join(FILES[3].0).exists());
    assert_eq!(
        fs::read(f.game.join("nativePC/wp/mus/mus003/mod/mus003_BML.tex")).unwrap(),
        FILES[3].1
    );
    assert_eq!(
        fs::read(f.game.join("nativePC/wp/mus/mus003/mod/mus003.mrl3")).unwrap(),
        single_texture_material("wp/mus/mus003/mod/mus003_BML")
    );
    assert_eq!(snapshot_file_tree(&f.package), original);
    f.uninstall();
}

#[test]
fn mixed_package_supports_swapping_combining_and_separating_kinsects_without_touching_plugins() {
    let f = Fixture::new(FILES);
    // 首次逐源安装也共用独立猎虫类型。
    install_equipment(
        &f.state,
        f.choices(&[("mus001", "mus002"), ("mus002", "mus001")]),
    );
    let original = snapshot_file_tree(&f.package);
    let first = read_fixture_manifest(&f.app_data);
    assert_eq!(first.replacement_bindings.len(), 4);
    assert_eq!(
        fs::read(f.game.join("nativePC/wp/mus/mus002/mod/mus002.mod3")).unwrap(),
        FILES[1].1
    );
    assert_eq!(
        fs::read(f.game.join("nativePC/wp/mus/mus001/epv/mus001.epv3")).unwrap(),
        FILES[4].1
    );
    f.run(f.choices(&[("mus001", "mus003"), ("mus002", "mus003")]));
    f.run(f.choices(&[("mus001", "mus001"), ("mus002", "mus002")]));
    let manifest = read_fixture_manifest(&f.app_data);
    assert_eq!(manifest.plugin_selections, first.plugin_selections);
    assert_eq!(manifest.entries.len(), FILES.len());
    for (path, bytes) in FILES {
        assert_eq!(fs::read(f.game.join(path)).unwrap(), *bytes, "{path}");
    }
    assert_eq!(snapshot_file_tree(&f.package), original);
    f.uninstall();
}

#[test]
fn old_bound_glaive_can_gain_kinsect_sources_without_resetting_its_target() {
    let f = Fixture::new(FILES);
    f.install();
    f.run(f.choices(&[("rod001", "rod002")]));
    let before = f.legacy_bindings();
    assert_eq!(before.replacement_bindings.len(), 2);
    let source_package = snapshot_file_tree(&f.package);
    f.run(f.choices(&[("mus001", "mus003")]));
    let after = read_fixture_manifest(&f.app_data);
    assert_eq!(after.replacement_bindings.len(), 4);
    for binding in &before.replacement_bindings {
        let current = after
            .replacement_bindings
            .iter()
            .find(|current| current.binding_id() == binding.binding_id())
            .unwrap();
        assert_eq!(current.binding(), binding.binding());
        assert_eq!(current.target_internal_id(), binding.target_internal_id());
    }
    assert_eq!(after.plugin_selections, before.plugin_selections);
    assert_eq!(
        fs::read(f.game.join("nativePC/wp/rod/rod002/mod/rod002/rod002.mod3")).unwrap(),
        FILES[0].1
    );
    assert_eq!(snapshot_file_tree(&f.package), source_package);
    let restarted = HmmRuntime::from_app_data_dir(f.app_data.clone()).unwrap();
    let summary = restarted
        .replacement_workflow
        .replacement_summary(
            hmm_app::AnalyzeImportedReplacementRequest {
                game_id: GameId::mhw(),
                mod_id: f.mod_id.clone(),
            },
            Some(&ProfileId::new("default")),
        )
        .unwrap();
    let current = summary.installed_targets.unwrap();
    assert!(current.iter().any(|target| target.kind == "kinsect"
        && target.internal_id == "mus003"
        && !target.display_names.is_empty()));
    f.uninstall();
}

#[test]
fn unchanged_old_install_does_not_backfill_bindings_or_permit_a_noop_commit() {
    let f = Fixture::new(FILES);
    f.install();
    f.legacy_bindings();
    let before = snapshot_file_tree(&f.game);
    let path = f.app_data.join("install/manifests/default.json");
    let manifest = fs::read(&path).unwrap();
    assert_eq!(
        f.preview(f.reapply()).status,
        ReinstallPreviewStatus::NoChanges
    );
    let prepared = f
        .state
        .reinstall_executor
        .prepare_equipment_retarget_reinstall(f.reapply())
        .unwrap();
    let token = prepared.plan_token().to_owned();
    assert!(f.state.reinstall_executor.commit(prepared, &token).is_err());
    assert_eq!(fs::read(path).unwrap(), manifest);
    assert_eq!(snapshot_file_tree(&f.game), before);
    assert_no_reinstall_recovery_transactions(&f.app_data);
}

#[test]
fn hover_summary_keeps_installed_kinsect_sources_after_importing_a_different_revision() {
    let f = Fixture::new(FILES);
    f.install();
    let manifest = f.legacy_bindings();
    let profile = ProfileId::new("default");
    let request = hmm_app::AnalyzeImportedReplacementRequest {
        game_id: GameId::mhw(),
        mod_id: f.mod_id.clone(),
    };
    let old = f
        .state
        .replacement_workflow
        .replacement_summary(request.clone(), Some(&profile))
        .unwrap();
    let archive = f._temp.path().join("new-kinsect-revision.zip");
    create_fixture_zip(
        &archive,
        &[(
            "nativePC/wp/mus/mus023/mod/mus023.mod3",
            b"different new kinsect",
        )],
    );
    import_candidate_fixture_revision(
        &f.state,
        &archive,
        &f.mod_id,
        manifest.entries[0].revision_id.as_ref().unwrap(),
    );
    let library = f
        .state
        .replacement_workflow
        .replacement_summary(request.clone(), None)
        .unwrap();
    let installed = f
        .state
        .replacement_workflow
        .replacement_summary(request, Some(&profile))
        .unwrap();
    assert_eq!(installed.sources, old.sources);
    assert_eq!(installed.installed_targets, old.installed_targets);
    assert_eq!(installed.source_package_id, old.source_package_id);
    assert_eq!(installed.package_id, library.package_id);
    assert_ne!(installed.source_package_id, installed.package_id);
    assert_eq!(library.package_id, library.source_package_id);
    assert_eq!(library.sources.len(), 1);
    assert_eq!(library.sources[0].internal_id, "mus023");
    assert_eq!(library.sources[0].display_names["en"], "Dragon Soul");
    assert_eq!(read_fixture_manifest(&f.app_data), manifest);
    f.uninstall();
}
