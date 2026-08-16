use crate::armor_retarget::normalize_armor_search_text;
use hmm_core::InstallTargetPath;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use thiserror::Error;

pub const MHW_EQUIPMENT_CANDIDATE_SCHEMA_VERSION: u32 = 1;
pub const MHW_EQUIPMENT_CANDIDATE_JSON_SCHEMA: &str =
    include_str!("../data/schemas/mhw-equipment-candidates.v1.schema.json");

const STABLE_ID_DOMAIN: &str = "hmm-mhw-equipment-candidate-v1";
const NATIVE_PC_ROOT: &str = "nativePC";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipmentCandidateTargetKind {
    Armor,
    Weapon,
}

impl EquipmentCandidateTargetKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Armor => "armor",
            Self::Weapon => "weapon",
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EquipmentCandidateCatalogError {
    #[error("equipment candidate catalog JSON is invalid")]
    InvalidJson,
    #[error("unsupported equipment candidate schema version: {schema_version}")]
    UnsupportedSchemaVersion { schema_version: u32 },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EquipmentCandidateIdentityError {
    #[error("equipment candidate resource path is unsafe")]
    UnsafeResourcePath,
    #[error("equipment candidate resource path is not canonical")]
    NonCanonicalResourcePath,
    #[error("equipment candidate path family is invalid for its target kind")]
    WrongPathFamily,
    #[error("equipment candidate armor resource path is invalid")]
    InvalidArmorResourcePath,
    #[error("equipment candidate weapon resource path is invalid")]
    InvalidWeaponResourcePath,
}

impl EquipmentCandidateIdentityError {
    fn issue_code(&self) -> &'static str {
        match self {
            Self::UnsafeResourcePath => "unsafe_resource_path",
            Self::NonCanonicalResourcePath => "non_canonical_resource_path",
            Self::WrongPathFamily => "wrong_path_family",
            Self::InvalidArmorResourcePath => "invalid_armor_resource_path",
            Self::InvalidWeaponResourcePath => "invalid_weapon_resource_path",
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EquipmentCandidateBundlingError {
    #[error(transparent)]
    Catalog(#[from] EquipmentCandidateCatalogError),
    #[error("equipment candidate catalog failed validation")]
    ValidationFailed,
    #[error("equipment candidate catalog is not eligible for bundling")]
    EligibilityBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EquipmentCandidateValidationIssue {
    pub code: String,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EquipmentCandidateBundleBlocker {
    pub code: String,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EquipmentCandidateValidationReport {
    pub schema_version: u32,
    pub catalog_version: String,
    pub target_count: usize,
    pub active_target_count: usize,
    pub hidden_target_count: usize,
    pub dummy_target_count: usize,
    pub valid: bool,
    pub bundled_eligible: bool,
    pub issues: Vec<EquipmentCandidateValidationIssue>,
    pub bundle_blockers: Vec<EquipmentCandidateBundleBlocker>,
}

#[derive(Debug, Deserialize)]
struct CandidateCatalogEnvelope {
    schema_version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateCatalog {
    schema_version: u32,
    catalog_version: String,
    game_id: String,
    sources: Vec<CandidateSource>,
    targets: Vec<CandidateTarget>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateSource {
    source_id: String,
    source_name: String,
    source_url: String,
    retrieved_at: String,
    license: CandidateLicense,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateLicense {
    status: CandidateLicenseStatus,
    spdx_expression: Option<String>,
    evidence_url: Option<String>,
    attribution: Option<String>,
    reviewed_by: Option<String>,
    reviewed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CandidateLicenseStatus {
    Unknown,
    Restricted,
    Redistributable,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateTarget {
    stable_id: String,
    target_kind: EquipmentCandidateTargetKind,
    path_family: String,
    resource_path: String,
    status: CandidateTargetStatus,
    names: BTreeMap<String, CandidateLocalizedNames>,
    source_ids: Vec<String>,
    #[serde(default)]
    legacy_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CandidateTargetStatus {
    Active,
    Hidden,
    Dummy,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateLocalizedNames {
    display_name: String,
    aliases: Vec<String>,
}

struct ValidatedIdentity {
    stable_id: String,
    normalized_path: String,
}

pub fn generate_mhw_equipment_stable_id(
    target_kind: EquipmentCandidateTargetKind,
    path_family: &str,
    resource_path: &str,
) -> Result<String, EquipmentCandidateIdentityError> {
    Ok(validate_resource_identity(target_kind, path_family, resource_path)?.stable_id)
}

pub fn validate_mhw_equipment_candidate_catalog(
    source: &str,
) -> Result<EquipmentCandidateValidationReport, EquipmentCandidateCatalogError> {
    let envelope: CandidateCatalogEnvelope =
        serde_json::from_str(source).map_err(|_| EquipmentCandidateCatalogError::InvalidJson)?;
    if envelope.schema_version != MHW_EQUIPMENT_CANDIDATE_SCHEMA_VERSION {
        return Err(EquipmentCandidateCatalogError::UnsupportedSchemaVersion {
            schema_version: envelope.schema_version,
        });
    }

    let catalog: CandidateCatalog =
        serde_json::from_str(source).map_err(|_| EquipmentCandidateCatalogError::InvalidJson)?;
    Ok(validate_candidate_catalog(catalog))
}

pub fn validate_mhw_equipment_candidate_catalog_for_bundling(
    source: &str,
) -> Result<EquipmentCandidateValidationReport, EquipmentCandidateBundlingError> {
    let report = validate_mhw_equipment_candidate_catalog(source)?;
    if !report.valid {
        return Err(EquipmentCandidateBundlingError::ValidationFailed);
    }
    if !report.bundled_eligible {
        return Err(EquipmentCandidateBundlingError::EligibilityBlocked);
    }
    Ok(report)
}

fn validate_candidate_catalog(catalog: CandidateCatalog) -> EquipmentCandidateValidationReport {
    let mut issues = Vec::new();
    let mut blockers = Vec::new();

    let catalog_version_is_safe = is_safe_slug(&catalog.catalog_version);
    if !catalog_version_is_safe {
        push_issue(&mut issues, "invalid_catalog_version", "catalog_version");
    }
    if catalog.game_id != "mhw" {
        push_issue(&mut issues, "wrong_game_id", "game_id");
    }
    if catalog.sources.is_empty() {
        push_issue(&mut issues, "empty_sources", "sources");
    }
    if catalog.targets.is_empty() {
        push_issue(&mut issues, "empty_targets", "targets");
    }

    let mut source_indices = BTreeMap::new();
    for (index, source) in catalog.sources.iter().enumerate() {
        let scope = format!("sources[{index}]");
        if !is_safe_slug(&source.source_id) {
            push_issue(
                &mut issues,
                "invalid_source_id",
                format!("{scope}.source_id"),
            );
        }
        if source_indices
            .insert(source.source_id.as_str(), index)
            .is_some()
        {
            push_issue(
                &mut issues,
                "duplicate_source_id",
                format!("{scope}.source_id"),
            );
        }
        if source.source_name.trim().is_empty() {
            push_issue(
                &mut issues,
                "empty_source_name",
                format!("{scope}.source_name"),
            );
        }
        if !is_https_url(&source.source_url) {
            push_issue(
                &mut issues,
                "invalid_source_url",
                format!("{scope}.source_url"),
            );
        }
        if !is_calendar_date_shape(&source.retrieved_at) {
            push_issue(
                &mut issues,
                "invalid_retrieved_at",
                format!("{scope}.retrieved_at"),
            );
        }

        match source.license.status {
            CandidateLicenseStatus::Unknown => {
                push_blocker(&mut blockers, "license_unknown", format!("{scope}.license"))
            }
            CandidateLicenseStatus::Restricted => push_blocker(
                &mut blockers,
                "license_restricted",
                format!("{scope}.license"),
            ),
            CandidateLicenseStatus::Redistributable => {
                let complete = source
                    .license
                    .spdx_expression
                    .as_deref()
                    .is_some_and(is_non_blank)
                    && source
                        .license
                        .evidence_url
                        .as_deref()
                        .is_some_and(is_https_url)
                    && source
                        .license
                        .attribution
                        .as_deref()
                        .is_some_and(is_non_blank)
                    && source
                        .license
                        .reviewed_by
                        .as_deref()
                        .is_some_and(is_non_blank)
                    && source
                        .license
                        .reviewed_at
                        .as_deref()
                        .is_some_and(is_calendar_date_shape);
                if !complete {
                    push_issue(
                        &mut issues,
                        "incomplete_redistributable_license",
                        format!("{scope}.license"),
                    );
                }
            }
        }
    }

    let mut referenced_sources = BTreeSet::new();
    let mut stable_ids = BTreeMap::new();
    let mut legacy_ids = BTreeMap::new();
    let mut path_keys = BTreeMap::new();
    let mut display_names = BTreeMap::new();

    for (index, target) in catalog.targets.iter().enumerate() {
        validate_target(
            target,
            index,
            &source_indices,
            &mut referenced_sources,
            &mut stable_ids,
            &mut legacy_ids,
            &mut path_keys,
            &mut display_names,
            &mut issues,
            &mut blockers,
        );
    }

    for (source_id, index) in &source_indices {
        if !referenced_sources.contains(*source_id) {
            push_issue(
                &mut issues,
                "unused_source",
                format!("sources[{index}].source_id"),
            );
        }
    }

    let active_target_count = catalog
        .targets
        .iter()
        .filter(|target| target.status == CandidateTargetStatus::Active)
        .count();
    let hidden_target_count = catalog
        .targets
        .iter()
        .filter(|target| target.status == CandidateTargetStatus::Hidden)
        .count();
    let dummy_target_count = catalog
        .targets
        .iter()
        .filter(|target| target.status == CandidateTargetStatus::Dummy)
        .count();
    let valid = issues.is_empty();
    let bundled_eligible = valid && blockers.is_empty();

    EquipmentCandidateValidationReport {
        schema_version: catalog.schema_version,
        catalog_version: if catalog_version_is_safe {
            catalog.catalog_version
        } else {
            "<invalid>".to_owned()
        },
        target_count: catalog.targets.len(),
        active_target_count,
        hidden_target_count,
        dummy_target_count,
        valid,
        bundled_eligible,
        issues,
        bundle_blockers: blockers,
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_target<'a>(
    target: &'a CandidateTarget,
    index: usize,
    source_indices: &BTreeMap<&'a str, usize>,
    referenced_sources: &mut BTreeSet<&'a str>,
    stable_ids: &mut BTreeMap<&'a str, usize>,
    legacy_ids: &mut BTreeMap<&'a str, usize>,
    path_keys: &mut BTreeMap<String, (usize, &'a str)>,
    display_names: &mut BTreeMap<(String, String), usize>,
    issues: &mut Vec<EquipmentCandidateValidationIssue>,
    blockers: &mut Vec<EquipmentCandidateBundleBlocker>,
) {
    let scope = format!("targets[{index}]");

    if !is_stable_id_shape(&target.stable_id, target.target_kind) {
        push_issue(
            &mut *issues,
            "invalid_stable_id",
            format!("{scope}.stable_id"),
        );
    }
    if stable_ids
        .insert(target.stable_id.as_str(), index)
        .is_some()
    {
        push_issue(
            &mut *issues,
            "duplicate_stable_id",
            format!("{scope}.stable_id"),
        );
    }

    match validate_resource_identity(
        target.target_kind,
        &target.path_family,
        &target.resource_path,
    ) {
        Ok(identity) => {
            if target.stable_id != identity.stable_id {
                push_issue(
                    &mut *issues,
                    "stable_id_mismatch",
                    format!("{scope}.stable_id"),
                );
            }

            let path_key = identity.normalized_path.to_ascii_lowercase();
            if let Some((_, previous_path)) =
                path_keys.insert(path_key, (index, target.resource_path.as_str()))
            {
                let code = if previous_path == target.resource_path {
                    "duplicate_resource_path"
                } else {
                    "case_insensitive_path_collision"
                };
                push_issue(&mut *issues, code, format!("{scope}.resource_path"));
            }
        }
        Err(error) => push_issue(
            &mut *issues,
            error.issue_code(),
            format!("{scope}.resource_path"),
        ),
    }

    validate_names(target, index, display_names, issues);
    validate_source_references(target, index, source_indices, referenced_sources, issues);
    validate_legacy_ids(target, index, legacy_ids, issues);

    if target.status == CandidateTargetStatus::Dummy {
        push_blocker(&mut *blockers, "dummy_target", scope);
    }
}

fn validate_names(
    target: &CandidateTarget,
    index: usize,
    display_names: &mut BTreeMap<(String, String), usize>,
    issues: &mut Vec<EquipmentCandidateValidationIssue>,
) {
    let scope = format!("targets[{index}].names");
    if target.names.is_empty() {
        push_issue(&mut *issues, "empty_names", scope);
        return;
    }

    for (locale_index, (locale, names)) in target.names.iter().enumerate() {
        let locale_scope = format!("{scope}[{locale_index}]");
        if !is_locale(locale) {
            push_issue(&mut *issues, "invalid_locale", locale_scope.clone());
        }

        let display_key = normalize_armor_search_text(&names.display_name);
        if display_key.is_empty() {
            push_issue(
                &mut *issues,
                "empty_display_name",
                format!("{locale_scope}.display_name"),
            );
        } else if display_names
            .insert((locale.to_owned(), display_key), index)
            .is_some()
        {
            push_issue(
                &mut *issues,
                "duplicate_display_name",
                format!("{locale_scope}.display_name"),
            );
        }

        let mut aliases = BTreeSet::new();
        for (alias_index, alias) in names.aliases.iter().enumerate() {
            let normalized = normalize_armor_search_text(alias);
            if normalized.is_empty() {
                push_issue(
                    &mut *issues,
                    "empty_alias",
                    format!("{locale_scope}.aliases[{alias_index}]"),
                );
            } else if !aliases.insert(normalized) {
                push_issue(
                    &mut *issues,
                    "duplicate_alias",
                    format!("{locale_scope}.aliases[{alias_index}]"),
                );
            }
        }
    }
}

fn validate_source_references<'a>(
    target: &'a CandidateTarget,
    index: usize,
    source_indices: &BTreeMap<&'a str, usize>,
    referenced_sources: &mut BTreeSet<&'a str>,
    issues: &mut Vec<EquipmentCandidateValidationIssue>,
) {
    let scope = format!("targets[{index}].source_ids");
    if target.source_ids.is_empty() {
        push_issue(&mut *issues, "empty_source_references", scope);
        return;
    }

    let mut local_sources = BTreeSet::new();
    for (source_index, source_id) in target.source_ids.iter().enumerate() {
        if !local_sources.insert(source_id.as_str()) {
            push_issue(
                &mut *issues,
                "duplicate_source_reference",
                format!("{scope}[{source_index}]"),
            );
        }
        if source_indices.contains_key(source_id.as_str()) {
            referenced_sources.insert(source_id);
        } else {
            push_issue(
                &mut *issues,
                "unknown_source_reference",
                format!("{scope}[{source_index}]"),
            );
        }
    }
}

fn validate_legacy_ids<'a>(
    target: &'a CandidateTarget,
    index: usize,
    legacy_ids: &mut BTreeMap<&'a str, usize>,
    issues: &mut Vec<EquipmentCandidateValidationIssue>,
) {
    let scope = format!("targets[{index}].legacy_ids");
    let mut local_ids = BTreeSet::new();
    for (legacy_index, legacy_id) in target.legacy_ids.iter().enumerate() {
        let item_scope = format!("{scope}[{legacy_index}]");
        if !is_legacy_id_shape(legacy_id, target.target_kind) {
            push_issue(&mut *issues, "invalid_legacy_id", item_scope.clone());
        }
        if legacy_id == &target.stable_id {
            push_issue(
                &mut *issues,
                "legacy_id_matches_stable_id",
                item_scope.clone(),
            );
        }
        if !local_ids.insert(legacy_id.as_str())
            || legacy_ids.insert(legacy_id.as_str(), index).is_some()
        {
            push_issue(&mut *issues, "duplicate_legacy_id", item_scope);
        }
    }
}

fn validate_resource_identity(
    target_kind: EquipmentCandidateTargetKind,
    path_family: &str,
    resource_path: &str,
) -> Result<ValidatedIdentity, EquipmentCandidateIdentityError> {
    let normalized = InstallTargetPath::parse(resource_path, [NATIVE_PC_ROOT])
        .map_err(|_| EquipmentCandidateIdentityError::UnsafeResourcePath)?;
    if normalized.as_str() != resource_path {
        return Err(EquipmentCandidateIdentityError::NonCanonicalResourcePath);
    }

    let segments = normalized.as_str().split('/').collect::<Vec<_>>();
    if !segments.iter().all(|segment| is_safe_path_segment(segment)) {
        return Err(EquipmentCandidateIdentityError::UnsafeResourcePath);
    }

    match target_kind {
        EquipmentCandidateTargetKind::Armor => {
            if path_family != "pl/f_equip" {
                return Err(EquipmentCandidateIdentityError::WrongPathFamily);
            }
            if segments.len() != 4
                || !segments[1].eq_ignore_ascii_case("pl")
                || !segments[2].eq_ignore_ascii_case("f_equip")
                || !is_valid_armor_internal_id(segments[3])
            {
                return Err(EquipmentCandidateIdentityError::InvalidArmorResourcePath);
            }
        }
        EquipmentCandidateTargetKind::Weapon => {
            let family = path_family
                .strip_prefix("wp/")
                .filter(|family| is_weapon_family(family))
                .ok_or(EquipmentCandidateIdentityError::WrongPathFamily)?;
            if segments.len() < 4
                || !segments[1].eq_ignore_ascii_case("wp")
                || !segments[2].eq_ignore_ascii_case(family)
            {
                return Err(EquipmentCandidateIdentityError::InvalidWeaponResourcePath);
            }
        }
    }

    let normalized_path = normalized.as_str().to_owned();
    let canonical_identity = format!(
        "{STABLE_ID_DOMAIN}\0mhw\0{}\0{}\0{}",
        target_kind.as_str(),
        path_family,
        normalized_path.to_ascii_lowercase()
    );
    let digest = Sha256::digest(canonical_identity.as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in digest {
        write!(&mut hex, "{byte:02x}").expect("writing to a String cannot fail");
    }

    Ok(ValidatedIdentity {
        stable_id: format!("mhw:{}:{hex}", target_kind.as_str()),
        normalized_path,
    })
}

fn is_valid_armor_internal_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && &bytes[..2] == b"pl"
        && bytes[2..5].iter().all(u8::is_ascii_digit)
        && bytes[5] == b'_'
        && bytes[6..].iter().all(u8::is_ascii_digit)
}

fn is_weapon_family(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn is_safe_path_segment(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn is_stable_id_shape(value: &str, target_kind: EquipmentCandidateTargetKind) -> bool {
    let prefix = format!("mhw:{}:", target_kind.as_str());
    value.strip_prefix(&prefix).is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn is_legacy_id_shape(value: &str, target_kind: EquipmentCandidateTargetKind) -> bool {
    let prefix = format!("mhw:{}:", target_kind.as_str());
    value.strip_prefix(&prefix).is_some_and(is_safe_slug)
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

fn is_calendar_date_shape(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return false;
    }

    let Ok(year) = value[..4].parse::<u16>() else {
        return false;
    };
    let Ok(month) = value[5..7].parse::<u8>() else {
        return false;
    };
    let Ok(day) = value[8..10].parse::<u8>() else {
        return false;
    };
    if year == 0 || !(1..=12).contains(&month) {
        return false;
    }

    let is_leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        2 if is_leap_year => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=days_in_month).contains(&day)
}

fn is_https_url(value: &str) -> bool {
    value
        .strip_prefix("https://")
        .is_some_and(|rest| !rest.is_empty() && !rest.chars().any(char::is_whitespace))
}

fn is_non_blank(value: &str) -> bool {
    !value.trim().is_empty()
}

fn push_issue(
    issues: &mut Vec<EquipmentCandidateValidationIssue>,
    code: impl Into<String>,
    scope: impl Into<String>,
) {
    issues.push(EquipmentCandidateValidationIssue {
        code: code.into(),
        scope: scope.into(),
    });
}

fn push_blocker(
    blockers: &mut Vec<EquipmentCandidateBundleBlocker>,
    code: impl Into<String>,
    scope: impl Into<String>,
) {
    blockers.push(EquipmentCandidateBundleBlocker {
        code: code.into(),
        scope: scope.into(),
    });
}
