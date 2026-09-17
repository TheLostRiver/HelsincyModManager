use super::*;
use crate::ModInstallationStateSession;
use hmm_core::{
    FileLayer, GameId, InstallManifest, InstallManifestEntry, InstallRecoveryRecord,
    InstallTargetPath, ModRevisionId, PackageFileId, ReinstallRecoveryTransaction,
    INSTALL_MANIFEST_SCHEMA_VERSION_V2,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

#[derive(Default)]
struct Metadata {
    manifest: Option<InstallManifest>,
    records: Vec<InstallRecoveryRecord>,
    transactions: Vec<ReinstallRecoveryTransaction>,
    failing: Option<&'static str>,
}

#[derive(Default)]
struct Repositories {
    data: Mutex<Metadata>,
    reads: AtomicUsize,
}

impl InstallManifestRepository for Repositories {
    fn load_manifest(&self, _: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        let data = self.data.lock().unwrap();
        anyhow::ensure!(data.failing != Some("manifest"), "unavailable");
        Ok(data.manifest.clone())
    }
    fn save_manifest(&self, _: &InstallManifest) -> anyhow::Result<()> {
        panic!("read only")
    }
}

impl InstallRecoveryRecordRepository for Repositories {
    fn load_record(
        &self,
        _: &ProfileId,
        _: &ModId,
    ) -> anyhow::Result<Option<InstallRecoveryRecord>> {
        panic!("must read records once per profile")
    }
    fn list_records(&self, _: &ProfileId) -> anyhow::Result<Vec<InstallRecoveryRecord>> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        let data = self.data.lock().unwrap();
        anyhow::ensure!(data.failing != Some("recovery"), "unavailable");
        Ok(data.records.clone())
    }
    fn save_record(&self, _: &InstallRecoveryRecord) -> anyhow::Result<()> {
        panic!("read only")
    }
    fn remove_record(&self, _: &ProfileId, _: &ModId) -> anyhow::Result<()> {
        panic!("read only")
    }
}

impl ReinstallRecoveryTransactionRepository for Repositories {
    fn load_transaction(
        &self,
        _: &ProfileId,
        _: &ModId,
    ) -> anyhow::Result<Option<ReinstallRecoveryTransaction>> {
        panic!("must read transactions once per profile")
    }
    fn list_transactions(
        &self,
        _: &ProfileId,
    ) -> anyhow::Result<Vec<ReinstallRecoveryTransaction>> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        let data = self.data.lock().unwrap();
        anyhow::ensure!(data.failing != Some("reinstall"), "unavailable");
        Ok(data.transactions.clone())
    }
    fn save_transaction(&self, _: &ReinstallRecoveryTransaction) -> anyhow::Result<()> {
        panic!("read only")
    }
    fn remove_transaction(&self, _: &ProfileId, _: &ModId) -> anyhow::Result<()> {
        panic!("read only")
    }
}

fn service(repositories: &Arc<Repositories>) -> Arc<ModInstallationStateQueryService> {
    Arc::new(ModInstallationStateQueryService::new(
        repositories.clone(),
        repositories.clone(),
        repositories.clone(),
    ))
}

fn profile() -> ProfileId {
    ProfileId::new("default")
}

fn entry(id: &str, path: &str) -> InstallManifestEntry {
    InstallManifestEntry {
        target_path: InstallTargetPath::parse(path, ["nativePC"]).unwrap(),
        mod_id: ModId::new(id),
        revision_id: Some(ModRevisionId::new(format!("revision-{id}"))),
        package_file_id: PackageFileId::new(path),
        layer: FileLayer::new("base", 0),
        backup_ref: None,
        installed_file: None,
        adopted: false,
    }
}

fn manifest(entries: Vec<InstallManifestEntry>) -> InstallManifest {
    let mut manifest = InstallManifest::completed(profile(), entries);
    manifest.schema_version = INSTALL_MANIFEST_SCHEMA_VERSION_V2;
    manifest.validate().unwrap();
    manifest
}

