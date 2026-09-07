use hmm_games_mhw::{normalize_armor_display_text, normalize_armor_search_text, MhwArmorCatalog};
use hmm_ports::ReplacementCatalogProvider;
use std::collections::BTreeSet;

#[test]
fn armor_catalog_is_versioned_and_uses_stable_hash_ids() {
    let provider = MhwArmorCatalog;
    let catalog = provider.replacement_catalog().expect("armor catalog");

    assert_eq!(catalog.version().as_str(), "mhw-armor-v4");
    assert_eq!(catalog.game_id().as_str(), "mhw");
    /*
     * `#356`：269 → 529。每件装备按它**实际存在的模型变体**产出目标，不再假设所有装备
     * 都有女性模型。实测游戏本体 272 个槽位（`equipment.json` 覆盖其中 269 个）：
     * 260 个两套模型都有 ⇒ 各出 2 条，4 个只有女性模型、5 个只有男性模型 ⇒ 各出 1 条。
     * 260×2 + 4 + 5 = 529。
     */
    assert_eq!(catalog.targets().len(), 529);

    let by_family = |family: &str| {
        catalog
            .targets()
            .iter()
            .filter(|target| {
                target
                    .metadata()
                    .get("path_family")
                    .and_then(|value| value.as_str())
                    == Some(family)
            })
            .count()
    };
    assert_eq!(by_family("pl/f_equip"), 264, "260 共享 + 4 仅女性模型");
    assert_eq!(by_family("pl/m_equip"), 265, "260 共享 + 5 仅男性模型");

    /*
     * 逐条钉住单模型的联动装（`#356`）。
     *
     * 光有计数防不住「哪一条被标错」——修复前正是这 5 条被标成 `pl/f_equip`，玩家选中
     * 「杰洛特」会被装到 `nativePC/pl/f_equip/pl118_0000/`，而游戏里那个路径不存在：
     * 安装成功、无效果、无诊断线索。名称本身自证性别（隆／杰洛特／巴耶克／里昂是男性
     * 角色，燕尾蝶**男**贝塔名字里就带「男」）。
     */
    let families_of = |internal_id: &str| {
        let mut found: Vec<_> = catalog
            .targets()
            .iter()
            .filter(|target| target.internal_id() == internal_id)
            .filter_map(|target| target.metadata().get("path_family")?.as_str())
            .collect();
        found.sort_unstable();
        found
    };

    for internal_id in [
        "pl057_0010", // 燕尾蝶男贝塔
        "pl069_0000", // 隆
        "pl118_0000", // 杰洛特
        "pl120_0000", // 巴耶克
        "pl130_0000", // 里昂
    ] {
        assert_eq!(
            families_of(internal_id),
            vec!["pl/m_equip"],
            "{internal_id} 只有男性模型，不得出现 f_equip 目标"
        );
    }

    for internal_id in [
        "pl070_0000",
        "pl119_0000",
        "pl131_0000", // 克莱尔
        "pl132_0010", // 精英·阿尔忒弥斯阿尔法
    ] {
        assert_eq!(
            families_of(internal_id),
            vec!["pl/f_equip"],
            "{internal_id} 只有女性模型，不得出现 m_equip 目标"
        );
    }

    // 常态：两套模型都有的装备各出一条，且互不重复。
    assert_eq!(families_of("pl001_0000"), vec!["pl/f_equip", "pl/m_equip"]);

    // AR6 之后全部使用 64 位 hex stable ID，不再有人类 slug——
    // slug 会把不同路径压成同一 ID，见 EQUIPMENT_CATALOG_GOVERNANCE.md。
    for target in catalog.targets() {
        let slug = target
            .id()
            .as_str()
            .strip_prefix("mhw:armor:")
            .expect("armor target id prefix");
        assert!(
            slug.len() == 64
                && slug
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()),
            "{} 不是小写 64 位 hex stable ID",
            target.id().as_str()
        );
    }

    // internal_id 必须唯一：它是改写目标槽位的实际依据。
    assert_eq!(
        catalog
            .targets()
            .iter()
            .map(|target| target.internal_id())
            .collect::<BTreeSet<_>>()
            .len(),
        269
    );
}

