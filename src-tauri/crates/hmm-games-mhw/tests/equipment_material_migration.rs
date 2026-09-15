#[path = "support/equipment_material.rs"]
mod fixture;
use fixture::{material, references, request, Materials};
use hmm_core::{PackageFileId, RetargetPlan};
use hmm_games_mhw::{MhwEquipmentMrl3TexturePathTransformer, MhwReplacementAdapter};
use hmm_ports::{
    ContentTransformRequest, ContentTransformerRegistry, ReplacementAdapter,
    ReplacementAdapterError,
};
use std::{collections::BTreeMap, sync::Arc};

fn transformed(plan: &RetargetPlan, path: &str, bytes: &[u8]) -> Vec<u8> {
    let action = plan
        .actions()
        .iter()
        .find(|action| action.package_file_id().as_str() == path)
        .unwrap();
    let registry =
        ContentTransformerRegistry::new(vec![Arc::new(MhwEquipmentMrl3TexturePathTransformer)])
            .unwrap();
    registry
        .transform(ContentTransformRequest::new(
            action.content_transform().unwrap(),
            action.package_file_id(),
            bytes,
            &BTreeMap::new(),
        ))
        .unwrap()
        .into_bytes()
}

#[test]
fn weapon_migrates_every_owned_resource_and_only_rewrites_packaged_texture_references() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mod3",
        "nativePC/wp/two/two028/mod/two028.mrl3",
        "nativePC/wp/two/two028/mod/two028_BML.tex",
        "nativePC/wp/two/two028/mod/作者自定义.tex",
        "nativePC/wp/two/two028/mod/custom.dds",
        "nativePC/wp/two/two028/notes/two028_two029.data",
        "nativePC/wp/author/shared.tex",
    ];
    let input = material(&[
        r"wp\two\two028\mod\two028_BML",
        "NATIVEPC/wp/two/two028/mod/作者自定义.tex",
        r"wp\two\two028\mod\vanilla_NM",
        "wp/author/shared",
    ]);
    let reader = Materials(BTreeMap::from([(
        PackageFileId::new(paths[1]),
        input.clone(),
    )]));
    let plan = MhwReplacementAdapter
        .build_retarget_plan_with_content(request(&paths, "two028", "two029", true), &reader)
        .unwrap();
    assert_eq!(plan.actions().len(), paths.len());
    for action in plan.actions().iter().filter(|action| {
        action
            .source_relative_path()
            .as_str()
            .starts_with("nativePC/wp/two/two028/")
    }) {
        assert!(action
            .target_relative_path()
            .as_str()
            .starts_with("nativePC/wp/two/two029/"));
    }
    assert_eq!(
        plan.actions()
            .iter()
            .find(|action| action.package_file_id().as_str() == paths[4])
            .unwrap()
            .target_relative_path()
            .as_str(),
        "nativePC/wp/two/two029/mod/custom.dds"
    );
    let output = transformed(&plan, paths[1], &input);
    assert_eq!(
        references(&output),
        [
            r"wp\two\two029\mod\two029_BML",
            "nativePC/wp/two/two029/mod/作者自定义.tex",
            r"wp\two\two028\mod\vanilla_NM",
            "wp/author/shared"
        ]
    );
    for (index, (before, after)) in input.iter().zip(&output).enumerate() {
        if before != after {
            assert!((56..312).contains(&index) || (328..584).contains(&index));
        }
    }
    assert_eq!(input.len(), output.len());
    plan.validate_transform_facts().unwrap();
}

#[test]
fn armor_migrates_named_and_custom_textures_for_all_parts_without_requiring_model_pairs() {
    for part in ["arm", "body", "helm", "leg", "wst"] {
        let root = format!("nativePC/pl/f_equip/pl078_0000/{part}/mod");
        let mrl = format!("{root}/f_{part}078_0000.mrl3");
        let tex = format!("{root}/f_{part}078_0000_BML.tex");
        let custom = format!("{root}/skin.custom.tex");
        let paths = [mrl.as_str(), tex.as_str(), custom.as_str()];
        let input = material(&[&tex[9..tex.len() - 4], &custom[9..custom.len() - 4]]);
        let reader = Materials(BTreeMap::from([(PackageFileId::new(&mrl), input.clone())]));
        let plan = MhwReplacementAdapter
            .build_retarget_plan_with_content(
                request(&paths, "pl078_0000", "pl129_0000", true),
                &reader,
            )
            .unwrap();
        assert!(plan.actions().iter().all(|action| !action
            .target_relative_path()
            .as_str()
            .contains("pl078_0000/")));
        assert_eq!(
            references(&transformed(&plan, &mrl, &input)),
            [
                format!("pl/f_equip/pl129_0000/{part}/mod/f_{part}129_0000_BML"),
                format!("pl/f_equip/pl129_0000/{part}/mod/skin.custom")
            ]
        );
    }
}

