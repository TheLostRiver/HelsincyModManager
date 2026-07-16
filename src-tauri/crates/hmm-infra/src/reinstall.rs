#[cfg(not(test))]
use crate::install_commit::atomic_write_file;
#[cfg(test)]
use crate::install_commit::{atomic_write_file_with_failure, AtomicWriteFailurePoint};
use crate::install_commit::{
    ensure_contained_existing_path, ensure_existing_directory, ensure_safe_write_target,
    recovery_record_file_name,
};
use anyhow::{Context, Result};
use hmm_core::{ModId, ProfileId, ReinstallRecoveryTransaction};
use hmm_ports::ReinstallRecoveryTransactionRepository;
use std::fs;
use std::path::PathBuf;

pub struct JsonReinstallRecoveryTransactionRepository {
    transaction_root: PathBuf,
    #[cfg(test)]
    atomic_write_failure: Option<AtomicWriteFailurePoint>,
}

impl JsonReinstallRecoveryTransactionRepository {
    pub fn new(transaction_root: PathBuf) -> Self {
        Self {
            transaction_root,
            #[cfg(test)]
            atomic_write_failure: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_atomic_write_failure(
        transaction_root: PathBuf,
        atomic_write_failure: AtomicWriteFailurePoint,
    ) -> Self {
        Self {
            transaction_root,
            atomic_write_failure: Some(atomic_write_failure),
        }
    }

    fn transaction_path(&self, profile_id: &ProfileId, mod_id: &ModId) -> PathBuf {
        self.transaction_root
            .join(recovery_record_file_name(profile_id, mod_id))
    }

    fn deserialize_transaction(
        &self,
        path: &std::path::Path,
    ) -> Result<ReinstallRecoveryTransaction> {
        let serialized =
            fs::read_to_string(path).context("failed to read reinstall recovery transaction")?;
        let transaction: ReinstallRecoveryTransaction = serde_json::from_str(&serialized)
            .context("failed to deserialize reinstall recovery transaction")?;
        transaction
            .validate()
            .context("invalid reinstall recovery transaction")?;
        Ok(transaction)
    }
}

impl ReinstallRecoveryTransactionRepository for JsonReinstallRecoveryTransactionRepository {
    fn load_transaction(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Result<Option<ReinstallRecoveryTransaction>> {
        match fs::symlink_metadata(&self.transaction_root) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("failed to inspect reinstall recovery root"),
        }

        ensure_existing_directory(&self.transaction_root, "reinstall recovery root")?;
        ensure_contained_existing_path(&self.transaction_root, &self.transaction_root)?;
        let transaction_path = self.transaction_path(profile_id, mod_id);
        let metadata = match fs::symlink_metadata(&transaction_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).context("failed to inspect reinstall recovery transaction")
            }
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            anyhow::bail!("reinstall recovery transaction is not a regular file");
        }
        ensure_contained_existing_path(&self.transaction_root, &transaction_path)?;

        let transaction = self.deserialize_transaction(&transaction_path)?;
        if transaction.profile_id != *profile_id || transaction.mod_id != *mod_id {
            anyhow::bail!("reinstall recovery transaction id does not match request");
        }
        Ok(Some(transaction))
    }

