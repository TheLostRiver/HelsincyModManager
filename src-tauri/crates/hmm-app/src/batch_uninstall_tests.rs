use super::*;
use crate::install::UninstallModPhase;
use crate::{UninstallModRequest, UninstallModService};
use hmm_core::{
    build_batch_plan, BatchExecutionPolicy, BatchId, BatchItemId, BatchItemPlan, BatchOperation,
    BatchPlan, BatchPlanRequest, BatchResourceLimits, BatchResourceUsage, BatchTargetWriteKind,
    FileLayer, GameId, InstallManifestEntry, InstallRecoveryRecord, InstallRecoveryRecordStatus,
    InstallTargetPath, InstalledFileSummary, ModRevisionId, PackageFileId, ProfileId,
    ReinstallRecoveryTransaction, ReinstallRecoveryTransactionStatus, ReplacementBinding,
    ReplacementBindingId, ReplacementBindingSnapshot, ReplacementSourceId, ReplacementTargetId,
    ReplacementTargetKind, SealedBatchItem, UninstallBatchItemInput, BATCH_PLAN_SCHEMA_VERSION,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, BatchPlanFactsProvider, InstallBackupStore,
    InstallGameFileSystem, InstallRecoveryRecordRepository, ReinstallRecoveryTransactionRepository,
    ReinstallSnapshotStore,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

#[derive(Default)]
struct FakeUninstallState {
    manifest: Mutex<Option<InstallManifest>>,
    manifest_read_error: AtomicBool,
    manifest_save_count: AtomicUsize,
    files: Mutex<BTreeMap<String, Vec<u8>>>,
    game_write_count: AtomicUsize,
    file_read_errors: Mutex<BTreeSet<String>>,
    backups: Mutex<BTreeMap<String, Vec<u8>>>,
    backup_read_errors: Mutex<BTreeSet<String>>,
    recovery_records: Mutex<BTreeMap<String, InstallRecoveryRecord>>,
    reinstall_transactions: Mutex<BTreeMap<String, ReinstallRecoveryTransaction>>,
}

impl FakeUninstallState {
    fn set_manifest(&self, manifest: InstallManifest) {
        *self.manifest.lock().expect("manifest") = Some(manifest);
    }

    fn add_file(&self, target: &str, bytes: &[u8]) {
        self.files
            .lock()
            .expect("files")
            .insert(target.to_ascii_lowercase(), bytes.to_vec());
    }

    fn add_backup(&self, backup_ref: &str, bytes: &[u8]) {
        self.backups
            .lock()
            .expect("backups")
            .insert(backup_ref.to_owned(), bytes.to_vec());
    }

    fn add_recovery_record(&self, record: InstallRecoveryRecord) {
        self.recovery_records
            .lock()
            .expect("recovery records")
            .insert(record.mod_id.as_str().to_owned(), record);
    }

    fn add_reinstall_transaction(&self, transaction: ReinstallRecoveryTransaction) {
        self.reinstall_transactions
            .lock()
            .expect("reinstall transactions")
            .insert(transaction.mod_id.as_str().to_owned(), transaction);
    }
}

impl InstallGameFileSystem for FakeUninstallState {
    fn read_game_file(&self, target_path: &InstallTargetPath) -> anyhow::Result<Option<Vec<u8>>> {
        if self
            .file_read_errors
            .lock()
            .expect("file read errors")
            .contains(&target_path.as_str().to_ascii_lowercase())
        {
            anyhow::bail!("injected file read failure");
        }
        Ok(self
            .files
            .lock()
            .expect("files")
            .get(&target_path.as_str().to_ascii_lowercase())
            .cloned())
    }

    fn write_game_file(&self, target_path: &InstallTargetPath, bytes: &[u8]) -> anyhow::Result<()> {
        self.game_write_count.fetch_add(1, Ordering::Relaxed);
        self.files
            .lock()
            .expect("files")
            .insert(target_path.as_str().to_ascii_lowercase(), bytes.to_vec());
        Ok(())
    }

    fn remove_game_file(&self, target_path: &InstallTargetPath) -> anyhow::Result<()> {
        self.game_write_count.fetch_add(1, Ordering::Relaxed);
        self.files
            .lock()
            .expect("files")
            .remove(&target_path.as_str().to_ascii_lowercase());
        Ok(())
    }
}

impl InstallBackupStore for FakeUninstallState {
    fn store_backup(
        &self,
        _target_path: &InstallTargetPath,
        _bytes: &[u8],
    ) -> anyhow::Result<String> {
        panic!("batch uninstall facts must be read-only")
    }

    fn read_backup(&self, backup_ref: &str) -> anyhow::Result<Option<Vec<u8>>> {
        if self
            .backup_read_errors
            .lock()
            .expect("backup read errors")
            .contains(backup_ref)
        {
            anyhow::bail!("injected backup read failure");
        }
        Ok(self
            .backups
            .lock()
            .expect("backups")
            .get(backup_ref)
            .cloned())
    }

    fn remove_backup(&self, backup_ref: &str) -> anyhow::Result<()> {
        self.backups.lock().expect("backups").remove(backup_ref);
        Ok(())
    }
}

impl InstallManifestRepository for FakeUninstallState {
    fn load_manifest(&self, _profile_id: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        if self.manifest_read_error.load(Ordering::Relaxed) {
            anyhow::bail!("injected manifest read failure");
        }
        Ok(self.manifest.lock().expect("manifest").clone())
    }

    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        self.manifest_save_count.fetch_add(1, Ordering::Relaxed);
        *self.manifest.lock().expect("manifest") = Some(manifest.clone());
        Ok(())
    }
}

