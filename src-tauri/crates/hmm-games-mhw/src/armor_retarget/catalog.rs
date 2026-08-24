use hmm_core::{
    GameId, LocalizedText, ReplacementCatalog, ReplacementCatalogVersion, ReplacementTarget,
    ReplacementTargetId, ReplacementTargetKind,
};
use hmm_ports::{ReplacementCatalogError, ReplacementCatalogProvider, ReplacementCatalogResult};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use unicode_normalization::UnicodeNormalization;

const MHW_ARMOR_CATALOG_SCHEMA_VERSION: u32 = 1;
const BUNDLED_ARMOR_CATALOG: &str = include_str!("../../data/mhw-armor-targets.v1.json");

#[derive(Debug, Clone, Copy, Default)]
pub struct MhwArmorCatalog;

#[derive(Debug, Deserialize)]
struct RawArmorCatalogEnvelope {
    schema_version: u32,
}

#[derive(Debug, Deserialize)]
struct RawArmorCatalog {
    catalog_version: String,
    game_id: String,
    targets: Vec<RawArmorTarget>,
}

#[derive(Debug, Deserialize)]
struct RawArmorTarget {
    id: String,
    target_type: String,
    display_name: BTreeMap<String, String>,
    aliases: Vec<String>,
    internal_id: String,
    metadata: BTreeMap<String, Value>,
}

impl ReplacementCatalogProvider for MhwArmorCatalog {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn replacement_catalog(&self) -> ReplacementCatalogResult<ReplacementCatalog> {
        parse_armor_catalog(BUNDLED_ARMOR_CATALOG)
    }

    fn find_replacement_target(
        &self,
        target_id: &ReplacementTargetId,
    ) -> ReplacementCatalogResult<ReplacementTarget> {
        resolve_target_allowing_legacy_ids(&self.replacement_catalog()?, target_id)
    }

    fn search_replacement_targets(
        &self,
        query: &str,
    ) -> ReplacementCatalogResult<Vec<ReplacementTarget>> {
        let query = normalize_armor_search_text(query);
        if query.is_empty() {
            return Ok(Vec::new());
        }

        Ok(self
            .replacement_catalog()?
            .targets()
            .iter()
            .filter(|target| target_matches_query(target, &query))
            .cloned()
            .collect())
    }
}

pub fn normalize_armor_display_text(value: &str) -> String {
    value.nfc().map(normalize_middle_dot).collect::<String>()
}

