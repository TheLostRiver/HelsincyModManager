use super::path::parse_safe_relative_path;
use super::{WeaponFamily, WeaponResourceRoot};
use crate::armor_retarget::{normalize_armor_display_text, normalize_armor_search_text};
use crate::{generate_mhw_equipment_stable_id, EquipmentCandidateTargetKind};
use hmm_core::ReplacementTargetId;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const MHW_WEAPON_CATALOG_SOURCE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WeaponCatalogSourceError {
    #[error("weapon catalog source JSON is invalid")]
    InvalidJson,
    #[error("unsupported weapon catalog source schema version: {schema_version}")]
    UnsupportedSchemaVersion { schema_version: u32 },
    #[error("weapon catalog source version is invalid")]
    InvalidCatalogVersion,
    #[error("weapon catalog source belongs to another game")]
    WrongGame,
    #[error("weapon catalog source is empty")]
    EmptyCatalog,
    #[error("weapon catalog source target type is invalid")]
    InvalidTargetType,
    #[error("weapon catalog source resource path is unsafe")]
    UnsafeResourcePath,
    #[error("weapon catalog source resource path is not canonical")]
    NonCanonicalResourcePath,
    #[error("weapon catalog source target metadata is invalid")]
    InvalidTargetMetadata,
    #[error("weapon catalog source stable id does not match its resource identity")]
    StableIdMismatch,
    #[error("weapon catalog source contains a duplicate stable id")]
    DuplicateStableId,
    #[error("weapon catalog source contains a duplicate resource path")]
    DuplicateResourcePath,
    #[error("weapon catalog source contains a case-insensitive resource path collision")]
    CaseInsensitivePathCollision,
    #[error("weapon catalog source contains invalid localized names")]
    InvalidNames,
    #[error("weapon catalog source contains a duplicate localized display name")]
    DuplicateDisplayName,
    #[error("weapon catalog source contains an invalid legacy id")]
    InvalidLegacyId,
    #[error("weapon catalog source contains an ambiguous stable or legacy id")]
    AmbiguousId,
    #[error("weapon catalog source contains a dummy target")]
    DummyTarget,
}

