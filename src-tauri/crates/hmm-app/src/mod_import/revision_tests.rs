use super::*;
use hmm_core::{Category, ModId, ModMetadataOverlay, ModRevisionId, PreviewImageRejectionReason};
use hmm_ports::{
    CategoryRepository, ModImportPackagePrepareRequest, ModImportPackagePreparer,
    ModImportResultRepository, ModMetadataRepository, ModPackageMetadata,
    ModPackageMetadataAnalysis, ModPackageMetadataAnalyzer, PreparedModPackage,
    PreviewImageProcessingResult, StoredImportPreviewImage, StoredLogicalMod,
    StoredModImportAnalysis, StoredModOriginProvenance, StoredModPackageMetadata,
    StoredModRevision, ThumbnailRef, ThumbnailStore,
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
fn revision_import_without_candidate_name_ignores_the_archive_file_name() {
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
    // archive_path() 的文件名是 candidate，但给既有 logical Mod 追加 revision 时
    // 必须继承它当前的展示名。文件名只在新建 logical Mod 时充当候选，
    // 否则玩家换个压缩包文件名就会把库里的 Mod 改名。
    assert_eq!(revisions[1].display_name, "Origin Revision");
    assert_ne!(revisions[1].display_name, "candidate");
}

#[test]
fn new_logical_mod_import_without_metadata_uses_the_archive_file_name() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("create import task");
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    let runner = runner_with_metadata_analyzer(
        Arc::clone(&task_manager),
        Box::new(SuccessfulPreparer::new("mod-import-task-id")),
        Arc::clone(&repository),
        Box::new(MissingDisplayNameMetadataAnalyzer),
    );

    runner
        .run_prepare_task(&task.task_id, archive_path())
        .expect("import succeeds");

    let mods = repository.list_mods().expect("list logical mods");
    assert_eq!(mods.len(), 1);
    let revisions = repository
        .list_revisions(&mods[0].mod_id)
        .expect("list revisions");
    assert_eq!(revisions.len(), 1);
    // 无元数据的新 Mod 取压缩包文件名，而不是 mod-import-<时间戳> 这类内部标识。
    assert_eq!(revisions[0].display_name, "candidate");
}

#[test]
fn new_logical_mod_import_appends_an_ordinal_when_the_name_is_already_used() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("create import task");
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    // 库里已有一个同名 Mod：玩家先前导入过同一个 candidate.zip。
    repository.seed_named("mod-a", "revision-v1", "package-v1", "candidate");
    let runner = runner_with_metadata_analyzer(
        Arc::clone(&task_manager),
        Box::new(SuccessfulPreparer::new("mod-import-task-id")),
        Arc::clone(&repository),
        Box::new(MissingDisplayNameMetadataAnalyzer),
    );

    runner
        .run_prepare_task(&task.task_id, archive_path())
        .expect("import succeeds");

    let imported = repository
        .list_mods()
        .expect("list logical mods")
        .into_iter()
        .find(|logical_mod| logical_mod.mod_id != ModId::new("mod-a"))
        .expect("new logical Mod exists");
    let revisions = repository
        .list_revisions(&imported.mod_id)
        .expect("list revisions");
    // 重名不阻断导入，但要能在列表里分辨出来。
    assert_eq!(revisions[0].display_name, "candidate (2)");
}

#[test]
fn deduplication_only_counts_names_currently_on_display() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("create import task");
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    repository.seed_named("mod-a", "revision-v1", "package-v1", "candidate");
    // 追加一条改名后的 revision 并让它成为展示版本：旧名字"candidate"退出界面。
    let mut renamed = candidate_revision("revision-v2", "mod-a", "task-v2");
    renamed.display_name = "改名后的 Mod".to_owned();
    repository
        .append_revision(&renamed)
        .expect("append renamed revision");
    let runner = runner_with_metadata_analyzer(
        Arc::clone(&task_manager),
        Box::new(SuccessfulPreparer::new("mod-import-task-id")),
        Arc::clone(&repository),
        Box::new(MissingDisplayNameMetadataAnalyzer),
    );

    runner
        .run_prepare_task(&task.task_id, archive_path())
        .expect("import succeeds");

    let imported = repository
        .list_mods()
        .expect("list logical mods")
        .into_iter()
        .find(|logical_mod| logical_mod.mod_id != ModId::new("mod-a"))
        .expect("new logical Mod exists");
    let revisions = repository
        .list_revisions(&imported.mod_id)
        .expect("list revisions");
    // 历史 revision 的旧名字不出现在界面上，不该凭空产生序号。
    assert_eq!(revisions[0].display_name, "candidate");
}

#[test]
fn deduplication_sees_collisions_created_by_a_user_rename() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("create import task");
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    repository.seed_named("mod-a", "revision-v1", "package-v1", "别的名字");
    // 用户手动把它改名成了 candidate：catalog 名字没变，界面上却已经叫这个了。
    let runner = runner_with_renames(
        Arc::clone(&task_manager),
        Arc::clone(&repository),
        &[("mod-a", "candidate")],
    );

    runner
        .run_prepare_task(&task.task_id, archive_path())
        .expect("import succeeds");

    assert_eq!(
        imported_display_name(&repository, &["mod-a"]),
        "candidate (2)"
    );
}

