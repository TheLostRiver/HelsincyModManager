use super::*;
use hmm_core::{Category, ModId, ModMetadataOverlay, ModRevisionId, PreviewImageRejectionReason};
use hmm_ports::{
    CategoryRepository, ModImportPackagePrepareRequest, ModImportPackagePreparer,
    ModImportResultRepository, ModMetadataRepository, ModPackageMetadata,
    ModPackageMetadataAnalyzer, PreparedModPackage, PreviewImageProcessingResult,
    StoredImportPreviewImage, StoredLogicalMod, StoredModImportAnalysis, StoredModOriginProvenance,
    StoredModPackageMetadata, StoredModRevision, ThumbnailRef, ThumbnailStore,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[test]
fn ordinary_import_uses_new_logical_mod_catalog_contract() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("create import task");
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    let runner = runner(
        Arc::clone(&task_manager),
        Box::new(SuccessfulPreparer::new("package-v1")),
        Arc::clone(&repository),
    );

    runner
        .run_prepare_task(&task.task_id, archive_path())
        .expect("ordinary import succeeds");

    assert_eq!(repository.operations(), vec!["save_new_mod"]);
    let logical_mods = repository.list_mods().expect("list logical Mods");
    let revisions = repository
        .list_revisions(&ModId::new("package-v1"))
        .expect("list origin revision");
    assert_eq!(logical_mods.len(), 1);
    assert_eq!(logical_mods[0].mod_id, ModId::new("package-v1"));
    assert_eq!(
        logical_mods[0].origin_revision_id,
        ModRevisionId::new("package-v1")
    );
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].mod_id, ModId::new("package-v1"));
}

#[test]
fn revision_import_appends_to_explicit_existing_logical_mod() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("create revision import task");
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    repository.seed("mod-a", "revision-v1", "package-v1");
    let runner = runner(
        Arc::clone(&task_manager),
        Box::new(SuccessfulPreparer::new("package-v2")),
        Arc::clone(&repository),
    );

    runner
        .run_prepare_revision_task(&task.task_id, archive_path(), ModId::new("mod-a"))
        .expect("revision import succeeds");

    assert_eq!(repository.operations(), vec!["append_revision"]);
    assert_eq!(repository.list_mods().expect("list logical Mods").len(), 1);
    assert!(repository
        .get_mod(&ModId::new("package-v2"))
        .expect("query accidental Mod")
        .is_none());
    let logical_mod = repository
        .get_mod(&ModId::new("mod-a"))
        .expect("read target Mod")
        .expect("target Mod exists");
    let revisions = repository
        .list_revisions(&ModId::new("mod-a"))
        .expect("list target revisions");
    assert_eq!(
        logical_mod.origin_revision_id,
        ModRevisionId::new("revision-v1")
    );
    assert_eq!(
        logical_mod.display_revision_id,
        ModRevisionId::new("package-v2")
    );
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[1].revision_id, ModRevisionId::new("package-v2"));
    assert_eq!(revisions[1].mod_id, ModId::new("mod-a"));
    assert_eq!(revisions[1].package_id, "package-v2");
    assert_eq!(revisions[1].import_task_id, task.task_id);
    assert_eq!(revisions[1].display_name, "Candidate Revision");
}

#[test]
fn revision_import_without_candidate_name_preserves_current_display_name() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("create revision import task");
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    repository.seed("mod-a", "revision-v1", "package-v1");
    let runner = runner_with_metadata_analyzer(
        Arc::clone(&task_manager),
        Box::new(SuccessfulPreparer::new("mod-import-task-id")),
        Arc::clone(&repository),
        Box::new(MissingDisplayNameMetadataAnalyzer),
    );

    runner
        .run_prepare_revision_task(&task.task_id, archive_path(), ModId::new("mod-a"))
        .expect("revision import succeeds");

    let revisions = repository
        .list_revisions(&ModId::new("mod-a"))
        .expect("list target revisions");
    assert_eq!(revisions.len(), 2);
    assert_eq!(
        revisions[1].revision_id,
        ModRevisionId::new("mod-import-task-id")
    );
    assert_eq!(revisions[1].display_name, "Origin Revision");
    assert_eq!(revisions[1].metadata.version.as_deref(), Some("2.0"));
}

