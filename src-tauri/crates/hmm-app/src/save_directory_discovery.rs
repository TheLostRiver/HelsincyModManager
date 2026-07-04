use hmm_core::{
    GameDirectoryStatus, GameId, ProfileBackupRetention, ProfileBackupSchedule,
    ProfileDirectoryMode, ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId,
    ProfileSaveSettings, SaveDirectoryCandidateConfidence, SaveDirectoryCandidateSource,
    SaveDirectoryCandidateSummary, SaveDirectoryDiscoveryOutcome, SaveDirectoryDiscoveryResult,
};
use hmm_ports::{
    AppClock, GameConfigRepository, GameSaveDirectoryRule, PendingSaveDirectoryCandidate,
    PendingSaveDirectoryCandidateStore, PendingSaveDirectoryDiscovery, ProfileRepository,
    ProfileSaveDirectoryValidator, ProfileSaveSettingsRepository, ScannedSaveDirectoryCandidate,
    SteamAccountProfileClient, SteamUserdataScanRequest, SteamUserdataScanner,
};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

const DISCOVERY_TTL_MILLIS: u128 = 10 * 60 * 1000;
const STEAM_PROFILE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverProfileSaveDirectoriesRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmProfileSaveDirectoryCandidateRequest {
    pub discovery_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SaveDirectoryDiscoveryError {
    #[error("profile is missing")]
    ProfileMissing,
    #[error("save directory discovery repository is unavailable")]
    RepositoryUnavailable,
    #[error("save directory discovery clock is unavailable")]
    ClockUnavailable,
    #[error("save directory discovery rule is unavailable")]
    RuleUnavailable,
    #[error("save directory discovery candidate expired")]
    CandidateExpired,
    #[error("save directory discovery candidate is invalid")]
    CandidateInvalid,
    #[error("save directory discovery pending store is unavailable")]
    PendingStoreUnavailable,
    #[error("save directory discovery settings are unavailable")]
    SettingsUnavailable,
}

impl SaveDirectoryDiscoveryError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProfileMissing => "save_directory_discovery_profile_missing",
            Self::RepositoryUnavailable => "save_directory_discovery_repository_unavailable",
            Self::ClockUnavailable => "save_directory_discovery_clock_unavailable",
            Self::RuleUnavailable => "save_directory_discovery_rule_unavailable",
            Self::CandidateExpired => "save_directory_discovery_candidate_expired",
            Self::CandidateInvalid => "save_directory_discovery_candidate_invalid",
            Self::PendingStoreUnavailable => "save_directory_discovery_pending_store_unavailable",
            Self::SettingsUnavailable => "save_directory_discovery_settings_unavailable",
        }
    }
}

pub struct ProfileSaveDirectoryDiscoveryService {
    game_config_repository: Arc<dyn GameConfigRepository>,
    profile_repository: Arc<dyn ProfileRepository>,
    save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
    save_directory_validator: Arc<dyn ProfileSaveDirectoryValidator>,
    save_directory_rules: Vec<Arc<dyn GameSaveDirectoryRule>>,
    scanner: Arc<dyn SteamUserdataScanner>,
    profile_client: Arc<dyn SteamAccountProfileClient>,
    pending_store: Arc<dyn PendingSaveDirectoryCandidateStore>,
    clock: Arc<dyn AppClock>,
}

