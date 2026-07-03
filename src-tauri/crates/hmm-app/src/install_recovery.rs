use hmm_core::{
    InstallManifest, InstallManifestStatus, InstallManifestStatusConsumption,
    InstallRecoveryRecord, InstallRecoveryRecordStatus, InstalledFileSummary, ModId, ProfileId,
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
    #[error("failed to save install manifest after recovery rollback")]
    ManifestSaveFailed,
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
    ManifestSave,
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

        // profile 级 manifest status 的读侧消费规则：失败/进行中状态优先于
        // 逐 entry 文件校验，保证失败状态不会被误报为已完成。
        let gated_status = match manifest.status.consumption() {
            InstallManifestStatusConsumption::TrustEntries => None,
            InstallManifestStatusConsumption::InFlight => Some(InstallRecoveryStatus::Unknown),
            InstallManifestStatusConsumption::RollbackRequired => {
                Some(InstallRecoveryStatus::RollbackRequired)
            }
            InstallManifestStatusConsumption::RepairRequired => {
                Some(InstallRecoveryStatus::RepairRequired)
            }
        };
        if let Some(status) = gated_status {
            return InstallRecoverySummary {
                profile_id: profile_id.clone(),
                mod_id: mod_id.clone(),
                status,
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
    manifest_repository: Option<Arc<dyn InstallManifestRepository>>,
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
            manifest_repository: None,
        }
    }

    pub fn new_with_manifest(
        game_files: Arc<dyn InstallGameFileSystem>,
        backup_store: Arc<dyn InstallBackupStore>,
        recovery_record_repository: Arc<dyn InstallRecoveryRecordRepository>,
        manifest_repository: Arc<dyn InstallManifestRepository>,
    ) -> Self {
        Self {
            game_files,
            backup_store,
            recovery_record_repository,
            manifest_repository: Some(manifest_repository),
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
        let original_manifest = self.load_manifest_for_rollback(&request.profile_id)?;
        let rolled_back_manifest = original_manifest
            .as_ref()
            .map(|manifest| manifest_marked_rolled_back(manifest, &request.mod_id));

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

        if let Some(manifest) = &rolled_back_manifest {
            if self.save_manifest(manifest).is_err() {
                return Err(self.rollback_or_error(
                    &applied_actions,
                    InstallRecoveryActionPhase::ManifestSave,
                    InstallRecoveryActionError::ManifestSaveFailed,
                ));
            }
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
            return Err(self.rollback_or_error_with_manifest(
                &applied_actions,
                original_manifest.as_ref(),
                rolled_back_manifest.is_some(),
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

    fn load_manifest_for_rollback(
        &self,
        profile_id: &ProfileId,
    ) -> Result<Option<InstallManifest>, InstallRecoveryActionError> {
        let Some(repository) = &self.manifest_repository else {
            return Ok(None);
        };

        repository
            .load_manifest(profile_id)
            .map_err(|_| InstallRecoveryActionError::ActionUnavailable)
    }

    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        let Some(repository) = &self.manifest_repository else {
            return Ok(());
        };

        repository.save_manifest(manifest)
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

    fn rollback_or_error_with_manifest(
        &self,
        applied_actions: &[AppliedRecoveryAction],
        original_manifest: Option<&InstallManifest>,
        restore_manifest: bool,
        failed_phase: InstallRecoveryActionPhase,
        fallback: InstallRecoveryActionError,
    ) -> InstallRecoveryActionError {
        if self.rollback_applied_actions(applied_actions).is_err() {
            return InstallRecoveryActionError::RollbackFailed { failed_phase };
        }

        if restore_manifest {
            if let (Some(repository), Some(original_manifest)) =
                (&self.manifest_repository, original_manifest)
            {
                if repository.save_manifest(original_manifest).is_err() {
                    return InstallRecoveryActionError::RollbackFailed { failed_phase };
                }
            }
        }

        fallback
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

fn manifest_marked_rolled_back(manifest: &InstallManifest, mod_id: &ModId) -> InstallManifest {
    let mut updated_manifest = manifest.clone();
    updated_manifest.status = InstallManifestStatus::RolledBack;
    updated_manifest
        .entries
        .retain(|entry| entry.mod_id != *mod_id);
    updated_manifest
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
#[path = "install_recovery_tests.rs"]
mod tests;