impl InstallRecoveryRecordRepository for FakeUninstallState {
    fn load_record(
        &self,
        _profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> anyhow::Result<Option<InstallRecoveryRecord>> {
        Ok(self
            .recovery_records
            .lock()
            .expect("recovery records")
            .get(mod_id.as_str())
            .cloned())
    }

    fn list_records(&self, _profile_id: &ProfileId) -> anyhow::Result<Vec<InstallRecoveryRecord>> {
        Ok(self
            .recovery_records
            .lock()
            .expect("recovery records")
            .values()
            .cloned()
            .collect())
    }

    fn save_record(&self, _record: &InstallRecoveryRecord) -> anyhow::Result<()> {
        panic!("batch uninstall facts must be read-only")
    }

    fn remove_record(&self, _profile_id: &ProfileId, _mod_id: &ModId) -> anyhow::Result<()> {
        panic!("batch uninstall facts must be read-only")
    }
}

impl ReinstallRecoveryTransactionRepository for FakeUninstallState {
    fn load_transaction(
        &self,
        _profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> anyhow::Result<Option<ReinstallRecoveryTransaction>> {
        Ok(self
            .reinstall_transactions
            .lock()
            .expect("reinstall transactions")
            .get(mod_id.as_str())
            .cloned())
    }

    fn list_transactions(
        &self,
        _profile_id: &ProfileId,
    ) -> anyhow::Result<Vec<ReinstallRecoveryTransaction>> {
        Ok(self
            .reinstall_transactions
            .lock()
            .expect("reinstall transactions")
            .values()
            .cloned()
            .collect())
    }

    fn save_transaction(&self, _transaction: &ReinstallRecoveryTransaction) -> anyhow::Result<()> {
        panic!("batch uninstall facts must be read-only")
    }

    fn remove_transaction(&self, _profile_id: &ProfileId, _mod_id: &ModId) -> anyhow::Result<()> {
        panic!("batch uninstall facts must be read-only")
    }
}

impl ReinstallSnapshotStore for FakeUninstallState {
    fn store_snapshot(
        &self,
        _target_path: &InstallTargetPath,
        _bytes: &[u8],
    ) -> anyhow::Result<String> {
        panic!("batch uninstall facts must be read-only")
    }

    fn read_snapshot(&self, _snapshot_ref: &str) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(None)
    }

