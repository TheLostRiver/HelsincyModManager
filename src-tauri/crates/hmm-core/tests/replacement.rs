use hmm_core::{
    GameId, LocalizedText, ModId, ProfileId, ReplacementBinding, ReplacementBindingId,
    ReplacementCatalog, ReplacementCatalogVersion, ReplacementError, ReplacementSourceId,
    ReplacementTarget, ReplacementTargetId, ReplacementTargetKind,
};
use serde_json::json;
use std::collections::BTreeMap;

fn localized_text() -> LocalizedText {
    LocalizedText::new(BTreeMap::from([
        ("en".to_owned(), "Fatalis Alpha +".to_owned()),
        ("zh_cn".to_owned(), "【精英·龙α】服装".to_owned()),
    ]))
    .expect("localized text")
}

fn target(id: &str, internal_id: &str) -> ReplacementTarget {
    ReplacementTarget::new(
        ReplacementTargetId::parse(id).expect("target id"),
        GameId::mhw(),
        ReplacementTargetKind::parse("armor").expect("target kind"),
        localized_text(),
        vec!["黑龙α".to_owned(), "Fatalis Alpha".to_owned()],
        internal_id,
        BTreeMap::from([
            ("is_full_body".to_owned(), json!(false)),
            ("monster".to_owned(), json!("fatalis")),
            (
                "parts".to_owned(),
                json!(["head", "body", "arms", "waist", "legs"]),
            ),
            ("path_family".to_owned(), json!("pl/f_equip")),
        ]),
    )
    .expect("replacement target")
}

