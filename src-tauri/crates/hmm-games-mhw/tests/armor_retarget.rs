use hmm_core::{
    GameId, ModId, PackageFileId, ProfileId, ReplacementBinding, ReplacementBindingId,
    ReplacementSourceId, ReplacementTargetId, ReplacementWarning,
};
use hmm_games_mhw::{ArmorPathError, ArmorResourcePath, MhwArmorReplacementAdapter};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAnalysisRequest, ReplacementAsset,
    RetargetPlanRequest,
};

fn asset(id: &str, relative_path: &str) -> ReplacementAsset {
    ReplacementAsset::new(PackageFileId::new(id), relative_path)
}

fn binding(source_id: &str, target_id: &str) -> ReplacementBinding {
    ReplacementBinding::new(
        ReplacementBindingId::parse("binding-1").expect("binding id"),
        ModId::new("mod-1"),
        ProfileId::new("profile-1"),
        ReplacementSourceId::parse(source_id).expect("source id"),
        ReplacementTargetId::parse(target_id).expect("target id"),
        42,
    )
    .expect("binding")
}

#[test]
fn armor_path_parser_normalizes_separators_and_retargets_only_the_slot_segment() {
    let forward =
        ArmorResourcePath::parse("nativePC/pl/f_equip/pl121_0000/arm/mod/f_121_0000_extra.mod3")
            .expect("forward path");
    let backward =
        ArmorResourcePath::parse(r"nativePC\pl\f_equip\pl121_0000\arm\mod\f_121_0000_extra.mod3")
            .expect("backslash path");

    assert_eq!(forward, backward);
    assert_eq!(forward.slot(), "pl121_0000");
    assert_eq!(forward.path_family(), "pl/f_equip");
    assert!(forward.is_supported());
    assert_eq!(
        forward.retarget("pl129_0000").expect("target").as_str(),
        "nativePC/pl/f_equip/pl129_0000/arm/mod/f_121_0000_extra.mod3"
    );
}

#[test]
fn armor_path_parser_recognizes_male_family_and_rejects_unsafe_or_malformed_paths() {
    let male = ArmorResourcePath::parse("nativePC/pl/m_equip/pl121_0000/arm/mod/m_body.mod3")
        .expect("male path is recognized for analysis");
    assert_eq!(male.path_family(), "pl/m_equip");
    assert!(!male.is_supported());

    assert_eq!(
        ArmorResourcePath::parse("../nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3"),
        Err(ArmorPathError::UnsafePath)
    );
    assert_eq!(
        ArmorResourcePath::parse("nativePC/pl/f_equip/not-a-slot/arm/mod/f_body.mod3"),
        Err(ArmorPathError::InvalidSlot)
    );
    assert_eq!(
        ArmorResourcePath::parse("nativePC/pl/f_equip/pl121_0000/body/f_body.mod3"),
        Err(ArmorPathError::MalformedArmorPath)
    );
}

#[test]
fn armor_analysis_accepts_one_source_and_ignores_unrelated_safe_assets() {
    let adapter = MhwArmorReplacementAdapter;
    let analysis = adapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: vec![
                asset(
                    "body.mod3",
                    "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
                ),
                asset(
                    "arms.mod3",
                    r"nativePC\pl\f_equip\pl121_0000\arm\mod\f_arms.mod3",
                ),
                asset("readme.txt", "readme.txt"),
            ],
        })
        .expect("analysis");

    assert!(analysis.is_retargetable());
    assert_eq!(analysis.matched_asset_count(), 2);
    assert_eq!(analysis.sources().len(), 1);
    assert_eq!(
        analysis.sources()[0].id().as_str(),
        "mhw:armor:f_equip:pl121_0000"
    );
    assert!(analysis.warnings().is_empty());
}

#[test]
fn armor_analysis_reports_no_source_without_failing_normal_mod_imports() {
    let analysis = MhwArmorReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: vec![asset("readme.txt", "readme.txt")],
        })
        .expect("non-armor analysis");

    assert!(!analysis.is_retargetable());
    assert!(analysis.sources().is_empty());
    assert_eq!(
        analysis.warnings(),
        &[ReplacementWarning::NoSupportedAssets]
    );
}

#[test]
fn armor_analysis_blocks_multiple_slots_and_male_or_mixed_sources() {
    let adapter = MhwArmorReplacementAdapter;
    let multiple = adapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: vec![
                asset(
                    "first.mod3",
                    "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
                ),
                asset(
                    "second.mod3",
                    "nativePC/pl/f_equip/pl122_0000/arm/mod/f_body.mod3",
                ),
            ],
        })
        .expect("multiple analysis");
    assert!(!multiple.is_retargetable());
    assert!(multiple
        .warnings()
        .contains(&ReplacementWarning::MultipleSources));

    let mixed = adapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: vec![
                asset(
                    "female.mod3",
                    "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
                ),
                asset(
                    "male.mod3",
                    "nativePC/pl/m_equip/pl121_0000/arm/mod/m_body.mod3",
                ),
            ],
        })
        .expect("mixed analysis");
    assert!(!mixed.is_retargetable());
    assert!(mixed
        .warnings()
        .contains(&ReplacementWarning::MultipleSources));
    assert!(mixed
        .warnings()
        .contains(&ReplacementWarning::UnsupportedSource));
}

