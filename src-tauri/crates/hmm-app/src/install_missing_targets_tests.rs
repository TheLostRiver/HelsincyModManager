use super::*;
use crate::{InstallRecoveryScanRequest, InstallRecoveryScanService, InstallRecoveryStatus};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

const MISSING: &str = "nativePC/missing.bin";
const PRESENT: &str = "nativePC/present.bin";
const OTHER: &str = "nativePC/other.bin";
const MOD_BYTES: &[u8] = b"fixture Mod content";
const ORIGINAL: &[u8] = b"fixture original content";

#[derive(Default)]
struct Files {
    bytes: Mutex<BTreeMap<String, Vec<u8>>>,
    reads: AtomicUsize,
    fail_read_at: Mutex<Option<usize>>,
    writes: Mutex<Vec<String>>,
    removals: Mutex<Vec<String>>,
}

impl Files {
    fn put(&self, path: &str, bytes: &[u8]) {
        self.bytes
            .lock()
            .unwrap()
            .insert(path.to_owned(), bytes.to_vec());
    }
    fn get(&self, path: &str) -> Option<Vec<u8>> {
        self.bytes.lock().unwrap().get(path).cloned()
    }
    fn assert_untouched(&self) {
        assert!(self.writes.lock().unwrap().is_empty());
        assert!(self.removals.lock().unwrap().is_empty());
    }
}

impl InstallGameFileSystem for Files {
    fn read_game_file(&self, target: &InstallTargetPath) -> anyhow::Result<Option<Vec<u8>>> {
        let count = self.reads.fetch_add(1, Ordering::SeqCst) + 1;
        if *self.fail_read_at.lock().unwrap() == Some(count) {
            anyhow::bail!("fixture read error");
        }
        Ok(self.get(target.as_str()))
    }
    fn write_game_file(&self, target: &InstallTargetPath, bytes: &[u8]) -> anyhow::Result<()> {
        self.writes.lock().unwrap().push(target.as_str().to_owned());
        self.put(target.as_str(), bytes);
        Ok(())
    }
    fn remove_game_file(&self, target: &InstallTargetPath) -> anyhow::Result<()> {
        self.removals
            .lock()
            .unwrap()
            .push(target.as_str().to_owned());
        self.bytes.lock().unwrap().remove(target.as_str());
        Ok(())
    }
}

#[derive(Default)]
struct Backups {
    bytes: Mutex<BTreeMap<String, Vec<u8>>>,
    fail_reads: AtomicBool,
    removals: Mutex<Vec<String>>,
}

impl InstallBackupStore for Backups {
    fn store_backup(&self, _target: &InstallTargetPath, _bytes: &[u8]) -> anyhow::Result<String> {
        anyhow::bail!("uninstall must not create backups")
    }
    fn read_backup(&self, reference: &str) -> anyhow::Result<Option<Vec<u8>>> {
        if self.fail_reads.load(Ordering::SeqCst) {
            anyhow::bail!("fixture backup read error");
        }
        Ok(self.bytes.lock().unwrap().get(reference).cloned())
    }
    fn remove_backup(&self, reference: &str) -> anyhow::Result<()> {
        self.removals.lock().unwrap().push(reference.to_owned());
        self.bytes.lock().unwrap().remove(reference);
        Ok(())
    }
}

struct Manifests {
    value: Mutex<InstallManifest>,
    fail_save: AtomicBool,
    replace_before_failure: Mutex<Option<ExternalFileChange>>,
}

struct ExternalFileChange {
    files: Arc<Files>,
    path: String,
    bytes: Vec<u8>,
}

impl InstallManifestRepository for Manifests {
    fn load_manifest(&self, _profile: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        Ok(Some(self.value.lock().unwrap().clone()))
    }
    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        if self.fail_save.load(Ordering::SeqCst) {
            if let Some(change) = self.replace_before_failure.lock().unwrap().take() {
                change.files.put(&change.path, &change.bytes);
            }
            anyhow::bail!("fixture manifest save error");
        }
        manifest
            .validate()
            .map_err(|error| anyhow::anyhow!("invalid fixture manifest: {error:?}"))?;
        *self.value.lock().unwrap() = manifest.clone();
        Ok(())
    }
}

