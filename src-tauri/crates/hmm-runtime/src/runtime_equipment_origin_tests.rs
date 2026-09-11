use super::*;
use hmm_core::{InstallManifestEntry, InstalledFileSummary};
use sha2::{Digest, Sha256};

#[path = "runtime_equipment_origin_commit_tests.rs"]
mod commit;

struct LegacyFixture {
    _temp: tempfile::TempDir,
    state: HmmRuntime,
    app_data: PathBuf,
    game: PathBuf,
    package: PathBuf,
    mod_id: ModId,
    revision: ModRevisionId,
    manifest: InstallManifest,
    manifests: Arc<FailNextManifestSaveRepository>,
}

impl LegacyFixture {
    fn new(revisioned: bool) -> Self {
        Self::with_files(revisioned, EQUIPMENT_FILES)
    }

    fn with_files(revisioned: bool, files: &[(&str, &[u8])]) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        let game = temp.path().join("game");
        prepare_game_root(&game);
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
        let archive = temp.path().join("legacy-equipment.zip");
        create_fixture_zip(&archive, files);
        let (_, mod_id, revision) = import_initial_fixture_revision(&state, &archive);
        let plan = state
            .install_preflight
            .preview_revision(
                &GameId::mhw(),
                &mod_id,
                &revision,
                &FileLayer::new("base", 0),
            )
            .unwrap()
            .plan;
        let entries = plan
            .actions
            .iter()
            .map(|action| {
                let (_, bytes) = files
                    .iter()
                    .find(|(path, _)| *path == action.target_path.as_str())
                    .unwrap();
                let file = game.join(action.target_path.as_str());
                fs::create_dir_all(file.parent().unwrap()).unwrap();
                fs::write(file, bytes).unwrap();
                InstallManifestEntry {
                    target_path: action.target_path.clone(),
                    mod_id: mod_id.clone(),
                    revision_id: revisioned.then(|| revision.clone()),
                    package_file_id: action.provider.package_file_id.clone(),
                    layer: action.provider.layer.clone(),
                    backup_ref: None,
                    installed_file: Some(InstalledFileSummary {
                        size_bytes: bytes.len() as u64,
                        sha256: format!("{:x}", Sha256::digest(bytes)),
                    }),
                    adopted: false,
                }
            })
            .collect();
        let mut manifest = InstallManifest::completed(ProfileId::new("default"), entries);
        manifest.schema_version = if revisioned { 2 } else { 1 };
        manifest.backend = Some("install_plan".to_owned());
        manifest.validate().unwrap();
        let package_id = state
            .replacement_workflow
            .replacement_summary(
                hmm_app::AnalyzeImportedReplacementRequest {
                    game_id: GameId::mhw(),
                    mod_id: mod_id.clone(),
                },
                None,
            )
            .unwrap()
            .package_id;
        let package = app_data.join("mod-import/sandboxes").join(package_id);
        assert!(package.is_dir());
        let fixture = Self {
            _temp: temp,
            state,
            app_data,
            game,
            package,
            mod_id,
            revision,
            manifest,
            manifests,
        };
        fixture.save_manifest();
        fixture
    }

    fn save_manifest(&self) {
        let file = self.app_data.join("install/manifests/default.json");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, serde_json::to_vec(&self.manifest).unwrap()).unwrap();
    }

    fn preview(&self) -> ReinstallPlanPreview {
        self.state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(selection(
                &self.state,
                &self.mod_id,
                "one002",
                Some("pl129_0000"),
            ))
            .unwrap()
    }
}

#[test]
fn verified_original_install_without_bindings_can_preview_a_target_switch() {
    for revisioned in [true, false] {
        let fixture = LegacyFixture::new(revisioned);
        let before_files = snapshot_file_tree(&fixture.game);
        let before_manifest =
            fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap();
        let preview = fixture.preview();
        assert_eq!(
            preview.status,
            ReinstallPreviewStatus::Ready,
            "matching original files must establish origin: {:?}",
            preview.blocking_reasons
        );
        assert!(preview.plan_token.is_some());
        assert_eq!(
            preview.installed_revision.unwrap().revision_id,
            fixture.revision
        );
        assert_eq!(snapshot_file_tree(&fixture.game), before_files);
        assert_eq!(
            fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap(),
            before_manifest
        );
        assert!(read_fixture_manifest(&fixture.app_data)
            .replacement_bindings
            .is_empty());
        assert_no_retarget_staging(&fixture.app_data);
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
    }
}