pub fn normalize_armor_search_text(value: &str) -> String {
    let normalized = value
        .nfkc()
        .map(normalize_middle_dot)
        .flat_map(char::to_lowercase)
        .collect::<String>();

    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_middle_dot(value: char) -> char {
    match value {
        '\u{2027}' | '\u{00b7}' | '\u{30fb}' | '\u{ff65}' => '·',
        value => value,
    }
}

fn parse_armor_catalog(source: &str) -> ReplacementCatalogResult<ReplacementCatalog> {
    let envelope: RawArmorCatalogEnvelope =
        serde_json::from_str(source).map_err(|_| ReplacementCatalogError::CatalogInvalid)?;

    if envelope.schema_version != MHW_ARMOR_CATALOG_SCHEMA_VERSION {
        return Err(ReplacementCatalogError::UnsupportedSchemaVersion {
            schema_version: envelope.schema_version,
        });
    }

    let raw: RawArmorCatalog =
        serde_json::from_str(source).map_err(|_| ReplacementCatalogError::CatalogInvalid)?;

    let game_id =
        GameId::parse(raw.game_id).map_err(|_| ReplacementCatalogError::CatalogInvalid)?;
    if game_id != GameId::mhw() {
        return Err(ReplacementCatalogError::CatalogInvalid);
    }

    let version = ReplacementCatalogVersion::parse(raw.catalog_version)
        .map_err(|_| ReplacementCatalogError::CatalogInvalid)?;
    let mut scoped_internal_ids = BTreeSet::new();
    let targets = raw
        .targets
        .into_iter()
        .map(|raw_target| build_target(raw_target, &game_id, &mut scoped_internal_ids))
        .collect::<ReplacementCatalogResult<Vec<_>>>()?;

    ReplacementCatalog::new(version, game_id, targets)
        .map_err(|_| ReplacementCatalogError::CatalogInvalid)
}

fn build_target(
    raw: RawArmorTarget,
    game_id: &GameId,
    scoped_internal_ids: &mut BTreeSet<(String, String)>,
) -> ReplacementCatalogResult<ReplacementTarget> {
    let has_stable_slug = raw
        .id
        .strip_prefix("mhw:armor:")
        .is_some_and(|slug| !slug.trim().is_empty());
    if !has_stable_slug
        || raw.target_type != "armor"
        || !is_valid_armor_internal_id(&raw.internal_id)
    {
        return Err(ReplacementCatalogError::CatalogInvalid);
    }

    let path_family = validate_armor_metadata(&raw.metadata)?;

    if !scoped_internal_ids.insert((path_family.to_owned(), raw.internal_id.clone())) {
        return Err(ReplacementCatalogError::CatalogInvalid);
    }

    let display_name = raw
        .display_name
        .into_iter()
        .map(|(locale, text)| (locale, normalize_armor_display_text(&text)))
        .collect();
    let aliases = raw
        .aliases
        .into_iter()
        .map(|alias| normalize_armor_display_text(&alias))
        .collect();
    let mut metadata = raw.metadata;
    // 这三个字段可选（见 validate_armor_metadata）；出现时仍然归一化，
    // 保证 monster 进搜索词时与 display name 走同一套比较规则。
    for field in ["monster", "rank", "variant"] {
        let Some(normalized) = metadata
            .get(field)
            .and_then(Value::as_str)
            .map(normalize_armor_search_text)
        else {
            continue;
        };
        metadata.insert(field.to_owned(), Value::String(normalized));
    }

    ReplacementTarget::new(
        ReplacementTargetId::parse(raw.id).map_err(|_| ReplacementCatalogError::CatalogInvalid)?,
        game_id.clone(),
        ReplacementTargetKind::parse(raw.target_type)
            .map_err(|_| ReplacementCatalogError::CatalogInvalid)?,
        LocalizedText::new(display_name).map_err(|_| ReplacementCatalogError::CatalogInvalid)?,
        aliases,
        raw.internal_id,
        metadata,
    )
    .map_err(|_| ReplacementCatalogError::CatalogInvalid)
}

/// 按 target ID 查找，找不到时回落到 `metadata.legacy_ids`。
///
/// AR6 把 catalog 从四条手工 slug ID 扩到全量 hash stable ID。玩家**已安装**的
/// manifest 与 binding snapshot 里存的是旧 slug（如 `mhw:armor:fatalis-alpha`），
/// 不做这层回落，升级后这些绑定会直接指向不存在的目标——等于碰坏玩家已有安装。
///
/// 回落只读 metadata，且只在游戏适配器里做：`hmm-core` 不对 metadata 内字段值
/// 做分支判断（见 docs/ARMOR_RETARGET_DESIGN.md 的核心层边界）。
///
/// `legacy_ids` 不是新的 stable identity，只用于解析旧绑定；
/// 治理契约见 docs/EQUIPMENT_CATALOG_GOVERNANCE.md。
pub(crate) fn resolve_target_allowing_legacy_ids(
    catalog: &ReplacementCatalog,
    target_id: &ReplacementTargetId,
) -> ReplacementCatalogResult<ReplacementTarget> {
    if let Some(target) = catalog.find(target_id) {
        return Ok(target.clone());
    }

    catalog
        .targets()
        .iter()
        .find(|target| {
            target
                .metadata()
                .get("legacy_ids")
                .and_then(Value::as_array)
                .is_some_and(|legacy| {
                    legacy
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|legacy_id| legacy_id == target_id.as_str())
                })
        })
        .cloned()
        .ok_or_else(|| ReplacementCatalogError::TargetNotFound {
            target_id: target_id.clone(),
        })
}

fn metadata_text<'a>(
    metadata: &'a BTreeMap<String, Value>,
    field: &str,
) -> ReplacementCatalogResult<&'a str> {
    metadata
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or(ReplacementCatalogError::CatalogInvalid)
}