#[test]
fn revision_query_returns_explicit_origin_display_and_owned_revision_ids() {
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    repository.seed("mod-a", "revision-v1", "package-v1");
    repository
        .append_revision(&candidate_revision("revision-v2", "mod-a", "task-v2"))
        .expect("append candidate revision");
    let service = ModLibraryService::new(
        repository,
        Arc::new(SingleMetadataRepository),
        Arc::new(SingleCategoryRepository),
    );

    let revisions = service
        .get_mod_revisions(&ModId::new("mod-a"))
        .expect("query revisions")
        .expect("logical Mod exists");

    assert_eq!(revisions.mod_id, ModId::new("mod-a"));
    assert_eq!(
        revisions.origin_revision_id,
        ModRevisionId::new("revision-v1")
    );
    assert_eq!(
        revisions.display_revision_id,
        ModRevisionId::new("revision-v2")
    );
    assert_eq!(
        revisions.revision_ids,
        vec![
            ModRevisionId::new("revision-v1"),
            ModRevisionId::new("revision-v2")
        ]
    );
}

#[test]
fn revision_import_rejects_missing_mod_before_preparing_a_sandbox() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("create revision import task");
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    let prepare_calls = Arc::new(AtomicUsize::new(0));
    let runner = runner(
        Arc::clone(&task_manager),
        Box::new(CountingPreparer {
            calls: Arc::clone(&prepare_calls),
        }),
        Arc::clone(&repository),
    );

    let error = runner
        .run_prepare_revision_task(&task.task_id, archive_path(), ModId::new("missing-mod"))
        .expect_err("missing Mod is rejected");

    assert_eq!(prepare_calls.load(Ordering::SeqCst), 0);
    assert!(repository.operations().is_empty());
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(crate::TaskStatus::Failed)
    );
    assert_eq!(
        event_phases(&error.events),
        vec!["mod_import.unpack.failed"]
    );
    assert_eq!(error.cause(), None);
}

#[test]
fn revision_import_preserves_repository_lookup_error_without_preparing_a_sandbox() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("create revision import task");
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    repository.fail_get_mod_with("revision catalog unavailable");
    let prepare_calls = Arc::new(AtomicUsize::new(0));
    let runner = runner(
        Arc::clone(&task_manager),
        Box::new(CountingPreparer {
            calls: Arc::clone(&prepare_calls),
        }),
        Arc::clone(&repository),
    );

    let error = runner
        .run_prepare_revision_task(&task.task_id, archive_path(), ModId::new("mod-a"))
        .expect_err("repository lookup failure is preserved");

    assert_eq!(prepare_calls.load(Ordering::SeqCst), 0);
    assert!(repository.operations().is_empty());
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(crate::TaskStatus::Failed)
    );
    assert_eq!(error.cause(), Some("revision catalog unavailable"));
}

#[test]
fn cancelled_revision_import_does_not_append_a_revision() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("create revision import task");
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    repository.seed("mod-a", "revision-v1", "package-v1");
    let runner = runner(
        Arc::clone(&task_manager),
        Box::new(CancellingPreparer {
            task_manager: Arc::clone(&task_manager),
            task_id: task.task_id.clone(),
        }),
        Arc::clone(&repository),
    );

    let error = runner
        .run_prepare_revision_task(&task.task_id, archive_path(), ModId::new("mod-a"))
        .expect_err("cancelled revision import stops");

    assert!(error.events.is_empty());
    assert!(repository.operations().is_empty());
    assert_eq!(
        repository
            .list_revisions(&ModId::new("mod-a"))
            .expect("list revisions")
            .len(),
        1
    );
}

