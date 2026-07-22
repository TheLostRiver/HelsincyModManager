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
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(crate) struct ModLibraryComposition {
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
        let freshness_guard = Arc::new(ModLibraryProjectionFreshnessGuard::default());

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
