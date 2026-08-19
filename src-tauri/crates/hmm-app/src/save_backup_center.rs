use crate::{SaveBackupError, SaveBackupService, SaveBackupTaskScopeRegistry};
use hmm_core::{
    GameId, ProfileBackupRetention, ProfileId, SaveBackupRetentionOutcome,
    SaveBackupRetentionReport, SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
    SteamAccountDisplaySummary,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, AuditWriteFailurePolicy,
    CrossProcessWriteAdmissionError, ProfileRepository, ProfileSaveSettingsRepository,
    SaveBackupCenterRepositoryQuery, SaveBackupRepository,
};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use thiserror::Error;

pub const DEFAULT_SAVE_BACKUP_CENTER_LIMIT: usize = 30;
pub const MAX_SAVE_BACKUP_CENTER_LIMIT: usize = 100;
pub const MAX_SAVE_BACKUP_CENTER_SEARCH_CHARS: usize = 100;
pub const MAX_SAVE_BACKUP_NOTE_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupCenterQuery {
    pub game_id: GameId,
    pub profile_id: Option<ProfileId>,
    pub trigger: Option<SaveBackupTrigger>,
    pub status: Option<SaveBackupStatus>,
    pub search: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupCenterSummary {
    pub backup_count: u32,
    pub archive_bytes: u64,
    pub protected_count: u32,
    pub attention_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupCenterProfileSummary {
    pub profile_id: ProfileId,
    pub profile_name: String,
    pub is_active: bool,
    pub steam_account: Option<SteamAccountDisplaySummary>,
    pub retention: ProfileBackupRetention,
    pub backup_count: u32,
    pub archive_bytes: u64,
    pub protected_count: u32,
    pub attention_count: u32,
    pub budget_satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupCenterItem {
    pub profile_name: String,
    pub backup: SaveBackupSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupCenterPage {
    pub offset: usize,
    pub limit: usize,
    pub total_count: usize,
    pub summary: SaveBackupCenterSummary,
    pub profiles: Vec<SaveBackupCenterProfileSummary>,
    pub items: Vec<SaveBackupCenterItem>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SaveBackupCenterError {
    #[error("save backup center query is invalid")]
    QueryInvalid,
    #[error("save backup center repository is unavailable")]
    RepositoryUnavailable,
    #[error("save backup center profile is missing")]
    ProfileMissing,
    #[error("save backup note is invalid")]
    NoteInvalid,
    #[error("save backup is missing")]
    BackupMissing,
    #[error("save backup task scope is busy")]
    TaskConflict,
    #[error("save backup retention failed")]
    RetentionFailed,
    #[error(transparent)]
    WriteAdmission(#[from] CrossProcessWriteAdmissionError),
}

impl SaveBackupCenterError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::QueryInvalid => "save_backup_center_query_invalid",
            Self::RepositoryUnavailable => "save_backup_center_unavailable",
            Self::ProfileMissing => "save_backup_center_profile_missing",
            Self::NoteInvalid => "save_backup_note_invalid",
            Self::BackupMissing => "save_backup_center_backup_missing",
            Self::TaskConflict => "save_backup_task_conflict",
            Self::RetentionFailed => "save_backup_retention_failed",
            Self::WriteAdmission(error) => error.code(),
        }
    }
}

pub struct SaveBackupCenterService {
    profile_repository: Arc<dyn ProfileRepository>,
    settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
    backup_repository: Arc<dyn SaveBackupRepository>,
    save_backup_service: Arc<SaveBackupService>,
    scope_registry: Arc<SaveBackupTaskScopeRegistry>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
}

impl SaveBackupCenterService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        profile_repository: Arc<dyn ProfileRepository>,
        settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
        backup_repository: Arc<dyn SaveBackupRepository>,
        save_backup_service: Arc<SaveBackupService>,
        scope_registry: Arc<SaveBackupTaskScopeRegistry>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            profile_repository,
            settings_repository,
            backup_repository,
            save_backup_service,
            scope_registry,
            audit_log,
            clock,
        }
    }

    pub fn query(
        &self,
        query: SaveBackupCenterQuery,
    ) -> Result<SaveBackupCenterPage, SaveBackupCenterError> {
        if query.limit == 0
            || query.limit > MAX_SAVE_BACKUP_CENTER_LIMIT
            || i64::try_from(query.offset).is_err()
        {
            return Err(SaveBackupCenterError::QueryInvalid);
        }
        let search = normalize_search(query.search.clone())?;
        let profiles = self
            .profile_repository
            .list_all()
            .map_err(|_| SaveBackupCenterError::RepositoryUnavailable)?;
        let repository_page = self
            .backup_repository
            .query_for_center(&SaveBackupCenterRepositoryQuery {
                game_id: query.game_id.clone(),
                profile_id: query.profile_id.clone(),
                trigger: query.trigger,
                status: query.status,
                search: search.clone(),
                offset: query.offset,
                limit: query.limit,
            })
            .map_err(|_| SaveBackupCenterError::RepositoryUnavailable)?;
        if let Some(repository_page) = repository_page {
            return self.page_from_repository(query, profiles, repository_page);
        }
        let profile_names = profiles
            .iter()
            .map(|profile| (profile.id.clone(), profile.name.clone()))
            .collect::<HashMap<_, _>>();
        let backups = self
            .backup_repository
            .list_for_game(&query.game_id)
            .map_err(|_| SaveBackupCenterError::RepositoryUnavailable)?;
        let filtered = backups
            .iter()
            .filter(|backup| {
                query
                    .profile_id
                    .as_ref()
                    .is_none_or(|profile_id| backup.profile_id == *profile_id)
            })
            .filter(|backup| {
                query
                    .trigger
                    .is_none_or(|trigger| backup.trigger == trigger)
            })
            .filter(|backup| query.status.is_none_or(|status| backup.status == status))
            .filter(|backup| {
                search.as_ref().is_none_or(|search| {
                    backup
                        .notes
                        .as_deref()
                        .is_some_and(|note| note.to_lowercase().contains(search))
                        || profile_names
                            .get(backup.profile_id.as_str())
                            .is_some_and(|name| name.to_lowercase().contains(search))
                })
            })
            .cloned()
            .collect::<Vec<_>>();

        let summary = summarize_backups(&filtered);
        // Profile cards and the filter control need authoritative per-profile facts even
        // when the history list is narrowed by profile, trigger, status, or search.
        let profile_summaries = profiles
            .into_iter()
            .map(|profile| {
                let profile_backups = backups
                    .iter()
                    .filter(|backup| backup.profile_id.as_str() == profile.id)
                    .cloned()
                    .collect::<Vec<_>>();
                let profile_summary = summarize_backups(&profile_backups);
                let settings = self
                    .settings_repository
                    .get_settings(&profile.id)
                    .map_err(|_| SaveBackupCenterError::RepositoryUnavailable)?;
                let retention = settings
                    .as_ref()
                    .map(|settings| settings.retention.clone())
                    .unwrap_or_default();
                let budget_satisfied = retention
                    .max_total_bytes
                    .is_none_or(|limit| profile_summary.archive_bytes <= limit);
                Ok(SaveBackupCenterProfileSummary {
                    profile_id: ProfileId::new(profile.id),
                    profile_name: profile.name,
                    is_active: profile.is_active,
                    steam_account: settings.and_then(|settings| settings.steam_account),
                    retention,
                    backup_count: profile_summary.backup_count,
                    archive_bytes: profile_summary.archive_bytes,
                    protected_count: profile_summary.protected_count,
                    attention_count: profile_summary.attention_count,
                    budget_satisfied,
                })
            })
            .collect::<Result<Vec<_>, SaveBackupCenterError>>()?;
        let total_count = filtered.len();
        let items = filtered
            .into_iter()
            .skip(query.offset)
            .take(query.limit)
            .map(|backup| SaveBackupCenterItem {
                profile_name: profile_names
                    .get(backup.profile_id.as_str())
                    .cloned()
                    .unwrap_or_else(|| backup.profile_id.as_str().to_owned()),
                backup,
            })
            .collect();

        Ok(SaveBackupCenterPage {
            offset: query.offset,
            limit: query.limit,
            total_count,
            summary,
            profiles: profile_summaries,
            items,
        })
    }

    fn page_from_repository(
        &self,
        query: SaveBackupCenterQuery,
        profiles: Vec<hmm_core::Profile>,
        repository_page: hmm_ports::SaveBackupCenterRepositoryPage,
    ) -> Result<SaveBackupCenterPage, SaveBackupCenterError> {
        let profile_facts = repository_page
            .profiles
            .into_iter()
            .map(|profile| (profile.profile_id, profile.facts))
            .collect::<HashMap<_, _>>();
        let profiles = profiles
            .into_iter()
            .map(|profile| {
                let facts = profile_facts
                    .get(&ProfileId::new(profile.id.clone()))
                    .cloned()
                    .unwrap_or_default();
                let settings = self
                    .settings_repository
                    .get_settings(&profile.id)
                    .map_err(|_| SaveBackupCenterError::RepositoryUnavailable)?;
                let retention = settings
                    .as_ref()
                    .map(|settings| settings.retention.clone())
                    .unwrap_or_default();
                let budget_satisfied = retention
                    .max_total_bytes
                    .is_none_or(|limit| facts.archive_bytes <= limit);
                Ok(SaveBackupCenterProfileSummary {
                    profile_id: ProfileId::new(profile.id),
                    profile_name: profile.name,
                    is_active: profile.is_active,
                    steam_account: settings.and_then(|settings| settings.steam_account),
                    retention,
                    backup_count: facts.backup_count,
                    archive_bytes: facts.archive_bytes,
                    protected_count: facts.protected_count,
                    attention_count: facts.attention_count,
                    budget_satisfied,
                })
            })
            .collect::<Result<Vec<_>, SaveBackupCenterError>>()?;
        let summary = SaveBackupCenterSummary {
            backup_count: repository_page.summary.backup_count,
            archive_bytes: repository_page.summary.archive_bytes,
            protected_count: repository_page.summary.protected_count,
            attention_count: repository_page.summary.attention_count,
        };
        let items = repository_page
            .items
            .into_iter()
            .map(|item| SaveBackupCenterItem {
                profile_name: item.profile_name,
                backup: item.backup,
            })
            .collect();
        Ok(SaveBackupCenterPage {
            offset: query.offset,
            limit: query.limit,
            total_count: repository_page.total_count,
            summary,
            profiles,
            items,
        })
    }

    pub fn update_note(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        backup_id: &str,
        note: Option<String>,
    ) -> Result<Option<String>, SaveBackupCenterError> {
        if self
            .profile_repository
            .get(profile_id.as_str())
            .map_err(|_| SaveBackupCenterError::RepositoryUnavailable)?
            .is_none()
        {
            return Err(SaveBackupCenterError::ProfileMissing);
        }
        let note = normalize_note(note)?;
        let updated = self
            .backup_repository
            .update_note(game_id, profile_id, backup_id, note.as_deref())
            .map_err(|_| SaveBackupCenterError::RepositoryUnavailable)?;
        if !updated {
            return Err(SaveBackupCenterError::BackupMissing);
        }
        Ok(note)
    }

    pub fn run_retention(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<SaveBackupRetentionReport, SaveBackupCenterError> {
        let _scope = self
            .scope_registry
            .reserve_maintenance(game_id, profile_id)
            .map_err(|_| SaveBackupCenterError::TaskConflict)?;
        let _cross_process_guard = self
            .scope_registry
            .acquire_cross_process_for_maintenance(game_id, profile_id)?;
        let mut report = self
            .save_backup_service
            .run_retention(game_id, profile_id)
            .map_err(|error| match error {
                SaveBackupError::ProfileMissing => SaveBackupCenterError::ProfileMissing,
                _ => SaveBackupCenterError::RetentionFailed,
            })?;
        if self
            .record_retention_audit(game_id, profile_id, &report)
            .is_err()
        {
            report.evidence_degraded = true;
        }
        Ok(report)
    }

    fn record_retention_audit(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        report: &SaveBackupRetentionReport,
    ) -> anyhow::Result<()> {
        let mut fields = BTreeMap::new();
        fields.insert("game_id".to_owned(), game_id.as_str().to_owned());
        fields.insert("profile_id".to_owned(), profile_id.as_str().to_owned());
        fields.insert("outcome".to_owned(), report.outcome.as_str().to_owned());
        fields.insert("scanned_count".to_owned(), report.scanned_count.to_string());
        fields.insert(
            "protected_count".to_owned(),
            report.protected_count.to_string(),
        );
        fields.insert("problem_count".to_owned(), report.problem_count.to_string());
        fields.insert(
            "candidate_count".to_owned(),
            report.candidate_count.to_string(),
        );
        fields.insert("deleted_count".to_owned(), report.deleted_count.to_string());
        fields.insert("partial_count".to_owned(), report.partial_count.to_string());
        fields.insert("blocked_count".to_owned(), report.blocked_count.to_string());
        fields.insert(
            "archive_bytes_before".to_owned(),
            report.archive_bytes_before.to_string(),
        );
        fields.insert(
            "archive_bytes_after".to_owned(),
            report.archive_bytes_after.to_string(),
        );
        fields.insert(
            "released_bytes".to_owned(),
            report.released_bytes.to_string(),
        );
        fields.insert(
            "budget_satisfied".to_owned(),
            report.budget_satisfied.to_string(),
        );
        let result = match report.outcome {
            SaveBackupRetentionOutcome::WithinPolicy | SaveBackupRetentionOutcome::Completed => {
                "success"
            }
            SaveBackupRetentionOutcome::Partial | SaveBackupRetentionOutcome::Blocked => "warning",
            SaveBackupRetentionOutcome::Failed => "failure",
        };
        self.audit_log.record_with_policy(
            AuditLogEvent {
                timestamp_unix_millis: self.clock.now_unix_millis().unwrap_or_default(),
                category: "save_backup".to_owned(),
                operation: "retention_pruning".to_owned(),
                result: result.to_owned(),
                fields,
            },
            AuditWriteFailurePolicy::for_commit_result(result),
        )
    }
}

fn normalize_search(search: Option<String>) -> Result<Option<String>, SaveBackupCenterError> {
    let search = search
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());
    if search
        .as_ref()
        .is_some_and(|value| value.chars().count() > MAX_SAVE_BACKUP_CENTER_SEARCH_CHARS)
    {
        return Err(SaveBackupCenterError::QueryInvalid);
    }
    Ok(search)
}

