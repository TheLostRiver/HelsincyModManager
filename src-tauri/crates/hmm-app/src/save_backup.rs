use hmm_core::{
    GameId, ProfileBackupRetention, ProfileDirectoryMode, ProfileDirectorySelection,
    ProfileDirectoryStatus, ProfileId, ProfileSaveSettings, SaveBackupRetentionOutcome,
    SaveBackupRetentionReason, SaveBackupRetentionReport, SaveBackupStatus, SaveBackupSummary,
    SaveBackupTrigger,
};
use hmm_ports::{
    AppClock, ProfileRepository, ProfileSaveDirectoryValidator, ProfileSaveSettingsRepository,
    SaveBackupRepository, SaveBackupWriteRequest, SaveBackupWriter,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSaveBackupRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSaveBackupResult {
    pub summary: SaveBackupSummary,
    pub warnings: Vec<SaveBackupWarning>,
    pub retention_report: Option<SaveBackupRetentionReport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveBackupWarning {
    RetentionPartial,
    RetentionBlocked,
    RetentionFailed,
}

impl SaveBackupWarning {
    pub fn code(self) -> &'static str {
        match self {
            Self::RetentionPartial => "save_backup_retention_partial",
            Self::RetentionBlocked => "save_backup_retention_blocked",
            Self::RetentionFailed => "save_backup_retention_failed",
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SaveBackupError {
    #[error("profile is missing")]
    ProfileMissing,
    #[error("save source directory is not configured")]
    SourceUnset,
    #[error("save source directory is invalid")]
    SourceInvalid,
    #[error("app clock is unavailable")]
    ClockUnavailable,
    #[error("save backup destination is unavailable")]
    DestinationUnavailable,
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
            Self::ClockUnavailable => "save_backup_clock_unavailable",
            Self::DestinationUnavailable => "save_backup_destination_unavailable",
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
    ) -> Result<CreateSaveBackupResult, SaveBackupError> {
        self.create_backup(request, SaveBackupTrigger::Manual)
    }

    pub fn create_backup(
        &self,
        request: CreateSaveBackupRequest,
        trigger: SaveBackupTrigger,
    ) -> Result<CreateSaveBackupResult, SaveBackupError> {
        let settings = self.settings_for(&request)?;
        let source_directory = validated_source_directory(&settings.save_directory)?;
        let created_at_unix_millis = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveBackupError::ClockUnavailable)?;

        let write_result = self
            .backup_writer
            .write_backup(SaveBackupWriteRequest {
                game_id: request.game_id.clone(),
                profile_id: request.profile_id.clone(),
                trigger,
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

        let mut warnings = Vec::new();
        let retention_report = if trigger == SaveBackupTrigger::PreRestore {
            None
        } else {
            match self.apply_retention(
                &request.game_id,
                &request.profile_id,
                &settings.retention,
                created_at_unix_millis,
            ) {
                Ok(report) if report.outcome == SaveBackupRetentionOutcome::Partial => {
                    warnings.push(SaveBackupWarning::RetentionPartial);
                    Some(report)
                }
                Ok(report) if report.outcome == SaveBackupRetentionOutcome::Blocked => {
                    warnings.push(SaveBackupWarning::RetentionBlocked);
                    Some(report)
                }
                Ok(report) => Some(report),
                Err(_) => {
                    warnings.push(SaveBackupWarning::RetentionFailed);
                    None
                }
            }
        };

        Ok(CreateSaveBackupResult {
            summary: write_result.summary,
            warnings,
            retention_report,
        })
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

    pub fn run_retention(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<SaveBackupRetentionReport, SaveBackupError> {
        let settings = self.settings_for(&CreateSaveBackupRequest {
            game_id: game_id.clone(),
            profile_id: profile_id.clone(),
            note: None,
        })?;
        let now = self
            .clock
            .now_unix_millis()
            .map_err(|_| SaveBackupError::ClockUnavailable)?;
        self.apply_retention(game_id, profile_id, &settings.retention, now)
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
                .map_err(|_| SaveBackupError::DestinationUnavailable)?,
            schedule: hmm_core::ProfileBackupSchedule::manual(),
            retention: hmm_core::ProfileBackupRetention::default(),
            steam_account: None,
            pre_restore_backup_enabled: true,
            updated_at: 0,
        })
    }

    fn apply_retention(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        retention: &ProfileBackupRetention,
        now_unix_millis: u128,
    ) -> Result<SaveBackupRetentionReport, SaveBackupError> {
        let summaries = self
            .backup_repository
            .list_for_profile(game_id, profile_id, None)
            .map_err(|_| SaveBackupError::RetentionFailed)?;
        let mut ordinary = summaries
            .iter()
            .filter(|summary| {
                summary.trigger != SaveBackupTrigger::PreRestore
                    && matches!(
                        summary.status,
                        SaveBackupStatus::Completed
                            | SaveBackupStatus::RetentionPending
                            | SaveBackupStatus::RetentionPartial
                    )
            })
            .cloned()
            .collect::<Vec<_>>();
        ordinary.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.backup_id.cmp(&left.backup_id))
        });

        let archive_bytes_before = active_archive_bytes(&summaries);
        let protected_count = summaries
            .iter()
            .filter(|summary| {
                summary.trigger == SaveBackupTrigger::PreRestore
                    && summary.status != SaveBackupStatus::DeletedByRetention
            })
            .count() as u32;
        let problem_count = summaries
            .iter()
            .filter(|summary| {
                matches!(
                    summary.status,
                    SaveBackupStatus::Missing | SaveBackupStatus::Invalid
                )
            })
            .count() as u32;
        let latest_completed_id = ordinary
            .iter()
            .find(|summary| summary.status == SaveBackupStatus::Completed)
            .map(|summary| summary.backup_id.clone());
        let mut candidates = BTreeMap::<String, BTreeSet<SaveBackupRetentionReason>>::new();

        for summary in &ordinary {
            if matches!(
                summary.status,
                SaveBackupStatus::RetentionPending | SaveBackupStatus::RetentionPartial
            ) {
                candidates
                    .entry(summary.backup_id.clone())
                    .or_default()
                    .insert(SaveBackupRetentionReason::Retry);
            }
        }

        if retention.max_count > 0 {
            for (index, summary) in ordinary
                .iter()
                .filter(|summary| summary.status == SaveBackupStatus::Completed)
                .enumerate()
            {
                if index >= retention.max_count as usize {
                    candidates
                        .entry(summary.backup_id.clone())
                        .or_default()
                        .insert(SaveBackupRetentionReason::Count);
                }
            }
        }

        if let Some(max_age_days) = retention.max_age_days {
            let cutoff = now_unix_millis.saturating_sub(u128::from(max_age_days) * 86_400_000);
            for summary in ordinary
                .iter()
                .filter(|summary| summary.status == SaveBackupStatus::Completed)
            {
                if summary.created_at < cutoff {
                    candidates
                        .entry(summary.backup_id.clone())
                        .or_default()
                        .insert(SaveBackupRetentionReason::Age);
                }
            }
        }

        if let Some(latest_id) = latest_completed_id.as_ref() {
            candidates.remove(latest_id);
        }

        let mut simulated_bytes =
            archive_bytes_before.saturating_sub(candidate_archive_bytes(&ordinary, &candidates));
        if let Some(max_total_bytes) = retention.max_total_bytes {
            if simulated_bytes > max_total_bytes {
                for summary in ordinary.iter().rev() {
                    if summary.status != SaveBackupStatus::Completed
                        || latest_completed_id.as_deref() == Some(summary.backup_id.as_str())
                        || candidates.contains_key(&summary.backup_id)
                    {
                        continue;
                    }
                    candidates
                        .entry(summary.backup_id.clone())
                        .or_default()
                        .insert(SaveBackupRetentionReason::Space);
                    simulated_bytes =
                        simulated_bytes.saturating_sub(remaining_archive_bytes(summary));
                    if simulated_bytes <= max_total_bytes {
                        break;
                    }
                }
            }
        }

        let planned_budget_satisfied = retention
            .max_total_bytes
            .is_none_or(|limit| simulated_bytes <= limit);
        let mut ordered_candidates = ordinary
            .iter()
            .filter_map(|summary| {
                candidates
                    .get(&summary.backup_id)
                    .map(|reasons| (summary.clone(), reasons.iter().copied().collect::<Vec<_>>()))
            })
            .collect::<Vec<_>>();
        ordered_candidates.sort_by(|(left, _), (right, _)| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.backup_id.cmp(&right.backup_id))
        });

        let mut deleted_count = 0_u32;
        let mut partial_count = 0_u32;
        let mut blocked_count = 0_u32;
        let mut released_bytes = 0_u64;
        let mut removed_known_archive_bytes = 0_u64;
        for (summary, reasons) in &ordered_candidates {
            let began = self
                .backup_repository
                .begin_retention(
                    game_id,
                    profile_id,
                    &summary.backup_id,
                    reasons,
                    now_unix_millis,
                )
                .map_err(|_| SaveBackupError::RetentionFailed)?;
            if !began {
                partial_count = partial_count.saturating_add(1);
                blocked_count = blocked_count.saturating_add(1);
                continue;
            }

            let report = match self
                .backup_writer
                .delete_backup_files_report(&summary.backup_directory, summary)
            {
                Ok(report) => report,
                Err(_) => {
                    self.backup_repository
                        .finish_retention(
                            game_id,
                            profile_id,
                            &summary.backup_id,
                            SaveBackupStatus::RetentionPartial,
                            Some("save_backup_retention_delete_failed"),
                            0,
                        )
                        .map_err(|_| SaveBackupError::RetentionFailed)?;
                    partial_count = partial_count.saturating_add(1);
                    blocked_count = blocked_count.saturating_add(1);
                    continue;
                }
            };
            let released = report.released_archive_bytes();
            released_bytes = released_bytes.saturating_add(released);
            if report.converged() {
                self.backup_repository
                    .finish_retention(
                        game_id,
                        profile_id,
                        &summary.backup_id,
                        SaveBackupStatus::DeletedByRetention,
                        None,
                        released,
                    )
                    .map_err(|_| SaveBackupError::RetentionFailed)?;
                deleted_count = deleted_count.saturating_add(1);
                removed_known_archive_bytes =
                    removed_known_archive_bytes.saturating_add(remaining_archive_bytes(summary));
            } else {
                self.backup_repository
                    .finish_retention(
                        game_id,
                        profile_id,
                        &summary.backup_id,
                        SaveBackupStatus::RetentionPartial,
                        report.stable_error_code(),
                        released,
                    )
                    .map_err(|_| SaveBackupError::RetentionFailed)?;
                partial_count = partial_count.saturating_add(1);
                blocked_count = blocked_count.saturating_add(1);
                removed_known_archive_bytes = removed_known_archive_bytes
                    .saturating_add(released.min(remaining_archive_bytes(summary)));
            }
        }

        let archive_bytes_after = archive_bytes_before.saturating_sub(removed_known_archive_bytes);
        let budget_satisfied = retention
            .max_total_bytes
            .is_none_or(|limit| archive_bytes_after <= limit);
        if !planned_budget_satisfied || !budget_satisfied {
            blocked_count = blocked_count.saturating_add(1);
        }
        let outcome = if partial_count > 0 {
            SaveBackupRetentionOutcome::Partial
        } else if blocked_count > 0 {
            SaveBackupRetentionOutcome::Blocked
        } else if ordered_candidates.is_empty() {
            SaveBackupRetentionOutcome::WithinPolicy
        } else {
            SaveBackupRetentionOutcome::Completed
        };

        Ok(SaveBackupRetentionReport {
            outcome,
            evidence_degraded: false,
            scanned_count: summaries.len() as u32,
            protected_count,
            problem_count,
            candidate_count: ordered_candidates.len() as u32,
            deleted_count,
            partial_count,
            blocked_count,
            archive_bytes_before,
            archive_bytes_after,
            released_bytes,
            max_total_bytes: retention.max_total_bytes,
            budget_satisfied,
        })
    }
}

fn active_archive_bytes(summaries: &[SaveBackupSummary]) -> u64 {
    summaries
        .iter()
        .filter(|summary| summary.status != SaveBackupStatus::DeletedByRetention)
        .fold(0_u64, |total, summary| {
            total.saturating_add(remaining_archive_bytes(summary))
        })
}

fn remaining_archive_bytes(summary: &SaveBackupSummary) -> u64 {
    summary.archive_size_bytes.saturating_sub(
        summary
            .retention_released_bytes
            .min(summary.archive_size_bytes),
    )
}

fn candidate_archive_bytes(
    summaries: &[SaveBackupSummary],
    candidates: &BTreeMap<String, BTreeSet<SaveBackupRetentionReason>>,
) -> u64 {
    summaries
        .iter()
        .filter(|summary| candidates.contains_key(&summary.backup_id))
        .fold(0_u64, |total, summary| {
            total.saturating_add(remaining_archive_bytes(summary))
        })
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
