#![allow(dead_code)] // The Task 6 runner will call this crate-internal prepared commit seam.

use crate::reinstall::{
    summarize, PreparedReinstall, PreparedReinstallTarget, ReinstallCandidateSourceReader,
};
use hmm_core::{
    replace_entries_and_bindings_for_mod, InstallManifest, InstallManifestEntry,
    InstallManifestStatus, InstallTargetPath, ReinstallRecoveryTarget, ReinstallRecoveryTransaction,
    ReinstallRecoveryTransactionStatus, ReinstallSnapshotCleanupOwner, ReinstallSnapshotPurpose,
    ReinstallSnapshotState, ReinstallTargetClass,
};
use hmm_ports::{
    InstallBackupStore, InstallGameFileSystem, InstallManifestRepository,
    ModImportResultRepository, ReinstallRecoveryTransactionRepository, ReinstallSnapshotStore,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReinstallCommitPhase {
    Revalidation,
    Snapshot,
    Recovery,
    Mutation,
    Manifest,
    Rollback,
    PostCommit,
    Cleanup,
}

impl ReinstallCommitPhase {
    pub fn code(self) -> &'static str {
        match self {
            Self::Revalidation => "revalidation",
            Self::Snapshot => "snapshot",
            Self::Recovery => "recovery",
            Self::Mutation => "mutation",
            Self::Manifest => "manifest",
            Self::Rollback => "rollback",
            Self::PostCommit => "post_commit",
            Self::Cleanup => "cleanup",
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReinstallCommitError {
    #[error("reinstall preview is stale")]
    PreviewStale,
    #[error("reinstall failed during {phase:?}")]
    Failed { phase: ReinstallCommitPhase },
    #[error(
        "reinstall failed during {failed_phase:?}; rollback succeeded; cleanup pending: {cleanup_pending}"
    )]
    RolledBack {
        failed_phase: ReinstallCommitPhase,
        cleanup_pending: bool,
    },
    #[error("reinstall failed during {failed_phase:?}; rollback is required")]
    RollbackRequired { failed_phase: ReinstallCommitPhase },
    #[error("reinstall failed during {failed_phase:?}; repair is required")]
    RepairRequired { failed_phase: ReinstallCommitPhase },
    #[error("reinstall manifest committed but post-commit bookkeeping failed")]
    PostCommit,
    #[error("reinstall committed but cleanup is pending")]
    CleanupPending,
}

impl ReinstallCommitError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::PreviewStale => "install_reinstall_failed:preview_stale",
            Self::Failed { phase } => phase_error_code(*phase),
            Self::RolledBack { failed_phase, .. } => phase_error_code(*failed_phase),
            Self::RollbackRequired { .. } => "install_reinstall_failed:rollback",
            Self::RepairRequired { .. } => "install_reinstall_failed:repair",
            Self::PostCommit => "install_reinstall_failed:post_commit",
            Self::CleanupPending => "install_reinstall_failed:cleanup",
        }
    }
}