    fn remove_snapshot(&self, _snapshot_ref: &str) -> anyhow::Result<()> {
        panic!("batch uninstall facts must be read-only")
    }
}

fn provider(state: Arc<FakeUninstallState>) -> BatchUninstallPlanFactsProvider {
    let game_files: Arc<dyn InstallGameFileSystem> = state.clone();
    let backups: Arc<dyn InstallBackupStore> = state.clone();
    let manifests: Arc<dyn InstallManifestRepository> = state.clone();
    let recovery_records: Arc<dyn InstallRecoveryRecordRepository> = state.clone();
    let reinstall_transactions: Arc<dyn ReinstallRecoveryTransactionRepository> = state.clone();
    let snapshots: Arc<dyn ReinstallSnapshotStore> = state;
    let scanner = InstallRecoveryScanService::new_with_recovery_records(
        game_files,
        backups,
        Arc::clone(&manifests),
        recovery_records,
    )
    .with_reinstall_recovery_transactions(reinstall_transactions, snapshots);
    BatchUninstallPlanFactsProvider::new(manifests, scanner, "env")
}

fn request(items: &[(&str, &str)]) -> hmm_core::NormalizedBatchPlanRequest {
    BatchPlanRequest {
        schema_version: BATCH_PLAN_SCHEMA_VERSION,
        operation: BatchOperation::Uninstall,
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        execution_policy: BatchExecutionPolicy::StopOnFailure,
        items: items
            .iter()
            .map(|(mod_id, revision_id)| {
                BatchItemInput::Uninstall(UninstallBatchItemInput {
                    mod_id: ModId::new(*mod_id),
                    expected_installed_revision_id: ModRevisionId::new(*revision_id),
                })
            })
            .collect(),
    }
    .normalize()
    .expect("normalized request")
}

fn manifest(entries: Vec<InstallManifestEntry>) -> InstallManifest {
    let mut manifest = InstallManifest::completed(ProfileId::new("default"), entries);
    manifest.schema_version = INSTALL_MANIFEST_SCHEMA_VERSION_V2;
    manifest
}

fn entry(
    mod_id: &str,
    revision_id: Option<&str>,
    target: &str,
    installed_bytes: &[u8],
    backup_ref: Option<&str>,
) -> InstallManifestEntry {
    InstallManifestEntry {
        target_path: InstallTargetPath::parse(target, ["nativePC"]).expect("target"),
        mod_id: ModId::new(mod_id),
        revision_id: revision_id.map(ModRevisionId::new),
        package_file_id: PackageFileId::new(target),
        layer: FileLayer::new("default", 0),
        backup_ref: backup_ref.map(str::to_owned),
        installed_file: Some(summary(installed_bytes)),
    }
}

fn summary(bytes: &[u8]) -> InstalledFileSummary {
    InstalledFileSummary {
        size_bytes: bytes.len() as u64,
        sha256: Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    }
}

fn replacement_snapshot(
    mod_id: &str,
    binding_id: &str,
    target_id: &str,
) -> ReplacementBindingSnapshot {
    ReplacementBindingSnapshot::new(
        ReplacementBinding::new(
            ReplacementBindingId::parse(binding_id).expect("binding id"),
            ModId::new(mod_id),
            ProfileId::new("default"),
            ReplacementSourceId::parse("mhw:armor:f_equip:pl121_0000").expect("source id"),
            ReplacementTargetId::parse(target_id).expect("target id"),
            42,
        )
        .expect("binding"),
        Some(ModRevisionId::new("rev-a")),
        "pl121_0000",
        "pl129_0000",
        "pl/f_equip",
        "pl/f_equip",
        ReplacementTargetKind::parse("armor").expect("kind"),
    )
    .expect("snapshot")
}

fn reasons(facts: &BatchPlanFacts, mod_id: &str) -> Vec<String> {
    facts
        .items
        .iter()
        .find(|item| item.mod_id == ModId::new(mod_id))
        .expect("item facts")
        .blocking_reasons
        .clone()
}

#[test]
fn ready_facts_distinguish_remove_and_restore_without_package_reads() {
    let state = Arc::new(FakeUninstallState::default());
    state.set_manifest(manifest(vec![
        entry("a", Some("rev-a"), "nativePC/a.bin", b"a", None),
        entry("b", Some("rev-b"), "nativePC/b.bin", b"b", Some("backup-b")),
    ]));
    state.add_file("nativePC/a.bin", b"a");
    state.add_file("nativePC/b.bin", b"b");
    state.add_backup("backup-b", b"baseline-b");

    let facts = provider(Arc::clone(&state))
        .read_batch_plan_facts(&request(&[("a", "rev-a"), ("b", "rev-b")]))
        .expect("facts");

    assert!(facts.global_blocking_reasons.is_empty());
    assert_eq!(
        facts.items[0].installed_revision_id,
        Some(ModRevisionId::new("rev-a"))
    );
    assert_eq!(
        facts.items[0].target_claims[0].kind,
        BatchTargetWriteKind::Remove
    );
    assert_eq!(
        facts.items[1].target_claims[0].kind,
        BatchTargetWriteKind::Restore
    );
    assert!(
        facts
            .items
            .iter()
            .all(|item| item.blocking_reasons.is_empty()),
        "unexpected facts: {:?}",
        facts.items
    );
    assert_eq!(state.game_write_count.load(Ordering::Relaxed), 0);
    assert_eq!(state.manifest_save_count.load(Ordering::Relaxed), 0);
}

#[test]
fn target_and_backup_failures_are_stable_item_blockers() {
    let state = Arc::new(FakeUninstallState::default());
    state.set_manifest(manifest(vec![
        entry("a", Some("rev"), "nativePC/missing.bin", b"a", None),
        entry("b", Some("rev"), "nativePC/changed.bin", b"b", None),
        entry(
            "c",
            Some("rev"),
            "nativePC/backup-missing.bin",
            b"c",
            Some("backup-missing"),
        ),
        entry(
            "d",
            Some("rev"),
            "nativePC/backup-error.bin",
            b"d",
            Some("backup-error"),
        ),
        entry("e", Some("rev"), "nativePC/read-error.bin", b"e", None),
    ]));
    state.add_file("nativePC/changed.bin", b"changed");
    state.add_file("nativePC/backup-missing.bin", b"c");
    state.add_file("nativePC/backup-error.bin", b"d");
    state.add_file("nativePC/read-error.bin", b"e");
    state
        .file_read_errors
        .lock()
        .expect("file errors")
        .insert("nativepc/read-error.bin".to_owned());
    state
        .backup_read_errors
        .lock()
        .expect("backup errors")
        .insert("backup-error".to_owned());

    let facts = provider(state)
        .read_batch_plan_facts(&request(&[
            ("a", "rev"),
            ("b", "rev"),
            ("c", "rev"),
            ("d", "rev"),
            ("e", "rev"),
        ]))
        .expect("facts");

    assert!(reasons(&facts, "a").contains(&"installed_target_missing".to_owned()));
    assert!(reasons(&facts, "b").contains(&"installed_target_changed".to_owned()));
    assert!(reasons(&facts, "c").contains(&"install_backup_missing".to_owned()));
    assert!(reasons(&facts, "d").contains(&"install_backup_unavailable".to_owned()));
    assert!(reasons(&facts, "e").contains(&"installed_target_unavailable".to_owned()));
}

#[test]
fn manifest_read_failure_makes_uninstall_facts_unavailable_without_writes() {
    let state = Arc::new(FakeUninstallState::default());
    state.manifest_read_error.store(true, Ordering::Relaxed);

    let result = provider(Arc::clone(&state)).read_batch_plan_facts(&request(&[("a", "rev-a")]));

    assert!(result.is_err());
    assert_eq!(state.game_write_count.load(Ordering::Relaxed), 0);
    assert_eq!(state.manifest_save_count.load(Ordering::Relaxed), 0);
}

#[test]
fn invalid_manifest_is_a_global_blocker_without_writes() {
    let state = Arc::new(FakeUninstallState::default());
    let mut mismatched = manifest(vec![entry(
        "a",
        Some("rev-a"),
        "nativePC/a.bin",
        b"a",
        None,
    )]);
    mismatched.profile_id = ProfileId::new("other-profile");
    state.set_manifest(mismatched);
    state.add_file("nativePC/a.bin", b"a");

    let facts = provider(Arc::clone(&state))
        .read_batch_plan_facts(&request(&[("a", "rev-a")]))
        .expect("invalid manifest is represented as fail-closed facts");

    assert!(facts
        .global_blocking_reasons
        .iter()
        .any(|reason| reason.code == "batch_global_manifest_invalid" && reason.count == 1));
    assert_eq!(state.game_write_count.load(Ordering::Relaxed), 0);
    assert_eq!(state.manifest_save_count.load(Ordering::Relaxed), 0);
}

#[test]
fn install_and_reinstall_recovery_are_global_blockers() {
    let state = Arc::new(FakeUninstallState::default());
    let current_manifest = manifest(vec![
        entry("a", Some("rev-a"), "nativePC/a.bin", b"a", None),
        entry("b", Some("rev-b"), "nativePC/b.bin", b"b", None),
    ]);
    state.set_manifest(current_manifest.clone());
    state.add_file("nativePC/a.bin", b"a");
    state.add_file("nativePC/b.bin", b"b");
    state.add_recovery_record(InstallRecoveryRecord {
        profile_id: ProfileId::new("default"),
        mod_id: ModId::new("a"),
        status: InstallRecoveryRecordStatus::Committing,
        entries: Vec::new(),
    });
    state.add_reinstall_transaction(ReinstallRecoveryTransaction {
        profile_id: ProfileId::new("default"),
        mod_id: ModId::new("b"),
        old_revision_id: ModRevisionId::new("rev-b"),
        candidate_revision_id: ModRevisionId::new("rev-c"),
        plan_token: "token".to_owned(),
        plan_hash: "hash".to_owned(),
        status: ReinstallRecoveryTransactionStatus::RepairRequired,
        pre_reinstall_manifest: current_manifest,
        candidate_replacement_bindings: Vec::new(),
        targets: Vec::new(),
    });

    let facts = provider(state)
        .read_batch_plan_facts(&request(&[("a", "rev-a"), ("b", "rev-b")]))
        .expect("facts");

    assert_eq!(
        facts.global_blocking_reasons[0].code,
        "batch_global_recovery_active"
    );
    assert_eq!(facts.global_blocking_reasons[0].count, 2);
    assert!(reasons(&facts, "a").contains(&"install_rollback_required".to_owned()));
    assert!(reasons(&facts, "b").contains(&"install_repair_required".to_owned()));
}

#[test]
fn legacy_or_mixed_revision_manifest_cannot_prove_exact_installed_revision() {
    let state = Arc::new(FakeUninstallState::default());
    let mut legacy = manifest(vec![entry("a", None, "nativePC/a.bin", b"a", None)]);
    legacy.schema_version = hmm_core::INSTALL_MANIFEST_SCHEMA_VERSION_V1;
    state.set_manifest(legacy);
    state.add_file("nativePC/a.bin", b"a");

    let normalized = request(&[("a", "rev-a")]);
    let facts = provider(state)
        .read_batch_plan_facts(&normalized)
        .expect("facts");
    let plan =
        build_batch_plan(normalized, facts.clone(), BatchResourceLimits::default()).expect("plan");

    assert_eq!(facts.items[0].installed_revision_id, None);
    assert!(reasons(&facts, "a").contains(&"install_manifest_legacy".to_owned()));
    assert!(plan.items[0]
        .blocking_reasons
        .contains(&"manifest_changed".to_owned()));
}

#[test]
fn same_revision_replacement_binding_changes_uninstall_manifest_digest() {
    let state = Arc::new(FakeUninstallState::default());
    let mut alpha = manifest(vec![entry(
        "a",
        Some("rev-a"),
        "nativePC/a.bin",
        b"a",
        None,
    )]);
    alpha.replacement_bindings = vec![replacement_snapshot(
        "a",
        "binding-a",
        "mhw:armor:fatalis-alpha",
    )];
    state.set_manifest(alpha);
    state.add_file("nativePC/a.bin", b"a");
    let normalized = request(&[("a", "rev-a")]);
    let alpha_facts = provider(Arc::clone(&state))
        .read_batch_plan_facts(&normalized)
        .expect("alpha facts");
    let mut beta = state
        .manifest
        .lock()
        .expect("manifest")
        .clone()
        .expect("manifest");
    beta.replacement_bindings = vec![replacement_snapshot(
        "a",
        "binding-b",
        "mhw:armor:fatalis-beta",
    )];
    state.set_manifest(beta);
    let beta_facts = provider(state)
        .read_batch_plan_facts(&normalized)
        .expect("beta facts");

    assert_ne!(
        alpha_facts.items[0].single_plan_digest,
        beta_facts.items[0].single_plan_digest
    );
    assert_ne!(
        alpha_facts.items[0].fact_digest,
        beta_facts.items[0].fact_digest
    );
}

#[test]
fn shared_and_external_ownership_fail_closed() {
    let state = Arc::new(FakeUninstallState::default());
    state.set_manifest(manifest(vec![
        entry(
            "a",
            Some("rev-a"),
            "nativePC/shared.bin",
            b"shared",
            Some("shared-backup"),
        ),
        entry(
            "a",
            Some("rev-a"),
            "nativePC/external.bin",
            b"external",
            Some("external-backup"),
        ),
        entry(
            "b",
            Some("rev-b"),
            "nativePC/shared.bin",
            b"shared",
            Some("shared-backup"),
        ),
        entry(
            "outside",
            Some("rev-outside"),
            "nativePC/external.bin",
            b"external",
            Some("external-backup"),
        ),
    ]));
    state.add_file("nativePC/shared.bin", b"shared");
    state.add_file("nativePC/external.bin", b"external");
    state.add_backup("shared-backup", b"baseline-shared");
    state.add_backup("external-backup", b"baseline-external");

    let normalized = request(&[("a", "rev-a"), ("b", "rev-b")]);
    let facts = provider(state)
        .read_batch_plan_facts(&normalized)
        .expect("facts");
    let plan =
        build_batch_plan(normalized, facts.clone(), BatchResourceLimits::default()).expect("plan");

    assert!(facts
        .global_blocking_reasons
        .iter()
        .any(|reason| reason.code == "batch_global_backup_conflict"));
    assert!(reasons(&facts, "a").contains(&"install_target_owned_by_other_mod".to_owned()));
    assert!(reasons(&facts, "a").contains(&"install_backup_owned_by_other_mod".to_owned()));
    assert!(plan
        .global_blocking_reasons
        .iter()
        .any(|reason| reason.code == "batch_global_target_conflict"));
}

#[test]
fn revision_bound_transactions_preserve_prior_success_and_unowned_files() {
    let state = Arc::new(FakeUninstallState::default());
    state.set_manifest(manifest(vec![
        entry("a", Some("rev-a"), "nativePC/a.bin", b"a", None),
        entry("b", Some("rev-b"), "nativePC/b.bin", b"b", None),
    ]));
    state.add_file("nativePC/a.bin", b"a");
    state.add_file("nativePC/b.bin", b"player-edit");
    state.add_file("nativePC/unowned-sentinel.bin", b"keep-me");
    let game_files: Arc<dyn InstallGameFileSystem> = state.clone();
    let backups: Arc<dyn InstallBackupStore> = state.clone();
    let manifests: Arc<dyn InstallManifestRepository> = state.clone();
    let service = UninstallModService::new(game_files, backups, manifests);

    service
        .uninstall_mod_for_revision(
            UninstallModRequest {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("a"),
            },
            ModRevisionId::new("rev-a"),
        )
        .expect("first uninstall succeeds");
    let error = service
        .uninstall_mod_for_revision(
            UninstallModRequest {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("b"),
            },
            ModRevisionId::new("rev-b"),
        )
        .expect_err("player-modified second target must fail closed");

    assert_eq!(error, UninstallModError::TargetStateMismatch);
    let files = state.files.lock().expect("files");
    assert!(!files.contains_key("nativepc/a.bin"));
    assert_eq!(files.get("nativepc/b.bin"), Some(&b"player-edit".to_vec()));
    assert_eq!(
        files.get("nativepc/unowned-sentinel.bin"),
        Some(&b"keep-me".to_vec())
    );
    drop(files);
    let current_manifest = state
        .manifest
        .lock()
        .expect("manifest")
        .clone()
        .expect("manifest remains");
    assert_eq!(current_manifest.entries.len(), 1);
    assert_eq!(current_manifest.entries[0].mod_id, ModId::new("b"));
}

#[derive(Clone)]
struct FixedClock;

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> anyhow::Result<u128> {
        Ok(10)
    }
}

