use hmm_games_mhw::{
    generate_mhw_equipment_stable_id, EquipmentCandidateTargetKind as Kind, KinsectId,
    KinsectResourceRoot, WeaponFamily, WeaponResourceRoot,
};

#[test]
fn kinsects_have_a_distinct_identity_without_changing_weapon_families() {
    assert_eq!(WeaponFamily::ALL.len(), 14);
    assert!(WeaponFamily::parse("mus").is_err());
    assert!(WeaponResourceRoot::parse("nativePC/wp/mus/mus001").is_err());
    assert_eq!(
        generate_mhw_equipment_stable_id(Kind::Weapon, "wp/one", "nativePC/wp/one/one002").unwrap(),
        "mhw:weapon:0784b06e3b1e031bee9d1da31deeb995cba0d35dca4f7583f1cd8a019c5facc1"
    );
    let root = KinsectResourceRoot::parse("nativePC/wp/mus/mus029").unwrap();
    assert_eq!(root.id().as_str(), "mus029");
    assert_eq!(root.id().number(), 29);
    assert_eq!(root.path_family(), "wp/mus");
    assert!(generate_mhw_equipment_stable_id(
        Kind::Kinsect,
        root.path_family(),
        root.normalized_path().as_str()
    )
    .unwrap()
    .starts_with("mhw:kinsect:"));
    assert!(
        generate_mhw_equipment_stable_id(Kind::Weapon, "wp/mus", "nativePC/wp/mus/mus001").is_err()
    );
}

#[test]
fn paths_require_a_complete_kinsect_identity_and_a_safe_resource_tail() {
    for id in [
        "mus1",
        "mus0001",
        "bs_mus001",
        "rod001",
        "mus０１",
        "MUS001",
        "mus001x",
    ] {
        assert!(KinsectId::parse(id).is_err(), "{id}");
    }
    for path in [
        "nativePC/wp/rod/mus001",
        "nativePC/wp/mus/rod001",
        "nativePC/wp/mus/mus001/mod",
        "nativePC/wp/mus/../mus001",
        "D:/nativePC/wp/mus/mus001",
        "/nativePC/wp/mus/mus001",
    ] {
        assert!(KinsectResourceRoot::parse(path).is_err(), "{path}");
        assert!(
            generate_mhw_equipment_stable_id(Kind::Kinsect, "wp/mus", path).is_err(),
            "{path}"
        );
    }
    assert!(
        KinsectResourceRoot::of_resource_path("nativePC/wp/mus/mus001/mod/mus001.mod3").is_some()
    );
    assert!(
        KinsectResourceRoot::of_resource_path("nativePC/wp/mus/mus001/../outside.mod3").is_none()
    );
    assert!(KinsectResourceRoot::of_resource_path("nativePC/wp/mus/mus001").is_none());
    // 未登记编号只能表示作者原始资源；可选目标另由正式 catalog 限定。
    assert!(KinsectId::parse("mus999").is_ok());
}
