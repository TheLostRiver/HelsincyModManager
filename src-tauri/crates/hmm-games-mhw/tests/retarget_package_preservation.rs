//! MHW 路径重定向必须完整保留资源，并维持原有的材质/贴图引用。
use hmm_core::{
    GameId, ModId, PackageFileId, ProfileId, ReplacementBinding, ReplacementBindingId, RetargetPlan,
};
use hmm_games_mhw::{MhwReplacementAdapter, MhwReplacementCatalog};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAdapterResult,
    ReplacementAnalysisRequest, ReplacementAsset, ReplacementAssetContentReader,
    ReplacementCatalogProvider, RetargetPlanRequest,
};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};

struct UnavailableBinary(AtomicUsize);

impl ReplacementAssetContentReader for UnavailableBinary {
    fn read_asset_content(&self, _: &PackageFileId, _: u64) -> ReplacementAdapterResult<Vec<u8>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(ReplacementAdapterError::SourceContentUnavailable)
    }
}

fn assets(paths: &[&str]) -> Vec<ReplacementAsset> {
    paths
        .iter()
        .map(|path| ReplacementAsset::new(PackageFileId::new(*path), *path))
        .collect()
}

fn plan(paths: &[&str], source_id: &str, target_id: &str, carrier: bool) -> RetargetPlan {
    let assets = assets(paths);
    let analysis = MhwReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets.clone(),
        })
        .expect("safe equipment resources must be recognized");
    let source = analysis
        .sources()
        .iter()
        .find(|source| source.internal_id() == source_id)
        .expect("requested equipment source must be reported");
    let catalog = MhwReplacementCatalog.replacement_catalog().unwrap();
    let target = catalog
        .targets()
        .iter()
        .find(|target| {
            target.internal_id() == target_id
                && target.target_type() == source.source_type()
                && target
                    .metadata()
                    .get("path_family")
                    .and_then(serde_json::Value::as_str)
                    == Some(source.path_family())
        })
        .expect("catalog target");
    let binding = ReplacementBinding::new(
        ReplacementBindingId::parse(format!("binding-{source_id}")).unwrap(),
        ModId::new("synthetic"),
        ProfileId::new("default"),
        source.id().clone(),
        target.id().clone(),
        1,
    )
    .unwrap();
    let reader = UnavailableBinary(AtomicUsize::new(0));
    let plan = MhwReplacementAdapter
        .build_retarget_plan_with_content(
            RetargetPlanRequest {
                game_id: GameId::mhw(),
                binding,
                assets,
                carries_package_companions: carrier,
            },
            &reader,
        )
        .expect("path retargeting must not depend on model binary parsing");
    assert_eq!(
        reader.0.load(Ordering::SeqCst),
        0,
        "unchanged contents need no binary parser"
    );
    assert!(plan
        .actions()
        .iter()
        .all(|action| action.content_transform().is_none()));
    plan
}

fn output<'a>(plan: &'a RetargetPlan, input: &str) -> &'a str {
    plan.actions()
        .iter()
        .find(|action| action.package_file_id().as_str() == input)
        .unwrap_or_else(|| panic!("missing input resource: {input}"))
        .target_relative_path()
        .as_str()
}

#[test]
fn weapon_retarget_keeps_texture_locations_and_material_contents() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mod3",
        "nativePC/wp/two/two028/mod/two028.mrl3",
        "nativePC/wp/two/two028/mod/two028_BML.tex",
    ];
    let plan = plan(&paths, "two028", "two029", true);
    assert_eq!(plan.actions().len(), paths.len());
    assert_eq!(
        output(&plan, paths[0]),
        "nativePC/wp/two/two029/mod/two029.mod3"
    );
    assert_eq!(
        output(&plan, paths[1]),
        "nativePC/wp/two/two029/mod/two029.mrl3"
    );
    assert_eq!(output(&plan, paths[2]), paths[2]);
}