struct ConfiguredAudit {
    fail: bool,
}

impl AuditLogWriter for ConfiguredAudit {
    fn record(&self, _event: AuditLogEvent) -> anyhow::Result<()> {
        if self.fail {
            anyhow::bail!("injected audit failure");
        }
        Ok(())
    }
}

struct ConfiguredUninstaller {
    result: Result<crate::UninstallModResult, UninstallModError>,
    expected_revisions: Mutex<Vec<ModRevisionId>>,
    expected_manifest_digests: Mutex<Vec<String>>,
}

impl crate::ModUninstaller for ConfiguredUninstaller {
    fn uninstall_mod(
        &self,
        _request: StartUninstallTaskRequest,
    ) -> Result<crate::UninstallModResult, UninstallModError> {
        self.result.clone()
    }

    fn uninstall_mod_for_revision(
        &self,
        _request: StartUninstallTaskRequest,
        expected_installed_revision_id: ModRevisionId,
    ) -> Result<crate::UninstallModResult, UninstallModError> {
        self.expected_revisions
            .lock()
            .expect("expected revisions")
            .push(expected_installed_revision_id);
        self.result.clone()
    }

    fn uninstall_mod_for_revision_and_manifest(
        &self,
        _request: StartUninstallTaskRequest,
        expected_installed_revision_id: ModRevisionId,
        expected_manifest_digest: &str,
    ) -> Result<crate::UninstallModResult, UninstallModError> {
        self.expected_revisions
            .lock()
            .expect("expected revisions")
            .push(expected_installed_revision_id);
        self.expected_manifest_digests
            .lock()
            .expect("expected manifest digests")
            .push(expected_manifest_digest.to_owned());
        self.result.clone()
    }
}