#[test]
fn failed_revision_prepare_does_not_append_a_revision() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("create revision import task");
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    repository.seed("mod-a", "revision-v1", "package-v1");
    let runner = runner(
        Arc::clone(&task_manager),
        Box::new(FailingPreparer),
        Arc::clone(&repository),
    );

    runner
        .run_prepare_revision_task(&task.task_id, archive_path(), ModId::new("mod-a"))
        .expect_err("prepare failure stops revision import");

    assert!(repository.operations().is_empty());
    assert_eq!(
        repository
            .list_revisions(&ModId::new("mod-a"))
            .expect("list revisions")
            .len(),
        1
    );
}

#[test]
fn metadata_and_category_overlays_remain_bound_to_logical_mod_after_append() {
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    repository.seed("mod-a", "revision-v1", "package-v1");
    repository
        .append_revision(&candidate_revision("package-v2", "mod-a", "task-v2"))
        .expect("append display revision");
    repository.clear_operations();
    let service = ModLibraryService::new(
        repository,
        Arc::new(SingleMetadataRepository),
        Arc::new(SingleCategoryRepository),
    );

    let library = service.get_mod_library().expect("load library");
    let detail = service
        .get_mod_detail("mod-a")
        .expect("load detail")
        .expect("detail exists");

    assert_eq!(library.len(), 1);
    assert_eq!(library[0].id, "mod-a");
    assert_eq!(library[0].name, "User Overlay Name");
    assert!(library[0]
        .category_labels
        .iter()
        .any(|label| label.name == "User Category"));
    assert_eq!(detail.id, "mod-a");
    assert_eq!(detail.package_id, "package-v2");
    assert_eq!(detail.name, "User Overlay Name");
}

fn runner(
    task_manager: Arc<crate::TaskManager>,
    preparer: Box<dyn ModImportPackagePreparer>,
    repository: Arc<FakeRevisionCatalogRepository>,
) -> ModImportTaskRunner {
    runner_with_metadata_analyzer(
        task_manager,
        preparer,
        repository,
        Box::new(FixedMetadataAnalyzer),
    )
}

fn runner_with_metadata_analyzer(
    task_manager: Arc<crate::TaskManager>,
    preparer: Box<dyn ModImportPackagePreparer>,
    repository: Arc<FakeRevisionCatalogRepository>,
    metadata_analyzer: Box<dyn ModPackageMetadataAnalyzer>,
) -> ModImportTaskRunner {
    ModImportTaskRunner::new(
        task_manager,
        Arc::new(ModImportPrepareService::new(
            preparer,
            ModImportAnalysisService::new(
                Box::new(FallbackPreviewProcessor),
                Box::new(NoopThumbnailStore),
                metadata_analyzer,
            ),
        )),
        repository,
    )
}

fn archive_path() -> PathBuf {
    Path::new("C:/mods/candidate.zip").to_path_buf()
}

fn event_phases(events: &[crate::TaskProgressEvent]) -> Vec<&str> {
    events.iter().map(|event| event.phase.as_str()).collect()
}

struct SuccessfulPreparer {
    package_id: String,
}

impl SuccessfulPreparer {
    fn new(package_id: &str) -> Self {
        Self {
            package_id: package_id.to_owned(),
        }
    }
}

impl ModImportPackagePreparer for SuccessfulPreparer {
    fn prepare_package(
        &self,
        _request: ModImportPackagePrepareRequest<'_>,
    ) -> anyhow::Result<PreparedModPackage> {
        Ok(PreparedModPackage {
            package_id: self.package_id.clone(),
            sandbox_root: PathBuf::from("sandbox"),
        })
    }
}

struct CountingPreparer {
    calls: Arc<AtomicUsize>,
}

impl ModImportPackagePreparer for CountingPreparer {
    fn prepare_package(
        &self,
        _request: ModImportPackagePrepareRequest<'_>,
    ) -> anyhow::Result<PreparedModPackage> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(PreparedModPackage {
            package_id: "unexpected-package".to_owned(),
            sandbox_root: PathBuf::from("unexpected-sandbox"),
        })
    }
}

struct CancellingPreparer {
    task_manager: Arc<crate::TaskManager>,
    task_id: String,
}