#[test]
fn replacement_ids_reject_blank_and_trim_stable_values() {
    assert!(ReplacementTargetId::parse(" ").is_err());
    assert!(ReplacementBindingId::parse("\t").is_err());
    assert!(ReplacementSourceId::parse("\n").is_err());
    assert!(ReplacementCatalogVersion::parse("").is_err());

    let id = ReplacementTargetId::parse("  mhw:armor:fatalis-alpha  ").expect("target id");
    assert_eq!(id.as_str(), "mhw:armor:fatalis-alpha");

    let invalid_json = serde_json::from_str::<ReplacementTargetId>(r#"" ""#);
    assert!(invalid_json.is_err());
}

#[test]
fn replacement_target_round_trips_structured_opaque_metadata() {
    let target = target("mhw:armor:fatalis-alpha", "pl129_0000");

    let encoded = serde_json::to_string(&target).expect("serialize target");
    let decoded: ReplacementTarget = serde_json::from_str(&encoded).expect("deserialize target");

    assert_eq!(decoded, target);
    assert_eq!(decoded.id().as_str(), "mhw:armor:fatalis-alpha");
    assert_eq!(decoded.internal_id(), "pl129_0000");
    assert_eq!(
        decoded.display_name().get("zh_cn"),
        Some("【精英·龙α】服装")
    );
    assert_eq!(decoded.metadata().get("is_full_body"), Some(&json!(false)));
    assert_eq!(
        decoded.metadata().get("parts"),
        Some(&json!(["head", "body", "arms", "waist", "legs"]))
    );
}

#[test]
fn replacement_target_without_localized_aliases_serializes_without_the_key() {
    // 铠甲 catalog 的别名是不带语言的平表：不知道就不写，不伪造成空表；老 JSON 也照常读回。
    let target = target("mhw:armor:fatalis-alpha", "pl129_0000");
    assert_eq!(target.localized_aliases(), None);

    let value = serde_json::to_value(&target).expect("serialize target");
    assert!(value.get("localized_aliases").is_none());

    let decoded: ReplacementTarget =
        serde_json::from_value(value).expect("deserialize target without the key");
    assert_eq!(decoded.localized_aliases(), None);
}

#[test]
fn replacement_target_carries_localized_aliases_and_round_trips_them() {
    let localized = BTreeMap::from([
        ("en".to_owned(), vec!["Fatalis Alpha".to_owned()]),
        ("zh_cn".to_owned(), vec![" 黑龙α ".to_owned()]),
    ]);
    let target = target("mhw:armor:fatalis-alpha", "pl129_0000")
        .with_localized_aliases(localized)
        .expect("localized aliases");

    let expected = BTreeMap::from([
        ("en".to_owned(), vec!["Fatalis Alpha".to_owned()]),
        ("zh_cn".to_owned(), vec!["黑龙α".to_owned()]),
    ]);
    assert_eq!(target.localized_aliases(), Some(&expected));
    // 平表不受影响：检索语义不变。
    assert_eq!(target.aliases(), ["黑龙α", "Fatalis Alpha"]);

    let encoded = serde_json::to_string(&target).expect("serialize target");
    let decoded: ReplacementTarget = serde_json::from_str(&encoded).expect("deserialize target");
    assert_eq!(decoded, target);
    assert_eq!(decoded.localized_aliases(), Some(&expected));
}

#[test]
fn localized_aliases_fail_closed_on_unknown_locale_blank_alias_and_unsearchable_alias() {
    let base = || target("mhw:armor:fatalis-alpha", "pl129_0000");

    // locale 没有展示名：前端沿展示名 fallback 链取词，孤儿 locale 永远显示不出来。
    assert_eq!(
        base()
            .with_localized_aliases(BTreeMap::from([(
                "ja".to_owned(),
                vec!["黑龙α".to_owned()],
            )]))
            .expect_err("unknown locale"),
        ReplacementError::LocalizedAliasLocaleUnknown {
            locale: "ja".to_owned(),
        }
    );
    assert_eq!(
        base()
            .with_localized_aliases(BTreeMap::from([
                (" ".to_owned(), vec!["黑龙α".to_owned()],)
            ]))
            .expect_err("blank locale"),
        ReplacementError::EmptyLocale
    );
    assert_eq!(
        base()
            .with_localized_aliases(BTreeMap::from([("en".to_owned(), vec!["  ".to_owned()])]))
            .expect_err("blank alias"),
        ReplacementError::EmptyAlias
    );
    // 行内展示的名字必须能被搜索命中：不在平表里的别名拒绝。
    assert_eq!(
        base()
            .with_localized_aliases(BTreeMap::from([(
                "en".to_owned(),
                vec!["Black Fatalis Alpha".to_owned()],
            )]))
            .expect_err("unsearchable alias"),
        ReplacementError::LocalizedAliasNotSearchable {
            alias: "Black Fatalis Alpha".to_owned(),
        }
    );

    // 同一条规则也守住 JSON 入口。
    let mut value = serde_json::to_value(base()).expect("serialize target");
    value["localized_aliases"] = json!({ "en": ["Black Fatalis Alpha"] });
    assert!(serde_json::from_value::<ReplacementTarget>(value).is_err());
}

#[test]
fn replacement_binding_references_stable_source_and_target_ids() {
    let binding = ReplacementBinding::new(
        ReplacementBindingId::parse("binding-1").expect("binding id"),
        ModId::new("mod-1"),
        ProfileId::new("profile-1"),
        ReplacementSourceId::parse("mhw:armor:f_equip:pl121_0000").expect("source id"),
        ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target id"),
        42,
    )
    .expect("binding");

    let value = serde_json::to_value(&binding).expect("serialize binding");
    assert_eq!(value["source_id"], "mhw:armor:f_equip:pl121_0000");
    assert_eq!(value["target_id"], "mhw:armor:fatalis-alpha");
    assert_eq!(value["created_at_unix_millis"], 42);

    let decoded: ReplacementBinding = serde_json::from_value(value).expect("deserialize binding");
    assert_eq!(decoded, binding);
}

#[test]
fn replacement_catalog_is_versioned_and_rejects_duplicate_stable_ids() {
    let version = ReplacementCatalogVersion::parse("mhw-armor-v1").expect("version");
    let first = target("mhw:armor:fatalis-alpha", "pl129_0000");
    let duplicate = target("mhw:armor:fatalis-alpha", "pl999_9999");

    let result = ReplacementCatalog::new(version.clone(), GameId::mhw(), vec![first, duplicate]);
    assert!(result.is_err());

    let catalog = ReplacementCatalog::new(
        version,
        GameId::mhw(),
        vec![target("mhw:armor:fatalis-alpha", "pl129_0000")],
    )
    .expect("catalog");
    assert_eq!(catalog.version().as_str(), "mhw-armor-v1");
    assert_eq!(catalog.targets().len(), 1);
}
