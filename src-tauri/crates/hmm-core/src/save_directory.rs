use crate::{GameId, ProfileDirectorySelection, ProfileId};

pub const STEAM_ID64_ACCOUNT_ID_OFFSET: u64 = 76_561_197_960_265_728;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SaveDirectoryDiscoveryOutcome {
    AutoSaved,
    ConfirmationRequired,
    NotFound,
    ExistingValid,
    ExistingInvalid,
    ScanFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SaveDirectoryCandidateSource {
    SteamUserdata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum SaveDirectoryCandidateConfidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveDirectoryCandidateSummary {
    pub candidate_id: String,
    pub source: SaveDirectoryCandidateSource,
    pub confidence: SaveDirectoryCandidateConfidence,
    pub recommended: bool,
    pub account_name: Option<String>,
    pub avatar_url: Option<String>,
    pub account_label: String,
    pub path_label: String,
    pub last_modified_at: Option<u128>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveDirectoryDiscoveryResult {
    pub discovery_id: String,
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub outcome: SaveDirectoryDiscoveryOutcome,
    pub recommended_candidate_id: Option<String>,
    pub candidates: Vec<SaveDirectoryCandidateSummary>,
    pub saved_settings: Option<ProfileDirectorySelection>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamAccountProfileSummary {
    pub account_name: Option<String>,
    pub avatar_url: Option<String>,
}

pub fn steam_id64_from_account_id32(account_id_32: u32) -> u64 {
    STEAM_ID64_ACCOUNT_ID_OFFSET + u64::from(account_id_32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_id64_from_account_id32_uses_public_offset() {
        assert_eq!(steam_id64_from_account_id32(1), 76_561_197_960_265_729);
        assert_eq!(
            steam_id64_from_account_id32(u32::MAX),
            76_561_202_255_233_023
        );
    }

    #[test]
    fn discovery_result_marks_recommended_candidate() {
        let result = SaveDirectoryDiscoveryResult {
            discovery_id: "discovery-a".to_owned(),
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            outcome: SaveDirectoryDiscoveryOutcome::ConfirmationRequired,
            recommended_candidate_id: Some("candidate-newer".to_owned()),
            candidates: vec![SaveDirectoryCandidateSummary {
                candidate_id: "candidate-newer".to_owned(),
                source: SaveDirectoryCandidateSource::SteamUserdata,
                confidence: SaveDirectoryCandidateConfidence::High,
                recommended: true,
                account_name: Some("Hunter".to_owned()),
                avatar_url: None,
                account_label: "Steam user ****1234".to_owned(),
                path_label: "Steam/userdata/<account>/582010/remote".to_owned(),
                last_modified_at: Some(2_000),
                evidence: vec!["Found MHW:I save file".to_owned()],
            }],
            saved_settings: None,
            error_code: None,
        };

        assert_eq!(
            result.recommended_candidate_id.as_deref(),
            Some("candidate-newer")
        );
        assert!(result.candidates[0].recommended);
    }
}
