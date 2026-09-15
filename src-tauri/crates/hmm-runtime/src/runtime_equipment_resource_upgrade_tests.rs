use super::*;
use hmm_app::{
    EquipmentRetargetReinstallTaskExecutor, ReinstallTaskExecutor, ReinstallTaskPrepared,
};
use hmm_core::{
    InstallTargetPath, ReplacementAdapterFacts, ReplacementBindingSnapshot, RetargetAction,
    RetargetPlan,
};
use sha2::{Digest, Sha256};

#[path = "runtime_equipment_resource_safety_tests.rs"]
mod safety;

struct ResourcePaths {
    original: [String; 4],
    current: [String; 4],
    source: &'static str,
    destination: &'static str,
}

impl ResourcePaths {
    fn new(armor: bool) -> Self {
        let (source, destination, prefix, source_model, target_model) = if armor {
            (
                "pl121_0000",
                "pl129_0000",
                "nativePC/pl/f_equip",
                "f_body121_0000",
                "f_body129_0000",
            )
        } else {
            ("two003", "two019", "nativePC/wp/two", "two003", "two019")
        };
        let part = if armor { "body/mod" } else { "mod" };
        let paths = |slot: &str, model: &str| {
            [
                format!("{prefix}/{slot}/{part}/{model}.mod3"),
                format!("{prefix}/{slot}/{part}/{model}.mrl3"),
                format!("{prefix}/{slot}/{part}/{model}_BML.tex"),
                format!("{prefix}/{slot}/{part}/author.dds"),
            ]
        };
        Self {
            original: paths(source, source_model),
            current: paths(destination, target_model),
            source,
            destination,
        }
    }

    fn reference(&self, current: bool) -> &str {
        let path = if current {
            &self.current[2]
        } else {
            &self.original[2]
        };
        &path[9..path.len() - 4]
    }

    fn legacy_path<'a>(&'a self, original: &'a str) -> &'a str {
        self.original[..2]
            .iter()
            .position(|path| path == original)
            .map_or(original, |index| self.current[index].as_str())
    }

    fn expected(&self, fixture: &Fixture) -> BTreeMap<String, Vec<u8>> {
        let mut expected = fixture.baseline.clone();
        for (index, path) in self.current.iter().enumerate() {
            let content = if index == 1 {
                single_texture_material(self.reference(true)).to_vec()
            } else {
                fixture.files[index].1.clone()
            };
            expected.insert(path.clone(), content);
        }
        expected.insert(PLUGIN.to_owned(), PLUGIN_BYTES.to_vec());
        expected.insert(COMPANION.to_owned(), COMPANION_BYTES.to_vec());
        expected
    }
}

