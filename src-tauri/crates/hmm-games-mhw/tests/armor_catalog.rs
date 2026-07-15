use hmm_games_mhw::{normalize_armor_display_text, normalize_armor_search_text, MhwArmorCatalog};
use hmm_ports::ReplacementCatalogProvider;
use std::collections::BTreeSet;

#[test]
fn armor_catalog_is_versioned_and_contains_stable_seed_targets() {
    let provider = MhwArmorCatalog;
    let catalog = provider.replacement_catalog().expect("armor catalog");

    assert_eq!(catalog.version().as_str(), "mhw-armor-v1");
    assert_eq!(catalog.game_id().as_str(), "mhw");
    assert_eq!(catalog.targets().len(), 4);
    assert_eq!(
        catalog
            .targets()
            .iter()
            .map(|target| target.id().as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "mhw:armor:alatreon-alpha",
            "mhw:armor:fatalis-alpha",
            "mhw:armor:fatalis-beta",
            "mhw:armor:guardian-alpha",
        ])
    );
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
