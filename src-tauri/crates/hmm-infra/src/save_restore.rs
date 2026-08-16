use crate::controlled_fs::{
    create_new_child_directory, create_new_regular_file,
    is_link_or_reparse as is_cap_link_or_reparse, open_child_directory_nofollow,
    open_existing_directory_nofollow, open_or_create_child_directory, open_regular_file_nofollow,
    remove_child_tree_nofollow,
};
use crate::save_backup::managed_backup_directory_for_summary;
use crate::save_path::{
    normalize_save_relative_path, record_parent_directories, MAX_SAVE_DIRECTORY_COUNT,
};
use cap_std::fs::{Dir, File};
use hmm_core::{SaveBackupManifest, SaveBackupManifestFile, SaveBackupSummary};
use hmm_ports::{
    PreparedSaveRestore, SaveRestoreCommitError, SaveRestoreCommitRequest, SaveRestoreFileSystem,
    SaveRestoreFinalizeError, SaveRestoreFinalizeRequest, SaveRestorePrepareError,
    SaveRestorePrepareRequest, SaveRestoreSourceError, SaveRestoreSourceValidator,
    ValidatedSaveRestoreSource,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use zip::ZipArchive;

const MAX_MANIFEST_BYTES: u64 = 2 * 1024 * 1024;
const MAX_FILE_COUNT: usize = 200;
const MAX_SINGLE_FILE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;

struct OpenSaveRestoreSource {
    manifest_bytes: Vec<u8>,
    manifest: SaveBackupManifest,
    archive: File,
}

pub struct FileSystemSaveRestoreSourceValidator {
    app_data_dir: PathBuf,
}

impl FileSystemSaveRestoreSourceValidator {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self { app_data_dir }
    }
}

impl SaveRestoreSourceValidator for FileSystemSaveRestoreSourceValidator {
    fn validate_source(
        &self,
        summary: &SaveBackupSummary,
    ) -> Result<ValidatedSaveRestoreSource, SaveRestoreSourceError> {
        let backup_dir = managed_backup_directory_for_summary(&self.app_data_dir, summary)
            .map_err(|_| SaveRestoreSourceError::BackupDirectoryUnavailable)?;
        let backup_dir = canonical_regular_directory(&backup_dir)
            .map_err(|_| SaveRestoreSourceError::BackupDirectoryUnavailable)?;
        let source = open_and_validate_source(&backup_dir, summary)?;
        let total_uncompressed_bytes =
            validate_archive_contents(source.archive, &source.manifest.files)?;

        Ok(ValidatedSaveRestoreSource {
            game_id: summary.game_id.clone(),
            profile_id: summary.profile_id.clone(),
            backup_id: summary.backup_id.clone(),
            evidence_digest: source_evidence_digest(summary, &source.manifest_bytes),
            file_count: source.manifest.files.len() as u32,
            total_uncompressed_bytes,
        })
    }
}

struct PreparedStage {
    transaction_id: String,
    backup_id: String,
    archive_sha256: String,
    target_root: PathBuf,
    target_parent_path: PathBuf,
    target_parent_dir: Dir,
    finalize_parent_dir: Option<Dir>,
    target_parent_identity: DirectoryIdentity,
    target_name: OsString,
    target_identity: DirectoryIdentity,
    staging_name: OsString,
    staging_identity: DirectoryIdentity,
    target_digest: String,
    staging_digest: String,
    file_count: u32,
}

struct FinalizeStage {
    parent_dir: Dir,
    children: [FinalizeChild; 4],
}

struct FinalizeChild {
    name: OsString,
    expected_identity: Option<DirectoryIdentity>,
    expected_digest: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectoryIdentity {
    #[cfg(windows)]
    Windows {
        volume_serial_number: u32,
        file_index: u64,
    },
    #[cfg(unix)]
    Unix { dev: u64, ino: u64 },
    #[cfg(not(any(windows, unix)))]
    Unsupported,
}

trait SaveRestoreDirectoryRenamer: Send + Sync {
    fn rename(&self, parent: &Dir, from: &OsStr, to: &OsStr) -> std::io::Result<()>;
}

struct StdSaveRestoreDirectoryRenamer;

impl SaveRestoreDirectoryRenamer for StdSaveRestoreDirectoryRenamer {
    fn rename(&self, parent: &Dir, from: &OsStr, to: &OsStr) -> std::io::Result<()> {
        parent.rename(from, parent, to)
    }
}

pub struct FileSystemSaveRestoreFileSystem {
    app_data_dir: PathBuf,
    prepared: Mutex<HashMap<String, PreparedStage>>,
    finalization: Mutex<HashMap<String, FinalizeStage>>,
    finalized: Mutex<BTreeSet<String>>,
    renamer: Arc<dyn SaveRestoreDirectoryRenamer>,
    #[cfg(test)]
    post_rename_parent_binding_override: Mutex<Option<bool>>,
}

impl FileSystemSaveRestoreFileSystem {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            app_data_dir,
            prepared: Mutex::new(HashMap::new()),
            finalization: Mutex::new(HashMap::new()),
            finalized: Mutex::new(BTreeSet::new()),
            renamer: Arc::new(StdSaveRestoreDirectoryRenamer),
            #[cfg(test)]
            post_rename_parent_binding_override: Mutex::new(None),
        }
    }

    #[cfg(test)]
    fn with_renamer(app_data_dir: PathBuf, renamer: Arc<dyn SaveRestoreDirectoryRenamer>) -> Self {
        Self {
            app_data_dir,
            prepared: Mutex::new(HashMap::new()),
            finalization: Mutex::new(HashMap::new()),
            finalized: Mutex::new(BTreeSet::new()),
            renamer,
            post_rename_parent_binding_override: Mutex::new(None),
        }
    }

    #[cfg(test)]
    fn with_post_rename_parent_binding_override(
        app_data_dir: PathBuf,
        override_result: bool,
    ) -> Self {
        Self {
            app_data_dir,
            prepared: Mutex::new(HashMap::new()),
            finalization: Mutex::new(HashMap::new()),
            finalized: Mutex::new(BTreeSet::new()),
            renamer: Arc::new(StdSaveRestoreDirectoryRenamer),
            post_rename_parent_binding_override: Mutex::new(Some(override_result)),
        }
    }
}

