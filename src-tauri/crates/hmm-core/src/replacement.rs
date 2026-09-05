use crate::{GameId, ModId, ModRevisionId, PackageFileId, ProfileId, RetargetPlan};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplacementError {
    #[error("replacement target id cannot be empty")]
    EmptyTargetId,
    #[error("replacement binding id cannot be empty")]
    EmptyBindingId,
    #[error("replacement source id cannot be empty")]
    EmptySourceId,
    #[error("replacement target kind cannot be empty")]
    EmptyTargetKind,
    #[error("replacement catalog version cannot be empty")]
    EmptyCatalogVersion,
    #[error("localized text cannot be empty")]
    EmptyLocalizedText,
    #[error("localized text locale cannot be empty")]
    EmptyLocale,
    #[error("localized text value cannot be empty for locale: {locale}")]
    EmptyLocalizedValue { locale: String },
    #[error("localized text contains a duplicate locale: {locale}")]
    DuplicateLocale { locale: String },
    #[error("replacement internal id cannot be empty")]
    EmptyInternalId,
    #[error("replacement alias cannot be empty")]
    EmptyAlias,
    #[error("localized alias locale has no display name: {locale}")]
    LocalizedAliasLocaleUnknown { locale: String },
    #[error("localized alias is not part of the searchable aliases: {alias}")]
    LocalizedAliasNotSearchable { alias: String },
    #[error("replacement binding mod id cannot be empty")]
    EmptyModId,
    #[error("replacement binding profile id cannot be empty")]
    EmptyProfileId,
    #[error("replacement snapshot source internal id cannot be empty")]
    EmptySnapshotSourceInternalId,
    #[error("replacement snapshot target internal id cannot be empty")]
    EmptySnapshotTargetInternalId,
    #[error("replacement snapshot source path family cannot be empty")]
    EmptySnapshotSourcePathFamily,
    #[error("replacement snapshot target path family cannot be empty")]
    EmptySnapshotTargetPathFamily,
    #[error("replacement catalog cannot be empty")]
    EmptyCatalog,
    #[error("replacement catalog contains a duplicate target id: {target_id}")]
    DuplicateTargetId { target_id: String },
    #[error(
        "replacement target {target_id} belongs to game {actual_game_id}, expected {expected_game_id}"
    )]
    TargetGameMismatch {
        target_id: String,
        expected_game_id: String,
        actual_game_id: String,
    },
    #[error("unsupported content transform invocation schema version: {schema_version}")]
    UnsupportedContentTransformSchemaVersion { schema_version: u32 },
    #[error("unsupported replacement adapter facts schema version: {schema_version}")]
    UnsupportedAdapterFactsSchemaVersion { schema_version: u32 },
    #[error("replacement transform identifier is invalid")]
    InvalidTransformIdentifier,
    #[error("replacement transform version is invalid")]
    InvalidTransformVersion,
    #[error("replacement transform digest is invalid")]
    InvalidTransformDigest,
    #[error("replacement transform dependencies exceed the bounded limit")]
    TooManyTransformDependencies,
    #[error("replacement transformer identities exceed the bounded limit")]
    TooManyTransformerIdentities,
    #[error("replacement transform dependency is invalid")]
    InvalidTransformDependency,
    #[error("replacement transform parameters exceed the bounded limit")]
    TooManyTransformParameters,
    #[error("replacement transform parameter is invalid")]
    InvalidTransformParameter,
    #[error("replacement transform aggregate counts are invalid")]
    InvalidTransformCounts,
}

pub const CONTENT_TRANSFORM_INVOCATION_SCHEMA_VERSION: u32 = 1;
pub const REPLACEMENT_ADAPTER_FACTS_SCHEMA_VERSION: u32 = 1;
const MAX_TRANSFORM_IDENTIFIER_BYTES: usize = 128;
const MAX_TRANSFORM_DEPENDENCIES: usize = 8;
const MAX_TRANSFORMER_IDENTITIES: usize = 8;
const MAX_TRANSFORM_PARAMETERS: usize = 16;
const MAX_TRANSFORM_PARAMETER_KEY_BYTES: usize = 64;
const MAX_TRANSFORM_PARAMETER_VALUE_BYTES: usize = 512;
const MAX_TRANSFORM_PACKAGE_FILE_ID_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "ContentTransformerIdentityWire")]
pub struct ContentTransformerIdentity {
    transformer_id: String,
    transformer_version: u32,
}