fn transaction(status: ReinstallRecoveryTransactionStatus) -> ReinstallRecoveryTransaction {
    let transaction = ReinstallRecoveryTransaction {
        profile_id: profile(),
        mod_id: ModId::new("a"),
        old_revision_id: ModRevisionId::new("revision-a"),
        candidate_revision_id: ModRevisionId::new("revision-next"),
        intent: Default::default(),
        plan_token: "token".to_owned(),
        plan_hash: "hash".to_owned(),
        status,
        pre_reinstall_manifest: manifest(vec![entry("a", "nativePC/a.bin")]),
        original_install_evidence: None,
        source_evidence_version: 1,
        additional_sources_evidence: None,
        candidate_replacement_bindings: Vec::new(),
        candidate_plugin_selections: Vec::new(),
        targets: Vec::new(),
    };
    transaction.validate().unwrap();
    transaction
}

#[test]
fn empty_metadata_is_explicitly_not_installed_without_file_io() {
    let repositories = Arc::new(Repositories::default());
    let snapshot = service(&repositories).inspect(&profile()).unwrap();
    let summary = snapshot.summary(&ModId::new("a"));
    assert_eq!(summary.status, InstallManifestStatus::NotInstalled);
    assert_eq!(summary.managed_file_count, 0);
    assert_eq!(summary.installed_revision_id, None);
    assert_eq!(repositories.reads.load(Ordering::Relaxed), 3);
}

#[test]
fn counts_all_mods_and_exact_revisions_in_one_metadata_read() {
    let repositories = Arc::new(Repositories::default());
    let mut backed_up = entry("a", "nativePC/a.bin");
    backed_up.backup_ref = Some("backup-a".to_owned());
    let mut adopted = entry("a", "nativePC/a-extra.bin");
    adopted.adopted = true;
    repositories.data.lock().unwrap().manifest = Some(manifest(vec![
        backed_up,
        adopted,
        entry("b", "nativePC/b.bin"),
    ]));
    let snapshot = service(&repositories).inspect(&profile()).unwrap();
    let summary = snapshot.summary(&ModId::new("a"));
    assert_eq!(
        (
            summary.managed_file_count,
            summary.backup_count,
            summary.adopted_file_count
        ),
        (2, 1, Some(1))
    );
    assert_eq!(
        summary.installed_revision_id,
        Some(ModRevisionId::new("revision-a"))
    );
    assert_eq!(
        snapshot.summary(&ModId::new("b")).status,
        InstallManifestStatus::Installed
    );
    for index in 0..100 {
        snapshot.summary(&ModId::new(format!("missing-{index}")));
    }
    assert_eq!(repositories.reads.load(Ordering::Relaxed), 3);
}

#[test]
fn invalid_foreign_or_unreadable_metadata_never_becomes_installed() {
    let repositories = Arc::new(Repositories::default());
    let query = service(&repositories);
    for failure in ["manifest", "recovery", "reinstall"] {
        repositories.data.lock().unwrap().failing = Some(failure);
        assert!(query.inspect(&profile()).is_err());
    }
    repositories.data.lock().unwrap().failing = None;
    let mut foreign = manifest(vec![entry("a", "nativePC/a.bin")]);
    foreign.profile_id = ProfileId::new("other");
    repositories.data.lock().unwrap().manifest = Some(foreign);
    assert!(query.inspect(&profile()).is_err());
    let mut invalid = manifest(vec![entry("a", "nativePC/a.bin")]);
    invalid.entries.push(entry("a", "nativePC/second.bin"));
    invalid.entries[1].revision_id = Some(ModRevisionId::new("other-revision"));
    repositories.data.lock().unwrap().manifest = Some(invalid);
    assert!(query.inspect(&profile()).is_err());
}