/// `path_family` 是唯一必填项——它参与 source/target 家族匹配，缺了会改变改写行为。
///
/// `monster` / `rank` / `variant` / `is_full_body` / `parts` 改为可选：
/// 这套必填要求是给 AR1 的四条手工条目设计的，扩容到全量防具后无法逐条诚实推导
/// （「【皮制】服装」推不出怪物）。而除 `monster` 会进搜索词外（见 target_terms），
/// 其余四个字段全仓库只被本函数校验、没有任何消费者。
///
/// 关键取舍：可选不等于不校验。字段**出现**时形状仍然必须正确，
/// 否则错误数据会静默混进 catalog——这正是留着必填想防的事。
fn validate_armor_metadata(metadata: &BTreeMap<String, Value>) -> ReplacementCatalogResult<&str> {
    let path_family = metadata_text(metadata, "path_family")?;
    if path_family != "pl/f_equip" {
        return Err(ReplacementCatalogError::CatalogInvalid);
    }
    for optional_text in ["monster", "rank", "variant"] {
        if metadata.contains_key(optional_text) {
            metadata_text(metadata, optional_text)?;
        }
    }
    if metadata.contains_key("is_full_body")
        && metadata
            .get("is_full_body")
            .and_then(Value::as_bool)
            .is_none()
    {
        return Err(ReplacementCatalogError::CatalogInvalid);
    }

    // legacy_ids 决定旧绑定能不能解析，形状错了会静默失去回落能力。
    if metadata.contains_key("legacy_ids") {
        let legacy_ids = metadata
            .get("legacy_ids")
            .and_then(Value::as_array)
            .ok_or(ReplacementCatalogError::CatalogInvalid)?;
        if !legacy_ids
            .iter()
            .all(|id| id.as_str().is_some_and(|id| !id.trim().is_empty()))
        {
            return Err(ReplacementCatalogError::CatalogInvalid);
        }
    }

    if metadata.contains_key("parts") {
        let parts = metadata
            .get("parts")
            .and_then(Value::as_array)
            .filter(|parts| !parts.is_empty())
            .ok_or(ReplacementCatalogError::CatalogInvalid)?;
        if !parts
            .iter()
            .all(|part| part.as_str().is_some_and(|part| !part.trim().is_empty()))
        {
            return Err(ReplacementCatalogError::CatalogInvalid);
        }
    }

    Ok(path_family)
}

fn is_valid_armor_internal_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && &bytes[..2] == b"pl"
        && bytes[2..5].iter().all(u8::is_ascii_digit)
        && bytes[5] == b'_'
        && bytes[6..].iter().all(u8::is_ascii_digit)
}

fn target_matches_query(target: &ReplacementTarget, query: &str) -> bool {
    target_terms(target)
        .into_iter()
        .any(|term| normalize_armor_search_text(term) == query)
}