fn execution_request(parent_task_id: String) -> BatchInstallItemRequest {
    let input = BatchItemInput::Uninstall(UninstallBatchItemInput {
        mod_id: ModId::new("a"),
        expected_installed_revision_id: ModRevisionId::new("rev-a"),
    });
    BatchInstallItemRequest {
        batch_id: BatchId::new("batch-a"),
        attempt_number: 0,
        item: SealedBatchItem {
            item_id: BatchItemId::new("item-a"),
            ordinal: 0,
            mod_id: ModId::new("a"),
        },
        plan: BatchPlan {
            plan_schema_version: BATCH_PLAN_SCHEMA_VERSION,
            operation: BatchOperation::Uninstall,
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            execution_policy: BatchExecutionPolicy::StopOnFailure,
            items: vec![BatchItemPlan {
                ordinal: 0,
                input_snapshot: input,
                source_revision_id: None,
                installed_revision_id: Some(ModRevisionId::new("rev-a")),
                fact_digest: "fact".to_owned(),
                single_plan_digest: "plan".to_owned(),
                prerequisite: BatchPreflightDecision {
                    status: BatchPreflightStatus::Ready,
                    rules_version: None,
                    codes: Vec::new(),
                },
                target_claims: Vec::new(),
                action_summary: BatchActionSummary::default(),
                blocking_reasons: Vec::new(),
                warning_codes: Vec::new(),
            }],
            environment_digest: "env".to_owned(),
            prerequisite_rules_version: None,
            resource_limits: BatchResourceLimits::default(),
            resource_usage: BatchResourceUsage {
                item_count: 1,
                target_action_count: 0,
                canonical_bytes: 1,
            },
            global_target_claims_digest: "claims".to_owned(),
            batch_digest: "digest".to_owned(),
            global_blocking_reasons: Vec::new(),
            warning_codes: Vec::new(),
        },
        parent_task_id,
    }
}

