use super::KinsectResourceRoot;
use crate::{
    generate_mhw_equipment_stable_id, normalize_armor_search_text, EquipmentCandidateTargetKind,
};
use hmm_core::{
    GameId, LocalizedText, ReplacementTarget, ReplacementTargetId, ReplacementTargetKind,
};
use hmm_ports::{ReplacementCatalogError, ReplacementCatalogResult};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn kinsect_targets() -> ReplacementCatalogResult<Vec<ReplacementTarget>> {
    parse_targets(include_str!("../../data/mhw-kinsect-targets.v1.json"))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogWire {
    schema_version: u32,
    catalog_version: String,
    game_id: String,
    targets: Vec<TargetWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetWire {
    stable_id: String,
    target_type: String,
    resource_path: String,
    internal_id: String,
    metadata: MetadataWire,
    status: String,
    names: BTreeMap<String, NamesWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataWire {
    path_family: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NamesWire {
    display_name: String,
    aliases: Vec<String>,
}

fn parse_targets(input: &str) -> ReplacementCatalogResult<Vec<ReplacementTarget>> {
    let invalid = || ReplacementCatalogError::CatalogInvalid;
    let wire: CatalogWire = serde_json::from_str(input).map_err(|_| invalid())?;
    if wire.schema_version != 1
        || wire.catalog_version != "mhw-kinsect-v1"
        || wire.game_id != "mhw"
        || wire.targets.is_empty()
    {
        return Err(invalid());
    }
    let mut ids = BTreeSet::new();
    let mut displays = BTreeSet::new();
    let mut result = Vec::new();
    for target in wire.targets {
        let root = KinsectResourceRoot::parse(&target.resource_path).map_err(|_| invalid())?;
        let stable = generate_mhw_equipment_stable_id(
            EquipmentCandidateTargetKind::Kinsect,
            root.path_family(),
            root.normalized_path().as_str(),
        )
        .map_err(|_| invalid())?;
        if target.target_type != "kinsect"
            || target.metadata.path_family != root.path_family()
            || target.resource_path != root.normalized_path().as_str()
            || target.internal_id != root.id().as_str()
            || target.stable_id != stable
            || !ids.insert(stable.clone())
            || !matches!(target.status.as_str(), "active" | "hidden")
            || target.names.keys().map(String::as_str).collect::<Vec<_>>() != ["en", "ja", "zh_cn"]
        {
            return Err(invalid());
        }
        let mut names = BTreeMap::new();
        let mut aliases = BTreeMap::new();
        for (locale, localized) in target.names {
            let name = normalize_armor_search_text(&localized.display_name);
            if name.is_empty() || !displays.insert((locale.clone(), name.clone())) {
                return Err(invalid());
            }
            let mut unique = BTreeSet::from([name]);
            for alias in &localized.aliases {
                let normalized = normalize_armor_search_text(alias);
                if normalized.is_empty() || !unique.insert(normalized) {
                    return Err(invalid());
                }
            }
            names.insert(locale.clone(), localized.display_name);
            aliases.insert(locale, localized.aliases);
        }
        let flat = aliases
            .values()
            .flatten()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let resolved = ReplacementTarget::new(
            ReplacementTargetId::parse(stable).map_err(|_| invalid())?,
            GameId::mhw(),
            ReplacementTargetKind::parse("kinsect").map_err(|_| invalid())?,
            LocalizedText::new(names).map_err(|_| invalid())?,
            flat,
            target.internal_id,
            BTreeMap::from([(
                "path_family".to_owned(),
                serde_json::json!(root.path_family()),
            )]),
        )
        .and_then(|target| target.with_localized_aliases(aliases))
        .map_err(|_| invalid())?;
        if target.status == "active" {
            result.push(resolved);
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
