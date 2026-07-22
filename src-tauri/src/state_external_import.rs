use hmm_app::{ExternalImportScanService, TaskManager};
use hmm_infra::{
    HuntingBoxDirectoryScanner, HuntingBoxDirectorySourceRegistry,
    SqliteExternalImportBatchRepository, SystemClock,
};
use hmm_ports::{
    ExternalImportBatchRepository, ExternalImportScanner, ExternalImportSourceRegistry,
};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub(crate) struct ExternalImportComposition {
    pub(crate) source_registry: Arc<HuntingBoxDirectorySourceRegistry>,
    pub(crate) scans: Arc<ExternalImportScanService>,
}

pub(crate) fn compose(
    app_data_dir: &Path,
    db: &Arc<Mutex<rusqlite::Connection>>,
    task_manager: &Arc<TaskManager>,
) -> Result<ExternalImportComposition, String> {
    let source_registry = Arc::new(
        HuntingBoxDirectorySourceRegistry::new(&app_data_dir.join("external-import"))
            .map_err(|_| "failed to initialize external import source registry".to_owned())?,
    );
    let source_registry_for_scans: Arc<dyn ExternalImportSourceRegistry> = source_registry.clone();
    let scanner: Arc<dyn ExternalImportScanner> = Arc::new(HuntingBoxDirectoryScanner::new(
        Arc::clone(&source_registry),
    ));
    let batch_repository: Arc<dyn ExternalImportBatchRepository> =
        Arc::new(SqliteExternalImportBatchRepository::new(Arc::clone(db)));
    let scans = Arc::new(ExternalImportScanService::new(
        Arc::clone(task_manager),
        source_registry_for_scans,
        scanner,
        batch_repository,
        Arc::new(SystemClock),
    ));

    Ok(ExternalImportComposition {
        source_registry,
        scans,
    })
}