impl SaveRestoreFileSystem for FileSystemSaveRestoreFileSystem {
    fn prepare_restore(
        &self,
        request: SaveRestorePrepareRequest,
    ) -> Result<PreparedSaveRestore, SaveRestorePrepareError> {
        let target_root = target_directory(&request.target_directory)
            .map_err(|_| SaveRestorePrepareError::TargetUnavailable)?;
        let backup_dir = managed_backup_directory_for_summary(&self.app_data_dir, &request.summary)
            .map_err(|_| {
                SaveRestorePrepareError::Source(SaveRestoreSourceError::BackupDirectoryUnavailable)
            })?;
        let backup_dir = canonical_regular_directory(&backup_dir).map_err(|_| {
            SaveRestorePrepareError::Source(SaveRestoreSourceError::BackupDirectoryUnavailable)
        })?;
        if paths_contain(&target_root, &backup_dir) || paths_contain(&backup_dir, &target_root) {
            return Err(SaveRestorePrepareError::TargetUnsafe);
        }
        let transaction_fragment = safe_transaction_fragment(&request.transaction_id)
            .ok_or(SaveRestorePrepareError::TargetUnsafe)?;
        let target_parent = target_root
            .parent()
            .ok_or(SaveRestorePrepareError::StagingUnavailable)?
            .to_path_buf();
        let target_name = target_root
            .file_name()
            .ok_or(SaveRestorePrepareError::TargetUnavailable)?
            .to_os_string();
        let target_parent_dir = open_existing_directory_nofollow(
            &target_parent,
            "save restore target parent directory",
        )
        .map_err(|_| SaveRestorePrepareError::StagingUnavailable)?;
        let target_parent_identity = directory_identity(&target_parent_dir)
            .map_err(|_| SaveRestorePrepareError::StagingUnavailable)?;
        let target_dir = open_child_directory_nofollow(
            &target_parent_dir,
            &target_name,
            "save restore target directory",
        )
        .map_err(|_| SaveRestorePrepareError::TargetUnavailable)?;
        let target_identity = directory_identity(&target_dir)
            .map_err(|_| SaveRestorePrepareError::TargetUnavailable)?;
        let target_digest = directory_content_digest_from_dir(&target_dir)
            .map_err(|_| SaveRestorePrepareError::TargetUnavailable)?;
        drop(target_dir);

        let staging_name =
            OsString::from(format!(".hmm-save-restore-{transaction_fragment}-staging"));
        let staging_dir = create_new_child_directory(
            &target_parent_dir,
            &staging_name,
            "save restore staging directory",
        )
        .map_err(|_| SaveRestorePrepareError::StagingUnavailable)?;
        let prepared = (|| {
            let source = open_and_validate_source(&backup_dir, &request.summary)
                .map_err(SaveRestorePrepareError::Source)?;
            let total = extract_and_validate(source.archive, &source.manifest.files, &staging_dir)
                .map_err(SaveRestorePrepareError::Source)?;
            let staging_digest = directory_content_digest_from_dir(&staging_dir)
                .map_err(|_| SaveRestorePrepareError::StagingUnavailable)?;
            let staging_identity = directory_identity(&staging_dir)
                .map_err(|_| SaveRestorePrepareError::StagingUnavailable)?;
            let finalize_parent_dir = target_parent_dir
                .try_clone()
                .map_err(|_| SaveRestorePrepareError::StagingUnavailable)?;
            let prepared_id = format!("prepared-{}", Uuid::new_v4());
            Ok((
                prepared_id,
                source_evidence_digest(&request.summary, &source.manifest_bytes),
                source.manifest.files.len() as u32,
                total,
                staging_identity,
                staging_digest,
                finalize_parent_dir,
            ))
        })();
        drop(staging_dir);

        let (
            prepared_id,
            evidence_digest,
            file_count,
            total_uncompressed_bytes,
            staging_identity,
            staging_digest,
            finalize_parent_dir,
        ) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                let _ = remove_child_tree_nofollow(
                    &target_parent_dir,
                    &staging_name,
                    "save restore staging directory",
                );
                return Err(error);
            }
        };

        let stage = PreparedStage {
            transaction_id: request.transaction_id,
            backup_id: request.summary.backup_id,
            archive_sha256: request.summary.archive_sha256,
            target_root,
            target_parent_path: target_parent,
            target_parent_dir,
            finalize_parent_dir: Some(finalize_parent_dir),
            target_parent_identity,
            target_name,
            target_identity,
            staging_name,
            staging_identity,
            target_digest,
            staging_digest,
            file_count,
        };
        let mut prepared = match self.prepared.lock() {
            Ok(prepared) => prepared,
            Err(_) => {
                let _ = remove_child_tree_nofollow(
                    &stage.target_parent_dir,
                    &stage.staging_name,
                    "save restore staging directory",
                );
                return Err(SaveRestorePrepareError::StagingUnavailable);
            }
        };
        if prepared.contains_key(&prepared_id) {
            let _ = remove_child_tree_nofollow(
                &stage.target_parent_dir,
                &stage.staging_name,
                "save restore staging directory",
            );
            return Err(SaveRestorePrepareError::StagingUnavailable);
        }
        prepared.insert(prepared_id.clone(), stage);

        Ok(PreparedSaveRestore {
            prepared_id,
            evidence_digest,
            file_count,
            total_uncompressed_bytes,
        })
    }

    fn discard_prepared(&self, prepared_id: &str) {
        let stage = self
            .prepared
            .lock()
            .ok()
            .and_then(|mut prepared| prepared.remove(prepared_id));
        if let Some(stage) = stage {
            let _ = remove_child_tree_nofollow(
                &stage.target_parent_dir,
                &stage.staging_name,
                "save restore staging directory",
            );
        }
    }

    fn commit_restore(
        &self,
        request: SaveRestoreCommitRequest,
    ) -> Result<hmm_ports::SaveRestoreCommitResult, SaveRestoreCommitError> {
        let mut stage = self
            .prepared
            .lock()
            .map_err(|_| SaveRestoreCommitError::PreparedMissing)?
            .remove(&request.prepared_id)
            .ok_or(SaveRestoreCommitError::PreparedMissing)?;
        let discard_staging = || {
            let _ = remove_child_tree_nofollow(
                &stage.target_parent_dir,
                &stage.staging_name,
                "save restore staging directory",
            );
        };
        if request.transaction_id != stage.transaction_id
            || request.summary.backup_id != stage.backup_id
            || request.summary.archive_sha256 != stage.archive_sha256
        {
            discard_staging();
            return Err(SaveRestoreCommitError::PreparedMissing);
        }
        if let Err(error) = validate_prepared_stage(&stage, &request.target_directory) {
            discard_staging();
            return Err(error);
        }
        let Some(fragment) = safe_transaction_fragment(&request.transaction_id) else {
            discard_staging();
            return Err(SaveRestoreCommitError::CommitFailed);
        };
        let rollback_name = OsString::from(format!(".hmm-save-restore-{fragment}-rollback"));
        let failed_name = OsString::from(format!(".hmm-save-restore-{fragment}-failed"));
        let fallback_name = OsString::from(format!(".hmm-save-restore-{fragment}-fallback"));
        if !child_entry_is_absent(&stage.target_parent_dir, &rollback_name)
            || !child_entry_is_absent(&stage.target_parent_dir, &failed_name)
            || !child_entry_is_absent(&stage.target_parent_dir, &fallback_name)
        {
            discard_staging();
            return Err(SaveRestoreCommitError::RecoveryRequired);
        }

        if self
            .renamer
            .rename(&stage.target_parent_dir, &stage.target_name, &rollback_name)
            .is_err()
        {
            discard_staging();
            return Err(SaveRestoreCommitError::CommitFailed);
        }
        if !directory_child_matches(
            &stage.target_parent_dir,
            &rollback_name,
            stage.target_identity,
            &stage.target_digest,
        ) {
            self.retain_recovery_finalization(
                &request.transaction_id,
                &mut stage,
                &rollback_name,
                &failed_name,
                &fallback_name,
            )?;
            return Err(SaveRestoreCommitError::RecoveryRequired);
        }
        if !self.post_rename_parent_binding_matches(&stage) {
            if self
                .renamer
                .rename(&stage.target_parent_dir, &rollback_name, &stage.target_name)
                .is_ok()
                && directory_child_matches(
                    &stage.target_parent_dir,
                    &stage.target_name,
                    stage.target_identity,
                    &stage.target_digest,
                )
                && target_parent_binding_matches(&stage)
            {
                discard_staging();
                self.retain_finalization(
                    &request.transaction_id,
                    finalization_children(
                        rollback_name,
                        None,
                        failed_name,
                        None,
                        fallback_name,
                        None,
                        (stage.staging_name.clone(), None),
                    ),
                    &mut stage,
                )?;
                return Err(SaveRestoreCommitError::RolledBack);
            }
            self.retain_recovery_finalization(
                &request.transaction_id,
                &mut stage,
                &rollback_name,
                &failed_name,
                &fallback_name,
            )?;
            return Err(SaveRestoreCommitError::RecoveryRequired);
        }

        if self
            .renamer
            .rename(
                &stage.target_parent_dir,
                &stage.staging_name,
                &stage.target_name,
            )
            .is_err()
        {
            if self
                .renamer
                .rename(&stage.target_parent_dir, &rollback_name, &stage.target_name)
                .is_ok()
                && directory_child_matches(
                    &stage.target_parent_dir,
                    &stage.target_name,
                    stage.target_identity,
                    &stage.target_digest,
                )
                && target_parent_binding_matches(&stage)
            {
                discard_staging();
                self.retain_finalization(
                    &request.transaction_id,
                    finalization_children(
                        rollback_name,
                        None,
                        failed_name,
                        None,
                        fallback_name,
                        None,
                        (stage.staging_name.clone(), None),
                    ),
                    &mut stage,
                )?;
                return Err(SaveRestoreCommitError::RolledBack);
            }
            if self
                .restore_pre_restore_fallback(
                    &request,
                    &stage,
                    &fallback_name,
                    stage.target_digest.as_str(),
                )
                .is_ok()
            {
                discard_staging();
                self.retain_finalization(
                    &request.transaction_id,
                    finalization_children(
                        rollback_name,
                        Some((stage.target_identity, stage.target_digest.clone())),
                        failed_name,
                        None,
                        fallback_name,
                        None,
                        (stage.staging_name.clone(), None),
                    ),
                    &mut stage,
                )?;
                return Err(SaveRestoreCommitError::RolledBack);
            }
            self.retain_recovery_finalization(
                &request.transaction_id,
                &mut stage,
                &rollback_name,
                &failed_name,
                &fallback_name,
            )?;
            return Err(SaveRestoreCommitError::RecoveryRequired);
        }

        if !directory_child_matches(
            &stage.target_parent_dir,
            &stage.target_name,
            stage.staging_identity,
            &stage.staging_digest,
        ) || !target_parent_binding_matches(&stage)
        {
            if self
                .renamer
                .rename(&stage.target_parent_dir, &stage.target_name, &failed_name)
                .is_ok()
                && self
                    .renamer
                    .rename(&stage.target_parent_dir, &rollback_name, &stage.target_name)
                    .is_ok()
                && directory_child_matches(
                    &stage.target_parent_dir,
                    &stage.target_name,
                    stage.target_identity,
                    &stage.target_digest,
                )
                && target_parent_binding_matches(&stage)
            {
                self.retain_finalization(
                    &request.transaction_id,
                    finalization_children(
                        rollback_name,
                        None,
                        failed_name,
                        Some((stage.staging_identity, stage.staging_digest.clone())),
                        fallback_name,
                        None,
                        (stage.staging_name.clone(), None),
                    ),
                    &mut stage,
                )?;
                return Err(SaveRestoreCommitError::RolledBack);
            }
            if child_entry_is_absent(&stage.target_parent_dir, &stage.target_name)
                && self
                    .restore_pre_restore_fallback(
                        &request,
                        &stage,
                        &fallback_name,
                        stage.target_digest.as_str(),
                    )
                    .is_ok()
            {
                self.retain_finalization(
                    &request.transaction_id,
                    finalization_children(
                        rollback_name,
                        Some((stage.target_identity, stage.target_digest.clone())),
                        failed_name,
                        Some((stage.staging_identity, stage.staging_digest.clone())),
                        fallback_name,
                        None,
                        (stage.staging_name.clone(), None),
                    ),
                    &mut stage,
                )?;
                return Err(SaveRestoreCommitError::RolledBack);
            }
            self.retain_recovery_finalization(
                &request.transaction_id,
                &mut stage,
                &rollback_name,
                &failed_name,
                &fallback_name,
            )?;
            return Err(SaveRestoreCommitError::RecoveryRequired);
        }

        self.retain_finalization(
            &request.transaction_id,
            finalization_children(
                rollback_name,
                Some((stage.target_identity, stage.target_digest.clone())),
                failed_name,
                None,
                fallback_name,
                None,
                (stage.staging_name.clone(), None),
            ),
            &mut stage,
        )?;
        Ok(hmm_ports::SaveRestoreCommitResult {
            restored_file_count: stage.file_count,
            rollback_performed: false,
        })
    }

    fn finalize_restore(
        &self,
        request: SaveRestoreFinalizeRequest,
    ) -> Result<(), SaveRestoreFinalizeError> {
        let transaction_id = request.transaction_id;
        let stage = self
            .finalization
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&transaction_id);
        let Some(mut stage) = stage else {
            return if self
                .finalized
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .contains(&transaction_id)
            {
                Ok(())
            } else {
                Err(SaveRestoreFinalizeError::TargetUnavailable)
            };
        };

        if let Err(error) = remove_finalization_stage(&mut stage) {
            self.finalization
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(transaction_id, stage);
            return Err(error);
        }
        self.finalized
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(transaction_id);
        Ok(())
    }
}

