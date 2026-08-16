use hmac::{Hmac, Mac};
use hmm_core::{
    GameId, ProfileDirectoryMode, ProfileDirectoryStatus, ProfileId, ProfileSaveSettings,
    SaveBackupStatus, SaveBackupSummary,
};
use hmm_ports::{
    AppClock, GameRunningDetector, GameRunningStatus, ProfileRepository,
    ProfileSaveSettingsRepository, SaveBackupRepository, SaveRestoreSourceError,
    SaveRestoreSourceValidator, SaveRestoreTransactionRepository, ValidatedSaveRestoreSource,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_SAVE_RESTORE_PREVIEW_TOKEN_TTL_MILLIS: u128 = 5 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewSaveRestoreRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub backup_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRestorePreview {
    pub backup: SaveBackupSummary,
    pub file_count: u32,
    pub total_uncompressed_bytes: u64,
    pub pre_restore_backup_enabled: bool,
    pub requires_additional_confirmation: bool,
    pub warning_codes: Vec<String>,
    pub preview_token: String,
    pub expires_at_unix_millis: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartSaveRestoreRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub backup_id: String,
    pub preview_token: String,
    pub confirmed: bool,
    pub confirmed_without_pre_restore: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRestoreCommitContext {
    pub request: StartSaveRestoreRequest,
    pub summary: SaveBackupSummary,
    pub settings: ProfileSaveSettings,
    pub validated_source: ValidatedSaveRestoreSource,
    pub facts_digest: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SaveRestorePreviewError {
    #[error("profile is missing")]
    ProfileMissing,
    #[error("save restore backup is missing")]
    BackupMissing,
    #[error("save restore backup is unavailable")]
    BackupUnavailable,
    #[error("save restore target is not configured")]
    TargetUnset,
    #[error("save restore target is invalid")]
    TargetInvalid,
    #[error("game is running")]
    GameRunning,
    #[error("game running state is unknown")]
    GameRunningUnknown,
    #[error("save restore source is invalid: {0}")]
    SourceInvalid(SaveRestoreSourceError),
    #[error("save restore source identity does not match the selected backup")]
    SourceIdentityMismatch,
    #[error("save restore transaction is pending recovery")]
    RecoveryRequired,
    #[error("save restore transaction history is unavailable")]
    TransactionUnavailable,
    #[error("save restore clock is unavailable")]
    ClockUnavailable,
    #[error("save restore token could not be issued")]
    TokenIssueFailed,
    #[error("save restore preview token is invalid")]
    InvalidToken,
    #[error("save restore preview token is expired")]
    ExpiredToken,
    #[error("save restore preview facts are stale")]
    StaleToken,
    #[error("save restore confirmation is required")]
    ConfirmationRequired,
    #[error("additional confirmation is required when pre-restore backup is disabled")]
    HighRiskConfirmationRequired,
}

impl SaveRestorePreviewError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProfileMissing => "save_restore_profile_missing",
            Self::BackupMissing => "save_restore_backup_missing",
            Self::BackupUnavailable => "save_restore_backup_unavailable",
            Self::TargetUnset => "save_restore_target_unset",
            Self::TargetInvalid => "save_restore_target_invalid",
            Self::GameRunning => "save_restore_game_running",
            Self::GameRunningUnknown => "save_restore_game_running_unknown",
            Self::SourceInvalid(error) => error.code(),
            Self::SourceIdentityMismatch => "save_restore_source_invalid",
            Self::RecoveryRequired => "save_restore_recovery_required",
            Self::TransactionUnavailable => "save_restore_transaction_unavailable",
            Self::ClockUnavailable => "save_restore_clock_unavailable",
            Self::TokenIssueFailed => "save_restore_token_issue_failed",
            Self::InvalidToken => "save_restore_token_invalid",
            Self::ExpiredToken => "save_restore_token_expired",
            Self::StaleToken => "save_restore_token_stale",
            Self::ConfirmationRequired => "save_restore_confirmation_required",
            Self::HighRiskConfirmationRequired => "save_restore_high_risk_confirmation_required",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveRestoreTokenError {
    Invalid,
    Expired,
    Mismatch,
}

pub trait SaveRestoreTokenCodec: Send + Sync {
    fn issue(
        &self,
        digest: &str,
        issued_at_unix_millis: u128,
        expires_at_unix_millis: u128,
    ) -> anyhow::Result<String>;

    fn verify(
        &self,
        token: &str,
        digest: &str,
        now_unix_millis: u128,
    ) -> Result<(), SaveRestoreTokenError>;
}

#[derive(Clone)]
pub struct Sha256SaveRestoreTokenCodec {
    secret: Arc<Vec<u8>>,
}

impl Sha256SaveRestoreTokenCodec {
    pub fn new(secret: impl AsRef<[u8]>) -> anyhow::Result<Self> {
        let secret = secret.as_ref();
        anyhow::ensure!(!secret.is_empty(), "save restore token secret is empty");
        Ok(Self {
            secret: Arc::new(secret.to_vec()),
        })
    }
}

impl SaveRestoreTokenCodec for Sha256SaveRestoreTokenCodec {
    fn issue(
        &self,
        digest: &str,
        issued_at_unix_millis: u128,
        expires_at_unix_millis: u128,
    ) -> anyhow::Result<String> {
        anyhow::ensure!(issued_at_unix_millis < expires_at_unix_millis);
        let signature = sign_token(
            &self.secret,
            digest,
            issued_at_unix_millis,
            expires_at_unix_millis,
        );
        Ok(format!(
            "hmm-save-restore-v1.{issued_at_unix_millis}.{expires_at_unix_millis}.{signature}"
        ))
    }

    fn verify(
        &self,
        token: &str,
        digest: &str,
        now_unix_millis: u128,
    ) -> Result<(), SaveRestoreTokenError> {
        let parts = token.split('.').collect::<Vec<_>>();
        if parts.len() != 4 || parts[0] != "hmm-save-restore-v1" {
            return Err(SaveRestoreTokenError::Invalid);
        }
        let issued_at = parts[1]
            .parse::<u128>()
            .map_err(|_| SaveRestoreTokenError::Invalid)?;
        let expires_at = parts[2]
            .parse::<u128>()
            .map_err(|_| SaveRestoreTokenError::Invalid)?;
        if !constant_time_eq(
            parts[3].as_bytes(),
            sign_token(&self.secret, digest, issued_at, expires_at).as_bytes(),
        ) {
            return Err(SaveRestoreTokenError::Mismatch);
        }
        if issued_at >= expires_at || now_unix_millis < issued_at {
            return Err(SaveRestoreTokenError::Invalid);
        }
        if now_unix_millis >= expires_at {
            return Err(SaveRestoreTokenError::Expired);
        }
        Ok(())
    }
}

fn sign_token(secret: &[u8], digest: &str, issued_at: u128, expires_at: u128) -> String {
    let message = format!("hmm-save-restore-token-v1\0{digest}\0{issued_at}\0{expires_at}");
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("secret is non-empty");
    mac.update(message.as_bytes());
    Sha256::digest(mac.finalize().into_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0u8, |acc, (left, right)| acc | (left ^ right))
            == 0
}

pub struct SaveRestoreService {
    profile_repository: Arc<dyn ProfileRepository>,
    settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
    backup_repository: Arc<dyn SaveBackupRepository>,
    source_validator: Arc<dyn SaveRestoreSourceValidator>,
    transaction_repository: Arc<dyn SaveRestoreTransactionRepository>,
    game_running_detector: Arc<dyn GameRunningDetector>,
    clock: Arc<dyn AppClock>,
    token_codec: Arc<dyn SaveRestoreTokenCodec>,
}

impl SaveRestoreService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        profile_repository: Arc<dyn ProfileRepository>,
        settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
        backup_repository: Arc<dyn SaveBackupRepository>,
        source_validator: Arc<dyn SaveRestoreSourceValidator>,
        transaction_repository: Arc<dyn SaveRestoreTransactionRepository>,
        game_running_detector: Arc<dyn GameRunningDetector>,
        clock: Arc<dyn AppClock>,
        token_codec: Arc<dyn SaveRestoreTokenCodec>,
    ) -> Self {
        Self {
            profile_repository,
            settings_repository,
            backup_repository,
            source_validator,
            transaction_repository,
            game_running_detector,
            clock,
            token_codec,
        }
    }

    pub fn preview(
        &self,
        request: PreviewSaveRestoreRequest,
    ) -> Result<SaveRestorePreview, SaveRestorePreviewError> {
        let (summary, settings, source) = self.load_and_validate(&request, None)?;
        let now = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveRestorePreviewError::ClockUnavailable)?;
        let expires_at = now.saturating_add(DEFAULT_SAVE_RESTORE_PREVIEW_TOKEN_TTL_MILLIS);
        let facts_digest = facts_digest(&request, &summary, &settings, &source);
        let token = self
            .token_codec
            .issue(&facts_digest, now, expires_at)
            .map_err(|_| SaveRestorePreviewError::TokenIssueFailed)?;

        Ok(SaveRestorePreview {
            backup: summary,
            file_count: source.file_count,
            total_uncompressed_bytes: source.total_uncompressed_bytes,
            pre_restore_backup_enabled: settings.pre_restore_backup_enabled,
            requires_additional_confirmation: !settings.pre_restore_backup_enabled,
            warning_codes: if settings.pre_restore_backup_enabled {
                Vec::new()
            } else {
                vec!["save_restore_pre_restore_disabled".to_owned()]
            },
            preview_token: token,
            expires_at_unix_millis: expires_at,
        })
    }

    pub fn validate_for_commit(
        &self,
        request: StartSaveRestoreRequest,
    ) -> Result<SaveRestoreCommitContext, SaveRestorePreviewError> {
        self.validate_for_commit_internal(request, None)
    }

    pub fn validate_for_commit_excluding_transaction(
        &self,
        request: StartSaveRestoreRequest,
        transaction_id: &str,
    ) -> Result<SaveRestoreCommitContext, SaveRestorePreviewError> {
        self.validate_for_commit_internal(request, Some(transaction_id))
    }

    pub fn validate_prepared_for_commit_excluding_transaction(
        &self,
        request: StartSaveRestoreRequest,
        validated_source: ValidatedSaveRestoreSource,
        transaction_id: &str,
    ) -> Result<SaveRestoreCommitContext, SaveRestorePreviewError> {
        self.validate_for_commit_internal_with_source(
            request,
            Some(transaction_id),
            Some(validated_source),
        )
    }

    fn validate_for_commit_internal(
        &self,
        request: StartSaveRestoreRequest,
        excluded_transaction_id: Option<&str>,
    ) -> Result<SaveRestoreCommitContext, SaveRestorePreviewError> {
        self.validate_for_commit_internal_with_source(request, excluded_transaction_id, None)
    }

    fn validate_for_commit_internal_with_source(
        &self,
        request: StartSaveRestoreRequest,
        excluded_transaction_id: Option<&str>,
        validated_source: Option<ValidatedSaveRestoreSource>,
    ) -> Result<SaveRestoreCommitContext, SaveRestorePreviewError> {
        if !request.confirmed {
            return Err(SaveRestorePreviewError::ConfirmationRequired);
        }
        let (summary, settings, source) = self.load_and_validate_with_source(
            &PreviewSaveRestoreRequest {
                game_id: request.game_id.clone(),
                profile_id: request.profile_id.clone(),
                backup_id: request.backup_id.clone(),
            },
            excluded_transaction_id,
            validated_source,
        )?;
        if !settings.pre_restore_backup_enabled && !request.confirmed_without_pre_restore {
            return Err(SaveRestorePreviewError::HighRiskConfirmationRequired);
        }
        let now = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveRestorePreviewError::ClockUnavailable)?;
        let digest = facts_digest(
            &PreviewSaveRestoreRequest {
                game_id: request.game_id.clone(),
                profile_id: request.profile_id.clone(),
                backup_id: request.backup_id.clone(),
            },
            &summary,
            &settings,
            &source,
        );
        self.token_codec
            .verify(&request.preview_token, &digest, now)
            .map_err(|error| match error {
                SaveRestoreTokenError::Invalid => SaveRestorePreviewError::InvalidToken,
                SaveRestoreTokenError::Expired => SaveRestorePreviewError::ExpiredToken,
                SaveRestoreTokenError::Mismatch => SaveRestorePreviewError::StaleToken,
            })?;
        Ok(SaveRestoreCommitContext {
            request,
            summary,
            settings,
            validated_source: source,
            facts_digest: digest,
        })
    }

    fn load_and_validate(
        &self,
        request: &PreviewSaveRestoreRequest,
        excluded_transaction_id: Option<&str>,
    ) -> Result<
        (
            SaveBackupSummary,
            ProfileSaveSettings,
            ValidatedSaveRestoreSource,
        ),
        SaveRestorePreviewError,
    > {
        self.load_and_validate_with_source(request, excluded_transaction_id, None)
    }

    fn load_and_validate_with_source(
        &self,
        request: &PreviewSaveRestoreRequest,
        excluded_transaction_id: Option<&str>,
        validated_source: Option<ValidatedSaveRestoreSource>,
    ) -> Result<
        (
            SaveBackupSummary,
            ProfileSaveSettings,
            ValidatedSaveRestoreSource,
        ),
        SaveRestorePreviewError,
    > {
        let profile = self
            .profile_repository
            .get(request.profile_id.as_str())
            .map_err(|_| SaveRestorePreviewError::ProfileMissing)?
            .ok_or(SaveRestorePreviewError::ProfileMissing)?;
        if profile.id != request.profile_id.as_str() {
            return Err(SaveRestorePreviewError::ProfileMissing);
        }
        if self
            .transaction_repository
            .has_incomplete_transaction_excluding(
                &request.game_id,
                &request.profile_id,
                excluded_transaction_id,
            )
            .map_err(|_| SaveRestorePreviewError::TransactionUnavailable)?
        {
            return Err(SaveRestorePreviewError::RecoveryRequired);
        }
        let summary = self
            .backup_repository
            .get_for_restore(&request.game_id, &request.profile_id, &request.backup_id)
            .map_err(|_| SaveRestorePreviewError::BackupUnavailable)?
            .ok_or(SaveRestorePreviewError::BackupMissing)?;
        if summary.game_id != request.game_id
            || summary.profile_id != request.profile_id
            || summary.backup_id != request.backup_id
        {
            return Err(SaveRestorePreviewError::BackupUnavailable);
        }
        if summary.status != SaveBackupStatus::Completed {
            return Err(SaveRestorePreviewError::BackupUnavailable);
        }
        let settings = self
            .settings_repository
            .get_settings(request.profile_id.as_str())
            .map_err(|_| SaveRestorePreviewError::TargetInvalid)?
            .ok_or(SaveRestorePreviewError::TargetUnset)?;
        if settings.profile_id != request.profile_id.as_str() {
            return Err(SaveRestorePreviewError::TargetInvalid);
        }
        match (
            settings.save_directory.mode,
            settings.save_directory.status,
            settings.save_directory.directory.as_deref(),
        ) {
            (ProfileDirectoryMode::Unset, _, _) | (_, ProfileDirectoryStatus::Unset, _) => {
                return Err(SaveRestorePreviewError::TargetUnset)
            }
            (_, ProfileDirectoryStatus::Valid, Some(_)) => {}
            _ => return Err(SaveRestorePreviewError::TargetInvalid),
        }
        match self
            .game_running_detector
            .game_running_status(&request.game_id)
        {
            GameRunningStatus::Running => return Err(SaveRestorePreviewError::GameRunning),
            GameRunningStatus::Unknown => return Err(SaveRestorePreviewError::GameRunningUnknown),
            GameRunningStatus::NotRunning => {}
        }
        let source = match validated_source {
            Some(source) => source,
            None => self
                .source_validator
                .validate_source(&summary)
                .map_err(map_source_error)?,
        };
        if source.game_id != summary.game_id
            || source.profile_id != summary.profile_id
            || source.backup_id != summary.backup_id
        {
            return Err(SaveRestorePreviewError::SourceIdentityMismatch);
        }
        Ok((summary, settings, source))
    }
}

