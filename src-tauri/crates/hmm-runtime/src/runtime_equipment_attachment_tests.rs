use super::*;
use hmm_app::{
    EquipmentRetargetReinstallTaskExecutor, ReinstallTaskExecutor, ReinstallTaskPrepared,
};
use hmm_core::{InstallTargetPath, PackageFileId};

#[path = "runtime_equipment_reapply_tests.rs"]
mod reapply;
#[path = "runtime_equipment_attachment_recovery_tests.rs"]
mod recovery;

const PLUGIN: &str = "nativePC/plugins/attachment_fixture.dll";
const TOOL: &str = "nativePC/wp/one/one001/mod/attachment_tool.exe";
const PLUGIN_BYTES: &[u8] = b"inert managed attachment";
const TOOL_BYTES: &[u8] = b"inert author tool";

struct Fixture {
    _temp: tempfile::TempDir,
    state: HmmRuntime,
    app_data: PathBuf,
    game: PathBuf,
    package: PathBuf,
    mod_id: ModId,
    baseline: std::collections::BTreeMap<String, Vec<u8>>,
    manifests: Arc<FailNextManifestSaveRepository>,
}

impl Fixture {
    fn new(single: bool, ordinary: bool) -> Self {
        Self::with_layout(single, ordinary, false)
    }

    fn with_layout(single: bool, ordinary: bool, sandbox: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        let game = if sandbox {
            app_data.join("fixtures/games/mhw-minimal")
        } else {
            temp.path().join("game")
        };
        if sandbox {
            fs::create_dir_all(&app_data).unwrap();
            fs::write(
                app_data.join(crate::SANDBOX_MARKER_FILE_NAME),
                crate::SANDBOX_MARKER_SCHEMA,
            )
            .unwrap();
        }
        prepare_game_root(&game);
        fs::create_dir_all(game.join("nativePC/plugins")).unwrap();
        fs::write(game.join(PLUGIN), b"original external file").unwrap();
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
        let mut files = if single {
            EQUIPMENT_FILES[..5].to_vec()
        } else {
            EQUIPMENT_FILES.to_vec()
        };
        files.extend([(PLUGIN, PLUGIN_BYTES), (TOOL, TOOL_BYTES)]);
        let archive = temp.path().join("attachments.zip");
        create_fixture_zip(&archive, &files);
        let mod_id = import_equipment(&state, archive);
        if ordinary {
            install_fixture_revision(&state, &mod_id, &ProfileId::new("default"));
        } else {
            install_equipment(&state, selection(&state, &mod_id, "one001", None));
        }
        let package = app_data.join("mod-import/sandboxes").join(
            state
                .replacement_workflow
                .replacement_summary(
                    hmm_app::AnalyzeImportedReplacementRequest {
                        game_id: GameId::mhw(),
                        mod_id: mod_id.clone(),
                    },
                    None,
                )
                .unwrap()
                .package_id,
        );
        Self {
            _temp: temp,
            state,
            app_data,
            game,
            package,
            mod_id,
            baseline,
            manifests,
        }
    }

    fn request(&self) -> EquipmentRetargetReinstallRequest {
        selection(&self.state, &self.mod_id, "one002", None)
    }

    fn preview(&self) -> ReinstallPlanPreview {
        self.state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(self.request())
            .unwrap()
    }

    fn run(&self, single: bool) {
        if single {
            let selection = RetargetReinstallRequest {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                mod_id: self.mod_id.clone(),
                target_id: target("one002", "wp/one"),
                layer: FileLayer::new("base", 0),
            };
            let preview = self
                .state
                .reinstall_executor
                .preview_retarget_reinstall(selection.clone())
                .unwrap();
            assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
            let request = StartRetargetReinstallTaskRequest {
                game_id: selection.game_id,
                profile_id: selection.profile_id,
                mod_id: selection.mod_id,
                target_id: selection.target_id,
                layer: selection.layer,
                plan_token: preview.plan_token.unwrap(),
            };
            let task = self
                .state
                .reinstall_tasks
                .start_retarget_reinstall_task(request.clone())
                .unwrap();
            self.state
                .reinstall_task_runner
                .run_retarget_reinstall_task(&task.task_id, request)
                .unwrap();
        } else {
            let preview = self.preview();
            assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
            let request = StartEquipmentRetargetReinstallTaskRequest {
                selection: self.request(),
                plan_token: preview.plan_token.unwrap(),
            };
            let task = self
                .state
                .reinstall_tasks
                .start_equipment_retarget_reinstall_task(request.clone())
                .unwrap();
            self.state
                .reinstall_task_runner
                .run_equipment_retarget_reinstall_task(&task.task_id, request)
                .unwrap();
        }
    }
}

