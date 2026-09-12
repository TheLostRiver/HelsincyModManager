use super::*;
use hmm_app::{
    EquipmentRetargetReinstallTaskExecutor, ReinstallTaskExecutor, ReinstallTaskPrepared,
};
use hmm_core::InstallTargetPath;
use sha2::{Digest, Sha256};
use std::panic::{catch_unwind, AssertUnwindSafe};

const CURRENT: [&str; 5] = [
    "nativePC/wp/two/two029/mod/two029/two029.mod3",
    "nativePC/wp/two/two029/mod/two029/custom.mrl3",
    "nativePC/wp/two/two028/mod/two028/skin.tex",
    "nativePC/wp/two/two003/mod/two003/two003.mod3",
    "nativePC/wp/two/two003/mod/two003/custom.mrl3",
];
const LEGACY: [&str; 5] = [
    "nativePC/wp/two/two029/mod/two028/two029.mod3",
    "nativePC/wp/two/two028/mod/two028/custom.mrl3",
    "nativePC/wp/two/two028/mod/two028/skin.tex",
    "nativePC/wp/two/two003/mod/two020/two003.mod3",
    "nativePC/wp/two/two020/mod/two020/custom.mrl3",
];

fn legacy_destination(original: &str) -> &str {
    FILES
        .iter()
        .position(|(path, _)| *path == original)
        .map_or(original, |index| LEGACY[index])
}

/// 用人工文件和明确的 v1 输出表重建存量安装，不在生产代码中保留旧映射入口。
fn legacy_fixture() -> Fixture {
    let fixture = Fixture::new(FILES);
    let selection = fixture.choices(&[("two028", "two029"), ("two020", "two003")]);
    let preflight = fixture
        .state
        .initial_retarget_install_preflight
        .preview(initial_request(selection.clone()))
        .unwrap();
    install_fixture_revision(&fixture.state, &fixture.mod_id, &ProfileId::new("default"));
    fixture.run(selection);
    fixture.assert_layout(&CURRENT);
    let mut manifest = read_fixture_manifest(&fixture.app_data);
    for (current, legacy) in CURRENT
        .into_iter()
        .zip(LEGACY)
        .filter(|(current, legacy)| current != legacy)
    {
        assert!(!fixture.game.join(legacy).exists());
        fs::create_dir_all(fixture.game.join(legacy).parent().unwrap()).unwrap();
        fs::rename(fixture.game.join(current), fixture.game.join(legacy)).unwrap();
        let entry = manifest
            .entries
            .iter_mut()
            .find(|entry| entry.target_path.as_str() == current)
            .unwrap();
        entry.target_path = InstallTargetPath::parse(legacy, ["nativePC"]).unwrap();
    }
    for binding in &mut manifest.replacement_bindings {
        let plan = preflight
            .planned
            .retarget_plans()
            .iter()
            .find(|plan| plan.source().id() == binding.binding().source_id())
            .unwrap();
        let mut closure = Sha256::new();
        for action in plan.actions() {
            for value in [
                action.package_file_id().as_str(),
                action.source_relative_path().as_str(),
                legacy_destination(action.source_relative_path().as_str()),
            ] {
                closure.update((value.len() as u64).to_le_bytes());
                closure.update(value.as_bytes());
            }
        }
        let mut facts = serde_json::to_value(plan.adapter_facts().unwrap()).unwrap();
        facts["strategy_version"] = serde_json::json!(1);
        facts["source_closure_sha256"] = serde_json::json!(format!("{:x}", closure.finalize()));
        *binding = binding
            .clone()
            .with_adapter_facts(serde_json::from_value(facts).unwrap());
    }
    fixture.manifests.save_manifest(&manifest).unwrap();
    fixture.assert_layout(&LEGACY);
    fixture.restart()
}