impl ProfileSaveDirectoryDiscoveryService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        game_config_repository: Arc<dyn GameConfigRepository>,
        profile_repository: Arc<dyn ProfileRepository>,
        save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
        save_directory_validator: Arc<dyn ProfileSaveDirectoryValidator>,
        save_directory_rules: Vec<Arc<dyn GameSaveDirectoryRule>>,
        scanner: Arc<dyn SteamUserdataScanner>,
        profile_client: Arc<dyn SteamAccountProfileClient>,
        pending_store: Arc<dyn PendingSaveDirectoryCandidateStore>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            game_config_repository,
            profile_repository,
            save_settings_repository,
            save_directory_validator,
            save_directory_rules,
            scanner,
            profile_client,
            pending_store,
            clock,
        }
    }

    pub fn discover(
        &self,
        request: DiscoverProfileSaveDirectoriesRequest,
    ) -> Result<SaveDirectoryDiscoveryResult, SaveDirectoryDiscoveryError> {
        self.ensure_profile(&request.profile_id)?;
        let discovery_id = new_discovery_id();
        let instance = match self
            .game_config_repository
            .load_game_instance(&request.game_id)
        {
            Ok(Some(instance)) if instance.status == GameDirectoryStatus::Configured => instance,
            Ok(_) => {
                return Ok(empty_result(
                    discovery_id,
                    request.game_id,
                    request.profile_id,
                    SaveDirectoryDiscoveryOutcome::ScanFailed,
                    Some("save_directory_discovery_game_unconfigured"),
                ));
            }
            Err(_) => {
                return Ok(empty_result(
                    discovery_id,
                    request.game_id,
                    request.profile_id,
                    SaveDirectoryDiscoveryOutcome::ScanFailed,
                    Some("save_directory_discovery_scan_failed"),
                ));
            }
        };
        let rule = self.rule_for(&request.game_id)?;
        let scan_request = scan_request_for(rule.as_ref(), Some(instance.root_dir.clone()));
        let existing_settings = self
            .save_settings_repository
            .get_settings(request.profile_id.as_str())
            .map_err(|_| SaveDirectoryDiscoveryError::SettingsUnavailable)?;

        if let Some(settings) = existing_settings.as_ref() {
            if let Some(directory) = settings.save_directory.directory.as_deref() {
                return match self
                    .scanner
                    .validate_save_directory(&scan_request, Path::new(directory))
                {
                    Ok(candidate)
                        if candidate.confidence >= SaveDirectoryCandidateConfidence::Medium =>
                    {
                        Ok(SaveDirectoryDiscoveryResult {
                            discovery_id,
                            game_id: request.game_id,
                            profile_id: request.profile_id,
                            outcome: SaveDirectoryDiscoveryOutcome::ExistingValid,
                            recommended_candidate_id: None,
                            candidates: vec![candidate_summary(candidate, None, false)],
                            saved_settings: Some(settings.save_directory.clone()),
                            error_code: None,
                        })
                    }
                    _ => Ok(SaveDirectoryDiscoveryResult {
                        discovery_id,
                        game_id: request.game_id,
                        profile_id: request.profile_id,
                        outcome: SaveDirectoryDiscoveryOutcome::ExistingInvalid,
                        recommended_candidate_id: None,
                        candidates: Vec::new(),
                        saved_settings: Some(settings.save_directory.clone()),
                        error_code: Some("save_directory_discovery_candidate_invalid".to_owned()),
                    }),
                };
            }
        }

        let mut scanned = match self.scanner.scan_save_directories(&scan_request) {
            Ok(candidates) => candidates,
            Err(_) => {
                return Ok(empty_result(
                    discovery_id,
                    request.game_id,
                    request.profile_id,
                    SaveDirectoryDiscoveryOutcome::ScanFailed,
                    Some("save_directory_discovery_scan_failed"),
                ));
            }
        };

        sort_candidates(&mut scanned);

        if scanned.is_empty() {
            return Ok(empty_result(
                discovery_id,
                request.game_id,
                request.profile_id,
                SaveDirectoryDiscoveryOutcome::NotFound,
                None,
            ));
        }

        if scanned.len() == 1 && scanned[0].confidence == SaveDirectoryCandidateConfidence::High {
            let settings = self.save_candidate_settings(
                &request.game_id,
                &request.profile_id,
                &scanned[0],
                existing_settings.as_ref(),
            )?;
            let summary = candidate_summary(scanned.remove(0), None, true);
            return Ok(SaveDirectoryDiscoveryResult {
                discovery_id,
                game_id: request.game_id,
                profile_id: request.profile_id,
                outcome: SaveDirectoryDiscoveryOutcome::AutoSaved,
                recommended_candidate_id: Some(summary.candidate_id.clone()),
                candidates: vec![summary],
                saved_settings: Some(settings.save_directory),
                error_code: None,
            });
        }

        let recommended_candidate_id = scanned
            .first()
            .map(|candidate| candidate.candidate_id.clone());
        let summaries = scanned
            .iter()
            .map(|candidate| {
                let profile = if scanned.len() > 1 {
                    self.profile_client
                        .fetch_profile(candidate.account_id_32, STEAM_PROFILE_TIMEOUT)
                        .ok()
                } else {
                    None
                };
                candidate_summary(
                    candidate.clone(),
                    profile,
                    Some(candidate.candidate_id.as_str()) == recommended_candidate_id.as_deref(),
                )
            })
            .collect::<Vec<_>>();
        let now = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveDirectoryDiscoveryError::ClockUnavailable)?;

        self.pending_store
            .put(PendingSaveDirectoryDiscovery {
                discovery_id: discovery_id.clone(),
                game_id: request.game_id.clone(),
                profile_id: request.profile_id.clone(),
                expires_at_unix_millis: now + DISCOVERY_TTL_MILLIS,
                candidates: scanned
                    .into_iter()
                    .zip(summaries.iter().cloned())
                    .map(|(candidate, summary)| PendingSaveDirectoryCandidate {
                        game_id: request.game_id.clone(),
                        profile_id: request.profile_id.clone(),
                        summary,
                        account_id_32: candidate.account_id_32,
                        directory: candidate.directory,
                    })
                    .collect(),
            })
            .map_err(|_| SaveDirectoryDiscoveryError::PendingStoreUnavailable)?;

        Ok(SaveDirectoryDiscoveryResult {
            discovery_id,
            game_id: request.game_id,
            profile_id: request.profile_id,
            outcome: SaveDirectoryDiscoveryOutcome::ConfirmationRequired,
            recommended_candidate_id,
            candidates: summaries,
            saved_settings: existing_settings.map(|settings| settings.save_directory),
            error_code: None,
        })
    }

    pub fn confirm_candidate(
        &self,
        request: ConfirmProfileSaveDirectoryCandidateRequest,
    ) -> Result<SaveDirectoryDiscoveryResult, SaveDirectoryDiscoveryError> {
        let now = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveDirectoryDiscoveryError::ClockUnavailable)?;
        let pending = self
            .pending_store
            .get_candidate(&request.discovery_id, &request.candidate_id, now)
            .map_err(|_| SaveDirectoryDiscoveryError::PendingStoreUnavailable)?
            .ok_or(SaveDirectoryDiscoveryError::CandidateExpired)?;

        self.ensure_profile(&pending.profile_id)?;
        let instance = match self
            .game_config_repository
            .load_game_instance(&pending.game_id)
        {
            Ok(Some(instance)) if instance.status == GameDirectoryStatus::Configured => instance,
            Ok(_) => return Err(SaveDirectoryDiscoveryError::CandidateInvalid),
            Err(_) => return Err(SaveDirectoryDiscoveryError::RepositoryUnavailable),
        };
        let rule = self.rule_for(&pending.game_id)?;
        let scan_request = scan_request_for(rule.as_ref(), Some(instance.root_dir));
        let validated = self
            .scanner
            .validate_save_directory(&scan_request, &pending.directory)
            .map_err(|_| SaveDirectoryDiscoveryError::CandidateInvalid)?;
        if validated.confidence < SaveDirectoryCandidateConfidence::Medium {
            return Err(SaveDirectoryDiscoveryError::CandidateInvalid);
        }

        let existing_settings = self
            .save_settings_repository
            .get_settings(pending.profile_id.as_str())
            .map_err(|_| SaveDirectoryDiscoveryError::SettingsUnavailable)?;
        let settings = self.save_candidate_settings(
            &pending.game_id,
            &pending.profile_id,
            &validated,
            existing_settings.as_ref(),
        )?;
        let summary = candidate_summary(validated, None, true);

        Ok(SaveDirectoryDiscoveryResult {
            discovery_id: request.discovery_id,
            game_id: pending.game_id,
            profile_id: pending.profile_id,
            outcome: SaveDirectoryDiscoveryOutcome::AutoSaved,
            recommended_candidate_id: Some(summary.candidate_id.clone()),
            candidates: vec![summary],
            saved_settings: Some(settings.save_directory),
            error_code: None,
        })
    }

    fn ensure_profile(&self, profile_id: &ProfileId) -> Result<(), SaveDirectoryDiscoveryError> {
        self.profile_repository
            .get(profile_id.as_str())
            .map_err(|_| SaveDirectoryDiscoveryError::RepositoryUnavailable)?
            .ok_or(SaveDirectoryDiscoveryError::ProfileMissing)?;
        Ok(())
    }

    fn rule_for(
        &self,
        game_id: &GameId,
    ) -> Result<Arc<dyn GameSaveDirectoryRule>, SaveDirectoryDiscoveryError> {
        self.save_directory_rules
            .iter()
            .find(|rule| rule.game_id() == *game_id)
            .cloned()
            .ok_or(SaveDirectoryDiscoveryError::RuleUnavailable)
    }

    fn save_candidate_settings(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        candidate: &ScannedSaveDirectoryCandidate,
        existing_settings: Option<&ProfileSaveSettings>,
    ) -> Result<ProfileSaveSettings, SaveDirectoryDiscoveryError> {
        let now = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveDirectoryDiscoveryError::ClockUnavailable)?;
        let settings = ProfileSaveSettings {
            profile_id: profile_id.as_str().to_owned(),
            save_directory: ProfileDirectorySelection {
                mode: ProfileDirectoryMode::Custom,
                status: ProfileDirectoryStatus::Valid,
                directory: Some(candidate.directory.to_string_lossy().to_string()),
                path_label: Some(candidate.path_label.clone()),
                messages: vec!["已自动关联 MHW:I 存档目录".to_owned()],
            },
            backup_directory: match existing_settings {
                Some(settings) => settings.backup_directory.clone(),
                None => self
                    .save_directory_validator
                    .default_backup_directory(game_id.as_str())
                    .map_err(|_| SaveDirectoryDiscoveryError::SettingsUnavailable)?,
            },
            schedule: existing_settings
                .map(|settings| settings.schedule.clone())
                .unwrap_or_else(ProfileBackupSchedule::manual),
            retention: existing_settings
                .map(|settings| settings.retention.clone())
                .unwrap_or_else(ProfileBackupRetention::default),
            updated_at: now,
        };

        self.save_settings_repository
            .save_settings(&settings)
            .map_err(|_| SaveDirectoryDiscoveryError::SettingsUnavailable)?;
        Ok(settings)
    }
}