#[test]
fn attachments_survive_single_multi_and_verified_legacy_switches_with_backup_ownership() {
    for single in [true, false] {
        for legacy in [true, false] {
            let fixture = Fixture::new(single, true);
            let mut before = read_fixture_manifest(&fixture.app_data);
            if legacy {
                before.schema_version = 1;
                before.replacement_bindings.clear();
                for entry in &mut before.entries {
                    entry.revision_id = None;
                }
                fixture.manifests.save_manifest(&before).unwrap();
            }
            let before_tree = snapshot_file_tree(&fixture.game);
            let attachment_times = [PLUGIN, TOOL].map(|path| {
                let file = fs::OpenOptions::new()
                    .write(true)
                    .open(fixture.game.join(path))
                    .unwrap();
                file.set_times(fs::FileTimes::new().set_modified(
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000),
                ))
                .unwrap();
                fs::metadata(fixture.game.join(path))
                    .unwrap()
                    .modified()
                    .unwrap()
            });
            let before_json =
                fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap();
            let preview = fixture.preview();
            assert_eq!(
                preview.attachment_counts,
                hmm_app::ReinstallAttachmentCounts {
                    retained: 2,
                    excluded: 0
                }
            );
            assert_eq!(snapshot_file_tree(&fixture.game), before_tree);
            assert_eq!(
                fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap(),
                before_json
            );
            fixture.run(single);
            assert_eq!(fs::read(fixture.game.join(PLUGIN)).unwrap(), PLUGIN_BYTES);
            assert_eq!(fs::read(fixture.game.join(TOOL)).unwrap(), TOOL_BYTES);
            for (path, before_time) in [PLUGIN, TOOL].into_iter().zip(attachment_times) {
                assert_eq!(
                    fs::metadata(fixture.game.join(path))
                        .unwrap()
                        .modified()
                        .unwrap(),
                    before_time,
                    "retained attachment must not be rewritten"
                );
            }
            assert!(!fixture
                .game
                .join("nativePC/wp/one/one001/mod/one001.mod3")
                .exists());
            assert!(fixture
                .game
                .join("nativePC/wp/one/one002/mod/one002.mod3")
                .is_file());
            let after = read_fixture_manifest(&fixture.app_data);
            for path in [PLUGIN, TOOL] {
                let old = before
                    .entries
                    .iter()
                    .find(|entry| entry.target_path.as_str() == path)
                    .unwrap();
                let kept = after
                    .entries
                    .iter()
                    .find(|entry| entry.target_path.as_str() == path)
                    .unwrap();
                assert_eq!(kept.package_file_id, old.package_file_id);
                assert_eq!(kept.layer, old.layer);
                assert_eq!(kept.installed_file, old.installed_file);
                assert_eq!(kept.backup_ref, old.backup_ref);
            }
            let Fixture {
                _temp,
                state,
                app_data,
                game,
                mod_id,
                baseline,
                ..
            } = fixture;
            drop(state);
            let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
            let uninstall = StartUninstallTaskRequest {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                mod_id,
            };
            let task = state
                .uninstall_tasks
                .start_uninstall_task(uninstall.clone())
                .unwrap();
            state
                .uninstall_task_runner
                .run_uninstall_task(&task.task_id, uninstall)
                .unwrap();
            assert_eq!(snapshot_file_tree(&game), baseline);
            assert_no_reinstall_recovery_transactions(&app_data);
            assert_no_retarget_staging(&app_data);
        }
    }
}

#[test]
fn excluded_files_never_installed_are_not_added_or_adopted_on_a_target_switch() {
    let fixture = Fixture::new(false, false);
    let preview = fixture.preview();
    assert_eq!(
        preview.attachment_counts,
        hmm_app::ReinstallAttachmentCounts {
            retained: 0,
            excluded: 2
        }
    );
    fixture.run(false);
    assert_eq!(
        fs::read(fixture.game.join(PLUGIN)).unwrap(),
        b"original external file"
    );
    assert!(!fixture.game.join(TOOL).exists());
    assert!(read_fixture_manifest(&fixture.app_data)
        .entries
        .iter()
        .all(|entry| ![PLUGIN, TOOL].contains(&entry.target_path.as_str())));
}