#[test]
fn deduplication_does_not_invent_an_ordinal_for_a_name_the_user_renamed_away() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("create import task");
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    repository.seed_named("mod-a", "revision-v1", "package-v1", "candidate");
    // 用户已经把它改名走了，界面上不存在 candidate 这个名字。
    let runner = runner_with_renames(
        Arc::clone(&task_manager),
        Arc::clone(&repository),
        &[("mod-a", "candidate-旧版")],
    );

    runner
        .run_prepare_task(&task.task_id, archive_path())
        .expect("import succeeds");

    // 加序号会凭空造出一个"(2)"，而界面上根本看不到与之对应的"(1)"。
    assert_eq!(imported_display_name(&repository, &["mod-a"]), "candidate");
}

#[test]
fn revision_import_keeps_the_inherited_name_even_when_another_mod_shares_it() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("create revision import task");
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    repository.seed_named("mod-a", "revision-v1", "package-v1", "共享名字");
    repository.seed_named("mod-b", "revision-b1", "package-b1", "共享名字");
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
    // 去重只作用于新建 logical Mod。给既有 Mod 更新版本时追加序号，
    // 会让同一个 Mod 每更新一版就改一次名。
    assert_eq!(revisions[1].display_name, "共享名字");
}

#[test]
fn import_keeps_the_original_name_when_the_catalog_cannot_be_read() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("create import task");
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    repository.fail_list_mods_with("catalog unavailable");
    let runner = runner_with_metadata_analyzer(
        Arc::clone(&task_manager),
        Box::new(SuccessfulPreparer::new("mod-import-task-id")),
        Arc::clone(&repository),
        Box::new(MissingDisplayNameMetadataAnalyzer),
    );

    // 展示名不参与身份判定，读不到 catalog 不该让一次已解包完成的导入失败。
    runner
        .run_prepare_task(&task.task_id, archive_path())
        .expect("import still succeeds without the catalog");

    let mods = repository.list_mods_unguarded();
    assert_eq!(mods.len(), 1);
    let revisions = repository
        .list_revisions(&mods[0].mod_id)
        .expect("list revisions");
    assert_eq!(revisions[0].display_name, "candidate");
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

#[test]
fn mod_detail_reports_external_import_origin_without_private_digests() {
    let repository = Arc::new(FakeRevisionCatalogRepository::default());
    repository.seed("mod-a", "revision-v1", "package-v1");
    repository.set_origin_provenance(
        "mod-a",
        StoredModOriginProvenance::ExternalImport {
            provenance: hmm_core::ExternalImportProvenance {
                adapter_id: hmm_core::ExternalImportAdapterId::new("hunting_box_directory_v1"),
                batch_id: hmm_core::ExternalImportBatchId::new("external-import-batch-1"),
                source_item_key_hash: "private-item-key".to_owned(),
                content_fingerprint: "sha256:private-content".to_owned(),
                imported_at_unix_millis: 1_724_000_000_000,
            },
        },
    );
    let service = ModLibraryService::new(
        repository,
        Arc::new(SingleMetadataRepository),
        Arc::new(SingleCategoryRepository),
    );

    let detail = service
        .get_mod_detail("mod-a")
        .expect("load detail")
        .expect("detail exists");

    // 展示摘要只携带稳定 ID 与导入时间;私有摘要连类型上都不允许存在。
    assert_eq!(
        detail.origin,
        crate::ModOriginSummary::ExternalImport {
            adapter_id: "hunting_box_directory_v1".to_owned(),
            batch_id: "external-import-batch-1".to_owned(),
            imported_at_unix_millis: 1_724_000_000_000,
        }
    );
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

fn runner_with_renames(
    task_manager: Arc<crate::TaskManager>,
    repository: Arc<FakeRevisionCatalogRepository>,
    renames: &[(&str, &str)],
) -> ModImportTaskRunner {
    runner_with_metadata_analyzer(
        task_manager,
        Box::new(SuccessfulPreparer::new("mod-import-task-id")),
        repository,
        Box::new(MissingDisplayNameMetadataAnalyzer),
    )
    .with_metadata_repository(Arc::new(RenameOverlayRepository::new(renames)))
}

fn imported_display_name(
    repository: &FakeRevisionCatalogRepository,
    seeded_mod_ids: &[&str],
) -> String {
    let imported = repository
        .list_mods()
        .expect("list logical mods")
        .into_iter()
        .find(|logical_mod| {
            !seeded_mod_ids
                .iter()
                .any(|seeded| logical_mod.mod_id == ModId::new(*seeded))
        })
        .expect("new logical Mod exists");
    repository
        .list_revisions(&imported.mod_id)
        .expect("list revisions")
        .swap_remove(0)
        .display_name
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
    ) -> anyhow::Result<ModPackageMetadataAnalysis> {
        Ok(ModPackageMetadataAnalysis {
            metadata: ModPackageMetadata {
                display_name: Some("Candidate Revision".to_owned()),
                version: Some("2.0".to_owned()),
                category: Some("candidate-import".to_owned()),
                ..ModPackageMetadata::default()
            },
            manifest_display_name: None,
        })
    }
}