#[test]
fn unsafe_profile_manifest_cannot_prove_even_an_absent_mod_safe() {
    let repositories = Arc::new(Repositories::default());
    let query = service(&repositories);
    for (status, expected) in [
        (
            hmm_core::InstallManifestStatus::Committing,
            InstallManifestStatus::Unknown,
        ),
        (
            hmm_core::InstallManifestStatus::RollbackRequired,
            InstallManifestStatus::RollbackRequired,
        ),
        (
            hmm_core::InstallManifestStatus::RepairRequired,
            InstallManifestStatus::RepairRequired,
        ),
    ] {
        let mut current = manifest(vec![entry("a", "nativePC/a.bin")]);
        current.status = status;
        repositories.data.lock().unwrap().manifest = Some(current);
        let snapshot = query.inspect(&profile()).unwrap();
        assert_eq!(snapshot.summary(&ModId::new("a")).status, expected);
        assert_eq!(snapshot.summary(&ModId::new("absent")).status, expected);
        assert_eq!(
            snapshot.summary(&ModId::new("a")).installed_revision_id,
            None
        );
    }
}

#[test]
fn ordinary_recovery_records_override_only_unfinished_or_broken_state() {
    let repositories = Arc::new(Repositories::default());
    repositories.data.lock().unwrap().manifest = Some(manifest(vec![entry("a", "nativePC/a.bin")]));
    let query = service(&repositories);
    for (status, expected) in [
        (
            InstallRecoveryRecordStatus::Planned,
            InstallManifestStatus::Installed,
        ),
        (
            InstallRecoveryRecordStatus::Completed,
            InstallManifestStatus::Installed,
        ),
        (
            InstallRecoveryRecordStatus::RolledBack,
            InstallManifestStatus::Installed,
        ),
        (
            InstallRecoveryRecordStatus::Committing,
            InstallManifestStatus::RollbackRequired,
        ),
        (
            InstallRecoveryRecordStatus::RollbackRequired,
            InstallManifestStatus::RollbackRequired,
        ),
        (
            InstallRecoveryRecordStatus::RepairRequired,
            InstallManifestStatus::RepairRequired,
        ),
    ] {
        repositories.data.lock().unwrap().records = vec![InstallRecoveryRecord {
            profile_id: profile(),
            mod_id: ModId::new("a"),
            status,
            entries: Vec::new(),
        }];
        assert_eq!(
            query
                .inspect(&profile())
                .unwrap()
                .summary(&ModId::new("a"))
                .status,
            expected
        );
    }
}

#[test]
fn reinstall_metadata_does_not_claim_candidate_content_has_been_verified() {
    let repositories = Arc::new(Repositories::default());
    let query = service(&repositories);
    for (status, expected) in [
        (
            ReinstallRecoveryTransactionStatus::Planned,
            InstallManifestStatus::RollbackRequired,
        ),
        (
            ReinstallRecoveryTransactionStatus::Committing,
            InstallManifestStatus::RollbackRequired,
        ),
        (
            ReinstallRecoveryTransactionStatus::RollbackRequired,
            InstallManifestStatus::RollbackRequired,
        ),
        (
            ReinstallRecoveryTransactionStatus::RepairRequired,
            InstallManifestStatus::RepairRequired,
        ),
        (
            ReinstallRecoveryTransactionStatus::Completed,
            InstallManifestStatus::CleanupPending,
        ),
        (
            ReinstallRecoveryTransactionStatus::RolledBack,
            InstallManifestStatus::CleanupPending,
        ),
    ] {
        repositories.data.lock().unwrap().transactions = vec![transaction(status)];
        let summary = query.inspect(&profile()).unwrap().summary(&ModId::new("a"));
        assert_eq!(summary.status, expected);
        assert_eq!(summary.installed_revision_id, None);
    }
    repositories.data.lock().unwrap().transactions[0]
        .plan_hash
        .clear();
    assert_eq!(
        query
            .inspect(&profile())
            .unwrap()
            .summary(&ModId::new("a"))
            .status,
        InstallManifestStatus::Unknown
    );
}