#[derive(Deserialize)]
struct ContentTransformerIdentityWire {
    transformer_id: String,
    transformer_version: u32,
}

impl ContentTransformerIdentity {
    pub fn new(
        transformer_id: impl Into<String>,
        transformer_version: u32,
    ) -> Result<Self, ReplacementError> {
        let transformer_id = transformer_id.into();
        if !is_bounded_identifier(&transformer_id) {
            return Err(ReplacementError::InvalidTransformIdentifier);
        }
        if transformer_version == 0 {
            return Err(ReplacementError::InvalidTransformVersion);
        }
        Ok(Self {
            transformer_id,
            transformer_version,
        })
    }

    pub fn transformer_id(&self) -> &str {
        &self.transformer_id
    }

    pub fn transformer_version(&self) -> u32 {
        self.transformer_version
    }
}

impl TryFrom<ContentTransformerIdentityWire> for ContentTransformerIdentity {
    type Error = ReplacementError;

    fn try_from(wire: ContentTransformerIdentityWire) -> Result<Self, Self::Error> {
        Self::new(wire.transformer_id, wire.transformer_version)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ContentTransformInvocationWire")]
pub struct ContentTransformInvocation {
    schema_version: u32,
    transformer_id: String,
    transformer_version: u32,
    source_content_sha256: String,
    output_content_sha256: String,
    canonical_mapping_sha256: String,
    dependencies: BTreeMap<PackageFileId, String>,
    parameters: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct ContentTransformInvocationWire {
    schema_version: u32,
    transformer_id: String,
    transformer_version: u32,
    source_content_sha256: String,
    output_content_sha256: String,
    canonical_mapping_sha256: String,
    #[serde(default)]
    dependencies: BTreeMap<PackageFileId, String>,
    #[serde(default)]
    parameters: BTreeMap<String, String>,
}

impl ContentTransformInvocation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: u32,
        transformer_id: impl Into<String>,
        transformer_version: u32,
        source_content_sha256: impl Into<String>,
        output_content_sha256: impl Into<String>,
        canonical_mapping_sha256: impl Into<String>,
        dependencies: BTreeMap<PackageFileId, String>,
        parameters: BTreeMap<String, String>,
    ) -> Result<Self, ReplacementError> {
        if schema_version != CONTENT_TRANSFORM_INVOCATION_SCHEMA_VERSION {
            return Err(ReplacementError::UnsupportedContentTransformSchemaVersion {
                schema_version,
            });
        }
        let transformer_id = transformer_id.into();
        if !is_bounded_identifier(&transformer_id) {
            return Err(ReplacementError::InvalidTransformIdentifier);
        }
        if transformer_version == 0 {
            return Err(ReplacementError::InvalidTransformVersion);
        }
        let source_content_sha256 = source_content_sha256.into();
        let output_content_sha256 = output_content_sha256.into();
        let canonical_mapping_sha256 = canonical_mapping_sha256.into();
        if !is_sha256_hex(&source_content_sha256)
            || !is_sha256_hex(&output_content_sha256)
            || !is_sha256_hex(&canonical_mapping_sha256)
        {
            return Err(ReplacementError::InvalidTransformDigest);
        }
        if dependencies.len() > MAX_TRANSFORM_DEPENDENCIES {
            return Err(ReplacementError::TooManyTransformDependencies);
        }
        if dependencies.iter().any(|(package_file_id, digest)| {
            let id = package_file_id.as_str();
            id.trim() != id
                || id.is_empty()
                || id.len() > MAX_TRANSFORM_PACKAGE_FILE_ID_BYTES
                || id.chars().any(char::is_control)
                || !is_sha256_hex(digest)
        }) {
            return Err(ReplacementError::InvalidTransformDependency);
        }
        if parameters.len() > MAX_TRANSFORM_PARAMETERS {
            return Err(ReplacementError::TooManyTransformParameters);
        }
        if parameters.iter().any(|(key, value)| {
            key.len() > MAX_TRANSFORM_PARAMETER_KEY_BYTES
                || !is_bounded_identifier(key)
                || value.is_empty()
                || value.trim() != value
                || value.len() > MAX_TRANSFORM_PARAMETER_VALUE_BYTES
                || value.chars().any(char::is_control)
        }) {
            return Err(ReplacementError::InvalidTransformParameter);
        }

        Ok(Self {
            schema_version,
            transformer_id,
            transformer_version,
            source_content_sha256,
            output_content_sha256,
            canonical_mapping_sha256,
            dependencies,
            parameters,
        })
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn transformer_id(&self) -> &str {
        &self.transformer_id
    }

    pub fn transformer_version(&self) -> u32 {
        self.transformer_version
    }

    pub fn source_content_sha256(&self) -> &str {
        &self.source_content_sha256
    }

    pub fn output_content_sha256(&self) -> &str {
        &self.output_content_sha256
    }

    pub fn canonical_mapping_sha256(&self) -> &str {
        &self.canonical_mapping_sha256
    }

    pub fn dependencies(&self) -> &BTreeMap<PackageFileId, String> {
        &self.dependencies
    }

    pub fn parameters(&self) -> &BTreeMap<String, String> {
        &self.parameters
    }

    pub fn transformer_identity(&self) -> ContentTransformerIdentity {
        ContentTransformerIdentity {
            transformer_id: self.transformer_id.clone(),
            transformer_version: self.transformer_version,
        }
    }
}

impl TryFrom<ContentTransformInvocationWire> for ContentTransformInvocation {
    type Error = ReplacementError;

    fn try_from(wire: ContentTransformInvocationWire) -> Result<Self, Self::Error> {
        Self::new(
            wire.schema_version,
            wire.transformer_id,
            wire.transformer_version,
            wire.source_content_sha256,
            wire.output_content_sha256,
            wire.canonical_mapping_sha256,
            wire.dependencies,
            wire.parameters,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ReplacementAdapterFactsWire")]
pub struct ReplacementAdapterFacts {
    schema_version: u32,
    adapter_id: String,
    strategy_id: String,
    strategy_version: u32,
    source_closure_sha256: String,
    part_set_sha256: String,
    transform_set_sha256: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    transformer_identities: Vec<ContentTransformerIdentity>,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    part_count: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    file_count: u32,
    /// 被拒绝清单挡下、**没有**进入计划的包内文件数（#336 切片③）。
    ///
    /// 与 `file_count` 是互斥的两堆：`file_count` 是计划真正会写盘的动作数，这里是
    /// 「适配器看见但主动丢弃」的数量。留在 facts 里是为了让「装的时候少了 N 个文件」
    /// 这件事随 manifest 落盘可审计——否则拒绝就是一次无痕的静默丢弃。
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    excluded_file_count: u32,
}

#[derive(Deserialize)]
struct ReplacementAdapterFactsWire {
    schema_version: u32,
    adapter_id: String,
    strategy_id: String,
    strategy_version: u32,
    source_closure_sha256: String,
    part_set_sha256: String,
    transform_set_sha256: String,
    #[serde(default)]
    transformer_identities: Vec<ContentTransformerIdentity>,
    #[serde(default)]
    part_count: u32,
    #[serde(default)]
    file_count: u32,
    #[serde(default)]
    excluded_file_count: u32,
}

impl ReplacementAdapterFacts {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: u32,
        adapter_id: impl Into<String>,
        strategy_id: impl Into<String>,
        strategy_version: u32,
        source_closure_sha256: impl Into<String>,
        part_set_sha256: impl Into<String>,
        transform_set_sha256: impl Into<String>,
    ) -> Result<Self, ReplacementError> {
        if schema_version != REPLACEMENT_ADAPTER_FACTS_SCHEMA_VERSION {
            return Err(ReplacementError::UnsupportedAdapterFactsSchemaVersion { schema_version });
        }
        let adapter_id = adapter_id.into();
        let strategy_id = strategy_id.into();
        if !is_bounded_identifier(&adapter_id) || !is_bounded_identifier(&strategy_id) {
            return Err(ReplacementError::InvalidTransformIdentifier);
        }
        if strategy_version == 0 {
            return Err(ReplacementError::InvalidTransformVersion);
        }
        let source_closure_sha256 = source_closure_sha256.into();
        let part_set_sha256 = part_set_sha256.into();
        let transform_set_sha256 = transform_set_sha256.into();
        if !is_sha256_hex(&source_closure_sha256)
            || !is_sha256_hex(&part_set_sha256)
            || !is_sha256_hex(&transform_set_sha256)
        {
            return Err(ReplacementError::InvalidTransformDigest);
        }
        Ok(Self {
            schema_version,
            adapter_id,
            strategy_id,
            strategy_version,
            source_closure_sha256,
            part_set_sha256,
            transform_set_sha256,
            transformer_identities: Vec::new(),
            part_count: 0,
            file_count: 0,
            excluded_file_count: 0,
        })
    }

    pub fn with_transformers(
        mut self,
        mut transformer_identities: Vec<ContentTransformerIdentity>,
        part_count: u32,
        file_count: u32,
    ) -> Result<Self, ReplacementError> {
        if transformer_identities.is_empty()
            || transformer_identities.len() > MAX_TRANSFORMER_IDENTITIES
        {
            return Err(ReplacementError::TooManyTransformerIdentities);
        }
        transformer_identities.sort();
        if transformer_identities
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(ReplacementError::InvalidTransformIdentifier);
        }
        if part_count == 0 || file_count == 0 {
            return Err(ReplacementError::InvalidTransformCounts);
        }
        self.transformer_identities = transformer_identities;
        self.part_count = part_count;
        self.file_count = file_count;
        Ok(self)
    }

    /// 记录本次分析中被适配器主动丢弃、不进入计划的包内文件数（#336 切片③ 的拒绝清单）。
    ///
    /// 与 `with_transformers` 相互独立：`0` 是常态（绝大多数包没有可拒绝的文件），
    /// 所以这里不像 `part_count` / `file_count` 那样把 `0` 当成非法值。
    pub fn with_excluded_file_count(mut self, excluded_file_count: u32) -> Self {
        self.excluded_file_count = excluded_file_count;
        self
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn adapter_id(&self) -> &str {
        &self.adapter_id
    }

    pub fn strategy_id(&self) -> &str {
        &self.strategy_id
    }

    pub fn strategy_version(&self) -> u32 {
        self.strategy_version
    }

    pub fn source_closure_sha256(&self) -> &str {
        &self.source_closure_sha256
    }

    pub fn part_set_sha256(&self) -> &str {
        &self.part_set_sha256
    }

    pub fn transform_set_sha256(&self) -> &str {
        &self.transform_set_sha256
    }

    pub fn transformer_identities(&self) -> &[ContentTransformerIdentity] {
        &self.transformer_identities
    }

    pub fn part_count(&self) -> u32 {
        self.part_count
    }

    pub fn file_count(&self) -> u32 {
        self.file_count
    }

    pub fn excluded_file_count(&self) -> u32 {
        self.excluded_file_count
    }
}

impl TryFrom<ReplacementAdapterFactsWire> for ReplacementAdapterFacts {
    type Error = ReplacementError;

    fn try_from(wire: ReplacementAdapterFactsWire) -> Result<Self, Self::Error> {
        let facts = Self::new(
            wire.schema_version,
            wire.adapter_id,
            wire.strategy_id,
            wire.strategy_version,
            wire.source_closure_sha256,
            wire.part_set_sha256,
            wire.transform_set_sha256,
        )?;
        // `excluded_file_count` 与 transformer 三元组无关，两条分支都要带过去：
        // 一个没有 transformer 的适配器同样可以丢弃非游戏资源文件。
        let facts = facts.with_excluded_file_count(wire.excluded_file_count);
        if wire.transformer_identities.is_empty() && wire.part_count == 0 && wire.file_count == 0 {
            Ok(facts)
        } else {
            facts.with_transformers(
                wire.transformer_identities,
                wire.part_count,
                wire.file_count,
            )
        }
    }
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

fn is_bounded_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TRANSFORM_IDENTIFIER_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

macro_rules! validated_string_id {
    ($name:ident, $empty_error:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self, ReplacementError> {
                let value = value.into();
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err($empty_error);
                }

                Ok(Self(trimmed.to_owned()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = ReplacementError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::parse(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

validated_string_id!(ReplacementTargetId, ReplacementError::EmptyTargetId);
validated_string_id!(ReplacementBindingId, ReplacementError::EmptyBindingId);
validated_string_id!(ReplacementSourceId, ReplacementError::EmptySourceId);
validated_string_id!(ReplacementTargetKind, ReplacementError::EmptyTargetKind);
validated_string_id!(
    ReplacementCatalogVersion,
    ReplacementError::EmptyCatalogVersion
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    try_from = "BTreeMap<String, String>",
    into = "BTreeMap<String, String>"
)]
pub struct LocalizedText(BTreeMap<String, String>);

impl LocalizedText {
    pub fn new(values: BTreeMap<String, String>) -> Result<Self, ReplacementError> {
        if values.is_empty() {
            return Err(ReplacementError::EmptyLocalizedText);
        }

        let mut normalized = BTreeMap::new();
        for (locale, value) in values {
            let locale = locale.trim();
            if locale.is_empty() {
                return Err(ReplacementError::EmptyLocale);
            }
            if value.trim().is_empty() {
                return Err(ReplacementError::EmptyLocalizedValue {
                    locale: locale.to_owned(),
                });
            }
            if normalized.insert(locale.to_owned(), value).is_some() {
                return Err(ReplacementError::DuplicateLocale {
                    locale: locale.to_owned(),
                });
            }
        }

        Ok(Self(normalized))
    }

    pub fn get(&self, locale: &str) -> Option<&str> {
        self.0.get(locale).map(String::as_str)
    }

    pub fn values(&self) -> impl Iterator<Item = &str> {
        self.0.values().map(String::as_str)
    }
}

impl TryFrom<BTreeMap<String, String>> for LocalizedText {
    type Error = ReplacementError;

    fn try_from(values: BTreeMap<String, String>) -> Result<Self, Self::Error> {
        Self::new(values)
    }
}

impl From<LocalizedText> for BTreeMap<String, String> {
    fn from(value: LocalizedText) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ReplacementTargetWire")]
pub struct ReplacementTarget {
    id: ReplacementTargetId,
    game_id: GameId,
    target_type: ReplacementTargetKind,
    display_name: LocalizedText,
    aliases: Vec<String>,
    /// 按 locale 分组的别名。只有来源本身按语言给出别名时才有值（武器 catalog）；
    /// 铠甲 catalog 的别名是一张不带语言的平表，这里保持 `None`——「不知道」不伪造成空表。
    /// `aliases` 仍是跨语言压平的检索平表，两者并存：前者供展示，后者供匹配。
    #[serde(skip_serializing_if = "Option::is_none")]
    localized_aliases: Option<BTreeMap<String, Vec<String>>>,
    internal_id: String,
    metadata: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct ReplacementTargetWire {
    id: ReplacementTargetId,
    game_id: GameId,
    target_type: ReplacementTargetKind,
    display_name: LocalizedText,
    aliases: Vec<String>,
    #[serde(default)]
    localized_aliases: Option<BTreeMap<String, Vec<String>>>,
    internal_id: String,
    metadata: BTreeMap<String, Value>,
}

impl ReplacementTarget {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: ReplacementTargetId,
        game_id: GameId,
        target_type: ReplacementTargetKind,
        display_name: LocalizedText,
        aliases: Vec<String>,
        internal_id: impl Into<String>,
        metadata: BTreeMap<String, Value>,
    ) -> Result<Self, ReplacementError> {
        let internal_id = internal_id.into();
        let internal_id = internal_id.trim();
        if internal_id.is_empty() {
            return Err(ReplacementError::EmptyInternalId);
        }

        let aliases = aliases
            .into_iter()
            .map(|alias| {
                let trimmed = alias.trim();
                if trimmed.is_empty() {
                    Err(ReplacementError::EmptyAlias)
                } else {
                    Ok(trimmed.to_owned())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            id,
            game_id,
            target_type,
            display_name,
            aliases,
            localized_aliases: None,
            internal_id: internal_id.to_owned(),
            metadata,
        })
    }

    /// 附加按 locale 分组的别名。校验 fail closed：locale 必须已有展示名（前端沿展示名的
    /// fallback 链取词，孤儿 locale 永远不会被显示），每个别名都必须出现在 `aliases` 平表里
    /// （行内展示的名字必须能被搜索命中，否则又回到「看得见却搜不到」）。
    pub fn with_localized_aliases(
        mut self,
        localized_aliases: BTreeMap<String, Vec<String>>,
    ) -> Result<Self, ReplacementError> {
        let searchable: BTreeSet<&str> = self.aliases.iter().map(String::as_str).collect();
        let mut normalized = BTreeMap::new();
        for (locale, aliases) in localized_aliases {
            let locale = locale.trim();
            if locale.is_empty() {
                return Err(ReplacementError::EmptyLocale);
            }
            if self.display_name.get(locale).is_none() {
                return Err(ReplacementError::LocalizedAliasLocaleUnknown {
                    locale: locale.to_owned(),
                });
            }
            let aliases = aliases
                .into_iter()
                .map(|alias| {
                    let trimmed = alias.trim();
                    if trimmed.is_empty() {
                        return Err(ReplacementError::EmptyAlias);
                    }
                    if !searchable.contains(trimmed) {
                        return Err(ReplacementError::LocalizedAliasNotSearchable {
                            alias: trimmed.to_owned(),
                        });
                    }
                    Ok(trimmed.to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            normalized.insert(locale.to_owned(), aliases);
        }
        self.localized_aliases = Some(normalized);
        Ok(self)
    }

    pub fn id(&self) -> &ReplacementTargetId {
        &self.id
    }

    pub fn game_id(&self) -> &GameId {
        &self.game_id
    }

    pub fn target_type(&self) -> &ReplacementTargetKind {
        &self.target_type
    }

    pub fn display_name(&self) -> &LocalizedText {
        &self.display_name
    }

    pub fn aliases(&self) -> &[String] {
        &self.aliases
    }

    /// 按 locale 分组的别名；来源不按语言给别名时为 `None`（区别于「每个语言都是空表」）。
    pub fn localized_aliases(&self) -> Option<&BTreeMap<String, Vec<String>>> {
        self.localized_aliases.as_ref()
    }

    pub fn internal_id(&self) -> &str {
        &self.internal_id
    }

    pub fn metadata(&self) -> &BTreeMap<String, Value> {
        &self.metadata
    }
}

impl TryFrom<ReplacementTargetWire> for ReplacementTarget {
    type Error = ReplacementError;

    fn try_from(wire: ReplacementTargetWire) -> Result<Self, Self::Error> {
        let target = Self::new(
            wire.id,
            wire.game_id,
            wire.target_type,
            wire.display_name,
            wire.aliases,
            wire.internal_id,
            wire.metadata,
        )?;
        match wire.localized_aliases {
            Some(localized_aliases) => target.with_localized_aliases(localized_aliases),
            None => Ok(target),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ReplacementBindingWire")]
pub struct ReplacementBinding {
    id: ReplacementBindingId,
    mod_id: ModId,
    profile_id: ProfileId,
    source_id: ReplacementSourceId,
    target_id: ReplacementTargetId,
    #[serde(serialize_with = "serialize_unix_millis")]
    created_at_unix_millis: u128,
}

#[derive(Deserialize)]
struct ReplacementBindingWire {
    id: ReplacementBindingId,
    mod_id: ModId,
    profile_id: ProfileId,
    source_id: ReplacementSourceId,
    target_id: ReplacementTargetId,
    created_at_unix_millis: u64,
}

impl ReplacementBinding {
    pub fn new(
        id: ReplacementBindingId,
        mod_id: ModId,
        profile_id: ProfileId,
        source_id: ReplacementSourceId,
        target_id: ReplacementTargetId,
        created_at_unix_millis: u128,
    ) -> Result<Self, ReplacementError> {
        if mod_id.as_str().trim().is_empty() {
            return Err(ReplacementError::EmptyModId);
        }
        if profile_id.as_str().trim().is_empty() {
            return Err(ReplacementError::EmptyProfileId);
        }

        Ok(Self {
            id,
            mod_id,
            profile_id,
            source_id,
            target_id,
            created_at_unix_millis,
        })
    }

    pub fn id(&self) -> &ReplacementBindingId {
        &self.id
    }

    pub fn mod_id(&self) -> &ModId {
        &self.mod_id
    }

    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    pub fn source_id(&self) -> &ReplacementSourceId {
        &self.source_id
    }

    pub fn target_id(&self) -> &ReplacementTargetId {
        &self.target_id
    }

    pub fn created_at_unix_millis(&self) -> u128 {
        self.created_at_unix_millis
    }
}

impl TryFrom<ReplacementBindingWire> for ReplacementBinding {
    type Error = ReplacementError;

    fn try_from(wire: ReplacementBindingWire) -> Result<Self, Self::Error> {
        Self::new(
            wire.id,
            wire.mod_id,
            wire.profile_id,
            wire.source_id,
            wire.target_id,
            u128::from(wire.created_at_unix_millis),
        )
    }
}

fn serialize_unix_millis<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let value = u64::try_from(*value)
        .map_err(|_| serde::ser::Error::custom("Unix millisecond timestamp exceeds u64"))?;
    serializer.serialize_u64(value)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ReplacementBindingSnapshotWire")]
pub struct ReplacementBindingSnapshot {
    binding: ReplacementBinding,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    revision_id: Option<ModRevisionId>,
    source_internal_id: String,
    target_internal_id: String,
    source_path_family: String,
    target_path_family: String,
    retarget_kind: ReplacementTargetKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    adapter_facts: Option<ReplacementAdapterFacts>,
}

#[derive(Deserialize)]
struct ReplacementBindingSnapshotWire {
    binding: ReplacementBinding,
    #[serde(default)]
    revision_id: Option<ModRevisionId>,
    source_internal_id: String,
    target_internal_id: String,
    source_path_family: String,
    target_path_family: String,
    retarget_kind: ReplacementTargetKind,
    #[serde(default)]
    adapter_facts: Option<ReplacementAdapterFacts>,
}

impl ReplacementBindingSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        binding: ReplacementBinding,
        revision_id: Option<ModRevisionId>,
        source_internal_id: impl Into<String>,
        target_internal_id: impl Into<String>,
        source_path_family: impl Into<String>,
        target_path_family: impl Into<String>,
        retarget_kind: ReplacementTargetKind,
    ) -> Result<Self, ReplacementError> {
        Ok(Self {
            binding,
            revision_id,
            source_internal_id: required_snapshot_field(
                source_internal_id.into(),
                ReplacementError::EmptySnapshotSourceInternalId,
            )?,
            target_internal_id: required_snapshot_field(
                target_internal_id.into(),
                ReplacementError::EmptySnapshotTargetInternalId,
            )?,
            source_path_family: required_snapshot_field(
                source_path_family.into(),
                ReplacementError::EmptySnapshotSourcePathFamily,
            )?,
            target_path_family: required_snapshot_field(
                target_path_family.into(),
                ReplacementError::EmptySnapshotTargetPathFamily,
            )?,
            retarget_kind,
            adapter_facts: None,
        })
    }

    pub fn from_retarget_plan(plan: &RetargetPlan, revision_id: Option<ModRevisionId>) -> Self {
        let action = &plan.actions()[0];
        let mut snapshot = Self::new(
            plan.binding().clone(),
            revision_id,
            plan.source().internal_id(),
            action.target_internal_id(),
            plan.source().path_family(),
            action.target_path_family(),
            plan.source().source_type().clone(),
        )
        .expect("validated RetargetPlan facts produce a valid replacement snapshot");
        snapshot.adapter_facts = plan.adapter_facts().cloned();
        snapshot
    }

    pub fn binding(&self) -> &ReplacementBinding {
        &self.binding
    }

    pub fn binding_id(&self) -> &ReplacementBindingId {
        self.binding.id()
    }

    pub fn mod_id(&self) -> &ModId {
        self.binding.mod_id()
    }

    pub fn profile_id(&self) -> &ProfileId {
        self.binding.profile_id()
    }

    pub fn revision_id(&self) -> Option<&ModRevisionId> {
        self.revision_id.as_ref()
    }

    pub fn source_internal_id(&self) -> &str {
        &self.source_internal_id
    }

    pub fn target_internal_id(&self) -> &str {
        &self.target_internal_id
    }

    pub fn source_path_family(&self) -> &str {
        &self.source_path_family
    }

    pub fn target_path_family(&self) -> &str {
        &self.target_path_family
    }

    pub fn retarget_kind(&self) -> &ReplacementTargetKind {
        &self.retarget_kind
    }

    pub fn adapter_facts(&self) -> Option<&ReplacementAdapterFacts> {
        self.adapter_facts.as_ref()
    }

    pub fn with_adapter_facts(mut self, adapter_facts: ReplacementAdapterFacts) -> Self {
        self.adapter_facts = Some(adapter_facts);
        self
    }
}

impl TryFrom<ReplacementBindingSnapshotWire> for ReplacementBindingSnapshot {
    type Error = ReplacementError;

    fn try_from(wire: ReplacementBindingSnapshotWire) -> Result<Self, Self::Error> {
        let mut snapshot = Self::new(
            wire.binding,
            wire.revision_id,
            wire.source_internal_id,
            wire.target_internal_id,
            wire.source_path_family,
            wire.target_path_family,
            wire.retarget_kind,
        )?;
        snapshot.adapter_facts = wire.adapter_facts;
        Ok(snapshot)
    }
}

fn required_snapshot_field(
    value: String,
    error: ReplacementError,
) -> Result<String, ReplacementError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(error);
    }
    Ok(value.to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ReplacementCatalogWire")]
pub struct ReplacementCatalog {
    version: ReplacementCatalogVersion,
    game_id: GameId,
    targets: Vec<ReplacementTarget>,
}

#[derive(Deserialize)]
struct ReplacementCatalogWire {
    version: ReplacementCatalogVersion,
    game_id: GameId,
    targets: Vec<ReplacementTarget>,
}

impl ReplacementCatalog {
    pub fn new(
        version: ReplacementCatalogVersion,
        game_id: GameId,
        targets: Vec<ReplacementTarget>,
    ) -> Result<Self, ReplacementError> {
        if targets.is_empty() {
            return Err(ReplacementError::EmptyCatalog);
        }

        let mut target_ids = BTreeSet::new();
        for target in &targets {
            if target.game_id() != &game_id {
                return Err(ReplacementError::TargetGameMismatch {
                    target_id: target.id().as_str().to_owned(),
                    expected_game_id: game_id.as_str().to_owned(),
                    actual_game_id: target.game_id().as_str().to_owned(),
                });
            }
            if !target_ids.insert(target.id().clone()) {
                return Err(ReplacementError::DuplicateTargetId {
                    target_id: target.id().as_str().to_owned(),
                });
            }
        }

        Ok(Self {
            version,
            game_id,
            targets,
        })
    }

    pub fn version(&self) -> &ReplacementCatalogVersion {
        &self.version
    }

    pub fn game_id(&self) -> &GameId {
        &self.game_id
    }

    pub fn targets(&self) -> &[ReplacementTarget] {
        &self.targets
    }

    pub fn find(&self, target_id: &ReplacementTargetId) -> Option<&ReplacementTarget> {
        self.targets.iter().find(|target| target.id() == target_id)
    }
}

impl TryFrom<ReplacementCatalogWire> for ReplacementCatalog {
    type Error = ReplacementError;

    fn try_from(wire: ReplacementCatalogWire) -> Result<Self, Self::Error> {
        Self::new(wire.version, wire.game_id, wire.targets)
    }
}