impl ModImportPackagePreparer for CancellingPreparer {
    fn prepare_package(
        &self,
        _request: ModImportPackagePrepareRequest<'_>,
    ) -> anyhow::Result<PreparedModPackage> {
        self.task_manager
            .cancel_task(&self.task_id)
            .expect("cancel running task");
        Ok(PreparedModPackage {
            package_id: "package-v2".to_owned(),
            sandbox_root: PathBuf::from("sandbox"),
        })
    }
}

struct FailingPreparer;

impl ModImportPackagePreparer for FailingPreparer {
    fn prepare_package(
        &self,
        _request: ModImportPackagePrepareRequest<'_>,
    ) -> anyhow::Result<PreparedModPackage> {
        anyhow::bail!("fixture prepare failure")
    }
}

struct FallbackPreviewProcessor;

impl ImportPreviewImageProcessor for FallbackPreviewProcessor {
    fn process_package_preview(
        &self,
        _task_id: &str,
        _package_id: &str,
        _sandbox_root: &Path,
    ) -> anyhow::Result<PreviewImageProcessingResult> {
        Ok(PreviewImageProcessingResult::Fallback(
            PreviewImageRejectionReason::Missing,
        ))
    }
}

struct NoopThumbnailStore;

impl ThumbnailStore for NoopThumbnailStore {
    fn put_thumbnail(
        &self,
        _package_id: &str,
        _content_hash: &str,
        _variant: &str,
        _extension: &str,
        _bytes: &[u8],
    ) -> anyhow::Result<ThumbnailRef> {
        unreachable!("fallback preview does not write a thumbnail")
    }

    fn resolve_url(&self, _thumbnail_ref: &ThumbnailRef) -> anyhow::Result<String> {
        unreachable!("fallback preview does not resolve a thumbnail")
    }
}

struct FixedMetadataAnalyzer;

impl ModPackageMetadataAnalyzer for FixedMetadataAnalyzer {
    fn analyze_metadata(
        &self,
        _package_id: &str,
        _sandbox_root: &Path,
    ) -> anyhow::Result<ModPackageMetadata> {
        Ok(ModPackageMetadata {
            display_name: Some("Candidate Revision".to_owned()),
            version: Some("2.0".to_owned()),
            category: Some("candidate-import".to_owned()),
            ..ModPackageMetadata::default()
        })
    }
}

struct MissingDisplayNameMetadataAnalyzer;

impl ModPackageMetadataAnalyzer for MissingDisplayNameMetadataAnalyzer {
    fn analyze_metadata(
        &self,
        _package_id: &str,
        _sandbox_root: &Path,
    ) -> anyhow::Result<ModPackageMetadata> {
        Ok(ModPackageMetadata {
            version: Some("2.0".to_owned()),
            ..ModPackageMetadata::default()
        })
    }
}

#[derive(Default)]
struct FakeRevisionCatalogRepository {
    mods: Mutex<Vec<StoredLogicalMod>>,
    revisions: Mutex<Vec<StoredModRevision>>,
    operations: Mutex<Vec<&'static str>>,
    get_mod_error: Mutex<Option<String>>,
}

impl FakeRevisionCatalogRepository {
    fn seed(&self, mod_id: &str, revision_id: &str, package_id: &str) {
        let revision_id = ModRevisionId::new(revision_id);
        self.mods.lock().expect("mods lock").push(StoredLogicalMod {
            mod_id: ModId::new(mod_id),
            origin_revision_id: revision_id.clone(),
            display_revision_id: revision_id.clone(),
            origin_provenance: StoredModOriginProvenance::Imported,
        });
        self.revisions
            .lock()
            .expect("revisions lock")
            .push(StoredModRevision {
                revision_id,
                mod_id: ModId::new(mod_id),
                import_task_id: "task-v1".to_owned(),
                package_id: package_id.to_owned(),
                display_name: "Origin Revision".to_owned(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: fallback_preview(),
            });
    }

    fn operations(&self) -> Vec<&'static str> {
        self.operations.lock().expect("operations lock").clone()
    }

