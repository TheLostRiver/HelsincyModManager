use hmm_app::{
    InstallRecoveryActionError, InstallRecoveryActionKind, InstallRecoveryActionRequest,
    InstallRecoveryActionService,
};
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
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeGameFiles {
    files: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl InstallGameFileSystem for FakeGameFiles {
    fn read_game_file(&self, target_path: &InstallTargetPath) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self
            .files
            .lock()
            .expect("files lock")
            .get(target_path.as_str())
            .cloned())
    }

    fn write_game_file(&self, target_path: &InstallTargetPath, bytes: &[u8]) -> anyhow::Result<()> {
        self.files
            .lock()
            .expect("files lock")
            .insert(target_path.as_str().to_owned(), bytes.to_vec());
        Ok(())
    }

    fn remove_game_file(&self, target_path: &InstallTargetPath) -> anyhow::Result<()> {
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
}

impl InstallBackupStore for FakeBackups {
    fn store_backup(
        &self,
        _target_path: &InstallTargetPath,
        _bytes: &[u8],
    ) -> anyhow::Result<String> {
        panic!("recovery action tests should not create backups")
    }

    fn read_backup(&self, backup_ref: &str) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self
            .backups
            .lock()
            .expect("backups lock")
            .get(backup_ref)
            .cloned())
    }

    fn remove_backup(&self, _backup_ref: &str) -> anyhow::Result<()> {
        panic!("recovery action tests should not remove backups")
    }
}

struct RecordingManifests {
    manifest: Option<InstallManifest>,
    saved_manifest: Mutex<Option<InstallManifest>>,
    fail_saves: Mutex<bool>,
}

impl RecordingManifests {
    fn new(manifest: Option<InstallManifest>) -> Self {
        Self {
            manifest,
            saved_manifest: Mutex::new(None),
            fail_saves: Mutex::new(false),
        }
    }

    fn fail_saves(&self) {
        *self.fail_saves.lock().expect("fail saves lock") = true;
    }

    fn take_saved_manifest(&self) -> Option<InstallManifest> {
        self.saved_manifest
            .lock()
            .expect("saved manifest lock")
            .take()
    }
}

impl InstallManifestRepository for RecordingManifests {
    fn load_manifest(&self, _profile_id: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        Ok(self.manifest.clone())
    }

    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        if *self.fail_saves.lock().expect("fail saves lock") {
            anyhow::bail!("simulated manifest save failure");
        }
        *self.saved_manifest.lock().expect("saved manifest lock") = Some(manifest.clone());
        Ok(())
    }
}

#[derive(Default)]
struct RecordingRecoveryRecords {
    records: Mutex<BTreeMap<String, InstallRecoveryRecord>>,
    fail_saves: Mutex<bool>,
}

impl RecordingRecoveryRecords {
    fn insert(&self, record: InstallRecoveryRecord) {
        self.records
            .lock()
            .expect("records lock")
            .insert(record_key(&record.profile_id, &record.mod_id), record);
    }

    fn fail_saves(&self) {
        *self.fail_saves.lock().expect("fail saves lock") = true;
    }
}

