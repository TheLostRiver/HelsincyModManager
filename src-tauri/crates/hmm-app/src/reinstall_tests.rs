use super::reinstall::*;
use anyhow::Result;
use hmm_core::{
    FileLayer, InstallConflict, InstallFileProvider, InstallManifest, InstallManifestEntry,
    InstallManifestStatus, InstallPlan, InstallTargetPath, InstalledFileSummary, ModId,
    ModRevisionId, PackageFileId, ProfileId, ReinstallRecoveryTransaction,
    ReinstallRecoveryTransactionStatus,
};
use hmm_ports::{
    InstallBackupStore, InstallGameFileSystem, InstallManifestRepository,
    ModImportResultRepository, ReinstallRecoveryTransactionRepository, StoredImportPreviewImage,
    StoredLogicalMod, StoredModImportAnalysis, StoredModOriginProvenance, StoredModPackageMetadata,
    StoredModRevision,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

#[test]
fn preview_fixture_returns_expected_counts_without_mutation_or_sensitive_facts() {
    let fixture = Fixture::ready();

    let preview = fixture.preview(default_request()).expect("ready preview");

    assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
    assert_eq!(preview.installed_revision, Some(revision_summary("v1")));
    assert_eq!(preview.candidate_revision, Some(revision_summary("v2")));
    assert_eq!(
        preview.counts,
        ReinstallTargetCounts {
            retained: 1,
            replaced: 2,
            added: 1,
            stale: 1,
        }
    );
    assert!(preview.plan_token.is_some());
    assert!(preview.blocking_reasons.is_empty());

    let public_debug = format!("{preview:?}");
    for secret in [
        "content/",
        "original-overwritten",
        "game-baseline-original",
        "candidate-added",
    ] {
        assert!(!public_debug.contains(secret));
    }
    fixture.assert_zero_mutations();
}

#[test]
fn preview_blocks_candidate_missing_owner_mismatch_unready_and_already_installed() {
    let missing = Fixture::ready();
    missing.catalog.remove_revision("v2");
    let preview = missing.preview(default_request()).expect("blocked preview");
    assert_blocked(&preview, ReinstallBlockingReason::CandidateNotFound);
    assert_eq!(preview.candidate_revision, None);
    assert_eq!(preview.plan_token, None);
    missing.assert_zero_mutations();

    let owner_mismatch = Fixture::ready();
    owner_mismatch
        .catalog
        .set_revision(candidate_revision("v2", "mod-b"));
    let preview = owner_mismatch
        .preview(default_request())
        .expect("blocked preview");
    assert_blocked(&preview, ReinstallBlockingReason::CandidateOwnerMismatch);
    owner_mismatch.assert_zero_mutations();

    let unready = Fixture::ready();
    unready
        .planner
        .set_error(ReinstallCandidatePlanError::NotReady);
    let preview = unready.preview(default_request()).expect("blocked preview");
    assert_blocked(&preview, ReinstallBlockingReason::CandidateNotReady);
    unready.assert_zero_mutations();

    let installed = Fixture::ready();
    let mut request = default_request();
    request.candidate_revision_id = ModRevisionId::new("v1");
    let preview = installed.preview(request).expect("blocked preview");
    assert_blocked(&preview, ReinstallBlockingReason::CandidateAlreadyInstalled);
    installed.assert_zero_mutations();

    let corrupt_lookup = Fixture::ready();
    corrupt_lookup
        .catalog
        .set_revision_for_lookup("v2", candidate_revision("v3", "mod-a"));
    assert_eq!(
        corrupt_lookup.preview(default_request()),
        Err(ReinstallPreviewError::CatalogUnavailable)
    );
    corrupt_lookup.assert_zero_mutations();
}

#[test]
fn preview_blocks_missing_or_unsafe_manifest_and_unknown_installed_revision() {
    let missing = Fixture::ready();
    missing.manifests.set_manifest(None);
    assert_blocked(
        &missing.preview(default_request()).expect("blocked preview"),
        ReinstallBlockingReason::NotInstalled,
    );

    let unsafe_manifest = Fixture::ready();
    unsafe_manifest
        .manifests
        .update_manifest(|manifest| manifest.status = InstallManifestStatus::Committing);
    assert_blocked(
        &unsafe_manifest
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::ManifestStateUnsafe,
    );

    let active_recovery = Fixture::ready();
    active_recovery.recovery.set_active(true);
    assert_blocked(
        &active_recovery
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::ManifestStateUnsafe,
    );

    let wrong_profile = Fixture::ready();
    wrong_profile.manifests.update_manifest(|manifest| {
        manifest.profile_id = ProfileId::new("other-profile");
    });
    assert_blocked(
        &wrong_profile
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::ManifestStateUnsafe,
    );

    let unsupported_schema = Fixture::ready();
    unsupported_schema
        .manifests
        .update_manifest(|manifest| manifest.schema_version = 999);
    assert_blocked(
        &unsupported_schema
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::ManifestStateUnsafe,
    );

    let unresolved = Fixture::ready();
    unresolved.catalog.set_logical_mod(None);
    assert_blocked(
        &unresolved
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::InstalledRevisionUnknown,
    );

    let mixed = Fixture::ready();
    mixed.manifests.update_manifest(|manifest| {
        manifest.schema_version = hmm_core::INSTALL_MANIFEST_SCHEMA_VERSION_V2;
        manifest.entries[0].revision_id = Some(ModRevisionId::new("v1"));
    });
    assert_blocked(
        &mixed.preview(default_request()).expect("blocked preview"),
        ReinstallBlockingReason::InstalledRevisionUnknown,
    );

    let missing_installed_revision = Fixture::ready();
    missing_installed_revision.catalog.remove_revision("v1");
    assert_blocked(
        &missing_installed_revision
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::InstalledRevisionUnknown,
    );

    let installed_owner_mismatch = Fixture::ready();
    installed_owner_mismatch
        .catalog
        .set_revision(candidate_revision("v1", "mod-b"));
    assert_blocked(
        &installed_owner_mismatch
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::InstalledRevisionUnknown,
    );

    for fixture in [
        missing,
        unsafe_manifest,
        active_recovery,
        wrong_profile,
        unsupported_schema,
        unresolved,
        mixed,
        missing_installed_revision,
        installed_owner_mismatch,
    ] {
        fixture.assert_zero_mutations();
    }
}

#[test]
fn preview_blocks_source_target_and_backup_preflight_failures() {
    let source = Fixture::ready();
    source.source.fail("added-v2");
    assert_blocked(
        &source.preview(default_request()).expect("blocked preview"),
        ReinstallBlockingReason::SourceUnavailable,
    );

    let target_missing = Fixture::ready();
    target_missing.game.remove_fixture("content/retained.bin");
    assert_blocked(
        &target_missing
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::TargetMissing,
    );

    let target_changed = Fixture::ready();
    target_changed
        .game
        .set_fixture("content/retained.bin", b"changed");
    assert_blocked(
        &target_changed
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::TargetChanged,
    );

    let target_read = Fixture::ready();
    target_read.game.fail_read("content/retained.bin");
    assert_blocked(
        &target_read
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::TargetReadFailed,
    );

    let backup_missing = Fixture::ready();
    backup_missing
        .backups
        .remove_fixture("original-overwritten");
    assert_blocked(
        &backup_missing
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::BackupMissing,
    );

    let backup_read = Fixture::ready();
    backup_read.backups.fail_read("original-overwritten");
    assert_blocked(
        &backup_read
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::BackupReadFailed,
    );

    for fixture in [
        source,
        target_missing,
        target_changed,
        target_read,
        backup_missing,
        backup_read,
    ] {
        fixture.assert_zero_mutations();
    }
}

#[test]
fn preview_blocks_plan_conflict_and_cross_mod_ownership() {
    let plan_conflict = Fixture::ready();
    let target = target("content/conflict.bin");
    plan_conflict.planner.set_plan(InstallPlan {
        actions: Vec::new(),
        conflicts: vec![InstallConflict {
            target_path: target.clone(),
            providers: vec![
                provider(&target, "mod-a", "a", 0),
                provider(&target, "mod-a", "b", 0),
            ],
        }],
    });
    assert_blocked(
        &plan_conflict
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::PlanConflict,
    );

    let provider_owner = Fixture::ready();
    let mut plan = candidate_plan();
    plan.actions[0].provider.mod_id = ModId::new("mod-b");
    provider_owner.planner.set_plan(plan);
    assert_blocked(
        &provider_owner
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::CrossModTargetConflict,
    );

    let target_owner = Fixture::ready();
    target_owner.manifests.update_manifest(|manifest| {
        manifest.entries.push(manifest_entry(
            "content/added-v2.bin",
            "mod-b",
            "other-owner",
            None,
            b"other",
        ));
    });
    assert_blocked(
        &target_owner
            .preview(default_request())
            .expect("blocked preview"),
        ReinstallBlockingReason::CrossModTargetConflict,
    );

    for fixture in [plan_conflict, provider_owner, target_owner] {
        fixture.assert_zero_mutations();
    }
}

#[test]
fn plan_token_changes_with_manifest_candidate_source_layer_target_and_backup_facts() {
    let base = Fixture::ready();
    let base_token = ready_token(&base, default_request());

    let manifest_changed = Fixture::ready();
    manifest_changed.manifests.update_manifest(|manifest| {
        manifest.entries[0].package_file_id = PackageFileId::new("retained-old-provider");
    });
    assert_ne!(
        base_token,
        ready_token(&manifest_changed, default_request())
    );

    let candidate_changed = Fixture::ready();
    candidate_changed
        .catalog
        .set_revision(candidate_revision("v3", "mod-a"));
    let mut candidate_request = default_request();
    candidate_request.candidate_revision_id = ModRevisionId::new("v3");
    assert_ne!(
        base_token,
        ready_token(&candidate_changed, candidate_request)
    );

    let source_changed = Fixture::ready();
    source_changed
        .source
        .set("added-v2", b"candidate-added-changed");
    assert_ne!(base_token, ready_token(&source_changed, default_request()));

    let layer_changed = Fixture::ready();
    let mut layer_request = default_request();
    layer_request.layer = FileLayer::new("overlay", 10);
    assert_ne!(base_token, ready_token(&layer_changed, layer_request));

    let target_changed = Fixture::ready();
    target_changed
        .game
        .set_fixture("content/added-v2.bin", b"unmanaged-pre-state");
    assert_ne!(base_token, ready_token(&target_changed, default_request()));

    let backup_changed = Fixture::ready();
    backup_changed
        .backups
        .set_fixture("original-overwritten", b"different-original-backup");
    assert_ne!(base_token, ready_token(&backup_changed, default_request()));
}

fn assert_blocked(preview: &ReinstallPlanPreview, reason: ReinstallBlockingReason) {
    assert_eq!(preview.status, ReinstallPreviewStatus::Blocked);
    assert_eq!(preview.plan_token, None);
    assert!(preview
        .blocking_reasons
        .iter()
        .any(|summary| summary.reason == reason && summary.count >= 1));
}

fn ready_token(fixture: &Fixture, request: ReinstallPreviewRequest) -> String {
    let preview = fixture.preview(request).expect("preview");
    assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
    preview.plan_token.expect("ready token")
}

fn default_request() -> ReinstallPreviewRequest {
    ReinstallPreviewRequest {
        game_id: hmm_core::GameId::mhw(),
        profile_id: ProfileId::new("default"),
        mod_id: ModId::new("mod-a"),
        candidate_revision_id: ModRevisionId::new("v2"),
        layer: FileLayer::new("base", 0),
    }
}

fn revision_summary(revision_id: &str) -> ReinstallRevisionSummary {
    ReinstallRevisionSummary {
        revision_id: ModRevisionId::new(revision_id),
    }
}

struct Fixture {
    service: ReinstallPreviewService,
    catalog: Arc<FakeCatalog>,
    planner: Arc<FakePlanner>,
    source: Arc<FakeCandidateSource>,
    game: Arc<FakeGameFiles>,
    backups: Arc<FakeBackups>,
    manifests: Arc<FakeManifests>,
    recovery: Arc<FakeRecoveryTransactions>,
}

impl Fixture {
    fn ready() -> Self {
        let catalog = Arc::new(FakeCatalog::ready());
        let planner = Arc::new(FakePlanner::new(candidate_plan()));
        let source = Arc::new(FakeCandidateSource::new([
            ("retained", b"same".as_slice()),
            ("replaced", b"candidate-replaced".as_slice()),
            ("overwritten", b"candidate-overwritten".as_slice()),
            ("added-v2", b"candidate-added".as_slice()),
        ]));
        let game = Arc::new(FakeGameFiles::new([
            ("content/retained.bin", b"same".as_slice()),
            ("content/replaced.bin", b"installed-replaced".as_slice()),
            (
                "content/overwritten.bin",
                b"installed-overwritten".as_slice(),
            ),
            ("content/stale.bin", b"installed-stale".as_slice()),
        ]));
        let backups = Arc::new(FakeBackups::new([(
            "original-overwritten",
            b"game-baseline-original".as_slice(),
        )]));
        let manifests = Arc::new(FakeManifests::new(Some(installed_manifest())));
        let recovery = Arc::new(FakeRecoveryTransactions::default());
        let service = ReinstallPreviewService::new(
            catalog.clone(),
            planner.clone(),
            source.clone(),
            game.clone(),
            backups.clone(),
            manifests.clone(),
            recovery.clone(),
        );
        Self {
            service,
            catalog,
            planner,
            source,
            game,
            backups,
            manifests,
            recovery,
        }
    }

    fn preview(
        &self,
        request: ReinstallPreviewRequest,
    ) -> Result<ReinstallPlanPreview, ReinstallPreviewError> {
        self.service.preview(request)
    }

    fn assert_zero_mutations(&self) {
        assert_eq!(self.game.write_count(), 0);
        assert_eq!(self.game.remove_count(), 0);
        assert_eq!(self.backups.store_count(), 0);
        assert_eq!(self.backups.remove_count(), 0);
        assert_eq!(self.manifests.save_count(), 0);
        assert_eq!(self.recovery.save_count(), 0);
        assert_eq!(self.recovery.remove_count(), 0);
    }
}

fn installed_manifest() -> InstallManifest {
    InstallManifest::completed(
        ProfileId::new("default"),
        vec![
            manifest_entry("content/retained.bin", "mod-a", "retained", None, b"same"),
            manifest_entry(
                "content/replaced.bin",
                "mod-a",
                "replaced",
                None,
                b"installed-replaced",
            ),
            manifest_entry(
                "content/overwritten.bin",
                "mod-a",
                "overwritten",
                Some("original-overwritten"),
                b"installed-overwritten",
            ),
            manifest_entry(
                "content/stale.bin",
                "mod-a",
                "stale",
                None,
                b"installed-stale",
            ),
        ],
    )
}

fn candidate_plan() -> InstallPlan {
    InstallPlan::from_providers([
        candidate_provider("content/retained.bin", "retained"),
        candidate_provider("content/replaced.bin", "replaced"),
        candidate_provider("content/overwritten.bin", "overwritten"),
        candidate_provider("content/added-v2.bin", "added-v2"),
    ])
}

fn candidate_provider(path: &str, package_file_id: &str) -> InstallFileProvider {
    let target = target(path);
    provider(&target, "mod-a", package_file_id, 0)
}

fn provider(
    target_path: &InstallTargetPath,
    mod_id: &str,
    package_file_id: &str,
    priority: i32,
) -> InstallFileProvider {
    InstallFileProvider::new(
        ModId::new(mod_id),
        PackageFileId::new(package_file_id),
        target_path.clone(),
        FileLayer::new("base", priority),
    )
}

fn manifest_entry(
    path: &str,
    mod_id: &str,
    package_file_id: &str,
    backup_ref: Option<&str>,
    bytes: &[u8],
) -> InstallManifestEntry {
    InstallManifestEntry {
        target_path: target(path),
        mod_id: ModId::new(mod_id),
        revision_id: None,
        package_file_id: PackageFileId::new(package_file_id),
        layer: FileLayer::new("base", 0),
        backup_ref: backup_ref.map(str::to_owned),
        installed_file: Some(summary(bytes)),
    }
}

fn target(path: &str) -> InstallTargetPath {
    InstallTargetPath::parse(path, ["content"]).expect("target")
}

fn summary(bytes: &[u8]) -> InstalledFileSummary {
    use sha2::{Digest, Sha256};
    InstalledFileSummary {
        size_bytes: bytes.len() as u64,
        sha256: Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    }
}

fn logical_mod() -> StoredLogicalMod {
    StoredLogicalMod {
        mod_id: ModId::new("mod-a"),
        origin_revision_id: ModRevisionId::new("v1"),
        display_revision_id: ModRevisionId::new("v2"),
        origin_provenance: StoredModOriginProvenance::Imported,
    }
}

fn candidate_revision(revision_id: &str, mod_id: &str) -> StoredModRevision {
    StoredModRevision {
        revision_id: ModRevisionId::new(revision_id),
        mod_id: ModId::new(mod_id),
        import_task_id: format!("task-{revision_id}"),
        package_id: format!("package-{revision_id}"),
        display_name: format!("Revision {revision_id}"),
        metadata: StoredModPackageMetadata::default(),
        preview_image: StoredImportPreviewImage::Fallback {
            reason: hmm_core::PreviewImageRejectionReason::Missing,
        },
    }
}

struct FakeCatalog {
    logical_mod: Mutex<Option<StoredLogicalMod>>,
    revisions: Mutex<BTreeMap<ModRevisionId, StoredModRevision>>,
}

impl FakeCatalog {
    fn ready() -> Self {
        Self {
            logical_mod: Mutex::new(Some(logical_mod())),
            revisions: Mutex::new(BTreeMap::from([
                (ModRevisionId::new("v1"), candidate_revision("v1", "mod-a")),
                (ModRevisionId::new("v2"), candidate_revision("v2", "mod-a")),
            ])),
        }
    }

    fn set_logical_mod(&self, value: Option<StoredLogicalMod>) {
        *self.logical_mod.lock().expect("logical mod lock") = value;
    }

    fn set_revision(&self, revision: StoredModRevision) {
        self.revisions
            .lock()
            .expect("revisions lock")
            .insert(revision.revision_id.clone(), revision);
    }

    fn set_revision_for_lookup(&self, lookup_id: &str, revision: StoredModRevision) {
        self.revisions
            .lock()
            .expect("revisions lock")
            .insert(ModRevisionId::new(lookup_id), revision);
    }

    fn remove_revision(&self, revision_id: &str) {
        self.revisions
            .lock()
            .expect("revisions lock")
            .remove(&ModRevisionId::new(revision_id));
    }
}

impl ModImportResultRepository for FakeCatalog {
    fn get_mod(&self, mod_id: &ModId) -> Result<Option<StoredLogicalMod>> {
        Ok(self
            .logical_mod
            .lock()
            .expect("logical mod lock")
            .clone()
            .filter(|logical_mod| logical_mod.mod_id == *mod_id))
    }

    fn get_revision(&self, revision_id: &ModRevisionId) -> Result<Option<StoredModRevision>> {
        Ok(self
            .revisions
            .lock()
            .expect("revisions lock")
            .get(revision_id)
            .cloned())
    }

    fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> Result<()> {
        Ok(())
    }

    fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>> {
        Ok(Vec::new())
    }

    fn get_analysis(&self, _mod_id: &str) -> Result<Option<StoredModImportAnalysis>> {
        Ok(None)
    }
}

struct FakePlanner {
    result: Mutex<Result<InstallPlan, ReinstallCandidatePlanError>>,
}

impl FakePlanner {
    fn new(plan: InstallPlan) -> Self {
        Self {
            result: Mutex::new(Ok(plan)),
        }
    }

    fn set_plan(&self, plan: InstallPlan) {
        *self.result.lock().expect("planner lock") = Ok(plan);
    }

    fn set_error(&self, error: ReinstallCandidatePlanError) {
        *self.result.lock().expect("planner lock") = Err(error);
    }
}

impl ReinstallCandidatePlanner for FakePlanner {
    fn build_candidate_plan(
        &self,
        _request: ReinstallCandidatePlanRequest<'_>,
    ) -> Result<InstallPlan, ReinstallCandidatePlanError> {
        self.result.lock().expect("planner lock").clone()
    }
}

struct FakeCandidateSource {
    files: Mutex<BTreeMap<String, Vec<u8>>>,
    failures: Mutex<BTreeSet<String>>,
}

impl FakeCandidateSource {
    fn new<const N: usize>(files: [(&str, &[u8]); N]) -> Self {
        Self {
            files: Mutex::new(
                files
                    .into_iter()
                    .map(|(id, bytes)| (id.to_owned(), bytes.to_vec()))
                    .collect(),
            ),
            failures: Mutex::new(BTreeSet::new()),
        }
    }

    fn set(&self, id: &str, bytes: &[u8]) {
        self.files
            .lock()
            .expect("source files lock")
            .insert(id.to_owned(), bytes.to_vec());
    }

    fn fail(&self, id: &str) {
        self.failures
            .lock()
            .expect("source failures lock")
            .insert(id.to_owned());
    }
}

impl ReinstallCandidateSourceReader for FakeCandidateSource {
    fn read_candidate_source_file(
        &self,
        _candidate: &StoredModRevision,
        package_file_id: &PackageFileId,
    ) -> Result<Vec<u8>> {
        if self
            .failures
            .lock()
            .expect("source failures lock")
            .contains(package_file_id.as_str())
        {
            anyhow::bail!("source failure");
        }
        self.files
            .lock()
            .expect("source files lock")
            .get(package_file_id.as_str())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("source missing"))
    }
}

