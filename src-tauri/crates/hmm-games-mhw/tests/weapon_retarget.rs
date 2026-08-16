use hmm_core::PackageFileId;
use hmm_games_mhw::{
    analyze_mhw_weapon_assets, generate_mhw_equipment_stable_id, EquipmentCandidateTargetKind,
    MhwWeaponCatalogSource, WeaponAnalysisError, WeaponAnalysisWarning, WeaponCatalogSourceError,
    WeaponFamily, WeaponFamilyError, WeaponMainId, WeaponModelAssetKind, WeaponModelAssetPath,
    WeaponPartRole, WeaponPathError, WeaponResourceRoot, WeaponTargetStatus,
    MHW_WEAPON_CATALOG_SOURCE_SCHEMA_VERSION,
};
use hmm_ports::ReplacementAsset;
use serde_json::{json, Value};

fn asset(id: &str, relative_path: &str) -> ReplacementAsset {
    ReplacementAsset::new(PackageFileId::new(id), relative_path)
}

fn model_path(family: &str, main_id: &str, part_id: &str, extension: &str) -> String {
    format!("nativePC/wp/{family}/{main_id}/mod/{part_id}.{extension}")
}

fn pair_assets(
    family: &str,
    main_id: &str,
    part_id: &str,
    id_prefix: &str,
) -> Vec<ReplacementAsset> {
    vec![
        asset(
            &format!("{id_prefix}-mod3"),
            &model_path(family, main_id, part_id, "mod3"),
        ),
        asset(
            &format!("{id_prefix}-mrl3"),
            &model_path(family, main_id, part_id, "mrl3"),
        ),
    ]
}

