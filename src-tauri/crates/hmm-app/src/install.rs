use hmm_core::{
    FileLayer, GameId, InstallAction, InstallFileProvider, InstallManifest, InstallManifestEntry,
    InstallPlan, InstallRecoveryRecord, InstallRecoveryRecordEntry, InstallRecoveryRecordStatus,
    InstallTargetPath, InstallTargetPathError, InstalledFileSummary, ModId, PackageFileId,
    ProfileId,
};
use hmm_ports::{
    GameAdapter, InstallBackupStore, InstallGameFileSystem, InstallManifestRepository,
    InstallRecoveryRecordRepository, InstallSourceFileReader, ModImportResultRepository,
    ModImportSandboxLocator, ModPackageInstallFileScanRequest, ModPackageInstallFileScanner,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
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
    pub profile_id: ProfileId,
    pub plan: InstallPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallCommitResult {
    pub manifest: InstallManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallModRequest {
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
    #[error("install plan has blocking conflicts")]
    PlanHasBlockingConflicts,
    #[error("install commit failed during {phase:?}")]
    Failed { phase: InstallCommitPhase },
    #[error("install commit failed during {failed_phase:?}; rollback succeeded")]
    RollbackSucceeded { failed_phase: InstallCommitPhase },
    #[error("install commit failed during {failed_phase:?}; rollback failed")]
    RollbackFailed { failed_phase: InstallCommitPhase },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum UninstallModError {
    #[error("game instance is unavailable")]
    GameInstanceUnavailable,
    #[error("install manifest is unavailable")]
    ManifestUnavailable,
    #[error("mod is not installed in this profile")]
    ModNotInstalled,
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
}

#[derive(Clone)]
pub struct UninstallModService {
    game_files: Arc<dyn InstallGameFileSystem>,
    backup_store: Arc<dyn InstallBackupStore>,
    manifest_repository: Arc<dyn InstallManifestRepository>,
}

#[derive(Clone)]
struct AppliedInstallChange {
    target_path: InstallTargetPath,
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
        }
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
        }
    }

    pub fn commit_plan(
        &self,
        request: CommitInstallPlanRequest,
    ) -> Result<InstallCommitResult, InstallCommitError> {
        let CommitInstallPlanRequest { profile_id, plan } = request;

        if plan.has_blocking_conflicts() {
            return Err(InstallCommitError::PlanHasBlockingConflicts);
        }

        let existing_manifest = self
            .manifest_repository
            .load_manifest(&profile_id)
            .map_err(|_| InstallCommitError::Failed {
                phase: InstallCommitPhase::ManifestRead,
            })?;
        let existing_backup_refs = manifest_backup_refs_by_target(existing_manifest.as_ref());
        let mut recovery_records = self
            .start_recovery_records(&profile_id, &plan.actions, &existing_backup_refs)
            .map_err(|_| InstallCommitError::Failed {
                phase: InstallCommitPhase::Manifest,
            })?;
        let mut applied_changes = Vec::new();

        for action in plan.actions {
            let source_bytes = match self
                .source_files
                .read_source_file(&action.provider.package_file_id)
            {
                Ok(bytes) => bytes,
                Err(_) => {
                    let error =
                        self.fail_or_rollback(&applied_changes, InstallCommitPhase::SourceRead);
                    Self::finish_recovery_records_after_failure(&mut recovery_records, &error);
                    return Err(error);
                }
            };
            let previous_bytes = match self.game_files.read_game_file(&action.target_path) {
                Ok(bytes) => bytes,
                Err(_) => {
                    let error =
                        self.fail_or_rollback(&applied_changes, InstallCommitPhase::TargetRead);
                    Self::finish_recovery_records_after_failure(&mut recovery_records, &error);
                    return Err(error);
                }
            };
            let backup_ref = if let Some(bytes) = previous_bytes.as_deref() {
                match self.backup_store.store_backup(&action.target_path, bytes) {
                    Ok(backup_ref) => Some(backup_ref),
                    Err(_) => {
                        let error =
                            self.fail_or_rollback(&applied_changes, InstallCommitPhase::Backup);
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
                package_file_id: action.provider.package_file_id.clone(),
                layer: action.provider.layer.clone(),
                backup_ref: manifest_backup_ref,
                installed_file: Some(installed_file_summary(&source_bytes)),
            };

            if let Some(records) = recovery_records.as_mut() {
                if records
                    .update_entry_for_rollback(&entry, backup_ref.clone())
                    .and_then(|_| records.ensure_committing())
                    .is_err()
                {
                    let error = self.fail_or_rollback_with_pending_backup(
                        &applied_changes,
                        backup_ref.as_deref(),
                        InstallCommitPhase::Manifest,
                    );
                    Self::finish_recovery_records_after_failure(&mut recovery_records, &error);
                    return Err(error);
                }
            }

            if self
                .game_files
                .write_game_file(&action.target_path, &source_bytes)
                .is_err()
            {
                let error = self.fail_or_rollback_with_pending_backup(
                    &applied_changes,
                    backup_ref.as_deref(),
                    InstallCommitPhase::Write,
                );
                Self::finish_recovery_records_after_failure(&mut recovery_records, &error);
                return Err(error);
            }

            applied_changes.push(AppliedInstallChange {
                target_path: action.target_path.clone(),
                previous_bytes,
                pending_backup_ref: backup_ref,
                entry,
            });
        }

        let manifest = merge_install_manifest(
            profile_id,
            existing_manifest,
            applied_changes
                .iter()
                .map(|change| change.entry.clone())
                .collect(),
        );

        if self.manifest_repository.save_manifest(&manifest).is_err() {
            let error = self.fail_or_rollback(&applied_changes, InstallCommitPhase::Manifest);
            Self::finish_recovery_records_after_failure(&mut recovery_records, &error);
            return Err(error);
        }
        if let Some(records) = recovery_records.as_mut() {
            records.update_entries_for_completed_manifest(&manifest.entries);
            records.mark_completed_best_effort();
        }
        self.remove_obsolete_pending_backups(&applied_changes);

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
        failed_phase: InstallCommitPhase,
    ) -> InstallCommitError {
        self.fail_or_rollback_with_pending_backup(applied_changes, None, failed_phase)
    }

    fn fail_or_rollback_with_pending_backup(
        &self,
        applied_changes: &[AppliedInstallChange],
        pending_backup_ref: Option<&str>,
        failed_phase: InstallCommitPhase,
    ) -> InstallCommitError {
        if applied_changes.is_empty() && pending_backup_ref.is_none() {
            return InstallCommitError::Failed {
                phase: failed_phase,
            };
        }

        let rollback_result = self.rollback(applied_changes);
        if let Some(backup_ref) = pending_backup_ref {
            let _ = self.backup_store.remove_backup(backup_ref);
        }

        if rollback_result.is_ok() {
            InstallCommitError::RollbackSucceeded { failed_phase }
        } else {
            InstallCommitError::RollbackFailed { failed_phase }
        }
    }

    fn rollback(&self, applied_changes: &[AppliedInstallChange]) -> anyhow::Result<()> {
        let mut rollback_error = None;

        for change in applied_changes.iter().rev() {
            let restore_result = if let Some(previous_bytes) = &change.previous_bytes {
                self.game_files
                    .write_game_file(&change.target_path, previous_bytes)
            } else {
                self.game_files.remove_game_file(&change.target_path)
            };

            if let Err(error) = restore_result {
                rollback_error.get_or_insert(error);
            }
        }

        for change in applied_changes.iter().rev() {
            if let Some(backup_ref) = &change.pending_backup_ref {
                let _ = self.backup_store.remove_backup(backup_ref);
            }
        }

        match rollback_error {
            Some(error) => Err(error),
            None => Ok(()),
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

    fn mark_completed_best_effort(&mut self) {
        if self
            .transition_all_to(InstallRecoveryRecordStatus::Completed)
            .is_err()
        {
            self.remove_all_best_effort();
        }
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
        }
    }

    pub fn uninstall_mod(
        &self,
        request: UninstallModRequest,
    ) -> Result<UninstallModResult, UninstallModError> {
        let manifest = self
            .manifest_repository
            .load_manifest(&request.profile_id)
            .map_err(|_| UninstallModError::ManifestUnavailable)?
            .ok_or(UninstallModError::ModNotInstalled)?;
        let InstallManifest {
            backend,
            created_at,
            entries,
            ..
        } = manifest;

        let (uninstall_entries, kept_entries): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .partition(|entry| entry.mod_id == request.mod_id);

        if uninstall_entries.is_empty() {
            return Err(UninstallModError::ModNotInstalled);
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
        let updated_manifest = InstallManifest::completed_with_metadata(
            request.profile_id,
            kept_entries,
            backend.or_else(|| Some(INSTALL_PLAN_MANIFEST_BACKEND.to_owned())),
            created_at.or(Some(completed_at.clone())),
            Some(completed_at),
            None,
        );
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
) -> InstallManifest {
    let (mut entries, created_at) = existing_manifest
        .map(|manifest| (manifest.entries, manifest.created_at))
        .unwrap_or_default();

    entries.retain(|entry| {
        !applied_entries
            .iter()
            .any(|applied_entry| applied_entry.target_path == entry.target_path)
    });
    entries.extend(applied_entries);

    let completed_at = current_manifest_timestamp();
    InstallManifest::completed_with_metadata(
        profile_id,
        entries,
        Some(INSTALL_PLAN_MANIFEST_BACKEND.to_owned()),
        created_at.or(Some(completed_at.clone())),
        Some(completed_at),
        None,
    )
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
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
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
        let adapter = sources
            .game_adapters
            .iter()
            .find(|adapter| adapter.game_id() == request.game_id)
            .ok_or_else(|| InstallPlanningError::GameAdapterNotFound {
                game_id: request.game_id.clone(),
            })?;
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
        let files = sources
            .file_scanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id: &analysis.package_id,
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
                    mod_id: request.mod_id.clone(),
                    package_file_id: PackageFileId::new(file.package_file_id),
                    target_path: file.target_path,
                    layer: request.layer.clone(),
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
