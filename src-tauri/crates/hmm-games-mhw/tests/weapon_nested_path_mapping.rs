//! 人工路径覆盖目录、文件名及整份映射，不读取真实游戏或 Mod 内容。
use hmm_core::{
    GameId, ModId, PackageFileId, ProfileId, ReplacementAdapterFacts, ReplacementBinding,
    ReplacementBindingId, RetargetFileDisposition, RetargetPlan,
};
use hmm_games_mhw::{MhwReplacementAdapter, MhwReplacementCatalog};
use hmm_ports::{
    ReplacementAdapter, ReplacementAnalysisRequest, ReplacementAsset, ReplacementCatalogProvider,
    RetargetPlanRequest,
};

fn plan(paths: &[&str], source: &str, target: &str) -> RetargetPlan {
    let assets = paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            ReplacementAsset::new(PackageFileId::new(format!("file-{index}")), *path)
        })
        .collect::<Vec<_>>();
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
    let catalog = MhwReplacementCatalog.replacement_catalog().unwrap();
    let target = catalog
        .targets()
        .iter()
        .find(|item| item.internal_id() == target && item.target_type() == source.source_type())
        .unwrap();
    let binding = ReplacementBinding::new(
        ReplacementBindingId::parse("binding-nested").unwrap(),
        ModId::new("synthetic"),
        ProfileId::new("default"),
        source.id().clone(),
        target.id().clone(),
        1,
    )
    .unwrap();
    let plan = MhwReplacementAdapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding,
            assets,
            carries_package_companions: true,
        })
        .unwrap();
    if analysis.sources().len() == 1 {
        assert_eq!(plan.actions().len(), paths.len());
        assert_eq!(plan.file_effects().len(), paths.len());
    }
    assert!(plan
        .actions()
        .iter()
        .all(|action| action.content_transform().is_none()));
    plan
}

fn output(plan: &RetargetPlan, index: usize) -> &str {
    plan.actions()
        .iter()
        .find(|action| action.package_file_id().as_str() == format!("file-{index}"))
        .unwrap()
        .target_relative_path()
        .as_str()
}

#[test]
fn nested_source_directories_and_filenames_are_mapped_together() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028/custom.mod3",
        "nativePC/wp/two/two028/mod/two028/two028.mod3",
        "nativePC/wp/two/two028/extra/two028/deeper/two028/two028.custom",
    ];
    let planned = plan(&paths, "two028", "two029");
    for (index, expected) in [
        "nativePC/wp/two/two029/mod/two029/custom.mod3",
        "nativePC/wp/two/two029/mod/two029/two029.mod3",
        "nativePC/wp/two/two029/extra/two029/deeper/two029/two029.custom",
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(output(&planned, index), expected);
    }
    assert!(planned
        .file_effects()
        .iter()
        .all(|effect| effect.disposition == RetargetFileDisposition::Relocated));
}

#[test]
fn explicit_numbered_directories_carry_author_filenames_but_leave_textures_in_place() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028/作者模型.mod3",
        "nativePC/wp/two/two028/mod/two028/131072_2599467785140006031 BML.dds",
        "nativePC/wp/two/two028/mod/two028/custom.mrl3",
        "nativePC/wp/two/two028/mod/two028/custom.TeX",
        "nativePC/wp/two/two028/mod/two028/two028_BML.tex",
        "nativePC/wp/two/two028/mod/custom.mod3",
    ];
    let planned = plan(&paths, "two028", "two029");
    for (index, path) in paths.iter().enumerate().take(3) {
        assert_eq!(output(&planned, index), path.replace("two028", "two029"));
    }
    for (index, path) in paths.iter().enumerate().skip(3) {
        assert_eq!(output(&planned, index), *path);
    }
}

#[test]
fn directory_mapping_preserves_case_and_normalizes_bs_identity_in_both_directions() {
    for (source, target, path, expected) in [
        (
            "bs_two012",
            "two020",
            "NATIVEPC/WP/TWO/BS_TWO012/MOD/BS_TWO012/BS_TWO012_Extra.MOD3",
            "nativePC/wp/two/two020/MOD/TWO020/TWO020_Extra.MOD3",
        ),
        (
            "two020",
            "bs_two012",
            "nativePC/wp/two/two020/mod/TWO020/TWO020_Extra.mod3",
            "nativePC/wp/two/bs_two012/mod/bs_TWO012/bs_TWO012_Extra.mod3",
        ),
    ] {
        let planned = plan(&[path], source, target);
        assert_eq!(output(&planned, 0), expected);
    }
}

#[test]
fn only_complete_source_ids_and_independently_proven_part_directories_are_renamed() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mod3",
        "nativePC/wp/two/two028/mod/ya028.mod3",
        "nativePC/wp/two/two028/mod/ya028/custom.mrl3",
        "nativePC/wp/two/two028/mod/ya028/ya028_Extra.ctc",
        "nativePC/wp/two/two028/mod/Author028/ya028.mod3",
        "nativePC/wp/two/two028/mod/20260913/two0280/two028.mod3",
        "nativePC/wp/two/two028/mod/author_two028/two028.mod3",
        "nativePC/wp/two/two028/作者A/2026-09-13/two028/自定义.foo123",
    ];
    let planned = plan(&paths, "two028", "two029");
    for (index, expected) in [
        "nativePC/wp/two/two029/mod/two029.mod3",
        "nativePC/wp/two/two029/mod/ya029.mod3",
        "nativePC/wp/two/two029/mod/ya029/custom.mrl3",
        "nativePC/wp/two/two029/mod/ya029/ya029_Extra.ctc",
        "nativePC/wp/two/two029/mod/Author028/ya029.mod3",
        "nativePC/wp/two/two029/mod/20260913/two0280/two029.mod3",
        "nativePC/wp/two/two029/mod/author_two028/two029.mod3",
        "nativePC/wp/two/two029/作者A/2026-09-13/two029/自定义.foo123",
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(output(&planned, index), expected);
    }
}