#[test]
fn group_rewrites_shared_references_in_unchanged_sources_and_package_materials() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mrl3",
        "nativePC/wp/two/two028/mod/shared.tex",
        "nativePC/wp/two/two020/mod/two020.mrl3",
        "nativePC/common/author.mrl3",
    ];
    let input = material(&["wp/two/two028/mod/shared"]);
    let reader = Materials(
        [paths[0], paths[2], paths[3]]
            .into_iter()
            .map(|path| (PackageFileId::new(path), input.clone()))
            .collect(),
    );
    let plans = MhwReplacementAdapter
        .build_retarget_plans_with_content(
            vec![
                request(&paths, "two028", "two029", false),
                request(&paths, "two020", "two020", true),
            ],
            &reader,
        )
        .unwrap();
    for (plan, path) in [
        (&plans[0], paths[0]),
        (&plans[1], paths[2]),
        (&plans[1], paths[3]),
    ] {
        assert_eq!(
            references(&transformed(plan, path, &input)),
            ["wp/two/two029/mod/shared"]
        );
    }
    assert!(plans[1]
        .actions()
        .iter()
        .all(|action| action.source_relative_path() == action.target_relative_path()));
    assert_eq!(
        plans
            .iter()
            .flat_map(|plan| plan.actions())
            .filter(|action| action.source_relative_path().as_str() == paths[1])
            .count(),
        1
    );
}

#[test]
fn swapped_targets_use_original_paths_once_instead_of_cascading_replacements() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mrl3",
        "nativePC/wp/two/two028/mod/two028_BML.tex",
        "nativePC/wp/two/two029/mod/two029.mrl3",
        "nativePC/wp/two/two029/mod/two029_BML.tex",
    ];
    let input = material(&[
        "wp/two/two028/mod/two028_BML",
        "wp/two/two029/mod/two029_BML",
    ]);
    let reader = Materials(
        [paths[0], paths[2]]
            .into_iter()
            .map(|path| (PackageFileId::new(path), input.clone()))
            .collect(),
    );
    let plans = MhwReplacementAdapter
        .build_retarget_plans_with_content(
            vec![
                request(&paths, "two028", "two029", true),
                request(&paths, "two029", "two028", false),
            ],
            &reader,
        )
        .unwrap();
    assert_eq!(
        references(&transformed(&plans[0], paths[0], &input)),
        [
            "wp/two/two029/mod/two029_BML",
            "wp/two/two028/mod/two028_BML"
        ]
    );
}

#[test]
fn malformed_material_table_and_missing_content_cannot_produce_an_incomplete_migration() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mrl3",
        "nativePC/wp/two/two028/mod/two028_BML.tex",
    ];
    let base = material(&["wp/two/two028/mod/two028_BML"]);
    let mut corruptions = vec![vec![0; 10]];
    let mut overlap = base.clone();
    overlap[32..40].copy_from_slice(&40u64.to_le_bytes());
    corruptions.push(overlap);
    let mut overflow = base.clone();
    overflow[24..32].copy_from_slice(&u64::MAX.to_le_bytes());
    corruptions.push(overflow);
    let mut unterminated = base.clone();
    unterminated[56..312].fill(b'x');
    corruptions.push(unterminated);
    let mut excessive = base;
    excessive[20..24].copy_from_slice(&4097u32.to_le_bytes());
    corruptions.push(excessive);
    for bytes in corruptions {
        let reader = Materials(BTreeMap::from([(PackageFileId::new(paths[0]), bytes)]));
        assert!(matches!(
            MhwReplacementAdapter.build_retarget_plan_with_content(
                request(&paths, "two028", "two029", true),
                &reader
            ),
            Err(ReplacementAdapterError::SourceAnalysisRejected {
                code: "equipment_material_format_invalid",
                ..
            })
        ));
    }
    assert_eq!(
        MhwReplacementAdapter
            .build_retarget_plan(request(&paths, "two028", "two029", true))
            .unwrap_err(),
        ReplacementAdapterError::SourceContentUnavailable
    );
    assert!(MhwReplacementAdapter
        .build_retarget_plan(request(&paths, "two028", "two028", true))
        .is_ok());
}