impl FileSystemSaveRestoreFileSystem {
    fn post_rename_parent_binding_matches(&self, stage: &PreparedStage) -> bool {
        #[cfg(test)]
        if let Some(result) = self
            .post_rename_parent_binding_override
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            return result;
        }
        target_parent_binding_matches(stage)
    }

    fn retain_finalization(
        &self,
        transaction_id: &str,
        children: [FinalizeChild; 4],
        stage: &mut PreparedStage,
    ) -> Result<(), SaveRestoreCommitError> {
        let parent_dir = stage
            .finalize_parent_dir
            .take()
            .ok_or(SaveRestoreCommitError::RecoveryRequired)?;
        self.finalized
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(transaction_id);
        self.finalization
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                transaction_id.to_owned(),
                FinalizeStage {
                    parent_dir,
                    children,
                },
            );
        Ok(())
    }

    fn retain_recovery_finalization(
        &self,
        transaction_id: &str,
        stage: &mut PreparedStage,
        rollback_name: &OsStr,
        failed_name: &OsStr,
        fallback_name: &OsStr,
    ) -> Result<(), SaveRestoreCommitError> {
        let rollback = known_directory_child(
            &stage.target_parent_dir,
            rollback_name,
            stage.target_identity,
            &stage.target_digest,
        );
        let failed = known_directory_child(
            &stage.target_parent_dir,
            failed_name,
            stage.staging_identity,
            &stage.staging_digest,
        );
        let staging = known_directory_child(
            &stage.target_parent_dir,
            &stage.staging_name,
            stage.staging_identity,
            &stage.staging_digest,
        );
        self.retain_finalization(
            transaction_id,
            finalization_children(
                rollback_name.to_os_string(),
                rollback,
                failed_name.to_os_string(),
                failed,
                fallback_name.to_os_string(),
                None,
                (stage.staging_name.clone(), staging),
            ),
            stage,
        )
    }

    fn restore_pre_restore_fallback(
        &self,
        request: &SaveRestoreCommitRequest,
        stage: &PreparedStage,
        fallback_name: &OsStr,
        expected_digest: &str,
    ) -> Result<(), ()> {
        if !child_entry_is_absent(&stage.target_parent_dir, &stage.target_name)
            || !target_parent_binding_matches(stage)
        {
            return Err(());
        }
        let summary = request.pre_restore_summary.as_ref().ok_or(())?;
        if summary.game_id != request.summary.game_id
            || summary.profile_id != request.summary.profile_id
            || summary.trigger != hmm_core::SaveBackupTrigger::PreRestore
        {
            return Err(());
        }
        let fallback_dir = create_new_child_directory(
            &stage.target_parent_dir,
            fallback_name,
            "save restore fallback directory",
        )
        .map_err(|_| ())?;
        let prepared = extract_summary_into(&self.app_data_dir, summary, &fallback_dir)
            .map_err(|_| ())
            .and_then(|()| {
                let identity = directory_identity(&fallback_dir)?;
                let digest = directory_content_digest_from_dir(&fallback_dir)?;
                (digest == expected_digest).then_some(identity).ok_or(())
            });
        drop(fallback_dir);
        let fallback_identity = match prepared {
            Ok(identity) => identity,
            Err(()) => {
                let _ = remove_child_tree_nofollow(
                    &stage.target_parent_dir,
                    fallback_name,
                    "save restore fallback directory",
                );
                return Err(());
            }
        };

        let result = self
            .renamer
            .rename(&stage.target_parent_dir, fallback_name, &stage.target_name)
            .map_err(|_| ())
            .and_then(|()| {
                (directory_child_matches(
                    &stage.target_parent_dir,
                    &stage.target_name,
                    fallback_identity,
                    expected_digest,
                ) && target_parent_binding_matches(stage))
                .then_some(())
                .ok_or(())
            });
        if result.is_err() && !child_entry_is_absent(&stage.target_parent_dir, fallback_name) {
            let _ = remove_child_tree_nofollow(
                &stage.target_parent_dir,
                fallback_name,
                "save restore fallback directory",
            );
        }
        result
    }
}

fn extract_summary_into(
    app_data_dir: &Path,
    summary: &SaveBackupSummary,
    staging_root: &Dir,
) -> Result<(), SaveRestoreSourceError> {
    let backup_dir = managed_backup_directory_for_summary(app_data_dir, summary)
        .map_err(|_| SaveRestoreSourceError::BackupDirectoryUnavailable)?;
    let backup_dir = canonical_regular_directory(&backup_dir)
        .map_err(|_| SaveRestoreSourceError::BackupDirectoryUnavailable)?;
    let source = open_and_validate_source(&backup_dir, summary)?;
    extract_and_validate(source.archive, &source.manifest.files, staging_root)?;
    Ok(())
}

fn validate_prepared_stage(
    stage: &PreparedStage,
    target_selection: &hmm_core::ProfileDirectorySelection,
) -> Result<(), SaveRestoreCommitError> {
    let target = target_directory(target_selection)
        .map_err(|_| SaveRestoreCommitError::TargetUnavailable)?;
    if target != stage.target_root || !target_parent_binding_matches(stage) {
        return Err(SaveRestoreCommitError::TargetChanged);
    }
    if !directory_child_matches(
        &stage.target_parent_dir,
        &stage.target_name,
        stage.target_identity,
        &stage.target_digest,
    ) {
        return Err(SaveRestoreCommitError::TargetChanged);
    }
    if !directory_child_matches(
        &stage.target_parent_dir,
        &stage.staging_name,
        stage.staging_identity,
        &stage.staging_digest,
    ) {
        return Err(SaveRestoreCommitError::CommitFailed);
    }
    Ok(())
}

fn target_parent_binding_matches(stage: &PreparedStage) -> bool {
    open_existing_directory_nofollow(
        &stage.target_parent_path,
        "save restore target parent directory",
    )
    .ok()
    .and_then(|directory| directory_identity(&directory).ok())
        == Some(stage.target_parent_identity)
}

fn directory_child_matches(
    parent: &Dir,
    name: &OsStr,
    expected_identity: DirectoryIdentity,
    expected_digest: &str,
) -> bool {
    let Ok(directory) = open_child_directory_nofollow(parent, name, "save restore directory")
    else {
        return false;
    };
    directory_identity(&directory).ok() == Some(expected_identity)
        && directory_content_digest_from_dir(&directory)
            .ok()
            .as_deref()
            == Some(expected_digest)
}

fn child_entry_is_absent(parent: &Dir, name: &OsStr) -> bool {
    matches!(
        parent.symlink_metadata(name),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    )
}

fn remove_finalization_stage(stage: &mut FinalizeStage) -> Result<(), SaveRestoreFinalizeError> {
    for child in &mut stage.children {
        remove_finalization_child(&stage.parent_dir, child)?;
        child.expected_identity = None;
        child.expected_digest = None;
    }
    Ok(())
}

fn finalization_children(
    rollback_name: OsString,
    rollback: Option<(DirectoryIdentity, String)>,
    failed_name: OsString,
    failed: Option<(DirectoryIdentity, String)>,
    fallback_name: OsString,
    fallback: Option<(DirectoryIdentity, String)>,
    staging: (OsString, Option<(DirectoryIdentity, String)>),
) -> [FinalizeChild; 4] {
    let (staging_name, staging) = staging;
    [
        FinalizeChild {
            name: rollback_name,
            expected_identity: rollback.as_ref().map(|(identity, _)| *identity),
            expected_digest: rollback.map(|(_, digest)| digest),
        },
        FinalizeChild {
            name: failed_name,
            expected_identity: failed.as_ref().map(|(identity, _)| *identity),
            expected_digest: failed.map(|(_, digest)| digest),
        },
        FinalizeChild {
            name: fallback_name,
            expected_identity: fallback.as_ref().map(|(identity, _)| *identity),
            expected_digest: fallback.map(|(_, digest)| digest),
        },
        FinalizeChild {
            name: staging_name,
            expected_identity: staging.as_ref().map(|(identity, _)| *identity),
            expected_digest: staging.map(|(_, digest)| digest),
        },
    ]
}

fn known_directory_child(
    parent: &Dir,
    name: &OsStr,
    expected_identity: DirectoryIdentity,
    expected_digest: &str,
) -> Option<(DirectoryIdentity, String)> {
    directory_child_matches(parent, name, expected_identity, expected_digest)
        .then(|| (expected_identity, expected_digest.to_owned()))
}

