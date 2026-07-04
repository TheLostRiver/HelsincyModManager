use crate::dto::ProfileDirectorySelectionDto;
use hmm_core::{
    SaveDirectoryCandidateConfidence, SaveDirectoryCandidateSource, SaveDirectoryCandidateSummary,
    SaveDirectoryDiscoveryOutcome, SaveDirectoryDiscoveryResult,
};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDirectoryDiscoveryDto {
    pub discovery_id: String,
    pub game_id: String,
    pub profile_id: String,
    pub outcome: String,
    pub recommended_candidate_id: Option<String>,
    pub candidates: Vec<SaveDirectoryCandidateDto>,
    pub saved_settings: Option<ProfileDirectorySelectionDto>,
    pub error_code: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDirectoryCandidateDto {
    pub candidate_id: String,
    pub source: String,
    pub confidence: String,
    pub recommended: bool,
    pub account_name: Option<String>,
    pub avatar_url: Option<String>,
    pub account_label: String,
    pub path_label: String,
    pub last_modified_at: Option<u128>,
    pub evidence: Vec<String>,
}

impl From<SaveDirectoryDiscoveryResult> for SaveDirectoryDiscoveryDto {
    fn from(result: SaveDirectoryDiscoveryResult) -> Self {
        Self {
            discovery_id: result.discovery_id,
            game_id: result.game_id.as_str().to_owned(),
            profile_id: result.profile_id.as_str().to_owned(),
            outcome: outcome_to_string(result.outcome),
            recommended_candidate_id: result.recommended_candidate_id,
            candidates: result
                .candidates
                .into_iter()
                .map(SaveDirectoryCandidateDto::from)
                .collect(),
            saved_settings: result
                .saved_settings
                .map(ProfileDirectorySelectionDto::from),
            error_code: result.error_code,
        }
    }
}

impl From<SaveDirectoryCandidateSummary> for SaveDirectoryCandidateDto {
    fn from(candidate: SaveDirectoryCandidateSummary) -> Self {
        Self {
            candidate_id: candidate.candidate_id,
            source: source_to_string(candidate.source),
            confidence: confidence_to_string(candidate.confidence),
            recommended: candidate.recommended,
            account_name: candidate.account_name,
            avatar_url: candidate.avatar_url,
            account_label: candidate.account_label,
            path_label: candidate.path_label,
            last_modified_at: candidate.last_modified_at,
            evidence: candidate.evidence,
        }
    }
}

fn outcome_to_string(outcome: SaveDirectoryDiscoveryOutcome) -> String {
    match outcome {
        SaveDirectoryDiscoveryOutcome::AutoSaved => "auto_saved",
        SaveDirectoryDiscoveryOutcome::ConfirmationRequired => "confirmation_required",
        SaveDirectoryDiscoveryOutcome::NotFound => "not_found",
        SaveDirectoryDiscoveryOutcome::ExistingValid => "existing_valid",
        SaveDirectoryDiscoveryOutcome::ExistingInvalid => "existing_invalid",
        SaveDirectoryDiscoveryOutcome::ScanFailed => "scan_failed",
    }
    .to_owned()
}

fn source_to_string(source: SaveDirectoryCandidateSource) -> String {
    match source {
        SaveDirectoryCandidateSource::SteamUserdata => "steam_userdata",
    }
    .to_owned()
}

fn confidence_to_string(confidence: SaveDirectoryCandidateConfidence) -> String {
    match confidence {
        SaveDirectoryCandidateConfidence::High => "high",
        SaveDirectoryCandidateConfidence::Medium => "medium",
        SaveDirectoryCandidateConfidence::Low => "low",
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        GameId, ProfileId, SaveDirectoryCandidateConfidence, SaveDirectoryCandidateSource,
        SaveDirectoryCandidateSummary, SaveDirectoryDiscoveryOutcome, SaveDirectoryDiscoveryResult,
    };
    use serde_json::Value;

    #[test]
    fn dto_serializes_without_raw_paths_or_steam_ids() {
        let result = SaveDirectoryDiscoveryResult {
            discovery_id: "discovery-a".to_owned(),
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            outcome: SaveDirectoryDiscoveryOutcome::ConfirmationRequired,
            recommended_candidate_id: Some("candidate-a".to_owned()),
            candidates: vec![SaveDirectoryCandidateSummary {
                candidate_id: "candidate-a".to_owned(),
                source: SaveDirectoryCandidateSource::SteamUserdata,
                confidence: SaveDirectoryCandidateConfidence::High,
                recommended: true,
                account_name: Some("Hunter".to_owned()),
                avatar_url: Some(
                    "https://avatars.akamai.steamstatic.com/example_medium.jpg".to_owned(),
                ),
                account_label: "Steam user ****1234".to_owned(),
                path_label: "Steam/userdata/<account>/582010/remote".to_owned(),
                last_modified_at: Some(1_000),
                evidence: vec!["Found MHW:I save file".to_owned()],
            }],
            saved_settings: None,
            error_code: None,
        };

        let value: Value =
            serde_json::to_value(SaveDirectoryDiscoveryDto::from(result)).expect("json");
        let serialized = value.to_string();

        assert_eq!(value["outcome"], "confirmation_required");
        assert_eq!(value["candidates"][0]["accountName"], "Hunter");
        assert!(!serialized.contains("C:/"));
        assert!(!serialized.contains(&["765", "6119"].concat()));
        assert!(!serialized.contains("1234/582010"));
    }
}