fn executor(
    result: Result<crate::UninstallModResult, UninstallModError>,
    audit_fails: bool,
) -> (
    UninstallTaskBatchItemExecutor,
    Arc<TaskManager>,
    Arc<ConfiguredUninstaller>,
) {
    let task_manager = Arc::new(TaskManager::new());
    let uninstaller = Arc::new(ConfiguredUninstaller {
        result,
        expected_revisions: Mutex::new(Vec::new()),
        expected_manifest_digests: Mutex::new(Vec::new()),
    });
    let runner = Arc::new(UninstallTaskRunner::new(
        Arc::clone(&task_manager),
        uninstaller.clone(),
        Arc::new(ConfiguredAudit { fail: audit_fails }),
        Arc::new(FixedClock),
    ));
    (
        UninstallTaskBatchItemExecutor::new(runner, Arc::clone(&task_manager)),
        task_manager,
        uninstaller,
    )
}

fn started_parent(task_manager: &TaskManager) -> String {
    let task = task_manager.create_task(TaskKind::Install).expect("parent");
    task_manager
        .start_task(&task.task_id)
        .expect("start parent");
    task.task_id
}

#[test]
fn executor_binds_exact_revision_and_surfaces_post_commit_audit_degradation() {
    let result = crate::UninstallModResult {
        manifest: manifest(Vec::new()),
        removed_file_count: 1,
        restored_file_count: 0,
    };
    let (executor, task_manager, uninstaller) = executor(Ok(result), true);
    let parent = started_parent(&task_manager);

    let execution = executor.execute(execution_request(parent.clone()));

    assert_eq!(
        execution,
        BatchInstallItemExecution::Succeeded {
            evidence_health_degraded: true,
        }
    );
    assert_eq!(
        *uninstaller
            .expected_revisions
            .lock()
            .expect("expected revisions"),
        vec![ModRevisionId::new("rev-a")]
    );
    assert_eq!(
        *uninstaller
            .expected_manifest_digests
            .lock()
            .expect("expected manifest digests"),
        vec!["plan".to_owned()]
    );
    assert!(
        task_manager.cancel_task(&parent).is_err(),
        "the parent and child must share the uninstall commit cancellation barrier"
    );
}

