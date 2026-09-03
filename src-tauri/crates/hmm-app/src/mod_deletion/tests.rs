use super::*;
use hmm_core::{
    FileLayer, GameId, InstallManifest, InstallManifestEntry, InstallManifestStatus,
    InstallTargetPath, ModMetadataOverlay, PackageFileId, Profile, ReinstallRecoveryTransaction,
    ReinstallRecoveryTransactionStatus, ReplacementBindingSnapshot,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, CategoryRepository, InstallManifestRepository,
    ModImportResultRepository, ModImportSandboxLocator, ModMetadataRepository, ProfileRepository,
    ReinstallRecoveryTransactionRepository, ReplacementSelectionRepository,
    StoredModImportAnalysis, StoredModPackageMetadata, ThumbnailStore,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

fn mod_id() -> ModId {
    ModId::new("mod-a")
}

fn revision(mod_id: &ModId, package_id: &str) -> hmm_ports::StoredModRevision {
    hmm_ports::StoredModRevision {
        revision_id: hmm_core::ModRevisionId::new(package_id),
        mod_id: mod_id.clone(),
        import_task_id: package_id.to_owned(),
        package_id: package_id.to_owned(),
        display_name: "Fixture Mod".to_owned(),
        metadata: StoredModPackageMetadata::default(),
        preview_image: hmm_ports::StoredImportPreviewImage::Fallback {
            reason: hmm_core::PreviewImageRejectionReason::Missing,
        },
    }
}

fn manifest_entry(mod_id: &ModId) -> InstallManifestEntry {
    InstallManifestEntry {
        target_path: InstallTargetPath::parse(
            "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
            ["nativePC"],
        )
        .expect("entry path"),
        mod_id: mod_id.clone(),
        revision_id: Some(hmm_core::ModRevisionId::new("package-r1")),
        package_file_id: PackageFileId::new("package-r1"),
        layer: FileLayer::new("base", 0),
        backup_ref: None,
        installed_file: None,
        adopted: false,
    }
}

fn manifest(status: InstallManifestStatus, mod_id: &ModId) -> InstallManifest {
    InstallManifest {
        profile_id: ProfileId::new("p1"),
        manifest_id: "manifest".to_owned(),
        schema_version: 2,
        schema_migration: None,
        backend: None,
        status,
        created_at: None,
        completed_at: None,
        plan_hash: None,
        entries: vec![manifest_entry(mod_id)],
        replacement_bindings: Vec::new(),
    }
}

fn profile(id: &str) -> Profile {
    Profile {
        id: id.to_owned(),
        name: id.to_owned(),
        description: None,
        is_active: false,
        created_at: 0,
        updated_at: 0,
    }
}

struct FakeProfileRepository {
    profiles: Vec<Profile>,
}

impl ProfileRepository for FakeProfileRepository {
    fn get(&self, profile_id: &str) -> anyhow::Result<Option<Profile>> {
        Ok(self.profiles.iter().find(|p| p.id == profile_id).cloned())
    }
    fn save(&self, _profile: &Profile) -> anyhow::Result<()> {
        Ok(())
    }
    fn delete(&self, _profile_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn list_all(&self) -> anyhow::Result<Vec<Profile>> {
        Ok(self.profiles.clone())
    }
    fn get_active(&self) -> anyhow::Result<Option<Profile>> {
        Ok(self.profiles.first().cloned())
    }
    fn set_active(&self, _profile_id: &str, _updated_at: u128) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct FakeInstallManifestRepository {
    manifests: Mutex<HashMap<String, InstallManifest>>,
}

impl InstallManifestRepository for FakeInstallManifestRepository {
    fn load_manifest(&self, profile_id: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        let manifests = self.manifests.lock().expect("manifest lock");
        Ok(manifests.get(profile_id.as_str()).cloned())
    }
    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        self.manifests
            .lock()
            .expect("manifest lock")
            .insert(manifest.profile_id.as_str().to_owned(), manifest.clone());
        Ok(())
    }
}

#[derive(Default)]
struct FakeReinstallRecoveryRepository {
    transactions: Mutex<Vec<ReinstallRecoveryTransaction>>,
}

impl ReinstallRecoveryTransactionRepository for FakeReinstallRecoveryRepository {
    fn load_transaction(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> anyhow::Result<Option<ReinstallRecoveryTransaction>> {
        Ok(self
            .transactions
            .lock()
            .expect("recovery lock")
            .iter()
            .find(|t| &t.profile_id == profile_id && &t.mod_id == mod_id)
            .cloned())
    }
    fn list_transactions(
        &self,
        profile_id: &ProfileId,
    ) -> anyhow::Result<Vec<ReinstallRecoveryTransaction>> {
        Ok(self
            .transactions
            .lock()
            .expect("recovery lock")
            .iter()
            .filter(|t| &t.profile_id == profile_id)
            .cloned()
            .collect())
    }
    fn save_transaction(&self, transaction: &ReinstallRecoveryTransaction) -> anyhow::Result<()> {
        self.transactions
            .lock()
            .expect("recovery lock")
            .push(transaction.clone());
        Ok(())
    }
    fn remove_transaction(&self, _profile_id: &ProfileId, _mod_id: &ModId) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct FakeImportResultRepository {
    mods: Mutex<Vec<hmm_ports::StoredLogicalMod>>,
    revisions: Mutex<Vec<hmm_ports::StoredModRevision>>,
    removed: Mutex<Vec<String>>,
}

impl FakeImportResultRepository {
    fn with_revisions(mod_id: &ModId, package_ids: &[&str]) -> Self {
        let revisions = package_ids
            .iter()
            .map(|package_id| revision(mod_id, package_id))
            .collect();
        Self {
            mods: Mutex::new(vec![hmm_ports::StoredLogicalMod {
                mod_id: mod_id.clone(),
                origin_revision_id: hmm_core::ModRevisionId::new(package_ids[0]),
                display_revision_id: hmm_core::ModRevisionId::new(package_ids[0]),
                origin_provenance: hmm_ports::StoredModOriginProvenance::Imported,
            }]),
            revisions: Mutex::new(revisions),
            removed: Mutex::new(Vec::new()),
        }
    }
}

impl ModImportResultRepository for FakeImportResultRepository {
    fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> anyhow::Result<()> {
        Ok(())
    }
    fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
        let revisions = self.revisions.lock().expect("revisions lock");
        Ok(revisions
            .iter()
            .map(|revision| StoredModImportAnalysis {
                mod_id: revision.mod_id.as_str().to_owned(),
                task_id: revision.import_task_id.clone(),
                package_id: revision.package_id.clone(),
                display_name: revision.display_name.clone(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: revision.preview_image.clone(),
            })
            .collect())
    }
    fn get_analysis(&self, mod_id: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
        Ok(self
            .list_analysis()?
            .into_iter()
            .find(|a| a.mod_id == mod_id))
    }
    fn get_mod(&self, mod_id: &ModId) -> anyhow::Result<Option<hmm_ports::StoredLogicalMod>> {
        Ok(self
            .mods
            .lock()
            .expect("mods lock")
            .iter()
            .find(|logical_mod| &logical_mod.mod_id == mod_id)
            .cloned())
    }
    fn list_mods(&self) -> anyhow::Result<Vec<hmm_ports::StoredLogicalMod>> {
        Ok(self.mods.lock().expect("mods lock").clone())
    }
    fn get_revision(
        &self,
        revision_id: &hmm_core::ModRevisionId,
    ) -> anyhow::Result<Option<hmm_ports::StoredModRevision>> {
        Ok(self
            .revisions
            .lock()
            .expect("revisions lock")
            .iter()
            .find(|revision| &revision.revision_id == revision_id)
            .cloned())
    }
    fn list_revisions(&self, mod_id: &ModId) -> anyhow::Result<Vec<hmm_ports::StoredModRevision>> {
        Ok(self
            .revisions
            .lock()
            .expect("revisions lock")
            .iter()
            .filter(|revision| &revision.mod_id == mod_id)
            .cloned()
            .collect())
    }
    fn remove_mod_with_revisions(&self, mod_id: &ModId) -> anyhow::Result<Vec<String>> {
        let package_ids = self
            .list_revisions(mod_id)?
            .into_iter()
            .map(|revision| revision.package_id)
            .collect::<Vec<_>>();
        self.revisions
            .lock()
            .expect("revisions lock")
            .retain(|revision| &revision.mod_id != mod_id);
        self.mods
            .lock()
            .expect("mods lock")
            .retain(|logical_mod| &logical_mod.mod_id != mod_id);
        self.removed
            .lock()
            .expect("removed lock")
            .extend(package_ids.iter().cloned());
        Ok(package_ids)
    }
}

#[derive(Default)]
struct FakeSandboxLocator {
    cleaned: Mutex<Vec<String>>,
}

impl ModImportSandboxLocator for FakeSandboxLocator {
    fn sandbox_root_for_package(&self, package_id: &str) -> anyhow::Result<PathBuf> {
        Ok(PathBuf::from(format!("sandbox/{package_id}")))
    }
    fn cleanup_sandbox_for_package(&self, package_id: &str) -> anyhow::Result<()> {
        self.cleaned
            .lock()
            .expect("cleaned lock")
            .push(package_id.to_owned());
        Ok(())
    }
}

#[derive(Default)]
struct FakeThumbnailStore {
    removed: Mutex<Vec<String>>,
}

impl ThumbnailStore for FakeThumbnailStore {
    fn put_thumbnail(
        &self,
        _package_id: &str,
        _content_hash: &str,
        _variant: &str,
        _extension: &str,
        _bytes: &[u8],
    ) -> anyhow::Result<hmm_ports::ThumbnailRef> {
        anyhow::bail!("not used in delete tests")
    }
    fn resolve_url(&self, thumbnail_ref: &hmm_ports::ThumbnailRef) -> anyhow::Result<String> {
        Ok(format!("thumbnail://{}", thumbnail_ref.package_id))
    }
    fn remove_package_thumbnails(&self, package_id: &str) -> anyhow::Result<()> {
        self.removed
            .lock()
            .expect("removed lock")
            .push(package_id.to_owned());
        Ok(())
    }
}

#[derive(Default)]
struct FakeMetadataRepository {
    deleted: Mutex<Vec<String>>,
}

impl ModMetadataRepository for FakeMetadataRepository {
    fn get(&self, _mod_id: &str) -> anyhow::Result<Option<ModMetadataOverlay>> {
        Ok(None)
    }
    fn save(&self, _overlay: &ModMetadataOverlay) -> anyhow::Result<()> {
        Ok(())
    }
    fn delete(&self, mod_id: &str) -> anyhow::Result<()> {
        self.deleted
            .lock()
            .expect("deleted lock")
            .push(mod_id.to_owned());
        Ok(())
    }
    fn list_all(&self) -> anyhow::Result<Vec<ModMetadataOverlay>> {
        Ok(Vec::new())
    }
}

#[derive(Default)]
struct FakeCategoryRepository {
    pairs: Mutex<Vec<(String, hmm_core::Category)>>,
    cleared: Mutex<Vec<String>>,
}

impl CategoryRepository for FakeCategoryRepository {
    fn get(&self, _category_id: &str) -> anyhow::Result<Option<hmm_core::Category>> {
        Ok(None)
    }
    fn save(&self, _category: &hmm_core::Category) -> anyhow::Result<()> {
        Ok(())
    }
    fn delete(&self, _category_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn list_all(&self) -> anyhow::Result<Vec<hmm_core::Category>> {
        Ok(Vec::new())
    }
    fn count_mods(&self, _category_id: &str) -> anyhow::Result<u32> {
        Ok(0)
    }
    fn get_mod_categories(&self, _mod_id: &str) -> anyhow::Result<Vec<hmm_core::Category>> {
        Ok(Vec::new())
    }
    fn set_mod_categories(&self, mod_id: &str, _category_ids: &[String]) -> anyhow::Result<()> {
        self.cleared
            .lock()
            .expect("cleared lock")
            .push(mod_id.to_owned());
        Ok(())
    }
    fn list_mod_category_pairs(&self) -> anyhow::Result<Vec<(String, hmm_core::Category)>> {
        Ok(self.pairs.lock().expect("pairs lock").clone())
    }
}

#[derive(Default)]
struct RecordingAuditLogWriter {
    events: Mutex<Vec<AuditLogEvent>>,
}

impl AuditLogWriter for RecordingAuditLogWriter {
    fn record(&self, event: AuditLogEvent) -> anyhow::Result<()> {
        self.events.lock().expect("audit lock").push(event);
        Ok(())
    }
}

struct FixedClock;

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> anyhow::Result<u128> {
        Ok(42)
    }
}

struct DeletionHarness {
    service: ModDeletionService,
    selections:
        Arc<crate::replacement_selection_test_support::InMemoryReplacementSelectionRepository>,
    results_impl: Arc<FakeImportResultRepository>,
    sandbox_impl: Arc<FakeSandboxLocator>,
    thumbnails_impl: Arc<FakeThumbnailStore>,
    metadata_impl: Arc<FakeMetadataRepository>,
    categories_impl: Arc<FakeCategoryRepository>,
    audit_impl: Arc<RecordingAuditLogWriter>,
}
fn harness(
    mod_id: &ModId,
    manifests: HashMap<String, InstallManifest>,
    with_selection: bool,
) -> DeletionHarness {
    let profiles = vec![profile("p1"), profile("p2")];
    let manifests = Arc::new(FakeInstallManifestRepository {
        manifests: Mutex::new(manifests),
    });
    let selections = Arc::new(
        crate::replacement_selection_test_support::InMemoryReplacementSelectionRepository::default(
        ),
    );
    if with_selection {
        selections
            .save_selection(&installed_binding(mod_id))
            .expect("save selection");
    }
    let results = Arc::new(FakeImportResultRepository::with_revisions(
        mod_id,
        &["package-r1", "package-r2"],
    ));
    let sandbox = Arc::new(FakeSandboxLocator::default());
    let thumbnails = Arc::new(FakeThumbnailStore::default());
    let metadata = Arc::new(FakeMetadataRepository::default());
    let categories = Arc::new(FakeCategoryRepository::default());
    let audit = Arc::new(RecordingAuditLogWriter::default());
    let service = ModDeletionService::new(
        Arc::new(FakeProfileRepository { profiles }),
        Arc::clone(&manifests) as Arc<dyn InstallManifestRepository>,
        Arc::new(FakeReinstallRecoveryRepository::default()),
        Arc::clone(&selections) as Arc<dyn ReplacementSelectionRepository>,
        Arc::clone(&results) as Arc<dyn ModImportResultRepository>,
        Arc::clone(&sandbox) as Arc<dyn ModImportSandboxLocator>,
        Arc::clone(&thumbnails) as Arc<dyn ThumbnailStore>,
        Arc::clone(&metadata) as Arc<dyn ModMetadataRepository>,
        Arc::clone(&categories) as Arc<dyn CategoryRepository>,
        Arc::clone(&audit) as Arc<dyn AuditLogWriter>,
        Arc::new(FixedClock),
    );
    DeletionHarness {
        service,
        selections,
        results_impl: Arc::clone(&results),
        sandbox_impl: Arc::clone(&sandbox),
        thumbnails_impl: Arc::clone(&thumbnails),
        metadata_impl: Arc::clone(&metadata),
        categories_impl: Arc::clone(&categories),
        audit_impl: Arc::clone(&audit),
    }
}

fn installed_binding(mod_id: &ModId) -> ReplacementBindingSnapshot {
    let source = hmm_core::ReplacementSource::new(
        hmm_core::ReplacementSourceId::parse("mhw:weapon:one:one001").expect("source id"),
        GameId::mhw(),
        hmm_core::ReplacementTargetKind::parse("weapon").expect("kind"),
        "one001",
        "wp/one",
        true,
    )
    .expect("source");
    ReplacementBindingSnapshot::new(
        hmm_core::ReplacementBinding::new(
            hmm_core::ReplacementBindingId::parse("binding-selection").expect("binding id"),
            mod_id.clone(),
            ProfileId::new("p1"),
            source.id().clone(),
            hmm_core::ReplacementTargetId::parse("mhw:weapon:one:one002").expect("target id"),
            42,
        )
        .expect("binding"),
        None,
        "one001",
        "one002",
        "wp/one",
        "wp/one",
        hmm_core::ReplacementTargetKind::parse("weapon").expect("target kind"),
    )
    .expect("installed binding")
}

#[test]
fn delete_fails_closed_when_mod_is_installed_in_any_profile() {
    let mod_id = mod_id();
    let harness = harness(
        &mod_id,
        HashMap::from([
            (
                "p1".to_owned(),
                manifest(InstallManifestStatus::Completed, &mod_id),
            ),
            (
                "p2".to_owned(),
                manifest(InstallManifestStatus::RolledBack, &mod_id),
            ),
        ]),
        false,
    );

    let error = harness
        .service
        .delete_mod(&mod_id)
        .expect_err("installed fact must block deletion");

    assert_eq!(
        error,
        ModDeletionError::BlockedInstalled {
            profiles: "p1, p2".to_owned(),
        }
    );
    // 权威目录未被触碰。
    assert!(harness
        .results_impl
        .get_mod(&mod_id)
        .expect("read")
        .is_some());
    assert!(harness
        .sandbox_impl
        .cleaned
        .lock()
        .expect("lock")
        .is_empty());
}

#[test]
fn delete_fails_closed_when_recovery_state_is_pending() {
    let mod_id = mod_id();
    let manifests = HashMap::from([(
        "p1".to_owned(),
        manifest(InstallManifestStatus::RollbackRequired, &mod_id),
    )]);
    let harness = harness(&mod_id, manifests, false);

    let error = harness
        .service
        .delete_mod(&mod_id)
        .expect_err("failed manifest state must block deletion");

    assert_eq!(error, ModDeletionError::BlockedRecovery);
}

#[test]
fn delete_fails_closed_when_reinstall_recovery_transaction_exists() {
    let mod_id = mod_id();
    let harness = harness(&mod_id, HashMap::new(), false);
    harness
        .service
        .reinstall_recovery
        .save_transaction(&ReinstallRecoveryTransaction {
            profile_id: ProfileId::new("p1"),
            mod_id: mod_id.clone(),
            old_revision_id: hmm_core::ModRevisionId::new("package-r1"),
            candidate_revision_id: hmm_core::ModRevisionId::new("package-r2"),
            plan_token: "token".to_owned(),
            plan_hash: "hash".to_owned(),
            status: ReinstallRecoveryTransactionStatus::RepairRequired,
            pre_reinstall_manifest: manifest(InstallManifestStatus::Completed, &mod_id),
            candidate_replacement_bindings: Vec::new(),
            targets: Vec::new(),
        })
        .expect("save transaction");

    let error = harness
        .service
        .delete_mod(&mod_id)
        .expect_err("pending reinstall recovery must block deletion");

    assert_eq!(error, ModDeletionError::BlockedRecovery);
}

#[test]
fn delete_reclaims_storage_and_catalog_for_uninstalled_mod() {
    let mod_id = mod_id();
    let harness = harness(&mod_id, HashMap::new(), true);
    harness
        .selections
        .save_selection(&installed_binding(&mod_id))
        .expect("save selection");

    let result = harness
        .service
        .delete_mod(&mod_id)
        .expect("uninstalled mod deletes cleanly");

    assert_eq!(result.removed_revision_count, 2);
    assert_eq!(
        result.removed_package_ids,
        vec!["package-r1".to_owned(), "package-r2".to_owned()]
    );
    assert_eq!(
        harness.sandbox_impl.cleaned.lock().expect("lock").clone(),
        vec!["package-r1".to_owned(), "package-r2".to_owned()]
    );
    assert_eq!(
        harness
            .thumbnails_impl
            .removed
            .lock()
            .expect("lock")
            .clone(),
        vec!["package-r1".to_owned(), "package-r2".to_owned()]
    );
    assert!(harness
        .results_impl
        .get_mod(&mod_id)
        .expect("read")
        .is_none());
    assert!(harness
        .results_impl
        .list_revisions(&mod_id)
        .expect("read")
        .is_empty());
    assert_eq!(
        harness.metadata_impl.deleted.lock().expect("lock").clone(),
        vec![mod_id.as_str().to_owned()]
    );
    assert_eq!(
        harness
            .categories_impl
            .cleared
            .lock()
            .expect("lock")
            .clone(),
        vec![mod_id.as_str().to_owned()]
    );
    // 选择意图随删除清除。
    assert!(harness
        .selections
        .load_selection(&ProfileId::new("p1"), &mod_id)
        .expect("read")
        .is_none());
    let audit = harness.audit_impl.events.lock().expect("lock");
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].operation, "delete_mod");
    assert_eq!(audit[0].result, "success");
}

#[test]
fn delete_reports_target_not_found_for_unknown_mod() {
    let harness = harness(&ModId::new("mod-a"), HashMap::new(), false);
    let error = harness
        .service
        .delete_mod(&ModId::new("missing"))
        .expect_err("unknown mod cannot be deleted");
    assert_eq!(error, ModDeletionError::ModNotFound);
}

#[test]
fn preview_reports_counts_and_affected_profiles() {
    let mod_id = mod_id();
    let harness = harness(
        &mod_id,
        HashMap::from([(
            "p2".to_owned(),
            manifest(InstallManifestStatus::Completed, &mod_id),
        )]),
        false,
    );
    harness.categories_impl.pairs.lock().expect("lock").push((
        mod_id.as_str().to_owned(),
        hmm_core::Category {
            id: "cat-1".to_owned(),
            name: "武器".to_owned(),
            color: None,
            sort_order: 0,
            created_at: 0,
        },
    ));

    let preview = harness
        .service
        .preview_mod_deletion(&mod_id)
        .expect("preview succeeds");

    assert_eq!(preview.display_name, "Fixture Mod");
    assert_eq!(preview.revision_count, 2);
    assert_eq!(preview.category_labels, vec!["武器".to_owned()]);
    assert_eq!(preview.affected_profiles, vec!["p2".to_owned()]);
}

#[test]
fn preview_fails_for_unknown_mod() {
    let harness = harness(&ModId::new("mod-a"), HashMap::new(), false);
    let error = harness
        .service
        .preview_mod_deletion(&ModId::new("missing"))
        .expect_err("unknown mod cannot be previewed");
    assert_eq!(error, ModDeletionError::ModNotFound);
}
