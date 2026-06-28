use hmm_core::{
    InstallManifest, InstallRecoveryRecord, InstallRecoveryRecordStatus, InstalledFileSummary,
    ModId, ProfileId,
};
use hmm_ports::{
    InstallBackupStore, InstallGameFileSystem, InstallManifestRepository,
    InstallRecoveryRecordRepository,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRecoveryScanRequest {
    pub profile_id: ProfileId,
    pub mod_ids: Vec<ModId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallRecoveryStatus {
    NotInstalled,
    Completed,
    RollbackRequired,
    RepairRequired,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InstallRecoveryIssue {
    MissingInstalledFileSummary,
    TargetMissing,
    TargetChanged,
    TargetReadFailed,
    BackupMissing,
    BackupReadFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRecoveryIssueSummary {
    pub issue: InstallRecoveryIssue,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRecoverySummary {
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub status: InstallRecoveryStatus,
    pub managed_file_count: usize,
    pub backup_count: usize,
    pub issue_count: usize,
    pub issues: Vec<InstallRecoveryIssueSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallRecoveryActionKind {
    RollbackInstall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallRecoveryActionAvailability {
    Available,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InstallRecoveryActionBlockReason {
    RollbackStateMissing,
    MissingInstalledFileSummary,
    TargetMissing,
    TargetChanged,
    TargetReadFailed,
    BackupMissing,
    BackupReadFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRecoveryActionBlockReasonSummary {
    pub reason: InstallRecoveryActionBlockReason,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRecoveryActionPreviewRequest {
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub action_kind: InstallRecoveryActionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRecoveryActionRequest {
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub action_kind: InstallRecoveryActionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRecoveryActionPreview {
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub action_kind: InstallRecoveryActionKind,
    pub availability: InstallRecoveryActionAvailability,
    pub remove_file_count: usize,
    pub restore_file_count: usize,
    pub backup_count: usize,
    pub blocking_issue_count: usize,
    pub blocking_reasons: Vec<InstallRecoveryActionBlockReasonSummary>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallRecoveryScanError {
    #[error("game instance is unavailable")]
    GameInstanceUnavailable,
    #[error("install recovery scan failed")]
    ManifestUnavailable,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallRecoveryActionPreviewError {
    #[error("game instance is unavailable")]
    GameInstanceUnavailable,
    #[error("install recovery action preview failed")]
    PreviewUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRecoveryActionResult {
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub action_kind: InstallRecoveryActionKind,
    pub remove_file_count: usize,
    pub restore_file_count: usize,
    pub backup_count: usize,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallRecoveryActionError {
    #[error("install recovery action failed")]
    ActionUnavailable,
    #[error("install recovery action is blocked")]
    Blocked {
        reasons: Vec<InstallRecoveryActionBlockReasonSummary>,
    },
    #[error("failed to remove installed file during recovery rollback")]
    RemoveFailed,
    #[error("failed to restore backup during recovery rollback")]
    RestoreFailed,
    #[error("failed to save recovery record after rollback")]
    RecoveryRecordSaveFailed,
    #[error("failed to rollback recovery action after {failed_phase:?}")]
    RollbackFailed {
        failed_phase: InstallRecoveryActionPhase,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallRecoveryActionPhase {
    Revalidate,
    Remove,
    Restore,
    RecoveryRecordSave,
}

#[derive(Clone)]
pub struct InstallRecoveryScanService {
    game_files: Arc<dyn InstallGameFileSystem>,
    backup_store: Arc<dyn InstallBackupStore>,
    manifest_repository: Arc<dyn InstallManifestRepository>,
    recovery_record_repository: Option<Arc<dyn InstallRecoveryRecordRepository>>,
}

impl InstallRecoveryScanService {
    pub fn new(
        game_files: Arc<dyn InstallGameFileSystem>,
        backup_store: Arc<dyn InstallBackupStore>,
        manifest_repository: Arc<dyn InstallManifestRepository>,
    ) -> Self {
        Self {
            game_files,
            backup_store,
            manifest_repository,
            recovery_record_repository: None,
        }
    }

    pub fn new_with_recovery_records(
        game_files: Arc<dyn InstallGameFileSystem>,
        backup_store: Arc<dyn InstallBackupStore>,
        manifest_repository: Arc<dyn InstallManifestRepository>,
        recovery_record_repository: Arc<dyn InstallRecoveryRecordRepository>,
    ) -> Self {
        Self {
            game_files,
            backup_store,
            manifest_repository,
            recovery_record_repository: Some(recovery_record_repository),
        }
    }

    pub fn scan(
        &self,
        request: InstallRecoveryScanRequest,
    ) -> Result<Vec<InstallRecoverySummary>, InstallRecoveryScanError> {
        let manifest = self
            .manifest_repository
            .load_manifest(&request.profile_id)
            .map_err(|_| InstallRecoveryScanError::ManifestUnavailable)?;

        let scan_all_mods = request.mod_ids.is_empty();
        let recovery_records = if scan_all_mods {
            self.list_recovery_records(&request.profile_id)?
        } else {
            BTreeMap::new()
        };

        let mod_ids = if scan_all_mods {
            recovery_scan_mod_ids(manifest.as_ref(), &recovery_records)
        } else {
            request.mod_ids
        };

        let mut summaries = Vec::with_capacity(mod_ids.len());
        for mod_id in mod_ids {
            let recovery_record = if let Some(record) = recovery_records.get(mod_id.as_str()) {
                Some(record.clone())
            } else if scan_all_mods {
                None
            } else {
                self.load_recovery_record(&request.profile_id, &mod_id)?
            };
            summaries.push(self.scan_mod(
                &request.profile_id,
                &mod_id,
                manifest.as_ref(),
                recovery_record.as_ref(),
            ));
        }

        Ok(summaries)
    }

    fn scan_mod(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
        manifest: Option<&InstallManifest>,
        recovery_record: Option<&InstallRecoveryRecord>,
    ) -> InstallRecoverySummary {
        if let Some(summary) = recovery_record_summary(profile_id, mod_id, recovery_record) {
            return summary;
        }

        let Some(manifest) = manifest else {
            return InstallRecoverySummary {
                profile_id: profile_id.clone(),
                mod_id: mod_id.clone(),
                status: InstallRecoveryStatus::NotInstalled,
                managed_file_count: 0,
                backup_count: 0,
                issue_count: 0,
                issues: Vec::new(),
            };
        };

        let entries: Vec<_> = manifest
            .entries
            .iter()
            .filter(|entry| entry.mod_id == *mod_id)
            .collect();
        let managed_file_count = entries.len();
        let backup_count = entries
            .iter()
            .filter(|entry| entry.backup_ref.is_some())
            .count();

        if managed_file_count == 0 {
            return InstallRecoverySummary {
                profile_id: profile_id.clone(),
                mod_id: mod_id.clone(),
                status: InstallRecoveryStatus::NotInstalled,
                managed_file_count,
                backup_count,
                issue_count: 0,
                issues: Vec::new(),
            };
        }

        let mut issues = BTreeMap::new();
        let mut has_unknown_issue = false;

        for entry in entries {
            let Some(expected) = entry.installed_file.as_ref() else {
                add_issue(
                    &mut issues,
                    InstallRecoveryIssue::MissingInstalledFileSummary,
                );
                continue;
            };

            match self.game_files.read_game_file(&entry.target_path) {
                Ok(Some(current_bytes)) if installed_file_summary(&current_bytes) == *expected => {}
                Ok(Some(_)) => add_issue(&mut issues, InstallRecoveryIssue::TargetChanged),
                Ok(None) => add_issue(&mut issues, InstallRecoveryIssue::TargetMissing),
                Err(_) => {
                    add_issue(&mut issues, InstallRecoveryIssue::TargetReadFailed);
                    has_unknown_issue = true;
                }
            }

            if let Some(backup_ref) = &entry.backup_ref {
                match self.backup_store.read_backup(backup_ref) {
                    Ok(Some(_)) => {}
                    Ok(None) => add_issue(&mut issues, InstallRecoveryIssue::BackupMissing),
                    Err(_) => {
                        add_issue(&mut issues, InstallRecoveryIssue::BackupReadFailed);
                        has_unknown_issue = true;
                    }
                }
            }
        }
        let issue_summaries: Vec<_> = issues
            .into_iter()
            .map(|(issue, count)| InstallRecoveryIssueSummary { issue, count })
            .collect();
        let issue_count = issue_summaries.iter().map(|summary| summary.count).sum();

        InstallRecoverySummary {
            profile_id: profile_id.clone(),
            mod_id: mod_id.clone(),
            status: if has_unknown_issue {
                InstallRecoveryStatus::Unknown
            } else if issue_count == 0 {
                InstallRecoveryStatus::Completed
            } else {
                InstallRecoveryStatus::RepairRequired
            },
            managed_file_count,
            backup_count,
            issue_count,
            issues: issue_summaries,
        }
    }

    fn load_recovery_record(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Result<Option<InstallRecoveryRecord>, InstallRecoveryScanError> {
        let Some(repository) = &self.recovery_record_repository else {
            return Ok(None);
        };

        repository
            .load_record(profile_id, mod_id)
            .map_err(|_| InstallRecoveryScanError::ManifestUnavailable)
    }

    fn list_recovery_records(
        &self,
        profile_id: &ProfileId,
    ) -> Result<BTreeMap<String, InstallRecoveryRecord>, InstallRecoveryScanError> {
        let Some(repository) = &self.recovery_record_repository else {
            return Ok(BTreeMap::new());
        };

        let records = repository
            .list_records(profile_id)
            .map_err(|_| InstallRecoveryScanError::ManifestUnavailable)?;

        Ok(records
            .into_iter()
            .map(|record| (record.mod_id.as_str().to_owned(), record))
            .collect())
    }
}

#[derive(Clone)]
pub struct InstallRecoveryActionPreviewService {
    game_files: Arc<dyn InstallGameFileSystem>,
    backup_store: Arc<dyn InstallBackupStore>,
    recovery_record_repository: Arc<dyn InstallRecoveryRecordRepository>,
}

#[derive(Clone)]
pub struct InstallRecoveryActionService {
    game_files: Arc<dyn InstallGameFileSystem>,
    backup_store: Arc<dyn InstallBackupStore>,
    recovery_record_repository: Arc<dyn InstallRecoveryRecordRepository>,
}

struct PreparedRecoveryAction {
    target_path: hmm_core::InstallTargetPath,
    current_bytes: Vec<u8>,
    backup_bytes: Option<Vec<u8>>,
}

struct AppliedRecoveryAction {
    target_path: hmm_core::InstallTargetPath,
    previous_bytes: Vec<u8>,
}

impl InstallRecoveryActionPreviewService {
    pub fn new(
        game_files: Arc<dyn InstallGameFileSystem>,
        backup_store: Arc<dyn InstallBackupStore>,
        recovery_record_repository: Arc<dyn InstallRecoveryRecordRepository>,
    ) -> Self {
        Self {
            game_files,
            backup_store,
            recovery_record_repository,
        }
    }

    pub fn preview(
        &self,
        request: InstallRecoveryActionPreviewRequest,
    ) -> Result<InstallRecoveryActionPreview, InstallRecoveryActionPreviewError> {
        match request.action_kind {
            InstallRecoveryActionKind::RollbackInstall => self.preview_rollback_install(request),
        }
    }

    fn preview_rollback_install(
        &self,
        request: InstallRecoveryActionPreviewRequest,
    ) -> Result<InstallRecoveryActionPreview, InstallRecoveryActionPreviewError> {
        let record = self
            .recovery_record_repository
            .load_record(&request.profile_id, &request.mod_id)
            .map_err(|_| InstallRecoveryActionPreviewError::PreviewUnavailable)?;

        let Some(record) = record else {
            return Ok(blocked_recovery_action_preview(
                request,
                0,
                0,
                0,
                [(InstallRecoveryActionBlockReason::RollbackStateMissing, 1)],
            ));
        };

        let is_rollback_state = matches!(
            record.status,
            InstallRecoveryRecordStatus::Committing | InstallRecoveryRecordStatus::RollbackRequired
        );
        if !is_rollback_state || record.entries.is_empty() {
            return Ok(blocked_recovery_action_preview(
                request,
                0,
                0,
                0,
                [(InstallRecoveryActionBlockReason::RollbackStateMissing, 1)],
            ));
        }

        let mut reasons = BTreeMap::new();
        let mut remove_file_count = 0;
        let mut restore_file_count = 0;
        let mut backup_count = 0;

        for entry in &record.entries {
            if entry.backup_ref.is_some() {
                restore_file_count += 1;
                backup_count += 1;
            } else {
                remove_file_count += 1;
            }

            if let Some(expected) = entry.installed_file.as_ref() {
                match self.game_files.read_game_file(&entry.target_path) {
                    Ok(Some(current_bytes))
                        if installed_file_summary(&current_bytes) == *expected => {}
                    Ok(Some(_)) => add_preview_reason(
                        &mut reasons,
                        InstallRecoveryActionBlockReason::TargetChanged,
                    ),
                    Ok(None) => add_preview_reason(
                        &mut reasons,
                        InstallRecoveryActionBlockReason::TargetMissing,
                    ),
                    Err(_) => add_preview_reason(
                        &mut reasons,
                        InstallRecoveryActionBlockReason::TargetReadFailed,
                    ),
                }
            } else {
                add_preview_reason(
                    &mut reasons,
                    InstallRecoveryActionBlockReason::MissingInstalledFileSummary,
                );
            }

            if let Some(backup_ref) = &entry.backup_ref {
                match self.backup_store.read_backup(backup_ref) {
                    Ok(Some(_)) => {}
                    Ok(None) => add_preview_reason(
                        &mut reasons,
                        InstallRecoveryActionBlockReason::BackupMissing,
                    ),
                    Err(_) => add_preview_reason(
                        &mut reasons,
                        InstallRecoveryActionBlockReason::BackupReadFailed,
                    ),
                }
            }
        }

        let blocking_reasons: Vec<_> = reasons
            .into_iter()
            .map(|(reason, count)| InstallRecoveryActionBlockReasonSummary { reason, count })
            .collect();
        let blocking_issue_count = blocking_reasons.iter().map(|summary| summary.count).sum();

        Ok(InstallRecoveryActionPreview {
            profile_id: request.profile_id,
            mod_id: request.mod_id,
            action_kind: request.action_kind,
            availability: if blocking_issue_count == 0 {
                InstallRecoveryActionAvailability::Available
            } else {
                InstallRecoveryActionAvailability::Blocked
            },
            remove_file_count,
            restore_file_count,
            backup_count,
            blocking_issue_count,
            blocking_reasons,
        })
    }
}

impl InstallRecoveryActionService {
    pub fn new(
        game_files: Arc<dyn InstallGameFileSystem>,
        backup_store: Arc<dyn InstallBackupStore>,
        recovery_record_repository: Arc<dyn InstallRecoveryRecordRepository>,
    ) -> Self {
        Self {
            game_files,
            backup_store,
            recovery_record_repository,
        }
    }

    pub fn run(
        &self,
        request: InstallRecoveryActionRequest,
    ) -> Result<InstallRecoveryActionResult, InstallRecoveryActionError> {
        match request.action_kind {
            InstallRecoveryActionKind::RollbackInstall => self.rollback_install(request),
        }
    }

    fn rollback_install(
        &self,
        request: InstallRecoveryActionRequest,
    ) -> Result<InstallRecoveryActionResult, InstallRecoveryActionError> {
        let mut record = self
            .recovery_record_repository
            .load_record(&request.profile_id, &request.mod_id)
            .map_err(|_| InstallRecoveryActionError::ActionUnavailable)?
            .ok_or_else(|| {
                blocked_recovery_action_error([(
                    InstallRecoveryActionBlockReason::RollbackStateMissing,
                    1,
                )])
            })?;

        let is_rollback_state = matches!(
            record.status,
            InstallRecoveryRecordStatus::Committing | InstallRecoveryRecordStatus::RollbackRequired
        );
        if !is_rollback_state || record.entries.is_empty() {
            return Err(blocked_recovery_action_error([(
                InstallRecoveryActionBlockReason::RollbackStateMissing,
                1,
            )]));
        }

        let mut reasons = BTreeMap::new();
        let mut prepared_actions = Vec::with_capacity(record.entries.len());
        let mut remove_file_count = 0;
        let mut restore_file_count = 0;
        let mut backup_count = 0;

        for entry in &record.entries {
            if entry.backup_ref.is_some() {
                restore_file_count += 1;
                backup_count += 1;
            } else {
                remove_file_count += 1;
            }

            let mut current_bytes = None;
            if let Some(expected) = entry.installed_file.as_ref() {
                match self.game_files.read_game_file(&entry.target_path) {
                    Ok(Some(bytes)) if installed_file_summary(&bytes) == *expected => {
                        current_bytes = Some(bytes);
                    }
                    Ok(Some(_)) => add_preview_reason(
                        &mut reasons,
                        InstallRecoveryActionBlockReason::TargetChanged,
                    ),
                    Ok(None) => add_preview_reason(
                        &mut reasons,
                        InstallRecoveryActionBlockReason::TargetMissing,
                    ),
                    Err(_) => add_preview_reason(
                        &mut reasons,
                        InstallRecoveryActionBlockReason::TargetReadFailed,
                    ),
                }
            } else {
                add_preview_reason(
                    &mut reasons,
                    InstallRecoveryActionBlockReason::MissingInstalledFileSummary,
                );
            }

            let backup_bytes = if let Some(backup_ref) = &entry.backup_ref {
                match self.backup_store.read_backup(backup_ref) {
                    Ok(Some(bytes)) => Some(bytes),
                    Ok(None) => {
                        add_preview_reason(
                            &mut reasons,
                            InstallRecoveryActionBlockReason::BackupMissing,
                        );
                        None
                    }
                    Err(_) => {
                        add_preview_reason(
                            &mut reasons,
                            InstallRecoveryActionBlockReason::BackupReadFailed,
                        );
                        None
                    }
                }
            } else {
                None
            };

            if let Some(current_bytes) = current_bytes {
                prepared_actions.push(PreparedRecoveryAction {
                    target_path: entry.target_path.clone(),
                    current_bytes,
                    backup_bytes,
                });
            }
        }

        if !reasons.is_empty() {
            return Err(InstallRecoveryActionError::Blocked {
                reasons: preview_reason_summaries(reasons),
            });
        }

        let mut applied_actions = Vec::with_capacity(prepared_actions.len());
        for action in &prepared_actions {
            if let Err(error) = self.revalidate_prepared_action(action) {
                return Err(self.rollback_or_error(
                    &applied_actions,
                    InstallRecoveryActionPhase::Revalidate,
                    error,
                ));
            }

            if let Some(backup_bytes) = &action.backup_bytes {
                if self
                    .game_files
                    .write_game_file(&action.target_path, backup_bytes)
                    .is_err()
                {
                    return Err(self.rollback_or_error(
                        &applied_actions,
                        InstallRecoveryActionPhase::Restore,
                        InstallRecoveryActionError::RestoreFailed,
                    ));
                }
            } else if self
                .game_files
                .remove_game_file(&action.target_path)
                .is_err()
            {
                return Err(self.rollback_or_error(
                    &applied_actions,
                    InstallRecoveryActionPhase::Remove,
                    InstallRecoveryActionError::RemoveFailed,
                ));
            }

            applied_actions.push(AppliedRecoveryAction {
                target_path: action.target_path.clone(),
                previous_bytes: action.current_bytes.clone(),
            });
        }

        let committing_transition_failed = record.status == InstallRecoveryRecordStatus::Committing
            && record
                .transition_to(InstallRecoveryRecordStatus::RollbackRequired)
                .is_err();
        if committing_transition_failed
            || record
                .transition_to(InstallRecoveryRecordStatus::RolledBack)
                .is_err()
            || self
                .recovery_record_repository
                .save_record(&record)
                .is_err()
        {
            return Err(self.rollback_or_error(
                &applied_actions,
                InstallRecoveryActionPhase::RecoveryRecordSave,
                InstallRecoveryActionError::RecoveryRecordSaveFailed,
            ));
        }

        Ok(InstallRecoveryActionResult {
            profile_id: request.profile_id,
            mod_id: request.mod_id,
            action_kind: request.action_kind,
            remove_file_count,
            restore_file_count,
            backup_count,
        })
    }

    fn revalidate_prepared_action(
        &self,
        action: &PreparedRecoveryAction,
    ) -> Result<(), InstallRecoveryActionError> {
        match self.game_files.read_game_file(&action.target_path) {
            Ok(Some(bytes)) if bytes == action.current_bytes => Ok(()),
            Ok(Some(_)) => Err(blocked_recovery_action_error([(
                InstallRecoveryActionBlockReason::TargetChanged,
                1,
            )])),
            Ok(None) => Err(blocked_recovery_action_error([(
                InstallRecoveryActionBlockReason::TargetMissing,
                1,
            )])),
            Err(_) => Err(blocked_recovery_action_error([(
                InstallRecoveryActionBlockReason::TargetReadFailed,
                1,
            )])),
        }
    }

    fn rollback_or_error(
        &self,
        applied_actions: &[AppliedRecoveryAction],
        failed_phase: InstallRecoveryActionPhase,
        fallback: InstallRecoveryActionError,
    ) -> InstallRecoveryActionError {
        match self.rollback_applied_actions(applied_actions) {
            Ok(()) => fallback,
            Err(()) => InstallRecoveryActionError::RollbackFailed { failed_phase },
        }
    }

    fn rollback_applied_actions(
        &self,
        applied_actions: &[AppliedRecoveryAction],
    ) -> Result<(), ()> {
        for action in applied_actions.iter().rev() {
            self.game_files
                .write_game_file(&action.target_path, &action.previous_bytes)
                .map_err(|_| ())?;
        }
        Ok(())
    }
}

fn recovery_record_summary(
    profile_id: &ProfileId,
    mod_id: &ModId,
    recovery_record: Option<&InstallRecoveryRecord>,
) -> Option<InstallRecoverySummary> {
    let recovery_record = recovery_record?;
    let status = match recovery_record.status {
        InstallRecoveryRecordStatus::Committing | InstallRecoveryRecordStatus::RollbackRequired => {
            InstallRecoveryStatus::RollbackRequired
        }
        InstallRecoveryRecordStatus::RepairRequired => InstallRecoveryStatus::RepairRequired,
        InstallRecoveryRecordStatus::Planned
        | InstallRecoveryRecordStatus::Completed
        | InstallRecoveryRecordStatus::RolledBack => return None,
    };

    Some(InstallRecoverySummary {
        profile_id: profile_id.clone(),
        mod_id: mod_id.clone(),
        status,
        managed_file_count: recovery_record.entries.len(),
        backup_count: recovery_record
            .entries
            .iter()
            .filter(|entry| entry.backup_ref.is_some())
            .count(),
        issue_count: 0,
        issues: Vec::new(),
    })
}

fn add_issue(issues: &mut BTreeMap<InstallRecoveryIssue, usize>, issue: InstallRecoveryIssue) {
    *issues.entry(issue).or_default() += 1;
}

fn add_preview_reason(
    reasons: &mut BTreeMap<InstallRecoveryActionBlockReason, usize>,
    reason: InstallRecoveryActionBlockReason,
) {
    *reasons.entry(reason).or_default() += 1;
}

fn preview_reason_summaries(
    reasons: BTreeMap<InstallRecoveryActionBlockReason, usize>,
) -> Vec<InstallRecoveryActionBlockReasonSummary> {
    reasons
        .into_iter()
        .map(|(reason, count)| InstallRecoveryActionBlockReasonSummary { reason, count })
        .collect()
}

fn blocked_recovery_action_error(
    reasons: impl IntoIterator<Item = (InstallRecoveryActionBlockReason, usize)>,
) -> InstallRecoveryActionError {
    InstallRecoveryActionError::Blocked {
        reasons: preview_reason_summaries(reasons.into_iter().collect()),
    }
}

fn blocked_recovery_action_preview(
    request: InstallRecoveryActionPreviewRequest,
    remove_file_count: usize,
    restore_file_count: usize,
    backup_count: usize,
    reasons: impl IntoIterator<Item = (InstallRecoveryActionBlockReason, usize)>,
) -> InstallRecoveryActionPreview {
    let blocking_reasons: Vec<_> = reasons
        .into_iter()
        .map(|(reason, count)| InstallRecoveryActionBlockReasonSummary { reason, count })
        .collect();
    let blocking_issue_count = blocking_reasons.iter().map(|summary| summary.count).sum();

    InstallRecoveryActionPreview {
        profile_id: request.profile_id,
        mod_id: request.mod_id,
        action_kind: request.action_kind,
        availability: InstallRecoveryActionAvailability::Blocked,
        remove_file_count,
        restore_file_count,
        backup_count,
        blocking_issue_count,
        blocking_reasons,
    }
}

fn manifest_mod_ids(manifest: &InstallManifest) -> Vec<ModId> {
    let mut mod_ids = BTreeMap::new();

    for entry in &manifest.entries {
        mod_ids
            .entry(entry.mod_id.as_str().to_owned())
            .or_insert_with(|| entry.mod_id.clone());
    }

    mod_ids.into_values().collect()
}

fn recovery_scan_mod_ids(
    manifest: Option<&InstallManifest>,
    recovery_records: &BTreeMap<String, InstallRecoveryRecord>,
) -> Vec<ModId> {
    let mut mod_ids = BTreeMap::new();

    if let Some(manifest) = manifest {
        for mod_id in manifest_mod_ids(manifest) {
            mod_ids.insert(mod_id.as_str().to_owned(), mod_id);
        }
    }

    for record in recovery_records.values() {
        mod_ids
            .entry(record.mod_id.as_str().to_owned())
            .or_insert_with(|| record.mod_id.clone());
    }

    mod_ids.into_values().collect()
}

fn installed_file_summary(bytes: &[u8]) -> InstalledFileSummary {
    InstalledFileSummary {
        size_bytes: bytes.len() as u64,
        sha256: sha256_hex(bytes),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        FileLayer, InstallManifest, InstallManifestEntry, InstallRecoveryRecord,
        InstallRecoveryRecordEntry, InstallRecoveryRecordStatus, InstallTargetPath,
        InstalledFileSummary, ModId, PackageFileId, ProfileId,
    };
    use hmm_ports::{
        InstallBackupStore, InstallGameFileSystem, InstallManifestRepository,
        InstallRecoveryRecordRepository,
    };
    use sha2::{Digest, Sha256};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct FakeGameFiles {
        files: Mutex<BTreeMap<String, Vec<u8>>>,
        error_targets: Mutex<BTreeSet<String>>,
        mutate_after_read: Mutex<BTreeMap<String, Vec<u8>>>,
        writes: Mutex<Vec<String>>,
        removals: Mutex<Vec<String>>,
    }

    impl InstallGameFileSystem for FakeGameFiles {
        fn read_game_file(
            &self,
            target_path: &InstallTargetPath,
        ) -> anyhow::Result<Option<Vec<u8>>> {
            if self
                .error_targets
                .lock()
                .expect("error targets lock")
                .contains(target_path.as_str())
            {
                anyhow::bail!("simulated target read failure");
            }

            let current = self
                .files
                .lock()
                .expect("files lock")
                .get(target_path.as_str())
                .cloned();
            if let Some(replacement) = self
                .mutate_after_read
                .lock()
                .expect("mutate after read lock")
                .remove(target_path.as_str())
            {
                self.files
                    .lock()
                    .expect("files lock")
                    .insert(target_path.as_str().to_owned(), replacement);
            }
            Ok(current)
        }

        fn write_game_file(
            &self,
            target_path: &InstallTargetPath,
            bytes: &[u8],
        ) -> anyhow::Result<()> {
            self.writes
                .lock()
                .expect("writes lock")
                .push(target_path.as_str().to_owned());
            self.files
                .lock()
                .expect("files lock")
                .insert(target_path.as_str().to_owned(), bytes.to_vec());
            Ok(())
        }

        fn remove_game_file(&self, target_path: &InstallTargetPath) -> anyhow::Result<()> {
            self.removals
                .lock()
                .expect("removals lock")
                .push(target_path.as_str().to_owned());
            self.files
                .lock()
                .expect("files lock")
                .remove(target_path.as_str());
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeBackups {
        backups: Mutex<BTreeMap<String, Vec<u8>>>,
        error_refs: Mutex<BTreeSet<String>>,
    }

    impl InstallBackupStore for FakeBackups {
        fn store_backup(
            &self,
            _target_path: &InstallTargetPath,
            _bytes: &[u8],
        ) -> anyhow::Result<String> {
            panic!("recovery scan must be read-only")
        }

        fn read_backup(&self, backup_ref: &str) -> anyhow::Result<Option<Vec<u8>>> {
            if self
                .error_refs
                .lock()
                .expect("error refs lock")
                .contains(backup_ref)
            {
                anyhow::bail!("simulated backup read failure");
            }

            Ok(self
                .backups
                .lock()
                .expect("backups lock")
                .get(backup_ref)
                .cloned())
        }

        fn remove_backup(&self, _backup_ref: &str) -> anyhow::Result<()> {
            panic!("recovery scan must be read-only")
        }
    }

    struct FakeManifests {
        manifest: Option<InstallManifest>,
    }

    impl InstallManifestRepository for FakeManifests {
        fn load_manifest(
            &self,
            _profile_id: &ProfileId,
        ) -> anyhow::Result<Option<InstallManifest>> {
            Ok(self.manifest.clone())
        }

        fn save_manifest(&self, _manifest: &InstallManifest) -> anyhow::Result<()> {
            panic!("recovery scan must be read-only")
        }
    }

    #[derive(Default)]
    struct FakeRecoveryRecords {
        records: Mutex<BTreeMap<String, InstallRecoveryRecord>>,
        loaded_records: Mutex<Vec<(ProfileId, ModId)>>,
        listed_profiles: Mutex<Vec<ProfileId>>,
        removed_records: Mutex<Vec<(ProfileId, ModId)>>,
        fail_saves: Mutex<bool>,
    }

    impl FakeRecoveryRecords {
        fn insert(&self, record: InstallRecoveryRecord) {
            self.records
                .lock()
                .expect("records lock")
                .insert(record_key(&record.profile_id, &record.mod_id), record);
        }
    }

    impl InstallRecoveryRecordRepository for FakeRecoveryRecords {
        fn load_record(
            &self,
            profile_id: &ProfileId,
            mod_id: &ModId,
        ) -> anyhow::Result<Option<InstallRecoveryRecord>> {
            self.loaded_records
                .lock()
                .expect("loaded records lock")
                .push((profile_id.clone(), mod_id.clone()));
            Ok(self
                .records
                .lock()
                .expect("records lock")
                .get(&record_key(profile_id, mod_id))
                .cloned())
        }

        fn list_records(
            &self,
            profile_id: &ProfileId,
        ) -> anyhow::Result<Vec<InstallRecoveryRecord>> {
            self.listed_profiles
                .lock()
                .expect("listed profiles lock")
                .push(profile_id.clone());
            Ok(self
                .records
                .lock()
                .expect("records lock")
                .values()
                .filter(|record| record.profile_id == *profile_id)
                .cloned()
                .collect())
        }

        fn save_record(&self, record: &InstallRecoveryRecord) -> anyhow::Result<()> {
            if *self.fail_saves.lock().expect("fail saves lock") {
                anyhow::bail!("simulated recovery record save failure");
            }
            self.records.lock().expect("records lock").insert(
                record_key(&record.profile_id, &record.mod_id),
                record.clone(),
            );
            Ok(())
        }

        fn remove_record(&self, profile_id: &ProfileId, mod_id: &ModId) -> anyhow::Result<()> {
            self.records
                .lock()
                .expect("records lock")
                .remove(&record_key(profile_id, mod_id));
            self.removed_records
                .lock()
                .expect("removed records lock")
                .push((profile_id.clone(), mod_id.clone()));
            Ok(())
        }
    }

    #[test]
    fn scan_marks_rollback_required_from_committing_recovery_record_without_manifest() {
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("target path");
        let modded_bytes = b"modded model".to_vec();
        let game_files = Arc::new(FakeGameFiles::default());
        let backups = Arc::new(FakeBackups::default());
        let manifests = Arc::new(FakeManifests { manifest: None });
        let recovery_records = Arc::new(FakeRecoveryRecords::default());
        recovery_records.insert(recovery_record(
            InstallRecoveryRecordStatus::Committing,
            target,
            ModId::new("mod-a"),
            Some(summary(&modded_bytes)),
            None,
        ));
        let service = InstallRecoveryScanService::new_with_recovery_records(
            game_files,
            backups,
            manifests,
            recovery_records,
        );

        let summaries = service
            .scan(InstallRecoveryScanRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![ModId::new("mod-a")],
            })
            .expect("scan should use durable recovery records");

        assert_eq!(
            summaries,
            vec![InstallRecoverySummary {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                status: InstallRecoveryStatus::RollbackRequired,
                managed_file_count: 1,
                backup_count: 0,
                issue_count: 0,
                issues: Vec::new(),
            }]
        );
    }

    #[test]
    fn scan_does_not_promote_planned_recovery_record_to_rollback_required() {
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("target path");
        let game_files = Arc::new(FakeGameFiles::default());
        let backups = Arc::new(FakeBackups::default());
        let manifests = Arc::new(FakeManifests { manifest: None });
        let recovery_records = Arc::new(FakeRecoveryRecords::default());
        recovery_records.insert(recovery_record(
            InstallRecoveryRecordStatus::Planned,
            target,
            ModId::new("mod-a"),
            None,
            None,
        ));
        let service = InstallRecoveryScanService::new_with_recovery_records(
            game_files,
            backups,
            manifests,
            recovery_records,
        );

        let summaries = service
            .scan(InstallRecoveryScanRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![ModId::new("mod-a")],
            })
            .expect("planned records should not become rollback_required");

        assert_eq!(
            summaries,
            vec![InstallRecoverySummary {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                status: InstallRecoveryStatus::NotInstalled,
                managed_file_count: 0,
                backup_count: 0,
                issue_count: 0,
                issues: Vec::new(),
            }]
        );
    }

    #[test]
    fn scan_empty_mod_ids_includes_recovery_record_mods_when_manifest_is_missing() {
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("target path");
        let game_files = Arc::new(FakeGameFiles::default());
        let backups = Arc::new(FakeBackups::default());
        let manifests = Arc::new(FakeManifests { manifest: None });
        let recovery_records = Arc::new(FakeRecoveryRecords::default());
        recovery_records.insert(recovery_record(
            InstallRecoveryRecordStatus::RollbackRequired,
            target,
            ModId::new("mod-b"),
            None,
            Some("backup-original".to_owned()),
        ));
        let service = InstallRecoveryScanService::new_with_recovery_records(
            game_files,
            backups,
            manifests,
            recovery_records,
        );

        let summaries = service
            .scan(InstallRecoveryScanRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: Vec::new(),
            })
            .expect("full profile scan should include recovery records");

        assert_eq!(
            summaries,
            vec![InstallRecoverySummary {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-b"),
                status: InstallRecoveryStatus::RollbackRequired,
                managed_file_count: 1,
                backup_count: 1,
                issue_count: 0,
                issues: Vec::new(),
            }]
        );
    }

    #[test]
    fn scan_empty_mod_ids_uses_listed_recovery_records_without_per_mod_record_probes() {
        let target_a = InstallTargetPath::parse("nativePC/models/player-a.mod3", ["nativePC"])
            .expect("target path a");
        let target_b = InstallTargetPath::parse("nativePC/models/player-b.mod3", ["nativePC"])
            .expect("target path b");
        let game_files = Arc::new(FakeGameFiles::default());
        {
            let mut files = game_files.files.lock().expect("files lock");
            files.insert(target_a.as_str().to_owned(), b"model a".to_vec());
            files.insert(target_b.as_str().to_owned(), b"model b".to_vec());
        }
        let backups = Arc::new(FakeBackups::default());
        let manifests = Arc::new(FakeManifests {
            manifest: Some(InstallManifest::completed(
                ProfileId::new("default"),
                vec![
                    InstallManifestEntry {
                        target_path: target_a,
                        mod_id: ModId::new("mod-a"),
                        package_file_id: PackageFileId::new("nativePC/models/player-a.mod3"),
                        layer: FileLayer::new("base", 0),
                        backup_ref: None,
                        installed_file: Some(summary(b"model a")),
                    },
                    InstallManifestEntry {
                        target_path: target_b,
                        mod_id: ModId::new("mod-b"),
                        package_file_id: PackageFileId::new("nativePC/models/player-b.mod3"),
                        layer: FileLayer::new("base", 0),
                        backup_ref: None,
                        installed_file: Some(summary(b"model b")),
                    },
                ],
            )),
        });
        let recovery_records = Arc::new(FakeRecoveryRecords::default());
        let service = InstallRecoveryScanService::new_with_recovery_records(
            game_files,
            backups,
            manifests,
            recovery_records.clone(),
        );

        let summaries = service
            .scan(InstallRecoveryScanRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: Vec::new(),
            })
            .expect("full profile scan should use listed recovery records");

        assert_eq!(summaries.len(), 2);
        assert_eq!(
            *recovery_records
                .listed_profiles
                .lock()
                .expect("listed profiles lock"),
            vec![ProfileId::new("default")]
        );
        assert!(
            recovery_records
                .loaded_records
                .lock()
                .expect("loaded records lock")
                .is_empty(),
            "full profile scan already listed recovery records and should not probe once per manifest mod"
        );
    }

    #[test]
    fn preview_rollback_action_is_available_when_recovery_record_targets_are_safe() {
        let new_target = InstallTargetPath::parse("nativePC/models/new-file.mod3", ["nativePC"])
            .expect("new target path");
        let overwritten_target =
            InstallTargetPath::parse("nativePC/models/overwritten.mod3", ["nativePC"])
                .expect("overwritten target path");
        let new_bytes = b"new modded model".to_vec();
        let overwritten_bytes = b"overwritten modded model".to_vec();
        let original_bytes = b"original model".to_vec();
        let game_files = Arc::new(FakeGameFiles::default());
        {
            let mut files = game_files.files.lock().expect("files lock");
            files.insert(new_target.as_str().to_owned(), new_bytes.clone());
            files.insert(
                overwritten_target.as_str().to_owned(),
                overwritten_bytes.clone(),
            );
        }
        let backups = Arc::new(FakeBackups::default());
        backups
            .backups
            .lock()
            .expect("backups lock")
            .insert("backup-original".to_owned(), original_bytes);
        let recovery_records = Arc::new(FakeRecoveryRecords::default());
        recovery_records.insert(InstallRecoveryRecord {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            status: InstallRecoveryRecordStatus::RollbackRequired,
            entries: vec![
                InstallRecoveryRecordEntry {
                    target_path: new_target,
                    package_file_id: PackageFileId::new("nativePC/models/new-file.mod3"),
                    backup_ref: None,
                    installed_file: Some(summary(&new_bytes)),
                },
                InstallRecoveryRecordEntry {
                    target_path: overwritten_target,
                    package_file_id: PackageFileId::new("nativePC/models/overwritten.mod3"),
                    backup_ref: Some("backup-original".to_owned()),
                    installed_file: Some(summary(&overwritten_bytes)),
                },
            ],
        });
        let service =
            InstallRecoveryActionPreviewService::new(game_files, backups, recovery_records);

        let preview = service
            .preview(InstallRecoveryActionPreviewRequest {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                action_kind: InstallRecoveryActionKind::RollbackInstall,
            })
            .expect("preview should succeed");

        assert_eq!(
            preview,
            InstallRecoveryActionPreview {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                action_kind: InstallRecoveryActionKind::RollbackInstall,
                availability: InstallRecoveryActionAvailability::Available,
                remove_file_count: 1,
                restore_file_count: 1,
                backup_count: 1,
                blocking_issue_count: 0,
                blocking_reasons: Vec::new(),
            }
        );
    }

    #[test]
    fn preview_rollback_action_blocks_unsafe_recovery_record_targets() {
        let changed_target = InstallTargetPath::parse("nativePC/models/changed.mod3", ["nativePC"])
            .expect("changed target path");
        let missing_target = InstallTargetPath::parse("nativePC/models/missing.mod3", ["nativePC"])
            .expect("missing target path");
        let unreadable_target =
            InstallTargetPath::parse("nativePC/models/unreadable.mod3", ["nativePC"])
                .expect("unreadable target path");
        let backup_missing_target =
            InstallTargetPath::parse("nativePC/models/backup-missing.mod3", ["nativePC"])
                .expect("backup missing target path");
        let backup_unreadable_target =
            InstallTargetPath::parse("nativePC/models/backup-unreadable.mod3", ["nativePC"])
                .expect("backup unreadable target path");
        let missing_summary_target =
            InstallTargetPath::parse("nativePC/models/missing-summary.mod3", ["nativePC"])
                .expect("missing summary target path");
        let expected_bytes = b"expected modded bytes".to_vec();
        let changed_bytes = b"externally changed bytes".to_vec();
        let game_files = Arc::new(FakeGameFiles::default());
        {
            let mut files = game_files.files.lock().expect("files lock");
            files.insert(changed_target.as_str().to_owned(), changed_bytes);
            files.insert(
                backup_missing_target.as_str().to_owned(),
                expected_bytes.clone(),
            );
            files.insert(
                backup_unreadable_target.as_str().to_owned(),
                expected_bytes.clone(),
            );
        }
        game_files
            .error_targets
            .lock()
            .expect("error targets lock")
            .insert(unreadable_target.as_str().to_owned());
        game_files
            .error_targets
            .lock()
            .expect("error targets lock")
            .insert(missing_summary_target.as_str().to_owned());
        let backups = Arc::new(FakeBackups::default());
        backups
            .error_refs
            .lock()
            .expect("error refs lock")
            .insert("backup-read-error".to_owned());
        let recovery_records = Arc::new(FakeRecoveryRecords::default());
        recovery_records.insert(InstallRecoveryRecord {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            status: InstallRecoveryRecordStatus::RollbackRequired,
            entries: vec![
                InstallRecoveryRecordEntry {
                    target_path: changed_target,
                    package_file_id: PackageFileId::new("nativePC/models/changed.mod3"),
                    backup_ref: None,
                    installed_file: Some(summary(&expected_bytes)),
                },
                InstallRecoveryRecordEntry {
                    target_path: missing_target,
                    package_file_id: PackageFileId::new("nativePC/models/missing.mod3"),
                    backup_ref: None,
                    installed_file: Some(summary(&expected_bytes)),
                },
                InstallRecoveryRecordEntry {
                    target_path: unreadable_target,
                    package_file_id: PackageFileId::new("nativePC/models/unreadable.mod3"),
                    backup_ref: None,
                    installed_file: Some(summary(&expected_bytes)),
                },
                InstallRecoveryRecordEntry {
                    target_path: backup_missing_target,
                    package_file_id: PackageFileId::new("nativePC/models/backup-missing.mod3"),
                    backup_ref: Some("backup-missing".to_owned()),
                    installed_file: Some(summary(&expected_bytes)),
                },
                InstallRecoveryRecordEntry {
                    target_path: backup_unreadable_target,
                    package_file_id: PackageFileId::new("nativePC/models/backup-unreadable.mod3"),
                    backup_ref: Some("backup-read-error".to_owned()),
                    installed_file: Some(summary(&expected_bytes)),
                },
                InstallRecoveryRecordEntry {
                    target_path: missing_summary_target,
                    package_file_id: PackageFileId::new("nativePC/models/missing-summary.mod3"),
                    backup_ref: Some("backup-missing-summary".to_owned()),
                    installed_file: None,
                },
            ],
        });
        let service =
            InstallRecoveryActionPreviewService::new(game_files, backups, recovery_records);

        let preview = service
            .preview(InstallRecoveryActionPreviewRequest {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                action_kind: InstallRecoveryActionKind::RollbackInstall,
            })
            .expect("preview should succeed");

        assert_eq!(
            preview.availability,
            InstallRecoveryActionAvailability::Blocked
        );
        assert_eq!(preview.remove_file_count, 3);
        assert_eq!(preview.restore_file_count, 3);
        assert_eq!(preview.backup_count, 3);
        assert_eq!(preview.blocking_issue_count, 7);
        assert_eq!(
            preview.blocking_reasons,
            vec![
                InstallRecoveryActionBlockReasonSummary {
                    reason: InstallRecoveryActionBlockReason::MissingInstalledFileSummary,
                    count: 1,
                },
                InstallRecoveryActionBlockReasonSummary {
                    reason: InstallRecoveryActionBlockReason::TargetMissing,
                    count: 1,
                },
                InstallRecoveryActionBlockReasonSummary {
                    reason: InstallRecoveryActionBlockReason::TargetChanged,
                    count: 1,
                },
                InstallRecoveryActionBlockReasonSummary {
                    reason: InstallRecoveryActionBlockReason::TargetReadFailed,
                    count: 1,
                },
                InstallRecoveryActionBlockReasonSummary {
                    reason: InstallRecoveryActionBlockReason::BackupMissing,
                    count: 2,
                },
                InstallRecoveryActionBlockReasonSummary {
                    reason: InstallRecoveryActionBlockReason::BackupReadFailed,
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn preview_rollback_action_blocks_when_rollback_state_is_missing() {
        let game_files = Arc::new(FakeGameFiles::default());
        let backups = Arc::new(FakeBackups::default());
        let recovery_records = Arc::new(FakeRecoveryRecords::default());
        let service =
            InstallRecoveryActionPreviewService::new(game_files, backups, recovery_records);

        let preview = service
            .preview(InstallRecoveryActionPreviewRequest {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                action_kind: InstallRecoveryActionKind::RollbackInstall,
            })
            .expect("preview should succeed");

        assert_eq!(
            preview.availability,
            InstallRecoveryActionAvailability::Blocked
        );
        assert_eq!(preview.remove_file_count, 0);
        assert_eq!(preview.restore_file_count, 0);
        assert_eq!(preview.backup_count, 0);
        assert_eq!(preview.blocking_issue_count, 1);
        assert_eq!(
            preview.blocking_reasons,
            vec![InstallRecoveryActionBlockReasonSummary {
                reason: InstallRecoveryActionBlockReason::RollbackStateMissing,
                count: 1,
            }]
        );
    }

    #[test]
    fn run_rollback_install_action_removes_new_files_restores_backups_and_marks_rolled_back() {
        let new_target = InstallTargetPath::parse("nativePC/models/new-file.mod3", ["nativePC"])
            .expect("new target path");
        let overwritten_target =
            InstallTargetPath::parse("nativePC/models/overwritten.mod3", ["nativePC"])
                .expect("overwritten target path");
        let new_bytes = b"new modded model".to_vec();
        let overwritten_bytes = b"overwritten modded model".to_vec();
        let original_bytes = b"original model".to_vec();
        let game_files = Arc::new(FakeGameFiles::default());
        {
            let mut files = game_files.files.lock().expect("files lock");
            files.insert(new_target.as_str().to_owned(), new_bytes.clone());
            files.insert(
                overwritten_target.as_str().to_owned(),
                overwritten_bytes.clone(),
            );
        }
        let backups = Arc::new(FakeBackups::default());
        backups
            .backups
            .lock()
            .expect("backups lock")
            .insert("backup-original".to_owned(), original_bytes.clone());
        let recovery_records = Arc::new(FakeRecoveryRecords::default());
        recovery_records.insert(InstallRecoveryRecord {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            status: InstallRecoveryRecordStatus::RollbackRequired,
            entries: vec![
                InstallRecoveryRecordEntry {
                    target_path: new_target.clone(),
                    package_file_id: PackageFileId::new("nativePC/models/new-file.mod3"),
                    backup_ref: None,
                    installed_file: Some(summary(&new_bytes)),
                },
                InstallRecoveryRecordEntry {
                    target_path: overwritten_target.clone(),
                    package_file_id: PackageFileId::new("nativePC/models/overwritten.mod3"),
                    backup_ref: Some("backup-original".to_owned()),
                    installed_file: Some(summary(&overwritten_bytes)),
                },
            ],
        });
        let service = InstallRecoveryActionService::new(
            game_files.clone(),
            backups,
            recovery_records.clone(),
        );

        let result = service
            .run(InstallRecoveryActionRequest {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                action_kind: InstallRecoveryActionKind::RollbackInstall,
            })
            .expect("rollback action should succeed");

        assert_eq!(result.remove_file_count, 1);
        assert_eq!(result.restore_file_count, 1);
        assert_eq!(result.backup_count, 1);
        let files = game_files.files.lock().expect("files lock");
        assert!(!files.contains_key(new_target.as_str()));
        assert_eq!(
            files.get(overwritten_target.as_str()),
            Some(&original_bytes)
        );
        let record = recovery_records
            .load_record(&ProfileId::new("default"), &ModId::new("mod-a"))
            .expect("record should load")
            .expect("record should remain");
        assert_eq!(record.status, InstallRecoveryRecordStatus::RolledBack);
    }

    #[test]
    fn run_rollback_install_action_rolls_back_committing_record() {
        let target = InstallTargetPath::parse("nativePC/models/new-file.mod3", ["nativePC"])
            .expect("target path");
        let installed_bytes = b"installed modded model".to_vec();
        let game_files = Arc::new(FakeGameFiles::default());
        game_files
            .files
            .lock()
            .expect("files lock")
            .insert(target.as_str().to_owned(), installed_bytes.clone());
        let backups = Arc::new(FakeBackups::default());
        let recovery_records = Arc::new(FakeRecoveryRecords::default());
        recovery_records.insert(recovery_record(
            InstallRecoveryRecordStatus::Committing,
            target.clone(),
            ModId::new("mod-a"),
            Some(summary(&installed_bytes)),
            None,
        ));
        let service = InstallRecoveryActionService::new(
            game_files.clone(),
            backups,
            recovery_records.clone(),
        );

        let result = service
            .run(InstallRecoveryActionRequest {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                action_kind: InstallRecoveryActionKind::RollbackInstall,
            })
            .expect("committing record should roll back");

        assert_eq!(result.remove_file_count, 1);
        assert_eq!(
            game_files
                .files
                .lock()
                .expect("files lock")
                .get(target.as_str()),
            None
        );
        let record = recovery_records
            .load_record(&ProfileId::new("default"), &ModId::new("mod-a"))
            .expect("record should load")
            .expect("record should remain");
        assert_eq!(record.status, InstallRecoveryRecordStatus::RolledBack);
    }

    #[test]
    fn run_rollback_install_action_revalidates_target_before_writing() {
        let changed_target = InstallTargetPath::parse("nativePC/models/changed.mod3", ["nativePC"])
            .expect("changed target path");
        let expected_bytes = b"expected modded bytes".to_vec();
        let changed_bytes = b"externally changed bytes".to_vec();
        let game_files = Arc::new(FakeGameFiles::default());
        game_files
            .files
            .lock()
            .expect("files lock")
            .insert(changed_target.as_str().to_owned(), expected_bytes.clone());
        game_files
            .mutate_after_read
            .lock()
            .expect("mutate after read lock")
            .insert(changed_target.as_str().to_owned(), changed_bytes.clone());
        let backups = Arc::new(FakeBackups::default());
        let recovery_records = Arc::new(FakeRecoveryRecords::default());
        recovery_records.insert(recovery_record(
            InstallRecoveryRecordStatus::RollbackRequired,
            changed_target.clone(),
            ModId::new("mod-a"),
            Some(summary(&expected_bytes)),
            None,
        ));
        let service = InstallRecoveryActionService::new(
            game_files.clone(),
            backups,
            recovery_records.clone(),
        );

        let error = service
            .run(InstallRecoveryActionRequest {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                action_kind: InstallRecoveryActionKind::RollbackInstall,
            })
            .expect_err("stale target should block rollback");

        assert_eq!(
            error,
            InstallRecoveryActionError::Blocked {
                reasons: vec![InstallRecoveryActionBlockReasonSummary {
                    reason: InstallRecoveryActionBlockReason::TargetChanged,
                    count: 1,
                }],
            }
        );
        assert_eq!(
            game_files
                .files
                .lock()
                .expect("files lock")
                .get(changed_target.as_str()),
            Some(&changed_bytes)
        );
        assert!(
            game_files.writes.lock().expect("writes lock").is_empty(),
            "blocked rollback must not write game files"
        );
        assert!(
            game_files
                .removals
                .lock()
                .expect("removals lock")
                .is_empty(),
            "blocked rollback must not remove game files"
        );
        let record = recovery_records
            .load_record(&ProfileId::new("default"), &ModId::new("mod-a"))
            .expect("record should load")
            .expect("record should remain");
        assert_eq!(record.status, InstallRecoveryRecordStatus::RollbackRequired);
    }

    #[test]
    fn run_rollback_install_action_rolls_back_removed_file_when_record_save_fails() {
        let target = InstallTargetPath::parse("nativePC/models/new-file.mod3", ["nativePC"])
            .expect("target path");
        let installed_bytes = b"installed modded model".to_vec();
        let game_files = Arc::new(FakeGameFiles::default());
        game_files
            .files
            .lock()
            .expect("files lock")
            .insert(target.as_str().to_owned(), installed_bytes.clone());
        let backups = Arc::new(FakeBackups::default());
        let recovery_records = Arc::new(FakeRecoveryRecords::default());
        recovery_records.insert(recovery_record(
            InstallRecoveryRecordStatus::RollbackRequired,
            target.clone(),
            ModId::new("mod-a"),
            Some(summary(&installed_bytes)),
            None,
        ));
        *recovery_records.fail_saves.lock().expect("fail saves lock") = true;
        let service = InstallRecoveryActionService::new(
            game_files.clone(),
            backups,
            recovery_records.clone(),
        );

        let error = service
            .run(InstallRecoveryActionRequest {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                action_kind: InstallRecoveryActionKind::RollbackInstall,
            })
            .expect_err("record save failure should fail the recovery action");

        assert_eq!(error, InstallRecoveryActionError::RecoveryRecordSaveFailed);
        assert_eq!(
            game_files
                .files
                .lock()
                .expect("files lock")
                .get(target.as_str()),
            Some(&installed_bytes),
            "failed recovery record save must roll back the file removal"
        );
        assert_eq!(
            *game_files.removals.lock().expect("removals lock"),
            vec![target.as_str().to_owned()]
        );
        assert_eq!(
            *game_files.writes.lock().expect("writes lock"),
            vec![target.as_str().to_owned()]
        );
        let record = recovery_records
            .load_record(&ProfileId::new("default"), &ModId::new("mod-a"))
            .expect("record should load")
            .expect("record should remain");
        assert_eq!(record.status, InstallRecoveryRecordStatus::RollbackRequired);
    }

    #[test]
    fn scan_marks_completed_when_target_summary_matches_and_backup_exists() {
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("target path");
        let modded_bytes = b"modded model".to_vec();
        let original_bytes = b"original model".to_vec();
        let game_files = Arc::new(FakeGameFiles::default());
        game_files
            .files
            .lock()
            .expect("files lock")
            .insert(target.as_str().to_owned(), modded_bytes.clone());
        let backups = Arc::new(FakeBackups::default());
        backups
            .backups
            .lock()
            .expect("backups lock")
            .insert("backup-original".to_owned(), original_bytes);
        let manifests = Arc::new(FakeManifests {
            manifest: Some(InstallManifest::completed(
                ProfileId::new("default"),
                vec![InstallManifestEntry {
                    target_path: target,
                    mod_id: ModId::new("mod-a"),
                    package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
                    layer: FileLayer::new("base", 0),
                    backup_ref: Some("backup-original".to_owned()),
                    installed_file: Some(summary(&modded_bytes)),
                }],
            )),
        });
        let service = InstallRecoveryScanService::new(game_files, backups, manifests);

        let summaries = service
            .scan(InstallRecoveryScanRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![ModId::new("mod-a")],
            })
            .expect("scan should succeed");

        assert_eq!(
            summaries,
            vec![InstallRecoverySummary {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                status: InstallRecoveryStatus::Completed,
                managed_file_count: 1,
                backup_count: 1,
                issue_count: 0,
                issues: Vec::new(),
            }]
        );
    }

    #[test]
    fn scan_empty_mod_ids_scans_all_unique_manifest_mods_in_stable_order() {
        let target_a =
            InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"]).expect("target");
        let target_b =
            InstallTargetPath::parse("nativePC/models/weapon.mod3", ["nativePC"]).expect("target");
        let target_a_extra =
            InstallTargetPath::parse("nativePC/models/player-extra.mod3", ["nativePC"])
                .expect("target");
        let bytes_a = b"player model".to_vec();
        let bytes_b = b"weapon model".to_vec();
        let game_files = Arc::new(FakeGameFiles::default());
        {
            let mut files = game_files.files.lock().expect("files lock");
            files.insert(target_a.as_str().to_owned(), bytes_a.clone());
            files.insert(target_a_extra.as_str().to_owned(), bytes_a.clone());
            files.insert(target_b.as_str().to_owned(), bytes_b.clone());
        }
        let backups = Arc::new(FakeBackups::default());
        let manifests = Arc::new(FakeManifests {
            manifest: Some(InstallManifest::completed(
                ProfileId::new("default"),
                vec![
                    InstallManifestEntry {
                        target_path: target_b,
                        mod_id: ModId::new("mod-b"),
                        package_file_id: PackageFileId::new("nativePC/models/weapon.mod3"),
                        layer: FileLayer::new("base", 0),
                        backup_ref: None,
                        installed_file: Some(summary(&bytes_b)),
                    },
                    InstallManifestEntry {
                        target_path: target_a,
                        mod_id: ModId::new("mod-a"),
                        package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
                        layer: FileLayer::new("base", 0),
                        backup_ref: None,
                        installed_file: Some(summary(&bytes_a)),
                    },
                    InstallManifestEntry {
                        target_path: target_a_extra,
                        mod_id: ModId::new("mod-a"),
                        package_file_id: PackageFileId::new("nativePC/models/player-extra.mod3"),
                        layer: FileLayer::new("base", 0),
                        backup_ref: None,
                        installed_file: Some(summary(&bytes_a)),
                    },
                ],
            )),
        });
        let service = InstallRecoveryScanService::new(game_files, backups, manifests);

        let summaries = service
            .scan(InstallRecoveryScanRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: Vec::new(),
            })
            .expect("scan should succeed");

        assert_eq!(
            summaries
                .iter()
                .map(|summary| summary.mod_id.as_str())
                .collect::<Vec<_>>(),
            vec!["mod-a", "mod-b"]
        );
        assert_eq!(summaries[0].managed_file_count, 2);
        assert_eq!(summaries[0].status, InstallRecoveryStatus::Completed);
        assert_eq!(summaries[1].managed_file_count, 1);
        assert_eq!(summaries[1].status, InstallRecoveryStatus::Completed);
    }

    #[test]
    fn scan_marks_unknown_when_target_state_cannot_be_read() {
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("target path");
        let modded_bytes = b"modded model".to_vec();
        let game_files = Arc::new(FakeGameFiles::default());
        game_files
            .error_targets
            .lock()
            .expect("error targets lock")
            .insert(target.as_str().to_owned());
        let backups = Arc::new(FakeBackups::default());
        let manifests = Arc::new(FakeManifests {
            manifest: Some(InstallManifest::completed(
                ProfileId::new("default"),
                vec![InstallManifestEntry {
                    target_path: target,
                    mod_id: ModId::new("mod-a"),
                    package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
                    layer: FileLayer::new("base", 0),
                    backup_ref: None,
                    installed_file: Some(summary(&modded_bytes)),
                }],
            )),
        });
        let service = InstallRecoveryScanService::new(game_files, backups, manifests);

        let summaries = service
            .scan(InstallRecoveryScanRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![ModId::new("mod-a")],
            })
            .expect("scan should return an unknown state rather than fail globally");

        assert_eq!(summaries[0].status, InstallRecoveryStatus::Unknown);
        assert_eq!(summaries[0].managed_file_count, 1);
        assert_eq!(summaries[0].backup_count, 0);
        assert_eq!(summaries[0].issue_count, 1);
        assert_eq!(
            summaries[0].issues,
            vec![InstallRecoveryIssueSummary {
                issue: InstallRecoveryIssue::TargetReadFailed,
                count: 1,
            }]
        );
    }

    #[test]
    fn scan_reports_repair_issue_when_backup_is_missing_without_exposing_backup_ref() {
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("target path");
        let modded_bytes = b"modded model".to_vec();
        let game_files = Arc::new(FakeGameFiles::default());
        game_files
            .files
            .lock()
            .expect("files lock")
            .insert(target.as_str().to_owned(), modded_bytes.clone());
        let backups = Arc::new(FakeBackups::default());
        let manifests = Arc::new(FakeManifests {
            manifest: Some(InstallManifest::completed(
                ProfileId::new("default"),
                vec![InstallManifestEntry {
                    target_path: target,
                    mod_id: ModId::new("mod-a"),
                    package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
                    layer: FileLayer::new("base", 0),
                    backup_ref: Some("backup-original".to_owned()),
                    installed_file: Some(summary(&modded_bytes)),
                }],
            )),
        });
        let service = InstallRecoveryScanService::new(game_files, backups, manifests);

        let summaries = service
            .scan(InstallRecoveryScanRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![ModId::new("mod-a")],
            })
            .expect("scan should succeed");

        assert_eq!(summaries[0].status, InstallRecoveryStatus::RepairRequired);
        assert_eq!(summaries[0].managed_file_count, 1);
        assert_eq!(summaries[0].backup_count, 1);
        assert_eq!(summaries[0].issue_count, 1);
        assert_eq!(
            summaries[0].issues,
            vec![InstallRecoveryIssueSummary {
                issue: InstallRecoveryIssue::BackupMissing,
                count: 1,
            }]
        );
    }

    #[test]
    fn scan_aggregates_recovery_issues_without_exposing_paths_or_backup_refs() {
        let missing_summary_target =
            InstallTargetPath::parse("nativePC/models/missing-summary.mod3", ["nativePC"])
                .expect("missing summary target path");
        let missing_target =
            InstallTargetPath::parse("nativePC/models/missing-target.mod3", ["nativePC"])
                .expect("missing target path");
        let changed_target =
            InstallTargetPath::parse("nativePC/models/changed-target.mod3", ["nativePC"])
                .expect("changed target path");
        let backup_error_target =
            InstallTargetPath::parse("nativePC/models/backup-error.mod3", ["nativePC"])
                .expect("backup error target path");
        let expected_bytes = b"expected bytes".to_vec();
        let changed_bytes = b"changed bytes".to_vec();
        let backup_error_ref = "backup-read-error";
        let game_files = Arc::new(FakeGameFiles::default());
        {
            let mut files = game_files.files.lock().expect("files lock");
            files.insert(changed_target.as_str().to_owned(), changed_bytes);
            files.insert(
                backup_error_target.as_str().to_owned(),
                expected_bytes.clone(),
            );
        }
        let backups = Arc::new(FakeBackups::default());
        backups
            .error_refs
            .lock()
            .expect("backup refs lock")
            .insert(backup_error_ref.to_owned());
        let manifests = Arc::new(FakeManifests {
            manifest: Some(InstallManifest::completed(
                ProfileId::new("default"),
                vec![
                    InstallManifestEntry {
                        target_path: missing_summary_target,
                        mod_id: ModId::new("mod-a"),
                        package_file_id: PackageFileId::new("nativePC/models/missing-summary.mod3"),
                        layer: FileLayer::new("base", 0),
                        backup_ref: None,
                        installed_file: None,
                    },
                    InstallManifestEntry {
                        target_path: missing_target,
                        mod_id: ModId::new("mod-a"),
                        package_file_id: PackageFileId::new("nativePC/models/missing-target.mod3"),
                        layer: FileLayer::new("base", 0),
                        backup_ref: None,
                        installed_file: Some(summary(&expected_bytes)),
                    },
                    InstallManifestEntry {
                        target_path: changed_target,
                        mod_id: ModId::new("mod-a"),
                        package_file_id: PackageFileId::new("nativePC/models/changed-target.mod3"),
                        layer: FileLayer::new("base", 0),
                        backup_ref: None,
                        installed_file: Some(summary(&expected_bytes)),
                    },
                    InstallManifestEntry {
                        target_path: backup_error_target,
                        mod_id: ModId::new("mod-a"),
                        package_file_id: PackageFileId::new("nativePC/models/backup-error.mod3"),
                        layer: FileLayer::new("base", 0),
                        backup_ref: Some(backup_error_ref.to_owned()),
                        installed_file: Some(summary(&expected_bytes)),
                    },
                ],
            )),
        });
        let service = InstallRecoveryScanService::new(game_files, backups, manifests);

        let summaries = service
            .scan(InstallRecoveryScanRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![ModId::new("mod-a")],
            })
            .expect("scan should succeed");

        assert_eq!(summaries[0].status, InstallRecoveryStatus::Unknown);
        assert_eq!(summaries[0].managed_file_count, 4);
        assert_eq!(summaries[0].backup_count, 1);
        assert_eq!(summaries[0].issue_count, 4);
        assert_eq!(
            summaries[0].issues,
            vec![
                InstallRecoveryIssueSummary {
                    issue: InstallRecoveryIssue::MissingInstalledFileSummary,
                    count: 1,
                },
                InstallRecoveryIssueSummary {
                    issue: InstallRecoveryIssue::TargetMissing,
                    count: 1,
                },
                InstallRecoveryIssueSummary {
                    issue: InstallRecoveryIssue::TargetChanged,
                    count: 1,
                },
                InstallRecoveryIssueSummary {
                    issue: InstallRecoveryIssue::BackupReadFailed,
                    count: 1,
                },
            ]
        );
    }

    fn summary(bytes: &[u8]) -> InstalledFileSummary {
        let digest = Sha256::digest(bytes);

        InstalledFileSummary {
            size_bytes: bytes.len() as u64,
            sha256: digest.iter().map(|byte| format!("{byte:02x}")).collect(),
        }
    }

    fn recovery_record(
        status: InstallRecoveryRecordStatus,
        target_path: InstallTargetPath,
        mod_id: ModId,
        installed_file: Option<InstalledFileSummary>,
        backup_ref: Option<String>,
    ) -> InstallRecoveryRecord {
        InstallRecoveryRecord {
            profile_id: ProfileId::new("default"),
            mod_id: mod_id.clone(),
            status,
            entries: vec![InstallRecoveryRecordEntry {
                target_path,
                package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
                backup_ref,
                installed_file,
            }],
        }
    }

    fn record_key(profile_id: &ProfileId, mod_id: &ModId) -> String {
        format!("{}:{}", profile_id.as_str(), mod_id.as_str())
    }
}