#[test]
fn executor_maps_uninstall_rollback_failure_to_recovery_required() {
    let (executor, task_manager, _) = executor(
        Err(UninstallModError::RollbackFailed {
            failed_phase: UninstallModPhase::ManifestSave,
        }),
        false,
    );
    let parent = started_parent(&task_manager);

    assert_eq!(
        executor.execute(execution_request(parent)),
        BatchInstallItemExecution::RecoveryRequired {
            reason_code: "uninstall_rollback_failed".to_owned(),
        }
    );
}

#[test]
fn executor_maps_revision_drift_to_non_retryable_blocker() {
    let (executor, task_manager, _) =
        executor(Err(UninstallModError::InstalledRevisionMismatch), false);
    let parent = started_parent(&task_manager);

    assert_eq!(
        executor.execute(execution_request(parent)),
        BatchInstallItemExecution::Blocked {
            reason_code: "uninstall_plan_stale".to_owned(),
        }
    );
}

#[test]
fn executor_retries_only_failures_proven_to_be_prewrite_or_rolled_back() {
    for error in [
        UninstallModError::ManifestUnavailable,
        UninstallModError::RemoveFailed,
        UninstallModError::ManifestSaveFailed,
    ] {
        let (executor, task_manager, _) = executor(Err(error.clone()), false);
        let parent = started_parent(&task_manager);

        assert_eq!(
            executor.execute(execution_request(parent)),
            BatchInstallItemExecution::Failed {
                reason_code: if error == UninstallModError::ManifestUnavailable {
                    "uninstall_unavailable".to_owned()
                } else {
                    "uninstall_rollback_succeeded".to_owned()
                },
                retryable: true,
                evidence_health_degraded: false,
            }
        );
    }
}

#[test]
fn committed_task_evidence_failure_cannot_make_uninstall_retryable() {
    let execution = classify_uninstall_task_failure(&UninstallTaskRunError {
        events: Vec::new(),
        uninstall_error: None,
        committed: true,
    });

    assert_eq!(
        execution,
        BatchInstallItemExecution::Succeeded {
            evidence_health_degraded: true,
        }
    );
}