#[test]
fn an_unknown_catalog_source_can_stay_in_place_without_becoming_an_arbitrary_target() {
    for path in [
        "nativePC/wp/one/one999/mod/custom.mod3",
        "nativePC/pl/f_equip/pl999_0000/body/custom.mod3",
    ] {
        let assets = assets(&[path]);
        let analysis = MhwReplacementAdapter
            .analyze_replacement_assets(ReplacementAnalysisRequest {
                game_id: GameId::mhw(),
                assets: assets.clone(),
            })
            .unwrap();
        let source = &analysis.sources()[0];
        let original = MhwReplacementCatalog
            .original_target_for_source(source)
            .unwrap();
        assert_eq!(original.internal_id(), source.internal_id());
        assert!(
            MhwReplacementCatalog
                .find_replacement_target(original.id())
                .is_err(),
            "unknown source identities are not selectable catalog targets"
        );
        let binding = ReplacementBinding::new(
            ReplacementBindingId::parse("binding-original").unwrap(),
            ModId::new("mod"),
            ProfileId::new("default"),
            source.id().clone(),
            original.id().clone(),
            0,
        )
        .unwrap();
        let request = RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding,
            assets,
            carries_package_companions: true,
        };
        let plan = MhwReplacementAdapter.build_retarget_plan(request).unwrap();
        assert_eq!(plan.actions().len(), 1);
        assert_eq!(output(&plan, path), path);
        assert!(plan.actions()[0].content_transform().is_none());
    }
}

#[test]
fn an_unmapped_weapon_model_does_not_report_success_when_nothing_can_move() {
    let assets = assets(&["nativePC/wp/two/two028/mod/custom.mod3"]);
    let analysis = MhwReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets.clone(),
        })
        .unwrap();
    let source = &analysis.sources()[0];
    let target = MhwReplacementCatalog
        .replacement_catalog()
        .unwrap()
        .targets()
        .iter()
        .find(|target| target.internal_id() == "two029")
        .unwrap()
        .id()
        .clone();
    let binding = ReplacementBinding::new(
        ReplacementBindingId::parse("binding-unmapped").unwrap(),
        ModId::new("mod"),
        ProfileId::new("default"),
        source.id().clone(),
        target,
        1,
    )
    .unwrap();
    assert_eq!(
        MhwReplacementAdapter
            .build_retarget_plan(RetargetPlanRequest {
                game_id: GameId::mhw(),
                binding,
                assets,
                carries_package_companions: true
            })
            .unwrap_err(),
        ReplacementAdapterError::AnalysisRejected {
            code: "weapon_no_relocatable_resources"
        }
    );
}

#[test]
fn individual_model_or_material_files_do_not_require_a_complete_pair() {
    for extension in ["mod3", "mrl3"] {
        let source = format!("nativePC/wp/two/two028/mod/two028.{extension}");
        let plan = plan(&[&source], "two028", "two029", true);
        assert_eq!(plan.actions().len(), 1);
        assert_eq!(
            output(&plan, &source),
            format!("nativePC/wp/two/two029/mod/two029.{extension}")
        );
    }
}

#[test]
fn unknown_models_unpaired_parts_and_ambiguous_companions_are_not_lost() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mod3",
        "nativePC/wp/two/two028/mod/two028.mrl3",
        "nativePC/wp/two/two028/mod/author_extra.mod3",
        "nativePC/wp/two/two028/mod/author_extra.mrl3",
        "nativePC/wp/two/two028/mod/ya028.mod3",
        "nativePC/wp/two/two028/mod/two028_two028.notes",
        "nativePC/wp/two/two028/mod/two028_BML.tex",
        "nativePC/common/effect/author_effect.bin",
    ];
    let plan = plan(&paths, "two028", "two029", true);
    assert_eq!(plan.actions().len(), paths.len());
    assert_eq!(output(&plan, paths[2]), paths[2]);
    assert_eq!(output(&plan, paths[3]), paths[3]);
    assert_eq!(
        output(&plan, paths[4]),
        "nativePC/wp/two/two029/mod/ya029.mod3"
    );
    for input in [paths[5], paths[6], paths[7]] {
        assert_eq!(output(&plan, input), input);
    }
    assert!(
        !plan.warnings().is_empty(),
        "unchanged ambiguous resources must be explained"
    );
}

