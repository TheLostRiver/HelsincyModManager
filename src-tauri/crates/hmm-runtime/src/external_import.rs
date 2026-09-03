use hmm_app::{
    ExternalImportBatchService, ExternalImportScanService, ModImportPrepareService, TaskManager,
};
use hmm_infra::{
    HuntingBoxDirectoryMaterializer, HuntingBoxDirectoryScanner, HuntingBoxDirectorySourceRegistry,
    SqliteExternalImportBatchRepository, SystemClock, ZipModImportPackagePreparer,
};
use hmm_ports::{
    CategoryRepository, ExternalImportBatchRepository, ExternalImportMaterializer,
    ExternalImportScanner, ExternalImportSourceRegistry, ModImportPackagePreparer,
    ModImportResultRepository, ModImportSandboxLocator,
};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct ExternalImportComposition {
    pub source_registry: Arc<HuntingBoxDirectorySourceRegistry>,
    pub scans: Arc<ExternalImportScanService>,
    pub batches: Arc<ExternalImportBatchService>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compose(
    app_data_dir: &Path,
    mod_storage_root: &Path,
    db: &Arc<Mutex<rusqlite::Connection>>,
    task_manager: &Arc<TaskManager>,
    catalog: Arc<dyn ModImportResultRepository>,
    category_repository: Arc<dyn CategoryRepository>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    prepare_service: Arc<ModImportPrepareService>,
) -> Result<ExternalImportComposition, String> {
    let source_registry = Arc::new(
        HuntingBoxDirectorySourceRegistry::new(&app_data_dir.join("external-import"))
            .map_err(|_| "failed to initialize external import source registry".to_owned())?,
    );
    let source_registry_for_scans: Arc<dyn ExternalImportSourceRegistry> = source_registry.clone();
    let source_registry_for_batches: Arc<dyn ExternalImportSourceRegistry> =
        source_registry.clone();
    let scanner: Arc<dyn ExternalImportScanner> = Arc::new(HuntingBoxDirectoryScanner::new(
        Arc::clone(&source_registry),
    ));
    let batch_repository: Arc<dyn ExternalImportBatchRepository> =
        Arc::new(SqliteExternalImportBatchRepository::new(Arc::clone(db)));
    let scans = Arc::new(ExternalImportScanService::new(
        Arc::clone(task_manager),
        source_registry_for_scans,
        scanner,
        Arc::clone(&batch_repository),
        Arc::new(SystemClock),
    ));
    // Materialized archives stay app-private (`external-import/materialized`); the unpacked
    // packages land in the Mod storage root like every other import (#275).
    let package_preparer: Arc<dyn ModImportPackagePreparer> = Arc::new(
        ZipModImportPackagePreparer::new_in_storage_root(mod_storage_root.to_path_buf()),
    );
    let materializer: Arc<dyn ExternalImportMaterializer> =
        Arc::new(HuntingBoxDirectoryMaterializer::new(
            Arc::clone(&source_registry),
            app_data_dir.to_path_buf(),
            package_preparer,
        ));
    let batches = Arc::new(ExternalImportBatchService::new(
        Arc::clone(task_manager),
        source_registry_for_batches,
        materializer,
        batch_repository,
        catalog,
        category_repository,
        sandbox_locator,
        prepare_service,
        Arc::new(SystemClock),
    ));
    batches
        .recover_interrupted_batches()
        .map_err(|_| "failed to recover interrupted external import batches".to_owned())?;
    // 保留期清理尽力而为:失败不得阻断启动。恢复(改变批次真实状态)才是必须成功的。
    let _ = batches.prune_batch_history();

    Ok(ExternalImportComposition {
        source_registry,
        scans,
        batches,
    })
}