#[test]
fn legacy_v1_reapply_updates_nested_paths_keeps_targets_and_uninstalls_to_baseline() {
    let fixture = legacy_fixture();
    let before = read_fixture_manifest(&fixture.app_data);
    let summary = fixture
        .state
        .replacement_workflow
        .replacement_summary(
            hmm_app::AnalyzeImportedReplacementRequest {
                game_id: GameId::mhw(),
                mod_id: fixture.mod_id.clone(),
            },
            None,
        )
        .unwrap();
    let package = fixture
        .app_data
        .join("mod-import/sandboxes")
        .join(summary.package_id);
    let original_package = snapshot_file_tree(&package);
    assert!(before.replacement_bindings.iter().all(|binding| binding
        .adapter_facts()
        .unwrap()
        .strategy_version()
        == 1));
    let preview = fixture
        .state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(fixture.reapply())
        .unwrap();
    assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
    assert_eq!((preview.counts.added, preview.counts.stale), (4, 4));
    fixture.assert_layout(&LEGACY);
    for (legacy, current) in LEGACY
        .into_iter()
        .zip(CURRENT)
        .filter(|(legacy, current)| legacy != current)
    {
        assert!(preview.file_effects.iter().any(|effect| {
            effect
                .installed_path
                .as_ref()
                .is_some_and(|path| path.as_str() == legacy)
                && effect
                    .effect
                    .target_path
                    .as_ref()
                    .is_some_and(|path| path.as_str() == current)
        }));
    }
    fixture.run(fixture.reapply());
    fixture.assert_layout(&CURRENT);
    assert_eq!(
        snapshot_file_tree(&package),
        original_package,
        "the imported package stays immutable"
    );
    let after = read_fixture_manifest(&fixture.app_data);
    assert_eq!(
        after.replacement_bindings.len(),
        before.replacement_bindings.len()
    );
    for previous in &before.replacement_bindings {
        let current = after
            .replacement_bindings
            .iter()
            .find(|binding| binding.binding_id() == previous.binding_id())
            .unwrap();
        assert_eq!(current.binding(), previous.binding());
        assert_eq!(current.target_internal_id(), previous.target_internal_id());
        assert_eq!(current.revision_id(), previous.revision_id());
        assert_eq!(current.adapter_facts().unwrap().strategy_version(), 2);
    }
    assert_eq!(
        after
            .entries
            .iter()
            .find(|entry| entry.target_path.as_str() == PLUGIN),
        before
            .entries
            .iter()
            .find(|entry| entry.target_path.as_str() == PLUGIN),
        "the installed attachment keeps ownership and its original backup",
    );
    let fixture = fixture.restart();
    let preview = fixture
        .state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(fixture.reapply())
        .unwrap();
    assert_eq!(preview.status, ReinstallPreviewStatus::NoChanges);
    assert!(preview.plan_token.is_none());
    assert_eq!(read_fixture_manifest(&fixture.app_data), after);
    fixture.uninstall();
}

#[test]
fn legacy_v1_reapply_manifest_failure_restores_paths_and_v1_facts() {
    let fixture = legacy_fixture();
    let before = read_fixture_manifest(&fixture.app_data);
    let prepared = fixture
        .state
        .reinstall_executor
        .prepare_equipment_retarget_reinstall(fixture.reapply())
        .unwrap();
    let token = prepared.plan_token().to_owned();
    fixture.manifests.fail_next_save();
    assert!(fixture
        .state
        .reinstall_executor
        .commit(prepared, &token)
        .is_err());
    fixture.assert_layout(&LEGACY);
    assert_eq!(read_fixture_manifest(&fixture.app_data), before);
    assert_no_retarget_staging(&fixture.app_data);
    assert_no_reinstall_recovery_transactions(&fixture.app_data);
    fixture.uninstall();
}

