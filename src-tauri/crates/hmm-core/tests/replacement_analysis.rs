use hmm_core::{
    GameId, InstallTargetPath, ModId, PackageFileId, ProfileId, ReplacementAnalysis,
    ReplacementBinding, ReplacementBindingId, ReplacementSource, ReplacementSourceId,
    ReplacementTargetId, ReplacementTargetKind, ReplacementWarning, RetargetAction, RetargetError,
    RetargetPlan,
};

fn source(id: &str, internal_id: &str, path_family: &str, supported: bool) -> ReplacementSource {
    ReplacementSource::new(
        ReplacementSourceId::parse(id).expect("source id"),
        GameId::mhw(),
        ReplacementTargetKind::parse("armor").expect("source kind"),
        internal_id,
        path_family,
        supported,
    )
    .expect("replacement source")
}

fn binding(source_id: &str) -> ReplacementBinding {
    ReplacementBinding::new(
        ReplacementBindingId::parse("binding-1").expect("binding id"),
        ModId::new("mod-1"),
        ProfileId::new("profile-1"),
        ReplacementSourceId::parse(source_id).expect("source id"),
        ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target id"),
        42,
    )
    .expect("binding")
}

fn action(
    package_file_id: &str,
    source_id: &str,
    source_path: &str,
    target_path: &str,
) -> RetargetAction {
    RetargetAction::new(
        PackageFileId::new(package_file_id),
        InstallTargetPath::parse(source_path, ["nativePC"]).expect("source path"),
        InstallTargetPath::parse(target_path, ["nativePC"]).expect("target path"),
        ReplacementSourceId::parse(source_id).expect("source id"),
        "pl121_0000",
        "pl129_0000",
        "pl/f_equip",
        "pl/f_equip",
    )
    .expect("retarget action")
}

#[test]
fn replacement_analysis_exposes_a_supported_single_source() {
    let source = source(
        "mhw:armor:f_equip:pl121_0000",
        "pl121_0000",
        "pl/f_equip",
        true,
    );
    let analysis = ReplacementAnalysis::new(GameId::mhw(), vec![source.clone()], 2, Vec::new())
        .expect("analysis");

    assert!(analysis.is_retargetable());
    assert_eq!(analysis.single_source(), Some(&source));
    assert_eq!(analysis.matched_asset_count(), 2);
    assert_eq!(source.internal_id(), "pl121_0000");
    assert_eq!(source.path_family(), "pl/f_equip");
}

#[test]
fn replacement_analysis_rejects_duplicate_source_ids() {
    let source = source(
        "mhw:armor:f_equip:pl121_0000",
        "pl121_0000",
        "pl/f_equip",
        true,
    );

    let error = ReplacementAnalysis::new(
        GameId::mhw(),
        vec![source.clone(), source],
        2,
        vec![ReplacementWarning::MultipleSources],
    )
    .expect_err("duplicate source ids must be rejected");

    assert_eq!(
        error,
        RetargetError::DuplicateSourceId {
            source_id: "mhw:armor:f_equip:pl121_0000".to_owned(),
        }
    );
}

#[test]
fn replacement_warnings_have_stable_serialized_codes() {
    let value = serde_json::to_value([
        ReplacementWarning::NoSupportedAssets,
        ReplacementWarning::MultipleSources,
        ReplacementWarning::UnsupportedSource,
        ReplacementWarning::SourceMatchesTarget,
    ])
    .expect("warnings serialize");

    assert_eq!(
        value,
        serde_json::json!([
            "no_supported_assets",
            "multiple_sources",
            "unsupported_source",
            "source_matches_target"
        ])
    );
}