fn normalize_note(note: Option<String>) -> Result<Option<String>, SaveBackupCenterError> {
    let note = note
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if note
        .as_ref()
        .is_some_and(|value| value.chars().count() > MAX_SAVE_BACKUP_NOTE_CHARS)
    {
        return Err(SaveBackupCenterError::NoteInvalid);
    }
    Ok(note)
}

fn summarize_backups(backups: &[SaveBackupSummary]) -> SaveBackupCenterSummary {
    SaveBackupCenterSummary {
        backup_count: backups.len() as u32,
        archive_bytes: backups
            .iter()
            .filter(|backup| backup.status != SaveBackupStatus::DeletedByRetention)
            .fold(0_u64, |total, backup| {
                total.saturating_add(
                    backup.archive_size_bytes.saturating_sub(
                        backup
                            .retention_released_bytes
                            .min(backup.archive_size_bytes),
                    ),
                )
            }),
        protected_count: backups
            .iter()
            .filter(|backup| {
                backup.trigger == SaveBackupTrigger::PreRestore
                    && backup.status != SaveBackupStatus::DeletedByRetention
            })
            .count() as u32,
        attention_count: backups
            .iter()
            .filter(|backup| {
                matches!(
                    backup.status,
                    SaveBackupStatus::RetentionPending
                        | SaveBackupStatus::RetentionPartial
                        | SaveBackupStatus::Missing
                        | SaveBackupStatus::Invalid
                )
            })
            .count() as u32,
    }
}