#[test]
fn suffixed_parts_provide_exact_directory_evidence_without_a_prefix_whitelist() {
    let paths = [
        "nativePC/wp/swo/swo035/mod/saya035ol.mod3",
        "nativePC/wp/swo/swo035/mod/saya035ol/custom.mrl3",
        "nativePC/wp/swo/swo035/mod/saya035/custom.mrl3",
    ];
    let planned = plan(&paths, "swo035", "swo019");
    assert_eq!(
        output(&planned, 0),
        "nativePC/wp/swo/swo019/mod/saya019ol.mod3"
    );
    assert_eq!(
        output(&planned, 1),
        "nativePC/wp/swo/swo019/mod/saya019ol/custom.mrl3"
    );
    assert_eq!(
        output(&planned, 2),
        paths[2],
        "a similar part name is not exact evidence"
    );
}

#[test]
fn ambiguous_or_contradictory_ids_keep_the_whole_original_path() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mod3",
        "nativePC/wp/two/two028/mod/two028/two028_two028.mod3",
        "nativePC/wp/two/two028/mod/two028/two028_two029.mod3",
        "nativePC/wp/two/two028/mod/two028/two029.mod3",
        "nativePC/wp/two/two028/mod/two030/two028.mod3",
        "nativePC/wp/two/two028/mod/two028/bs_two028.mod3",
        "nativePC/wp/two/two028/mod/two028/swo028.mod3",
        "nativePC/wp/two/two028/mod/two028/ya027.mod3",
    ];
    let planned = plan(&paths, "two028", "two029");
    for (index, path) in paths.iter().enumerate().skip(1) {
        assert_eq!(
            output(&planned, index),
            *path,
            "contradictory path must not be partly renamed"
        );
        let effect = planned
            .file_effects()
            .iter()
            .find(|effect| effect.package_file_id.as_str() == format!("file-{index}"))
            .unwrap();
        assert_eq!(effect.disposition, RetargetFileDisposition::KeptInPlace);
        assert_eq!(
            effect.reason,
            if index == 1 {
                hmm_core::RetargetFileReason::AmbiguousResourceIdentity
            } else {
                hmm_core::RetargetFileReason::ConflictingResourceIdentity
            }
        );
    }
}

#[test]
fn part_directory_evidence_ignores_textures() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mod3",
        "nativePC/wp/two/two028/mod/ya028.tex",
        "nativePC/wp/two/two028/mod/ya028/custom.mod3",
        "nativePC/wp/two/two028/mod/unknown028/custom.mod3",
    ];
    let planned = plan(&paths, "two028", "two029");
    for (index, path) in paths.iter().enumerate().skip(1) {
        assert_eq!(output(&planned, index), *path);
    }
}

#[test]
fn new_strategy_is_versioned_and_legacy_v1_facts_remain_readable() {
    let planned = plan(
        &["nativePC/wp/two/two028/mod/two028.mod3"],
        "two028",
        "two029",
    );
    let facts = planned.adapter_facts().unwrap();
    assert_eq!(facts.strategy_version(), 2);
    let mut legacy = serde_json::to_value(facts).unwrap();
    legacy["strategy_version"] = serde_json::json!(1);
    let loaded: ReplacementAdapterFacts = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(loaded.strategy_version(), 1);
    assert_eq!(serde_json::to_value(loaded).unwrap(), legacy);
}

#[test]
fn another_source_or_a_contradictory_model_cannot_prove_a_part_directory() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mod3",
        "nativePC/wp/two/two028/mod/ya028/custom.mod3",
        "nativePC/wp/bow/bow028/mod/ya028.mod3",
        "nativePC/wp/two/two028/mod/two030/ya028.mod3",
    ];
    let planned = plan(&paths, "two028", "two029");
    assert_eq!(planned.actions().len(), 3);
    assert_eq!(output(&planned, 1), paths[1]);
    assert_eq!(output(&planned, 3), paths[3]);
    assert!(planned
        .actions()
        .iter()
        .all(|action| action.package_file_id().as_str() != "file-2"));
}

#[test]
fn windows_texture_extension_aliases_keep_the_complete_original_path() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028/two028.mod3",
        "nativePC/wp/two/two028/mod/two028/custom.TeX.",
        "nativePC/wp/two/two028/mod/two028/two028_BML.tex...",
    ];
    let planned = plan(&paths, "two028", "two029");
    for (index, path) in paths.iter().enumerate().skip(1) {
        assert_eq!(output(&planned, index), *path);
        let effect = planned
            .file_effects()
            .iter()
            .find(|effect| effect.package_file_id.as_str() == format!("file-{index}"))
            .unwrap();
        assert_eq!(
            effect.reason,
            hmm_core::RetargetFileReason::TextureReference
        );
    }
}

#[test]
fn a_model_in_a_contradictory_part_directory_cannot_authorize_another_directory() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mod3",
        "nativePC/wp/two/two028/mod/ya028.mod3",
        "nativePC/wp/two/two028/mod/ya027/aux028.mod3",
        "nativePC/wp/two/two028/mod/aux028/custom.mod3",
    ];
    let planned = plan(&paths, "two028", "two029");
    assert_eq!(output(&planned, 2), paths[2]);
    assert_eq!(
        output(&planned, 3),
        paths[3],
        "a contradictory model is not relocation evidence"
    );
    let mut reversed = paths;
    reversed.reverse();
    assert_eq!(output(&plan(&reversed, "two028", "two029"), 0), paths[3]);
}