struct MissingDisplayNameMetadataAnalyzer;

impl ModPackageMetadataAnalyzer for MissingDisplayNameMetadataAnalyzer {
    fn analyze_metadata(
        &self,
        _package_id: &str,
        _sandbox_root: &Path,
    ) -> anyhow::Result<ModPackageMetadataAnalysis> {
        Ok(ModPackageMetadataAnalysis {
            metadata: ModPackageMetadata {
                version: Some("2.0".to_owned()),
                ..ModPackageMetadata::default()
            },
            manifest_display_name: None,
        })
    }
}

#[derive(Default)]
struct FakeRevisionCatalogRepository {
    mods: Mutex<Vec<StoredLogicalMod>>,
    revisions: Mutex<Vec<StoredModRevision>>,
    operations: Mutex<Vec<&'static str>>,
    get_mod_error: Mutex<Option<String>>,
    list_mods_error: Mutex<Option<String>>,
}

impl FakeRevisionCatalogRepository {
    fn seed(&self, mod_id: &str, revision_id: &str, package_id: &str) {
        self.seed_named(mod_id, revision_id, package_id, "Origin Revision");
    }

    fn seed_named(&self, mod_id: &str, revision_id: &str, package_id: &str, display_name: &str) {
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
                display_name: display_name.to_owned(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: fallback_preview(),
            });
    }

    fn set_origin_provenance(&self, mod_id: &str, provenance: StoredModOriginProvenance) {
        let mut mods = self.mods.lock().expect("mods lock");
        let logical_mod = mods
            .iter_mut()
            .find(|logical_mod| logical_mod.mod_id.as_str() == mod_id)
            .expect("seeded logical mod exists");
        logical_mod.origin_provenance = provenance;
    }

    fn fail_list_mods_with(&self, message: &str) {
        *self.list_mods_error.lock().expect("list Mods error lock") = Some(message.to_owned());
    }

    /// 绕过 `fail_list_mods_with` 注入的故障读取内部状态，供断言检查故障期间写入的结果。
    fn list_mods_unguarded(&self) -> Vec<StoredLogicalMod> {
        self.mods.lock().expect("mods lock").clone()
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
        if let Some(message) = self
            .list_mods_error
            .lock()
            .expect("list Mods error lock")
            .clone()
        {
            anyhow::bail!(message);
        }
        Ok(self.mods.lock().expect("mods lock").clone())
    }

    /// 必须覆写：默认实现只按 `display_revision_id` 逐条取，产出的 `revisions`
    /// 恰好只有展示中的那些，而生产实现（JSON catalog）直接返回全部 revision。
    /// 沿用默认实现会让"只统计展示中的名字"这条断言在测试里恒真。
    fn catalog_snapshot(&self) -> anyhow::Result<hmm_ports::ModImportCatalogSnapshot> {
        Ok(hmm_ports::ModImportCatalogSnapshot {
            logical_mods: self.list_mods()?,
            revisions: self.revisions.lock().expect("revisions lock").clone(),
        })
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

/// 只提供改名 overlay 的仓储，用于验证去重按用户实际看到的名字比较。
struct RenameOverlayRepository {
    renames: Vec<(String, String)>,
}

impl RenameOverlayRepository {
    fn new(renames: &[(&str, &str)]) -> Self {
        Self {
            renames: renames
                .iter()
                .map(|(mod_id, display_name)| ((*mod_id).to_owned(), (*display_name).to_owned()))
                .collect(),
        }
    }

    fn overlay(mod_id: &str, display_name: &str) -> ModMetadataOverlay {
        ModMetadataOverlay {
            mod_id: ModId::new(mod_id),
            display_name: Some(display_name.to_owned()),
            author: None,
            version: None,
            description: None,
            nexus_mod_id: None,
            updated_at: 0,
        }
    }
}

impl ModMetadataRepository for RenameOverlayRepository {
    fn get(&self, mod_id: &str) -> anyhow::Result<Option<ModMetadataOverlay>> {
        Ok(self
            .renames
            .iter()
            .find(|(id, _)| id == mod_id)
            .map(|(id, display_name)| Self::overlay(id, display_name)))
    }

    fn save(&self, _overlay: &ModMetadataOverlay) -> anyhow::Result<()> {
        Ok(())
    }

    fn delete(&self, _mod_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn list_all(&self) -> anyhow::Result<Vec<ModMetadataOverlay>> {
        Ok(self
            .renames
            .iter()
            .map(|(id, display_name)| Self::overlay(id, display_name))
            .collect())
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
