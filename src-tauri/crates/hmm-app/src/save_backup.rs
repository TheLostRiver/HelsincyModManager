use hmm_core::{
    GameId, ProfileDirectoryMode, ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId,
    ProfileSaveSettings, SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
};
use hmm_ports::{
    AppClock, ProfileRepository, ProfileSaveDirectoryValidator, ProfileSaveSettingsRepository,
    SaveBackupRepository, SaveBackupWriteRequest, SaveBackupWriter,
};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSaveBackupRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub note: Option<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SaveBackupError {
    #[error("profile is missing")]
    ProfileMissing,
    #[error("save source directory is not configured")]
    SourceUnset,
    #[error("save source directory is invalid")]
    SourceInvalid,
    #[error("save backup writer is unavailable")]
    WriterUnavailable,
    #[error("save backup history is unavailable")]
    HistoryUnavailable,
    #[error("save backup retention failed")]
    RetentionFailed,
}

impl SaveBackupError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProfileMissing => "save_backup_profile_missing",
            Self::SourceUnset => "save_backup_source_unset",
            Self::SourceInvalid => "save_backup_source_invalid",
            Self::WriterUnavailable => "save_backup_archive_write_failed",
            Self::HistoryUnavailable => "save_backup_history_unavailable",
            Self::RetentionFailed => "save_backup_retention_failed",
        }
    }
}

pub struct SaveBackupService {
    profile_repository: Arc<dyn ProfileRepository>,
    save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
    save_directory_validator: Arc<dyn ProfileSaveDirectoryValidator>,
    backup_repository: Arc<dyn SaveBackupRepository>,
    backup_writer: Arc<dyn SaveBackupWriter>,
    clock: Arc<dyn AppClock>,
}

impl SaveBackupService {
    pub fn new(
        profile_repository: Arc<dyn ProfileRepository>,
        save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
        save_directory_validator: Arc<dyn ProfileSaveDirectoryValidator>,
        backup_repository: Arc<dyn SaveBackupRepository>,
        backup_writer: Arc<dyn SaveBackupWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            profile_repository,
            save_settings_repository,
            save_directory_validator,
            backup_repository,
            backup_writer,
            clock,
        }
    }

    pub fn create_manual_backup(
        &self,
        request: CreateSaveBackupRequest,
    ) -> Result<SaveBackupSummary, SaveBackupError> {
        let settings = self.settings_for(&request)?;
        let source_directory = validated_source_directory(&settings.save_directory)?;
        let created_at_unix_millis = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveBackupError::WriterUnavailable)?;

        let write_result = self
            .backup_writer
            .write_backup(SaveBackupWriteRequest {
                game_id: request.game_id.clone(),
                profile_id: request.profile_id.clone(),
                trigger: SaveBackupTrigger::Manual,
                source_directory: Some(source_directory),
                source_directory_selection: settings.save_directory.clone(),
                backup_directory: settings.backup_directory.clone(),
                retention: settings.retention.clone(),
                note: normalize_note(request.note),
                created_at_unix_millis,
            })
            .map_err(|_| SaveBackupError::WriterUnavailable)?;

        self.backup_repository
            .save(&write_result.summary)
            .map_err(|_| SaveBackupError::HistoryUnavailable)?;
        self.apply_max_count_retention(
            &request.game_id,
            &request.profile_id,
            &settings.backup_directory,
            settings.retention.max_count,
        )?;

        Ok(write_result.summary)
    }

    pub fn list_backups(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        limit: Option<usize>,
    ) -> Result<Vec<SaveBackupSummary>, SaveBackupError> {
        self.backup_repository
            .list_for_profile(game_id, profile_id, limit)
            .map_err(|_| SaveBackupError::HistoryUnavailable)
    }

    fn settings_for(
        &self,
        request: &CreateSaveBackupRequest,
    ) -> Result<ProfileSaveSettings, SaveBackupError> {
        self.profile_repository
            .get(request.profile_id.as_str())
            .map_err(|_| SaveBackupError::HistoryUnavailable)?
            .ok_or(SaveBackupError::ProfileMissing)?;

        if let Some(settings) = self
            .save_settings_repository
            .get_settings(request.profile_id.as_str())
            .map_err(|_| SaveBackupError::HistoryUnavailable)?
        {
            return Ok(settings);
        }

        Ok(ProfileSaveSettings {
            profile_id: request.profile_id.as_str().to_owned(),
            save_directory: unset_save_directory(),
            backup_directory: self
                .save_directory_validator
                .default_backup_directory(request.game_id.as_str())
                .map_err(|_| SaveBackupError::WriterUnavailable)?,
            schedule: hmm_core::ProfileBackupSchedule::manual(),
            retention: hmm_core::ProfileBackupRetention::default(),
            updated_at: 0,
        })
    }

    fn apply_max_count_retention(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        backup_directory: &ProfileDirectorySelection,
        max_count: u32,
    ) -> Result<(), SaveBackupError> {
        let mut summaries = self
            .backup_repository
            .list_for_profile(game_id, profile_id, None)
            .map_err(|_| SaveBackupError::HistoryUnavailable)?
            .into_iter()
            .filter(|summary| summary.status == SaveBackupStatus::Completed)
            .collect::<Vec<_>>();

        summaries.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.backup_id.cmp(&left.backup_id))
        });

        for summary in summaries.into_iter().skip(max_count as usize) {
            self.backup_writer
                .delete_backup_files(backup_directory, &summary)
                .map_err(|_| SaveBackupError::RetentionFailed)?;
            self.backup_repository
                .mark_status(&summary.backup_id, SaveBackupStatus::DeletedByRetention)
                .map_err(|_| SaveBackupError::RetentionFailed)?;
        }

        Ok(())
    }
}

fn validated_source_directory(
    selection: &ProfileDirectorySelection,
) -> Result<String, SaveBackupError> {
    match (
        selection.mode,
        selection.status,
        selection.directory.as_ref(),
    ) {
        (ProfileDirectoryMode::Unset, _, _)
        | (_, ProfileDirectoryStatus::Unset, _)
        | (_, _, None) => Err(SaveBackupError::SourceUnset),
        (_, ProfileDirectoryStatus::Valid, Some(directory)) => Ok(directory.clone()),
        _ => Err(SaveBackupError::SourceInvalid),
    }
}

fn normalize_note(note: Option<String>) -> Option<String> {
    note.map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn unset_save_directory() -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Unset,
        status: ProfileDirectoryStatus::Unset,
        directory: None,
        path_label: None,
        messages: vec!["尚未选择游戏存档目录".to_owned()],
    }
}