fn phase_error_code(phase: ReinstallCommitPhase) -> &'static str {
    match phase {
        ReinstallCommitPhase::Revalidation => "install_reinstall_failed:revalidation",
        ReinstallCommitPhase::Snapshot => "install_reinstall_failed:snapshot",
        ReinstallCommitPhase::Recovery => "install_reinstall_failed:recovery",
        ReinstallCommitPhase::Mutation => "install_reinstall_failed:commit",
        ReinstallCommitPhase::Manifest => "install_reinstall_failed:manifest",
        ReinstallCommitPhase::Rollback => "install_reinstall_failed:rollback",
        ReinstallCommitPhase::PostCommit => "install_reinstall_failed:post_commit",
        ReinstallCommitPhase::Cleanup => "install_reinstall_failed:cleanup",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinstallCommitResult {
    pub manifest: InstallManifest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CleanupProgressError {
    Resource,
    Recovery,
}

#[derive(Clone)]
pub struct ReinstallCommitService {
    catalog: Arc<dyn ModImportResultRepository>,
    source: Arc<dyn ReinstallCandidateSourceReader>,
    game: Arc<dyn InstallGameFileSystem>,
    backups: Arc<dyn InstallBackupStore>,
    manifests: Arc<dyn InstallManifestRepository>,
    recovery: Arc<dyn ReinstallRecoveryTransactionRepository>,
    snapshots: Arc<dyn ReinstallSnapshotStore>,
}

impl ReinstallCommitService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        catalog: Arc<dyn ModImportResultRepository>,
        source: Arc<dyn ReinstallCandidateSourceReader>,
        game: Arc<dyn InstallGameFileSystem>,
        backups: Arc<dyn InstallBackupStore>,
        manifests: Arc<dyn InstallManifestRepository>,
        recovery: Arc<dyn ReinstallRecoveryTransactionRepository>,
        snapshots: Arc<dyn ReinstallSnapshotStore>,
    ) -> Self {
        Self {
            catalog,
            source,
            game,
            backups,
            manifests,
            recovery,
            snapshots,
        }
    }

    pub(crate) fn commit(
        &self,
        prepared: PreparedReinstall,
        expected_plan_token: &str,
    ) -> Result<ReinstallCommitResult, ReinstallCommitError> {
        self.revalidate(&prepared, expected_plan_token)?;

        let mut transaction = self.create_transaction(&prepared)?;
        if self.recovery.save_transaction(&transaction).is_err() {
            self.abort_before_mutation(&transaction);
            return Err(ReinstallCommitError::Failed {
                phase: ReinstallCommitPhase::Recovery,
            });
        }
        transaction
            .transition_to(ReinstallRecoveryTransactionStatus::Committing)
            .expect("planned transaction can enter committing");
        if self.recovery.save_transaction(&transaction).is_err() {
            self.abort_before_mutation(&transaction);
            return Err(ReinstallCommitError::Failed {
                phase: ReinstallCommitPhase::Recovery,
            });
        }

        let mut applied_targets = BTreeSet::new();
        for target in &prepared.targets {
            if target.class == ReinstallTargetClass::Retained {
                continue;
            }
            applied_targets.insert(target.target_path.clone());
            if self.apply_target(target).is_err() {
                return Err(self.rollback(
                    transaction,
                    &applied_targets,
                    ReinstallCommitPhase::Mutation,
                ));
            }
        }

        let candidate_manifest = match build_candidate_manifest(&prepared, &transaction) {
            Ok(manifest) => manifest,
            Err(()) => {
                return Err(self.rollback(
                    transaction,
                    &applied_targets,
                    ReinstallCommitPhase::Manifest,
                ));
            }
        };

        if self.manifests.save_manifest(&candidate_manifest).is_err() {
            return Err(self.handle_manifest_error(
                transaction,
                &applied_targets,
                &candidate_manifest,
            ));
        }

        promote_manifest_snapshots(&mut transaction);
        transaction
            .transition_to(ReinstallRecoveryTransactionStatus::Completed)
            .expect("committing transaction can complete");
        if self.recovery.save_transaction(&transaction).is_err() {
            return Err(ReinstallCommitError::PostCommit);
        }
        if self.cleanup_committed(&transaction).is_err() {
            return Err(ReinstallCommitError::CleanupPending);
        }
        Ok(ReinstallCommitResult {
            manifest: candidate_manifest,
        })
    }

    fn revalidate(
        &self,
        prepared: &PreparedReinstall,
        expected_plan_token: &str,
    ) -> Result<(), ReinstallCommitError> {
        if expected_plan_token != prepared.plan_token {
            return Err(ReinstallCommitError::PreviewStale);
        }
        let manifest = self
            .manifests
            .load_manifest(&prepared.request.profile_id)
            .map_err(|_| ReinstallCommitError::Failed {
                phase: ReinstallCommitPhase::Revalidation,
            })?;
        if manifest.as_ref() != Some(&prepared.old_manifest) {
            return Err(ReinstallCommitError::PreviewStale);
        }
        let active = self
            .recovery
            .list_transactions(&prepared.request.profile_id)
            .map_err(|_| ReinstallCommitError::Failed {
                phase: ReinstallCommitPhase::Revalidation,
            })?;
        if !active.is_empty() {
            return Err(ReinstallCommitError::PreviewStale);
        }
        let candidate = self
            .catalog
            .get_revision(&prepared.candidate.revision_id)
            .map_err(|_| ReinstallCommitError::Failed {
                phase: ReinstallCommitPhase::Revalidation,
            })?;
        if candidate.as_ref() != Some(&prepared.candidate) {
            return Err(ReinstallCommitError::PreviewStale);
        }
        let legacy_provenance = self
            .catalog
            .get_mod(&prepared.request.mod_id)
            .map_err(|_| ReinstallCommitError::Failed {
                phase: ReinstallCommitPhase::Revalidation,
            })?
            .filter(|logical_mod| logical_mod.mod_id == prepared.request.mod_id)
            .map(|logical_mod| vec![logical_mod.origin_revision_id])
            .unwrap_or_default();
        if legacy_provenance != prepared.legacy_provenance {
            return Err(ReinstallCommitError::PreviewStale);
        }
        for source in &prepared.source_files {
            let bytes = self
                .source
                .read_candidate_source_file(&prepared.candidate, &source.provider.package_file_id)
                .map_err(|_| ReinstallCommitError::Failed {
                    phase: ReinstallCommitPhase::Revalidation,
                })?;
            if summarize(&bytes) != source.summary {
                return Err(ReinstallCommitError::PreviewStale);
            }
        }
        for target in &prepared.targets {
            let current = self.game.read_game_file(&target.target_path).map_err(|_| {
                ReinstallCommitError::Failed {
                    phase: ReinstallCommitPhase::Revalidation,
                }
            })?;
            let current_summary = current.as_deref().map(summarize);
            let prepared_summary = target.pre_file.as_ref().map(|file| file.summary.clone());
            if current_summary != prepared_summary {
                return Err(ReinstallCommitError::PreviewStale);
            }
        }
        for (backup_ref, prepared_file) in &prepared.backup_files {
            let current =
                self.backups
                    .read_backup(backup_ref)
                    .map_err(|_| ReinstallCommitError::Failed {
                        phase: ReinstallCommitPhase::Revalidation,
                    })?;
            if current.as_deref().map(summarize) != Some(prepared_file.summary.clone()) {
                return Err(ReinstallCommitError::PreviewStale);
            }
        }
        Ok(())
    }

    fn create_transaction(
        &self,
        prepared: &PreparedReinstall,
    ) -> Result<ReinstallRecoveryTransaction, ReinstallCommitError> {
        let mut recovery_targets = Vec::with_capacity(prepared.targets.len());
        let mut created_snapshots = Vec::new();
        for target in &prepared.targets {
            let snapshot = match target.class {
                ReinstallTargetClass::Retained => ReinstallSnapshotState::NotRequired,
                ReinstallTargetClass::Added if target.pre_file.is_none() => {
                    ReinstallSnapshotState::PreStateAbsent
                }
                ReinstallTargetClass::Added => match self.store_snapshot(target) {
                    Ok(snapshot_ref) => {
                        created_snapshots.push(snapshot_ref.clone());
                        ReinstallSnapshotState::Stored {
                            snapshot_ref,
                            purpose: ReinstallSnapshotPurpose::OriginalBackupCandidate,
                            cleanup_owner: ReinstallSnapshotCleanupOwner::PromoteOnCommit,
                        }
                    }
                    Err(()) => {
                        self.remove_snapshot_refs(&created_snapshots);
                        return Err(ReinstallCommitError::Failed {
                            phase: ReinstallCommitPhase::Snapshot,
                        });
                    }
                },
                ReinstallTargetClass::Replaced | ReinstallTargetClass::Stale => {
                    match self.store_snapshot(target) {
                        Ok(snapshot_ref) => {
                            created_snapshots.push(snapshot_ref.clone());
                            ReinstallSnapshotState::Stored {
                                snapshot_ref,
                                purpose: ReinstallSnapshotPurpose::TransactionRollback,
                                cleanup_owner: ReinstallSnapshotCleanupOwner::Transaction,
                            }
                        }
                        Err(()) => {
                            self.remove_snapshot_refs(&created_snapshots);
                            return Err(ReinstallCommitError::Failed {
                                phase: ReinstallCommitPhase::Snapshot,
                            });
                        }
                    }
                }
            };
            recovery_targets.push(ReinstallRecoveryTarget {
                target_path: target.target_path.clone(),
                class: target.class,
                pre_state: target.pre_file.as_ref().map(|file| file.summary.clone()),
                candidate_state: candidate_file(target).map(|file| file.summary.clone()),
                snapshot,
                original_backup_ref: target.original_backup_ref.clone(),
            });
        }

        let transaction = ReinstallRecoveryTransaction {
            profile_id: prepared.request.profile_id.clone(),
            mod_id: prepared.request.mod_id.clone(),
            old_revision_id: prepared.installed_revision_id.clone(),
            candidate_revision_id: prepared.candidate.revision_id.clone(),
            plan_token: prepared.plan_token.clone(),
            plan_hash: prepared.plan_hash.clone(),
            status: ReinstallRecoveryTransactionStatus::Planned,
            pre_reinstall_manifest: prepared.old_manifest.clone(),
            candidate_replacement_bindings: prepared.candidate_replacement_bindings.clone(),
            targets: recovery_targets,
        };
        if transaction.validate().is_err() {
            self.remove_snapshot_refs(&created_snapshots);
            return Err(ReinstallCommitError::Failed {
                phase: ReinstallCommitPhase::Recovery,
            });
        }
        Ok(transaction)
    }

    fn store_snapshot(&self, target: &PreparedReinstallTarget) -> Result<String, ()> {
        let bytes = target.pre_file.as_ref().ok_or(())?;
        self.snapshots
            .store_snapshot(&target.target_path, bytes.bytes.as_ref())
            .map_err(|_| ())
    }

    fn apply_target(&self, target: &PreparedReinstallTarget) -> Result<(), ()> {
        match target.class {
            ReinstallTargetClass::Retained => Ok(()),
            ReinstallTargetClass::Replaced | ReinstallTargetClass::Added => {
                let candidate = candidate_file(target).ok_or(())?;
                self.game
                    .write_game_file(&target.target_path, candidate.bytes.as_ref())
                    .map_err(|_| ())
            }
            ReinstallTargetClass::Stale => match &target.original_backup_file {
                Some(original) => self
                    .game
                    .write_game_file(&target.target_path, original.bytes.as_ref())
                    .map_err(|_| ()),
                None => self
                    .game
                    .remove_game_file(&target.target_path)
                    .map_err(|_| ()),
            },
        }
    }

    fn handle_manifest_error(
        &self,
        mut transaction: ReinstallRecoveryTransaction,
        applied_targets: &BTreeSet<InstallTargetPath>,
        candidate_manifest: &InstallManifest,
    ) -> ReinstallCommitError {
        match self.manifests.load_manifest(&transaction.profile_id) {
            Ok(Some(current)) if current == transaction.pre_reinstall_manifest => {
                self.rollback(transaction, applied_targets, ReinstallCommitPhase::Manifest)
            }
            Ok(Some(current))
                if current == *candidate_manifest
                    && self
                        .manifests
                        .save_manifest(&transaction.pre_reinstall_manifest)
                        .is_ok() =>
            {
                self.rollback(transaction, applied_targets, ReinstallCommitPhase::Manifest)
            }
            Ok(Some(current)) if current == *candidate_manifest => {
                self.mark_repair_required(&mut transaction);
                ReinstallCommitError::RepairRequired {
                    failed_phase: ReinstallCommitPhase::Manifest,
                }
            }
            _ => {
                self.mark_repair_required(&mut transaction);
                ReinstallCommitError::RepairRequired {
                    failed_phase: ReinstallCommitPhase::Manifest,
                }
            }
        }
    }

    fn rollback(
        &self,
        mut transaction: ReinstallRecoveryTransaction,
        applied_targets: &BTreeSet<InstallTargetPath>,
        failed_phase: ReinstallCommitPhase,
    ) -> ReinstallCommitError {
        transaction
            .transition_to(ReinstallRecoveryTransactionStatus::RollbackRequired)
            .expect("committing transaction can require rollback");
        if self.recovery.save_transaction(&transaction).is_err() {
            self.restore_targets_best_effort(&transaction, applied_targets);
            self.mark_repair_required(&mut transaction);
            return ReinstallCommitError::RepairRequired { failed_phase };
        }

        let mut restore_failed = false;
        let mut cleanup_failed = false;
        let targets = transaction.targets.clone();
        let ordered_targets = targets
            .iter()
            .rev()
            .filter(|target| applied_targets.contains(&target.target_path))
            .chain(
                targets
                    .iter()
                    .rev()
                    .filter(|target| !applied_targets.contains(&target.target_path)),
            )
            .cloned()
            .collect::<Vec<_>>();
        for target in &ordered_targets {
            if applied_targets.contains(&target.target_path) && self.restore_target(target).is_err()
            {
                restore_failed = true;
                continue;
            }

            match self.cleanup_target_snapshot(&mut transaction, &target.target_path) {
                Ok(()) => {}
                Err(CleanupProgressError::Resource) => {
                    cleanup_failed = true;
                    continue;
                }
                Err(CleanupProgressError::Recovery) => {
                    self.restore_targets_best_effort(&transaction, applied_targets);
                    self.mark_repair_required(&mut transaction);
                    return ReinstallCommitError::RepairRequired { failed_phase };
                }
            }

            let mut pruned = transaction.clone();
            pruned
                .targets
                .retain(|candidate| candidate.target_path != target.target_path);
            if self.recovery.save_transaction(&pruned).is_err() {
                self.restore_targets_best_effort(&transaction, applied_targets);
                self.mark_repair_required(&mut transaction);
                return ReinstallCommitError::RepairRequired { failed_phase };
            }
            transaction = pruned;
        }

        if restore_failed || cleanup_failed {
            if self.recovery.save_transaction(&transaction).is_err() {
                self.mark_repair_required(&mut transaction);
                return ReinstallCommitError::RepairRequired { failed_phase };
            }
            return ReinstallCommitError::RollbackRequired { failed_phase };
        }
        transaction
            .transition_to(ReinstallRecoveryTransactionStatus::RolledBack)
            .expect("rollback-required transaction can complete rollback");
        if self.recovery.save_transaction(&transaction).is_err() {
            self.mark_repair_required(&mut transaction);
            return ReinstallCommitError::RepairRequired { failed_phase };
        }
        let cleanup_pending = self
            .recovery
            .remove_transaction(&transaction.profile_id, &transaction.mod_id)
            .is_err();
        ReinstallCommitError::RolledBack {
            failed_phase,
            cleanup_pending,
        }
    }

    fn restore_target(&self, target: &ReinstallRecoveryTarget) -> Result<(), ()> {
        match &target.snapshot {
            ReinstallSnapshotState::NotRequired => Ok(()),
            ReinstallSnapshotState::PreStateAbsent => self
                .game
                .remove_game_file(&target.target_path)
                .map_err(|_| ()),
            ReinstallSnapshotState::Stored { snapshot_ref, .. } => {
                let bytes = self
                    .snapshots
                    .read_snapshot(snapshot_ref)
                    .map_err(|_| ())?
                    .ok_or(())?;
                self.game
                    .write_game_file(&target.target_path, &bytes)
                    .map_err(|_| ())
            }
            ReinstallSnapshotState::CleanupPending { .. }
            | ReinstallSnapshotState::Cleaned { .. } => Ok(()),
        }
    }

    fn restore_targets_best_effort(
        &self,
        transaction: &ReinstallRecoveryTransaction,
        applied_targets: &BTreeSet<InstallTargetPath>,
    ) {
        for target in transaction.targets.iter().rev() {
            if applied_targets.contains(&target.target_path) {
                let _ = self.restore_target(target);
            }
        }
    }

    fn mark_repair_required(&self, transaction: &mut ReinstallRecoveryTransaction) {
        if transaction
            .transition_to(ReinstallRecoveryTransactionStatus::RepairRequired)
            .is_ok()
        {
            let _ = self.recovery.save_transaction(transaction);
        }
    }

    fn abort_before_mutation(&self, transaction: &ReinstallRecoveryTransaction) {
        let mut transaction = transaction.clone();
        if self.recovery.save_transaction(&transaction).is_err() {
            match self
                .recovery
                .load_transaction(&transaction.profile_id, &transaction.mod_id)
            {
                Ok(Some(durable)) if same_pre_mutation_operation(&durable, &transaction) => {
                    transaction = durable;
                }
                Ok(None) => {
                    let snapshot_refs = transaction
                        .targets
                        .iter()
                        .filter_map(snapshot_ref)
                        .map(str::to_owned)
                        .collect::<Vec<_>>();
                    self.remove_snapshot_refs(&snapshot_refs);
                    return;
                }
                Ok(Some(_)) | Err(_) => return,
            }
        }
        let target_paths = transaction
            .targets
            .iter()
            .map(|target| target.target_path.clone())
            .collect::<Vec<_>>();
        for target_path in target_paths {
            if self
                .cleanup_target_snapshot(&mut transaction, &target_path)
                .is_err()
            {
                return;
            }
        }
        let _ = self
            .recovery
            .remove_transaction(&transaction.profile_id, &transaction.mod_id);
    }

    fn remove_snapshot_refs(&self, refs: &[String]) {
        for snapshot_ref in refs.iter().rev() {
            let _ = self.snapshots.remove_snapshot(snapshot_ref);
        }
    }

    pub(crate) fn cleanup_committed(
        &self,
        transaction: &ReinstallRecoveryTransaction,
    ) -> Result<(), ()> {
        cleanup_reinstall_transaction(
            self.backups.as_ref(),
            self.recovery.as_ref(),
            self.snapshots.as_ref(),
            transaction,
            true,
        )
    }

    fn cleanup_target_snapshot(
        &self,
        transaction: &mut ReinstallRecoveryTransaction,
        target_path: &InstallTargetPath,
    ) -> Result<(), CleanupProgressError> {
        cleanup_reinstall_target_snapshot(
            self.recovery.as_ref(),
            self.snapshots.as_ref(),
            transaction,
            target_path,
        )
    }
}