/// 只在临时目录重建 v3：模型／材质已移走，贴图和作者资源仍在原目录，材质引用未更新。
fn legacy_resources(armor: bool) -> (Fixture, ResourcePaths) {
    let paths = ResourcePaths::new(armor);
    let material = single_texture_material(paths.reference(false));
    let mut fixture = Fixture::with_sandbox(
        &[
            (&paths.original[0], b"artificial model"),
            (&paths.original[1], &material),
            (&paths.original[2], b"artificial texture"),
            (&paths.original[3], b"author texture resource"),
        ],
        true,
    );
    fs::create_dir_all(fixture.game.join(&paths.original[2]).parent().unwrap()).unwrap();
    fs::write(
        fixture.game.join(&paths.original[2]),
        b"baseline texture before install",
    )
    .unwrap();
    let sentinel = format!("{}.unknown", paths.original[0]);
    fs::write(fixture.game.join(&sentinel), b"unmanaged user file").unwrap();
    // 同目录另一个 Mod 的文件不得被按目录清理。
    let other_path = format!("{}.other", paths.original[0]);
    let archive = fixture._temp.path().join("other-resource.zip");
    create_fixture_zip(&archive, &[(&other_path, b"another mod resource")]);
    let other = import_equipment(&fixture.state, archive);
    install_fixture_revision(&fixture.state, &other, &ProfileId::new("default"));
    fixture.baseline = snapshot_file_tree(&fixture.game);
    let preflight = fixture
        .state
        .initial_retarget_install_preflight
        .preview(initial_request(
            fixture.choices(&[(paths.source, paths.destination)]),
        ))
        .unwrap();
    fixture.install_legacy();
    let mut manifest = read_fixture_manifest(&fixture.app_data);
    for index in 0..2 {
        let original = &paths.original[index];
        let destination = &paths.current[index];
        fs::create_dir_all(fixture.game.join(destination).parent().unwrap()).unwrap();
        fs::rename(fixture.game.join(original), fixture.game.join(destination)).unwrap();
        let entry = manifest
            .entries
            .iter_mut()
            .find(|entry| entry.mod_id == fixture.mod_id && entry.target_path.as_str() == original)
            .unwrap();
        assert!(entry.backup_ref.is_none());
        entry.target_path = InstallTargetPath::parse(destination, ["nativePC"]).unwrap();
    }
    manifest
        .replacement_bindings
        .retain(|binding| binding.mod_id() != &fixture.mod_id);
    for plan in preflight.planned.retarget_plans() {
        let mut closure = Sha256::new();
        let actions = plan
            .actions()
            .iter()
            .map(|action| {
                let old_path = paths.legacy_path(action.source_relative_path().as_str());
                for value in [
                    action.package_file_id().as_str(),
                    action.source_relative_path().as_str(),
                    old_path,
                ] {
                    closure.update((value.len() as u64).to_le_bytes());
                    closure.update(value.as_bytes());
                }
                RetargetAction::new(
                    action.package_file_id().clone(),
                    action.source_relative_path().clone(),
                    InstallTargetPath::parse(old_path, ["nativePC"]).unwrap(),
                    plan.source().id().clone(),
                    plan.source().internal_id(),
                    paths.destination,
                    plan.source().path_family(),
                    plan.source().path_family(),
                )
                .unwrap()
            })
            .collect();
        let legacy = RetargetPlan::new(
            plan.binding().clone(),
            plan.source().clone(),
            actions,
            Vec::new(),
        )
        .unwrap();
        let facts = ReplacementAdapterFacts::new(
            1,
            "mhw.equipment",
            "path-only-resource-preserving",
            3,
            format!("{:x}", closure.finalize()),
            plan.adapter_facts().unwrap().part_set_sha256(),
            legacy.content_transform_set_sha256(),
        )
        .unwrap();
        manifest
            .replacement_bindings
            .push(ReplacementBindingSnapshot::from_retarget_plan(
                &legacy.with_adapter_facts(facts).unwrap(),
                Some(preflight.planned.revision_id().clone()),
            ));
    }
    fixture.manifests.save_manifest(&manifest).unwrap();
    let fixture = fixture.restart();
    (fixture, paths)
}

fn package_root(fixture: &Fixture) -> PathBuf {
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
    fixture
        .app_data
        .join("mod-import/sandboxes")
        .join(summary.package_id)
}

