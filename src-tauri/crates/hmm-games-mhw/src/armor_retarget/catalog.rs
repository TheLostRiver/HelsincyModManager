use super::ArmorEquipFamily;
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

/// 防具 catalog 按 `path_family` 拆成两份分片（`#356`），合并规则见 `parse_armor_catalog_shards`。
///
/// 529 条的单文件是 310KB / 12024 行，超出 policy 的体积硬限（256KB / 10000 行）。分片键取
/// `path_family` 是因为它本来就是领域边界——跨变体重定向由 `retarget.rs` 拒绝，与武器侧按
/// family 分片（`weapon_retarget/replacement.rs`）同一个道理。
///
/// **这份清单必须与 `data/armor/` 下的文件一一对应：少一份分片等于那一套模型的重定向目标
/// 整体消失。** 那不是解析错误，而是「我的角色性别下什么防具都改不了」——正是 `#356` 修掉的
/// 症状。分片文件缺失会编译失败，但「文件在磁盘上、没登记进这个数组」不会，
/// 由 `tests/armor_catalog.rs` 的分片覆盖回归钉住。
const ARMOR_CATALOG_SHARDS: [&str; 2] = [
    include_str!("../../data/armor/mhw-armor-targets.f_equip.v1.json"),
    include_str!("../../data/armor/mhw-armor-targets.m_equip.v1.json"),
];

/// 防具目标的 `path_family` 白名单从 [`ArmorEquipFamily`] 派生，不在这里再抄一份清单
/// （`#356`：同一个假设抄在多处、改一处漏一处，正是那个 issue 的成因）。
///
/// 两个变体都必须是合法目标：此前只放行 `pl/f_equip`，导致男角玩家改任何防具外观都会被
/// 装到游戏不读的路径去，而且装完不报错。
fn is_supported_armor_path_family(path_family: &str) -> bool {
    ArmorEquipFamily::from_path_family(path_family).is_some()
}

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
        parse_armor_catalog_shards(&ARMOR_CATALOG_SHARDS)
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

/// 单文件入口，只给单元测试用（生产路径是 `ARMOR_CATALOG_SHARDS`）。
///
/// 它就是「一份分片」的分片解析，不是另一条实现——所以用它写的校验用例与生产加载走的是
/// 同一段代码，不存在「测试测了一条产品不走的路」。
#[cfg(test)]
fn parse_armor_catalog(source: &str) -> ReplacementCatalogResult<ReplacementCatalog> {
    parse_armor_catalog_shards(&[source])
}

/// 把分片合并成一份再走**同一条**校验路径。
///
/// 关键：**不能**各自解析完再拼 target 列表。`(path_family, internal_id)` 唯一性是在单次
/// 校验里累积判定的（`scoped_internal_ids`），逐份校验会把它降级成「每个分片内部唯一」——
/// 两份各自合法、合起来撞车的分片就会被放行。合并后单次校验则一条不漏。
///
/// 每份分片各带一份 `schema_version` / `catalog_version` / `game_id` 信封，三者都要核：
/// schema 逐份对照支持版本，另两项跨分片必须一致，否则合出来的是个拼接怪物。
fn parse_armor_catalog_shards(sources: &[&str]) -> ReplacementCatalogResult<ReplacementCatalog> {
    let mut merged: Option<RawArmorCatalog> = None;
    for source in sources {
        let envelope: RawArmorCatalogEnvelope =
            serde_json::from_str(source).map_err(|_| ReplacementCatalogError::CatalogInvalid)?;

        if envelope.schema_version != MHW_ARMOR_CATALOG_SCHEMA_VERSION {
            return Err(ReplacementCatalogError::UnsupportedSchemaVersion {
                schema_version: envelope.schema_version,
            });
        }

        let raw: RawArmorCatalog =
            serde_json::from_str(source).map_err(|_| ReplacementCatalogError::CatalogInvalid)?;

        match merged.as_mut() {
            None => merged = Some(raw),
            Some(base) => {
                if base.catalog_version != raw.catalog_version || base.game_id != raw.game_id {
                    return Err(ReplacementCatalogError::CatalogInvalid);
                }
                base.targets.extend(raw.targets);
            }
        }
    }

    validate_armor_catalog(merged.ok_or(ReplacementCatalogError::CatalogInvalid)?)
}