#[test]
fn retarget_plan_requires_binding_source_and_unique_target_paths() {
    let source_id = "mhw:armor:f_equip:pl121_0000";
    let source = source(source_id, "pl121_0000", "pl/f_equip", true);
    let first = action(
        "body.mod3",
        source_id,
        "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
        "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
    );
    let plan = RetargetPlan::new(
        binding(source_id),
        source.clone(),
        vec![first.clone()],
        Vec::new(),
    )
    .expect("retarget plan");

    assert_eq!(plan.source(), &source);
    assert_eq!(plan.actions(), &[first]);
    assert_eq!(
        plan.actions()[0].target_relative_path().as_str(),
        "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3"
    );

    let mismatch = RetargetPlan::new(
        binding("mhw:armor:f_equip:pl999_0000"),
        source.clone(),
        vec![action(
            "body.mod3",
            source_id,
            "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
            "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
        )],
        Vec::new(),
    )
    .expect_err("binding source mismatch");
    assert_eq!(mismatch, RetargetError::BindingSourceMismatch);

    let duplicate_target = RetargetPlan::new(
        binding(source_id),
        source,
        vec![
            action(
                "body-a.mod3",
                source_id,
                "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body_a.mod3",
                "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
            ),
            action(
                "body-b.mod3",
                source_id,
                "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body_b.mod3",
                "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
            ),
        ],
        Vec::new(),
    )
    .expect_err("duplicate target paths");
    assert_eq!(
        duplicate_target,
        RetargetError::DuplicateRetargetTargetPath {
            target_path: "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3".to_owned(),
        }
    );
}

#[test]
fn retarget_action_requires_a_non_empty_package_file_id() {
    let error = RetargetAction::new(
        PackageFileId::new(" "),
        InstallTargetPath::parse(
            "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
            ["nativePC"],
        )
        .expect("source path"),
        InstallTargetPath::parse(
            "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
            ["nativePC"],
        )
        .expect("target path"),
        ReplacementSourceId::parse("mhw:armor:f_equip:pl121_0000").expect("source id"),
        "pl121_0000",
        "pl129_0000",
        "pl/f_equip",
        "pl/f_equip",
    )
    .expect_err("empty package file id");

    assert_eq!(error, RetargetError::EmptyPackageFileId);
}

#[test]
fn retarget_plan_rejects_inconsistent_action_facts_and_duplicate_package_files() {
    let source_id = "mhw:armor:f_equip:pl121_0000";
    let source = source(source_id, "pl121_0000", "pl/f_equip", true);
    let wrong_source_fact = RetargetAction::new(
        PackageFileId::new("wrong-source.mod3"),
        InstallTargetPath::parse(
            "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
            ["nativePC"],
        )
        .expect("source path"),
        InstallTargetPath::parse(
            "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
            ["nativePC"],
        )
        .expect("target path"),
        ReplacementSourceId::parse(source_id).expect("source id"),
        "pl999_0000",
        "pl129_0000",
        "pl/f_equip",
        "pl/f_equip",
    )
    .expect("action");
    let error = RetargetPlan::new(
        binding(source_id),
        source.clone(),
        vec![wrong_source_fact],
        Vec::new(),
    )
    .expect_err("source facts must match");
    assert_eq!(error, RetargetError::ActionSourceMismatch);

    let inconsistent_target = RetargetAction::new(
        PackageFileId::new("arms.mod3"),
        InstallTargetPath::parse(
            "nativePC/pl/f_equip/pl121_0000/arm/mod/f_arms.mod3",
            ["nativePC"],
        )
        .expect("source path"),
        InstallTargetPath::parse(
            "nativePC/pl/f_equip/pl130_0000/arm/mod/f_arms.mod3",
            ["nativePC"],
        )
        .expect("target path"),
        ReplacementSourceId::parse(source_id).expect("source id"),
        "pl121_0000",
        "pl130_0000",
        "pl/f_equip",
        "pl/f_equip",
    )
    .expect("action");
    let error = RetargetPlan::new(
        binding(source_id),
        source.clone(),
        vec![
            action(
                "body.mod3",
                source_id,
                "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
                "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
            ),
            inconsistent_target,
        ],
        Vec::new(),
    )
    .expect_err("target facts must be consistent");
    assert_eq!(error, RetargetError::InconsistentRetargetTarget);

    let error = RetargetPlan::new(
        binding(source_id),
        source,
        vec![
            action(
                "duplicate.mod3",
                source_id,
                "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
                "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
            ),
            action(
                "duplicate.mod3",
                source_id,
                "nativePC/pl/f_equip/pl121_0000/arm/mod/f_arms.mod3",
                "nativePC/pl/f_equip/pl129_0000/arm/mod/f_arms.mod3",
            ),
        ],
        Vec::new(),
    )
    .expect_err("package file ids must be unique");
    assert_eq!(
        error,
        RetargetError::DuplicateRetargetPackageFile {
            package_file_id: "duplicate.mod3".to_owned(),
        }
    );
}