pub(crate) fn cleanup_reinstall_transaction(
    backups: &dyn InstallBackupStore,
    recovery: &dyn ReinstallRecoveryTransactionRepository,
    snapshots: &dyn ReinstallSnapshotStore,
    transaction: &ReinstallRecoveryTransaction,
    remove_stale_original_backups: bool,
) -> Result<(), ()> {
    let mut transaction = transaction.clone();
    let target_paths = transaction
        .targets
        .iter()
        .map(|target| target.target_path.clone())
        .collect::<Vec<_>>();
    for target_path in target_paths {
        cleanup_reinstall_target_snapshot(recovery, snapshots, &mut transaction, &target_path)
            .map_err(|_| ())?;

        let original_backup_ref = remove_stale_original_backups
            .then(|| {
                transaction
                    .targets
                    .iter()
                    .find(|target| target.target_path == target_path)
                    .filter(|target| target.class == ReinstallTargetClass::Stale)
                    .and_then(|target| target.original_backup_ref.clone())
            })
            .flatten();
        if let Some(original_backup_ref) = original_backup_ref {
            backups
                .remove_backup(&original_backup_ref)
                .map_err(|_| ())?;
            let target_index = transaction
                .targets
                .iter()
                .position(|target| target.target_path == target_path)
                .expect("cleanup target remains in transaction");
            transaction.targets[target_index].original_backup_ref = None;
            if recovery.save_transaction(&transaction).is_err() {
                transaction.targets[target_index].original_backup_ref = Some(original_backup_ref);
                return Err(());
            }
        }
    }
    recovery
        .remove_transaction(&transaction.profile_id, &transaction.mod_id)
        .map_err(|_| ())
}