fn remove_finalization_child(
    parent: &Dir,
    child: &FinalizeChild,
) -> Result<(), SaveRestoreFinalizeError> {
    let metadata = match parent.symlink_metadata(&child.name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return if child.expected_identity.is_none() {
                Ok(())
            } else {
                Err(SaveRestoreFinalizeError::RecoveryEvidenceUnsafe)
            };
        }
        Err(_) => return Err(SaveRestoreFinalizeError::CleanupFailed),
    };
    let Some(expected_identity) = child.expected_identity else {
        return Err(SaveRestoreFinalizeError::RecoveryEvidenceUnsafe);
    };
    let Some(expected_digest) = child.expected_digest.as_deref() else {
        return Err(SaveRestoreFinalizeError::RecoveryEvidenceUnsafe);
    };
    if is_cap_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(SaveRestoreFinalizeError::RecoveryEvidenceUnsafe);
    }
    let directory =
        open_child_directory_nofollow(parent, &child.name, "save restore recovery evidence")
            .map_err(|_| SaveRestoreFinalizeError::CleanupFailed)?;
    if directory_identity(&directory).ok() != Some(expected_identity)
        || directory_content_digest_from_dir(&directory)
            .ok()
            .as_deref()
            != Some(expected_digest)
    {
        return Err(SaveRestoreFinalizeError::RecoveryEvidenceUnsafe);
    }
    drop(directory);
    remove_child_tree_nofollow(parent, &child.name, "save restore recovery evidence")
        .map_err(|_| SaveRestoreFinalizeError::CleanupFailed)
}

fn directory_identity(directory: &Dir) -> Result<DirectoryIdentity, ()> {
    let metadata = directory.dir_metadata().map_err(|_| ())?;
    if is_cap_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(());
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle as _;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };

        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        let succeeded = unsafe {
            GetFileInformationByHandle(
                directory.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE,
                &mut information,
            )
        };
        if succeeded == 0 {
            return Err(());
        }
        Ok(DirectoryIdentity::Windows {
            volume_serial_number: information.dwVolumeSerialNumber,
            file_index: (u64::from(information.nFileIndexHigh) << 32)
                | u64::from(information.nFileIndexLow),
        })
    }
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt as _;

        Ok(DirectoryIdentity::Unix {
            dev: metadata.dev(),
            ino: metadata.ino(),
        })
    }
    #[cfg(not(any(windows, unix)))]
    {
        Ok(DirectoryIdentity::Unsupported)
    }
}

fn canonical_regular_directory(path: &Path) -> Result<PathBuf, ()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ())?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(());
    }
    path.canonicalize().map_err(|_| ())
}

fn target_directory(selection: &hmm_core::ProfileDirectorySelection) -> Result<PathBuf, ()> {
    if selection.mode == hmm_core::ProfileDirectoryMode::Unset
        || selection.status != hmm_core::ProfileDirectoryStatus::Valid
    {
        return Err(());
    }
    let path = selection.directory.as_deref().ok_or(())?;
    canonical_regular_directory(Path::new(path))
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

fn directory_content_digest_from_dir(root: &Dir) -> Result<String, ()> {
    let mut files = BTreeMap::new();
    let mut directories = BTreeSet::new();
    let mut collision_keys = BTreeSet::new();
    let mut total_bytes = 0_u64;
    let mut directory_count = 0_usize;
    collect_directory_facts_from_dir(
        root,
        "",
        &mut files,
        &mut directories,
        &mut collision_keys,
        &mut total_bytes,
        &mut directory_count,
    )?;

    let mut digest = Sha256::new();
    digest.update(b"hmm-save-restore-directory-v1\0");
    for relative_path in directories {
        digest.update(b"directory\0");
        digest.update(relative_path.as_bytes());
        digest.update(b"\0");
    }
    for (relative_path, (size, hash)) in files {
        digest.update(b"file\0");
        digest.update(relative_path.as_bytes());
        digest.update(b"\0");
        digest.update(size.to_string().as_bytes());
        digest.update(b"\0");
        digest.update(hash.as_bytes());
        digest.update(b"\0");
    }
    Ok(format_sha256(digest.finalize()))
}

fn collect_directory_facts_from_dir(
    current: &Dir,
    prefix: &str,
    files: &mut BTreeMap<String, (u64, String)>,
    directories: &mut BTreeSet<String>,
    collision_keys: &mut BTreeSet<String>,
    total_bytes: &mut u64,
    directory_count: &mut usize,
) -> Result<(), ()> {
    for entry in current.entries().map_err(|_| ())? {
        let entry = entry.map_err(|_| ())?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(())?;
        let relative_path = if prefix.is_empty() {
            normalize_relative_path(name).map_err(|_| ())?
        } else {
            normalize_relative_path(&format!("{prefix}/{name}")).map_err(|_| ())?
        };
        let metadata = current
            .symlink_metadata(entry.file_name())
            .map_err(|_| ())?;
        if is_cap_link_or_reparse(&metadata) {
            return Err(());
        }
        if metadata.is_dir() {
            *directory_count = directory_count.checked_add(1).ok_or(())?;
            if *directory_count > MAX_SAVE_DIRECTORY_COUNT
                || !directories.insert(relative_path.clone())
            {
                return Err(());
            }
            let child = open_child_directory_nofollow(
                current,
                &entry.file_name(),
                "save restore digest directory entry",
            )
            .map_err(|_| ())?;
            collect_directory_facts_from_dir(
                &child,
                &relative_path,
                files,
                directories,
                collision_keys,
                total_bytes,
                directory_count,
            )?;
            continue;
        }
        if !metadata.is_file() || files.len() >= MAX_FILE_COUNT {
            return Err(());
        }
        if metadata.len() > MAX_SINGLE_FILE_BYTES {
            return Err(());
        }
        *total_bytes = total_bytes.checked_add(metadata.len()).ok_or(())?;
        if *total_bytes > MAX_TOTAL_BYTES {
            return Err(());
        }

        if !collision_keys.insert(relative_path.to_lowercase()) {
            return Err(());
        }
        let mut file =
            open_regular_file_nofollow(current, &entry.file_name(), "save restore digest file")
                .map_err(|_| ())?;
        let opened = file.metadata().map_err(|_| ())?;
        if is_cap_link_or_reparse(&opened) || !opened.is_file() || opened.len() != metadata.len() {
            return Err(());
        }
        let hash = sha256_regular_file(&mut file, metadata.len())?;
        if files
            .insert(relative_path, (metadata.len(), hash))
            .is_some()
        {
            return Err(());
        }
    }
    Ok(())
}

fn sha256_regular_file(file: &mut File, expected_size: u64) -> Result<String, ()> {
    let before = file.metadata().map_err(|_| ())?;
    if is_cap_link_or_reparse(&before) || !before.is_file() || before.len() != expected_size {
        return Err(());
    }
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer).map_err(|_| ())?;
        if read == 0 {
            break;
        }
        bytes = bytes.checked_add(read as u64).ok_or(())?;
        if bytes > expected_size {
            return Err(());
        }
        hasher.update(&buffer[..read]);
    }
    let after = file.metadata().map_err(|_| ())?;
    if is_cap_link_or_reparse(&after)
        || !after.is_file()
        || bytes != expected_size
        || after.len() != expected_size
    {
        return Err(());
    }
    Ok(format_sha256(hasher.finalize()))
}

fn paths_contain(parent: &Path, child: &Path) -> bool {
    let parent = parent.to_string_lossy().replace('\\', "/").to_lowercase();
    let child = child.to_string_lossy().replace('\\', "/").to_lowercase();
    child == parent || child.starts_with(&format!("{parent}/"))
}

fn safe_transaction_fragment(value: &str) -> Option<String> {
    let fragment = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        .collect::<String>();
    (!fragment.is_empty() && fragment.len() <= 80 && fragment == value).then_some(fragment)
}

fn safe_child_file_name(file_name: &str) -> Result<&OsStr, SaveRestoreSourceError> {
    if file_name.is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name == "."
        || file_name == ".."
    {
        return Err(SaveRestoreSourceError::UnsafePath);
    }
    Ok(OsStr::new(file_name))
}

fn open_and_validate_source(
    backup_dir: &Path,
    summary: &SaveBackupSummary,
) -> Result<OpenSaveRestoreSource, SaveRestoreSourceError> {
    let backup_dir = open_existing_directory_nofollow(backup_dir, "save restore backup directory")
        .map_err(|_| SaveRestoreSourceError::BackupDirectoryUnavailable)?;
    let manifest_name = safe_child_file_name(&summary.manifest_file_name)?;
    let archive_name = safe_child_file_name(&summary.archive_file_name)?;
    let mut manifest_file =
        open_regular_file_nofollow(&backup_dir, manifest_name, "save restore manifest")
            .map_err(|_| SaveRestoreSourceError::ManifestUnavailable)?;
    let manifest_bytes = read_manifest(&mut manifest_file)?;
    let manifest: SaveBackupManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| SaveRestoreSourceError::ManifestInvalid)?;
    validate_manifest_identity(summary, &manifest)?;

    let mut archive = open_regular_file_nofollow(&backup_dir, archive_name, "save restore archive")
        .map_err(|_| SaveRestoreSourceError::ArchiveUnavailable)?;
    validate_archive_identity(summary, &mut archive)?;
    Ok(OpenSaveRestoreSource {
        manifest_bytes,
        manifest,
        archive,
    })
}

