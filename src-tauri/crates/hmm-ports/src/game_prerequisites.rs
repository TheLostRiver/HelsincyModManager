use hmm_core::{GameId, GameSetupErrorCode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePrerequisiteReportState {
    NotConfigured,
    GameDirectoryInvalid,
    RulesUnavailable,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePrerequisiteItemStatus {
    Missing,
    Misconfigured,
    InstalledVerified,
    InstalledUnverified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePrerequisiteSummaryStatus {
    Verified,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePrerequisiteIssueCode {
    MissingRequiredFile,
    SignatureUnverified,
    ConfigReadFailed,
    ConfigInvalidJson,
    ConfigFieldMismatch,
    RulesUnavailable,
    RulesCorrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePrerequisiteIssue {
    pub code: GamePrerequisiteIssueCode,
    pub path: String,
}

impl GamePrerequisiteIssue {
    pub fn new(code: GamePrerequisiteIssueCode, path: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePrerequisiteItem {
    pub id: String,
    pub display_name: String,
    pub status: GamePrerequisiteItemStatus,
    pub issues: Vec<GamePrerequisiteIssue>,
}

impl GamePrerequisiteItem {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        status: GamePrerequisiteItemStatus,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            status,
            issues: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePrerequisiteReport {
    pub game_id: GameId,
    pub state: GamePrerequisiteReportState,
    pub summary_status: Option<GamePrerequisiteSummaryStatus>,
    pub items: Vec<GamePrerequisiteItem>,
    pub error_code: Option<GameSetupErrorCode>,
    pub message: Option<String>,
}

impl GamePrerequisiteReport {
    pub fn not_configured(game_id: GameId) -> Self {
        Self {
            game_id,
            state: GamePrerequisiteReportState::NotConfigured,
            summary_status: None,
            items: Vec::new(),
            error_code: None,
            message: None,
        }
    }

    pub fn game_directory_invalid(
        game_id: GameId,
        error_code: GameSetupErrorCode,
        message: impl Into<String>,
    ) -> Self {
        Self {
            game_id,
            state: GamePrerequisiteReportState::GameDirectoryInvalid,
            summary_status: None,
            items: Vec::new(),
            error_code: Some(error_code),
            message: Some(message.into()),
        }
    }

    pub fn ready(
        game_id: GameId,
        summary_status: GamePrerequisiteSummaryStatus,
        items: Vec<GamePrerequisiteItem>,
    ) -> Self {
        Self {
            game_id,
            state: GamePrerequisiteReportState::Ready,
            summary_status: Some(summary_status),
            items,
            error_code: None,
            message: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePrerequisiteRuleSet {
    pub version: u32,
    pub game_id: GameId,
    pub prerequisites: Vec<GamePrerequisiteRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePrerequisiteRule {
    pub id: String,
    pub display_name: String,
    pub required_files: Vec<String>,
    pub signature_files: Vec<GamePrerequisiteSignatureRule>,
    pub json_checks: Vec<GamePrerequisiteJsonCheckRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePrerequisiteSignatureRule {
    pub path: String,
    pub sha256: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePrerequisiteJsonCheckRule {
    pub path: String,
    pub required_boolean_fields: BTreeMap<String, bool>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GamePrerequisiteRuleRepositoryError {
    #[error("storage failed: {0}")]
    StorageFailed(String),
    #[error("storage corrupted")]
    StorageCorrupted,
}

pub trait GamePrerequisiteRuleRepository: Send + Sync {
    fn load_rules(
        &self,
        game_id: &GameId,
        bundled_default: &str,
    ) -> Result<GamePrerequisiteRuleSet, GamePrerequisiteRuleRepositoryError>;
}

pub fn summarize_prerequisite_items(
    items: &[GamePrerequisiteItem],
) -> GamePrerequisiteSummaryStatus {
    if items.iter().any(|item| {
        matches!(
            item.status,
            GamePrerequisiteItemStatus::Missing | GamePrerequisiteItemStatus::Misconfigured
        )
    }) {
        return GamePrerequisiteSummaryStatus::Error;
    }

    if items
        .iter()
        .any(|item| item.status == GamePrerequisiteItemStatus::InstalledUnverified)
    {
        return GamePrerequisiteSummaryStatus::Warning;
    }

    GamePrerequisiteSummaryStatus::Verified
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_items_treats_missing_as_error() {
        let items = vec![
            GamePrerequisiteItem::new(
                "stracker_loader",
                "Stracker's Loader",
                GamePrerequisiteItemStatus::InstalledVerified,
            ),
            GamePrerequisiteItem::new(
                "crc_bypass",
                "CRCBypass",
                GamePrerequisiteItemStatus::Missing,
            ),
        ];

        assert_eq!(
            summarize_prerequisite_items(&items),
            GamePrerequisiteSummaryStatus::Error
        );
    }

    #[test]
    fn summarize_items_treats_misconfigured_as_error() {
        let items = vec![
            GamePrerequisiteItem::new(
                "stracker_loader",
                "Stracker's Loader",
                GamePrerequisiteItemStatus::InstalledVerified,
            ),
            GamePrerequisiteItem::new(
                "crc_bypass",
                "CRCBypass",
                GamePrerequisiteItemStatus::Misconfigured,
            ),
        ];

        assert_eq!(
            summarize_prerequisite_items(&items),
            GamePrerequisiteSummaryStatus::Error
        );
    }

    #[test]
    fn prerequisite_rule_file_uses_stable_json_field_names() {
        let value: GamePrerequisiteRuleSet = serde_json::from_str(
            r#"{
              "version": 1,
              "gameId": "mhw",
              "prerequisites": [
                {
                  "id": "stracker_loader",
                  "displayName": "Stracker's Loader",
                  "requiredFiles": ["dinput8.dll"],
                  "signatureFiles": [{"path": "dinput8.dll", "sha256": ["abc"]}],
                  "jsonChecks": [{"path": "loader-config.json", "requiredBooleanFields": {"enablePluginLoader": true}}]
                }
              ]
            }"#,
        )
        .expect("schema should deserialize");

        assert_eq!(value.version, 1);
        assert_eq!(
            value.prerequisites[0].required_files,
            vec!["dinput8.dll".to_owned()]
        );
    }
}