#[derive(Default)]
struct FakeGameFiles {
    files: Mutex<BTreeMap<String, Vec<u8>>>,
    read_failures: Mutex<BTreeSet<String>>,
    writes: Mutex<usize>,
    removes: Mutex<usize>,
}

impl FakeGameFiles {
    fn new<const N: usize>(files: [(&str, &[u8]); N]) -> Self {
        Self {
            files: Mutex::new(
                files
                    .into_iter()
                    .map(|(path, bytes)| (path.to_owned(), bytes.to_vec()))
                    .collect(),
            ),
            ..Self::default()
        }
    }

    fn set_fixture(&self, path: &str, bytes: &[u8]) {
        self.files
            .lock()
            .expect("game files lock")
            .insert(path.to_owned(), bytes.to_vec());
    }

    fn remove_fixture(&self, path: &str) {
        self.files.lock().expect("game files lock").remove(path);
    }

    fn fail_read(&self, path: &str) {
        self.read_failures
            .lock()
            .expect("game failures lock")
            .insert(path.to_owned());
    }

    fn write_count(&self) -> usize {
        *self.writes.lock().expect("writes lock")
    }

    fn remove_count(&self) -> usize {
        *self.removes.lock().expect("removes lock")
    }
}

impl InstallGameFileSystem for FakeGameFiles {
    fn read_game_file(&self, target_path: &InstallTargetPath) -> Result<Option<Vec<u8>>> {
        if self
            .read_failures
            .lock()
            .expect("game failures lock")
            .contains(target_path.as_str())
        {
            anyhow::bail!("target read failure");
        }
        Ok(self
            .files
            .lock()
            .expect("game files lock")
            .get(target_path.as_str())
            .cloned())
    }