/// 磁盘上的每一份分片都必须被编译进 catalog（`#356`）。
///
/// `ARMOR_CATALOG_SHARDS` 登记了不存在的文件会编译失败，反过来不会：**新增一份分片却忘了
/// 登记，编译、解析、加载全部正常，只是那批目标整体消失**——玩家侧的症状是「这些防具在列表
/// 里根本搜不到」，没有任何报错，正是 `#356` 修的那类失效。
///
/// 所以这里拿目录的实际内容与加载结果对账，而不是断言一个写死的分片数：数据扩容加分片时，
/// 这条测试不用跟着改就继续承重。整套模型缺失另有 `by_family` 的计数断言兜着。
#[test]
fn every_armor_catalog_shard_on_disk_is_compiled_into_the_catalog() {
    let shard_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/armor");
    let mut shard_files: Vec<_> = std::fs::read_dir(&shard_dir)
        .unwrap_or_else(|error| panic!("读不到分片目录 {}: {error}", shard_dir.display()))
        .map(|entry| entry.expect("分片目录项").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    shard_files.sort();
    assert!(
        shard_files.len() >= 2,
        "分片目录里只有 {} 个文件，拆分本身没生效",
        shard_files.len()
    );

    let mut on_disk = BTreeSet::new();
    for path in &shard_files {
        let text = std::fs::read_to_string(path).expect("分片内容");
        let raw: serde_json::Value = serde_json::from_str(&text).expect("分片 JSON");
        let targets = raw["targets"].as_array().expect("targets array");
        assert!(!targets.is_empty(), "{} 是一份空分片", path.display());
        for target in targets {
            on_disk.insert(target["id"].as_str().expect("target id").to_owned());
        }
    }

    let catalog = MhwArmorCatalog
        .replacement_catalog()
        .expect("armor catalog");
    let loaded: BTreeSet<_> = catalog
        .targets()
        .iter()
        .map(|target| target.id().as_str().to_owned())
        .collect();

    let unregistered: Vec<_> = on_disk.difference(&loaded).collect();
    let phantom: Vec<_> = loaded.difference(&on_disk).collect();
    assert!(
        unregistered.is_empty() && phantom.is_empty(),
        "分片与 catalog 不一致：磁盘上有却没编译进来 {unregistered:?}；编译进来磁盘上却没有 {phantom:?}"
    );
}

#[test]
fn armor_catalog_keeps_the_original_seed_slots_and_gains_three_locales() {
    let provider = MhwArmorCatalog;
    let catalog = provider.replacement_catalog().expect("armor catalog");

    // AR1 的四个槽位在扩容后必须仍然在（旧 ID 的解析另见 legacy 回归）。
    for internal_id in ["pl121_0000", "pl129_0000", "pl129_0010", "pl052_0000"] {
        assert!(
            catalog
                .targets()
                .iter()
                .any(|target| target.internal_id() == internal_id),
            "扩容后丢失了原有槽位 {internal_id}"
        );
    }

    // v4 起 528/529 条覆盖中英日三语展示名。
    // 唯一例外 pl057_0010：官方英文名与 pl019_0000 重名，按治理规则 en 走 alias，
    // 键集细节由 catalog 单元测试锁定。它只有男性模型，所以只占 1 条。
    let with_three_locales = catalog
        .targets()
        .iter()
        .filter(|target| {
            ["zh_cn", "en", "ja"]
                .iter()
                .all(|locale| target.display_name().get(locale).is_some())
        })
        .count();
    assert_eq!(with_three_locales, 528, "中英日三语覆盖数量变了");
}

#[test]
fn armor_catalog_normalizes_nfc_middle_dots_width_and_case() {
    assert_eq!(normalize_armor_display_text("Cafe\u{301}‧龙"), "Café·龙");
    assert_eq!(normalize_armor_display_text("精英・龙"), "精英·龙");
    assert_eq!(normalize_armor_display_text("精英･龙"), "精英·龙");
    assert_eq!(normalize_armor_search_text("  ＦＡＴＡＬＩＳ  "), "fatalis");

    let provider = MhwArmorCatalog;
    let u2027 = provider
        .search_replacement_targets("【精英‧龙α】服装")
        .expect("U+2027 search");
    let u00b7 = provider
        .search_replacement_targets("【精英·龙α】服装")
        .expect("U+00B7 search");

    assert_eq!(u2027, u00b7);
    /*
     * `#356`：同一件装备的两套模型是两条目标，**同名**，所以 catalog 层的搜索命中翻倍。
     *
     * 这不会让玩家看到两条无法区分的结果：`list_compatible_targets` 按源包的
     * `path_family` 筛过，他一次只看得到自己那个变体。治理文档的 display_name 唯一性
     * 也据此收敛到「同一 path_family 内唯一」。
     */
    assert_eq!(u2027.len(), 2);
    assert!(u2027
        .iter()
        .all(|target| target.internal_id() == "pl129_0000"));
    let families: Vec<_> = u2027
        .iter()
        .filter_map(|target| target.metadata().get("path_family")?.as_str())
        .collect();
    assert_eq!(families.len(), 2, "两条命中必须各带 path_family");
    assert_ne!(families[0], families[1], "两条命中是不同的模型变体");
}

#[test]
fn official_english_duplicate_name_stays_searchable_for_both_butterfly_models() {
    // pl057_0010（男版燕尾蝶）的官方英文名与女版逐字同为 "Butterfly β"，
    // 按治理规则走 alias。这里用公开搜索 API 锁定可达性：alias 或搜索归一化
    // 逻辑回归时，英文检索会静默丢失男版条目——原始 JSON 断言拦不住这种回归。
    let provider = MhwArmorCatalog;
    let hits = provider
        .search_replacement_targets("Butterfly β")
        .expect("english duplicate-name search");

    let internal_ids: BTreeSet<_> = hits
        .iter()
        .map(|target| target.internal_id().to_owned())
        .collect();
    assert!(
        internal_ids.contains("pl019_0000") && internal_ids.contains("pl057_0010"),
        "英文官方名必须同时命中女版与男版燕尾蝶，实际命中: {internal_ids:?}"
    );
}

#[test]
fn armor_catalog_search_distinguishes_fatalis_from_alatreon() {
    let provider = MhwArmorCatalog;

    let fatalis = provider
        .search_replacement_targets("黑龙")
        .expect("fatalis search");
    let alatreon = provider
        .search_replacement_targets("煌黑龙")
        .expect("alatreon search");

    assert!(!fatalis.is_empty());
    assert!(fatalis.iter().all(|target| {
        target
            .metadata()
            .get("monster")
            .and_then(|value| value.as_str())
            == Some("fatalis")
    }));
    assert!(!alatreon.is_empty());
    assert!(alatreon.iter().all(|target| {
        target
            .metadata()
            .get("monster")
            .and_then(|value| value.as_str())
            == Some("alatreon")
    }));
}

#[test]
fn armor_catalog_validates_mhw_internal_ids_and_path_family_in_adapter() {
    let provider = MhwArmorCatalog;
    let catalog = provider.replacement_catalog().expect("armor catalog");

    for target in catalog.targets() {
        let internal_id = target.internal_id().as_bytes();
        assert_eq!(internal_id.len(), 10);
        assert_eq!(&internal_id[..2], b"pl");
        assert!(internal_id[2..5].iter().all(u8::is_ascii_digit));
        assert_eq!(internal_id[5], b'_');
        assert!(internal_id[6..].iter().all(u8::is_ascii_digit));
        // `#356`：两套模型都是合法的 path_family，adapter 侧按源包的变体匹配。
        let path_family = target
            .metadata()
            .get("path_family")
            .and_then(|value| value.as_str());
        assert!(
            matches!(path_family, Some("pl/f_equip") | Some("pl/m_equip")),
            "非法 path_family: {path_family:?}"
        );
    }
}

#[test]
fn legacy_binding_ids_still_resolve_after_catalog_expansion() {
    // AR6 把 catalog 从四条手工 slug ID 扩到全量 hash stable ID。
    // 玩家已安装的 manifest / binding snapshot 里存的是旧 slug——
    // 解析不了就等于碰坏他们已有的安装，所以这条回归必须一直绿。
    let provider = MhwArmorCatalog;
    let catalog = provider.replacement_catalog().expect("armor catalog");

    for target in catalog.targets() {
        let legacy_ids = target
            .metadata()
            .get("legacy_ids")
            .and_then(serde_json::Value::as_array)
            .map(|ids| {
                ids.iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for legacy_id in legacy_ids {
            let parsed = hmm_core::ReplacementTargetId::parse(legacy_id)
                .expect("legacy id should be a parseable target id");
            let resolved = provider
                .find_replacement_target(&parsed)
                .unwrap_or_else(|error| panic!("旧绑定 {legacy_id} 解析失败: {error:?}"));
            assert_eq!(
                resolved.internal_id(),
                target.internal_id(),
                "旧绑定 {legacy_id} 必须解析回同一个槽位"
            );
        }
    }
}

#[test]
fn unknown_target_ids_still_fail_closed() {
    // 回落不能变成"什么都能解析"：未知 ID 必须继续报 TargetNotFound。
    let provider = MhwArmorCatalog;
    let unknown = hmm_core::ReplacementTargetId::parse("mhw:armor:does-not-exist")
        .expect("parseable target id");

    assert!(provider.find_replacement_target(&unknown).is_err());
}