fn family_fixtures() -> [(WeaponFamily, &'static str); 14] {
    [
        (WeaponFamily::GreatSword, "two"),
        (WeaponFamily::SwordAndShield, "one"),
        (WeaponFamily::DualBlades, "sou"),
        (WeaponFamily::LongSword, "swo"),
        (WeaponFamily::Hammer, "ham"),
        (WeaponFamily::HuntingHorn, "hue"),
        (WeaponFamily::Lance, "lan"),
        (WeaponFamily::Gunlance, "gun"),
        (WeaponFamily::SwitchAxe, "saxe"),
        (WeaponFamily::ChargeBlade, "caxe"),
        (WeaponFamily::InsectGlaive, "rod"),
        (WeaponFamily::Bow, "bow"),
        (WeaponFamily::HeavyBowgun, "hbg"),
        (WeaponFamily::LightBowgun, "lbg"),
    ]
}

fn catalog_target(family: WeaponFamily, number: u16, status: &str) -> Value {
    let main_id = format!("{}{:03}", family.as_str(), number);
    let resource_path = format!("nativePC/wp/{}/{main_id}", family.as_str());
    let stable_id = generate_mhw_equipment_stable_id(
        EquipmentCandidateTargetKind::Weapon,
        family.path_family(),
        &resource_path,
    )
    .expect("artificial target identity");

    json!({
        "stable_id": stable_id,
        "target_type": "weapon",
        "resource_path": resource_path,
        "internal_id": main_id,
        "metadata": {
            "family": family.as_str(),
            "path_family": family.path_family()
        },
        "status": status,
        "names": {
            "en": {
                "display_name": format!("Artificial {} target", family.as_str()),
                "aliases": [format!("{} fixture alias", family.as_str())]
            }
        },
        "legacy_ids": [format!("mhw:weapon:fixture-{}", family.as_str())]
    })
}

fn catalog_json(targets: Vec<Value>) -> String {
    json!({
        "schema_version": MHW_WEAPON_CATALOG_SOURCE_SCHEMA_VERSION,
        "catalog_version": "artificial-weapon-v1",
        "game_id": "mhw",
        "targets": targets
    })
    .to_string()
}

#[test]
fn family_registry_parses_all_fourteen_normal_and_bs_main_ids() {
    assert_eq!(WeaponFamily::ALL.len(), 14);

    for (family, token) in family_fixtures() {
        assert_eq!(WeaponFamily::parse(token), Ok(family));
        assert_eq!(family.as_str(), token);
        assert_eq!(family.path_family(), format!("wp/{token}"));

        let normal = WeaponMainId::parse(&format!("{token}007")).expect("normal main id");
        assert_eq!(normal.family(), family);
        assert_eq!(normal.number(), 7);
        assert!(!normal.has_bs_prefix());

        let bs = WeaponMainId::parse(&format!("bs_{token}007")).expect("bs main id");
        assert_eq!(bs.family(), family);
        assert_eq!(bs.number(), 7);
        assert!(bs.has_bs_prefix());
    }
}

#[test]
fn family_registry_derives_the_six_known_secondary_part_mappings() {
    let fixtures = [
        ("one", WeaponPartRole::Shield, "sld"),
        ("sou", WeaponPartRole::Right, "sou_r"),
        ("swo", WeaponPartRole::Sheath, "saya"),
        ("lan", WeaponPartRole::Shield, "sld"),
        ("gun", WeaponPartRole::Shield, "sld"),
        ("caxe", WeaponPartRole::Shield, "sld"),
    ];

    for (family_token, role, part_prefix) in fixtures {
        let family = WeaponFamily::parse(family_token).expect("known family");
        let secondary = family.secondary_part().expect("known secondary part");
        assert_eq!(secondary.role(), role);
        assert_eq!(secondary.prefix(), part_prefix);

        let main = WeaponMainId::parse(&format!("bs_{family_token}042")).expect("main id");
        assert_eq!(
            main.part_for_role(role).expect("secondary part").as_str(),
            format!("bs_{part_prefix}042")
        );
    }

    assert!(WeaponFamily::GreatSword.secondary_part().is_none());
}

#[test]
fn family_registry_rejects_unknown_malformed_and_mismatched_ids() {
    assert_eq!(
        WeaponFamily::parse("unknown"),
        Err(WeaponFamilyError::UnknownFamily)
    );
    for invalid in ["one01", "one0001", "ONE001", "bs_one01", "one0a1"] {
        assert_eq!(
            WeaponMainId::parse(invalid),
            Err(WeaponFamilyError::InvalidMainId)
        );
    }
    assert_eq!(
        WeaponMainId::parse_for_family("two001", WeaponFamily::SwordAndShield),
        Err(WeaponFamilyError::FamilyMismatch)
    );

    let main = WeaponMainId::parse("one001").expect("main id");
    assert_eq!(
        main.part_for_role(WeaponPartRole::Right),
        Err(WeaponFamilyError::UnknownPart)
    );
}

#[test]
fn path_parser_normalizes_separators_and_retargets_only_structured_segments() {
    let forward = WeaponResourceRoot::parse("nativePC/wp/one/one001").expect("forward root");
    let backward = WeaponResourceRoot::parse(r"nativePC\wp\one\one001").expect("backslash root");
    assert_eq!(forward, backward);
    assert_eq!(forward.family(), WeaponFamily::SwordAndShield);
    assert_eq!(forward.main_id().as_str(), "one001");

    let main =
        WeaponModelAssetPath::parse("nativePC/wp/one/one001/mod/one001.mod3").expect("main model");
    assert_eq!(main.kind(), WeaponModelAssetKind::Mod3);
    assert_eq!(main.part_id().role(), WeaponPartRole::Main);

    let shield = WeaponModelAssetPath::parse(r"nativePC\wp\one\one001\mod\sld001.mrl3")
        .expect("shield material");
    assert_eq!(shield.kind(), WeaponModelAssetKind::Mrl3);
    assert_eq!(shield.part_id().role(), WeaponPartRole::Shield);

    let target = WeaponMainId::parse("one042").expect("same-family target");
    assert_eq!(
        shield.retarget(&target).expect("retarget").as_str(),
        "nativePC/wp/one/one042/mod/sld042.mrl3"
    );
    let cross_family = WeaponMainId::parse("two042").expect("other-family target");
    assert_eq!(
        shield.retarget(&cross_family),
        Err(WeaponPathError::CrossFamilyTarget)
    );

    let bs_shield = WeaponModelAssetPath::parse("nativePC/wp/caxe/bs_caxe042/mod/bs_sld042.mod3")
        .expect("bs shield model");
    assert_eq!(bs_shield.part_id().role(), WeaponPartRole::Shield);
    assert!(bs_shield.part_id().has_bs_prefix());
}

#[test]
fn path_parser_rejects_unsafe_unknown_and_unsupported_resources() {
    for unsafe_path in [
        "../nativePC/wp/one/one001/mod/one001.mod3",
        "C:/nativePC/wp/one/one001/mod/one001.mod3",
        " nativePC/wp/one/one001/mod/one001.mod3",
    ] {
        assert_eq!(
            WeaponModelAssetPath::parse(unsafe_path),
            Err(WeaponPathError::UnsafePath)
        );
    }

    assert_eq!(
        WeaponResourceRoot::parse("nativePC/wp/unknown/unknown001"),
        Err(WeaponPathError::UnknownFamily)
    );
    assert_eq!(
        WeaponResourceRoot::parse("nativePC/wp/one/two001"),
        Err(WeaponPathError::InvalidMainId)
    );
    assert_eq!(
        WeaponModelAssetPath::parse("nativePC/wp/one/one001/mod/other001.mod3"),
        Err(WeaponPathError::UnknownPart)
    );
    for unsupported in [
        "nativePC/wp/one/one001/mod/one001.tex",
        "nativePC/wp/one/one001/mod/one001.MOD3",
        "nativePC/wp/one/one001/patch/one001.mod3",
        "nativePC/wp/one/one001/mod/nested/one001.mod3",
    ] {
        assert_eq!(
            WeaponModelAssetPath::parse(unsupported),
            Err(WeaponPathError::UnsupportedResource)
        );
    }
}

#[test]
fn source_analysis_builds_a_deterministic_full_pair_closure() {
    let mut assets = pair_assets("one", "one001", "sld001", "shield");
    assets.extend(pair_assets("one", "one001", "one001", "main"));
    assets.reverse();

    let closure = analyze_mhw_weapon_assets(&assets).expect("valid source closure");
    assert_eq!(closure.family(), WeaponFamily::SwordAndShield);
    assert_eq!(closure.root().main_id().as_str(), "one001");
    assert_eq!(closure.asset_count(), 4);
    assert_eq!(closure.pairs().len(), 2);
    assert!(closure.warnings().is_empty());
    assert_eq!(closure.pairs()[0].part_id().role(), WeaponPartRole::Main);
    assert_eq!(closure.pairs()[1].part_id().role(), WeaponPartRole::Shield);
    assert_eq!(
        closure.pairs()[0].mod3().package_file_id().as_str(),
        "main-mod3"
    );

    let expected_source_id = generate_mhw_equipment_stable_id(
        EquipmentCandidateTargetKind::Weapon,
        "wp/one",
        "nativePC/wp/one/one001",
    )
    .expect("source id");
    assert_eq!(closure.source_id().as_str(), expected_source_id);
}

#[test]
fn source_analysis_accepts_main_only_and_secondary_only_complete_pairs_with_warning() {
    let main_only = analyze_mhw_weapon_assets(&pair_assets("one", "one001", "one001", "main"))
        .expect("main-only closure");
    assert_eq!(
        main_only.warnings(),
        &[WeaponAnalysisWarning::PartialPartSet]
    );

    let secondary_only =
        analyze_mhw_weapon_assets(&pair_assets("one", "one001", "sld001", "shield"))
            .expect("secondary-only closure");
    assert_eq!(secondary_only.pairs().len(), 1);
    assert_eq!(
        secondary_only.warnings(),
        &[WeaponAnalysisWarning::PartialPartSet]
    );

    let great_sword = analyze_mhw_weapon_assets(&pair_assets("two", "two001", "two001", "main"))
        .expect("family without a known secondary part");
    assert!(great_sword.warnings().is_empty());
}

#[test]
fn source_analysis_rejects_incomplete_unknown_and_unsupported_parts() {
    assert_eq!(
        analyze_mhw_weapon_assets(&[asset("main-mod3", "nativePC/wp/one/one001/mod/one001.mod3",)]),
        Err(WeaponAnalysisError::IncompleteBinaryPair)
    );
    assert_eq!(
        analyze_mhw_weapon_assets(&[asset(
            "unknown-mod3",
            "nativePC/wp/one/one001/mod/other001.mod3",
        )]),
        Err(WeaponAnalysisError::UnknownPart)
    );
    assert_eq!(
        analyze_mhw_weapon_assets(&[asset("texture", "nativePC/wp/one/one001/mod/one001.tex",)]),
        Err(WeaponAnalysisError::UnsupportedResource)
    );
}

#[test]
fn source_analysis_rejects_package_identity_and_path_collisions() {
    assert_eq!(
        analyze_mhw_weapon_assets(&[asset(" ", "nativePC/wp/two/two001/mod/two001.mod3",)]),
        Err(WeaponAnalysisError::InvalidPackageFileId)
    );

    let duplicate_id = vec![
        asset("same", "nativePC/wp/two/two001/mod/two001.mod3"),
        asset("same", "nativePC/wp/two/two001/mod/two001.mrl3"),
    ];
    assert_eq!(
        analyze_mhw_weapon_assets(&duplicate_id),
        Err(WeaponAnalysisError::DuplicatePackageFileId)
    );

    let duplicate_path = vec![
        asset("first", "nativePC/wp/two/two001/mod/two001.mod3"),
        asset("second", r"nativePC\wp\two\two001\mod\two001.mod3"),
    ];
    assert_eq!(
        analyze_mhw_weapon_assets(&duplicate_path),
        Err(WeaponAnalysisError::DuplicateAssetPath)
    );

    let case_collision = vec![
        asset("first", "nativePC/wp/two/two001/mod/two001.mod3"),
        asset("second", "nativePC/wp/two/two001/mod/TWO001.mod3"),
    ];
    assert_eq!(
        analyze_mhw_weapon_assets(&case_collision),
        Err(WeaponAnalysisError::CaseInsensitivePathCollision)
    );

    assert_eq!(
        analyze_mhw_weapon_assets(&[asset("escape", "../escape.mod3")]),
        Err(WeaponAnalysisError::UnsafePath)
    );
}

#[test]
fn source_analysis_rejects_multiple_mixed_and_non_weapon_payloads() {
    let mut multiple = pair_assets("one", "one001", "one001", "first");
    multiple.extend(pair_assets("one", "one002", "one002", "second"));
    assert_eq!(
        analyze_mhw_weapon_assets(&multiple),
        Err(WeaponAnalysisError::MultipleSourceRoots)
    );

    let mut mixed_family = pair_assets("one", "one001", "one001", "one");
    mixed_family.extend(pair_assets("two", "two001", "two001", "two"));
    assert_eq!(
        analyze_mhw_weapon_assets(&mixed_family),
        Err(WeaponAnalysisError::MixedFamily)
    );

    let mut mixed_payload = pair_assets("one", "one001", "one001", "main");
    mixed_payload.push(asset(
        "armor",
        "nativePC/pl/f_equip/pl900_0000/arm/mod/body.mod3",
    ));
    assert_eq!(
        analyze_mhw_weapon_assets(&mixed_payload),
        Err(WeaponAnalysisError::MixedInstallPayload)
    );
    assert_eq!(
        analyze_mhw_weapon_assets(&[asset("readme", "readme.txt")]),
        Err(WeaponAnalysisError::SourceNotFound)
    );
}

#[test]
fn artificial_catalog_parses_all_families_and_resolves_aliases_and_legacy_ids() {
    let targets = family_fixtures()
        .into_iter()
        .enumerate()
        .map(|(index, (family, _))| {
            let status = if family == WeaponFamily::LightBowgun {
                "hidden"
            } else {
                "active"
            };
            catalog_target(family, 900 + index as u16, status)
        })
        .collect();
    let catalog = MhwWeaponCatalogSource::parse(&catalog_json(targets))
        .expect("artificial 14-family catalog");

    assert_eq!(catalog.catalog_version(), "artificial-weapon-v1");
    assert_eq!(catalog.targets().len(), 14);
    let one = catalog
        .resolve("mhw:weapon:fixture-one")
        .expect("unique legacy id");
    assert_eq!(one.family(), WeaponFamily::SwordAndShield);
    assert_eq!(one.display_name("zh_cn"), "Artificial one target");
    assert_eq!(catalog.search("one fixture alias", false), vec![one]);

    assert!(catalog.search("lbg fixture alias", false).is_empty());
    let hidden = catalog.search("lbg fixture alias", true);
    assert_eq!(hidden.len(), 1);
    assert_eq!(hidden[0].status(), WeaponTargetStatus::Hidden);
    assert!(catalog.resolve(hidden[0].id().as_str()).is_some());
}

#[test]
fn catalog_identity_ignores_display_alias_order_and_status() {
    let mut first = catalog_target(WeaponFamily::Bow, 950, "active");
    first["names"]["en"]["aliases"] = json!(["First alias", "Second alias"]);
    let mut second = first.clone();
    second["status"] = json!("hidden");
    second["names"]["en"]["display_name"] = json!("Renamed artificial target");
    second["names"]["en"]["aliases"] = json!(["Second alias", "First alias"]);

    let first_catalog =
        MhwWeaponCatalogSource::parse(&catalog_json(vec![first])).expect("first catalog");
    let second_catalog =
        MhwWeaponCatalogSource::parse(&catalog_json(vec![second])).expect("second catalog");
    assert_eq!(
        first_catalog.targets()[0].id(),
        second_catalog.targets()[0].id()
    );
    assert_eq!(
        first_catalog.targets()[0].aliases(),
        second_catalog.targets()[0].aliases()
    );
}

#[test]
fn catalog_rejects_noncanonical_paths_identity_and_metadata_drift() {
    let mut noncanonical = catalog_target(WeaponFamily::SwordAndShield, 960, "active");
    noncanonical["resource_path"] = json!(r"nativePC\wp\one\one960");
    assert_eq!(
        MhwWeaponCatalogSource::parse(&catalog_json(vec![noncanonical])),
        Err(WeaponCatalogSourceError::NonCanonicalResourcePath)
    );

    let mut bad_id = catalog_target(WeaponFamily::SwordAndShield, 961, "active");
    bad_id["stable_id"] = json!("mhw:weapon:not-the-resource-hash");
    assert_eq!(
        MhwWeaponCatalogSource::parse(&catalog_json(vec![bad_id])),
        Err(WeaponCatalogSourceError::StableIdMismatch)
    );

    let mut bad_metadata = catalog_target(WeaponFamily::SwordAndShield, 962, "active");
    bad_metadata["metadata"]["family"] = json!("two");
    assert_eq!(
        MhwWeaponCatalogSource::parse(&catalog_json(vec![bad_metadata])),
        Err(WeaponCatalogSourceError::InvalidTargetMetadata)
    );

    let mut unknown_field: Value = serde_json::from_str(&catalog_json(vec![catalog_target(
        WeaponFamily::SwordAndShield,
        963,
        "active",
    )]))
    .expect("catalog value");
    unknown_field["targets"][0]["source_path"] = json!("sensitive-value");
    assert_eq!(
        MhwWeaponCatalogSource::parse(&unknown_field.to_string()),
        Err(WeaponCatalogSourceError::InvalidJson)
    );
}

#[test]
fn catalog_rejects_path_collisions_ambiguous_legacy_ids_and_dummy_targets() {
    let first = catalog_target(WeaponFamily::SwordAndShield, 970, "active");
    assert_eq!(
        MhwWeaponCatalogSource::parse(&catalog_json(vec![first.clone(), first.clone()])),
        Err(WeaponCatalogSourceError::DuplicateResourcePath)
    );

    let mut case_collision = first.clone();
    case_collision["resource_path"] = json!("nativePC/wp/one/ONE970");
    assert_eq!(
        MhwWeaponCatalogSource::parse(&catalog_json(vec![first.clone(), case_collision])),
        Err(WeaponCatalogSourceError::CaseInsensitivePathCollision)
    );

    let mut legacy_owner = first;
    let second = catalog_target(WeaponFamily::GreatSword, 971, "active");
    legacy_owner["legacy_ids"] = json!([second["stable_id"].clone()]);
    assert_eq!(
        MhwWeaponCatalogSource::parse(&catalog_json(vec![legacy_owner, second])),
        Err(WeaponCatalogSourceError::AmbiguousId)
    );

    let dummy = catalog_target(WeaponFamily::Bow, 972, "dummy");
    assert_eq!(
        MhwWeaponCatalogSource::parse(&catalog_json(vec![dummy])),
        Err(WeaponCatalogSourceError::DummyTarget)
    );
}

#[test]
fn public_errors_expose_stable_codes_without_echoing_candidate_values() {
    let path_error =
        WeaponModelAssetPath::parse("C:/Users/Sensitive/nativePC/wp/one/one001/mod/one001.mod3")
            .expect_err("unsafe path");
    assert_eq!(path_error.code(), "weapon_unsafe_path");
    assert!(!path_error.to_string().contains("Sensitive"));

    let mut invalid = catalog_target(WeaponFamily::SwordAndShield, 980, "active");
    invalid["resource_path"] = json!("C:/Users/Sensitive/weapon");
    let catalog_error = MhwWeaponCatalogSource::parse(&catalog_json(vec![invalid]))
        .expect_err("unsafe catalog path");
    assert_eq!(catalog_error.code(), "weapon_catalog_unsafe_resource_path");
    assert!(!catalog_error.to_string().contains("Sensitive"));

    assert_eq!(
        WeaponAnalysisError::MixedInstallPayload.code(),
        "weapon_mixed_install_payload"
    );
    assert_eq!(
        WeaponAnalysisWarning::PartialPartSet.code(),
        "weapon_partial_part_set"
    );
}
