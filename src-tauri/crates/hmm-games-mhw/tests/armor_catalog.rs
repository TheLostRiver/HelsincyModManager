use hmm_games_mhw::{normalize_armor_display_text, normalize_armor_search_text, MhwArmorCatalog};
use hmm_ports::ReplacementCatalogProvider;
use std::collections::BTreeSet;

#[test]
fn armor_catalog_is_versioned_and_uses_stable_hash_ids() {
    let provider = MhwArmorCatalog;
    let catalog = provider.replacement_catalog().expect("armor catalog");

    assert_eq!(catalog.version().as_str(), "mhw-armor-v3");
    assert_eq!(catalog.game_id().as_str(), "mhw");
    assert_eq!(catalog.targets().len(), 269);

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

    // v3 起全部 269 条覆盖中英日三语（v2 时 5 条活动/联动装缺 en/ja，已从 kiranico 补齐）。
    let with_three_locales = catalog
        .targets()
        .iter()
        .filter(|target| {
            ["zh_cn", "en", "ja"]
                .iter()
                .all(|locale| target.display_name().get(locale).is_some())
        })
        .count();
    assert_eq!(with_three_locales, 269, "中英日三语覆盖数量变了");
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
    assert_eq!(u2027.len(), 1);
    assert_eq!(u2027[0].internal_id(), "pl129_0000");
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
        assert_eq!(
            target
                .metadata()
                .get("path_family")
                .and_then(|value| value.as_str()),
            Some("pl/f_equip")
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