fn read_manifest(file: &mut File) -> Result<Vec<u8>, SaveRestoreSourceError> {
    let metadata = file
        .metadata()
        .map_err(|_| SaveRestoreSourceError::ManifestUnavailable)?;
    if metadata.len() == 0 || metadata.len() > MAX_MANIFEST_BYTES {
        return Err(SaveRestoreSourceError::ManifestInvalid);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SaveRestoreSourceError::ManifestUnavailable)?;
    let after = file
        .metadata()
        .map_err(|_| SaveRestoreSourceError::ManifestUnavailable)?;
    if is_cap_link_or_reparse(&after)
        || !after.is_file()
        || after.len() != metadata.len()
        || bytes.len() as u64 != metadata.len()
    {
        return Err(SaveRestoreSourceError::ManifestInvalid);
    }
    Ok(bytes)
}

fn validate_manifest_identity(
    summary: &SaveBackupSummary,
    manifest: &SaveBackupManifest,
) -> Result<(), SaveRestoreSourceError> {
    if manifest.schema_version != hmm_core::SAVE_BACKUP_MANIFEST_SCHEMA_VERSION
        || manifest.backup_id != summary.backup_id
        || manifest.game_id != summary.game_id
        || manifest.profile_id != summary.profile_id
        || manifest.trigger != summary.trigger
        || manifest.archive_file_name != summary.archive_file_name
        || manifest.archive_size_bytes != summary.archive_size_bytes
        || manifest.archive_sha256 != summary.archive_sha256
        || manifest.files.len() != summary.file_count as usize
    {
        return Err(SaveRestoreSourceError::ManifestInvalid);
    }
    if manifest.files.is_empty() || manifest.files.len() > MAX_FILE_COUNT {
        return Err(SaveRestoreSourceError::SizeLimitExceeded);
    }
    Ok(())
}

fn source_evidence_digest(summary: &SaveBackupSummary, manifest_bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"hmm-save-restore-source-v1\0");
    digest.update(summary.backup_id.as_bytes());
    digest.update(b"\0");
    digest.update(summary.archive_sha256.as_bytes());
    digest.update(b"\0");
    digest.update(manifest_bytes);
    format_sha256(digest.finalize())
}

fn validate_archive_identity(
    summary: &SaveBackupSummary,
    archive: &mut File,
) -> Result<(), SaveRestoreSourceError> {
    let metadata = archive
        .metadata()
        .map_err(|_| SaveRestoreSourceError::ArchiveUnavailable)?;
    if metadata.len() != summary.archive_size_bytes {
        return Err(SaveRestoreSourceError::HashMismatch);
    }
    archive
        .seek(SeekFrom::Start(0))
        .map_err(|_| SaveRestoreSourceError::ArchiveUnavailable)?;
    let actual = sha256_file(archive, summary.archive_size_bytes)?;
    if actual != summary.archive_sha256 {
        return Err(SaveRestoreSourceError::HashMismatch);
    }
    archive
        .seek(SeekFrom::Start(0))
        .map_err(|_| SaveRestoreSourceError::ArchiveUnavailable)?;
    Ok(())
}

fn extract_and_validate(
    archive: File,
    manifest_files: &[SaveBackupManifestFile],
    staging_root: &Dir,
) -> Result<u64, SaveRestoreSourceError> {
    process_archive(archive, manifest_files, Some(staging_root))
}

fn validate_archive_contents(
    archive: File,
    manifest_files: &[SaveBackupManifestFile],
) -> Result<u64, SaveRestoreSourceError> {
    process_archive(archive, manifest_files, None)
}

fn process_archive(
    archive_file: File,
    manifest_files: &[SaveBackupManifestFile],
    staging_root: Option<&Dir>,
) -> Result<u64, SaveRestoreSourceError> {
    let mut archive =
        ZipArchive::new(archive_file).map_err(|_| SaveRestoreSourceError::ArchiveInvalid)?;
    if archive.len() != manifest_files.len() || archive.len() > MAX_FILE_COUNT {
        return Err(SaveRestoreSourceError::ArchiveInvalid);
    }

    let expected = manifest_file_map(manifest_files)?;
    let mut seen = BTreeSet::new();
    let mut total_bytes = 0_u64;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|_| SaveRestoreSourceError::ArchiveInvalid)?;
        if entry.is_dir() || is_link_mode(entry.unix_mode()) {
            return Err(SaveRestoreSourceError::UnsafePath);
        }
        let relative_path = normalize_relative_path(entry.name())?;
        let collision_key = relative_path.to_lowercase();
        if !seen.insert(collision_key) {
            return Err(SaveRestoreSourceError::UnsafePath);
        }
        let expected_file = expected
            .get(&relative_path)
            .ok_or(SaveRestoreSourceError::ArchiveInvalid)?;
        if entry.size() != expected_file.size_bytes || entry.size() > MAX_SINGLE_FILE_BYTES {
            return Err(SaveRestoreSourceError::SizeLimitExceeded);
        }

        let mut output = if let Some(staging_root) = staging_root {
            Some(create_staging_file(staging_root, &relative_path)?)
        } else {
            None
        };
        let mut hasher = Sha256::new();
        let mut file_bytes = 0_u64;
        let mut buffer = [0_u8; 8192];
        loop {
            let read = entry
                .read(&mut buffer)
                .map_err(|_| SaveRestoreSourceError::ArchiveInvalid)?;
            if read == 0 {
                break;
            }
            file_bytes = file_bytes
                .checked_add(read as u64)
                .ok_or(SaveRestoreSourceError::SizeLimitExceeded)?;
            total_bytes = total_bytes
                .checked_add(read as u64)
                .ok_or(SaveRestoreSourceError::SizeLimitExceeded)?;
            if file_bytes > MAX_SINGLE_FILE_BYTES || total_bytes > MAX_TOTAL_BYTES {
                return Err(SaveRestoreSourceError::SizeLimitExceeded);
            }
            hasher.update(&buffer[..read]);
            if let Some(output) = output.as_mut() {
                output
                    .write_all(&buffer[..read])
                    .map_err(|_| SaveRestoreSourceError::StagingUnavailable)?;
            }
        }
        if file_bytes != expected_file.size_bytes
            || format_sha256(hasher.finalize()) != expected_file.sha256
        {
            return Err(SaveRestoreSourceError::HashMismatch);
        }
        if let Some(output) = output.as_mut() {
            output
                .flush()
                .map_err(|_| SaveRestoreSourceError::StagingUnavailable)?;
            let metadata = output
                .metadata()
                .map_err(|_| SaveRestoreSourceError::StagingUnavailable)?;
            if is_cap_link_or_reparse(&metadata)
                || !metadata.is_file()
                || metadata.len() != expected_file.size_bytes
            {
                return Err(SaveRestoreSourceError::StagingUnavailable);
            }
        }
    }

    if seen.len() != expected.len() {
        return Err(SaveRestoreSourceError::ArchiveInvalid);
    }
    Ok(total_bytes)
}

fn create_staging_file(
    staging_root: &Dir,
    relative_path: &str,
) -> Result<File, SaveRestoreSourceError> {
    let mut components = relative_path.split('/').collect::<Vec<_>>();
    let file_name = components
        .pop()
        .filter(|value| !value.is_empty())
        .ok_or(SaveRestoreSourceError::UnsafePath)?;
    let mut parent = staging_root
        .try_clone()
        .map_err(|_| SaveRestoreSourceError::StagingUnavailable)?;
    for component in components {
        parent = open_or_create_child_directory(
            &parent,
            OsStr::new(component),
            "save restore staging directory",
        )
        .map_err(|_| SaveRestoreSourceError::StagingUnavailable)?;
    }
    create_new_regular_file(&parent, OsStr::new(file_name), "save restore staging file")
        .map_err(|_| SaveRestoreSourceError::StagingUnavailable)
}

fn manifest_file_map(
    files: &[SaveBackupManifestFile],
) -> Result<BTreeMap<String, &SaveBackupManifestFile>, SaveRestoreSourceError> {
    let mut result = BTreeMap::new();
    let mut collision_keys = BTreeSet::new();
    let mut directories = BTreeSet::new();
    let mut total = 0_u64;
    for file in files {
        let path = normalize_relative_path(&file.relative_path)?;
        if path != file.relative_path {
            return Err(SaveRestoreSourceError::UnsafePath);
        }
        if file.size_bytes > MAX_SINGLE_FILE_BYTES {
            return Err(SaveRestoreSourceError::SizeLimitExceeded);
        }
        total = total
            .checked_add(file.size_bytes)
            .ok_or(SaveRestoreSourceError::SizeLimitExceeded)?;
        if total > MAX_TOTAL_BYTES {
            return Err(SaveRestoreSourceError::SizeLimitExceeded);
        }
        if !record_parent_directories(&path, &mut directories) {
            return Err(SaveRestoreSourceError::SizeLimitExceeded);
        }
        if !collision_keys.insert(path.to_lowercase()) || result.insert(path, file).is_some() {
            return Err(SaveRestoreSourceError::UnsafePath);
        }
    }
    Ok(result)
}

fn normalize_relative_path(raw: &str) -> Result<String, SaveRestoreSourceError> {
    normalize_save_relative_path(raw).ok_or(SaveRestoreSourceError::UnsafePath)
}

fn is_link_mode(mode: Option<u32>) -> bool {
    mode.is_some_and(|mode| mode & 0o170000 == 0o120000)
}

fn sha256_file(file: &mut File, expected_size: u64) -> Result<String, SaveRestoreSourceError> {
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| SaveRestoreSourceError::ArchiveUnavailable)?;
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(read as u64)
            .ok_or(SaveRestoreSourceError::HashMismatch)?;
        if bytes > expected_size {
            return Err(SaveRestoreSourceError::HashMismatch);
        }
        hasher.update(&buffer[..read]);
    }
    let after = file
        .metadata()
        .map_err(|_| SaveRestoreSourceError::ArchiveUnavailable)?;
    if is_cap_link_or_reparse(&after)
        || !after.is_file()
        || after.len() != expected_size
        || bytes != expected_size
    {
        return Err(SaveRestoreSourceError::HashMismatch);
    }
    Ok(format_sha256(hasher.finalize()))
}

