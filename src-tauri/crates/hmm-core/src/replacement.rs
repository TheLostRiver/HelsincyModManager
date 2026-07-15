use crate::{GameId, ModId, ProfileId};
use serde::{Deserialize, Serialize};
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
    #[error("replacement binding mod id cannot be empty")]
    EmptyModId,
    #[error("replacement binding profile id cannot be empty")]
    EmptyProfileId,
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
            internal_id: internal_id.to_owned(),
            metadata,
        })
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
        Self::new(
            wire.id,
            wire.game_id,
            wire.target_type,
            wire.display_name,
            wire.aliases,
            wire.internal_id,
            wire.metadata,
        )
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
    created_at_unix_millis: u128,
}

#[derive(Deserialize)]
struct ReplacementBindingWire {
    id: ReplacementBindingId,
    mod_id: ModId,
    profile_id: ProfileId,
    source_id: ReplacementSourceId,
    target_id: ReplacementTargetId,
    created_at_unix_millis: u128,
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
            wire.created_at_unix_millis,
        )
    }
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