    fn write_game_file(&self, _target_path: &InstallTargetPath, _bytes: &[u8]) -> Result<()> {
        *self.writes.lock().expect("writes lock") += 1;
        Ok(())
    }

    fn remove_game_file(&self, _target_path: &InstallTargetPath) -> Result<()> {
        *self.removes.lock().expect("removes lock") += 1;
        Ok(())
    }
}

#[derive(Default)]
struct FakeBackups {
    files: Mutex<BTreeMap<String, Vec<u8>>>,
    read_failures: Mutex<BTreeSet<String>>,
    stores: Mutex<usize>,
    removes: Mutex<usize>,
}

impl FakeBackups {
    fn new<const N: usize>(files: [(&str, &[u8]); N]) -> Self {
        Self {
            files: Mutex::new(
                files
                    .into_iter()
                    .map(|(reference, bytes)| (reference.to_owned(), bytes.to_vec()))
                    .collect(),
            ),
            ..Self::default()
        }
    }

    fn remove_fixture(&self, reference: &str) {
        self.files
            .lock()
            .expect("backup files lock")
            .remove(reference);
    }

    fn set_fixture(&self, reference: &str, bytes: &[u8]) {
        self.files
            .lock()
            .expect("backup files lock")
            .insert(reference.to_owned(), bytes.to_vec());
    }