#[test]
fn armor_analysis_rejects_unsafe_and_malformed_candidate_paths() {
    let adapter = MhwArmorReplacementAdapter;
    let unsafe_error = adapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: vec![asset("escape.mod3", "../escape.mod3")],
        })
        .expect_err("unsafe asset path");
    assert_eq!(unsafe_error, ReplacementAdapterError::UnsafeRetargetPath);

    let malformed_error = adapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: vec![asset(
                "invalid.mod3",
                "nativePC/pl/f_equip/not-a-slot/arm/mod/f_body.mod3",
            )],
        })
        .expect_err("invalid source slot");
    assert_eq!(
        malformed_error,
        ReplacementAdapterError::UnrecognizedSourceSlot
    );
}

#[test]
fn armor_retarget_plan_preserves_package_identity_and_non_slot_segments() {
    let source_id = "mhw:armor:f_equip:pl121_0000";
    let plan = MhwArmorReplacementAdapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding(source_id, "mhw:armor:fatalis-alpha"),
            assets: vec![
                asset(
                    "body.mod3",
                    r"nativePC\pl\f_equip\pl121_0000\arm\mod\f_121_0000_extra.mod3",
                ),
                asset("readme.txt", "readme.txt"),
            ],
        })
        .expect("retarget plan");

    assert_eq!(plan.actions().len(), 1);
    let action = &plan.actions()[0];
    assert_eq!(action.package_file_id().as_str(), "body.mod3");
    assert_eq!(
        action.source_relative_path().as_str(),
        "nativePC/pl/f_equip/pl121_0000/arm/mod/f_121_0000_extra.mod3"
    );
    assert_eq!(
        action.target_relative_path().as_str(),
        "nativePC/pl/f_equip/pl129_0000/arm/mod/f_121_0000_extra.mod3"
    );
    assert_eq!(action.source_id().as_str(), source_id);
    assert_eq!(action.source_internal_id(), "pl121_0000");
    assert_eq!(action.target_internal_id(), "pl129_0000");
}

#[test]
fn armor_retarget_plan_rejects_ambiguous_source_unknown_target_and_binding_mismatch() {
    let adapter = MhwArmorReplacementAdapter;
    let ambiguous = adapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding("mhw:armor:f_equip:pl121_0000", "mhw:armor:fatalis-alpha"),
            assets: vec![
                asset(
                    "female.mod3",
                    "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
                ),
                asset(
                    "male.mod3",
                    "nativePC/pl/m_equip/pl121_0000/arm/mod/m_body.mod3",
                ),
            ],
        })
        .expect_err("ambiguous source");
    assert_eq!(ambiguous, ReplacementAdapterError::AmbiguousSourceSlot);

    let missing_id = ReplacementTargetId::parse("mhw:armor:missing").expect("target id");
    let missing = adapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding("mhw:armor:f_equip:pl121_0000", missing_id.as_str()),
            assets: vec![asset(
                "body.mod3",
                "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
            )],
        })
        .expect_err("unknown target");
    assert_eq!(
        missing,
        ReplacementAdapterError::TargetCatalogMissing {
            target_id: missing_id
        }
    );

    let mismatch = adapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding("mhw:armor:f_equip:pl999_0000", "mhw:armor:fatalis-alpha"),
            assets: vec![asset(
                "body.mod3",
                "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
            )],
        })
        .expect_err("binding mismatch");
    assert_eq!(mismatch, ReplacementAdapterError::SourceBindingMismatch);
}

#[test]
fn armor_retarget_plan_warns_when_source_already_matches_target() {
    let source_id = "mhw:armor:f_equip:pl129_0000";
    let plan = MhwArmorReplacementAdapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding(source_id, "mhw:armor:fatalis-alpha"),
            assets: vec![asset(
                "body.mod3",
                "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
            )],
        })
        .expect("same-target plan");

    assert_eq!(plan.warnings(), &[ReplacementWarning::SourceMatchesTarget]);
}

#[test]
fn armor_retarget_plan_rejects_duplicate_normalized_target_paths() {
    let error = MhwArmorReplacementAdapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding("mhw:armor:f_equip:pl121_0000", "mhw:armor:fatalis-alpha"),
            assets: vec![
                asset(
                    "first.mod3",
                    "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
                ),
                asset(
                    "second.mod3",
                    r"nativePC\pl\f_equip\pl121_0000\arm\mod\f_body.mod3",
                ),
            ],
        })
        .expect_err("duplicate final target");

    assert_eq!(error, ReplacementAdapterError::InvalidRetargetPlan);
}
