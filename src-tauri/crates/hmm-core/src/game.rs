use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

pub const MHW_GAME_ID: &str = "mhw";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GameIdError {
    #[error("game id cannot be empty")]
    Empty,
    #[error("unsupported game id: {0}")]
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GameId(String);

impl GameId {
    fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, GameIdError> {
        let value = value.into();
        let trimmed = value.trim();

        if trimmed.is_empty() {
            return Err(GameIdError::Empty);
        }

        if trimmed != MHW_GAME_ID {
            return Err(GameIdError::Unsupported(trimmed.to_owned()));
        }

        Ok(Self::new(trimmed.to_owned()))
    }

    pub fn mhw() -> Self {
        Self::new(MHW_GAME_ID.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GameId {
    type Error = GameIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<GameId> for String {
    fn from(value: GameId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameDirectoryStatus {
    NotConfigured,
    Invalid,
    Configured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameDirectoryEvidenceKind {
    DirectoryExists,
    DirectoryMissing,
    FoundExecutable,
    MissingExecutable,
    FoundNativePc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameDirectoryEvidence {
    pub kind: GameDirectoryEvidenceKind,
    pub label: String,
}

impl GameDirectoryEvidence {
    pub fn new(kind: GameDirectoryEvidenceKind, label: impl Into<String>) -> Self {
        Self {
            kind,
            label: label.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameSetupErrorCode {
    UnsupportedGame,
    DirectoryNotFound,
    DirectoryNotAbsolute,
    MissingExecutable,
    StorageFailed,
    StorageCorrupted,
    ScanFailed,
    ScanNotImplemented,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameDirectoryValidation {
    pub game_id: GameId,
    pub directory: PathBuf,
    pub is_valid: bool,
    pub confidence: u8,
    pub evidence: Vec<GameDirectoryEvidence>,
    pub errors: Vec<GameSetupErrorCode>,
}

impl GameDirectoryValidation {
    pub fn new(game_id: GameId, directory: PathBuf) -> Self {
        Self {
            game_id,
            directory,
            is_valid: true,
            confidence: 0,
            evidence: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn add_evidence(&mut self, evidence: GameDirectoryEvidence) {
        self.evidence.push(evidence);
    }

    pub fn add_error(&mut self, error: GameSetupErrorCode) {
        self.is_valid = false;
        self.errors.push(error);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameInstance {
    pub id: String,
    pub game_id: GameId,
    pub display_name: String,
    pub root_dir: PathBuf,
    pub status: GameDirectoryStatus,
    pub configured_at_unix_millis: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSetupStatus {
    pub game_id: GameId,
    pub status: GameDirectoryStatus,
    pub instance: Option<GameInstance>,
    pub error_code: Option<GameSetupErrorCode>,
    pub message: Option<String>,
}

impl GameSetupStatus {
    pub fn not_configured(game_id: GameId) -> Self {
        Self {
            game_id,
            status: GameDirectoryStatus::NotConfigured,
            instance: None,
            error_code: None,
            message: None,
        }
    }

    pub fn configured(instance: GameInstance) -> Self {
        Self {
            game_id: instance.game_id.clone(),
            status: GameDirectoryStatus::Configured,
            instance: Some(instance),
            error_code: None,
            message: None,
        }
    }

    pub fn invalid(
        game_id: GameId,
        error_code: GameSetupErrorCode,
        message: impl Into<String>,
    ) -> Self {
        Self {
            game_id,
            status: GameDirectoryStatus::Invalid,
            instance: None,
            error_code: Some(error_code),
            message: Some(message.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_game_id() {
        let id = GameId::parse("mhw").expect("mhw should be supported");
        assert_eq!(id.as_str(), "mhw");
    }

    #[test]
    fn rejects_empty_game_id() {
        let result = GameId::parse(" ");
        assert_eq!(result, Err(GameIdError::Empty));
    }

    #[test]
    fn rejects_unsupported_game_id() {
        let result = GameId::parse("rise");
        assert_eq!(result, Err(GameIdError::Unsupported("rise".to_owned())));
    }

    #[test]
    fn rejects_unsupported_game_id_during_deserialization() {
        let result = serde_json::from_str::<GameId>(r#""rise""#);

        assert!(result.is_err());
    }

    #[test]
    fn validation_becomes_invalid_after_error() {
        let mut validation = GameDirectoryValidation::new(GameId::mhw(), PathBuf::from("C:/Game"));

        validation.add_error(GameSetupErrorCode::MissingExecutable);

        assert!(!validation.is_valid);
        assert_eq!(
            validation.errors,
            vec![GameSetupErrorCode::MissingExecutable]
        );
    }

    #[test]
    fn configured_status_wraps_instance() {
        let instance = GameInstance {
            id: "mhw-default".to_owned(),
            game_id: GameId::mhw(),
            display_name: "Monster Hunter: World - Iceborne".to_owned(),
            root_dir: PathBuf::from("C:/Game"),
            status: GameDirectoryStatus::Configured,
            configured_at_unix_millis: 1,
        };

        let status = GameSetupStatus::configured(instance);

        assert_eq!(status.status, GameDirectoryStatus::Configured);
        assert!(status.instance.is_some());
    }
}