    fn fail_read(&self, reference: &str) {
        self.read_failures
            .lock()
            .expect("backup failures lock")
            .insert(reference.to_owned());
    }

    fn store_count(&self) -> usize {
        *self.stores.lock().expect("stores lock")
    }

    fn remove_count(&self) -> usize {
        *self.removes.lock().expect("removes lock")
    }
}

impl InstallBackupStore for FakeBackups {
    fn store_backup(&self, _target_path: &InstallTargetPath, _bytes: &[u8]) -> Result<String> {
        *self.stores.lock().expect("stores lock") += 1;
        Ok("unexpected-store".to_owned())
    }

    fn read_backup(&self, backup_ref: &str) -> Result<Option<Vec<u8>>> {
        if self
            .read_failures
            .lock()
            .expect("backup failures lock")
            .contains(backup_ref)
        {
            anyhow::bail!("backup read failure");
        }
        Ok(self
            .files
            .lock()
            .expect("backup files lock")
            .get(backup_ref)
            .cloned())
    }

    fn remove_backup(&self, _backup_ref: &str) -> Result<()> {
        *self.removes.lock().expect("removes lock") += 1;
        Ok(())
    }
}

struct FakeManifests {
    manifest: Mutex<Option<InstallManifest>>,
    saves: Mutex<usize>,
}