fn cleanup_reinstall_target_snapshot(
    recovery: &dyn ReinstallRecoveryTransactionRepository,
    snapshots: &dyn ReinstallSnapshotStore,
    transaction: &mut ReinstallRecoveryTransaction,
    target_path: &InstallTargetPath,
) -> Result<(), CleanupProgressError> {
    let snapshot = transaction
        .targets
        .iter()
        .find(|target| target.target_path == *target_path)
        .map(|target| target.snapshot.clone())
        .ok_or(CleanupProgressError::Recovery)?;
    let (snapshot_ref, purpose) = match snapshot {
        ReinstallSnapshotState::Stored {
            snapshot_ref,
            purpose,
            cleanup_owner:
                cleanup_owner @ (ReinstallSnapshotCleanupOwner::Transaction
                | ReinstallSnapshotCleanupOwner::PromoteOnCommit),
        } => {
            let target_index = transaction
                .targets
                .iter()
                .position(|target| target.target_path == *target_path)
                .expect("cleanup target remains in transaction");
            transaction.targets[target_index].snapshot = ReinstallSnapshotState::CleanupPending {
                snapshot_ref: snapshot_ref.clone(),
                purpose,
            };
            if recovery.save_transaction(transaction).is_err() {
                transaction.targets[target_index].snapshot = ReinstallSnapshotState::Stored {
                    snapshot_ref,
                    purpose,
                    cleanup_owner,
                };
                return Err(CleanupProgressError::Recovery);
            }
            (snapshot_ref, purpose)
        }
        ReinstallSnapshotState::CleanupPending {
            snapshot_ref,
            purpose,
        } => (snapshot_ref, purpose),
        ReinstallSnapshotState::NotRequired
        | ReinstallSnapshotState::PreStateAbsent
        | ReinstallSnapshotState::Cleaned { .. }
        | ReinstallSnapshotState::Stored {
            cleanup_owner: ReinstallSnapshotCleanupOwner::Manifest,
            ..
        } => return Ok(()),
    };

    snapshots
        .remove_snapshot(&snapshot_ref)
        .map_err(|_| CleanupProgressError::Resource)?;
    let target_index = transaction
        .targets
        .iter()
        .position(|target| target.target_path == *target_path)
        .expect("cleanup target remains in transaction");
    transaction.targets[target_index].snapshot = ReinstallSnapshotState::Cleaned { purpose };
    if recovery.save_transaction(transaction).is_err() {
        transaction.targets[target_index].snapshot = ReinstallSnapshotState::CleanupPending {
            snapshot_ref,
            purpose,
        };
        return Err(CleanupProgressError::Recovery);
    }
    Ok(())
}