#[test]
fn a_policy_version_change_alone_does_not_rewrite_a_matching_legacy_install() {
    let fixture = Fixture::new(&[(
        "nativePC/wp/two/two028/mod/two028.mod3",
        b"flat legacy model",
    )]);
    install_equipment(&fixture.state, fixture.choices(&[("two028", "two029")]));
    let mut manifest = read_fixture_manifest(&fixture.app_data);
    for binding in &mut manifest.replacement_bindings {
        let mut facts = serde_json::to_value(binding.adapter_facts().unwrap()).unwrap();
        facts["strategy_version"] = serde_json::json!(1);
        *binding = binding
            .clone()
            .with_adapter_facts(serde_json::from_value(facts).unwrap());
    }
    fixture.manifests.save_manifest(&manifest).unwrap();
    let before = snapshot_file_tree(&fixture.game);
    let manifest_bytes = fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap();
    let fixture = fixture.restart();
    let preview = fixture
        .state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(fixture.reapply())
        .unwrap();
    assert_eq!(preview.status, ReinstallPreviewStatus::NoChanges);
    assert!(preview.plan_token.is_none());
    assert_eq!(snapshot_file_tree(&fixture.game), before);
    assert_eq!(
        fs::read(fixture.app_data.join("install/manifests/default.json")).unwrap(),
        manifest_bytes
    );
    assert_no_reinstall_recovery_transactions(&fixture.app_data);
    fixture.uninstall();
}

struct CrashBeforeManifest {
    repository: JsonInstallManifestRepository,
    crash: AtomicBool,
}

impl InstallManifestRepository for CrashBeforeManifest {
    fn load_manifest(&self, profile: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        self.repository.load_manifest(profile)
    }
    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        assert!(
            !self.crash.swap(false, Ordering::SeqCst),
            "synthetic interruption before manifest save"
        );
        self.repository.save_manifest(manifest)
    }
}

#[test]
fn interrupted_v1_upgrade_recovers_the_original_paths_and_policy_facts_after_restart() {
    let Fixture {
        _temp,
        state,
        app_data,
        game,
        mod_id,
        ..
    } = legacy_fixture();
    let before = snapshot_file_tree(&game);
    let before_manifest = read_fixture_manifest(&app_data);
    drop(state);
    let manifests = Arc::new(CrashBeforeManifest {
        repository: JsonInstallManifestRepository::new(app_data.join("install/manifests")),
        crash: AtomicBool::new(false),
    });
    let state = HmmRuntime::builder(app_data.clone())
        .with_install_manifest_repository(manifests.clone())
        .build()
        .unwrap();
    let selection = EquipmentRetargetReinstallRequest::reapply(
        GameId::mhw(),
        ProfileId::new("default"),
        mod_id.clone(),
    );
    let preview = state
        .reinstall_executor
        .preview_equipment_retarget_reinstall(selection.clone())
        .unwrap();
    let request = StartEquipmentRetargetReinstallTaskRequest {
        selection,
        plan_token: preview.plan_token.unwrap(),
    };
    let task = state
        .reinstall_tasks
        .start_equipment_retarget_reinstall_task(request.clone())
        .unwrap();
    manifests.crash.store(true, Ordering::SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = state
            .reinstall_task_runner
            .run_equipment_retarget_reinstall_task(&task.task_id, request);
    }))
    .is_err());
    let transactions = state
        .reinstall_recovery_repository
        .list_transactions(&ProfileId::new("default"))
        .unwrap();
    assert_eq!(transactions.len(), 1);
    assert_eq!(
        transactions[0].intent,
        hmm_core::ReinstallIntent::ReapplyEquipmentTargets
    );
    assert_eq!(transactions[0].pre_reinstall_manifest, before_manifest);
    transactions[0].validate().unwrap();
    assert_ne!(snapshot_file_tree(&game), before);
    drop(state);
    let state = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
    let statuses = state
        .install_recovery_scanner
        .scan(
            GameId::mhw(),
            InstallRecoveryScanRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![mod_id.clone()],
            },
        )
        .unwrap();
    assert_eq!(statuses[0].status, InstallRecoveryStatus::RollbackRequired);
    let request = hmm_app::StartRecoveryActionTaskRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        mod_id,
        action_kind: hmm_app::InstallRecoveryActionKind::ReconcileReinstall,
        plan_token: None,
    };
    let task = state
        .recovery_action_tasks
        .start_recovery_action_task(request.clone())
        .unwrap();
    state
        .recovery_action_task_runner
        .run_recovery_action_task(&task.task_id, request)
        .unwrap();
    assert_eq!(snapshot_file_tree(&game), before);
    assert_eq!(read_fixture_manifest(&app_data), before_manifest);
    assert_no_reinstall_recovery_transactions(&app_data);
    assert_no_retarget_staging(&app_data);
}
