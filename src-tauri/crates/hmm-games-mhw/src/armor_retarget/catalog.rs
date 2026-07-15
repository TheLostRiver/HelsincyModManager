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
    for field in ["monster", "rank", "variant"] {
        let normalized = metadata
            .get(field)
            .and_then(Value::as_str)
            .map(normalize_armor_search_text)
            .ok_or(ReplacementCatalogError::CatalogInvalid)?;
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

fn validate_armor_metadata(metadata: &BTreeMap<String, Value>) -> ReplacementCatalogResult<&str> {
    let path_family = metadata_text(metadata, "path_family")?;
    if path_family != "pl/f_equip" {
        return Err(ReplacementCatalogError::CatalogInvalid);
    }
    for required in ["monster", "rank", "variant"] {
        metadata_text(metadata, required)?;
    }
    if metadata
        .get("is_full_body")
        .and_then(Value::as_bool)
        .is_none()
    {
        return Err(ReplacementCatalogError::CatalogInvalid);
    }

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