#[test]
fn origin_recovery_rejects_changed_missing_unowned_and_incomplete_facts() {
    for problem in [
        "source", "target", "missing", "adopted", "summary", "file-set",
    ] {
        let mut fixture = LegacyFixture::new(true);
        let path = fixture.manifest.entries[0].target_path.as_str().to_owned();
        match problem {
            "source" => {
                fs::write(
                    fixture
                        .package
                        .join(fixture.manifest.entries[0].package_file_id.as_str()),
                    b"changed cached source",
                )
                .unwrap();
            }
            "target" => {
                fs::write(fixture.game.join(&path), b"changed installed contents").unwrap();
            }
            "missing" => {
                fs::remove_file(fixture.game.join(&path)).unwrap();
            }
            "adopted" => {
                fixture.manifest.entries[0].adopted = true;
            }
            "summary" => {
                fixture.manifest.entries[0].installed_file = None;
            }
            "file-set" => {
                fixture.manifest.entries.pop();
            }
            _ => unreachable!(),
        }
        fixture.save_manifest();
        let before = snapshot_file_tree(&fixture.game);
        let manifest = fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap();
        assert_eq!(
            fixture.preview().status,
            ReinstallPreviewStatus::Blocked,
            "accepted {problem}"
        );
        assert_eq!(snapshot_file_tree(&fixture.game), before);
        assert_eq!(
            fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap(),
            manifest
        );
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
    }
}

#[test]
fn missing_revision_with_multiple_imported_versions_is_not_guessed() {
    let fixture = LegacyFixture::new(false);
    let archive = fixture._temp.path().join("another-version.zip");
    create_fixture_zip(
        &archive,
        &[("nativePC/wp/one/one004/mod/one004.mod3", b"another version")],
    );
    import_candidate_fixture_revision(&fixture.state, &archive, &fixture.mod_id, &fixture.revision);
    let preview = fixture.preview();
    assert_eq!(preview.status, ReinstallPreviewStatus::Blocked);
    assert!(preview
        .blocking_reasons
        .iter()
        .any(|reason| reason.reason == hmm_app::ReinstallBlockingReason::InstalledRevisionUnknown));
}

#[test]
fn recorded_old_revision_drives_source_display_and_selection_after_a_new_import() {
    for single in [true, false] {
        let fixture = LegacyFixture::with_files(
            true,
            if single {
                &EQUIPMENT_FILES[..3]
            } else {
                EQUIPMENT_FILES
            },
        );
        let before = fixture
            .state
            .replacement_workflow
            .equipment_configuration(
                &GameId::mhw(),
                &fixture.mod_id,
                Some(&ProfileId::new("default")),
            )
            .unwrap();
        let archive = fixture._temp.path().join("different-sources.zip");
        create_fixture_zip(
            &archive,
            &[(
                "nativePC/wp/swo/swo035/mod/swo035.mod3",
                b"different new source",
            )],
        );
        import_candidate_fixture_revision(
            &fixture.state,
            &archive,
            &fixture.mod_id,
            &fixture.revision,
        );
        let after = fixture
            .state
            .replacement_workflow
            .equipment_configuration(
                &GameId::mhw(),
                &fixture.mod_id,
                Some(&ProfileId::new("default")),
            )
            .unwrap();
        assert_eq!(
            after
                .sources
                .iter()
                .map(|item| item.source.id())
                .collect::<Vec<_>>(),
            before
                .sources
                .iter()
                .map(|item| item.source.id())
                .collect::<Vec<_>>()
        );
        let request = hmm_app::AnalyzeImportedReplacementRequest {
            game_id: GameId::mhw(),
            mod_id: fixture.mod_id.clone(),
        };
        let profiled = fixture
            .state
            .replacement_workflow
            .analyze_imported_mod_in_profile(request.clone(), Some(&ProfileId::new("default")))
            .unwrap();
        assert!(profiled
            .sources()
            .iter()
            .any(|source| source.internal_id() == "one001"));
        let unprofiled = fixture
            .state
            .replacement_workflow
            .analyze_imported_mod(request)
            .unwrap();
        assert_eq!(unprofiled.sources()[0].internal_id(), "swo035");
        if single {
            let targets = fixture
                .state
                .replacement_workflow
                .list_compatible_targets_in_profile(
                    &GameId::mhw(),
                    &fixture.mod_id,
                    Some(&ProfileId::new("default")),
                    None,
                )
                .unwrap();
            assert!(targets
                .iter()
                .any(|target| target.internal_id() == "one002"));
        } else {
            assert_eq!(fixture.preview().status, ReinstallPreviewStatus::Ready);
        }
    }
}