#[test]
fn an_excluded_file_owned_by_another_mod_stays_with_that_owner() {
    use sha2::{Digest, Sha256};
    let fixture = Fixture::new(false, false);
    let mut manifest = read_fixture_manifest(&fixture.app_data);
    let mut foreign = manifest.entries[0].clone();
    foreign.mod_id = ModId::new("other-mod");
    foreign.revision_id = Some(ModRevisionId::new("other-revision"));
    foreign.package_file_id = PackageFileId::new("other-plugin");
    foreign.target_path = InstallTargetPath::parse(PLUGIN, ["nativePC"]).unwrap();
    foreign.backup_ref = None;
    let bytes = fs::read(fixture.game.join(PLUGIN)).unwrap();
    foreign.installed_file = Some(hmm_core::InstalledFileSummary {
        size_bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
    });
    manifest.entries.push(foreign.clone());
    fixture.manifests.save_manifest(&manifest).unwrap();
    fixture.run(false);
    let after = read_fixture_manifest(&fixture.app_data);
    assert_eq!(
        after
            .entries
            .iter()
            .find(|entry| entry.target_path.as_str() == PLUGIN),
        Some(&foreign)
    );
    assert_eq!(fs::read(fixture.game.join(PLUGIN)).unwrap(), bytes);
}

#[test]
fn unverified_attachment_metadata_or_bytes_block_without_deleting_player_files() {
    for problem in [
        "adopted",
        "summary",
        "source",
        "source-missing",
        "game",
        "missing",
        "file-id",
        "path",
    ] {
        let fixture = Fixture::new(false, true);
        let mut manifest = read_fixture_manifest(&fixture.app_data);
        let entry = manifest
            .entries
            .iter_mut()
            .find(|entry| entry.target_path.as_str() == PLUGIN)
            .unwrap();
        match problem {
            "adopted" => {
                entry.adopted = true;
                entry.backup_ref = None;
            }
            "summary" => entry.installed_file = None,
            "source" => fs::write(
                fixture.package.join(entry.package_file_id.as_str()),
                b"modified package",
            )
            .unwrap(),
            "source-missing" => {
                fs::remove_file(fixture.package.join(entry.package_file_id.as_str())).unwrap()
            }
            "game" => fs::write(fixture.game.join(PLUGIN), b"modified game file").unwrap(),
            "missing" => fs::remove_file(fixture.game.join(PLUGIN)).unwrap(),
            "file-id" => entry.package_file_id = PackageFileId::new("different-file"),
            "path" => {
                entry.target_path =
                    InstallTargetPath::parse("nativePC/plugins/other.dll", ["nativePC"]).unwrap()
            }
            _ => unreachable!(),
        }
        fixture.manifests.save_manifest(&manifest).unwrap();
        let before = snapshot_file_tree(&fixture.game);
        assert_eq!(
            fixture.preview().status,
            ReinstallPreviewStatus::Blocked,
            "accepted {problem}"
        );
        assert_eq!(snapshot_file_tree(&fixture.game), before);
        assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
    }
}

#[test]
fn attachment_source_or_game_changes_after_staging_make_commit_stale() {
    for source in [true, false] {
        let fixture = Fixture::new(false, true);
        let before_manifest = read_fixture_manifest(&fixture.app_data);
        let prepared = fixture
            .state
            .reinstall_executor
            .prepare_equipment_retarget_reinstall(fixture.request())
            .unwrap();
        let token = prepared.plan_token().to_owned();
        let entry = before_manifest
            .entries
            .iter()
            .find(|entry| entry.target_path.as_str() == PLUGIN)
            .unwrap();
        let path = if source {
            fixture.package.join(entry.package_file_id.as_str())
        } else {
            fixture.game.join(PLUGIN)
        };
        fs::write(path, b"changed after staging").unwrap();
        let before_tree = snapshot_file_tree(&fixture.game);
        assert_eq!(
            fixture
                .state
                .reinstall_executor
                .commit(prepared, &token)
                .unwrap_err(),
            hmm_app::ReinstallCommitError::PreviewStale
        );
        assert_eq!(snapshot_file_tree(&fixture.game), before_tree);
        assert_eq!(read_fixture_manifest(&fixture.app_data), before_manifest);
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
        assert_no_retarget_staging(&fixture.app_data);
    }
}

