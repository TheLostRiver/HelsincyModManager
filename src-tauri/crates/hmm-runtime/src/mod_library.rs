use hmm_app::{
    InstallManifestQueryService, ModLibraryProjectionFreshnessGuard,
    ModLibraryProjectionRefreshService, ModLibraryQueryService, ModLibraryService,
    ProjectionTrackingCategoryRepository, ProjectionTrackingInstallManifestRepository,
    ProjectionTrackingModImportResultRepository, ProjectionTrackingModMetadataRepository,
};
use hmm_infra::{
    JsonModImportResultRepository, SqliteCategoryRepository, SqliteModLibraryProjectionRepository,
    SqliteModMetadataRepository,
};
use hmm_ports::{
    CategoryRepository, InstallManifestRepository, ModImportResultRepository,
    ModLibraryProjectionQueryRepository, ModLibraryProjectionRepository, ModMetadataRepository,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, Weak};

// Batch commands compose short-lived runtimes alongside the GUI. They must use the same
// refresh/write guard, otherwise a GUI rebuild can erase a batch writer's dirty marker.
fn shared_freshness_guard(
    db: &Arc<Mutex<rusqlite::Connection>>,
) -> Result<Arc<ModLibraryProjectionFreshnessGuard>, String> {
    static GUARDS: OnceLock<Mutex<HashMap<PathBuf, Weak<ModLibraryProjectionFreshnessGuard>>>> =
        OnceLock::new();
    let database_path = db
        .lock()
        .map_err(|_| "Mod library database lock is unavailable")?
        .path()
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let Some(database_path) = database_path else {
        return Ok(Arc::new(ModLibraryProjectionFreshnessGuard::default()));
    };
    let key = database_path
        .canonicalize()
        .map_err(|_| "Mod library database identity is unavailable")?;
    #[cfg(windows)]
    let key = PathBuf::from(key.to_string_lossy().to_lowercase());
    let mut guards = GUARDS
        .get_or_init(Mutex::default)
        .lock()
        .map_err(|_| "Mod library projection coordination is unavailable")?;
    guards.retain(|_, guard| guard.strong_count() > 0);
    if let Some(guard) = guards.get(&key).and_then(Weak::upgrade) {
        return Ok(guard);
    }
    let guard = Arc::new(ModLibraryProjectionFreshnessGuard::default());
    guards.insert(key, Arc::downgrade(&guard));
    Ok(guard)
}

pub(super) struct ModLibraryComposition {
    mod_import_result_repository: Arc<dyn ModImportResultRepository>,
    mod_metadata_repository: Arc<dyn ModMetadataRepository>,
    category_repository: Arc<dyn CategoryRepository>,
    projection_writer: Arc<dyn ModLibraryProjectionRepository>,
    projection_query_repository: Arc<dyn ModLibraryProjectionQueryRepository>,
    freshness_guard: Arc<ModLibraryProjectionFreshnessGuard>,
}

impl ModLibraryComposition {
    pub(crate) fn new(
        db: &Arc<Mutex<rusqlite::Connection>>,
        mod_import_results_path: PathBuf,
    ) -> Result<Self, String> {
        let projection_repository =
            Arc::new(SqliteModLibraryProjectionRepository::new(Arc::clone(db)));
        let projection_writer: Arc<dyn ModLibraryProjectionRepository> =
            projection_repository.clone();
        projection_writer
            .mark_dirty(None)
            .map_err(|error| format!("failed to invalidate Mod library projection: {error}"))?;
        let projection_query_repository: Arc<dyn ModLibraryProjectionQueryRepository> =
            projection_repository;
        let freshness_guard = shared_freshness_guard(db)?;

        let mod_import_result_repository: Arc<dyn ModImportResultRepository> =
            Arc::new(ProjectionTrackingModImportResultRepository::new(
                Arc::new(JsonModImportResultRepository::new(mod_import_results_path)),
                Arc::clone(&projection_writer),
                Arc::clone(&freshness_guard),
            ));
        let mod_metadata_repository: Arc<dyn ModMetadataRepository> =
            Arc::new(ProjectionTrackingModMetadataRepository::new(
                Arc::new(SqliteModMetadataRepository::new(Arc::clone(db))),
                Arc::clone(&projection_writer),
                Arc::clone(&freshness_guard),
            ));
        let category_repository: Arc<dyn CategoryRepository> =
            Arc::new(ProjectionTrackingCategoryRepository::new(
                Arc::new(SqliteCategoryRepository::new(Arc::clone(db))),
                Arc::clone(&projection_writer),
                Arc::clone(&freshness_guard),
            ));

        Ok(Self {
            mod_import_result_repository,
            mod_metadata_repository,
            category_repository,
            projection_writer,
            projection_query_repository,
            freshness_guard,
        })
    }

    pub(crate) fn mod_import_result_repository(&self) -> Arc<dyn ModImportResultRepository> {
        Arc::clone(&self.mod_import_result_repository)
    }

    pub(crate) fn mod_metadata_repository(&self) -> Arc<dyn ModMetadataRepository> {
        Arc::clone(&self.mod_metadata_repository)
    }

    pub(crate) fn category_repository(&self) -> Arc<dyn CategoryRepository> {
        Arc::clone(&self.category_repository)
    }

    pub(crate) fn install_manifest_repository(
        &self,
        delegate: Arc<dyn InstallManifestRepository>,
    ) -> Arc<dyn InstallManifestRepository> {
        Arc::new(ProjectionTrackingInstallManifestRepository::new(
            delegate,
            Arc::clone(&self.projection_writer),
            Arc::clone(&self.freshness_guard),
        ))
    }

    pub(crate) fn library_service(&self) -> Arc<ModLibraryService> {
        Arc::new(ModLibraryService::new(
            self.mod_import_result_repository(),
            self.mod_metadata_repository(),
            self.category_repository(),
        ))
    }

    pub(crate) fn query_service(
        &self,
        library_service: Arc<ModLibraryService>,
        status_provider: Arc<InstallManifestQueryService>,
    ) -> Arc<ModLibraryQueryService> {
        let refresh_service = Arc::new(ModLibraryProjectionRefreshService::new(
            library_service,
            status_provider,
            Arc::clone(&self.projection_writer),
            Arc::clone(&self.freshness_guard),
        ));
        Arc::new(ModLibraryQueryService::new_projection(
            Arc::clone(&self.projection_query_repository),
            refresh_service,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtimes_on_the_same_database_share_projection_refresh_and_write_coordination() {
        let temp = tempfile::tempdir().expect("temporary projection databases");
        let path = temp.path().join("hmm.db");
        let first_db = Arc::new(Mutex::new(hmm_infra::open_database(&path).unwrap()));
        let second_db = Arc::new(Mutex::new(
            hmm_infra::open_database(&temp.path().join(".").join("hmm.db")).unwrap(),
        ));
        let other_db = Arc::new(Mutex::new(
            hmm_infra::open_database(&temp.path().join("other.db")).unwrap(),
        ));
        let results = temp.path().join("results.json");
        let first = ModLibraryComposition::new(&first_db, results.clone()).unwrap();
        let second = ModLibraryComposition::new(&second_db, results.clone()).unwrap();
        let other = ModLibraryComposition::new(&other_db, results).unwrap();
        assert!(
            Arc::ptr_eq(&first.freshness_guard, &second.freshness_guard),
            "batch and GUI runtimes must not rebuild over each other's dirty markers"
        );
        assert!(
            !Arc::ptr_eq(&first.freshness_guard, &other.freshness_guard),
            "an unavailable projection in another data root must stay isolated"
        );
    }
}