fn entry(mod_id: &str, path: &str, backup: Option<&str>) -> InstallManifestEntry {
    InstallManifestEntry {
        target_path: InstallTargetPath::parse(path, ["nativePC"]).unwrap(),
        mod_id: ModId::new(mod_id),
        revision_id: None,
        package_file_id: PackageFileId::new(path),
        layer: FileLayer::new("base", 0),
        backup_ref: backup.map(str::to_owned),
        installed_file: Some(installed_file_summary(MOD_BYTES)),
        adopted: false,
    }
}

struct Fixture {
    files: Arc<Files>,
    backups: Arc<Backups>,
    manifests: Arc<Manifests>,
    service: UninstallModService,
}

impl Fixture {
    fn missing(with_backup: bool) -> Self {
        let files = Arc::new(Files::default());
        files.put(OTHER, MOD_BYTES);
        let backups = Arc::new(Backups::default());
        if with_backup {
            backups
                .bytes
                .lock()
                .unwrap()
                .insert("original-a".to_owned(), ORIGINAL.to_vec());
        }
        let manifests = Arc::new(Manifests {
            value: Mutex::new(InstallManifest::completed(
                ProfileId::new("profile"),
                vec![
                    entry("mod-a", MISSING, with_backup.then_some("original-a")),
                    entry("mod-b", OTHER, None),
                ],
            )),
            fail_save: AtomicBool::new(false),
            replace_before_failure: Mutex::new(None),
        });
        let service = UninstallModService::new(files.clone(), backups.clone(), manifests.clone());
        Self {
            files,
            backups,
            manifests,
            service,
        }
    }
    fn request(&self) -> UninstallModRequest {
        UninstallModRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("profile"),
            mod_id: ModId::new("mod-a"),
        }
    }
    fn add_present(&self) {
        self.files.put(PRESENT, MOD_BYTES);
        self.manifests
            .value
            .lock()
            .unwrap()
            .entries
            .push(entry("mod-a", PRESENT, None));
    }
    fn preview(&self) -> MissingTargetUninstallPreview {
        self.service
            .preview_missing_target_uninstall(&self.request())
            .unwrap()
    }
    fn execute(&self, token: &str) -> Result<UninstallModResult, UninstallModError> {
        self.service
            .uninstall_missing_targets(self.request(), token)
    }
    fn scan(&self) -> Vec<crate::InstallRecoverySummary> {
        InstallRecoveryScanService::new(
            self.files.clone(),
            self.backups.clone(),
            self.manifests.clone(),
        )
        .scan(InstallRecoveryScanRequest {
            profile_id: ProfileId::new("profile"),
            mod_ids: vec![ModId::new("mod-a"), ModId::new("mod-b")],
        })
        .unwrap()
    }
}

#[test]
fn missing_file_repair_clears_only_owned_records_without_game_writes() {
    let fixture = Fixture::missing(false);
    assert_eq!(
        fixture.scan()[0].status,
        InstallRecoveryStatus::RepairRequired
    );
    assert_eq!(
        fixture.service.uninstall_mod(fixture.request()),
        Err(UninstallModError::TargetStateMismatch)
    );
    let preview = fixture.preview();
    assert_eq!(
        (
            preview.remove_file_count,
            preview.restore_file_count,
            preview.missing_file_count
        ),
        (0, 0, 1)
    );
    let result = fixture.execute(&preview.plan_token).unwrap();
    assert_eq!(result.manifest.entries, vec![entry("mod-b", OTHER, None)]);
    fixture.files.assert_untouched();
    assert_eq!(fixture.files.get(OTHER), Some(MOD_BYTES.to_vec()));
    assert_eq!(
        fixture
            .scan()
            .iter()
            .map(|summary| summary.status)
            .collect::<Vec<_>>(),
        vec![
            InstallRecoveryStatus::NotInstalled,
            InstallRecoveryStatus::Completed
        ]
    );
}

