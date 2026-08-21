use hmm_core::{
    FileLayer, GameId, InstallAction, InstallFileProvider, InstallManifest, InstallManifestEntry,
    InstallManifestStatus, InstallPlan, InstallRecoveryRecord, InstallRecoveryRecordEntry,
    InstallRecoveryRecordStatus, InstallTargetPath, InstallTargetPathError, InstalledFileSummary,
    ModId, ModRevisionId, PackageFileId, ProfileId, ReplacementBindingSnapshot,
    INSTALL_MANIFEST_SCHEMA_VERSION_V2,
};
use hmm_ports::{
    GameAdapter, GameRunningDetector, GameRunningStatus, InstallBackupStore, InstallGameFileSystem,
    InstallManifestRepository, InstallRecoveryRecordRepository, InstallSourceFileReader,
    ModImportResultRepository, ModImportSandboxLocator, ModPackageInstallFileScanRequest,
    ModPackageInstallFileScanner,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const INSTALL_PLAN_MANIFEST_BACKEND: &str = "install_plan";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInstallPlanRequest {
    pub allowed_target_roots: Vec<String>,
    pub files: Vec<InstallPlanFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildImportedModInstallPlanRequest {
    pub game_id: GameId,
    pub mod_id: ModId,
    pub layer: FileLayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlanFile {
    pub mod_id: ModId,
    pub package_file_id: PackageFileId,
    pub target_path: String,
    pub layer: FileLayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInstallPlanRequest {
    /// 供游戏运行中闸门使用。写入玩家文件的请求必须指明目标游戏，
    /// 否则无法在动任何文件之前判断该游戏是否正在运行。
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub plan: InstallPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallCommitResult {
    pub manifest: InstallManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallModRequest {
    /// 同 [`CommitInstallPlanRequest::game_id`]：卸载同样会删除和还原玩家文件。
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallModResult {
    pub manifest: InstallManifest,
    pub removed_file_count: usize,
    pub restored_file_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallCommitPhase {
    ManifestRead,
    SourceRead,
    TargetRead,
    Backup,
    Write,
    Manifest,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallCommitError {
    #[error("game is running")]
    GameRunning,
    #[error("game running state is unknown")]
    GameRunningUnknown,
    #[error("install plan has blocking conflicts")]
    PlanHasBlockingConflicts,
    #[error("install plan replacement bindings are invalid")]
    PlanHasInvalidReplacementBindings,
    #[error("install plan does not match the expected Mod revision identity")]
    PlanHasInvalidRevisionIdentity,
    #[error("install commit failed during {phase:?}")]
    Failed { phase: InstallCommitPhase },
    #[error("install commit failed during {failed_phase:?}; rollback succeeded")]
    RollbackSucceeded { failed_phase: InstallCommitPhase },
    #[error("install commit failed during {failed_phase:?}; rollback failed")]
    RollbackFailed { failed_phase: InstallCommitPhase },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum UninstallModError {
    #[error("game is running")]
    GameRunning,
    #[error("game running state is unknown")]
    GameRunningUnknown,
    #[error("game instance is unavailable")]
    GameInstanceUnavailable,
    #[error("install manifest is unavailable")]
    ManifestUnavailable,
    #[error("mod is not installed in this profile")]
    ModNotInstalled,
    #[error("installed Mod revision does not match the expected revision")]
    InstalledRevisionMismatch,
    #[error("install manifest state does not match the expected uninstall snapshot")]
    ManifestStateMismatch,
    #[error("installed file summary is required for safe uninstall")]
    MissingInstalledFileSummary,
    #[error("installed target state does not match manifest")]
    TargetStateMismatch,
    #[error("install backup is unavailable")]
    BackupUnavailable,
    #[error("uninstall failed during manifest save")]
    ManifestSaveFailed,
    #[error("uninstall failed while removing game file")]
    RemoveFailed,
    #[error("uninstall failed while restoring game file")]
    RestoreFailed,
    #[error("uninstall rollback failed")]
    RollbackFailed { failed_phase: UninstallModPhase },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallModPhase {
    Revalidate,
    Remove,
    Restore,
    ManifestSave,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallPlanningError {
    #[error("invalid install target path")]
    InvalidTargetPath {
        package_file_id: PackageFileId,
        source: InstallTargetPathError,
    },
    #[error("imported mod install planning sources are not configured")]
    ImportedModSourcesUnavailable,
    #[error("game adapter not found")]
    GameAdapterNotFound { game_id: GameId },
    #[error("imported mod was not found")]
    ImportedModNotFound { mod_id: ModId },
    #[error("failed to read imported mod analysis")]
    ImportedModAnalysisUnavailable,
    #[error("failed to locate imported mod sandbox")]
    ImportedModSandboxUnavailable,
    #[error("failed to scan imported mod files")]
    ImportedModFileScanUnavailable,
}

#[derive(Clone)]
struct ImportedModInstallPlanSources {
    result_repository: Arc<dyn ModImportResultRepository>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    file_scanner: Arc<dyn ModPackageInstallFileScanner>,
    game_adapters: Vec<Arc<dyn GameAdapter>>,
}

#[derive(Default, Clone)]
pub struct InstallPlanningService {
    imported_mod_sources: Option<ImportedModInstallPlanSources>,
}

#[derive(Clone)]
pub struct InstallCommitService {
    source_files: Arc<dyn InstallSourceFileReader>,
    game_files: Arc<dyn InstallGameFileSystem>,
    backup_store: Arc<dyn InstallBackupStore>,
    manifest_repository: Arc<dyn InstallManifestRepository>,
    recovery_record_repository: Option<Arc<dyn InstallRecoveryRecordRepository>>,
    game_running_detector: Option<Arc<dyn GameRunningDetector>>,
}

#[derive(Clone)]
pub struct UninstallModService {
    game_files: Arc<dyn InstallGameFileSystem>,
    backup_store: Arc<dyn InstallBackupStore>,
    manifest_repository: Arc<dyn InstallManifestRepository>,
    game_running_detector: Option<Arc<dyn GameRunningDetector>>,
}

/// 游戏运行中不得写入玩家文件。
///
/// MHW:I 运行时持有 `nativePC` 下的文件句柄，写入会触发 sharing violation，
/// 而随后的 rollback 要写回同一批仍被锁的文件、同样会失败，把一次普通的安装失败
/// 升级成需要人工恢复的 `RollbackRequired`。因此这里必须在建立任何 backup 或
/// recovery 记录之前 fail closed。
///
/// `Unknown` 与 `Running` 同样拒绝：判定不出来时不能假设游戏没开。
/// 与 `save_restore.rs` 的存档恢复闸门保持同一语义。
///
/// detector 缺席时放行，让大量只用 fake 文件系统的单元测试不必各自装配 detector。
/// 这一层因此是 fail-open 的，真正的强制不在类型上，而在
/// `hmm-runtime` 的生产装配测试：它断言 composition 构造出来的 install/uninstall
/// 服务在游戏运行时确实拒绝写入。改动装配时那个测试会先红。
fn ensure_game_not_running<E>(
    detector: Option<&Arc<dyn GameRunningDetector>>,
    game_id: &GameId,
    running: E,
    unknown: E,
) -> Result<(), E> {
    match detector {
        Some(detector) => match detector.game_running_status(game_id) {
            GameRunningStatus::Running => Err(running),
            GameRunningStatus::Unknown => Err(unknown),
            GameRunningStatus::NotRunning => Ok(()),
        },
        None => Ok(()),
    }
}

#[derive(Clone)]
struct AppliedInstallChange {
    target_path: InstallTargetPath,
    source_bytes: Vec<u8>,
    previous_bytes: Option<Vec<u8>>,
    pending_backup_ref: Option<String>,
    entry: InstallManifestEntry,
}

struct ActiveInstallRecoveryRecords {
    repository: Arc<dyn InstallRecoveryRecordRepository>,
    records: BTreeMap<ModId, InstallRecoveryRecord>,
    committing_saved: bool,
}

struct PreparedUninstallChange {
    entry: InstallManifestEntry,
    current_bytes: Vec<u8>,
    backup_bytes: Option<Vec<u8>>,
}

struct AppliedUninstallChange {
    target_path: InstallTargetPath,
    previous_bytes: Vec<u8>,
}

impl InstallCommitService {
    pub fn new(
        source_files: Arc<dyn InstallSourceFileReader>,
        game_files: Arc<dyn InstallGameFileSystem>,
        backup_store: Arc<dyn InstallBackupStore>,
        manifest_repository: Arc<dyn InstallManifestRepository>,
    ) -> Self {
        Self {
            source_files,
            game_files,
            backup_store,
            manifest_repository,
            recovery_record_repository: None,
            game_running_detector: None,
        }
    }

    /// 接入游戏运行中闸门。生产装配必须调用；缺失时由 hmm-runtime 的装配测试兜底。
    #[must_use]
    pub fn with_game_running_detector(
        mut self,
        game_running_detector: Arc<dyn GameRunningDetector>,
    ) -> Self {
        self.game_running_detector = Some(game_running_detector);
        self
    }

    pub fn new_with_recovery_records(
        source_files: Arc<dyn InstallSourceFileReader>,
        game_files: Arc<dyn InstallGameFileSystem>,
        backup_store: Arc<dyn InstallBackupStore>,
        manifest_repository: Arc<dyn InstallManifestRepository>,
        recovery_record_repository: Arc<dyn InstallRecoveryRecordRepository>,
    ) -> Self {
        Self {
            source_files,
            game_files,
            backup_store,
            manifest_repository,
            recovery_record_repository: Some(recovery_record_repository),
            game_running_detector: None,
        }
    }

    pub fn commit_plan(
        &self,
        request: CommitInstallPlanRequest,
    ) -> Result<InstallCommitResult, InstallCommitError> {
        self.commit_plan_with_revision(request, None)
    }

    pub fn commit_plan_for_revision(
        &self,
        request: CommitInstallPlanRequest,
        mod_id: ModId,
        revision_id: ModRevisionId,
    ) -> Result<InstallCommitResult, InstallCommitError> {
        self.commit_plan_with_revision(request, Some((mod_id, revision_id)))
    }

    fn commit_plan_with_revision(
        &self,
        request: CommitInstallPlanRequest,
        expected_revision: Option<(ModId, ModRevisionId)>,
    ) -> Result<InstallCommitResult, InstallCommitError> {
        let CommitInstallPlanRequest {
            game_id,
            profile_id,
            plan,
        } = request;

        // 必须是本函数的第一件事：此时还没读 manifest、没建 backup、
        // 没写 recovery 记录，拒绝是完全无副作用的。
        ensure_game_not_running(
            self.game_running_detector.as_ref(),
            &game_id,
            InstallCommitError::GameRunning,
            InstallCommitError::GameRunningUnknown,
        )?;

        if plan.has_blocking_conflicts() {
            return Err(InstallCommitError::PlanHasBlockingConflicts);
        }
        let expected_revision_id = expected_revision
            .as_ref()
            .map(|(_, revision_id)| revision_id);
        if plan
            .validate_replacement_bindings_for_profile_and_revision(
                &profile_id,
                expected_revision_id,
            )
            .is_err()
        {
            return Err(InstallCommitError::PlanHasInvalidReplacementBindings);
        }
        if let Some((expected_mod_id, _)) = &expected_revision {
            if plan.actions.is_empty()
                || plan
                    .actions
                    .iter()
                    .any(|action| action.provider.mod_id != *expected_mod_id)
            {
                return Err(InstallCommitError::PlanHasInvalidRevisionIdentity);
            }
        }

        let plan_hash = install_plan_hash(&plan);
        let replacement_bindings = plan.replacement_bindings.clone();
        let existing_manifest = self
            .manifest_repository
            .load_manifest(&profile_id)
            .map_err(|_| InstallCommitError::Failed {
                phase: InstallCommitPhase::ManifestRead,
            })?;
        if let Some((expected_mod_id, expected_revision_id)) = &expected_revision {
            let existing_revisions = existing_manifest
                .as_ref()
                .into_iter()
                .flat_map(|manifest| &manifest.entries)
                .filter(|entry| entry.mod_id == *expected_mod_id)
                .map(|entry| entry.revision_id.as_ref())
                .collect::<std::collections::BTreeSet<_>>();
            if !existing_revisions.is_empty()
                && existing_revisions != BTreeSet::from([Some(expected_revision_id)])
            {
                return Err(InstallCommitError::PlanHasInvalidRevisionIdentity);
            }
        }
        let existing_backup_refs = manifest_backup_refs_by_target(existing_manifest.as_ref());
        let mut recovery_records = self
            .start_recovery_records(&profile_id, &plan.actions, &existing_backup_refs)
            .map_err(|_| InstallCommitError::Failed {
                phase: InstallCommitPhase::Manifest,
            })?;
        let mut sourced_actions = Vec::with_capacity(plan.actions.len());

        for action in plan.actions {
            let source_bytes = match self
                .source_files
                .read_source_file(&action.provider.package_file_id)
            {
                Ok(bytes) => bytes,
                Err(_) => {
                    let error = InstallCommitError::Failed {
                        phase: InstallCommitPhase::SourceRead,
                    };
                    Self::finish_recovery_records_after_failure(&mut recovery_records, &error);
                    return Err(error);
                }
            };
            sourced_actions.push((action, source_bytes));
        }

        let mut prepared_changes =
            Vec::<AppliedInstallChange>::with_capacity(sourced_actions.len());
        let mut prepared_target_indexes = HashMap::<InstallTargetPath, usize>::new();

        for (action, source_bytes) in sourced_actions {
            let previous_bytes = if let Some(index) =
                prepared_target_indexes.get(&action.target_path)
            {
                Some(prepared_changes[*index].source_bytes.clone())
            } else {
                match self.game_files.read_game_file(&action.target_path) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        let error = InstallCommitError::Failed {
                            phase: InstallCommitPhase::TargetRead,
                        };
                        self.remove_pending_backups(&prepared_changes);
                        Self::finish_recovery_records_after_failure(&mut recovery_records, &error);
                        return Err(error);
                    }
                }
            };
            let backup_ref = if let Some(bytes) = previous_bytes.as_deref() {
                match self.backup_store.store_backup(&action.target_path, bytes) {
                    Ok(backup_ref) => Some(backup_ref),
                    Err(_) => {
                        let error = InstallCommitError::Failed {
                            phase: InstallCommitPhase::Backup,
                        };
                        self.remove_pending_backups(&prepared_changes);
                        Self::finish_recovery_records_after_failure(&mut recovery_records, &error);
                        return Err(error);
                    }
                }
            } else {
                None
            };
            let manifest_backup_ref = match existing_backup_refs.get(&action.target_path) {
                Some(existing_backup_ref) => existing_backup_ref.clone(),
                None => backup_ref.clone(),
            };
            let entry = InstallManifestEntry {
                target_path: action.target_path.clone(),
                mod_id: action.provider.mod_id.clone(),
                revision_id: expected_revision_id.cloned(),
                package_file_id: action.provider.package_file_id.clone(),
                layer: action.provider.layer.clone(),
                backup_ref: manifest_backup_ref,
                installed_file: Some(installed_file_summary(&source_bytes)),
            };

            prepared_target_indexes.insert(action.target_path.clone(), prepared_changes.len());
            prepared_changes.push(AppliedInstallChange {
                target_path: action.target_path,
                source_bytes,
                previous_bytes,
                pending_backup_ref: backup_ref,
                entry,
            });
        }

        for (index, change) in prepared_changes.iter().enumerate() {
            if let Some(records) = recovery_records.as_mut() {
                if records
                    .update_entry_for_rollback(&change.entry, change.pending_backup_ref.clone())
                    .and_then(|_| records.ensure_committing())
                    .is_err()
                {
                    let error = self.fail_or_rollback_with_pending_backup(
                        &prepared_changes[..index],
                        change.pending_backup_ref.as_deref(),
                        &mut recovery_records,
                        InstallCommitPhase::Manifest,
                    );
                    self.remove_pending_backups(&prepared_changes[index + 1..]);
                    Self::finish_recovery_records_after_failure(&mut recovery_records, &error);
                    return Err(error);
                }
            }

            if self
                .game_files
                .write_game_file(&change.target_path, &change.source_bytes)
                .is_err()
            {
                let error = self.fail_or_rollback(
                    &prepared_changes[..=index],
                    &mut recovery_records,
                    InstallCommitPhase::Write,
                );
                self.remove_pending_backups(&prepared_changes[index + 1..]);
                Self::finish_recovery_records_after_failure(&mut recovery_records, &error);
                return Err(error);
            }
        }

        let manifest = merge_install_manifest(
            profile_id,
            existing_manifest,
            prepared_changes
                .iter()
                .map(|change| change.entry.clone())
                .collect(),
            replacement_bindings,
            plan_hash,
        );

        if self.manifest_repository.save_manifest(&manifest).is_err() {
            let error = self.fail_or_rollback(
                &prepared_changes,
                &mut recovery_records,
                InstallCommitPhase::Manifest,
            );
            Self::finish_recovery_records_after_failure(&mut recovery_records, &error);
            return Err(error);
        }
        if let Some(records) = recovery_records.as_mut() {
            records.update_entries_for_completed_manifest(&manifest.entries);
            records.mark_completed_best_effort();
        }
        self.remove_obsolete_pending_backups(&prepared_changes);

        Ok(InstallCommitResult { manifest })
    }

    fn start_recovery_records(
        &self,
        profile_id: &ProfileId,
        actions: &[InstallAction],
        existing_backup_refs: &HashMap<InstallTargetPath, Option<String>>,
    ) -> anyhow::Result<Option<ActiveInstallRecoveryRecords>> {
        let Some(repository) = self.recovery_record_repository.clone() else {
            return Ok(None);
        };

        let mut records = BTreeMap::<ModId, InstallRecoveryRecord>::new();
        for action in actions {
            let record = records
                .entry(action.provider.mod_id.clone())
                .or_insert_with(|| InstallRecoveryRecord {
                    profile_id: profile_id.clone(),
                    mod_id: action.provider.mod_id.clone(),
                    status: InstallRecoveryRecordStatus::Planned,
                    entries: Vec::new(),
                });
            record.entries.push(InstallRecoveryRecordEntry {
                target_path: action.target_path.clone(),
                package_file_id: action.provider.package_file_id.clone(),
                backup_ref: existing_backup_refs
                    .get(&action.target_path)
                    .cloned()
                    .flatten(),
                installed_file: None,
            });
        }

        if records.is_empty() {
            return Ok(None);
        }

        let active_records = ActiveInstallRecoveryRecords {
            repository,
            records,
            committing_saved: false,
        };
        if let Err(error) = active_records.save_all() {
            active_records.remove_all_best_effort();
            return Err(error);
        }
        Ok(Some(active_records))
    }

    fn finish_recovery_records_after_failure(
        recovery_records: &mut Option<ActiveInstallRecoveryRecords>,
        error: &InstallCommitError,
    ) {
        if let Some(records) = recovery_records.as_mut() {
            match error {
                InstallCommitError::RollbackFailed { .. } => {
                    records.mark_rollback_required_best_effort();
                }
                _ => records.remove_all_best_effort(),
            }
        }
    }

    fn fail_or_rollback(
        &self,
        applied_changes: &[AppliedInstallChange],
        recovery_records: &mut Option<ActiveInstallRecoveryRecords>,
        failed_phase: InstallCommitPhase,
    ) -> InstallCommitError {
        self.fail_or_rollback_with_pending_backup(
            applied_changes,
            None,
            recovery_records,
            failed_phase,
        )
    }

    fn fail_or_rollback_with_pending_backup(
        &self,
        applied_changes: &[AppliedInstallChange],
        pending_backup_ref: Option<&str>,
        recovery_records: &mut Option<ActiveInstallRecoveryRecords>,
        failed_phase: InstallCommitPhase,
    ) -> InstallCommitError {
        if applied_changes.is_empty() && pending_backup_ref.is_none() {
            return InstallCommitError::Failed {
                phase: failed_phase,
            };
        }

        let rollback_result = self.rollback(applied_changes, recovery_records);
        if let Some(backup_ref) = pending_backup_ref {
            let _ = self.backup_store.remove_backup(backup_ref);
        }

        if rollback_result.is_ok() {
            InstallCommitError::RollbackSucceeded { failed_phase }
        } else {
            InstallCommitError::RollbackFailed { failed_phase }
        }
    }

    fn rollback(
        &self,
        applied_changes: &[AppliedInstallChange],
        recovery_records: &mut Option<ActiveInstallRecoveryRecords>,
    ) -> anyhow::Result<()> {
        let mut rollback_error = None;

        for change in applied_changes.iter().rev() {
            let restore_result = if let Some(previous_bytes) = &change.previous_bytes {
                self.game_files
                    .write_game_file(&change.target_path, previous_bytes)
            } else {
                self.game_files.remove_game_file(&change.target_path)
            };

            match restore_result {
                Ok(()) => {
                    let recovery_result = match recovery_records.as_mut() {
                        Some(records) => records.remove_rolled_back_entry(change),
                        None => Ok(()),
                    };
                    match recovery_result {
                        Ok(()) => {
                            if let Some(backup_ref) = &change.pending_backup_ref {
                                let _ = self.backup_store.remove_backup(backup_ref);
                            }
                        }
                        Err(error) => {
                            rollback_error.get_or_insert(error);
                        }
                    }
                }
                Err(error) => {
                    rollback_error.get_or_insert(error);
                }
            }
        }

        match rollback_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn remove_pending_backups(&self, changes: &[AppliedInstallChange]) {
        for change in changes.iter().rev() {
            if let Some(backup_ref) = &change.pending_backup_ref {
                let _ = self.backup_store.remove_backup(backup_ref);
            }
        }
    }

    fn remove_obsolete_pending_backups(&self, applied_changes: &[AppliedInstallChange]) {
        for change in applied_changes {
            if let Some(pending_backup_ref) = &change.pending_backup_ref {
                if change.entry.backup_ref.as_deref() != Some(pending_backup_ref.as_str()) {
                    let _ = self.backup_store.remove_backup(pending_backup_ref);
                }
            }
        }
    }
}

impl ActiveInstallRecoveryRecords {
    fn save_all(&self) -> anyhow::Result<()> {
        for record in self.records.values() {
            self.repository.save_record(record)?;
        }
        Ok(())
    }

    fn ensure_committing(&mut self) -> anyhow::Result<()> {
        if self.committing_saved {
            return Ok(());
        }

        self.transition_all_to(InstallRecoveryRecordStatus::Committing)?;
        self.committing_saved = true;
        Ok(())
    }

    fn update_entry_for_rollback(
        &mut self,
        entry: &InstallManifestEntry,
        rollback_backup_ref: Option<String>,
    ) -> anyhow::Result<()> {
        let Some(record) = self.records.get_mut(&entry.mod_id) else {
            return Ok(());
        };
        let Some(record_entry) = record.entries.iter_mut().find(|record_entry| {
            record_entry.target_path == entry.target_path
                && record_entry.package_file_id == entry.package_file_id
        }) else {
            return Ok(());
        };

        record_entry.backup_ref = rollback_backup_ref;
        record_entry.installed_file = entry.installed_file.clone();
        if self.committing_saved {
            self.repository.save_record(record)?;
        }
        Ok(())
    }

    fn update_entries_for_completed_manifest(&mut self, entries: &[InstallManifestEntry]) {
        for entry in entries {
            let Some(record) = self.records.get_mut(&entry.mod_id) else {
                continue;
            };
            let Some(record_entry) = record.entries.iter_mut().find(|record_entry| {
                record_entry.target_path == entry.target_path
                    && record_entry.package_file_id == entry.package_file_id
            }) else {
                continue;
            };

            record_entry.backup_ref = entry.backup_ref.clone();
            record_entry.installed_file = entry.installed_file.clone();
        }
    }

    fn remove_rolled_back_entry(&mut self, change: &AppliedInstallChange) -> anyhow::Result<()> {
        let Some(record) = self.records.get(&change.entry.mod_id) else {
            return Ok(());
        };
        let mut updated = record.clone();
        let Some(index) = updated.entries.iter().position(|entry| {
            entry.target_path == change.entry.target_path
                && entry.package_file_id == change.entry.package_file_id
        }) else {
            return Ok(());
        };
        updated.entries.remove(index);

        if updated.entries.is_empty() {
            self.repository
                .remove_record(&updated.profile_id, &updated.mod_id)?;
            self.records.remove(&updated.mod_id);
        } else {
            self.repository.save_record(&updated)?;
            self.records.insert(updated.mod_id.clone(), updated);
        }
        Ok(())
    }

    fn mark_completed_best_effort(&mut self) {
        let _ = self.transition_all_to(InstallRecoveryRecordStatus::Completed);
        self.remove_all_best_effort();
    }

    fn mark_rollback_required_best_effort(&mut self) {
        let _ = self.transition_all_to(InstallRecoveryRecordStatus::RollbackRequired);
    }

    fn remove_all_best_effort(&self) {
        for record in self.records.values() {
            let _ = self
                .repository
                .remove_record(&record.profile_id, &record.mod_id);
        }
    }

    fn transition_all_to(&mut self, status: InstallRecoveryRecordStatus) -> anyhow::Result<()> {
        for record in self.records.values_mut() {
            record.transition_to(status)?;
            self.repository.save_record(record)?;
        }
        Ok(())
    }
}

impl UninstallModService {
    pub fn new(
        game_files: Arc<dyn InstallGameFileSystem>,
        backup_store: Arc<dyn InstallBackupStore>,
        manifest_repository: Arc<dyn InstallManifestRepository>,
    ) -> Self {
        Self {
            game_files,
            backup_store,
            manifest_repository,
            game_running_detector: None,
        }
    }

    /// 接入游戏运行中闸门。生产装配必须调用；缺失时由 hmm-runtime 的装配测试兜底。
    #[must_use]
    pub fn with_game_running_detector(
        mut self,
        game_running_detector: Arc<dyn GameRunningDetector>,
    ) -> Self {
        self.game_running_detector = Some(game_running_detector);
        self
    }

    pub fn uninstall_mod(
        &self,
        request: UninstallModRequest,
    ) -> Result<UninstallModResult, UninstallModError> {
        self.uninstall_mod_internal(request, None, None)
    }

    pub fn uninstall_mod_for_revision(
        &self,
        request: UninstallModRequest,
        expected_installed_revision_id: ModRevisionId,
    ) -> Result<UninstallModResult, UninstallModError> {
        self.uninstall_mod_internal(request, Some(&expected_installed_revision_id), None)
    }

    pub fn uninstall_mod_for_revision_and_manifest(
        &self,
        request: UninstallModRequest,
        expected_installed_revision_id: ModRevisionId,
        expected_manifest_digest: &str,
    ) -> Result<UninstallModResult, UninstallModError> {
        self.uninstall_mod_internal(
            request,
            Some(&expected_installed_revision_id),
            Some(expected_manifest_digest),
        )
    }

    fn uninstall_mod_internal(
        &self,
        request: UninstallModRequest,
        expected_installed_revision_id: Option<&ModRevisionId>,
        expected_manifest_digest: Option<&str>,
    ) -> Result<UninstallModResult, UninstallModError> {
        // 必须先于 manifest 读取与任何删除/还原动作。
        ensure_game_not_running(
            self.game_running_detector.as_ref(),
            &request.game_id,
            UninstallModError::GameRunning,
            UninstallModError::GameRunningUnknown,
        )?;

        let manifest = self
            .manifest_repository
            .load_manifest(&request.profile_id)
            .map_err(|_| UninstallModError::ManifestUnavailable)?
            .ok_or(UninstallModError::ModNotInstalled)?;
        if expected_manifest_digest.is_some_and(|expected| {
            manifest.profile_id != request.profile_id
                || manifest.validate().is_err()
                || uninstall_manifest_snapshot_digest(&manifest, &request.mod_id) != expected
        }) {
            return Err(UninstallModError::ManifestStateMismatch);
        }
        let InstallManifest {
            manifest_id,
            schema_version,
            schema_migration,
            backend,
            created_at,
            status,
            entries,
            replacement_bindings,
            ..
        } = manifest;

        let (uninstall_entries, kept_entries): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .partition(|entry| entry.mod_id == request.mod_id);
        let kept_replacement_bindings = replacement_bindings
            .into_iter()
            .filter(|snapshot| snapshot.mod_id() != &request.mod_id)
            .collect();

        if uninstall_entries.is_empty() {
            return Err(UninstallModError::ModNotInstalled);
        }
        if let Some(expected_revision_id) = expected_installed_revision_id {
            if schema_version != INSTALL_MANIFEST_SCHEMA_VERSION_V2
                || uninstall_entries
                    .iter()
                    .any(|entry| entry.revision_id.as_ref() != Some(expected_revision_id))
            {
                return Err(UninstallModError::InstalledRevisionMismatch);
            }
        }

        let mut prepared_changes = Vec::with_capacity(uninstall_entries.len());
        for entry in uninstall_entries {
            let expected = entry
                .installed_file
                .as_ref()
                .ok_or(UninstallModError::MissingInstalledFileSummary)?;
            let current = self
                .game_files
                .read_game_file(&entry.target_path)
                .map_err(|_| UninstallModError::TargetStateMismatch)?
                .ok_or(UninstallModError::TargetStateMismatch)?;

            if &installed_file_summary(&current) != expected {
                return Err(UninstallModError::TargetStateMismatch);
            }

            let backup_bytes = match &entry.backup_ref {
                Some(backup_ref) => Some(
                    self.backup_store
                        .read_backup(backup_ref)
                        .map_err(|_| UninstallModError::BackupUnavailable)?
                        .ok_or(UninstallModError::BackupUnavailable)?,
                ),
                None => None,
            };

            prepared_changes.push(PreparedUninstallChange {
                entry,
                current_bytes: current,
                backup_bytes,
            });
        }

        let mut removed_file_count = 0;
        let mut restored_file_count = 0;
        let mut applied_changes = Vec::with_capacity(prepared_changes.len());
        for change in &prepared_changes {
            if !self.target_still_matches(&change.entry.target_path, &change.current_bytes) {
                return Err(self.rollback_or_error(
                    &applied_changes,
                    UninstallModPhase::Revalidate,
                    UninstallModError::TargetStateMismatch,
                ));
            }

            if let Some(backup_bytes) = &change.backup_bytes {
                if self
                    .game_files
                    .write_game_file(&change.entry.target_path, backup_bytes)
                    .is_err()
                {
                    return Err(self.rollback_or_error(
                        &applied_changes,
                        UninstallModPhase::Restore,
                        UninstallModError::RestoreFailed,
                    ));
                }
                restored_file_count += 1;
            } else {
                if self
                    .game_files
                    .remove_game_file(&change.entry.target_path)
                    .is_err()
                {
                    return Err(self.rollback_or_error(
                        &applied_changes,
                        UninstallModPhase::Remove,
                        UninstallModError::RemoveFailed,
                    ));
                }
                removed_file_count += 1;
            }
            applied_changes.push(AppliedUninstallChange {
                target_path: change.entry.target_path.clone(),
                previous_bytes: change.current_bytes.clone(),
            });
        }

        let completed_at = current_manifest_timestamp();
        let mut updated_manifest = InstallManifest::completed_with_metadata(
            request.profile_id,
            kept_entries,
            backend.or_else(|| Some(INSTALL_PLAN_MANIFEST_BACKEND.to_owned())),
            created_at.or(Some(completed_at.clone())),
            Some(completed_at),
            None,
        );
        updated_manifest.manifest_id = manifest_id;
        updated_manifest.schema_version = schema_version;
        updated_manifest.schema_migration = schema_migration;
        updated_manifest.status = status;
        updated_manifest.replacement_bindings = kept_replacement_bindings;
        if self
            .manifest_repository
            .save_manifest(&updated_manifest)
            .is_err()
        {
            return Err(self.rollback_or_error(
                &applied_changes,
                UninstallModPhase::ManifestSave,
                UninstallModError::ManifestSaveFailed,
            ));
        }

        for change in &prepared_changes {
            if let Some(backup_ref) = &change.entry.backup_ref {
                let _ = self.backup_store.remove_backup(backup_ref);
            }
        }

        Ok(UninstallModResult {
            manifest: updated_manifest,
            removed_file_count,
            restored_file_count,
        })
    }

    fn target_still_matches(&self, target_path: &InstallTargetPath, expected_bytes: &[u8]) -> bool {
        self.game_files
            .read_game_file(target_path)
            .ok()
            .flatten()
            .as_deref()
            == Some(expected_bytes)
    }

    fn rollback_or_error(
        &self,
        applied_changes: &[AppliedUninstallChange],
        failed_phase: UninstallModPhase,
        fallback: UninstallModError,
    ) -> UninstallModError {
        match self.rollback_uninstall(applied_changes) {
            Ok(()) => fallback,
            Err(()) => UninstallModError::RollbackFailed { failed_phase },
        }
    }

    fn rollback_uninstall(&self, applied_changes: &[AppliedUninstallChange]) -> Result<(), ()> {
        for change in applied_changes.iter().rev() {
            self.game_files
                .write_game_file(&change.target_path, &change.previous_bytes)
                .map_err(|_| ())?;
        }
        Ok(())
    }
}

fn manifest_backup_refs_by_target(
    existing_manifest: Option<&InstallManifest>,
) -> HashMap<InstallTargetPath, Option<String>> {
    let mut backup_refs = HashMap::new();
    if let Some(manifest) = existing_manifest {
        for entry in &manifest.entries {
            backup_refs
                .entry(entry.target_path.clone())
                .or_insert_with(|| entry.backup_ref.clone());
        }
    }

    backup_refs
}

fn merge_install_manifest(
    profile_id: ProfileId,
    existing_manifest: Option<InstallManifest>,
    applied_entries: Vec<InstallManifestEntry>,
    applied_replacement_bindings: Vec<ReplacementBindingSnapshot>,
    plan_hash: String,
) -> InstallManifest {
    let (
        mut entries,
        mut replacement_bindings,
        created_at,
        status,
        manifest_id,
        schema_version,
        schema_migration,
    ) = existing_manifest
        .map(|manifest| {
            (
                manifest.entries,
                manifest.replacement_bindings,
                manifest.created_at,
                manifest.status,
                Some(manifest.manifest_id),
                Some(manifest.schema_version),
                manifest.schema_migration,
            )
        })
        .unwrap_or_default();

    let touched_mods = applied_entries
        .iter()
        .map(|entry| entry.mod_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    entries.retain(|entry| {
        !applied_entries
            .iter()
            .any(|applied_entry| applied_entry.target_path == entry.target_path)
    });
    entries.extend(applied_entries);
    replacement_bindings.retain(|snapshot| !touched_mods.contains(snapshot.mod_id()));
    replacement_bindings.extend(applied_replacement_bindings);

    let completed_at = current_manifest_timestamp();
    let mut manifest = InstallManifest::completed_with_metadata(
        profile_id,
        entries,
        Some(INSTALL_PLAN_MANIFEST_BACKEND.to_owned()),
        created_at.or(Some(completed_at.clone())),
        Some(completed_at),
        Some(plan_hash),
    );
    if let Some(manifest_id) = manifest_id {
        manifest.manifest_id = manifest_id;
    }
    if manifest
        .entries
        .iter()
        .any(|entry| entry.revision_id.is_some())
    {
        manifest.schema_version = INSTALL_MANIFEST_SCHEMA_VERSION_V2;
    } else if let Some(schema_version) = schema_version {
        manifest.schema_version = schema_version;
    }
    manifest.schema_migration = schema_migration;
    manifest.status = status;
    manifest.replacement_bindings = replacement_bindings;
    manifest
}

pub(crate) fn uninstall_manifest_snapshot_digest(
    manifest: &InstallManifest,
    mod_id: &ModId,
) -> String {
    let mut entries = manifest
        .entries
        .iter()
        .filter(|entry| entry.mod_id == *mod_id)
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left.target_path
            .as_str()
            .cmp(right.target_path.as_str())
            .then_with(|| left.package_file_id.cmp(&right.package_file_id))
            .then_with(|| left.layer.priority.cmp(&right.layer.priority))
            .then_with(|| left.layer.name.cmp(&right.layer.name))
    });

    let mut hasher = Sha256::new();
    hasher.update(b"hmm-uninstall-manifest-snapshot-v1");
    update_hash_str(&mut hasher, manifest.profile_id.as_str());
    hasher.update(manifest.schema_version.to_be_bytes());
    update_hash_str(&mut hasher, install_manifest_status_code(manifest.status));
    update_hash_str(&mut hasher, mod_id.as_str());
    hasher.update((entries.len() as u64).to_be_bytes());
    for entry in entries {
        update_hash_str(&mut hasher, entry.target_path.as_str());
        update_hash_str(&mut hasher, entry.mod_id.as_str());
        update_optional_hash_str(
            &mut hasher,
            entry.revision_id.as_ref().map(ModRevisionId::as_str),
        );
        update_hash_str(&mut hasher, entry.package_file_id.as_str());
        update_hash_str(&mut hasher, &entry.layer.name);
        hasher.update(entry.layer.priority.to_be_bytes());
        update_optional_hash_str(&mut hasher, entry.backup_ref.as_deref());
        match &entry.installed_file {
            Some(summary) => {
                hasher.update([1]);
                hasher.update(summary.size_bytes.to_be_bytes());
                update_hash_str(&mut hasher, &summary.sha256);
            }
            None => hasher.update([0]),
        }
    }
    let bindings = manifest
        .replacement_bindings
        .iter()
        .filter(|snapshot| snapshot.mod_id() == mod_id)
        .cloned()
        .collect::<Vec<_>>();
    hash_replacement_snapshots(&mut hasher, &bindings);

    format!("sha256:{}", digest_to_hex(&hasher.finalize()))
}

pub(crate) fn install_manifest_status_code(status: InstallManifestStatus) -> &'static str {
    match status {
        InstallManifestStatus::Planned => "planned",
        InstallManifestStatus::Committing => "committing",
        InstallManifestStatus::Completed => "completed",
        InstallManifestStatus::RollbackRequired => "rollback_required",
        InstallManifestStatus::RolledBack => "rolled_back",
        InstallManifestStatus::RepairRequired => "repair_required",
    }
}

fn update_optional_hash_str(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            update_hash_str(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn install_plan_hash(plan: &InstallPlan) -> String {
    let mut actions = plan.actions.iter().collect::<Vec<_>>();
    actions.sort_by(|left, right| {
        left.target_path
            .cmp(&right.target_path)
            .then_with(|| left.provider.mod_id.cmp(&right.provider.mod_id))
            .then_with(|| {
                left.provider
                    .package_file_id
                    .cmp(&right.provider.package_file_id)
            })
            .then_with(|| {
                left.provider
                    .layer
                    .priority
                    .cmp(&right.provider.layer.priority)
            })
            .then_with(|| left.provider.layer.name.cmp(&right.provider.layer.name))
    });

    let mut hasher = Sha256::new();
    hasher.update(b"hmm-install-plan-v1");
    hasher.update((actions.len() as u64).to_be_bytes());
    for action in actions {
        update_hash_str(&mut hasher, action.target_path.as_str());
        update_hash_str(&mut hasher, action.provider.mod_id.as_str());
        update_hash_str(&mut hasher, action.provider.package_file_id.as_str());
        update_hash_str(&mut hasher, &action.provider.layer.name);
        hasher.update(action.provider.layer.priority.to_be_bytes());
    }
    hash_replacement_snapshots(&mut hasher, &plan.replacement_bindings);

    let digest = hasher.finalize();
    format!("sha256:{}", digest_to_hex(&digest))
}

fn hash_replacement_snapshots(hasher: &mut Sha256, snapshots: &[ReplacementBindingSnapshot]) {
    let mut snapshots = snapshots.iter().collect::<Vec<_>>();
    snapshots.sort_by(|left, right| left.binding_id().cmp(right.binding_id()));
    hasher.update((snapshots.len() as u64).to_be_bytes());
    for snapshot in snapshots {
        update_hash_str(hasher, snapshot.binding_id().as_str());
        update_hash_str(hasher, snapshot.mod_id().as_str());
        update_hash_str(hasher, snapshot.profile_id().as_str());
        match snapshot.revision_id() {
            Some(revision_id) => {
                hasher.update([1]);
                update_hash_str(hasher, revision_id.as_str());
            }
            None => hasher.update([0]),
        }
        update_hash_str(hasher, snapshot.binding().source_id().as_str());
        update_hash_str(hasher, snapshot.binding().target_id().as_str());
        hasher.update(snapshot.binding().created_at_unix_millis().to_be_bytes());
        update_hash_str(hasher, snapshot.source_internal_id());
        update_hash_str(hasher, snapshot.target_internal_id());
        update_hash_str(hasher, snapshot.source_path_family());
        update_hash_str(hasher, snapshot.target_path_family());
        update_hash_str(hasher, snapshot.retarget_kind().as_str());
        match snapshot.adapter_facts() {
            Some(facts) => {
                hasher.update([1]);
                hasher.update(b"hmm-replacement-adapter-facts-v1");
                hasher.update(facts.schema_version().to_be_bytes());
                update_hash_str(hasher, facts.adapter_id());
                update_hash_str(hasher, facts.strategy_id());
                hasher.update(facts.strategy_version().to_be_bytes());
                update_hash_str(hasher, facts.source_closure_sha256());
                update_hash_str(hasher, facts.part_set_sha256());
                update_hash_str(hasher, facts.transform_set_sha256());
                hasher.update(facts.part_count().to_be_bytes());
                hasher.update(facts.file_count().to_be_bytes());
                hasher.update((facts.transformer_identities().len() as u64).to_be_bytes());
                for identity in facts.transformer_identities() {
                    update_hash_str(hasher, identity.transformer_id());
                    hasher.update(identity.transformer_version().to_be_bytes());
                }
            }
            None => hasher.update([0]),
        }
    }
}

fn update_hash_str(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn current_manifest_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("unix:{}", duration.as_secs()))
        .unwrap_or_else(|_| "unix:0".to_owned())
}

fn installed_file_summary(bytes: &[u8]) -> InstalledFileSummary {
    InstalledFileSummary {
        size_bytes: bytes.len() as u64,
        sha256: sha256_hex(bytes),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest_to_hex(&digest)
}

fn digest_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl InstallPlanningService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_imported_mod_sources(
        result_repository: Arc<dyn ModImportResultRepository>,
        sandbox_locator: Arc<dyn ModImportSandboxLocator>,
        file_scanner: Arc<dyn ModPackageInstallFileScanner>,
        game_adapters: Vec<Arc<dyn GameAdapter>>,
    ) -> Self {
        Self {
            imported_mod_sources: Some(ImportedModInstallPlanSources {
                result_repository,
                sandbox_locator,
                file_scanner,
                game_adapters,
            }),
        }
    }

    pub fn build_plan(
        &self,
        request: BuildInstallPlanRequest,
    ) -> Result<InstallPlan, InstallPlanningError> {
        let mut providers = Vec::with_capacity(request.files.len());

        for file in request.files {
            let target_path =
                InstallTargetPath::parse(file.target_path, request.allowed_target_roots.iter())
                    .map_err(|source| InstallPlanningError::InvalidTargetPath {
                        package_file_id: file.package_file_id.clone(),
                        source,
                    })?;

            providers.push(InstallFileProvider::new(
                file.mod_id,
                file.package_file_id,
                target_path,
                file.layer,
            ));
        }

        Ok(InstallPlan::from_providers(providers))
    }

    pub fn build_plan_from_imported_mod(
        &self,
        request: BuildImportedModInstallPlanRequest,
    ) -> Result<InstallPlan, InstallPlanningError> {
        let sources = self
            .imported_mod_sources
            .as_ref()
            .ok_or(InstallPlanningError::ImportedModSourcesUnavailable)?;
        let analysis = sources
            .result_repository
            .get_analysis(request.mod_id.as_str())
            .map_err(|_| InstallPlanningError::ImportedModAnalysisUnavailable)?
            .ok_or_else(|| InstallPlanningError::ImportedModNotFound {
                mod_id: request.mod_id.clone(),
            })?;
        let sandbox_root = sources
            .sandbox_locator
            .sandbox_root_for_package(&analysis.package_id)
            .map_err(|_| InstallPlanningError::ImportedModSandboxUnavailable)?;
        self.build_plan_from_imported_package(
            sources,
            &request.game_id,
            &request.mod_id,
            &analysis.package_id,
            &request.layer,
            sandbox_root,
        )
    }

    pub(crate) fn build_plan_from_imported_revision(
        &self,
        game_id: &GameId,
        mod_id: &ModId,
        package_id: &str,
        layer: &FileLayer,
    ) -> Result<InstallPlan, InstallPlanningError> {
        let sources = self
            .imported_mod_sources
            .as_ref()
            .ok_or(InstallPlanningError::ImportedModSourcesUnavailable)?;
        let sandbox_root = sources
            .sandbox_locator
            .sandbox_root_for_package(package_id)
            .map_err(|_| InstallPlanningError::ImportedModSandboxUnavailable)?;
        self.build_plan_from_imported_package(
            sources,
            game_id,
            mod_id,
            package_id,
            layer,
            sandbox_root,
        )
    }

    pub(crate) fn build_plan_from_imported_revision_id(
        &self,
        game_id: &GameId,
        mod_id: &ModId,
        revision_id: &ModRevisionId,
        layer: &FileLayer,
    ) -> Result<InstallPlan, InstallPlanningError> {
        let sources = self
            .imported_mod_sources
            .as_ref()
            .ok_or(InstallPlanningError::ImportedModSourcesUnavailable)?;
        let revision = sources
            .result_repository
            .get_revision(revision_id)
            .map_err(|_| InstallPlanningError::ImportedModAnalysisUnavailable)?
            .filter(|revision| revision.mod_id == *mod_id)
            .ok_or_else(|| InstallPlanningError::ImportedModNotFound {
                mod_id: mod_id.clone(),
            })?;
        self.build_plan_from_imported_revision(game_id, mod_id, &revision.package_id, layer)
    }

    fn build_plan_from_imported_package(
        &self,
        sources: &ImportedModInstallPlanSources,
        game_id: &GameId,
        mod_id: &ModId,
        package_id: &str,
        layer: &FileLayer,
        sandbox_root: std::path::PathBuf,
    ) -> Result<InstallPlan, InstallPlanningError> {
        let adapter = sources
            .game_adapters
            .iter()
            .find(|adapter| adapter.game_id() == *game_id)
            .ok_or_else(|| InstallPlanningError::GameAdapterNotFound {
                game_id: game_id.clone(),
            })?;
        let files = sources
            .file_scanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id,
                sandbox_root: &sandbox_root,
            })
            .map_err(|_| InstallPlanningError::ImportedModFileScanUnavailable)?;
        let allowed_target_roots = adapter.allowed_install_roots();

        self.build_plan(BuildInstallPlanRequest {
            allowed_target_roots: allowed_target_roots.clone(),
            files: files
                .into_iter()
                .filter(|file| is_installable_target_path(&file.target_path, &allowed_target_roots))
                .map(|file| InstallPlanFile {
                    mod_id: mod_id.clone(),
                    package_file_id: PackageFileId::new(file.package_file_id),
                    target_path: file.target_path,
                    layer: layer.clone(),
                })
                .collect(),
        })
    }
}

fn is_installable_target_path(target_path: &str, allowed_target_roots: &[String]) -> bool {
    InstallTargetPath::parse(target_path, allowed_target_roots).is_ok()
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod install_tests;