impl FakeManifests {
    fn new(manifest: Option<InstallManifest>) -> Self {
        Self {
            manifest: Mutex::new(manifest),
            saves: Mutex::new(0),
        }
    }

    fn set_manifest(&self, manifest: Option<InstallManifest>) {
        *self.manifest.lock().expect("manifest lock") = manifest;
    }

    fn update_manifest(&self, update: impl FnOnce(&mut InstallManifest)) {
        update(
            self.manifest
                .lock()
                .expect("manifest lock")
                .as_mut()
                .expect("manifest"),
        );
    }

    fn save_count(&self) -> usize {
        *self.saves.lock().expect("manifest saves lock")
    }
}

impl InstallManifestRepository for FakeManifests {
    fn load_manifest(&self, _profile_id: &ProfileId) -> Result<Option<InstallManifest>> {
        Ok(self.manifest.lock().expect("manifest lock").clone())
    }

    fn save_manifest(&self, _manifest: &InstallManifest) -> Result<()> {
        *self.saves.lock().expect("manifest saves lock") += 1;
        Ok(())
    }
}

#[derive(Default)]
struct FakeRecoveryTransactions {
    active: Mutex<bool>,
    saves: Mutex<usize>,
    removes: Mutex<usize>,
}

impl FakeRecoveryTransactions {
    fn set_active(&self, active: bool) {
        *self.active.lock().expect("active lock") = active;
    }