#[test]
fn material_with_many_texture_fields_is_supported_and_transform_hashes_reject_tampering() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mrl3",
        "nativePC/wp/two/two028/mod/custom.tex",
    ];
    let input = material(&vec!["wp/two/two028/mod/custom"; 64]);
    let reader = Materials(BTreeMap::from([(
        PackageFileId::new(paths[0]),
        input.clone(),
    )]));
    let plan = MhwReplacementAdapter
        .build_retarget_plan_with_content(request(&paths, "two028", "two029", true), &reader)
        .unwrap();
    assert_eq!(
        references(&transformed(&plan, paths[0], &input)),
        vec!["wp/two/two029/mod/custom"; 64]
    );
    let action = plan
        .actions()
        .iter()
        .find(|action| action.content_transform().is_some())
        .unwrap();
    let registry =
        ContentTransformerRegistry::new(vec![Arc::new(MhwEquipmentMrl3TexturePathTransformer)])
            .unwrap();
    let mut changed = input;
    changed[8] ^= 1;
    assert!(registry
        .transform(ContentTransformRequest::new(
            action.content_transform().unwrap(),
            action.package_file_id(),
            &changed,
            &BTreeMap::new()
        ))
        .is_err());
}

#[test]
fn material_and_texture_win32_suffixes_still_migrate_the_same_reference() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.MRL3.",
        "nativePC/wp/two/two028/mod/skin.TeX.",
    ];
    let input = material(&["wp/two/two028/mod/skin", ""]);
    let reader = Materials(BTreeMap::from([(
        PackageFileId::new(paths[0]),
        input.clone(),
    )]));
    let plan = MhwReplacementAdapter
        .build_retarget_plan_with_content(request(&paths, "two028", "two029", true), &reader)
        .unwrap();
    assert_eq!(
        references(&transformed(&plan, paths[0], &input)),
        ["wp/two/two029/mod/skin", ""]
    );
}

#[test]
fn unsafe_or_undecodable_nonempty_references_cannot_leave_unresolved_old_texture_paths() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mrl3",
        "nativePC/wp/two/two028/mod/skin.tex",
    ];
    let mut undecodable = material(&["wp/two/two028/mod/skin"]);
    undecodable[56] = 0xff;
    for input in [
        undecodable,
        material(&["../wp/two/two028/mod/skin"]),
        material(&["C:/nativePC/wp/two/two028/mod/skin"]),
    ] {
        let reader = Materials(BTreeMap::from([(PackageFileId::new(paths[0]), input)]));
        assert!(matches!(
            MhwReplacementAdapter.build_retarget_plan_with_content(
                request(&paths, "two028", "two029", true),
                &reader
            ),
            Err(ReplacementAdapterError::SourceAnalysisRejected {
                code: "equipment_material_reference_unsafe",
                ..
            })
        ));
    }
}

#[test]
fn repeated_source_requests_cannot_silently_override_a_texture_destination() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mrl3",
        "nativePC/wp/two/two028/mod/skin.tex",
    ];
    let reader = Materials(BTreeMap::from([(
        PackageFileId::new(paths[0]),
        material(&["wp/two/two028/mod/skin"]),
    )]));
    assert_eq!(
        MhwReplacementAdapter
            .build_retarget_plans_with_content(
                vec![
                    request(&paths, "two028", "two029", false),
                    request(&paths, "two028", "two003", true),
                ],
                &reader
            )
            .unwrap_err(),
        ReplacementAdapterError::InvalidRetargetPlan
    );
}

#[test]
fn maximum_texture_table_is_bounded_and_overlong_output_is_rejected() {
    let paths = [
        "nativePC/wp/two/two028/mod/two028.mrl3",
        "nativePC/wp/two/two028/mod/custom.tex",
    ];
    let input = material(&vec!["wp/two/two028/mod/custom"; 4096]);
    let reader = Materials(BTreeMap::from([(
        PackageFileId::new(paths[0]),
        input.clone(),
    )]));
    let plan = MhwReplacementAdapter
        .build_retarget_plan_with_content(request(&paths, "two028", "two029", true), &reader)
        .unwrap();
    assert_eq!(
        references(&transformed(&plan, paths[0], &input)),
        vec!["wp/two/two029/mod/custom"; 4096]
    );
    let action = plan
        .actions()
        .iter()
        .find(|action| action.content_transform().is_some())
        .unwrap();
    let original = action.content_transform().unwrap();
    let too_long = hmm_core::ContentTransformInvocation::new(
        1,
        original.transformer_id(),
        original.transformer_version(),
        original.source_content_sha256(),
        original.output_content_sha256(),
        original.canonical_mapping_sha256(),
        BTreeMap::new(),
        BTreeMap::from([("texture_0".to_owned(), "x".repeat(256))]),
    )
    .unwrap();
    let registry =
        ContentTransformerRegistry::new(vec![Arc::new(MhwEquipmentMrl3TexturePathTransformer)])
            .unwrap();
    let error = registry
        .transform(ContentTransformRequest::new(
            &too_long,
            action.package_file_id(),
            &input,
            &BTreeMap::new(),
        ))
        .unwrap_err();
    assert_eq!(
        error,
        hmm_ports::ContentTransformDispatchError::TransformFailed(
            hmm_ports::ContentTransformerError::rejected("equipment_material_path_too_long")
        )
    );
}