#[test]
fn a_weapon_tree_texture_does_not_hide_an_armor_source() {
    let paths = [
        "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3",
        "nativePC/wp/author_textures/shared.tex",
        "nativePC/common/effect/author_effect.bin",
    ];
    let plan = plan(&paths, "pl078_0000", "pl129_0000", true);
    assert_eq!(plan.actions().len(), paths.len());
    assert_eq!(
        output(&plan, paths[0]),
        "nativePC/pl/f_equip/pl129_0000/body/mod/f_body129_0000.mod3"
    );
    assert_eq!(output(&plan, paths[1]), paths[1]);
    assert_eq!(output(&plan, paths[2]), paths[2]);
}

#[test]
fn mixed_equipment_sources_and_package_companions_are_accounted_once() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mod3",
        "nativePC/wp/two/two028/mod/two028.mrl3",
        "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3",
        "nativePC/pl/f_equip/pl078_0000/body/mod/skin.tex",
        "nativePC/common/effect/shared.bin",
    ];
    let analysis = MhwReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets(&paths),
        })
        .unwrap();
    assert_eq!(analysis.sources().len(), 2);
    let weapon = plan(&paths, "two028", "two029", true);
    let armor = plan(&paths, "pl078_0000", "pl129_0000", false);
    let ids = weapon
        .actions()
        .iter()
        .chain(armor.actions())
        .map(|action| action.package_file_id().as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), paths.len());
    assert_eq!(
        ids.iter().copied().collect::<BTreeSet<_>>(),
        paths.into_iter().collect()
    );
    assert_eq!(output(&armor, paths[3]), paths[3]);
}

#[test]
fn textures_referenced_by_other_slots_remain_at_their_original_locations() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mod3",
        "nativePC/wp/two/two028/mod/two028.mrl3",
        "nativePC/wp/two/two020/mod/two020.mod3",
        "nativePC/wp/two/two020/mod/two020.mrl3",
        "nativePC/wp/two/two020/mod/shared_skin.tex",
    ];
    let first = plan(&paths, "two028", "two029", true);
    let second = plan(&paths, "two020", "two003", false);
    assert_eq!(output(&second, paths[4]), paths[4]);
    assert!(first
        .actions()
        .iter()
        .chain(second.actions())
        .all(|action| action.content_transform().is_none()));
}

#[test]
fn unsafe_paths_and_case_collisions_are_still_rejected() {
    for extra in [
        "../outside.bin",
        "D:/outside.bin",
        "nativePC/wp/two/two028/mod/../../../../outside.bin",
        "nativePC/wp/two/two028/mod/TWO028.mod3",
    ] {
        let paths = [
            "nativePC/wp/two/two028/mod/two028.mod3",
            "nativePC/wp/two/two028/mod/two028.mrl3",
            extra,
        ];
        assert!(
            MhwReplacementAdapter
                .analyze_replacement_assets(ReplacementAnalysisRequest {
                    game_id: GameId::mhw(),
                    assets: assets(&paths),
                })
                .is_err(),
            "must reject {extra}"
        );
    }
}

#[test]
fn executable_exclusion_does_not_discard_other_resources() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mod3",
        "nativePC/wp/two/two028/mod/two028.mrl3",
        "nativePC/wp/two/two028/mod/helper.exe",
        "nativePC/common/safe.custom",
    ];
    let plan = plan(&paths, "two028", "two029", true);
    assert_eq!(plan.actions().len(), 3);
    assert_eq!(output(&plan, paths[3]), paths[3]);
    assert_eq!(plan.adapter_facts().unwrap().excluded_file_count(), 1);
}