    fn save_count(&self) -> usize {
        *self.saves.lock().expect("recovery saves lock")
    }

    fn remove_count(&self) -> usize {
        *self.removes.lock().expect("recovery removes lock")
    }
}

impl ReinstallRecoveryTransactionRepository for FakeRecoveryTransactions {
    fn load_transaction(
        &self,
        _profile_id: &ProfileId,
        _mod_id: &ModId,
    ) -> Result<Option<ReinstallRecoveryTransaction>> {
        if *self.active.lock().expect("active lock") {
            return Ok(Some(ReinstallRecoveryTransaction {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                old_revision_id: ModRevisionId::new("v1"),
                candidate_revision_id: ModRevisionId::new("v2"),
                plan_token: "active-preview-token".to_owned(),
                plan_hash: "active-plan-hash".to_owned(),
                status: ReinstallRecoveryTransactionStatus::Planned,
                pre_reinstall_manifest: installed_manifest(),
                targets: Vec::new(),
            }));
        }
        Ok(None)
    }

    fn list_transactions(
        &self,
        _profile_id: &ProfileId,
    ) -> Result<Vec<ReinstallRecoveryTransaction>> {
        Ok(Vec::new())
    }

    fn save_transaction(&self, _transaction: &ReinstallRecoveryTransaction) -> Result<()> {
        *self.saves.lock().expect("recovery saves lock") += 1;
        Ok(())
    }

    fn remove_transaction(&self, _profile_id: &ProfileId, _mod_id: &ModId) -> Result<()> {
        *self.removes.lock().expect("recovery removes lock") += 1;
        Ok(())
    }
}