fn scan_request_for(
    rule: &dyn GameSaveDirectoryRule,
    game_root_hint: Option<std::path::PathBuf>,
) -> SteamUserdataScanRequest {
    SteamUserdataScanRequest {
        game_id: rule.game_id(),
        game_root_hint,
        steam_app_id: rule.steam_app_id(),
        remote_relative_path: rule.steam_remote_relative_path().to_owned(),
        known_save_file_names: rule
            .known_save_file_names()
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        path_label: rule.path_label().to_owned(),
    }
}

fn candidate_summary(
    candidate: ScannedSaveDirectoryCandidate,
    profile: Option<hmm_core::SteamAccountProfileSummary>,
    recommended: bool,
) -> SaveDirectoryCandidateSummary {
    SaveDirectoryCandidateSummary {
        candidate_id: candidate.candidate_id,
        source: SaveDirectoryCandidateSource::SteamUserdata,
        confidence: candidate.confidence,
        recommended,
        account_name: profile
            .as_ref()
            .and_then(|summary| summary.account_name.clone()),
        avatar_url: profile.and_then(|summary| summary.avatar_url),
        account_label: candidate.account_label,
        path_label: candidate.path_label,
        last_modified_at: candidate.last_modified_at,
        evidence: candidate.evidence,
    }
}

fn sort_candidates(candidates: &mut [ScannedSaveDirectoryCandidate]) {
    candidates.sort_by(|left, right| {
        right
            .confidence
            .cmp(&left.confidence)
            .then_with(|| right.last_modified_at.cmp(&left.last_modified_at))
            .then_with(|| left.account_label.cmp(&right.account_label))
    });
}

fn empty_result(
    discovery_id: String,
    game_id: GameId,
    profile_id: ProfileId,
    outcome: SaveDirectoryDiscoveryOutcome,
    error_code: Option<&str>,
) -> SaveDirectoryDiscoveryResult {
    SaveDirectoryDiscoveryResult {
        discovery_id,
        game_id,
        profile_id,
        outcome,
        recommended_candidate_id: None,
        candidates: Vec::new(),
        saved_settings: None,
        error_code: error_code.map(str::to_owned),
    }
}

fn new_discovery_id() -> String {
    format!("save-directory-discovery-{}", Uuid::new_v4())
}