    fn fail_get_mod_with(&self, message: &str) {
        *self.get_mod_error.lock().expect("get Mod error lock") = Some(message.to_owned());
    }

    fn clear_operations(&self) {
        self.operations.lock().expect("operations lock").clear();
    }

    fn projected_analysis(&self) -> Vec<StoredModImportAnalysis> {
        let mods = self.mods.lock().expect("mods lock").clone();
        let revisions = self.revisions.lock().expect("revisions lock").clone();
        mods.into_iter()
            .filter_map(|logical_mod| {
                revisions
                    .iter()
                    .find(|revision| revision.revision_id == logical_mod.display_revision_id)
                    .map(StoredModRevision::as_analysis)
            })
            .collect()
    }
}

impl ModImportResultRepository for FakeRevisionCatalogRepository {
    fn save_new_mod(
        &self,
        logical_mod: &StoredLogicalMod,
        revision: &StoredModRevision,
    ) -> anyhow::Result<()> {
        self.operations
            .lock()
            .expect("operations lock")
            .push("save_new_mod");
        self.mods
            .lock()
            .expect("mods lock")
            .push(logical_mod.clone());
        self.revisions
            .lock()
            .expect("revisions lock")
            .push(revision.clone());
        Ok(())
    }

    fn append_revision(&self, revision: &StoredModRevision) -> anyhow::Result<()> {
        self.operations
            .lock()
            .expect("operations lock")
            .push("append_revision");
        let mut mods = self.mods.lock().expect("mods lock");
        let logical_mod = mods
            .iter_mut()
            .find(|logical_mod| logical_mod.mod_id == revision.mod_id)
            .ok_or_else(|| anyhow::anyhow!("logical Mod not found"))?;
        if self
            .revisions
            .lock()
            .expect("revisions lock")
            .iter()
            .any(|stored| stored.revision_id == revision.revision_id)
        {
            anyhow::bail!("revision already exists");
        }
        logical_mod.display_revision_id = revision.revision_id.clone();
        self.revisions
            .lock()
            .expect("revisions lock")
            .push(revision.clone());
        Ok(())
    }

    fn get_mod(&self, mod_id: &ModId) -> anyhow::Result<Option<StoredLogicalMod>> {
        if let Some(message) = self
            .get_mod_error
            .lock()
            .expect("get Mod error lock")
            .clone()
        {
            anyhow::bail!(message);
        }
        Ok(self
            .mods
            .lock()
            .expect("mods lock")
            .iter()
            .find(|logical_mod| &logical_mod.mod_id == mod_id)
            .cloned())
    }

    fn list_mods(&self) -> anyhow::Result<Vec<StoredLogicalMod>> {
        Ok(self.mods.lock().expect("mods lock").clone())
    }

    fn get_revision(
        &self,
        revision_id: &ModRevisionId,
    ) -> anyhow::Result<Option<StoredModRevision>> {
        Ok(self
            .revisions
            .lock()
            .expect("revisions lock")
            .iter()
            .find(|revision| &revision.revision_id == revision_id)
            .cloned())
    }

    fn list_revisions(&self, mod_id: &ModId) -> anyhow::Result<Vec<StoredModRevision>> {
        Ok(self
            .revisions
            .lock()
            .expect("revisions lock")
            .iter()
            .filter(|revision| &revision.mod_id == mod_id)
            .cloned()
            .collect())
    }

    fn save_analysis(&self, analysis: &StoredModImportAnalysis) -> anyhow::Result<()> {
        self.operations
            .lock()
            .expect("operations lock")
            .push("save_analysis");
        let mod_id = ModId::new(&analysis.mod_id);
        let mut mods = self.mods.lock().expect("mods lock");
        if let Some(logical_mod) = mods.iter().find(|logical_mod| logical_mod.mod_id == mod_id) {
            if let Some(revision) = self
                .revisions
                .lock()
                .expect("revisions lock")
                .iter_mut()
                .find(|revision| revision.revision_id == logical_mod.display_revision_id)
            {
                revision.preview_image = analysis.preview_image.clone();
            }
            return Ok(());
        }

        let revision_id = ModRevisionId::new(&analysis.package_id);
        mods.push(StoredLogicalMod {
            mod_id: mod_id.clone(),
            origin_revision_id: revision_id.clone(),
            display_revision_id: revision_id.clone(),
            origin_provenance: StoredModOriginProvenance::Imported,
        });
        self.revisions
            .lock()
            .expect("revisions lock")
            .push(StoredModRevision {
                revision_id,
                mod_id,
                import_task_id: analysis.task_id.clone(),
                package_id: analysis.package_id.clone(),
                display_name: analysis.display_name.clone(),
                metadata: analysis.metadata.clone(),
                preview_image: analysis.preview_image.clone(),
            });
        Ok(())
    }

    fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
        Ok(self.projected_analysis())
    }

    fn get_analysis(&self, mod_id: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
        Ok(self
            .projected_analysis()
            .into_iter()
            .find(|analysis| analysis.mod_id == mod_id))
    }
}

struct SingleMetadataRepository;

impl ModMetadataRepository for SingleMetadataRepository {
    fn get(&self, mod_id: &str) -> anyhow::Result<Option<ModMetadataOverlay>> {
        Ok((mod_id == "mod-a").then(metadata_overlay))
    }

    fn save(&self, _overlay: &ModMetadataOverlay) -> anyhow::Result<()> {
        Ok(())
    }

    fn delete(&self, _mod_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn list_all(&self) -> anyhow::Result<Vec<ModMetadataOverlay>> {
        Ok(vec![metadata_overlay()])
    }
}

struct SingleCategoryRepository;

impl CategoryRepository for SingleCategoryRepository {
    fn get(&self, category_id: &str) -> anyhow::Result<Option<Category>> {
        Ok((category_id == "user-category").then(user_category))
    }

    fn save(&self, _category: &Category) -> anyhow::Result<()> {
        Ok(())
    }

    fn delete(&self, _category_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn list_all(&self) -> anyhow::Result<Vec<Category>> {
        Ok(vec![user_category()])
    }

    fn count_mods(&self, category_id: &str) -> anyhow::Result<u32> {
        Ok(u32::from(category_id == "user-category"))
    }

    fn get_mod_categories(&self, mod_id: &str) -> anyhow::Result<Vec<Category>> {
        Ok(if mod_id == "mod-a" {
            vec![user_category()]
        } else {
            Vec::new()
        })
    }

    fn set_mod_categories(&self, _mod_id: &str, _category_ids: &[String]) -> anyhow::Result<()> {
        Ok(())
    }

    fn list_mod_category_pairs(&self) -> anyhow::Result<Vec<(String, Category)>> {
        Ok(vec![("mod-a".to_owned(), user_category())])
    }
}

fn metadata_overlay() -> ModMetadataOverlay {
    ModMetadataOverlay {
        mod_id: ModId::new("mod-a"),
        display_name: Some("User Overlay Name".to_owned()),
        author: None,
        version: None,
        description: None,
        nexus_mod_id: None,
        updated_at: 1,
    }
}

fn user_category() -> Category {
    Category {
        id: "user-category".to_owned(),
        name: "User Category".to_owned(),
        color: Some("#123456".to_owned()),
        sort_order: 0,
        created_at: 1,
    }
}

fn candidate_revision(package_id: &str, mod_id: &str, task_id: &str) -> StoredModRevision {
    StoredModRevision {
        revision_id: ModRevisionId::new(package_id),
        mod_id: ModId::new(mod_id),
        import_task_id: task_id.to_owned(),
        package_id: package_id.to_owned(),
        display_name: "Candidate Revision".to_owned(),
        metadata: StoredModPackageMetadata {
            version: Some("2.0".to_owned()),
            category: Some("candidate-import".to_owned()),
            ..StoredModPackageMetadata::default()
        },
        preview_image: fallback_preview(),
    }
}

fn fallback_preview() -> StoredImportPreviewImage {
    StoredImportPreviewImage::Fallback {
        reason: PreviewImageRejectionReason::Missing,
    }
}