fn same_pre_mutation_operation(
    durable: &ReinstallRecoveryTransaction,
    attempted: &ReinstallRecoveryTransaction,
) -> bool {
    matches!(
        durable.status,
        ReinstallRecoveryTransactionStatus::Planned
            | ReinstallRecoveryTransactionStatus::Committing
    ) && durable.profile_id == attempted.profile_id
        && durable.mod_id == attempted.mod_id
        && durable.old_revision_id == attempted.old_revision_id
        && durable.candidate_revision_id == attempted.candidate_revision_id
        && durable.plan_token == attempted.plan_token
        && durable.plan_hash == attempted.plan_hash
        && durable.pre_reinstall_manifest == attempted.pre_reinstall_manifest
        && durable.candidate_replacement_bindings == attempted.candidate_replacement_bindings
        && durable.targets == attempted.targets
}

fn candidate_file(
    target: &PreparedReinstallTarget,
) -> Option<&crate::reinstall::PreparedSourceFile> {
    target
        .candidate_files
        .iter()
        .max_by_key(|source| source.provider.layer.priority)
}

fn snapshot_ref(target: &ReinstallRecoveryTarget) -> Option<&str> {
    match &target.snapshot {
        ReinstallSnapshotState::Stored { snapshot_ref, .. }
        | ReinstallSnapshotState::CleanupPending { snapshot_ref, .. } => Some(snapshot_ref),
        ReinstallSnapshotState::NotRequired
        | ReinstallSnapshotState::PreStateAbsent
        | ReinstallSnapshotState::Cleaned { .. } => None,
    }
}