fn switch_request(fixture: &LegacyFixture) -> StartEquipmentRetargetReinstallTaskRequest {
    let preview = fixture.preview();
    assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
    StartEquipmentRetargetReinstallTaskRequest {
        selection: selection(
            &fixture.state,
            &fixture.mod_id,
            "one002",
            Some("pl129_0000"),
        ),
        plan_token: preview.plan_token.unwrap(),
    }
}

#[test]
fn verified_legacy_switch_persists_all_origins_and_remains_manageable_after_restart() {
    for revisioned in [true, false] {
        let fixture = LegacyFixture::new(revisioned);
        let before = snapshot_file_tree(&fixture.game);
        let request = switch_request(&fixture);
        let task = fixture
            .state
            .reinstall_tasks
            .start_equipment_retarget_reinstall_task(request.clone())
            .unwrap();
        let events = fixture
            .state
            .reinstall_task_runner
            .run_equipment_retarget_reinstall_task(&task.task_id, request)
            .unwrap();
        assert_eq!(events.last().unwrap().status, TaskStatus::Completed);
        let installed = read_fixture_manifest(&fixture.app_data);
        assert_eq!(installed.replacement_bindings.len(), 3);
        assert!(installed
            .replacement_bindings
            .iter()
            .all(|binding| binding.revision_id() == Some(&fixture.revision)));
        assert!(installed
            .entries
            .iter()
            .all(|entry| entry.revision_id.as_ref() == Some(&fixture.revision)));
        assert!(fixture.manifest.replacement_bindings.is_empty());
        let mut expected = before;
        for (old, new) in [
            (
                "nativePC/wp/one/one001/mod/one001.mod3",
                "nativePC/wp/one/one002/mod/one002.mod3",
            ),
            (
                "nativePC/wp/one/one001/mod/one001.mrl3",
                "nativePC/wp/one/one002/mod/one002.mrl3",
            ),
            (
                "nativePC/wp/one/one001/mod/ya001.mod3",
                "nativePC/wp/one/one002/mod/ya002.mod3",
            ),
            (ARMOR_SOURCE_TARGET, ARMOR_RETARGETED_TARGET),
        ] {
            let bytes = expected.remove(old).unwrap();
            expected.insert(new.to_owned(), bytes);
        }
        assert_eq!(snapshot_file_tree(&fixture.game), expected);
        let LegacyFixture {
            state,
            app_data,
            game,
            mod_id,
            ..
        } = fixture;
        drop(state);
        let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
        let config = state
            .replacement_workflow
            .equipment_configuration(&GameId::mhw(), &mod_id, Some(&ProfileId::new("default")))
            .unwrap();
        assert_eq!(config.installed_targets.unwrap().len(), 3);
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
        for (path, _) in EQUIPMENT_FILES {
            assert!(!game.join(path).exists());
        }
        assert_no_reinstall_recovery_transactions(&app_data);
    }
}

#[test]
fn origin_recovery_rolls_back_game_files_and_keeps_the_original_unbound_manifest() {
    let fixture = LegacyFixture::new(false);
    let before = snapshot_file_tree(&fixture.game);
    let request = switch_request(&fixture);
    let task = fixture
        .state
        .reinstall_tasks
        .start_equipment_retarget_reinstall_task(request.clone())
        .unwrap();
    fixture.manifests.fail_next_save();
    let error = fixture
        .state
        .reinstall_task_runner
        .run_equipment_retarget_reinstall_task(&task.task_id, request)
        .unwrap_err();
    assert_eq!(
        error.events.last().unwrap().error.as_deref(),
        Some("install_reinstall_failed:manifest")
    );
    assert_eq!(snapshot_file_tree(&fixture.game), before);
    assert_eq!(read_fixture_manifest(&fixture.app_data), fixture.manifest);
    assert_no_reinstall_recovery_transactions(&fixture.app_data);
    assert_no_retarget_staging(&fixture.app_data);
}

#[test]
fn changed_origin_after_preview_cannot_commit_or_leave_recovered_bindings() {
    let fixture = LegacyFixture::new(true);
    let request = switch_request(&fixture);
    let before = snapshot_file_tree(&fixture.game);
    fs::write(
        fixture
            .package
            .join(fixture.manifest.entries[0].package_file_id.as_str()),
        b"changed after preview",
    )
    .unwrap();
    let task = fixture
        .state
        .reinstall_tasks
        .start_equipment_retarget_reinstall_task(request.clone())
        .unwrap();
    assert!(fixture
        .state
        .reinstall_task_runner
        .run_equipment_retarget_reinstall_task(&task.task_id, request)
        .is_err());
    assert_eq!(snapshot_file_tree(&fixture.game), before);
    assert_eq!(read_fixture_manifest(&fixture.app_data), fixture.manifest);
    assert_no_reinstall_recovery_transactions(&fixture.app_data);
}