impl WeaponCatalogSourceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidJson => "weapon_catalog_invalid_json",
            Self::UnsupportedSchemaVersion { .. } => "weapon_catalog_unsupported_schema_version",
            Self::InvalidCatalogVersion => "weapon_catalog_invalid_version",
            Self::WrongGame => "weapon_catalog_wrong_game",
            Self::EmptyCatalog => "weapon_catalog_empty",
            Self::InvalidTargetType => "weapon_catalog_invalid_target_type",
            Self::UnsafeResourcePath => "weapon_catalog_unsafe_resource_path",
            Self::NonCanonicalResourcePath => "weapon_catalog_non_canonical_resource_path",
            Self::InvalidTargetMetadata => "weapon_catalog_invalid_target_metadata",
            Self::StableIdMismatch => "weapon_catalog_stable_id_mismatch",
            Self::DuplicateStableId => "weapon_catalog_duplicate_stable_id",
            Self::DuplicateResourcePath => "weapon_catalog_duplicate_resource_path",
            Self::CaseInsensitivePathCollision => "weapon_catalog_case_insensitive_path_collision",
            Self::InvalidNames => "weapon_catalog_invalid_names",
            Self::DuplicateDisplayName => "weapon_catalog_duplicate_display_name",
            Self::InvalidLegacyId => "weapon_catalog_invalid_legacy_id",
            Self::AmbiguousId => "weapon_catalog_ambiguous_id",
            Self::DummyTarget => "weapon_catalog_dummy_target",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponTargetStatus {
    Active,
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponTargetMetadata {
    id: ReplacementTargetId,
    root: WeaponResourceRoot,
    status: WeaponTargetStatus,
    display_names: BTreeMap<String, String>,
    aliases: BTreeMap<String, Vec<String>>,
    legacy_ids: Vec<String>,
    search_terms: BTreeSet<String>,
}

impl WeaponTargetMetadata {
    pub fn id(&self) -> &ReplacementTargetId {
        &self.id
    }

    pub fn root(&self) -> &WeaponResourceRoot {
        &self.root
    }

    pub fn family(&self) -> WeaponFamily {
        self.root.family()
    }

    pub fn status(&self) -> WeaponTargetStatus {
        self.status
    }

    pub fn display_names(&self) -> &BTreeMap<String, String> {
        &self.display_names
    }

    pub fn display_name(&self, locale: &str) -> &str {
        self.display_names
            .get(locale)
            .or_else(|| self.display_names.get("en"))
            .or_else(|| self.display_names.values().next())
            .expect("validated weapon targets always have a display name")
    }

    pub fn aliases(&self) -> &BTreeMap<String, Vec<String>> {
        &self.aliases
    }

    pub fn legacy_ids(&self) -> &[String] {
        &self.legacy_ids
    }

    fn matches_query(&self, query: &str) -> bool {
        self.search_terms.iter().any(|term| term.contains(query))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MhwWeaponCatalogSource {
    catalog_version: String,
    targets: Vec<WeaponTargetMetadata>,
    resolver: BTreeMap<String, usize>,
}

impl MhwWeaponCatalogSource {
    pub fn parse(source: &str) -> Result<Self, WeaponCatalogSourceError> {
        parse_catalog_source(source)
    }

    /// 解析按 family 分片的 catalog。
    ///
    /// 全量武器 catalog 有 601 条目、7566 条别名，单文件会超出 policy 的体积硬限，
    /// 因此按 family 拆分——family 本来就是领域边界（跨 family 重定向被禁）。
    /// 校验与单文件完全一致：分片先合并再走同一条校验路径，
    /// 跨分片的 stable_id / legacy_id / 展示名 / 路径碰撞检查一条不漏。
    pub fn parse_sharded(sources: &[&str]) -> Result<Self, WeaponCatalogSourceError> {
        validate_catalog_wire(merge_catalog_wires(sources)?)
    }

    pub fn catalog_version(&self) -> &str {
        &self.catalog_version
    }

    pub fn targets(&self) -> &[WeaponTargetMetadata] {
        &self.targets
    }

    pub fn resolve(&self, id: &str) -> Option<&WeaponTargetMetadata> {
        self.resolver
            .get(id)
            .and_then(|index| self.targets.get(*index))
    }

    pub fn search(&self, query: &str, include_hidden: bool) -> Vec<&WeaponTargetMetadata> {
        let query = normalize_armor_search_text(query);
        if query.is_empty() {
            return Vec::new();
        }

        self.targets
            .iter()
            .filter(|target| include_hidden || target.status == WeaponTargetStatus::Active)
            .filter(|target| target.matches_query(&query))
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct CatalogEnvelope {
    schema_version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogSourceWire {
    schema_version: u32,
    catalog_version: String,
    game_id: String,
    targets: Vec<TargetWire>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetWire {
    stable_id: String,
    target_type: String,
    resource_path: String,
    internal_id: String,
    metadata: TargetMetadataWire,
    status: TargetStatusWire,
    names: BTreeMap<String, LocalizedNamesWire>,
    #[serde(default)]
    legacy_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetMetadataWire {
    family: String,
    path_family: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TargetStatusWire {
    Active,
    Hidden,
    Dummy,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalizedNamesWire {
    display_name: String,
    aliases: Vec<String>,
}

fn parse_catalog_wire(source: &str) -> Result<CatalogSourceWire, WeaponCatalogSourceError> {
    let envelope: CatalogEnvelope =
        serde_json::from_str(source).map_err(|_| WeaponCatalogSourceError::InvalidJson)?;
    if envelope.schema_version != MHW_WEAPON_CATALOG_SOURCE_SCHEMA_VERSION {
        return Err(WeaponCatalogSourceError::UnsupportedSchemaVersion {
            schema_version: envelope.schema_version,
        });
    }

    let raw: CatalogSourceWire =
        serde_json::from_str(source).map_err(|_| WeaponCatalogSourceError::InvalidJson)?;
    if raw.schema_version != MHW_WEAPON_CATALOG_SOURCE_SCHEMA_VERSION {
        return Err(WeaponCatalogSourceError::UnsupportedSchemaVersion {
            schema_version: raw.schema_version,
        });
    }
    Ok(raw)
}

/// 把若干分片合并成一份 wire 再走同一条校验路径。
///
/// 关键：**不能**各自 parse 完再拼 target 列表——stable_id 唯一性、
/// legacy_id 歧义、展示名唯一性和资源路径碰撞都是在单次校验内累积判定的，
/// 分开校验等于把这些保证降级成"每个分片内部唯一"。合并后单次校验则一条不漏。
fn merge_catalog_wires(sources: &[&str]) -> Result<CatalogSourceWire, WeaponCatalogSourceError> {
    let mut merged: Option<CatalogSourceWire> = None;
    for source in sources {
        let wire = parse_catalog_wire(source)?;
        match merged.as_mut() {
            None => merged = Some(wire),
            Some(base) => {
                // 分片必须同属一份 catalog，否则合出来的是个拼接怪物。
                if base.catalog_version != wire.catalog_version || base.game_id != wire.game_id {
                    return Err(WeaponCatalogSourceError::InvalidCatalogVersion);
                }
                base.targets.extend(wire.targets);
            }
        }
    }
    merged.ok_or(WeaponCatalogSourceError::EmptyCatalog)
}

fn parse_catalog_source(source: &str) -> Result<MhwWeaponCatalogSource, WeaponCatalogSourceError> {
    validate_catalog_wire(parse_catalog_wire(source)?)
}

fn validate_catalog_wire(
    mut raw: CatalogSourceWire,
) -> Result<MhwWeaponCatalogSource, WeaponCatalogSourceError> {
    if !is_safe_slug(&raw.catalog_version) {
        return Err(WeaponCatalogSourceError::InvalidCatalogVersion);
    }
    if raw.game_id != "mhw" {
        return Err(WeaponCatalogSourceError::WrongGame);
    }
    if raw.targets.is_empty() {
        return Err(WeaponCatalogSourceError::EmptyCatalog);
    }

    validate_resource_path_collisions(&raw.targets)?;
    raw.targets.sort_by(|left, right| {
        left.resource_path
            .to_ascii_lowercase()
            .cmp(&right.resource_path.to_ascii_lowercase())
            .then_with(|| left.resource_path.cmp(&right.resource_path))
    });

    let mut stable_ids = BTreeSet::new();
    let mut all_ids = BTreeSet::new();
    let mut display_names = BTreeSet::new();
    let mut targets = Vec::with_capacity(raw.targets.len());
    for target in raw.targets {
        if target.target_type != "weapon" {
            return Err(WeaponCatalogSourceError::InvalidTargetType);
        }
        let status = match target.status {
            TargetStatusWire::Active => WeaponTargetStatus::Active,
            TargetStatusWire::Hidden => WeaponTargetStatus::Hidden,
            TargetStatusWire::Dummy => return Err(WeaponCatalogSourceError::DummyTarget),
        };

        let root = WeaponResourceRoot::parse(&target.resource_path)
            .map_err(|_| WeaponCatalogSourceError::InvalidTargetMetadata)?;
        if root.normalized_path().as_str() != target.resource_path {
            return Err(WeaponCatalogSourceError::NonCanonicalResourcePath);
        }
        if target.metadata.family != root.family().as_str()
            || target.metadata.path_family != root.path_family()
            || target.internal_id != root.main_id().as_str()
        {
            return Err(WeaponCatalogSourceError::InvalidTargetMetadata);
        }

        let expected_id = generate_mhw_equipment_stable_id(
            EquipmentCandidateTargetKind::Weapon,
            root.path_family(),
            root.normalized_path().as_str(),
        )
        .map_err(|_| WeaponCatalogSourceError::InvalidTargetMetadata)?;
        if target.stable_id != expected_id {
            return Err(WeaponCatalogSourceError::StableIdMismatch);
        }
        if !stable_ids.insert(target.stable_id.clone()) {
            return Err(WeaponCatalogSourceError::DuplicateStableId);
        }
        if !all_ids.insert(target.stable_id.clone()) {
            return Err(WeaponCatalogSourceError::AmbiguousId);
        }

        let (localized_display_names, aliases, search_terms) =
            validate_names(target.names, &mut display_names)?;
        let mut legacy_ids = target.legacy_ids;
        legacy_ids.sort();
        for legacy_id in &legacy_ids {
            if !is_legacy_id(legacy_id) {
                return Err(WeaponCatalogSourceError::InvalidLegacyId);
            }
            if !all_ids.insert(legacy_id.clone()) {
                return Err(WeaponCatalogSourceError::AmbiguousId);
            }
        }

        targets.push(WeaponTargetMetadata {
            id: ReplacementTargetId::parse(target.stable_id)
                .map_err(|_| WeaponCatalogSourceError::StableIdMismatch)?,
            root,
            status,
            display_names: localized_display_names,
            aliases,
            legacy_ids,
            search_terms,
        });
    }

    targets.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    let mut resolver = BTreeMap::new();
    for (index, target) in targets.iter().enumerate() {
        resolver.insert(target.id.as_str().to_owned(), index);
        for legacy_id in &target.legacy_ids {
            resolver.insert(legacy_id.clone(), index);
        }
    }

    Ok(MhwWeaponCatalogSource {
        catalog_version: raw.catalog_version,
        targets,
        resolver,
    })
}

fn validate_resource_path_collisions(
    targets: &[TargetWire],
) -> Result<(), WeaponCatalogSourceError> {
    let mut path_keys = BTreeMap::<String, String>::new();
    for target in targets {
        let path = parse_safe_relative_path(&target.resource_path)
            .map_err(|_| WeaponCatalogSourceError::UnsafeResourcePath)?;
        if path.as_str() != target.resource_path {
            return Err(WeaponCatalogSourceError::NonCanonicalResourcePath);
        }

        let canonical = path.as_str().to_owned();
        if let Some(previous) = path_keys.insert(canonical.to_ascii_lowercase(), canonical.clone())
        {
            return if previous == canonical {
                Err(WeaponCatalogSourceError::DuplicateResourcePath)
            } else {
                Err(WeaponCatalogSourceError::CaseInsensitivePathCollision)
            };
        }
    }
    Ok(())
}

type ValidatedNames = (
    BTreeMap<String, String>,
    BTreeMap<String, Vec<String>>,
    BTreeSet<String>,
);

fn validate_names(
    names: BTreeMap<String, LocalizedNamesWire>,
    global_display_names: &mut BTreeSet<(String, String)>,
) -> Result<ValidatedNames, WeaponCatalogSourceError> {
    if names.is_empty() {
        return Err(WeaponCatalogSourceError::InvalidNames);
    }

    let mut display_names = BTreeMap::new();
    let mut aliases_by_locale = BTreeMap::new();
    let mut search_terms = BTreeSet::new();
    for (locale, names) in names {
        if !is_locale(&locale) {
            return Err(WeaponCatalogSourceError::InvalidNames);
        }
        let normalized_display = normalize_armor_display_text(&names.display_name);
        let display_search = normalize_armor_search_text(&normalized_display);
        if display_search.is_empty() {
            return Err(WeaponCatalogSourceError::InvalidNames);
        }
        if !global_display_names.insert((locale.clone(), display_search.clone())) {
            return Err(WeaponCatalogSourceError::DuplicateDisplayName);
        }
        search_terms.insert(display_search);

        let mut normalized_aliases = BTreeMap::new();
        for alias in names.aliases {
            let normalized_alias = normalize_armor_display_text(&alias);
            let alias_key = normalize_armor_search_text(&normalized_alias);
            if alias_key.is_empty()
                || normalized_aliases
                    .insert(alias_key.clone(), normalized_alias)
                    .is_some()
            {
                return Err(WeaponCatalogSourceError::InvalidNames);
            }
            search_terms.insert(alias_key);
        }

        display_names.insert(locale.clone(), normalized_display);
        aliases_by_locale.insert(locale, normalized_aliases.into_values().collect());
    }

    Ok((display_names, aliases_by_locale, search_terms))
}

fn is_safe_slug(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn is_legacy_id(value: &str) -> bool {
    value.strip_prefix("mhw:weapon:").is_some_and(is_safe_slug)
}

fn is_locale(value: &str) -> bool {
    let mut segments = value.split('_');
    let Some(language) = segments.next() else {
        return false;
    };
    if !(2..=3).contains(&language.len()) || !language.bytes().all(|byte| byte.is_ascii_lowercase())
    {
        return false;
    }

    segments.all(|segment| {
        (2..=8).contains(&segment.len())
            && segment
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    })
}