fn target_terms(target: &ReplacementTarget) -> Vec<&str> {
    let mut terms = vec![target.id().as_str(), target.internal_id()];
    terms.extend(target.display_name().values());
    terms.extend(target.aliases().iter().map(String::as_str));
    if let Some(monster) = target.metadata().get("monster").and_then(Value::as_str) {
        terms.push(monster);
    }
    terms
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bundled_armor_catalog_names_cover_all_mhw_locales() {
        // 键集即 per-game locale 能力声明（I18N-08）：少一种语言，对应语言界面
        // 会 fallback 显示其他语言，且该语言的名称检索不命中。扩容或修名时必须保持齐全
        // （AR6 曾漏 5 条 en/ja，本测试防止回归）。
        //
        // 唯一记录在案的例外：pl057_0010（男版燕尾蝶）的官方英文名与 pl019_0000（女版）
        // 逐字同为 "Butterfly β"。治理规则要求同 locale display_name 跨目标唯一、alias
        // 允许重复指向多目标，故男版 en 官方名走 alias（检索可达），display_name 不占用重名。
        let raw: Value =
            serde_json::from_str(BUNDLED_ARMOR_CATALOG).expect("bundled armor catalog json");
        for target in raw["targets"].as_array().expect("targets array") {
            let internal_id = target["internal_id"].as_str().expect("internal id");
            let names = target["display_name"]
                .as_object()
                .expect("display_name object");
            let mut keys: Vec<_> = names.keys().map(String::as_str).collect();
            keys.sort_unstable();
            if internal_id == "pl057_0010" {
                assert_eq!(keys, ["ja", "zh_cn"], "pl057_0010 keeps zh_cn/ja names");
                assert!(
                    target["aliases"]
                        .as_array()
                        .expect("aliases array")
                        .iter()
                        .any(|alias| alias == "Butterfly β"),
                    "pl057_0010 must keep its official English name searchable via alias"
                );
                continue;
            }
            assert_eq!(
                keys,
                ["en", "ja", "zh_cn"],
                "armor target {internal_id} must carry the full locale set"
            );
        }
    }

    #[test]
    fn rejects_unsupported_catalog_schema_version() {
        let error = parse_armor_catalog(r#"{"schema_version":99}"#)
            .expect_err("unsupported schema should not require v1 fields");

        assert_eq!(
            error,
            ReplacementCatalogError::UnsupportedSchemaVersion { schema_version: 99 }
        );
    }

    #[test]
    fn validates_mhw_armor_internal_id_shape_inside_adapter() {
        assert!(is_valid_armor_internal_id("pl129_0000"));
        assert!(!is_valid_armor_internal_id("weapon-129"));
        assert!(!is_valid_armor_internal_id("pl12_0000"));
        assert!(!is_valid_armor_internal_id("pl129-0000"));
    }

    #[test]
    fn resolves_legacy_ids_and_still_fails_closed_on_unknown_ids() {
        // AR6 扩容后旧绑定必须还能解析；同时回落不能退化成"什么都能解析"。
        let mut target = valid_target("mhw:armor:new-stable-id", "pl129_0000");
        target["metadata"]["legacy_ids"] = json!(["mhw:armor:fatalis-alpha"]);
        let catalog = parse_armor_catalog(&catalog_source(vec![target])).expect("catalog");

        let legacy = ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("legacy id");
        let resolved =
            resolve_target_allowing_legacy_ids(&catalog, &legacy).expect("legacy id must resolve");
        assert_eq!(resolved.id().as_str(), "mhw:armor:new-stable-id");
        assert_eq!(resolved.internal_id(), "pl129_0000");

        let current = ReplacementTargetId::parse("mhw:armor:new-stable-id").expect("current id");
        assert!(resolve_target_allowing_legacy_ids(&catalog, &current).is_ok());

        let unknown = ReplacementTargetId::parse("mhw:armor:nope").expect("unknown id");
        assert!(resolve_target_allowing_legacy_ids(&catalog, &unknown).is_err());
    }

    #[test]
    fn metadata_beyond_path_family_is_optional_but_still_shape_checked() {
        // 只留 path_family 应当通过（AR6 生成条目推不出 monster/rank）。
        let mut minimal = valid_target("mhw:armor:minimal", "pl130_0000");
        minimal["metadata"] = json!({ "path_family": "pl/f_equip" });
        assert!(parse_armor_catalog(&catalog_source(vec![minimal])).is_ok());

        // 但字段出现时形状错了仍须拒绝，否则等于取消了校验。
        for (field, bad) in [
            ("monster", json!("")),
            ("is_full_body", json!("false")),
            ("parts", json!([])),
            ("legacy_ids", json!("mhw:armor:fatalis-alpha")),
            ("legacy_ids", json!([""])),
        ] {
            let mut invalid = valid_target("mhw:armor:bad-shape", "pl131_0000");
            invalid["metadata"][field] = bad.clone();
            assert!(
                parse_armor_catalog(&catalog_source(vec![invalid])).is_err(),
                "{field} = {bad} 形状非法时必须拒绝"
            );
        }
    }

    fn valid_target(id: &str, internal_id: &str) -> Value {
        json!({
            "id": id,
            "target_type": "armor",
            "display_name": { "en": "Test Armor" },
            "aliases": ["Test Armor"],
            "internal_id": internal_id,
            "metadata": {
                "path_family": "pl/f_equip",
                "monster": "test",
                "rank": "master",
                "variant": "alpha",
                "is_full_body": false,
                "parts": ["head"]
            }
        })
    }

    fn catalog_source(targets: Vec<Value>) -> String {
        json!({
            "schema_version": 1,
            "catalog_version": "test-v1",
            "game_id": "mhw",
            "targets": targets
        })
        .to_string()
    }

    #[test]
    fn rejects_target_id_without_stable_slug() {
        let source = catalog_source(vec![valid_target("mhw:armor:", "pl999_0000")]);

        assert_eq!(
            parse_armor_catalog(&source),
            Err(ReplacementCatalogError::CatalogInvalid)
        );
    }

    #[test]
    fn rejects_each_invalid_structured_armor_metadata_field() {
        let cases = [
            ("is_full_body type", "is_full_body", json!("false")),
            ("parts type", "parts", json!("head")),
            ("empty parts", "parts", json!([])),
            ("blank part", "parts", json!(["head", " "])),
        ];

        for (case, field, invalid_value) in cases {
            let mut target = valid_target("mhw:armor:test", "pl999_0000");
            target["metadata"][field] = invalid_value;
            let source = catalog_source(vec![target]);

            assert_eq!(
                parse_armor_catalog(&source),
                Err(ReplacementCatalogError::CatalogInvalid),
                "case: {case}"
            );
        }
    }

    #[test]
    fn rejects_duplicate_internal_id_in_the_same_path_family() {
        let source = catalog_source(vec![
            valid_target("mhw:armor:first", "pl999_0000"),
            valid_target("mhw:armor:second", "pl999_0000"),
        ]);

        assert_eq!(
            parse_armor_catalog(&source),
            Err(ReplacementCatalogError::CatalogInvalid)
        );
    }
}
