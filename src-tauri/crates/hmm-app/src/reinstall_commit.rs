#![allow(dead_code)] // The Task 6 runner will call this crate-internal prepared commit seam.

use crate::reinstall::{
    summarize, PreparedReinstall, PreparedReinstallTarget, ReinstallCandidateSourceReader,
};
use hmm_core::{
    replace_entries_for_mod, InstallManifest, InstallManifestEntry, InstallManifestStatus,
    InstallTargetPath, ReinstallRecoveryTarget, ReinstallRecoveryTransaction,
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
    #[error("reinstall failed during {failed_phase:?}; rollback succeeded")]
    RolledBack { failed_phase: ReinstallCommitPhase },
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
            Self::RolledBack { failed_phase } => phase_error_code(*failed_phase),
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
            .load_transaction(&prepared.request.profile_id, &prepared.request.mod_id)
            .map_err(|_| ReinstallCommitError::Failed {
                phase: ReinstallCommitPhase::Revalidation,
            })?;
        if active.is_some() {
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

            let mut pruned = transaction.clone();
            pruned
                .targets
                .retain(|candidate| candidate.target_path != target.target_path);
            if self.recovery.save_transaction(&pruned).is_err() {
                self.restore_targets_best_effort(&transaction, applied_targets);
                self.mark_repair_required(&mut transaction);
                return ReinstallCommitError::RepairRequired { failed_phase };
            }
            if let Some(snapshot_ref) = snapshot_ref(target) {
                if self.snapshots.remove_snapshot(snapshot_ref).is_err() {
                    self.restore_targets_best_effort(&transaction, applied_targets);
                    if self.recovery.save_transaction(&transaction).is_err() {
                        self.mark_repair_required(&mut transaction);
                        return ReinstallCommitError::RepairRequired { failed_phase };
                    }
                    return ReinstallCommitError::RollbackRequired { failed_phase };
                }
            }
            transaction = pruned;
        }

        if restore_failed {
            let _ = self.recovery.save_transaction(&transaction);
            return ReinstallCommitError::RollbackRequired { failed_phase };
        }
        transaction
            .transition_to(ReinstallRecoveryTransactionStatus::RolledBack)
            .expect("rollback-required transaction can complete rollback");
        if self.recovery.save_transaction(&transaction).is_err() {
            self.mark_repair_required(&mut transaction);
            return ReinstallCommitError::RepairRequired { failed_phase };
        }
        let _ = self
            .recovery
            .remove_transaction(&transaction.profile_id, &transaction.mod_id);
        ReinstallCommitError::RolledBack { failed_phase }
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
        if self
            .recovery
            .remove_transaction(&transaction.profile_id, &transaction.mod_id)
            .is_ok()
        {
            let refs = transaction
                .targets
                .iter()
                .filter_map(snapshot_ref)
                .map(str::to_owned)
                .collect::<Vec<_>>();
            self.remove_snapshot_refs(&refs);
        }
    }

    fn remove_snapshot_refs(&self, refs: &[String]) {
        for snapshot_ref in refs.iter().rev() {
            let _ = self.snapshots.remove_snapshot(snapshot_ref);
        }
    }

    fn cleanup_committed(&self, transaction: &ReinstallRecoveryTransaction) -> Result<(), ()> {
        for target in &transaction.targets {
            if let ReinstallSnapshotState::Stored {
                snapshot_ref,
                cleanup_owner: ReinstallSnapshotCleanupOwner::Transaction,
                ..
            } = &target.snapshot
            {
                self.snapshots
                    .remove_snapshot(snapshot_ref)
                    .map_err(|_| ())?;
            }
            if target.class == ReinstallTargetClass::Stale {
                if let Some(original_backup_ref) = &target.original_backup_ref {
                    self.backups
                        .remove_backup(original_backup_ref)
                        .map_err(|_| ())?;
                }
            }
        }
        self.recovery
            .remove_transaction(&transaction.profile_id, &transaction.mod_id)
            .map_err(|_| ())
    }
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
        ReinstallSnapshotState::Stored { snapshot_ref, .. } => Some(snapshot_ref),
        ReinstallSnapshotState::NotRequired | ReinstallSnapshotState::PreStateAbsent => None,
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
    let mut manifest = replace_entries_for_mod(
        &prepared.old_manifest,
        &prepared.request.mod_id,
        &prepared.legacy_provenance,
        &prepared.candidate.revision_id,
        entries,
    )
    .map_err(|_| ())?;
    manifest.status = InstallManifestStatus::Completed;
    manifest.completed_at = Some(manifest_timestamp());
    manifest.plan_hash = Some(prepared.plan_hash.clone());
    Ok(manifest)
}

fn promote_manifest_snapshots(transaction: &mut ReinstallRecoveryTransaction) {
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