#[test]
fn manifest_failure_rolls_back_equipment_while_retaining_attachments_and_the_old_manifest() {
    let fixture = Fixture::new(false, true);
    let before = snapshot_file_tree(&fixture.game);
    let manifest = read_fixture_manifest(&fixture.app_data);
    let preview = fixture.preview();
    let request = StartEquipmentRetargetReinstallTaskRequest {
        selection: fixture.request(),
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
    assert_no_reinstall_recovery_transactions(&fixture.app_data);
}

#[test]
fn missing_staged_equipment_cannot_fall_back_to_the_attachment_package_reader() {
    let fixture = Fixture::new(false, true);
    let manifest = read_fixture_manifest(&fixture.app_data);
    let before = snapshot_file_tree(&fixture.game);
    let prepared = fixture
        .state
        .reinstall_executor
        .prepare_equipment_retarget_reinstall(fixture.request())
        .unwrap();
    let token = prepared.plan_token().to_owned();
    let staging_root = fixture.app_data.join("install/retarget-staging");
    let staged = snapshot_file_tree(&staging_root);
    let paths = staged
        .keys()
        .filter(|path| path.ends_with("nativePC/wp/one/one002/mod/one002.mod3"))
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1);
    fs::remove_file(staging_root.join(paths[0])).unwrap();
    assert!(fixture
        .state
        .reinstall_executor
        .commit(prepared, &token)
        .is_err());
    assert_eq!(snapshot_file_tree(&fixture.game), before);
    assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
    assert_no_reinstall_recovery_transactions(&fixture.app_data);
    assert_no_retarget_staging(&fixture.app_data);
}

#[test]
fn batch_retarget_preview_and_commit_share_attachment_retention_facts() {
    use hmm_core::{
        BatchExecutionPolicy, BatchItemInput, BatchOperation, BatchPlanRequest,
        ReinstallBatchItemInput, BATCH_PLAN_SCHEMA_VERSION,
    };
    let fixture = Fixture::with_layout(true, true, true);
    let manifest = read_fixture_manifest(&fixture.app_data);
    let before = snapshot_file_tree(&fixture.game);
    let revision = manifest.entries[0].revision_id.clone().unwrap();
    let request = crate::BatchLifecyclePlanRequest {
        plan: BatchPlanRequest {
            schema_version: BATCH_PLAN_SCHEMA_VERSION,
            operation: BatchOperation::Reinstall,
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            execution_policy: BatchExecutionPolicy::StopOnFailure,
            items: vec![BatchItemInput::Reinstall(ReinstallBatchItemInput {
                intent: Default::default(),
                mod_id: fixture.mod_id.clone(),
                installed_revision_id: revision.clone(),
                candidate_revision_id: revision,
                layer: FileLayer::new("base", 0),
                replacement_binding_snapshot: None,
            })],
        },
        replacement_targets: std::collections::BTreeMap::from([(
            fixture.mod_id.clone(),
            target("one002", "wp/one"),
        )]),
    };
    let environment = crate::RuntimeEnvironment::sandbox(fixture.app_data.clone()).unwrap();
    let preview =
        crate::BatchLifecycleAutomation::preview_request(&environment, request.clone()).unwrap();
    assert_eq!(preview.plan.status(), hmm_core::BatchPlanStatus::Ready);
    assert_eq!(snapshot_file_tree(&fixture.game), before);
    assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
    let database = fixture.state.database_handle();
    let (_, sealed) = crate::BatchLifecycleAutomation::seal_request_with_database(
        &environment,
        request,
        preview.preview_token.as_deref().unwrap(),
        Arc::clone(&database),
    )
    .unwrap();
    let (_, result) = crate::BatchLifecycleAutomation::start_request_with_database(
        &environment,
        &sealed.batch_id,
        &sealed.plan_token,
        database,
    )
    .unwrap();
    assert_eq!(result.status, hmm_core::BatchAttemptStatus::Completed);
    assert_eq!(fs::read(fixture.game.join(PLUGIN)).unwrap(), PLUGIN_BYTES);
    assert_eq!(fs::read(fixture.game.join(TOOL)).unwrap(), TOOL_BYTES);
    assert!(!fixture
        .game
        .join("nativePC/wp/one/one001/mod/one001.mod3")
        .exists());
    assert_eq!(
        fs::read(fixture.game.join("nativePC/wp/one/one002/mod/one002.mod3")).unwrap(),
        EQUIPMENT_FILES[0].1
    );
    let after = read_fixture_manifest(&fixture.app_data);
    for path in [PLUGIN, TOOL] {
        let old = manifest
            .entries
            .iter()
            .find(|entry| entry.target_path.as_str() == path)
            .unwrap();
        let kept = after
            .entries
            .iter()
            .find(|entry| entry.target_path.as_str() == path)
            .unwrap();
        assert_eq!(kept, old);
    }
    assert_no_reinstall_recovery_transactions(&fixture.app_data);
    assert_no_retarget_staging(&fixture.app_data);
}