#[test]
fn mixed_missing_and_present_files_uninstall_together() {
    let fixture = Fixture::missing(false);
    fixture.add_present();
    let preview = fixture.preview();
    assert_eq!(
        (preview.remove_file_count, preview.missing_file_count),
        (1, 1)
    );
    let result = fixture.execute(&preview.plan_token).unwrap();
    assert_eq!(result.removed_file_count, 1);
    assert_eq!(*fixture.files.removals.lock().unwrap(), vec![PRESENT]);
    assert_eq!(fixture.files.get(OTHER), Some(MOD_BYTES.to_vec()));
}

#[test]
fn missing_overwritten_file_restores_its_backup() {
    let fixture = Fixture::missing(true);
    let preview = fixture.preview();
    assert_eq!(
        (
            preview.remove_file_count,
            preview.restore_file_count,
            preview.backup_count,
            preview.missing_file_count
        ),
        (0, 1, 1, 1)
    );
    let result = fixture.execute(&preview.plan_token).unwrap();
    assert_eq!(result.restored_file_count, 1);
    assert_eq!(fixture.files.get(MISSING), Some(ORIGINAL.to_vec()));
    assert_eq!(
        *fixture.backups.removals.lock().unwrap(),
        vec!["original-a"]
    );
}

#[test]
fn changed_or_unreadable_targets_never_become_missing_targets() {
    let changed = Fixture::missing(false);
    changed.add_present();
    changed.files.put(PRESENT, b"foreign content");
    assert!(changed
        .service
        .preview_missing_target_uninstall(&changed.request())
        .is_err());
    changed.files.assert_untouched();
    let unreadable = Fixture::missing(false);
    *unreadable.files.fail_read_at.lock().unwrap() = Some(1);
    assert!(unreadable
        .service
        .preview_missing_target_uninstall(&unreadable.request())
        .is_err());
    unreadable.files.assert_untouched();
}

#[test]
fn absent_target_revalidation_does_not_flatten_a_read_error_into_absence() {
    let fixture = Fixture::missing(false);
    let preview = fixture.preview();
    *fixture.files.fail_read_at.lock().unwrap() =
        Some(fixture.files.reads.load(Ordering::SeqCst) + 2);
    assert_eq!(
        fixture.execute(&preview.plan_token),
        Err(UninstallModError::TargetStateMismatch)
    );
    fixture.files.assert_untouched();
    assert_eq!(fixture.manifests.value.lock().unwrap().entries.len(), 2);
}

#[test]
fn missing_or_unreadable_backups_block_before_changes() {
    for unreadable in [false, true] {
        let fixture = Fixture::missing(true);
        if unreadable {
            fixture.backups.fail_reads.store(true, Ordering::SeqCst);
        } else {
            fixture.backups.bytes.lock().unwrap().clear();
        }
        assert_eq!(
            fixture
                .service
                .preview_missing_target_uninstall(&fixture.request()),
            Err(UninstallModError::BackupUnavailable)
        );
        fixture.files.assert_untouched();
        assert!(fixture.backups.removals.lock().unwrap().is_empty());
    }
}

#[test]
fn preview_token_binds_manifest_and_backup_content() {
    let manifest = Fixture::missing(false);
    let preview = manifest.preview();
    manifest.manifests.value.lock().unwrap().entries[0]
        .layer
        .name = "changed-layer".to_owned();
    assert_eq!(
        manifest.execute(&preview.plan_token),
        Err(UninstallModError::ManifestStateMismatch)
    );
    manifest.files.assert_untouched();
    let backup = Fixture::missing(true);
    let preview = backup.preview();
    backup
        .backups
        .bytes
        .lock()
        .unwrap()
        .insert("original-a".to_owned(), b"different backup".to_vec());
    assert_eq!(
        backup.execute(&preview.plan_token),
        Err(UninstallModError::ManifestStateMismatch)
    );
    backup.files.assert_untouched();
}

