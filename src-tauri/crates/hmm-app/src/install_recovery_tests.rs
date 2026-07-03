use super::*;
use hmm_core::{
    FileLayer, InstallManifest, InstallManifestEntry, InstallManifestStatus, InstallRecoveryRecord,
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

fn scan_status_for_manifest_status(
    manifest_status: InstallManifestStatus,
) -> InstallRecoverySummary {
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
    let mut manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: target,
            mod_id: ModId::new("mod-a"),
            package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: Some(summary(&modded_bytes)),
        }],
    );
    manifest.status = manifest_status;
    let manifests = Arc::new(FakeManifests {
        manifest: Some(manifest),
    });
    let service = InstallRecoveryScanService::new(game_files, backups, manifests);

    let summaries = service
        .scan(InstallRecoveryScanRequest {
            profile_id: ProfileId::new("default"),
            mod_ids: vec![ModId::new("mod-a")],
        })
        .expect("scan should succeed");

    summaries[0].clone()
}

#[test]
fn scan_reports_rollback_required_when_manifest_status_requires_rollback() {
    let summary = scan_status_for_manifest_status(InstallManifestStatus::RollbackRequired);

    assert_eq!(summary.status, InstallRecoveryStatus::RollbackRequired);
    assert_eq!(summary.managed_file_count, 1);
    assert_eq!(summary.issue_count, 0);
    assert!(summary.issues.is_empty());
}

#[test]
fn scan_reports_repair_required_when_manifest_status_requires_repair() {
    let summary = scan_status_for_manifest_status(InstallManifestStatus::RepairRequired);

    assert_eq!(summary.status, InstallRecoveryStatus::RepairRequired);
    assert_eq!(summary.issue_count, 0);
}

#[test]
fn scan_reports_unknown_while_manifest_commit_is_in_flight() {
    assert_eq!(
        scan_status_for_manifest_status(InstallManifestStatus::Planned).status,
        InstallRecoveryStatus::Unknown
    );
    assert_eq!(
        scan_status_for_manifest_status(InstallManifestStatus::Committing).status,
        InstallRecoveryStatus::Unknown
    );
}

#[test]
fn scan_trusts_file_checks_for_remaining_mods_when_manifest_was_rolled_back() {
    let summary = scan_status_for_manifest_status(InstallManifestStatus::RolledBack);

    assert_eq!(summary.status, InstallRecoveryStatus::Completed);
    assert_eq!(summary.issue_count, 0);
}