    fn list_transactions(
        &self,
        profile_id: &ProfileId,
    ) -> Result<Vec<ReinstallRecoveryTransaction>> {
        match fs::symlink_metadata(&self.transaction_root) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error).context("failed to inspect reinstall recovery root"),
        }

        ensure_existing_directory(&self.transaction_root, "reinstall recovery root")?;
        ensure_contained_existing_path(&self.transaction_root, &self.transaction_root)?;
        let mut transactions = Vec::new();
        for entry in fs::read_dir(&self.transaction_root)
            .context("failed to read reinstall recovery root")?
        {
            let entry = entry.context("failed to read reinstall recovery entry")?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.starts_with("record-") || !file_name.ends_with(".json") {
                continue;
            }
            let transaction_path = entry.path();
            let metadata = fs::symlink_metadata(&transaction_path)
                .context("failed to inspect reinstall recovery transaction")?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                anyhow::bail!("reinstall recovery transaction is not a regular file");
            }
            ensure_contained_existing_path(&self.transaction_root, &transaction_path)?;
            let transaction = self.deserialize_transaction(&transaction_path)?;
            if transaction.profile_id == *profile_id {
                transactions.push(transaction);
            }
        }
        transactions.sort_by(|left, right| left.mod_id.cmp(&right.mod_id));
        Ok(transactions)
    }

    fn save_transaction(&self, transaction: &ReinstallRecoveryTransaction) -> Result<()> {
        transaction
            .validate()
            .context("refusing to save invalid reinstall recovery transaction")?;
        fs::create_dir_all(&self.transaction_root)
            .context("failed to create reinstall recovery root")?;
        ensure_existing_directory(&self.transaction_root, "reinstall recovery root")?;
        ensure_contained_existing_path(&self.transaction_root, &self.transaction_root)?;
        let transaction_path = self.transaction_path(&transaction.profile_id, &transaction.mod_id);
        ensure_safe_write_target(&self.transaction_root, &transaction_path)?;
        let serialized = serde_json::to_string_pretty(transaction)
            .context("failed to serialize reinstall recovery transaction")?;
        #[cfg(test)]
        let write_result = atomic_write_file_with_failure(
            &transaction_path,
            serialized.as_bytes(),
            self.atomic_write_failure,
        );
        #[cfg(not(test))]
        let write_result = atomic_write_file(&transaction_path, serialized.as_bytes());
        write_result.context("failed to write reinstall recovery transaction")
    }

    fn remove_transaction(&self, profile_id: &ProfileId, mod_id: &ModId) -> Result<()> {
        match fs::symlink_metadata(&self.transaction_root) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error).context("failed to inspect reinstall recovery root"),
        }
        ensure_existing_directory(&self.transaction_root, "reinstall recovery root")?;
        ensure_contained_existing_path(&self.transaction_root, &self.transaction_root)?;
        let transaction_path = self.transaction_path(profile_id, mod_id);
        let metadata = match fs::symlink_metadata(&transaction_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(error).context("failed to inspect reinstall recovery transaction")
            }
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            anyhow::bail!("reinstall recovery transaction is not a regular file");
        }
        ensure_contained_existing_path(&self.transaction_root, &transaction_path)?;
        fs::remove_file(transaction_path).context("failed to remove reinstall recovery transaction")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileSystemInstallBackupStore;
    use hmm_core::{
        FileLayer, InstallManifest, InstallManifestEntry, InstallTargetPath, InstalledFileSummary,
        ModId, ModRevisionId, PackageFileId, ProfileId, ReinstallRecoveryTarget,
        ReinstallRecoveryTransaction, ReinstallRecoveryTransactionStatus,
        ReinstallSnapshotCleanupOwner, ReinstallSnapshotPurpose, ReinstallSnapshotState,
        ReinstallTargetClass,
    };
    use hmm_ports::{ReinstallRecoveryTransactionRepository, ReinstallSnapshotStore};
    use std::fs;
    use std::path::Path;

    #[test]
    fn transaction_repository_round_trips_lists_and_removes_records() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository =
            JsonReinstallRecoveryTransactionRepository::new(temp.path().join("reinstall-recovery"));
        let transaction = sample_transaction(ReinstallRecoveryTransactionStatus::Planned);

        repository
            .save_transaction(&transaction)
            .expect("save transaction");

        assert_eq!(
            repository
                .load_transaction(&ProfileId::new("default"), &ModId::new("mod-a"))
                .expect("load transaction"),
            Some(transaction.clone())
        );
        assert_eq!(
            repository
                .list_transactions(&ProfileId::new("default"))
                .expect("list transactions"),
            vec![transaction]
        );

        repository
            .remove_transaction(&ProfileId::new("default"), &ModId::new("mod-a"))
            .expect("remove transaction");
        assert!(repository
            .load_transaction(&ProfileId::new("default"), &ModId::new("mod-a"))
            .expect("load removed transaction")
            .is_none());
    }

    #[test]
    fn transaction_atomic_write_faults_preserve_previous_json_and_remove_temp_files() {
        for fault in [
            AtomicWriteFailurePoint::TempWrite,
            AtomicWriteFailurePoint::TempSync,
            AtomicWriteFailurePoint::Rename,
        ] {
            let temp = tempfile::tempdir().expect("temp dir");
            let root = temp.path().join("reinstall-recovery");
            let repository = JsonReinstallRecoveryTransactionRepository::new(root.clone());
            let original = sample_transaction(ReinstallRecoveryTransactionStatus::Planned);
            repository
                .save_transaction(&original)
                .expect("seed transaction");

            let failing = JsonReinstallRecoveryTransactionRepository::with_atomic_write_failure(
                root.clone(),
                fault,
            );
            let updated = sample_transaction(ReinstallRecoveryTransactionStatus::Committing);
            failing
                .save_transaction(&updated)
                .expect_err("injected atomic write failure must propagate");

            assert_eq!(
                repository
                    .load_transaction(&ProfileId::new("default"), &ModId::new("mod-a"))
                    .expect("reload original transaction"),
                Some(original)
            );
            assert_eq!(
                fs::read_dir(&root)
                    .expect("read recovery root")
                    .filter_map(Result::ok)
                    .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
                    .count(),
                0
            );
        }
    }

    #[test]
    fn transaction_repository_rejects_invalid_snapshot_ownership_before_writing() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().join("reinstall-recovery");
        let repository = JsonReinstallRecoveryTransactionRepository::new(root.clone());
        let mut transaction = sample_transaction(ReinstallRecoveryTransactionStatus::Planned);
        transaction.targets[1].snapshot = ReinstallSnapshotState::Stored {
            snapshot_ref: "snapshot-added".to_owned(),
            purpose: ReinstallSnapshotPurpose::TransactionRollback,
            cleanup_owner: ReinstallSnapshotCleanupOwner::Transaction,
        };

        repository
            .save_transaction(&transaction)
            .expect_err("added target cannot use transaction rollback ownership");

        assert!(!root.exists());
    }

    #[test]
    fn snapshot_store_rejects_absolute_traversal_and_symlink_refs() {
        let temp = tempfile::tempdir().expect("temp dir");
        let backup_root = temp.path().join("backups");
        let store = FileSystemInstallBackupStore::new(backup_root.clone());

        assert!(store.read_snapshot("../outside.bin").is_err());
        assert!(store.read_snapshot("C:/outside.bin").is_err());

        fs::create_dir_all(&backup_root).expect("backup root");
        let outside = temp.path().join("outside.bin");
        fs::write(&outside, b"outside").expect("outside file");
        let link = backup_root.join("snapshot-link");
        if try_create_file_symlink(&outside, &link) {
            assert!(store.read_snapshot("snapshot-link").is_err());
            assert!(store.remove_snapshot("snapshot-link").is_err());
            assert_eq!(fs::read(outside).expect("outside remains"), b"outside");
        }
    }

    fn sample_transaction(
        status: ReinstallRecoveryTransactionStatus,
    ) -> ReinstallRecoveryTransaction {
        let retained = target("retained.bin");
        ReinstallRecoveryTransaction {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            old_revision_id: ModRevisionId::new("v1"),
            candidate_revision_id: ModRevisionId::new("v2"),
            plan_token: "preview-token".to_owned(),
            plan_hash: "sha256:plan".to_owned(),
            status,
            pre_reinstall_manifest: InstallManifest::completed(
                ProfileId::new("default"),
                vec![InstallManifestEntry {
                    target_path: retained.clone(),
                    mod_id: ModId::new("mod-a"),
                    revision_id: None,
                    package_file_id: PackageFileId::new("retained-v1"),
                    layer: FileLayer::new("base", 0),
                    backup_ref: Some("original-retained".to_owned()),
                    installed_file: Some(summary("same")),
                }],
            ),
            candidate_replacement_bindings: Vec::new(),
            targets: vec![
                ReinstallRecoveryTarget {
                    target_path: retained,
                    class: ReinstallTargetClass::Retained,
                    pre_state: Some(summary("same")),
                    candidate_state: Some(summary("same")),
                    snapshot: ReinstallSnapshotState::NotRequired,
                    original_backup_ref: Some("original-retained".to_owned()),
                },
                ReinstallRecoveryTarget {
                    target_path: target("added.bin"),
                    class: ReinstallTargetClass::Added,
                    pre_state: Some(summary("unmanaged")),
                    candidate_state: Some(summary("candidate")),
                    snapshot: ReinstallSnapshotState::Stored {
                        snapshot_ref: "snapshot-added".to_owned(),
                        purpose: ReinstallSnapshotPurpose::OriginalBackupCandidate,
                        cleanup_owner: ReinstallSnapshotCleanupOwner::PromoteOnCommit,
                    },
                    original_backup_ref: None,
                },
            ],
        }
    }

    fn target(path: &str) -> InstallTargetPath {
        InstallTargetPath::parse(format!("content/{path}"), ["content"]).expect("target")
    }

    fn summary(hash: &str) -> InstalledFileSummary {
        InstalledFileSummary {
            size_bytes: hash.len() as u64,
            sha256: hash.to_owned(),
        }
    }

    #[cfg(unix)]
    fn try_create_file_symlink(original: impl AsRef<Path>, link: impl AsRef<Path>) -> bool {
        std::os::unix::fs::symlink(original, link).is_ok()
    }

    #[cfg(windows)]
    fn try_create_file_symlink(original: impl AsRef<Path>, link: impl AsRef<Path>) -> bool {
        std::os::windows::fs::symlink_file(original, link).is_ok()
    }
}
