use hmm_core::{
    InstallManifest, InstallManifestStatus, InstallManifestStatusConsumption,
    InstallRecoveryRecord, InstallRecoveryRecordStatus, InstalledFileSummary, ModId, ProfileId,
    ReinstallRecoveryTransaction, ReinstallRecoveryTransactionStatus,
    ReinstallSnapshotCleanupOwner, ReinstallSnapshotState, ReinstallTargetClass,
};
use hmm_ports::{
    InstallBackupStore, InstallGameFileSystem, InstallManifestRepository,
    InstallRecoveryRecordRepository, ReinstallRecoveryTransactionRepository,
    ReinstallSnapshotStore,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use thiserror::Error;

use crate::reinstall_commit::{cleanup_reinstall_transaction, promote_manifest_snapshots};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRecoveryScanRequest {
    pub profile_id: ProfileId,
    pub mod_ids: Vec<ModId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallRecoveryStatus {
    NotInstalled,
    Completed,
    CommittedCleanupPending,
    CleanupPending,
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
    ReconcileReinstall,
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
    #[error("reinstall recovery state requires repair")]
    ReinstallRepairRequired,
    #[error("reinstall post-commit bookkeeping failed")]
    ReinstallPostCommitFailed,
    #[error("reinstall recovery cleanup remains pending")]
    ReinstallCleanupFailed,
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
    reinstall_recovery_repository: Option<Arc<dyn ReinstallRecoveryTransactionRepository>>,
    reinstall_snapshot_store: Option<Arc<dyn ReinstallSnapshotStore>>,
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
            reinstall_recovery_repository: None,
            reinstall_snapshot_store: None,
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
            reinstall_recovery_repository: None,
            reinstall_snapshot_store: None,
        }
    }

    pub fn with_reinstall_recovery_transactions(
        mut self,
        recovery_repository: Arc<dyn ReinstallRecoveryTransactionRepository>,
        snapshot_store: Arc<dyn ReinstallSnapshotStore>,
    ) -> Self {
        self.reinstall_recovery_repository = Some(recovery_repository);
        self.reinstall_snapshot_store = Some(snapshot_store);
        self
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
        let reinstall_transactions = if scan_all_mods {
            self.list_reinstall_transactions(&request.profile_id)?
        } else {
            BTreeMap::new()
        };

        let mod_ids = if scan_all_mods {
            recovery_scan_mod_ids(
                manifest.as_ref(),
                &recovery_records,
                &reinstall_transactions,
            )
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
            let reinstall_transaction =
                if let Some(transaction) = reinstall_transactions.get(mod_id.as_str()) {
                    Some(transaction.clone())
                } else if scan_all_mods {
                    None
                } else {
                    self.load_reinstall_transaction(&request.profile_id, &mod_id)?
                };
            summaries.push(self.scan_mod(
                &request.profile_id,
                &mod_id,
                manifest.as_ref(),
                recovery_record.as_ref(),
                reinstall_transaction.as_ref(),
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
        reinstall_transaction: Option<&ReinstallRecoveryTransaction>,
    ) -> InstallRecoverySummary {
        if let Some(transaction) = reinstall_transaction {
            return self.scan_reinstall_transaction(profile_id, mod_id, manifest, transaction);
        }
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

    fn scan_reinstall_transaction(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
        manifest: Option<&InstallManifest>,
        transaction: &ReinstallRecoveryTransaction,
    ) -> InstallRecoverySummary {
        let (managed_file_count, backup_count) = reinstall_manifest_counts(
            manifest.unwrap_or(&transaction.pre_reinstall_manifest),
            mod_id,
        );
        let valid_identity = transaction.profile_id == *profile_id
            && transaction.mod_id == *mod_id
            && transaction.validate().is_ok();
        let status = if valid_identity {
            self.derive_reinstall_status(manifest, transaction)
        } else {
            InstallRecoveryStatus::RepairRequired
        };

        InstallRecoverySummary {
            profile_id: profile_id.clone(),
            mod_id: mod_id.clone(),
            status,
            managed_file_count,
            backup_count,
            issue_count: 0,
            issues: Vec::new(),
        }
    }

    fn derive_reinstall_status(
        &self,
        manifest: Option<&InstallManifest>,
        transaction: &ReinstallRecoveryTransaction,
    ) -> InstallRecoveryStatus {
        use ReinstallRecoveryTransactionStatus::{
            Committing, Completed, Planned, RepairRequired, RollbackRequired, RolledBack,
        };

        if transaction.status == RepairRequired || !self.reinstall_snapshots_are_usable(transaction)
        {
            return InstallRecoveryStatus::RepairRequired;
        }

        let manifest_is_pre = manifest == Some(&transaction.pre_reinstall_manifest);
        let manifest_is_candidate = manifest
            .is_some_and(|manifest| reinstall_candidate_manifest_matches(manifest, transaction));
        let targets = self.observe_reinstall_targets(transaction);

        match transaction.status {
            Planned if manifest_is_pre && targets == ReinstallTargetObservation::AllPre => {
                InstallRecoveryStatus::CleanupPending
            }
            Committing
                if manifest_is_candidate && targets == ReinstallTargetObservation::AllCandidate =>
            {
                InstallRecoveryStatus::CommittedCleanupPending
            }
            Committing | RollbackRequired
                if manifest_is_pre && targets.is_known_rollback_state() =>
            {
                InstallRecoveryStatus::RollbackRequired
            }
            Completed
                if manifest_is_candidate && targets == ReinstallTargetObservation::AllCandidate =>
            {
                InstallRecoveryStatus::CleanupPending
            }
            RolledBack if manifest_is_pre && targets == ReinstallTargetObservation::AllPre => {
                InstallRecoveryStatus::CleanupPending
            }
            Planned | Committing | Completed | RollbackRequired | RolledBack | RepairRequired => {
                InstallRecoveryStatus::RepairRequired
            }
        }
    }

    fn observe_reinstall_targets(
        &self,
        transaction: &ReinstallRecoveryTransaction,
    ) -> ReinstallTargetObservation {
        let mut all_pre = true;
        let mut all_candidate = true;
        let mut all_known = true;

        for target in &transaction.targets {
            let current = match self.game_files.read_game_file(&target.target_path) {
                Ok(bytes) => bytes,
                Err(_) => return ReinstallTargetObservation::Unknown,
            };
            let current_summary = current.as_deref().map(installed_file_summary);
            let matches_pre = current_summary == target.pre_state;
            let matches_candidate = match target.class {
                ReinstallTargetClass::Retained
                | ReinstallTargetClass::Replaced
                | ReinstallTargetClass::Added => current_summary == target.candidate_state,
                ReinstallTargetClass::Stale => match &target.original_backup_ref {
                    Some(backup_ref) => match self.backup_store.read_backup(backup_ref) {
                        Ok(Some(bytes)) => current_summary == Some(installed_file_summary(&bytes)),
                        Ok(None)
                            if transaction.status
                                == ReinstallRecoveryTransactionStatus::Completed
                                && matches!(
                                    target.snapshot,
                                    ReinstallSnapshotState::Cleaned { .. }
                                ) =>
                        {
                            true
                        }
                        Ok(None) | Err(_) => return ReinstallTargetObservation::Unknown,
                    },
                    None if transaction.status == ReinstallRecoveryTransactionStatus::Completed
                        && matches!(target.snapshot, ReinstallSnapshotState::Cleaned { .. }) =>
                    {
                        true
                    }
                    None => current.is_none(),
                },
            };
            all_pre &= matches_pre;
            all_candidate &= matches_candidate;
            all_known &= matches_pre || matches_candidate;
        }

        if all_pre {
            ReinstallTargetObservation::AllPre
        } else if all_candidate {
            ReinstallTargetObservation::AllCandidate
        } else if all_known {
            ReinstallTargetObservation::MixedKnown
        } else {
            ReinstallTargetObservation::Unknown
        }
    }

    fn reinstall_snapshots_are_usable(&self, transaction: &ReinstallRecoveryTransaction) -> bool {
        let Some(snapshot_store) = &self.reinstall_snapshot_store else {
            return false;
        };

        transaction
            .targets
            .iter()
            .all(|target| match &target.snapshot {
                ReinstallSnapshotState::Stored { snapshot_ref, .. } => {
                    matches!(snapshot_store.read_snapshot(snapshot_ref), Ok(Some(_)))
                }
                ReinstallSnapshotState::CleanupPending { snapshot_ref, .. } => {
                    snapshot_store.read_snapshot(snapshot_ref).is_ok()
                }
                ReinstallSnapshotState::NotRequired
                | ReinstallSnapshotState::PreStateAbsent
                | ReinstallSnapshotState::Cleaned { .. } => true,
            })
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

    fn load_reinstall_transaction(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Result<Option<ReinstallRecoveryTransaction>, InstallRecoveryScanError> {
        let Some(repository) = &self.reinstall_recovery_repository else {
            return Ok(None);
        };
        repository
            .load_transaction(profile_id, mod_id)
            .map_err(|_| InstallRecoveryScanError::ManifestUnavailable)
    }

    fn list_reinstall_transactions(
        &self,
        profile_id: &ProfileId,
    ) -> Result<BTreeMap<String, ReinstallRecoveryTransaction>, InstallRecoveryScanError> {
        let Some(repository) = &self.reinstall_recovery_repository else {
            return Ok(BTreeMap::new());
        };
        Ok(repository
            .list_transactions(profile_id)
            .map_err(|_| InstallRecoveryScanError::ManifestUnavailable)?
            .into_iter()
            .map(|transaction| (transaction.mod_id.as_str().to_owned(), transaction))
            .collect())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReinstallTargetObservation {
    AllPre,
    AllCandidate,
    MixedKnown,
    Unknown,
}

impl ReinstallTargetObservation {
    fn is_known_rollback_state(self) -> bool {
        matches!(self, Self::AllPre | Self::AllCandidate | Self::MixedKnown)
    }
}

fn reinstall_manifest_counts(manifest: &InstallManifest, mod_id: &ModId) -> (usize, usize) {
    manifest
        .entries
        .iter()
        .filter(|entry| entry.mod_id == *mod_id)
        .fold((0, 0), |(managed, backups), entry| {
            (
                managed + 1,
                backups + usize::from(entry.backup_ref.is_some()),
            )
        })
}

fn reinstall_candidate_manifest_matches(
    manifest: &InstallManifest,
    transaction: &ReinstallRecoveryTransaction,
) -> bool {
    if manifest.validate().is_err()
        || manifest.profile_id != transaction.profile_id
        || manifest.status.consumption() != InstallManifestStatusConsumption::TrustEntries
        || manifest.plan_hash.as_deref() != Some(transaction.plan_hash.as_str())
    {
        return false;
    }

    let candidate_entries = manifest
        .entries
        .iter()
        .filter(|entry| entry.mod_id == transaction.mod_id)
        .collect::<Vec<_>>();
    if candidate_entries.is_empty()
        || candidate_entries
            .iter()
            .any(|entry| entry.revision_id.as_ref() != Some(&transaction.candidate_revision_id))
    {
        return false;
    }
    let candidate_targets = candidate_entries
        .iter()
        .map(|entry| entry.target_path.clone())
        .collect::<BTreeSet<_>>();
    let expected_targets = transaction
        .targets
        .iter()
        .filter(|target| target.candidate_state.is_some())
        .map(|target| target.target_path.clone())
        .collect::<BTreeSet<_>>();
    if candidate_targets != expected_targets {
        return false;
    }
    for target in transaction
        .targets
        .iter()
        .filter(|target| target.candidate_state.is_some())
    {
        let target_entries = candidate_entries
            .iter()
            .copied()
            .filter(|entry| entry.target_path == target.target_path)
            .collect::<Vec<_>>();
        let mut priorities = BTreeSet::new();
        if target_entries
            .iter()
            .any(|entry| entry.installed_file.is_none() || !priorities.insert(entry.layer.priority))
        {
            return false;
        }
        let Some(final_entry) = target_entries
            .iter()
            .max_by_key(|entry| entry.layer.priority)
        else {
            return false;
        };
        if final_entry.installed_file.as_ref() != target.candidate_state.as_ref() {
            return false;
        }

        let expected_backup_ref = match &target.snapshot {
            ReinstallSnapshotState::Stored {
                snapshot_ref,
                cleanup_owner:
                    ReinstallSnapshotCleanupOwner::PromoteOnCommit
                    | ReinstallSnapshotCleanupOwner::Manifest,
                ..
            } => Some(snapshot_ref.as_str()),
            _ => target.original_backup_ref.as_deref(),
        };
        if target_entries
            .iter()
            .any(|entry| entry.backup_ref.as_deref() != expected_backup_ref)
        {
            return false;
        }
    }

    let current_other_entries = manifest
        .entries
        .iter()
        .filter(|entry| entry.mod_id != transaction.mod_id)
        .collect::<Vec<_>>();
    let old_other_entries = transaction
        .pre_reinstall_manifest
        .entries
        .iter()
        .filter(|entry| entry.mod_id != transaction.mod_id)
        .collect::<Vec<_>>();
    current_other_entries == old_other_entries
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
    reinstall_recovery_repository: Option<Arc<dyn ReinstallRecoveryTransactionRepository>>,
    reinstall_snapshot_store: Option<Arc<dyn ReinstallSnapshotStore>>,
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

struct PreparedReinstallRollbackAction {
    target_path: hmm_core::InstallTargetPath,
    current_bytes: Option<Vec<u8>>,
    pre_bytes: Option<Vec<u8>>,
}

#[derive(Clone)]
struct AppliedReinstallRollbackAction {
    target_path: hmm_core::InstallTargetPath,
    previous_bytes: Option<Vec<u8>>,
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
            InstallRecoveryActionKind::ReconcileReinstall => Ok(blocked_recovery_action_preview(
                request,
                0,
                0,
                0,
                [(InstallRecoveryActionBlockReason::RollbackStateMissing, 1)],
            )),
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
            reinstall_recovery_repository: None,
            reinstall_snapshot_store: None,
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
            reinstall_recovery_repository: None,
            reinstall_snapshot_store: None,
        }
    }

    pub fn with_reinstall_reconciliation(
        mut self,
        recovery_repository: Arc<dyn ReinstallRecoveryTransactionRepository>,
        snapshot_store: Arc<dyn ReinstallSnapshotStore>,
    ) -> Self {
        self.reinstall_recovery_repository = Some(recovery_repository);
        self.reinstall_snapshot_store = Some(snapshot_store);
        self
    }

    pub fn run(
        &self,
        request: InstallRecoveryActionRequest,
    ) -> Result<InstallRecoveryActionResult, InstallRecoveryActionError> {
        match request.action_kind {
            InstallRecoveryActionKind::RollbackInstall => self.rollback_install(request),
            InstallRecoveryActionKind::ReconcileReinstall => self.reconcile_reinstall(request),
        }
    }

    fn reconcile_reinstall(
        &self,
        request: InstallRecoveryActionRequest,
    ) -> Result<InstallRecoveryActionResult, InstallRecoveryActionError> {
        let manifest_repository = self
            .manifest_repository
            .as_ref()
            .ok_or(InstallRecoveryActionError::ActionUnavailable)?;
        let recovery_repository = self
            .reinstall_recovery_repository
            .as_ref()
            .ok_or(InstallRecoveryActionError::ActionUnavailable)?;
        let snapshot_store = self
            .reinstall_snapshot_store
            .as_ref()
            .ok_or(InstallRecoveryActionError::ActionUnavailable)?;
        let mut transaction = recovery_repository
            .load_transaction(&request.profile_id, &request.mod_id)
            .map_err(|_| InstallRecoveryActionError::ActionUnavailable)?
            .ok_or(InstallRecoveryActionError::ActionUnavailable)?;
        if transaction.profile_id != request.profile_id
            || transaction.mod_id != request.mod_id
            || transaction.validate().is_err()
        {
            return Err(InstallRecoveryActionError::ReinstallRepairRequired);
        }
        let manifest = manifest_repository
            .load_manifest(&request.profile_id)
            .map_err(|_| InstallRecoveryActionError::ActionUnavailable)?;
        let scanner = InstallRecoveryScanService::new(
            Arc::clone(&self.game_files),
            Arc::clone(&self.backup_store),
            Arc::clone(manifest_repository),
        )
        .with_reinstall_recovery_transactions(
            Arc::clone(recovery_repository),
            Arc::clone(snapshot_store),
        );
        let derived = scanner.derive_reinstall_status(manifest.as_ref(), &transaction);

        let remove_stale_original_backups = match derived {
            InstallRecoveryStatus::CommittedCleanupPending => {
                promote_manifest_snapshots(&mut transaction);
                transaction
                    .transition_to(ReinstallRecoveryTransactionStatus::Completed)
                    .map_err(|_| InstallRecoveryActionError::ReinstallRepairRequired)?;
                recovery_repository
                    .save_transaction(&transaction)
                    .map_err(|_| InstallRecoveryActionError::ReinstallPostCommitFailed)?;
                true
            }
            InstallRecoveryStatus::CleanupPending => {
                transaction.status == ReinstallRecoveryTransactionStatus::Completed
            }
            InstallRecoveryStatus::RollbackRequired => {
                return self.rollback_reinstall(
                    request,
                    transaction,
                    recovery_repository.as_ref(),
                    snapshot_store.as_ref(),
                );
            }
            InstallRecoveryStatus::RepairRequired
            | InstallRecoveryStatus::Unknown
            | InstallRecoveryStatus::Completed
            | InstallRecoveryStatus::NotInstalled => {
                if matches!(
                    transaction.status,
                    ReinstallRecoveryTransactionStatus::Committing
                        | ReinstallRecoveryTransactionStatus::RollbackRequired
                ) && transaction
                    .transition_to(ReinstallRecoveryTransactionStatus::RepairRequired)
                    .is_ok()
                {
                    let _ = recovery_repository.save_transaction(&transaction);
                }
                return Err(InstallRecoveryActionError::ReinstallRepairRequired);
            }
        };
        let cleanup_count =
            reinstall_cleanup_resource_count(&transaction, remove_stale_original_backups);
        cleanup_reinstall_transaction(
            self.backup_store.as_ref(),
            recovery_repository.as_ref(),
            snapshot_store.as_ref(),
            &transaction,
            remove_stale_original_backups,
        )
        .map_err(|_| InstallRecoveryActionError::ReinstallCleanupFailed)?;

        Ok(InstallRecoveryActionResult {
            profile_id: request.profile_id,
            mod_id: request.mod_id,
            action_kind: request.action_kind,
            remove_file_count: 0,
            restore_file_count: 0,
            backup_count: cleanup_count,
        })
    }

    fn rollback_reinstall(
        &self,
        request: InstallRecoveryActionRequest,
        mut transaction: ReinstallRecoveryTransaction,
        recovery_repository: &dyn ReinstallRecoveryTransactionRepository,
        snapshot_store: &dyn ReinstallSnapshotStore,
    ) -> Result<InstallRecoveryActionResult, InstallRecoveryActionError> {
        if transaction.status == ReinstallRecoveryTransactionStatus::Committing {
            if transaction
                .transition_to(ReinstallRecoveryTransactionStatus::RollbackRequired)
                .is_err()
            {
                self.mark_reinstall_repair_required(recovery_repository, &mut transaction);
                return Err(InstallRecoveryActionError::ReinstallRepairRequired);
            }
            recovery_repository
                .save_transaction(&transaction)
                .map_err(|_| InstallRecoveryActionError::RecoveryRecordSaveFailed)?;
        }
        if transaction.status != ReinstallRecoveryTransactionStatus::RollbackRequired {
            self.mark_reinstall_repair_required(recovery_repository, &mut transaction);
            return Err(InstallRecoveryActionError::ReinstallRepairRequired);
        }

        let actions = match self.prepare_reinstall_rollback(&transaction, snapshot_store) {
            Ok(actions) => actions,
            Err(()) => {
                self.mark_reinstall_repair_required(recovery_repository, &mut transaction);
                return Err(InstallRecoveryActionError::ReinstallRepairRequired);
            }
        };
        let remove_file_count = actions
            .iter()
            .filter(|action| action.pre_bytes.is_none())
            .count();
        let restore_file_count = actions.len() - remove_file_count;
        let backup_count = reinstall_cleanup_resource_count(&transaction, false);
        let mut applied = Vec::with_capacity(actions.len());

        for action in actions.iter().rev() {
            let current = match self.game_files.read_game_file(&action.target_path) {
                Ok(current) => current,
                Err(_) => {
                    return Err(self.reinstall_rollback_failure(
                        recovery_repository,
                        &mut transaction,
                        &applied,
                        InstallRecoveryActionPhase::Revalidate,
                        InstallRecoveryActionError::ReinstallRepairRequired,
                        true,
                    ));
                }
            };
            if current != action.current_bytes {
                return Err(self.reinstall_rollback_failure(
                    recovery_repository,
                    &mut transaction,
                    &applied,
                    InstallRecoveryActionPhase::Revalidate,
                    InstallRecoveryActionError::ReinstallRepairRequired,
                    true,
                ));
            }

            applied.push(AppliedReinstallRollbackAction {
                target_path: action.target_path.clone(),
                previous_bytes: action.current_bytes.clone(),
            });
            let (phase, fallback, mutation) = if let Some(pre_bytes) = &action.pre_bytes {
                (
                    InstallRecoveryActionPhase::Restore,
                    InstallRecoveryActionError::RestoreFailed,
                    self.game_files
                        .write_game_file(&action.target_path, pre_bytes),
                )
            } else {
                (
                    InstallRecoveryActionPhase::Remove,
                    InstallRecoveryActionError::RemoveFailed,
                    self.game_files.remove_game_file(&action.target_path),
                )
            };
            if mutation.is_err() {
                return Err(self.reinstall_rollback_failure(
                    recovery_repository,
                    &mut transaction,
                    &applied,
                    phase,
                    fallback,
                    false,
                ));
            }
        }

        if transaction
            .transition_to(ReinstallRecoveryTransactionStatus::RolledBack)
            .is_err()
        {
            self.mark_reinstall_repair_required(recovery_repository, &mut transaction);
            return Err(InstallRecoveryActionError::ReinstallRepairRequired);
        }
        recovery_repository
            .save_transaction(&transaction)
            .map_err(|_| InstallRecoveryActionError::RecoveryRecordSaveFailed)?;
        cleanup_reinstall_transaction(
            self.backup_store.as_ref(),
            recovery_repository,
            snapshot_store,
            &transaction,
            false,
        )
        .map_err(|_| InstallRecoveryActionError::ReinstallCleanupFailed)?;

        Ok(InstallRecoveryActionResult {
            profile_id: request.profile_id,
            mod_id: request.mod_id,
            action_kind: request.action_kind,
            remove_file_count,
            restore_file_count,
            backup_count,
        })
    }

    fn prepare_reinstall_rollback(
        &self,
        transaction: &ReinstallRecoveryTransaction,
        snapshot_store: &dyn ReinstallSnapshotStore,
    ) -> Result<Vec<PreparedReinstallRollbackAction>, ()> {
        let mut actions = Vec::new();
        for target in &transaction.targets {
            let current_bytes = self
                .game_files
                .read_game_file(&target.target_path)
                .map_err(|_| ())?;
            let current_summary = current_bytes.as_deref().map(installed_file_summary);
            if current_summary == target.pre_state {
                continue;
            }
            if !self.reinstall_candidate_matches(target, current_bytes.as_deref())? {
                return Err(());
            }
            let pre_bytes = match (&target.pre_state, &target.snapshot) {
                (None, ReinstallSnapshotState::PreStateAbsent) => None,
                (Some(expected), ReinstallSnapshotState::Stored { snapshot_ref, .. }) => {
                    let bytes = snapshot_store
                        .read_snapshot(snapshot_ref)
                        .map_err(|_| ())?
                        .ok_or(())?;
                    if installed_file_summary(&bytes) != *expected {
                        return Err(());
                    }
                    Some(bytes)
                }
                _ => return Err(()),
            };
            actions.push(PreparedReinstallRollbackAction {
                target_path: target.target_path.clone(),
                current_bytes,
                pre_bytes,
            });
        }
        Ok(actions)
    }

    fn reinstall_candidate_matches(
        &self,
        target: &hmm_core::ReinstallRecoveryTarget,
        current_bytes: Option<&[u8]>,
    ) -> Result<bool, ()> {
        let current_summary = current_bytes.map(installed_file_summary);
        match target.class {
            ReinstallTargetClass::Retained
            | ReinstallTargetClass::Replaced
            | ReinstallTargetClass::Added => Ok(current_summary == target.candidate_state),
            ReinstallTargetClass::Stale => match &target.original_backup_ref {
                Some(backup_ref) => {
                    let bytes = self
                        .backup_store
                        .read_backup(backup_ref)
                        .map_err(|_| ())?
                        .ok_or(())?;
                    Ok(current_summary == Some(installed_file_summary(&bytes)))
                }
                None => Ok(current_bytes.is_none()),
            },
        }
    }

    fn reinstall_rollback_failure(
        &self,
        recovery_repository: &dyn ReinstallRecoveryTransactionRepository,
        transaction: &mut ReinstallRecoveryTransaction,
        applied: &[AppliedReinstallRollbackAction],
        phase: InstallRecoveryActionPhase,
        fallback: InstallRecoveryActionError,
        repair_required: bool,
    ) -> InstallRecoveryActionError {
        if self.rollback_reinstall_actions(applied).is_err() {
            self.mark_reinstall_repair_required(recovery_repository, transaction);
            return InstallRecoveryActionError::RollbackFailed {
                failed_phase: phase,
            };
        }
        if repair_required {
            self.mark_reinstall_repair_required(recovery_repository, transaction);
        }
        fallback
    }

    fn rollback_reinstall_actions(
        &self,
        applied: &[AppliedReinstallRollbackAction],
    ) -> Result<(), ()> {
        for action in applied.iter().rev() {
            match &action.previous_bytes {
                Some(bytes) => self
                    .game_files
                    .write_game_file(&action.target_path, bytes)
                    .map_err(|_| ())?,
                None => self
                    .game_files
                    .remove_game_file(&action.target_path)
                    .map_err(|_| ())?,
            }
        }
        Ok(())
    }

    fn mark_reinstall_repair_required(
        &self,
        recovery_repository: &dyn ReinstallRecoveryTransactionRepository,
        transaction: &mut ReinstallRecoveryTransaction,
    ) {
        if transaction
            .transition_to(ReinstallRecoveryTransactionStatus::RepairRequired)
            .is_ok()
        {
            let _ = recovery_repository.save_transaction(transaction);
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

fn reinstall_cleanup_resource_count(
    transaction: &ReinstallRecoveryTransaction,
    remove_stale_original_backups: bool,
) -> usize {
    let snapshot_count = transaction
        .targets
        .iter()
        .filter(|target| {
            matches!(
                target.snapshot,
                ReinstallSnapshotState::Stored { .. }
                    | ReinstallSnapshotState::CleanupPending { .. }
            )
        })
        .count();
    let stale_backup_count = if remove_stale_original_backups {
        transaction
            .targets
            .iter()
            .filter(|target| {
                target.class == ReinstallTargetClass::Stale && target.original_backup_ref.is_some()
            })
            .count()
    } else {
        0
    };
    snapshot_count + stale_backup_count
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
    reinstall_transactions: &BTreeMap<String, ReinstallRecoveryTransaction>,
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

    for transaction in reinstall_transactions.values() {
        mod_ids
            .entry(transaction.mod_id.as_str().to_owned())
            .or_insert_with(|| transaction.mod_id.clone());
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
