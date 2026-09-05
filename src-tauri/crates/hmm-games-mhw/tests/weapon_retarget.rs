use hmm_core::{PackageFileId, ReplacementTargetId};
use hmm_games_mhw::{
    analyze_mhw_weapon_assets, generate_mhw_equipment_stable_id, EquipmentCandidateTargetKind,
    MhwReplacementCatalog, MhwWeaponCatalogSource, WeaponAnalysisError, WeaponAnalysisWarning,
    WeaponCatalogSourceError, WeaponFamily, WeaponFamilyError, WeaponMainId, WeaponModelAssetKind,
    WeaponModelAssetPath, WeaponPartRole, WeaponPathError, WeaponResourceRoot, WeaponTargetStatus,
    WeaponUnresolvedModelReason, MHW_WEAPON_CATALOG_SOURCE_SCHEMA_VERSION,
};
use hmm_ports::{ReplacementAsset, ReplacementCatalogProvider};
use serde_json::{json, Value};
use std::collections::BTreeSet;

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
    /*
     * #343：未登记前缀**不再**是拒绝理由。`other001` 带着本槽位的数字 `001`，改名规则
     * 完全知道该怎么做（`other001` → `other002`），前缀叫什么无关紧要。
     */
    let auxiliary = WeaponModelAssetPath::parse("nativePC/wp/one/one001/mod/other001.mod3")
        .expect("未登记前缀的模型不得被拒绝");
    assert_eq!(auxiliary.part_id().role(), WeaponPartRole::Auxiliary);
    assert_eq!(
        auxiliary
            .retarget(&WeaponMainId::parse("one002").expect("target"))
            .expect("retarget")
            .as_str(),
        "nativePC/wp/one/one002/mod/other002.mod3",
        "前缀逐字保留，只换槽位数字"
    );

    // 仍然失败关闭的：模型不带本槽位的数字，改名无从下手。
    for unknown in [
        "nativePC/wp/one/one001/mod/other999.mod3",
        "nativePC/wp/one/one001/mod/nodigits.mod3",
    ] {
        assert_eq!(
            WeaponModelAssetPath::parse(unknown),
            Err(WeaponPathError::UnknownPart),
            "{unknown}"
        );
    }
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

    let closure = analyze_mhw_weapon_assets(&assets)
        .expect("valid source closure")
        .sole_unit()
        .expect("恰好一个可重定向单元")
        .clone();
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
        .expect("main-only closure")
        .sole_unit()
        .expect("恰好一个可重定向单元")
        .clone();
    assert_eq!(
        main_only.warnings(),
        &[WeaponAnalysisWarning::PartialPartSet]
    );

    let secondary_only =
        analyze_mhw_weapon_assets(&pair_assets("one", "one001", "sld001", "shield"))
            .expect("secondary-only closure")
            .sole_unit()
            .expect("恰好一个可重定向单元")
            .clone();
    assert_eq!(secondary_only.pairs().len(), 1);
    assert_eq!(
        secondary_only.warnings(),
        &[WeaponAnalysisWarning::PartialPartSet]
    );

    let great_sword = analyze_mhw_weapon_assets(&pair_assets("two", "two001", "two001", "main"))
        .expect("family without a known secondary part")
        .sole_unit()
        .expect("恰好一个可重定向单元")
        .clone();
    assert!(great_sword.warnings().is_empty());
}