impl InstallRecoveryRecordRepository for RecordingRecoveryRecords {
    fn load_record(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> anyhow::Result<Option<InstallRecoveryRecord>> {
        Ok(self
            .records
            .lock()
            .expect("records lock")
            .get(&record_key(profile_id, mod_id))
            .cloned())
    }

    fn list_records(&self, profile_id: &ProfileId) -> anyhow::Result<Vec<InstallRecoveryRecord>> {
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
        Ok(())
    }
}

#[test]
fn rollback_action_persists_manifest_rolled_back_without_stale_mod_entries() {
    let rolled_back_target = target("nativePC/models/rolled-back.mod3");
    let kept_target = target("nativePC/models/keep.mod3");
    let rolled_back_bytes = b"rolled back modded model".to_vec();
    let kept_bytes = b"kept modded model".to_vec();
    let game_files = Arc::new(FakeGameFiles::default());
    insert_game_file(&game_files, &rolled_back_target, &rolled_back_bytes);
    insert_game_file(&game_files, &kept_target, &kept_bytes);
    let backups = Arc::new(FakeBackups::default());
    let recovery_records = Arc::new(RecordingRecoveryRecords::default());
    recovery_records.insert(recovery_record(
        InstallRecoveryRecordStatus::RollbackRequired,
        rolled_back_target.clone(),
        ModId::new("mod-a"),
        Some(summary(&rolled_back_bytes)),
        None,
    ));
    let manifests = Arc::new(RecordingManifests::new(Some(
        InstallManifest::completed_with_metadata(
            ProfileId::new("default"),
            vec![
                manifest_entry("mod-a", rolled_back_target.clone(), &rolled_back_bytes),
                manifest_entry("mod-b", kept_target.clone(), &kept_bytes),
            ],
            Some("install_plan".to_owned()),
            Some("2026-06-29T00:00:00Z".to_owned()),
            Some("2026-06-29T00:00:01Z".to_owned()),
            Some("sha256:existing-plan".to_owned()),
        ),
    )));
    let service = InstallRecoveryActionService::new_with_manifest(
        game_files.clone(),
        backups,
        recovery_records,
        manifests.clone(),
    );

    service
        .run(rollback_request("mod-a"))
        .expect("rollback action should persist manifest status");

    let manifest = manifests
        .take_saved_manifest()
        .expect("rolled back manifest should be saved");
    assert_eq!(manifest.status, InstallManifestStatus::RolledBack);
    assert_eq!(manifest.backend.as_deref(), Some("install_plan"));
    assert_eq!(manifest.created_at.as_deref(), Some("2026-06-29T00:00:00Z"));
    assert_eq!(manifest.plan_hash.as_deref(), Some("sha256:existing-plan"));
    assert_eq!(manifest.entries.len(), 1);
    assert_eq!(manifest.entries[0].mod_id.as_str(), "mod-b");
    assert_eq!(manifest.entries[0].target_path, kept_target);
    assert!(game_file(&game_files, &rolled_back_target).is_none());
    assert_eq!(game_file(&game_files, &kept_target), Some(kept_bytes));
}

#[test]
fn rollback_action_rolls_back_removed_file_when_manifest_save_fails() {
    let target = target("nativePC/models/new-file.mod3");
    let installed_bytes = b"installed modded model".to_vec();
    let game_files = Arc::new(FakeGameFiles::default());
    insert_game_file(&game_files, &target, &installed_bytes);
    let backups = Arc::new(FakeBackups::default());
    let recovery_records = Arc::new(RecordingRecoveryRecords::default());
    recovery_records.insert(recovery_record(
        InstallRecoveryRecordStatus::RollbackRequired,
        target.clone(),
        ModId::new("mod-a"),
        Some(summary(&installed_bytes)),
        None,
    ));
    let manifests = Arc::new(RecordingManifests::new(Some(InstallManifest::completed(
        ProfileId::new("default"),
        vec![manifest_entry("mod-a", target.clone(), &installed_bytes)],
    ))));
    manifests.fail_saves();
    let service = InstallRecoveryActionService::new_with_manifest(
        game_files.clone(),
        backups,
        recovery_records.clone(),
        manifests.clone(),
    );

    let error = service
        .run(rollback_request("mod-a"))
        .expect_err("manifest save failure should fail the recovery action");

    assert_eq!(error, InstallRecoveryActionError::ManifestSaveFailed);
    assert_eq!(game_file(&game_files, &target), Some(installed_bytes));
    assert!(manifests.take_saved_manifest().is_none());
    assert_eq!(
        recovery_record_status(&recovery_records, "mod-a"),
        Some(InstallRecoveryRecordStatus::RollbackRequired)
    );
}

#[test]
fn rollback_action_rolls_back_committing_record() {
    let target = target("nativePC/models/new-file.mod3");
    let installed_bytes = b"installed modded model".to_vec();
    let game_files = Arc::new(FakeGameFiles::default());
    insert_game_file(&game_files, &target, &installed_bytes);
    let backups = Arc::new(FakeBackups::default());
    let recovery_records = Arc::new(RecordingRecoveryRecords::default());
    recovery_records.insert(recovery_record(
        InstallRecoveryRecordStatus::Committing,
        target.clone(),
        ModId::new("mod-a"),
        Some(summary(&installed_bytes)),
        None,
    ));
    let service =
        InstallRecoveryActionService::new(game_files.clone(), backups, recovery_records.clone());

    let result = service
        .run(rollback_request("mod-a"))
        .expect("committing record should roll back");

    assert_eq!(result.remove_file_count, 1);
    assert!(game_file(&game_files, &target).is_none());
    assert_eq!(
        recovery_record_status(&recovery_records, "mod-a"),
        Some(InstallRecoveryRecordStatus::RolledBack)
    );
}

#[test]
fn rollback_action_without_manifest_rolls_back_removed_file_when_record_save_fails() {
    let target = target("nativePC/models/new-file.mod3");
    let installed_bytes = b"installed modded model".to_vec();
    let game_files = Arc::new(FakeGameFiles::default());
    insert_game_file(&game_files, &target, &installed_bytes);
    let backups = Arc::new(FakeBackups::default());
    let recovery_records = Arc::new(RecordingRecoveryRecords::default());
    recovery_records.insert(recovery_record(
        InstallRecoveryRecordStatus::RollbackRequired,
        target.clone(),
        ModId::new("mod-a"),
        Some(summary(&installed_bytes)),
        None,
    ));
    recovery_records.fail_saves();
    let service =
        InstallRecoveryActionService::new(game_files.clone(), backups, recovery_records.clone());

    let error = service
        .run(rollback_request("mod-a"))
        .expect_err("record save failure should fail the recovery action");

    assert_eq!(error, InstallRecoveryActionError::RecoveryRecordSaveFailed);
    assert_eq!(game_file(&game_files, &target), Some(installed_bytes));
    assert_eq!(
        recovery_record_status(&recovery_records, "mod-a"),
        Some(InstallRecoveryRecordStatus::RollbackRequired)
    );
}

#[test]
fn rollback_action_restores_manifest_when_record_save_fails() {
    let target = target("nativePC/models/new-file.mod3");
    let installed_bytes = b"installed modded model".to_vec();
    let original_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![manifest_entry("mod-a", target.clone(), &installed_bytes)],
    );
    let game_files = Arc::new(FakeGameFiles::default());
    insert_game_file(&game_files, &target, &installed_bytes);
    let backups = Arc::new(FakeBackups::default());
    let recovery_records = Arc::new(RecordingRecoveryRecords::default());
    recovery_records.insert(recovery_record(
        InstallRecoveryRecordStatus::RollbackRequired,
        target.clone(),
        ModId::new("mod-a"),
        Some(summary(&installed_bytes)),
        None,
    ));
    recovery_records.fail_saves();
    let manifests = Arc::new(RecordingManifests::new(Some(original_manifest.clone())));
    let service = InstallRecoveryActionService::new_with_manifest(
        game_files.clone(),
        backups,
        recovery_records.clone(),
        manifests.clone(),
    );