#[test]
fn v3_weapon_and_armor_reapply_remove_old_overrides_fix_materials_and_uninstall_exactly() {
    for armor in [false, true] {
        let (fixture, paths) = legacy_resources(armor);
        let original = snapshot_file_tree(&package_root(&fixture));
        let old_manifest = read_fixture_manifest(&fixture.app_data);
        let old_game = snapshot_file_tree(&fixture.game);
        let preview = fixture
            .state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(fixture.reapply())
            .unwrap();
        assert_eq!(preview.status, ReinstallPreviewStatus::Ready, "{preview:?}");
        assert_eq!(
            (
                preview.counts.added,
                preview.counts.stale,
                preview.counts.replaced
            ),
            (2, 2, 1)
        );
        assert_eq!(snapshot_file_tree(&fixture.game), old_game);
        fixture.run(fixture.reapply());
        assert_eq!(snapshot_file_tree(&fixture.game), paths.expected(&fixture));
        assert_eq!(snapshot_file_tree(&package_root(&fixture)), original);
        let current = read_fixture_manifest(&fixture.app_data);
        for old in &old_manifest.replacement_bindings {
            let now = current
                .replacement_bindings
                .iter()
                .find(|binding| binding.binding_id() == old.binding_id())
                .unwrap();
            assert_eq!(now.binding(), old.binding());
            assert_eq!(now.target_internal_id(), old.target_internal_id());
            if old.mod_id() == &fixture.mod_id {
                assert_eq!(now.adapter_facts().unwrap().strategy_version(), 4);
                assert!(!now
                    .adapter_facts()
                    .unwrap()
                    .transformer_identities()
                    .is_empty());
            }
        }
        for old in old_manifest
            .entries
            .iter()
            .filter(|entry| entry.mod_id != fixture.mod_id)
        {
            assert!(current.entries.contains(old));
        }
        let fixture = fixture.restart();
        let preview = fixture
            .state
            .reinstall_executor
            .preview_equipment_retarget_reinstall(fixture.reapply())
            .unwrap();
        assert_eq!(preview.status, ReinstallPreviewStatus::NoChanges);
        assert!(preview.plan_token.is_none());
        assert_eq!(read_fixture_manifest(&fixture.app_data), current);
        fixture.uninstall();
    }
}

#[test]
fn shared_material_references_stage_even_when_their_equipment_keeps_its_original_target() {
    let before = single_texture_material("wp/two/two003/mod/shared");
    let after = single_texture_material("wp/two/two019/mod/shared");
    let files: &[(&str, &[u8])] = &[
        ("nativePC/wp/two/two003/mod/two003.mrl3", &before),
        ("nativePC/wp/two/two003/mod/shared.tex", b"shared texture"),
        ("nativePC/wp/one/one004/mod/one004.mrl3", &before),
        ("nativePC/common/author.mrl3", &before),
    ];
    let fixture = Fixture::new(files);
    let choices = fixture.choices(&[("two003", "two019"), ("one004", "one004")]);
    let preflight = fixture
        .state
        .initial_retarget_install_preflight
        .preview(initial_request(choices.clone()))
        .unwrap();
    assert_eq!(preflight.planned.staged_binding_ids().len(), 2);
    install_equipment(&fixture.state, choices);
    let mut expected = fixture.baseline.clone();
    expected.insert(
        "nativePC/wp/two/two019/mod/two019.mrl3".to_owned(),
        after.to_vec(),
    );
    expected.insert(
        "nativePC/wp/two/two019/mod/shared.tex".to_owned(),
        b"shared texture".to_vec(),
    );
    expected.insert(files[2].0.to_owned(), after.to_vec());
    expected.insert(files[3].0.to_owned(), after.to_vec());
    expected.insert(COMPANION.to_owned(), COMPANION_BYTES.to_vec());
    assert_eq!(snapshot_file_tree(&fixture.game), expected);
    let unchanged_binding = read_fixture_manifest(&fixture.app_data)
        .replacement_bindings
        .into_iter()
        .find(|binding| binding.source_internal_id() == "one004")
        .unwrap();
    assert!(!hmm_app::is_identity_replacement_binding(
        &unchanged_binding
    ));
    fixture.run(fixture.choices(&[("two003", "two003"), ("one004", "one004")]));
    let mut expected = fixture.baseline.clone();
    for (path, bytes) in files {
        expected.insert((*path).to_owned(), bytes.to_vec());
    }
    expected.insert(COMPANION.to_owned(), COMPANION_BYTES.to_vec());
    assert_eq!(snapshot_file_tree(&fixture.game), expected);
    fixture.uninstall();
}