/// `#349`：一个槽位里**没有任何**完整模型对时，那个单元不成立；整个包里一个单元都没有
/// 才落到包级下限 `SourceNotFound`。
///
/// 原先这几种形态各报一个专门的错误码并否决整包（`IncompleteBinaryPair` / `UnknownPart`）。
/// 现在包级只剩一条下限——「没有可重定向的东西」是事实陈述，不是对包的判决。
#[test]
fn a_slot_without_any_complete_pair_yields_no_unit() {
    // 只有 `.mod3`、缺配套 `.mrl3`。
    assert_eq!(
        analyze_mhw_weapon_assets(&[asset("main-mod3", "nativePC/wp/one/one001/mod/one001.mod3",)]),
        Err(WeaponAnalysisError::SourceNotFound)
    );
    // 未登记前缀的副件同理——`#343` 之后它已被当成正常部件，缺一半就是缺一半。
    assert_eq!(
        analyze_mhw_weapon_assets(&[asset(
            "auxiliary-mod3",
            "nativePC/wp/one/one001/mod/other001.mod3",
        )]),
        Err(WeaponAnalysisError::SourceNotFound)
    );
    // 不带本槽位数字的模型：认不出对应哪个部件，同样不构成单元。
    assert_eq!(
        analyze_mhw_weapon_assets(&[asset(
            "unknown-mod3",
            "nativePC/wp/one/one001/mod/other999.mod3",
        )]),
        Err(WeaponAnalysisError::SourceNotFound)
    );
    /*
     * #336：只含一张贴图、没有任何模型的包，旧版报 `UnsupportedResource`
     * （「包含当前版本不支持的资源类型，只支持 .mod3 与 .mrl3」）——那正是被抱怨的误导性
     * 文案：贴图本身没问题，问题是**没有模型**。现在诚实地报 `SourceNotFound`。
     * `.tex` 在有模型的包里会被归为随行文件（见 weapon_package_classifier.rs）。
     */
    assert_eq!(
        analyze_mhw_weapon_assets(&[asset("texture", "nativePC/wp/one/one001/mod/one001.tex",)]),
        Err(WeaponAnalysisError::SourceNotFound)
    );
}

/// `#349` 的核心收益：**主件正常就该能重定向，不该被副件拖累。**
///
/// 此前「主件成对 + 副件只有 `.mod3`」会拒整包——主件完全正常、完全可重定向，只因为
/// 副件缺了配套的 `.mrl3`。
#[test]
fn a_complete_main_part_survives_an_incomplete_auxiliary_part() {
    let mut assets = pair_assets("bow", "bow017", "bow017", "main");
    assets.push(asset(
        "aux-mod3-only",
        "nativePC/wp/bow/bow017/mod/ya017.mod3",
    ));

    let unit = analyze_mhw_weapon_assets(&assets)
        .expect("主件成对就该成立一个单元")
        .sole_unit()
        .expect("恰好一个可重定向单元")
        .clone();

    assert_eq!(unit.pairs().len(), 1, "只有主件成对");
    assert_eq!(
        unit.pairs()[0].part_id().as_str(),
        "bow017",
        "成对的那个是主件"
    );
    assert_eq!(
        unit.unresolved_models().len(),
        1,
        "缺一半的副件被单独记下，而不是拖累整包"
    );
    assert_eq!(
        unit.unresolved_models()[0].relative_path().as_str(),
        "nativePC/wp/bow/bow017/mod/ya017.mod3"
    );
    assert_eq!(
        unit.unresolved_models()[0].reason(),
        WeaponUnresolvedModelReason::IncompleteModelPair
    );
}