    let error = service
        .run(rollback_request("mod-a"))
        .expect_err("record save failure should fail the recovery action");

    assert_eq!(error, InstallRecoveryActionError::RecoveryRecordSaveFailed);
    assert_eq!(game_file(&game_files, &target), Some(installed_bytes));
    assert_eq!(
        manifests.take_saved_manifest(),
        Some(original_manifest),
        "failed recovery record save must restore the previous manifest"
    );
    assert_eq!(
        recovery_record_status(&recovery_records, "mod-a"),
        Some(InstallRecoveryRecordStatus::RollbackRequired)
    );
}

fn rollback_request(mod_id: &str) -> InstallRecoveryActionRequest {
    InstallRecoveryActionRequest {
        profile_id: ProfileId::new("default"),
        mod_id: ModId::new(mod_id),
        action_kind: InstallRecoveryActionKind::RollbackInstall,
    }
}

fn target(path: &str) -> InstallTargetPath {
    InstallTargetPath::parse(path, ["nativePC"]).expect("target path")
}

fn manifest_entry(
    mod_id: &str,
    target_path: InstallTargetPath,
    bytes: &[u8],
) -> InstallManifestEntry {
    InstallManifestEntry {
        package_file_id: PackageFileId::new(target_path.as_str()),
        target_path,
        mod_id: ModId::new(mod_id),
        layer: FileLayer::new("base", 0),
        backup_ref: None,
        installed_file: Some(summary(bytes)),
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
        mod_id,
        status,
        entries: vec![InstallRecoveryRecordEntry {
            target_path,
            package_file_id: PackageFileId::new("nativePC/models/new-file.mod3"),
            backup_ref,
            installed_file,
        }],
    }
}

fn insert_game_file(files: &FakeGameFiles, target_path: &InstallTargetPath, bytes: &[u8]) {
    files
        .files
        .lock()
        .expect("files lock")
        .insert(target_path.as_str().to_owned(), bytes.to_vec());
}

fn game_file(files: &FakeGameFiles, target_path: &InstallTargetPath) -> Option<Vec<u8>> {
    files
        .files
        .lock()
        .expect("files lock")
        .get(target_path.as_str())
        .cloned()
}

fn recovery_record_status(
    records: &RecordingRecoveryRecords,
    mod_id: &str,
) -> Option<InstallRecoveryRecordStatus> {
    records
        .load_record(&ProfileId::new("default"), &ModId::new(mod_id))
        .expect("record load")
        .map(|record| record.status)
}

fn record_key(profile_id: &ProfileId, mod_id: &ModId) -> String {
    format!("{}:{}", profile_id.as_str(), mod_id.as_str())
}

fn summary(bytes: &[u8]) -> InstalledFileSummary {
    let digest = Sha256::digest(bytes);
    InstalledFileSummary {
        size_bytes: bytes.len() as u64,
        sha256: digest.iter().map(|byte| format!("{byte:02x}")).collect(),
    }
}