fn validate_armor_catalog(raw: RawArmorCatalog) -> ReplacementCatalogResult<ReplacementCatalog> {
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
    if !is_supported_armor_path_family(path_family) {
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
        // 逐份分片扫原始 JSON：语言键集是数据层事实，合并后再看会分不清是哪份分片缺的。
        for shard in ARMOR_CATALOG_SHARDS {
            let raw: Value = serde_json::from_str(shard).expect("bundled armor catalog shard json");
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

    /// 分片合并必须真的合并同一件装备的两套模型，而不是「两份互不相干的 catalog」。
    ///
    /// 这是真实数据的形状：`pl001_0000` 在两份分片里各占一条，**且逐字同名**（同一件装备的
    /// 两套模型不该被迫叫两个名字，唯一性按 `path_family` 分组，见治理文档）。正向用例单列，
    /// 否则下面那些拒绝用例可以被「什么都拒」满足。
    #[test]
    fn merges_the_same_slot_from_both_model_variant_shards() {
        let mut female = valid_target("mhw:armor:shared-female", "pl001_0000");
        female["metadata"]["path_family"] = json!("pl/f_equip");
        let mut male = valid_target("mhw:armor:shared-male", "pl001_0000");
        male["metadata"]["path_family"] = json!("pl/m_equip");

        let catalog = parse_armor_catalog_shards(&[
            &catalog_source(vec![female]),
            &catalog_source(vec![male]),
        ])
        .expect("两套模型的分片必须能合并");

        assert_eq!(catalog.targets().len(), 2);
        let mut families: Vec<_> = catalog
            .targets()
            .iter()
            .filter_map(|target| target.metadata().get("path_family")?.as_str())
            .collect();
        families.sort_unstable();
        assert_eq!(families, ["pl/f_equip", "pl/m_equip"]);
    }

    /// 合并不得削弱任何一道校验。
    ///
    /// 单文件时 `(path_family, internal_id)` 唯一性是在单次校验里累积判定的。如果
    /// `parse_armor_catalog_shards` 只是逐份解析再拼列表，这个保证会悄悄降级成
    /// 「每个分片内部唯一」——下面每条都是「单独合法、合并冲突」，必须全部被拒。
    #[test]
    fn sharded_parse_still_rejects_conflicts_that_span_shards() {
        let shard = catalog_source(vec![valid_target("mhw:armor:first", "pl999_0000")]);

        // 同一个 (path_family, internal_id) 出现在两份分片里。
        let duplicate = catalog_source(vec![valid_target("mhw:armor:second", "pl999_0000")]);
        assert_eq!(
            parse_armor_catalog_shards(&[&shard, &duplicate]),
            Err(ReplacementCatalogError::CatalogInvalid),
            "跨分片的同槽位重复必须被拒"
        );

        // 分片属于不同版本的 catalog：合出来的是个拼接怪物。
        let other_version = catalog_source(vec![valid_target("mhw:armor:other", "pl998_0000")])
            .replace("test-v1", "test-v2");
        assert_eq!(
            parse_armor_catalog_shards(&[&shard, &other_version]),
            Err(ReplacementCatalogError::CatalogInvalid),
            "catalog_version 不一致的分片必须被拒"
        );

        // 分片属于另一个游戏。
        let other_game = catalog_source(vec![valid_target("mhw:armor:alien", "pl997_0000")])
            .replace("\"mhw\"", "\"mhr\"");
        assert_eq!(
            parse_armor_catalog_shards(&[&shard, &other_game]),
            Err(ReplacementCatalogError::CatalogInvalid),
            "game_id 不一致的分片必须被拒"
        );

        // 一份分片都没有：宁可报错也不能产出一份空 catalog。
        assert_eq!(
            parse_armor_catalog_shards(&[]),
            Err(ReplacementCatalogError::CatalogInvalid),
            "空分片列表必须被拒"
        );
    }
}