/// 部件名认不出的模型也留在单元里，而不是否决整包。
///
/// 这条对着 `#349` 正文第一节那个内部不一致：同一个包，多余文件是 `.dds` 就归档放行、
/// 是 `.mod3` 就拒整包。现在两者都归档，只是归到不同的档。
#[test]
fn a_model_with_an_unrecognized_part_name_stays_inside_its_unit() {
    let mut assets = pair_assets("bow", "bow017", "bow017", "main");
    assets.push(asset("arrow-mod3", "nativePC/wp/bow/bow017/mod/arrow.mod3"));
    assets.push(asset("arrow-mrl3", "nativePC/wp/bow/bow017/mod/arrow.mrl3"));

    let unit = analyze_mhw_weapon_assets(&assets)
        .expect("认不出的模型不该拒整包")
        .sole_unit()
        .expect("恰好一个可重定向单元")
        .clone();

    assert_eq!(unit.pairs().len(), 1);
    assert_eq!(
        unit.unresolved_models()
            .iter()
            .map(|model| model.relative_path().as_str())
            .collect::<Vec<_>>(),
        vec![
            "nativePC/wp/bow/bow017/mod/arrow.mod3",
            "nativePC/wp/bow/bow017/mod/arrow.mrl3",
        ]
    );
    for model in unit.unresolved_models() {
        assert_eq!(
            model.reason(),
            WeaponUnresolvedModelReason::UnrecognizedPartName
        );
    }
    assert!(
        unit.companions().is_empty(),
        "认不出的模型不能混进随行档——那会让它的 MRL3 引用在重定向后断链"
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

/// `#349`：一个包里多把武器**不再拒整包**，每把各自成为一个独立单元。
///
/// 「作者一次发布多件装备」是正常的发布习惯，不是坏包。`MultipleSourceRoots` 与
/// `MixedFamily` 两个错误码按 `#342` / `#343` 的先例保留、不再产生。
#[test]
fn source_analysis_splits_multiple_source_roots_into_independent_units() {
    let mut multiple = pair_assets("one", "one001", "one001", "first");
    multiple.extend(pair_assets("one", "one002", "one002", "second"));
    let analysis = analyze_mhw_weapon_assets(&multiple).expect("两把同族武器不该拒整包");

    assert_eq!(analysis.units().len(), 2, "两把武器 = 两个单元");
    assert_eq!(
        analysis
            .units()
            .iter()
            .map(|unit| unit.root().main_id().as_str())
            .collect::<Vec<_>>(),
        vec!["one001", "one002"],
        "单元按槽位根排序，顺序确定"
    );
    // 每个单元各带自己的模型对与身份，互不干扰。
    for unit in analysis.units() {
        assert_eq!(unit.pairs().len(), 1);
        assert!(unit.unresolved_models().is_empty());
    }
    assert_ne!(
        analysis.units()[0].source_id(),
        analysis.units()[1].source_id(),
        "两个单元必须是两个可分别绑定的源"
    );

    let mut mixed_family = pair_assets("one", "one001", "one001", "one");
    mixed_family.extend(pair_assets("two", "two001", "two001", "two"));
    let analysis = analyze_mhw_weapon_assets(&mixed_family).expect("跨族也不该拒整包");

    assert_eq!(analysis.units().len(), 2);
    assert_eq!(
        analysis
            .units()
            .iter()
            .map(|unit| unit.family())
            .collect::<BTreeSet<_>>()
            .len(),
        2,
        "两个单元分属不同武器族"
    );
}

/// 真实武器 Mod 几乎必然携带 readme、预览图、甚至顺带一份防具。这类杂项
/// 过去会触发 `MixedInstallPayload` 把整个 Mod 判死，现在只被忽略。
#[test]
fn source_analysis_ignores_non_weapon_payloads_but_requires_a_weapon_source() {
    let mut with_extras = pair_assets("one", "one001", "one001", "main");
    with_extras.push(asset("readme", "readme.txt"));
    with_extras.push(asset("preview", "preview/preview.png"));
    with_extras.push(asset(
        "armor",
        "nativePC/pl/f_equip/pl900_0000/arm/mod/body.mod3",
    ));

    let closure = analyze_mhw_weapon_assets(&with_extras)
        .expect("weapon closure alongside extras")
        .sole_unit()
        .expect("恰好一个可重定向单元")
        .clone();
    assert_eq!(closure.pairs().len(), 1);
    assert_eq!(closure.asset_count(), 2);
    assert_eq!(
        closure.root().normalized_path().as_str(),
        "nativePC/wp/one/one001"
    );

    // 门禁下限：一件武器资源都没有仍然必须失败，否则纯杂物包会被放过。
    assert_eq!(
        analyze_mhw_weapon_assets(&[asset("readme", "readme.txt")]),
        Err(WeaponAnalysisError::SourceNotFound)
    );
}

/// 绝大多数真实压缩包在 `nativePC` 之外还包了一层作者自建目录，而上游
/// 解压与扫描链路不会剥离它。武器 analysis 必须自己归一化，否则这类包
/// 会被送去防具适配器、报一个与武器无关的错误。
#[test]
fn source_analysis_strips_author_package_root_directory() {
    let wrapped = vec![
        asset(
            "mod3",
            "MyWeaponMod v1.2/nativePC/wp/one/one001/mod/one001.mod3",
        ),
        asset(
            "mrl3",
            "MyWeaponMod v1.2/nativePC/wp/one/one001/mod/one001.mrl3",
        ),
        asset("readme", "MyWeaponMod v1.2/readme.txt"),
    ];
    let closure = analyze_mhw_weapon_assets(&wrapped)
        .expect("wrapped weapon closure")
        .sole_unit()
        .expect("恰好一个可重定向单元")
        .clone();
    assert_eq!(
        closure.root().normalized_path().as_str(),
        "nativePC/wp/one/one001"
    );
    assert_eq!(closure.pairs().len(), 1);
}

/// 剥离不能成为绕过父目录遍历检测的旁路：安全校验必须先于剥离发生。
#[test]
fn source_analysis_still_rejects_parent_traversal_through_stripping() {
    assert_eq!(
        analyze_mhw_weapon_assets(&[asset(
            "escape",
            "outer/../../../escape/nativePC/wp/one/one001/mod/one001.mod3"
        )]),
        Err(WeaponAnalysisError::UnsafePath)
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

/// WR-02B 生成的 production 武器 catalog 必须能被真正的解析器接受。
///
/// 这个解析器比候选 validator 更严：它会重算每条 stable_id、要求 resource_path
/// 已是规范形式、要求 metadata.family 与路径解析出的 family 一致、并拒绝重复
/// 展示名与 dummy 条目。artifact 是脚本生成的，任何一条不合规都必须在这里红，
/// 而不是等到运行时 catalog 加载失败。
#[test]
fn production_weapon_catalog_artifact_parses_and_covers_all_families() {
    const SHARDS: [&str; 14] = [
        include_str!("../data/weapons/mhw-weapon-targets.bow.v1.json"),
        include_str!("../data/weapons/mhw-weapon-targets.caxe.v1.json"),
        include_str!("../data/weapons/mhw-weapon-targets.gun.v1.json"),
        include_str!("../data/weapons/mhw-weapon-targets.ham.v1.json"),
        include_str!("../data/weapons/mhw-weapon-targets.hbg.v1.json"),
        include_str!("../data/weapons/mhw-weapon-targets.hue.v1.json"),
        include_str!("../data/weapons/mhw-weapon-targets.lan.v1.json"),
        include_str!("../data/weapons/mhw-weapon-targets.lbg.v1.json"),
        include_str!("../data/weapons/mhw-weapon-targets.one.v1.json"),
        include_str!("../data/weapons/mhw-weapon-targets.rod.v1.json"),
        include_str!("../data/weapons/mhw-weapon-targets.saxe.v1.json"),
        include_str!("../data/weapons/mhw-weapon-targets.sou.v1.json"),
        include_str!("../data/weapons/mhw-weapon-targets.swo.v1.json"),
        include_str!("../data/weapons/mhw-weapon-targets.two.v1.json"),
    ];

    // 用 parse_sharded 而不是逐份 parse：跨分片的 stable_id / 展示名 / 路径碰撞
    // 校验只有合并后单次校验才成立，逐份 parse 会把它降级成分片内唯一。
    let catalog =
        MhwWeaponCatalogSource::parse_sharded(&SHARDS).expect("production weapon catalog");

    assert_eq!(catalog.catalog_version(), "mhw-weapon-v1");
    assert_eq!(catalog.targets().len(), 601);

    // 14 类武器一个都不能少：少一类等于那类武器的重定向目标整体消失。
    let families = catalog
        .targets()
        .iter()
        .map(|target| target.family().as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(families.len(), 14, "families: {families:?}");

    // 展示名三语齐全，否则英日界面会露出中文或空白。
    for target in catalog.targets() {
        for locale in ["zh_cn", "en", "ja"] {
            assert!(
                !target.display_name(locale).trim().is_empty(),
                "{} 缺少 {locale} 展示名",
                target.id().as_str()
            );
        }
    }
}

/// 分片合并不得削弱任何一道校验。
///
/// 单文件时 stable_id 唯一性、legacy_id 歧义、展示名唯一性和资源路径碰撞都是在
/// 单次校验里累积判定的。如果 parse_sharded 只是逐份 parse 再拼列表，这些保证会
/// 悄悄降级成「每个分片内部唯一」——两份各自合法、合起来冲突的分片就会被放行。
/// 下面每条都是「单独合法、合并冲突」，必须全部被拒。
#[test]
fn sharded_parse_still_rejects_conflicts_that_span_shards() {
    let shard_a = catalog_json(vec![catalog_target(WeaponFamily::GreatSword, 1, "active")]);

    // 同一条目出现在两份分片里：stable_id、legacy_id、展示名、资源路径全部撞车。
    let duplicate = catalog_json(vec![catalog_target(WeaponFamily::GreatSword, 1, "active")]);
    assert!(
        MhwWeaponCatalogSource::parse_sharded(&[&shard_a, &duplicate]).is_err(),
        "跨分片重复条目必须被拒"
    );

    // 展示名撞车：不同资源路径、不同 stable_id，但展示名相同。
    let mut clashing_name = catalog_target(WeaponFamily::GreatSword, 2, "active");
    clashing_name["names"]["en"]["display_name"] = json!("Artificial two target");
    clashing_name["legacy_ids"] = json!(["mhw:weapon:fixture-two-alt"]);
    let shard_b = catalog_json(vec![clashing_name]);
    assert!(
        MhwWeaponCatalogSource::parse_sharded(&[&shard_a, &shard_b]).is_err(),
        "跨分片重复展示名必须被拒"
    );

    // 对照组：真正不冲突的两份分片必须能合并，否则上面的断言可能只是"什么都拒"。
    let shard_ok = catalog_json(vec![catalog_target(WeaponFamily::LongSword, 1, "active")]);
    let merged = MhwWeaponCatalogSource::parse_sharded(&[&shard_a, &shard_ok])
        .expect("互不冲突的分片应当合并成功");
    assert_eq!(merged.targets().len(), 2);

    // 分片必须同属一份 catalog：catalog_version 不同要拒，否则合出来是拼接怪物。
    let foreign = shard_ok.replace("artificial-weapon-v1", "artificial-weapon-v2");
    assert!(
        MhwWeaponCatalogSource::parse_sharded(&[&shard_a, &foreign]).is_err(),
        "catalog_version 不一致的分片必须被拒"
    );
}

/// WR-05：聚合 catalog = armor v2 + WR-02B 全量武器分片，Production 与 Sandbox 共用。
///
/// 这条测试固定门禁翻转后的 catalog 组成：armor 部分不变、武器全量在册、
/// 元数据形状满足 `list_compatible_targets` 过滤与 plan 阶段 family 校验的需要。
#[test]
fn aggregate_catalog_exposes_full_weapon_targets() {
    let catalog = MhwReplacementCatalog
        .replacement_catalog()
        .expect("aggregate replacement catalog");

    assert_eq!(catalog.version().as_str(), "mhw-replacement-v1");
    let armor_count = catalog
        .targets()
        .iter()
        .filter(|target| target.target_type().as_str() == "armor")
        .count();
    let weapon_targets: Vec<_> = catalog
        .targets()
        .iter()
        .filter(|target| target.target_type().as_str() == "weapon")
        .collect();
    assert_eq!(armor_count, 269, "armor 目标不应被本次接线改动");
    assert_eq!(weapon_targets.len(), 601, "WR-02B 全量武器目标必须整体在册");

    // path_family 是 list_compatible_targets 的过滤键（缺失等于目标不可见），
    // family 是 plan 阶段跨 family 拒绝的依据。WR-05 起 seed 退役，
    // catalog 不再有 scope 之分，任何目标都不得携带 catalog_scope 标记。
    for target in weapon_targets {
        let metadata = target.metadata();
        let main = WeaponMainId::parse(target.internal_id()).expect("weapon internal id parses");
        assert_eq!(
            metadata.get("path_family").and_then(Value::as_str),
            Some(main.family().path_family()),
            "{} 缺少 path_family",
            target.id().as_str()
        );
        assert_eq!(
            metadata.get("family").and_then(Value::as_str),
            Some(main.family().as_str())
        );
        assert!(
            metadata.get("catalog_scope").is_none(),
            "{} 不应携带 catalog_scope",
            target.id().as_str()
        );
    }
}

/// 聚合查找必须能按精确 stable_id 解析 artifact 里的武器目标——reinstall/preview
/// 流程依赖 `find_replacement_target`，而不是只看得到列表。
#[test]
fn aggregate_catalog_resolves_artifact_weapon_target_ids() {
    let shard: Value = serde_json::from_str(include_str!(
        "../data/weapons/mhw-weapon-targets.one.v1.json"
    ))
    .expect("one shard artifact parses");
    let stable_id = shard["targets"][0]["stable_id"]
        .as_str()
        .expect("stable id present");
    let target_id = ReplacementTargetId::parse(stable_id).expect("stable id parses");

    let target = MhwReplacementCatalog
        .find_replacement_target(&target_id)
        .expect("weapon target resolves from aggregate catalog");
    assert_eq!(target.id().as_str(), stable_id);
    assert_eq!(target.target_type().as_str(), "weapon");

    // 未知武器 ID 仍然 fail closed。
    let unknown = ReplacementTargetId::parse(
        "mhw:weapon:0000000000000000000000000000000000000000000000000000000000000000",
    )
    .expect("unknown id parses");
    assert!(MhwReplacementCatalog
        .find_replacement_target(&unknown)
        .is_err());
}