#[test]
fn a_reappearing_target_requires_a_new_preview() {
    let fixture = Fixture::missing(false);
    fixture
        .manifests
        .value
        .lock()
        .unwrap()
        .entries
        .push(entry("mod-a", PRESENT, None));
    let preview = fixture.preview();
    fixture.files.put(MISSING, MOD_BYTES);
    assert_eq!(
        fixture.execute(&preview.plan_token),
        Err(UninstallModError::ManifestStateMismatch)
    );
    fixture.files.assert_untouched();
}

#[test]
fn manifest_failure_rolls_back_to_the_original_mix_of_absence_and_contents() {
    let fixture = Fixture::missing(true);
    fixture.add_present();
    let preview = fixture.preview();
    let before = fixture.manifests.value.lock().unwrap().clone();
    fixture.manifests.fail_save.store(true, Ordering::SeqCst);
    assert_eq!(
        fixture.execute(&preview.plan_token),
        Err(UninstallModError::ManifestSaveFailed)
    );
    assert_eq!(fixture.files.get(MISSING), None);
    assert_eq!(fixture.files.get(PRESENT), Some(MOD_BYTES.to_vec()));
    assert_eq!(*fixture.manifests.value.lock().unwrap(), before);
    assert!(fixture.backups.removals.lock().unwrap().is_empty());
}

#[test]
fn rollback_preserves_foreign_content_that_replaced_a_restored_file() {
    let fixture = Fixture::missing(true);
    let preview = fixture.preview();
    fixture.manifests.fail_save.store(true, Ordering::SeqCst);
    *fixture.manifests.replace_before_failure.lock().unwrap() = Some(ExternalFileChange {
        files: fixture.files.clone(),
        path: MISSING.to_owned(),
        bytes: b"foreign content".to_vec(),
    });
    assert_eq!(
        fixture.execute(&preview.plan_token),
        Err(UninstallModError::RollbackFailed {
            failed_phase: UninstallModPhase::ManifestSave
        })
    );
    assert_eq!(
        fixture.files.get(MISSING),
        Some(b"foreign content".to_vec())
    );
    assert!(fixture.files.removals.lock().unwrap().is_empty());
    assert!(fixture.backups.removals.lock().unwrap().is_empty());
}

#[test]
fn untrusted_or_mismatched_manifests_do_not_authorize_repair() {
    for status in [
        InstallManifestStatus::Committing,
        InstallManifestStatus::RepairRequired,
        InstallManifestStatus::RollbackRequired,
    ] {
        let fixture = Fixture::missing(false);
        fixture.manifests.value.lock().unwrap().status = status;
        assert_eq!(
            fixture
                .service
                .preview_missing_target_uninstall(&fixture.request()),
            Err(UninstallModError::ManifestStateMismatch)
        );
        fixture.files.assert_untouched();
    }
    let fixture = Fixture::missing(false);
    fixture.manifests.value.lock().unwrap().profile_id = ProfileId::new("different");
    assert!(fixture
        .service
        .preview_missing_target_uninstall(&fixture.request())
        .is_err());
    fixture.files.assert_untouched();
}

#[test]
fn conflicting_ownership_and_empty_missing_sets_are_rejected() {
    let collision = Fixture::missing(false);
    collision
        .manifests
        .value
        .lock()
        .unwrap()
        .entries
        .push(entry("another-mod", MISSING, None));
    assert!(collision
        .service
        .preview_missing_target_uninstall(&collision.request())
        .is_err());
    collision.files.assert_untouched();
    let intact = Fixture::missing(false);
    intact.files.put(MISSING, MOD_BYTES);
    assert!(intact
        .service
        .preview_missing_target_uninstall(&intact.request())
        .is_err());
    intact.files.assert_untouched();
}

#[test]
fn case_colliding_manifest_targets_never_authorize_missing_target_cleanup() {
    for owner in ["mod-a", "other-mod"] {
        let fixture = Fixture::missing(false);
        fixture.manifests.value.lock().unwrap().entries.push(entry(
            owner,
            "nativePC/MISSING.BIN",
            None,
        ));
        assert_eq!(
            fixture
                .service
                .preview_missing_target_uninstall(&fixture.request()),
            Err(UninstallModError::ManifestStateMismatch)
        );
        fixture.files.assert_untouched();
    }
}