fn format_sha256(bytes: impl AsRef<[u8]>) -> String {
    let hex = bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileSystemSaveBackupWriter;
    use hmm_core::{
        GameId, ProfileBackupRetention, ProfileDirectoryMode, ProfileDirectorySelection,
        ProfileDirectoryStatus, ProfileId, SaveBackupTrigger,
    };
    use hmm_ports::{
        SaveBackupWriteRequest, SaveBackupWriter, SaveRestoreCommitRequest, SaveRestoreFileSystem,
        SaveRestoreFinalizeRequest, SaveRestorePrepareRequest,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn relative_path_rejects_windows_aliases_and_traversal() {
        for value in ["../save", "C:/save", "folder//save", "CON", "save. "] {
            assert_eq!(
                normalize_relative_path(value),
                Err(SaveRestoreSourceError::UnsafePath)
            );
        }
        assert_eq!(
            normalize_relative_path("nested/SAVEDATA1000").expect("safe path"),
            "nested/SAVEDATA1000"
        );
    }

    #[test]
    fn manifest_paths_reject_excess_directory_nodes() {
        let files = (0..MAX_FILE_COUNT)
            .map(|index| SaveBackupManifestFile {
                relative_path: format!("a{index}/b{index}/c{index}/SAVEDATA1000"),
                size_bytes: 0,
                sha256: format_sha256(Sha256::digest([])),
                modified_at_utc: None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            manifest_file_map(&files),
            Err(SaveRestoreSourceError::SizeLimitExceeded)
        );
    }

    #[test]
    fn directory_digest_rejects_paths_deeper_than_restore_budget() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().join("save-root");
        let mut nested = root.clone();
        for index in 0..crate::save_path::MAX_SAVE_PATH_COMPONENTS {
            nested.push(format!("d{index}"));
        }
        fs::create_dir_all(&nested).expect("create deep target");
        fs::write(nested.join("save.bin"), b"save").expect("write deep target file");
        let root = open_existing_directory_nofollow(&root, "deep save root").expect("open root");

        assert!(directory_content_digest_from_dir(&root).is_err());
    }

    #[test]
    fn extraction_rejects_linked_staging_subdirectory_without_external_write() {
        let temp = tempfile::tempdir().expect("temp dir");
        let app_data = temp.path().join("app-data");
        let backup_source = temp.path().join("backup-source");
        let nested_source = backup_source.join("nested");
        fs::create_dir_all(&nested_source).expect("create nested backup source");
        fs::write(nested_source.join("SAVEDATA1000"), b"backup-save")
            .expect("write nested backup source");
        let summary = FileSystemSaveBackupWriter::new(app_data.clone())
            .write_backup(SaveBackupWriteRequest {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                trigger: SaveBackupTrigger::Manual,
                source_directory: Some(backup_source.to_string_lossy().into_owned()),
                source_directory_selection: custom_selection(&backup_source),
                backup_directory: default_selection(),
                retention: ProfileBackupRetention::default(),
                note: None,
                created_at_unix_millis: 0,
            })
            .expect("write nested fixture backup")
            .summary;
        let backup_dir = managed_backup_directory_for_summary(&app_data, &summary)
            .expect("resolve fixture backup directory");
        let backup_dir =
            canonical_regular_directory(&backup_dir).expect("canonical fixture backup directory");
        let source =
            open_and_validate_source(&backup_dir, &summary).expect("open validated fixture source");

        let staging = temp.path().join("staging");
        let external = temp.path().join("external");
        fs::create_dir(&staging).expect("create staging root");
        fs::create_dir(&external).expect("create external directory");
        fs::write(external.join("keep.txt"), b"keep").expect("write external sentinel");
        let linked_subdirectory = staging.join("nested");
        create_directory_link(&linked_subdirectory, &external);
        let staging_dir =
            open_existing_directory_nofollow(&staging, "save restore linked staging fixture")
                .expect("open staging root");

        let error = extract_and_validate(source.archive, &source.manifest.files, &staging_dir)
            .expect_err("linked staging child must fail closed");

        assert_eq!(error, SaveRestoreSourceError::StagingUnavailable);
        assert_eq!(
            fs::read(external.join("keep.txt")).expect("read external sentinel"),
            b"keep"
        );
        assert!(!external.join("SAVEDATA1000").exists());
        drop(staging_dir);
        remove_directory_link(&linked_subdirectory);
    }

    #[test]
    fn filesystem_restore_swaps_validated_staging_into_target() {
        let fixture = restore_fixture();
        let file_system = FileSystemSaveRestoreFileSystem::new(fixture.app_data.clone());
        let transaction_id = "restore-success";
        let prepared = file_system
            .prepare_restore(prepare_request(&fixture, transaction_id))
            .expect("prepare restore");

        let result = file_system
            .commit_restore(commit_request(
                &fixture,
                transaction_id,
                prepared.prepared_id,
            ))
            .expect("commit restore");

        assert_eq!(result.restored_file_count, 1);
        assert_eq!(
            fs::read(fixture.target.join("SAVEDATA1000")).expect("read restored save"),
            b"backup-save"
        );
        let rollback = fixture
            .target
            .parent()
            .expect("target parent")
            .join(".hmm-save-restore-restore-success-rollback");
        assert!(rollback.exists());

        file_system
            .finalize_restore(SaveRestoreFinalizeRequest {
                transaction_id: transaction_id.to_owned(),
                target_directory: custom_selection(&fixture.target),
            })
            .expect("finalize restore after durable completion");
        assert!(!rollback.exists());
    }

    #[test]
    fn finalize_refuses_replaced_rollback_directory_and_preserves_unknown_contents() {
        let fixture = restore_fixture();
        let file_system = FileSystemSaveRestoreFileSystem::new(fixture.app_data.clone());
        let transaction_id = "restore-finalize-replaced-rollback";
        let prepared = file_system
            .prepare_restore(prepare_request(&fixture, transaction_id))
            .expect("prepare restore");
        file_system
            .commit_restore(commit_request(
                &fixture,
                transaction_id,
                prepared.prepared_id,
            ))
            .expect("commit restore");
        let parent = fixture.target.parent().expect("target parent");
        let rollback = parent.join(".hmm-save-restore-restore-finalize-replaced-rollback-rollback");
        let moved_rollback = parent.join("original-rollback-evidence");
        fs::rename(&rollback, &moved_rollback).expect("move original rollback evidence");
        fs::create_dir(&rollback).expect("plant replacement rollback directory");
        fs::write(rollback.join("keep.txt"), b"keep").expect("write replacement sentinel");

        let error = file_system
            .finalize_restore(SaveRestoreFinalizeRequest {
                transaction_id: transaction_id.to_owned(),
                target_directory: custom_selection(&fixture.target),
            })
            .expect_err("replacement rollback identity must fail closed");

        assert_eq!(error, SaveRestoreFinalizeError::RecoveryEvidenceUnsafe);
        assert_eq!(
            fs::read(rollback.join("keep.txt")).expect("read replacement sentinel"),
            b"keep"
        );
        assert!(moved_rollback.exists());
    }

    #[test]
    fn finalize_refuses_same_identity_rollback_content_drift() {
        let fixture = restore_fixture();
        let file_system = FileSystemSaveRestoreFileSystem::new(fixture.app_data.clone());
        let transaction_id = "restore-finalize-drifted-rollback";
        let prepared = file_system
            .prepare_restore(prepare_request(&fixture, transaction_id))
            .expect("prepare restore");
        file_system
            .commit_restore(commit_request(
                &fixture,
                transaction_id,
                prepared.prepared_id,
            ))
            .expect("commit restore");

        let rollback = fixture
            .target
            .parent()
            .expect("target parent")
            .join(".hmm-save-restore-restore-finalize-drifted-rollback-rollback");
        fs::write(rollback.join("unexpected.txt"), b"keep").expect("write drift sentinel");

        let error = file_system
            .finalize_restore(SaveRestoreFinalizeRequest {
                transaction_id: transaction_id.to_owned(),
                target_directory: custom_selection(&fixture.target),
            })
            .expect_err("same-identity content drift must fail closed");

        assert_eq!(error, SaveRestoreFinalizeError::RecoveryEvidenceUnsafe);
        assert_eq!(
            fs::read(rollback.join("unexpected.txt")).expect("read drift sentinel"),
            b"keep"
        );
    }

    #[test]
    fn finalize_refuses_same_identity_rollback_empty_directory_drift() {
        let fixture = restore_fixture();
        let file_system = FileSystemSaveRestoreFileSystem::new(fixture.app_data.clone());
        let transaction_id = "restore-finalize-empty-directory-drift";
        let prepared = file_system
            .prepare_restore(prepare_request(&fixture, transaction_id))
            .expect("prepare restore");
        file_system
            .commit_restore(commit_request(
                &fixture,
                transaction_id,
                prepared.prepared_id,
            ))
            .expect("commit restore");

        let rollback = fixture
            .target
            .parent()
            .expect("target parent")
            .join(".hmm-save-restore-restore-finalize-empty-directory-drift-rollback");
        let unexpected = rollback.join("unexpected-empty-directory");
        fs::create_dir(&unexpected).expect("create empty directory drift sentinel");

        let error = file_system
            .finalize_restore(SaveRestoreFinalizeRequest {
                transaction_id: transaction_id.to_owned(),
                target_directory: custom_selection(&fixture.target),
            })
            .expect_err("same-identity empty-directory drift must fail closed");

        assert_eq!(error, SaveRestoreFinalizeError::RecoveryEvidenceUnsafe);
        assert!(unexpected.is_dir());
    }

    #[test]
    fn finalize_retry_remembers_children_already_removed_before_a_later_failure() {
        let fixture = restore_fixture();
        let file_system = FileSystemSaveRestoreFileSystem::new(fixture.app_data.clone());
        let transaction_id = "restore-finalize-retry";
        let prepared = file_system
            .prepare_restore(prepare_request(&fixture, transaction_id))
            .expect("prepare restore");
        file_system
            .commit_restore(commit_request(
                &fixture,
                transaction_id,
                prepared.prepared_id,
            ))
            .expect("commit restore");
        let parent = fixture.target.parent().expect("target parent");
        let rollback = parent.join(".hmm-save-restore-restore-finalize-retry-rollback");
        let failed = parent.join(".hmm-save-restore-restore-finalize-retry-failed");
        fs::create_dir(&failed).expect("plant later unsafe child");
        fs::write(failed.join("keep.txt"), b"keep").expect("write unsafe child sentinel");

        let error = file_system
            .finalize_restore(SaveRestoreFinalizeRequest {
                transaction_id: transaction_id.to_owned(),
                target_directory: custom_selection(&fixture.target),
            })
            .expect_err("later unsafe child must block finalization");

        assert_eq!(error, SaveRestoreFinalizeError::RecoveryEvidenceUnsafe);
        assert!(!rollback.exists(), "first child was already removed");
        assert_eq!(
            fs::read(failed.join("keep.txt")).expect("read unsafe child sentinel"),
            b"keep"
        );

        fs::remove_dir_all(&failed).expect("remove temporary blocker");
        file_system
            .finalize_restore(SaveRestoreFinalizeRequest {
                transaction_id: transaction_id.to_owned(),
                target_directory: custom_selection(&fixture.target),
            })
            .expect("retry resumes after the already removed child");
        file_system
            .finalize_restore(SaveRestoreFinalizeRequest {
                transaction_id: transaction_id.to_owned(),
                target_directory: custom_selection(&fixture.target),
            })
            .expect("completed finalization remains idempotent");
    }

    #[test]
    fn filesystem_restore_rejects_target_drift_after_prepare() {
        let fixture = restore_fixture();
        let file_system = FileSystemSaveRestoreFileSystem::new(fixture.app_data.clone());
        let transaction_id = "restore-target-drift";
        let prepared = file_system
            .prepare_restore(prepare_request(&fixture, transaction_id))
            .expect("prepare restore");
        fs::write(fixture.target.join("SAVEDATA1000"), b"external-drift").expect("drift target");

        let error = file_system
            .commit_restore(commit_request(
                &fixture,
                transaction_id,
                prepared.prepared_id,
            ))
            .expect_err("target drift must fail");

        assert_eq!(error, SaveRestoreCommitError::TargetChanged);
        assert_eq!(
            fs::read(fixture.target.join("SAVEDATA1000")).expect("read drifted save"),
            b"external-drift"
        );
        assert!(!fixture
            .target
            .parent()
            .expect("target parent")
            .join(".hmm-save-restore-restore-target-drift-staging")
            .exists());
    }

    #[test]
    fn filesystem_restore_rejects_same_content_target_replacement() {
        let fixture = restore_fixture();
        let file_system = FileSystemSaveRestoreFileSystem::new(fixture.app_data.clone());
        let transaction_id = "restore-target-replaced";
        let prepared = file_system
            .prepare_restore(prepare_request(&fixture, transaction_id))
            .expect("prepare restore");
        let parent = fixture.target.parent().expect("target parent");
        let original = parent.join("save-target-original");
        fs::rename(&fixture.target, &original).expect("move original target");
        fs::create_dir(&fixture.target).expect("create replacement target");
        fs::write(fixture.target.join("SAVEDATA1000"), b"current-save")
            .expect("write same-content replacement");

        let error = file_system
            .commit_restore(commit_request(
                &fixture,
                transaction_id,
                prepared.prepared_id,
            ))
            .expect_err("same-content replacement must fail closed");

        assert_eq!(error, SaveRestoreCommitError::TargetChanged);
        assert_eq!(
            fs::read(fixture.target.join("SAVEDATA1000")).expect("read replacement target"),
            b"current-save"
        );
        assert_eq!(
            fs::read(original.join("SAVEDATA1000")).expect("read original target"),
            b"current-save"
        );
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_restore_rejects_rebound_target_parent_without_external_write() {
        let fixture = restore_fixture();
        let file_system = FileSystemSaveRestoreFileSystem::new(fixture.app_data.clone());
        let transaction_id = "restore-parent-replaced";
        let prepared = file_system
            .prepare_restore(prepare_request(&fixture, transaction_id))
            .expect("prepare restore");
        let parent = fixture.target.parent().expect("target parent");
        let moved_parent = parent.with_file_name("save-parent-original");
        fs::rename(parent, &moved_parent).expect("move original target parent");
        fs::create_dir(parent).expect("create replacement target parent");
        fs::create_dir(&fixture.target).expect("create replacement target");
        fs::write(fixture.target.join("SAVEDATA1000"), b"current-save")
            .expect("write replacement target");
        fs::write(parent.join("external-sentinel"), b"keep").expect("write external sentinel");

        let error = file_system
            .commit_restore(commit_request(
                &fixture,
                transaction_id,
                prepared.prepared_id,
            ))
            .expect_err("rebound parent must fail closed");

        assert_eq!(error, SaveRestoreCommitError::TargetChanged);
        assert_eq!(
            fs::read(parent.join("external-sentinel")).expect("read external sentinel"),
            b"keep"
        );
        assert_eq!(
            fs::read(fixture.target.join("SAVEDATA1000")).expect("read replacement target"),
            b"current-save"
        );
        assert!(!moved_parent
            .join(".hmm-save-restore-restore-parent-replaced-staging")
            .exists());
    }

    #[cfg(unix)]
    #[test]
    fn finalize_uses_bound_parent_after_path_rebinding() {
        let fixture = restore_fixture();
        let file_system = FileSystemSaveRestoreFileSystem::new(fixture.app_data.clone());
        let transaction_id = "restore-finalize-parent";
        let prepared = file_system
            .prepare_restore(prepare_request(&fixture, transaction_id))
            .expect("prepare restore");
        file_system
            .commit_restore(commit_request(
                &fixture,
                transaction_id,
                prepared.prepared_id,
            ))
            .expect("commit restore");
        let parent = fixture.target.parent().expect("target parent");
        let moved_parent = parent.with_file_name("save-parent-committed");
        fs::rename(parent, &moved_parent).expect("move committed target parent");
        fs::create_dir(parent).expect("create replacement target parent");
        fs::create_dir(&fixture.target).expect("create replacement target");
        let planted = parent.join(".hmm-save-restore-restore-finalize-parent-rollback");
        fs::create_dir(&planted).expect("plant unrelated rollback name");
        fs::write(planted.join("keep.txt"), b"keep").expect("write planted sentinel");

        file_system
            .finalize_restore(SaveRestoreFinalizeRequest {
                transaction_id: transaction_id.to_owned(),
                target_directory: custom_selection(&fixture.target),
            })
            .expect("finalize through retained capability");

        assert!(!moved_parent
            .join(".hmm-save-restore-restore-finalize-parent-rollback")
            .exists());
        assert_eq!(
            fs::read(planted.join("keep.txt")).expect("read planted sentinel"),
            b"keep"
        );
    }

    #[cfg(unix)]
    #[test]
    fn discard_prepared_does_not_follow_a_replaced_staging_symlink() {
        use std::os::unix::fs::symlink;

        let fixture = restore_fixture();
        let file_system = FileSystemSaveRestoreFileSystem::new(fixture.app_data.clone());
        let transaction_id = "restore-staging-link";
        let prepared = file_system
            .prepare_restore(prepare_request(&fixture, transaction_id))
            .expect("prepare restore");
        let parent = fixture.target.parent().expect("target parent");
        let staging = parent.join(".hmm-save-restore-restore-staging-link-staging");
        fs::remove_dir_all(&staging).expect("remove original staging fixture");
        let external = parent.join("external-sentinel");
        fs::create_dir(&external).expect("create external fixture");
        fs::write(external.join("keep.txt"), b"keep").expect("write sentinel");
        symlink(&external, &staging).expect("replace staging with symlink");

        file_system.discard_prepared(&prepared.prepared_id);

        assert_eq!(
            fs::read(external.join("keep.txt")).expect("read sentinel"),
            b"keep"
        );
        assert!(fs::symlink_metadata(&staging)
            .expect("staging link remains")
            .file_type()
            .is_symlink());
    }

    #[test]
    fn filesystem_restore_rolls_back_when_second_rename_fails() {
        let fixture = restore_fixture();
        let file_system = FileSystemSaveRestoreFileSystem::with_renamer(
            fixture.app_data.clone(),
            Arc::new(FailingRenamer::new([2])),
        );
        let transaction_id = "restore-rollback";
        let prepared = file_system
            .prepare_restore(prepare_request(&fixture, transaction_id))
            .expect("prepare restore");

        let error = file_system
            .commit_restore(commit_request(
                &fixture,
                transaction_id,
                prepared.prepared_id,
            ))
            .expect_err("injected commit failure must roll back");

        assert_eq!(error, SaveRestoreCommitError::RolledBack);
        assert_eq!(
            fs::read(fixture.target.join("SAVEDATA1000")).expect("read rolled back save"),
            b"current-save"
        );
        assert!(!fixture
            .target
            .parent()
            .expect("target parent")
            .join(".hmm-save-restore-restore-rollback-staging")
            .exists());
    }

    #[test]
    fn filesystem_restore_finalizes_after_transient_parent_binding_failure_rolls_back() {
        let fixture = restore_fixture();
        let file_system = FileSystemSaveRestoreFileSystem::with_post_rename_parent_binding_override(
            fixture.app_data.clone(),
            false,
        );
        let transaction_id = "restore-parent-binding-rollback";
        let prepared = file_system
            .prepare_restore(prepare_request(&fixture, transaction_id))
            .expect("prepare restore");

        let error = file_system
            .commit_restore(commit_request(
                &fixture,
                transaction_id,
                prepared.prepared_id,
            ))
            .expect_err("transient parent binding failure must roll back");

        assert_eq!(error, SaveRestoreCommitError::RolledBack);
        assert_eq!(
            fs::read(fixture.target.join("SAVEDATA1000")).expect("read rolled back save"),
            b"current-save"
        );
        let parent = fixture.target.parent().expect("target parent");
        for suffix in ["staging", "rollback", "failed", "fallback"] {
            assert!(!parent
                .join(format!(".hmm-save-restore-{transaction_id}-{suffix}"))
                .exists());
        }

        file_system
            .finalize_restore(SaveRestoreFinalizeRequest {
                transaction_id: transaction_id.to_owned(),
                target_directory: custom_selection(&fixture.target),
            })
            .expect("finalize clean rolled-back restore");
    }

    #[test]
    fn filesystem_restore_preserves_siblings_when_rollback_fails() {
        let fixture = restore_fixture();
        let file_system = FileSystemSaveRestoreFileSystem::with_renamer(
            fixture.app_data.clone(),
            Arc::new(FailingRenamer::new([2, 3])),
        );
        let transaction_id = "restore-recovery";
        let prepared = file_system
            .prepare_restore(prepare_request(&fixture, transaction_id))
            .expect("prepare restore");

        let error = file_system
            .commit_restore(commit_request(
                &fixture,
                transaction_id,
                prepared.prepared_id,
            ))
            .expect_err("failed rollback must require recovery");

        assert_eq!(error, SaveRestoreCommitError::RecoveryRequired);
        let parent = fixture.target.parent().expect("target parent");
        assert!(parent
            .join(".hmm-save-restore-restore-recovery-staging")
            .exists());
        assert!(parent
            .join(".hmm-save-restore-restore-recovery-rollback")
            .exists());
        assert!(!fixture.target.exists());

        let finalization = file_system.finalization.lock().expect("finalization lock");
        let retained = finalization
            .get(transaction_id)
            .expect("recovery finalization stage");
        assert_eq!(retained.children.len(), 4);
        assert!(retained.children.iter().any(|child| {
            child.name == ".hmm-save-restore-restore-recovery-rollback"
                && child.expected_identity.is_some()
                && child.expected_digest.is_some()
        }));
        assert!(retained.children.iter().any(|child| {
            child.name == ".hmm-save-restore-restore-recovery-staging"
                && child.expected_identity.is_some()
                && child.expected_digest.is_some()
        }));
    }

    #[test]
    fn filesystem_restore_uses_valid_pre_restore_backup_when_directory_rollback_fails() {
        let fixture = restore_fixture();
        let file_system = FileSystemSaveRestoreFileSystem::with_renamer(
            fixture.app_data.clone(),
            Arc::new(FailingRenamer::new([2, 3])),
        );
        let transaction_id = "restore-pre-restore-fallback";
        let prepared = file_system
            .prepare_restore(prepare_request(&fixture, transaction_id))
            .expect("prepare restore");
        let mut request = commit_request(&fixture, transaction_id, prepared.prepared_id);
        request.pre_restore_summary = Some(fixture.pre_restore_summary.clone());

        let error = file_system
            .commit_restore(request)
            .expect_err("failed directory rollback must use pre-restore backup");

        assert_eq!(error, SaveRestoreCommitError::RolledBack);
        assert_eq!(
            fs::read(fixture.target.join("SAVEDATA1000")).expect("read fallback save"),
            b"current-save"
        );
        let parent = fixture.target.parent().expect("target parent");
        assert!(parent
            .join(".hmm-save-restore-restore-pre-restore-fallback-rollback")
            .exists());
        file_system
            .finalize_restore(SaveRestoreFinalizeRequest {
                transaction_id: transaction_id.to_owned(),
                target_directory: custom_selection(&fixture.target),
            })
            .expect("finalize rolled back restore");
        assert!(!parent
            .join(".hmm-save-restore-restore-pre-restore-fallback-rollback")
            .exists());
    }

    struct RestoreFixture {
        _temp: tempfile::TempDir,
        app_data: PathBuf,
        target: PathBuf,
        summary: SaveBackupSummary,
        pre_restore_summary: SaveBackupSummary,
    }

    fn restore_fixture() -> RestoreFixture {
        let temp = tempfile::tempdir().expect("temp dir");
        let app_data = temp.path().join("app-data");
        let backup_source = temp.path().join("backup-source");
        let target = temp.path().join("save-parent").join("save-target");
        fs::create_dir_all(&backup_source).expect("create backup source");
        fs::create_dir_all(&target).expect("create target");
        fs::write(backup_source.join("SAVEDATA1000"), b"backup-save").expect("write backup source");
        fs::write(target.join("SAVEDATA1000"), b"current-save").expect("write target");
        let writer = FileSystemSaveBackupWriter::new(app_data.clone());
        let summary = writer
            .write_backup(SaveBackupWriteRequest {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                trigger: SaveBackupTrigger::Manual,
                source_directory: Some(backup_source.to_string_lossy().into_owned()),
                source_directory_selection: custom_selection(&backup_source),
                backup_directory: default_selection(),
                retention: ProfileBackupRetention::default(),
                note: None,
                created_at_unix_millis: 0,
            })
            .expect("write fixture backup")
            .summary;
        let pre_restore_summary = writer
            .write_backup(SaveBackupWriteRequest {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                trigger: SaveBackupTrigger::PreRestore,
                source_directory: Some(target.to_string_lossy().into_owned()),
                source_directory_selection: custom_selection(&target),
                backup_directory: default_selection(),
                retention: ProfileBackupRetention::default(),
                note: Some("pre-restore fixture".to_owned()),
                created_at_unix_millis: 1,
            })
            .expect("write pre-restore fixture backup")
            .summary;
        RestoreFixture {
            _temp: temp,
            app_data,
            target,
            summary,
            pre_restore_summary,
        }
    }

    fn prepare_request(
        fixture: &RestoreFixture,
        transaction_id: &str,
    ) -> SaveRestorePrepareRequest {
        SaveRestorePrepareRequest {
            transaction_id: transaction_id.to_owned(),
            summary: fixture.summary.clone(),
            target_directory: custom_selection(&fixture.target),
        }
    }

    fn commit_request(
        fixture: &RestoreFixture,
        transaction_id: &str,
        prepared_id: String,
    ) -> SaveRestoreCommitRequest {
        SaveRestoreCommitRequest {
            transaction_id: transaction_id.to_owned(),
            prepared_id,
            summary: fixture.summary.clone(),
            target_directory: custom_selection(&fixture.target),
            pre_restore_summary: None,
        }
    }

    fn default_selection() -> ProfileDirectorySelection {
        ProfileDirectorySelection {
            mode: ProfileDirectoryMode::Default,
            status: ProfileDirectoryStatus::Defaulted,
            directory: None,
            path_label: Some("HelsincyModManager/backups".to_owned()),
            messages: Vec::new(),
        }
    }

    fn custom_selection(path: &Path) -> ProfileDirectorySelection {
        ProfileDirectorySelection {
            mode: ProfileDirectoryMode::Custom,
            status: ProfileDirectoryStatus::Valid,
            directory: Some(path.to_string_lossy().into_owned()),
            path_label: Some("fixture".to_owned()),
            messages: Vec::new(),
        }
    }

    #[cfg(unix)]
    fn create_directory_link(link: &Path, target: &Path) {
        std::os::unix::fs::symlink(target, link).expect("create directory symlink");
    }

    #[cfg(windows)]
    fn create_directory_link(link: &Path, target: &Path) {
        let output = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link.to_str().expect("junction path"),
                target.to_str().expect("junction target"),
            ])
            .output()
            .expect("create directory junction");
        assert!(
            output.status.success(),
            "mklink failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    fn remove_directory_link(link: &Path) {
        fs::remove_file(link).expect("remove directory symlink");
    }

    #[cfg(windows)]
    fn remove_directory_link(link: &Path) {
        fs::remove_dir(link).expect("remove directory junction");
    }

    struct FailingRenamer {
        calls: AtomicUsize,
        fail_calls: BTreeSet<usize>,
    }

    impl FailingRenamer {
        fn new(fail_calls: impl IntoIterator<Item = usize>) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                fail_calls: fail_calls.into_iter().collect(),
            }
        }
    }

    impl SaveRestoreDirectoryRenamer for FailingRenamer {
        fn rename(&self, parent: &Dir, from: &OsStr, to: &OsStr) -> std::io::Result<()> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if self.fail_calls.contains(&call) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "injected rename failure",
                ));
            }
            parent.rename(from, parent, to)
        }
    }
}