fn build_candidate_manifest(
    prepared: &PreparedReinstall,
    transaction: &ReinstallRecoveryTransaction,
) -> Result<InstallManifest, ()> {
    let backup_refs = transaction
        .targets
        .iter()
        .map(|target| {
            let promoted = match &target.snapshot {
                ReinstallSnapshotState::Stored {
                    snapshot_ref,
                    cleanup_owner: ReinstallSnapshotCleanupOwner::PromoteOnCommit,
                    ..
                } => Some(snapshot_ref.clone()),
                _ => None,
            };
            (
                target.target_path.clone(),
                target.original_backup_ref.clone().or(promoted),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let entries = prepared
        .source_files
        .iter()
        .map(|source| InstallManifestEntry {
            target_path: source.provider.target_path.clone(),
            mod_id: source.provider.mod_id.clone(),
            revision_id: Some(prepared.candidate.revision_id.clone()),
            package_file_id: source.provider.package_file_id.clone(),
            layer: source.provider.layer.clone(),
            backup_ref: backup_refs
                .get(&source.provider.target_path)
                .cloned()
                .flatten(),
            installed_file: Some(source.summary.clone()),
        })
        .collect();
    let mut manifest = replace_entries_and_bindings_for_mod(
        &prepared.old_manifest,
        &prepared.request.mod_id,
        &prepared.legacy_provenance,
        &prepared.candidate.revision_id,
        entries,
        prepared.candidate_replacement_bindings.clone(),
    )
    .map_err(|_| ())?;
    manifest.status = InstallManifestStatus::Completed;
    manifest.completed_at = Some(manifest_timestamp());
    manifest.plan_hash = Some(prepared.plan_hash.clone());
    Ok(manifest)
}

pub(crate) fn promote_manifest_snapshots(transaction: &mut ReinstallRecoveryTransaction) {
    for target in &mut transaction.targets {
        if let ReinstallSnapshotState::Stored { cleanup_owner, .. } = &mut target.snapshot {
            if *cleanup_owner == ReinstallSnapshotCleanupOwner::PromoteOnCommit {
                *cleanup_owner = ReinstallSnapshotCleanupOwner::Manifest;
            }
        }
    }
}

fn manifest_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("unix:{}", duration.as_secs()))
        .unwrap_or_else(|_| "unix:0".to_owned())
}
