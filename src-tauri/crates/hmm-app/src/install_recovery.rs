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

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallRecoveryScanError {
    #[error("game instance is unavailable")]
    GameInstanceUnavailable,
    #[error("install recovery scan failed")]
    ManifestUnavailable,
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

            Ok(self
                .files
                .lock()
                .expect("files lock")
                .get(target_path.as_str())
                .cloned())
        }

        fn write_game_file(
            &self,
            _target_path: &InstallTargetPath,
            _bytes: &[u8],
        ) -> anyhow::Result<()> {
            panic!("recovery scan must be read-only")
        }

        fn remove_game_file(&self, _target_path: &InstallTargetPath) -> anyhow::Result<()> {
            panic!("recovery scan must be read-only")
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
            manifest: Some(InstallManifest {
                profile_id: ProfileId::new("default"),
                entries: vec![
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
            }),
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
            manifest: Some(InstallManifest {
                profile_id: ProfileId::new("default"),
                entries: vec![InstallManifestEntry {
                    target_path: target,
                    mod_id: ModId::new("mod-a"),
                    package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
                    layer: FileLayer::new("base", 0),
                    backup_ref: Some("backup-original".to_owned()),
                    installed_file: Some(summary(&modded_bytes)),
                }],
            }),
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
            manifest: Some(InstallManifest {
                profile_id: ProfileId::new("default"),
                entries: vec![
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
            }),
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
            manifest: Some(InstallManifest {
                profile_id: ProfileId::new("default"),
                entries: vec![InstallManifestEntry {
                    target_path: target,
                    mod_id: ModId::new("mod-a"),
                    package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
                    layer: FileLayer::new("base", 0),
                    backup_ref: None,
                    installed_file: Some(summary(&modded_bytes)),
                }],
            }),
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
            manifest: Some(InstallManifest {
                profile_id: ProfileId::new("default"),
                entries: vec![InstallManifestEntry {
                    target_path: target,
                    mod_id: ModId::new("mod-a"),
                    package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
                    layer: FileLayer::new("base", 0),
                    backup_ref: Some("backup-original".to_owned()),
                    installed_file: Some(summary(&modded_bytes)),
                }],
            }),
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
            manifest: Some(InstallManifest {
                profile_id: ProfileId::new("default"),
                entries: vec![
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
            }),
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
