use hmm_core::{
    GameId, ModId, PackageFileId, ProfileId, ReplacementBinding, ReplacementBindingId, RetargetPlan,
};
use hmm_games_mhw::{MhwReplacementAdapter, MhwReplacementCatalog};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAnalysisRequest, ReplacementAsset,
    ReplacementCatalogProvider, RetargetPlanRequest,
};
use std::collections::BTreeSet;

fn assets(paths: &[&str]) -> Vec<ReplacementAsset> {
    paths
        .iter()
        .map(|path| ReplacementAsset::new(PackageFileId::new(*path), *path))
        .collect()
}

fn request(paths: &[&str], source: &str, target: &str, carrier: bool) -> RetargetPlanRequest {
    let assets = assets(paths);
    let analysis = MhwReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets.clone(),
        })
        .unwrap();
    let source = analysis
        .sources()
        .iter()
        .find(|item| item.internal_id() == source)
        .unwrap();
    let target = MhwReplacementCatalog
        .replacement_catalog()
        .unwrap()
        .targets()
        .iter()
        .find(|item| item.internal_id() == target)
        .unwrap()
        .id()
        .clone();
    RetargetPlanRequest {
        game_id: GameId::mhw(),
        assets,
        carries_package_companions: carrier,
        binding: ReplacementBinding::new(
            ReplacementBindingId::parse(format!("fixture-{}", source.internal_id())).unwrap(),
            ModId::new("fixture"),
            ProfileId::new("profile"),
            source.id().clone(),
            target,
            1,
        )
        .unwrap(),
    }
}

fn output<'a>(plan: &'a RetargetPlan, input: &str) -> &'a str {
    plan.actions()
        .iter()
        .find(|action| action.package_file_id().as_str() == input)
        .unwrap()
        .target_relative_path()
        .as_str()
}

#[test]
fn bundled_catalog_has_all_checked_names_and_original_target_uses_the_same_record() {
    let catalog = MhwReplacementCatalog.replacement_catalog().unwrap();
    let targets = catalog
        .targets()
        .iter()
        .filter(|target| target.target_type().as_str() == "kinsect")
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 29);
    assert_eq!(
        targets
            .iter()
            .map(|target| target.internal_id().to_owned())
            .collect::<BTreeSet<_>>(),
        (1..=29).map(|number| format!("mus{number:03}")).collect()
    );
    for locale in ["zh_cn", "en", "ja"] {
        assert_eq!(
            targets
                .iter()
                .map(|target| 1 + target.localized_aliases().unwrap()[locale].len())
                .sum::<usize>(),
            105
        );
    }
    let analysis = MhwReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets(&["nativePC/wp/mus/mus023/mod/mus023.mod3"]),
        })
        .unwrap();
    let original = MhwReplacementCatalog
        .original_target_for_source(&analysis.sources()[0])
        .unwrap();
    assert_eq!(original.display_name().get("en"), Some("Dragon Soul"));
    assert_eq!(
        original.localized_aliases().unwrap()["en"],
        ["True Dragon Soul", "Nexus Dragon Soul"]
    );
    assert_eq!(
        MhwReplacementCatalog
            .find_replacement_target(original.id())
            .unwrap(),
        original
    );
    assert!(MhwReplacementCatalog
        .search_replacement_targets("Nexus Dragon Soul")
        .unwrap()
        .contains(&original));
}