#[test]
fn duplicate_foreign_and_conflicting_recovery_facts_fail_closed() {
    let repositories = Arc::new(Repositories::default());
    let query = service(&repositories);
    let record = InstallRecoveryRecord {
        profile_id: profile(),
        mod_id: ModId::new("a"),
        status: InstallRecoveryRecordStatus::Completed,
        entries: Vec::new(),
    };
    repositories.data.lock().unwrap().records = vec![record.clone(), record.clone()];
    assert!(query.inspect(&profile()).is_err());
    repositories.data.lock().unwrap().records = vec![InstallRecoveryRecord {
        profile_id: ProfileId::new("other"),
        ..record.clone()
    }];
    assert!(query.inspect(&profile()).is_err());
    repositories.data.lock().unwrap().records = vec![record];
    let transaction = transaction(ReinstallRecoveryTransactionStatus::Completed);
    repositories.data.lock().unwrap().transactions = vec![transaction.clone()];
    assert_eq!(
        query
            .inspect(&profile())
            .unwrap()
            .summary(&ModId::new("a"))
            .status,
        InstallManifestStatus::Unknown
    );
    repositories
        .data
        .lock()
        .unwrap()
        .transactions
        .push(transaction);
    assert!(query.inspect(&profile()).is_err());
}

#[test]
fn session_includes_other_mods_affected_by_ownership_changes_and_never_reuses_a_read() {
    let repositories = Arc::new(Repositories::default());
    repositories.data.lock().unwrap().manifest =
        Some(manifest(vec![entry("a", "nativePC/shared.bin")]));
    let session = ModInstallationStateSession::new(service(&repositories));
    let first = session
        .read(&GameId::mhw(), &profile(), &[ModId::new("a")])
        .unwrap();
    repositories.data.lock().unwrap().manifest =
        Some(manifest(vec![entry("b", "nativePC/shared.bin")]));
    let next = session
        .read(&GameId::mhw(), &profile(), &[ModId::new("c")])
        .unwrap();
    assert_eq!(next.epoch, first.epoch);
    assert_eq!(next.revision, first.revision + 1);
    assert!(first.reset && !next.reset && next.available);
    assert_eq!(
        next.mod_ids,
        vec![ModId::new("a"), ModId::new("b"), ModId::new("c")]
    );
    assert_eq!(
        next.summaries[0].status,
        InstallManifestStatus::NotInstalled
    );
    assert_eq!(next.summaries[1].status, InstallManifestStatus::Installed);
    assert_eq!(repositories.reads.load(Ordering::Relaxed), 6);
}

#[test]
fn session_failure_is_versioned_and_clears_success_until_a_fresh_read() {
    let repositories = Arc::new(Repositories::default());
    let session = ModInstallationStateSession::new(service(&repositories));
    let read = || {
        session
            .read(&GameId::mhw(), &profile(), &[ModId::new("a")])
            .unwrap()
    };
    let first = read();
    repositories.data.lock().unwrap().failing = Some("manifest");
    let failed = read();
    assert!(!failed.available && failed.reset && failed.summaries.is_empty());
    assert_eq!(failed.revision, first.revision + 1);
    repositories.data.lock().unwrap().failing = None;
    let recovered = read();
    assert!(recovered.available && recovered.reset);
    assert_eq!(recovered.epoch, first.epoch);
    assert_eq!(recovered.revision, failed.revision + 1);
}

#[test]
fn scope_eviction_changes_epoch_and_requests_are_bounded() {
    let repositories = Arc::new(Repositories::default());
    let session = ModInstallationStateSession::new(service(&repositories));
    let game = GameId::mhw();
    let first_profile = ProfileId::new("000");
    let first = session.read(&game, &first_profile, &[]).unwrap();
    for index in 1..=16 {
        let update = session
            .read(&game, &ProfileId::new(format!("{index:03}")), &[])
            .unwrap();
        assert_ne!(update.epoch, first.epoch);
        assert_eq!(update.revision, 1);
    }
    let repeated = session.read(&game, &first_profile, &[]).unwrap();
    assert_ne!(repeated.epoch, first.epoch);
    assert!(repeated.reset);
    assert!(session
        .read(
            &game,
            &profile(),
            &vec![ModId::new("a"); crate::MAX_MOD_INSTALLATION_STATE_IDS + 1]
        )
        .is_err());
}