fn map_source_error(error: SaveRestoreSourceError) -> SaveRestorePreviewError {
    SaveRestorePreviewError::SourceInvalid(error)
}

fn facts_digest(
    request: &PreviewSaveRestoreRequest,
    summary: &SaveBackupSummary,
    settings: &ProfileSaveSettings,
    source: &ValidatedSaveRestoreSource,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"hmm-save-restore-facts-v1\0");
    for value in [
        request.game_id.as_str(),
        request.profile_id.as_str(),
        &request.backup_id,
        &summary.backup_id,
        summary.game_id.as_str(),
        summary.profile_id.as_str(),
        summary.trigger.as_str(),
        summary.status.as_str(),
        &summary.archive_file_name,
        &summary.manifest_file_name,
        &summary.archive_size_bytes.to_string(),
        &summary.archive_sha256,
        &summary.file_count.to_string(),
        &summary.created_at.to_string(),
        &summary.source_path_hash,
        directory_mode_code(summary.backup_directory.mode),
        directory_status_code(summary.backup_directory.status),
        summary.backup_directory.directory.as_deref().unwrap_or(""),
        &source.evidence_digest,
        source.game_id.as_str(),
        source.profile_id.as_str(),
        &source.backup_id,
        &source.file_count.to_string(),
        &source.total_uncompressed_bytes.to_string(),
        &settings.profile_id,
        &settings.updated_at.to_string(),
        &settings.pre_restore_backup_enabled.to_string(),
        directory_mode_code(settings.save_directory.mode),
        directory_status_code(settings.save_directory.status),
        settings.save_directory.directory.as_deref().unwrap_or(""),
        directory_mode_code(settings.backup_directory.mode),
        directory_status_code(settings.backup_directory.status),
        settings.backup_directory.directory.as_deref().unwrap_or(""),
    ] {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    format!(
        "sha256:{}",
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn directory_mode_code(mode: ProfileDirectoryMode) -> &'static str {
    match mode {
        ProfileDirectoryMode::Unset => "unset",
        ProfileDirectoryMode::Custom => "custom",
        ProfileDirectoryMode::Default => "default",
    }
}

fn directory_status_code(status: ProfileDirectoryStatus) -> &'static str {
    match status {
        ProfileDirectoryStatus::Unset => "unset",
        ProfileDirectoryStatus::Valid => "valid",
        ProfileDirectoryStatus::Invalid => "invalid",
        ProfileDirectoryStatus::Defaulted => "defaulted",
    }
}

pub fn new_save_restore_transaction_id() -> String {
    format!("restore-{}", Uuid::new_v4())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_codec_rejects_digest_change_and_expiry() {
        let codec = Sha256SaveRestoreTokenCodec::new("test-secret").expect("codec");
        let token = codec.issue("digest", 100, 200).expect("token");
        assert_eq!(
            codec.verify(&token, "other", 150),
            Err(SaveRestoreTokenError::Mismatch)
        );
        assert_eq!(
            codec.verify(&token, "digest", 200),
            Err(SaveRestoreTokenError::Expired)
        );
    }

    #[test]
    fn source_validation_errors_keep_their_stable_code() {
        let error = map_source_error(SaveRestoreSourceError::ArchiveUnavailable);
        assert_eq!(error.code(), "save_restore_archive_unavailable");
    }
}