#[test]
fn kinsect_mapping_preserves_textures_custom_resources_and_conflicting_identities() {
    let paths = [
        "Wrapper/nativePC/WP/MUS/MUS001/mod/mus001.mod3",
        "nativePC/wp/mus/mus001/mod/mus001.mrl3",
        "nativePC/wp/mus/mus001/epv/mus001.epv3",
        "nativePC/wp/mus/mus001/mod/wing001.mod3",
        "nativePC/wp/mus/mus001/wing001/作者/mesh.bin",
        "nativePC/wp/mus/mus001/mus001/author.dat",
        "nativePC/wp/mus/mus001/mod/mus001_BML.tex",
        "nativePC/wp/mus/mus001/mod/custom.mod3",
        "nativePC/wp/mus/mus001/mod/rod001.mod3",
        "nativePC/wp/mus/mus001/mod/mus002.mod3",
        "nativePC/wp/mus/mus001/mod/mus001_mus001.ctc",
        "nativePC/wp/mus/mus001/mod/mus0011.ctc",
        "nativePC/wp/mus/author/common.tex",
    ];
    let plan = MhwReplacementAdapter
        .build_retarget_plan(request(&paths, "mus001", "mus029", true))
        .unwrap();
    assert_eq!(plan.actions().len(), paths.len());
    assert_eq!(
        output(&plan, paths[0]),
        "nativePC/wp/mus/mus029/mod/mus029.mod3"
    );
    assert_eq!(
        output(&plan, paths[1]),
        "nativePC/wp/mus/mus029/mod/mus029.mrl3"
    );
    assert_eq!(
        output(&plan, paths[2]),
        "nativePC/wp/mus/mus029/epv/mus029.epv3"
    );
    assert_eq!(
        output(&plan, paths[3]),
        "nativePC/wp/mus/mus029/mod/wing029.mod3"
    );
    assert_eq!(
        output(&plan, paths[4]),
        "nativePC/wp/mus/mus029/wing029/作者/mesh.bin"
    );
    assert_eq!(
        output(&plan, paths[5]),
        "nativePC/wp/mus/mus029/mus029/author.dat"
    );
    for path in &paths[6..] {
        assert_eq!(output(&plan, path), *path);
    }
    assert!(plan
        .actions()
        .iter()
        .all(|action| action.content_transform().is_none()));
    assert_eq!(plan.file_effects().len(), paths.len());
}

#[test]
fn mixed_package_assigns_each_kinsect_glaive_and_armor_source_once() {
    let paths = [
        "nativePC/wp/mus/mus001/mod/mus001.mod3",
        "nativePC/wp/mus/mus002/mod/mus002.mod3",
        "nativePC/wp/rod/rod001/mod/rod001.mod3",
        "nativePC/pl/f_equip/pl001_0000/body/body.mod3",
        "nativePC/wp/mus/common/author.tex",
    ];
    let analysis = MhwReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets(&paths),
        })
        .unwrap();
    assert_eq!(analysis.sources().len(), 4);
    assert_eq!(
        analysis
            .sources()
            .iter()
            .filter(|source| source.source_type().as_str() == "kinsect")
            .count(),
        2
    );
    let first = MhwReplacementAdapter
        .build_retarget_plan(request(&paths, "mus001", "mus002", true))
        .unwrap();
    let second = MhwReplacementAdapter
        .build_retarget_plan(request(&paths, "mus002", "mus001", false))
        .unwrap();
    assert_eq!(first.actions().len(), 2);
    assert_eq!(second.actions().len(), 1);
    assert_eq!(output(&first, paths[0]), paths[1]);
    assert_eq!(output(&second, paths[1]), paths[0]);
    assert_eq!(output(&first, paths[4]), paths[4]);
    assert!(matches!(
        MhwReplacementAdapter.build_retarget_plan(request(&paths, "mus001", "rod001", true)),
        Err(ReplacementAdapterError::UnsupportedReplacementTarget)
    ));
}

#[test]
fn textures_do_not_create_sources_and_unmappable_sources_can_only_stay_in_place() {
    let paths = ["nativePC/wp/mus/mus001/mod/mus001_BML.tex"];
    let analysis = MhwReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets(&paths),
        })
        .unwrap();
    assert!(analysis.sources().is_empty());
    let paths = ["nativePC/wp/mus/mus001/mod/custom.mod3"];
    assert!(matches!(
        MhwReplacementAdapter.build_retarget_plan(request(&paths, "mus001", "mus002", true)),
        Err(ReplacementAdapterError::SourceAnalysisRejected { .. })
    ));
    let kept = MhwReplacementAdapter
        .build_retarget_plan(request(&paths, "mus001", "mus001", true))
        .unwrap();
    assert_eq!(output(&kept, paths[0]), paths[0]);
}
