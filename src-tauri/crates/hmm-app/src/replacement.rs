use hmm_core::{
    FileLayer, GameId, InstallFileProvider, InstallPlan, InstallPlanValidationError, ModId,
    ModRevisionId, PackageFileId, ProfileId, ReplacementAnalysis, ReplacementBinding,
    ReplacementBindingId, ReplacementBindingSnapshot, ReplacementTarget, ReplacementTargetId,
    RetargetPlan,
};
use hmm_ports::{
    AppClock, ModImportResultRepository, ModImportSandboxLocator, ModPackageInstallFileScanRequest,
    ModPackageInstallFileScanner, ReplacementAdapter, ReplacementAdapterError,
    ReplacementAnalysisRequest, ReplacementAsset, ReplacementCatalogError,
    ReplacementCatalogProvider, RetargetPlanRequest, RetargetStagingError, RetargetStagingFile,
    RetargetStagingMaterializer,
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use crate::InstallRecoveryStatus;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplacementServiceError {
    #[error("replacement is unsupported for the requested game")]
    UnsupportedGame,
    #[error("replacement adapter failed")]
    Adapter(#[from] ReplacementAdapterError),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RetargetMaterializeError {
    #[error("retarget install plan is invalid")]
    InvalidInstallPlan(#[from] InstallPlanValidationError),
    #[error("retarget staging failed")]
    Staging(#[from] RetargetStagingError),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InitialRetargetInstallStatusError {
    #[error("initial retarget install status is unavailable")]
    Unavailable,
}

pub trait InitialRetargetInstallStatusReader: Send + Sync {
    fn recovery_status(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Result<InstallRecoveryStatus, InitialRetargetInstallStatusError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplacementWorkflowError {
    #[error("replacement is unsupported for the requested game")]
    UnsupportedGame,
    #[error("replacement target catalog is unavailable")]
    CatalogUnavailable,
    #[error("replacement target was not found")]
    TargetNotFound,
    #[error("imported Mod repository is unavailable")]
    ModRepositoryUnavailable,
    #[error("imported Mod was not found")]
    ModNotFound,
    #[error("imported Mod revision was not found")]
    RevisionNotFound,
    #[error("imported Mod sandbox is unavailable")]
    SandboxUnavailable,
    #[error("imported Mod files are unavailable")]
    PackageFilesUnavailable,
    #[error("replacement analysis failed")]
    Analysis(ReplacementServiceError),
    #[error("replacement source is not retargetable")]
    SourceNotRetargetable,
    #[error("initial retarget install status is unavailable")]
    InstallStatusUnavailable,
    #[error("initial retarget install is blocked by status {status:?}")]
    InitialInstallBlocked { status: InstallRecoveryStatus },
    #[error("replacement binding could not be created")]
    BindingUnavailable,
    #[error("installed replacement binding is unavailable")]
    InstalledBindingUnavailable,
    #[error("replacement target is already selected")]
    TargetAlreadySelected,
    #[error("retarget install plan is unavailable")]
    PlanUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzeImportedReplacementRequest {
    pub game_id: GameId,
    pub mod_id: ModId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewInitialRetargetInstallRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub target_id: ReplacementTargetId,
    pub layer: FileLayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewRetargetReinstallRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub installed_revision_id: ModRevisionId,
    pub installed_binding: ReplacementBindingSnapshot,
    pub target_id: ReplacementTargetId,
    pub layer: FileLayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetargetReinstallRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub target_id: ReplacementTargetId,
    pub layer: FileLayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedInitialRetargetInstall {
    package_id: String,
    revision_id: ModRevisionId,
    layer: FileLayer,
    analysis: ReplacementAnalysis,
    target: ReplacementTarget,
    retarget_plan: RetargetPlan,
    install_plan: InstallPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedRetargetReinstall {
    package_id: String,
    revision_id: ModRevisionId,
    layer: FileLayer,
    analysis: ReplacementAnalysis,
    target: ReplacementTarget,
    retarget_plan: RetargetPlan,
    install_plan: InstallPlan,
}

impl PlannedRetargetReinstall {
    pub fn package_id(&self) -> &str {
        &self.package_id
    }

    pub fn revision_id(&self) -> &ModRevisionId {
        &self.revision_id
    }

    pub fn install_plan(&self) -> &InstallPlan {
        &self.install_plan
    }

    pub fn binding_id(&self) -> &ReplacementBindingId {
        self.retarget_plan.binding().id()
    }
}

impl PlannedInitialRetargetInstall {
    pub fn package_id(&self) -> &str {
        &self.package_id
    }

    pub fn revision_id(&self) -> &ModRevisionId {
        &self.revision_id
    }

    pub fn analysis(&self) -> &ReplacementAnalysis {
        &self.analysis
    }

    pub fn target(&self) -> &ReplacementTarget {
        &self.target
    }

    pub fn retarget_plan(&self) -> &RetargetPlan {
        &self.retarget_plan
    }

    pub fn install_plan(&self) -> &InstallPlan {
        &self.install_plan
    }

    pub fn binding_id(&self) -> &ReplacementBindingId {
        self.retarget_plan.binding().id()
    }
}

struct ResolvedImportedReplacement {
    package_id: String,
    revision_id: ModRevisionId,
    assets: Vec<ReplacementAsset>,
    analysis: ReplacementAnalysis,
}

pub struct ReplacementWorkflowService {
    replacement: ReplacementService,
    catalogs: Vec<Arc<dyn ReplacementCatalogProvider>>,
    result_repository: Arc<dyn ModImportResultRepository>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    file_scanner: Arc<dyn ModPackageInstallFileScanner>,
    install_status: Arc<dyn InitialRetargetInstallStatusReader>,
    clock: Arc<dyn AppClock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializeRetargetRequest {
    pub plan: RetargetPlan,
    pub layer: FileLayer,
    pub revision_id: Option<ModRevisionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedRetarget {
    retarget_plan: RetargetPlan,
    install_plan: InstallPlan,
}

impl MaterializedRetarget {
    pub fn retarget_plan(&self) -> &RetargetPlan {
        &self.retarget_plan
    }

    pub fn install_plan(&self) -> &InstallPlan {
        &self.install_plan
    }

    pub fn into_parts(self) -> (RetargetPlan, InstallPlan) {
        (self.retarget_plan, self.install_plan)
    }
}

pub struct ReplacementService {
    adapters: Vec<Arc<dyn ReplacementAdapter>>,
}

impl ReplacementService {
    pub fn new(adapters: Vec<Arc<dyn ReplacementAdapter>>) -> Self {
        Self { adapters }
    }

    pub fn analyze(
        &self,
        request: ReplacementAnalysisRequest,
    ) -> Result<ReplacementAnalysis, ReplacementServiceError> {
        let adapter = self.adapter_for(&request.game_id)?;
        adapter
            .analyze_replacement_assets(request)
            .map_err(Into::into)
    }

    pub fn build_retarget_plan(
        &self,
        request: RetargetPlanRequest,
    ) -> Result<RetargetPlan, ReplacementServiceError> {
        let adapter = self.adapter_for(&request.game_id)?;
        adapter.build_retarget_plan(request).map_err(Into::into)
    }

    pub fn materialize_retarget(
        &self,
        staging: &dyn RetargetStagingMaterializer,
        request: MaterializeRetargetRequest,
    ) -> Result<MaterializedRetarget, RetargetMaterializeError> {
        let staging_files = request
            .plan
            .actions()
            .iter()
            .map(|action| {
                RetargetStagingFile::new(
                    action.package_file_id().clone(),
                    action.target_relative_path().clone(),
                )
            })
            .collect::<Vec<_>>();
        let install_plan =
            self.build_retarget_install_plan(&request.plan, request.layer, request.revision_id)?;

        staging.materialize(&staging_files)?;

        Ok(MaterializedRetarget {
            retarget_plan: request.plan,
            install_plan,
        })
    }

    pub fn build_retarget_install_plan(
        &self,
        plan: &RetargetPlan,
        layer: FileLayer,
        revision_id: Option<ModRevisionId>,
    ) -> Result<InstallPlan, RetargetMaterializeError> {
        let snapshot = ReplacementBindingSnapshot::from_retarget_plan(plan, revision_id);
        let providers = plan.actions().iter().map(|action| {
            InstallFileProvider::new(
                plan.binding().mod_id().clone(),
                action.package_file_id().clone(),
                action.target_relative_path().clone(),
                layer.clone(),
            )
        });
        Ok(InstallPlan::from_providers(providers).with_replacement_bindings(vec![snapshot])?)
    }

    fn adapter_for(
        &self,
        game_id: &hmm_core::GameId,
    ) -> Result<Arc<dyn ReplacementAdapter>, ReplacementServiceError> {
        self.adapters
            .iter()
            .find(|adapter| adapter.game_id() == *game_id)
            .cloned()
            .ok_or(ReplacementServiceError::UnsupportedGame)
    }
}

impl ReplacementWorkflowService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        replacement_adapters: Vec<Arc<dyn ReplacementAdapter>>,
        catalogs: Vec<Arc<dyn ReplacementCatalogProvider>>,
        result_repository: Arc<dyn ModImportResultRepository>,
        sandbox_locator: Arc<dyn ModImportSandboxLocator>,
        file_scanner: Arc<dyn ModPackageInstallFileScanner>,
        install_status: Arc<dyn InitialRetargetInstallStatusReader>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            replacement: ReplacementService::new(replacement_adapters),
            catalogs,
            result_repository,
            sandbox_locator,
            file_scanner,
            install_status,
            clock,
        }
    }

    pub fn list_targets(
        &self,
        game_id: &GameId,
        query: Option<&str>,
    ) -> Result<Vec<ReplacementTarget>, ReplacementWorkflowError> {
        let catalog = self.catalog_for(game_id)?;
        let query = query.map(str::trim).filter(|query| !query.is_empty());
        match query {
            Some(query) => catalog
                .search_replacement_targets(query)
                .map_err(map_catalog_error),
            None => catalog
                .replacement_catalog()
                .map(|catalog| catalog.targets().to_vec())
                .map_err(map_catalog_error),
        }
    }

    pub fn analyze_imported_mod(
        &self,
        request: AnalyzeImportedReplacementRequest,
    ) -> Result<ReplacementAnalysis, ReplacementWorkflowError> {
        self.resolve_imported_replacement(&request.game_id, &request.mod_id)
            .map(|resolved| resolved.analysis)
    }

    pub fn preview_canonical_source_install_plan(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
        revision_id: &ModRevisionId,
        layer: &FileLayer,
    ) -> Result<Option<InstallPlan>, ReplacementWorkflowError> {
        let resolved = self.resolve_imported_revision(game_id, mod_id, revision_id)?;
        let Some(source) = resolved.analysis.single_source().cloned() else {
            return Ok(None);
        };
        let catalog = self
            .catalog_for(game_id)?
            .replacement_catalog()
            .map_err(map_catalog_error)?;
        let mut matching_targets = catalog.targets().iter().filter(|target| {
            target.target_type() == source.source_type()
                && target.internal_id() == source.internal_id()
                && target
                    .metadata()
                    .get("path_family")
                    .and_then(serde_json::Value::as_str)
                    == Some(source.path_family())
        });
        let (Some(target), None) = (matching_targets.next(), matching_targets.next()) else {
            return Ok(None);
        };
        let binding = ReplacementBinding::new(
            canonical_source_binding_id(game_id, profile_id, mod_id, source.id(), target.id())?,
            mod_id.clone(),
            profile_id.clone(),
            source.id().clone(),
            target.id().clone(),
            0,
        )
        .map_err(|_| ReplacementWorkflowError::BindingUnavailable)?;
        let retarget_plan = self
            .replacement
            .build_retarget_plan(RetargetPlanRequest {
                game_id: game_id.clone(),
                binding,
                assets: resolved.assets,
            })
            .map_err(ReplacementWorkflowError::Analysis)?;
        let install_plan = self
            .replacement
            .build_retarget_install_plan(&retarget_plan, layer.clone(), Some(revision_id.clone()))
            .map_err(|_| ReplacementWorkflowError::PlanUnavailable)?;
        Ok(Some(install_plan))
    }

    pub fn preview_initial_install(
        &self,
        request: PreviewInitialRetargetInstallRequest,
    ) -> Result<PlannedInitialRetargetInstall, ReplacementWorkflowError> {
        self.ensure_initial_install_allowed(
            &request.game_id,
            &request.profile_id,
            &request.mod_id,
        )?;
        let resolved = self.resolve_imported_replacement(&request.game_id, &request.mod_id)?;
        let source = resolved
            .analysis
            .single_source()
            .cloned()
            .ok_or(ReplacementWorkflowError::SourceNotRetargetable)?;
        let target = self
            .catalog_for(&request.game_id)?
            .find_replacement_target(&request.target_id)
            .map_err(map_catalog_error)?;
        let binding = ReplacementBinding::new(
            ReplacementBindingId::parse(format!("binding-{}", Uuid::new_v4()))
                .map_err(|_| ReplacementWorkflowError::BindingUnavailable)?,
            request.mod_id,
            request.profile_id,
            source.id().clone(),
            target.id().clone(),
            self.clock
                .now_unix_millis()
                .map_err(|_| ReplacementWorkflowError::BindingUnavailable)?,
        )
        .map_err(|_| ReplacementWorkflowError::BindingUnavailable)?;
        let retarget_plan = self
            .replacement
            .build_retarget_plan(RetargetPlanRequest {
                game_id: request.game_id,
                binding,
                assets: resolved.assets,
            })
            .map_err(ReplacementWorkflowError::Analysis)?;
        let install_plan = self
            .replacement
            .build_retarget_install_plan(
                &retarget_plan,
                request.layer.clone(),
                Some(resolved.revision_id.clone()),
            )
            .map_err(|_| ReplacementWorkflowError::PlanUnavailable)?;

        Ok(PlannedInitialRetargetInstall {
            package_id: resolved.package_id,
            revision_id: resolved.revision_id,
            layer: request.layer,
            analysis: resolved.analysis,
            target,
            retarget_plan,
            install_plan,
        })
    }

    pub fn materialize_initial_install(
        &self,
        staging: &dyn RetargetStagingMaterializer,
        planned: PlannedInitialRetargetInstall,
    ) -> Result<InstallPlan, ReplacementWorkflowError> {
        let materialized = self
            .replacement
            .materialize_retarget(
                staging,
                MaterializeRetargetRequest {
                    plan: planned.retarget_plan,
                    layer: planned.layer,
                    revision_id: Some(planned.revision_id),
                },
            )
            .map_err(|_| ReplacementWorkflowError::PlanUnavailable)?;
        Ok(materialized.into_parts().1)
    }

    pub fn preview_reinstall_target(
        &self,
        request: PreviewRetargetReinstallRequest,
    ) -> Result<PlannedRetargetReinstall, ReplacementWorkflowError> {
        let resolved = self.resolve_imported_revision(
            &request.game_id,
            &request.mod_id,
            &request.installed_revision_id,
        )?;
        let source = resolved
            .analysis
            .single_source()
            .cloned()
            .ok_or(ReplacementWorkflowError::SourceNotRetargetable)?;
        let installed = &request.installed_binding;
        if installed.mod_id() != &request.mod_id
            || installed.profile_id() != &request.profile_id
            || installed
                .revision_id()
                .is_some_and(|revision| revision != &request.installed_revision_id)
            || installed.binding().source_id() != source.id()
            || installed.source_internal_id() != source.internal_id()
            || installed.source_path_family() != source.path_family()
            || installed.retarget_kind() != source.source_type()
        {
            return Err(ReplacementWorkflowError::InstalledBindingUnavailable);
        }
        let target = self
            .catalog_for(&request.game_id)?
            .find_replacement_target(&request.target_id)
            .map_err(map_catalog_error)?;
        if installed.binding().target_id() == target.id()
            || installed.target_internal_id() == target.internal_id()
        {
            return Err(ReplacementWorkflowError::TargetAlreadySelected);
        }
        let binding = ReplacementBinding::new(
            installed.binding_id().clone(),
            request.mod_id,
            request.profile_id,
            source.id().clone(),
            target.id().clone(),
            installed.binding().created_at_unix_millis(),
        )
        .map_err(|_| ReplacementWorkflowError::BindingUnavailable)?;
        let retarget_plan = self
            .replacement
            .build_retarget_plan(RetargetPlanRequest {
                game_id: request.game_id,
                binding,
                assets: resolved.assets,
            })
            .map_err(ReplacementWorkflowError::Analysis)?;
        let install_plan = self
            .replacement
            .build_retarget_install_plan(
                &retarget_plan,
                request.layer.clone(),
                Some(request.installed_revision_id.clone()),
            )
            .map_err(|_| ReplacementWorkflowError::PlanUnavailable)?;

        Ok(PlannedRetargetReinstall {
            package_id: resolved.package_id,
            revision_id: request.installed_revision_id,
            layer: request.layer,
            analysis: resolved.analysis,
            target,
            retarget_plan,
            install_plan,
        })
    }

    pub fn materialize_reinstall_target(
        &self,
        staging: &dyn RetargetStagingMaterializer,
        planned: PlannedRetargetReinstall,
    ) -> Result<InstallPlan, ReplacementWorkflowError> {
        let materialized = self
            .replacement
            .materialize_retarget(
                staging,
                MaterializeRetargetRequest {
                    plan: planned.retarget_plan,
                    layer: planned.layer,
                    revision_id: Some(planned.revision_id),
                },
            )
            .map_err(|_| ReplacementWorkflowError::PlanUnavailable)?;
        Ok(materialized.into_parts().1)
    }

    fn resolve_imported_replacement(
        &self,
        game_id: &GameId,
        mod_id: &ModId,
    ) -> Result<ResolvedImportedReplacement, ReplacementWorkflowError> {
        let logical_mod = self
            .result_repository
            .get_mod(mod_id)
            .map_err(|_| ReplacementWorkflowError::ModRepositoryUnavailable)?
            .ok_or(ReplacementWorkflowError::ModNotFound)?;
        self.resolve_imported_revision(game_id, mod_id, &logical_mod.display_revision_id)
    }

    fn resolve_imported_revision(
        &self,
        game_id: &GameId,
        mod_id: &ModId,
        revision_id: &ModRevisionId,
    ) -> Result<ResolvedImportedReplacement, ReplacementWorkflowError> {
        let revision = self
            .result_repository
            .get_revision(revision_id)
            .map_err(|_| ReplacementWorkflowError::ModRepositoryUnavailable)?
            .filter(|revision| revision.mod_id == *mod_id)
            .ok_or(ReplacementWorkflowError::RevisionNotFound)?;
        let sandbox_root = self
            .sandbox_locator
            .sandbox_root_for_package(&revision.package_id)
            .map_err(|_| ReplacementWorkflowError::SandboxUnavailable)?;
        let assets = self.scan_assets(&revision.package_id, sandbox_root)?;
        let analysis = self
            .replacement
            .analyze(ReplacementAnalysisRequest {
                game_id: game_id.clone(),
                assets: assets.clone(),
            })
            .map_err(ReplacementWorkflowError::Analysis)?;

        Ok(ResolvedImportedReplacement {
            package_id: revision.package_id,
            revision_id: revision.revision_id,
            assets,
            analysis,
        })
    }

    fn scan_assets(
        &self,
        package_id: &str,
        sandbox_root: PathBuf,
    ) -> Result<Vec<ReplacementAsset>, ReplacementWorkflowError> {
        self.file_scanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id,
                sandbox_root: &sandbox_root,
            })
            .map_err(|_| ReplacementWorkflowError::PackageFilesUnavailable)
            .map(|files| {
                files
                    .into_iter()
                    .map(|file| {
                        ReplacementAsset::new(
                            PackageFileId::new(file.package_file_id),
                            file.target_path,
                        )
                    })
                    .collect()
            })
    }

    fn ensure_initial_install_allowed(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Result<(), ReplacementWorkflowError> {
        let status = self
            .install_status
            .recovery_status(game_id, profile_id, mod_id)
            .map_err(|_| ReplacementWorkflowError::InstallStatusUnavailable)?;
        if status == InstallRecoveryStatus::NotInstalled {
            Ok(())
        } else {
            Err(ReplacementWorkflowError::InitialInstallBlocked { status })
        }
    }

    fn catalog_for(
        &self,
        game_id: &GameId,
    ) -> Result<Arc<dyn ReplacementCatalogProvider>, ReplacementWorkflowError> {
        self.catalogs
            .iter()
            .find(|catalog| catalog.game_id() == *game_id)
            .cloned()
            .ok_or(ReplacementWorkflowError::UnsupportedGame)
    }
}

fn map_catalog_error(error: ReplacementCatalogError) -> ReplacementWorkflowError {
    match error {
        ReplacementCatalogError::TargetNotFound { .. } => ReplacementWorkflowError::TargetNotFound,
        ReplacementCatalogError::CatalogUnavailable
        | ReplacementCatalogError::CatalogInvalid
        | ReplacementCatalogError::UnsupportedSchemaVersion { .. } => {
            ReplacementWorkflowError::CatalogUnavailable
        }
    }
}

pub fn is_identity_replacement_binding(snapshot: &ReplacementBindingSnapshot) -> bool {
    snapshot.binding().created_at_unix_millis() == 0
        && snapshot.source_internal_id() == snapshot.target_internal_id()
        && snapshot.source_path_family() == snapshot.target_path_family()
}

fn canonical_source_binding_id(
    game_id: &GameId,
    profile_id: &ProfileId,
    mod_id: &ModId,
    source_id: &hmm_core::ReplacementSourceId,
    target_id: &ReplacementTargetId,
) -> Result<ReplacementBindingId, ReplacementWorkflowError> {
    let mut hasher = Sha256::new();
    for value in [
        "hmm-canonical-source-binding-v1",
        game_id.as_str(),
        profile_id.as_str(),
        mod_id.as_str(),
        source_id.as_str(),
        target_id.as_str(),
    ] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    ReplacementBindingId::parse(format!("binding-{}", Uuid::from_bytes(bytes)))
        .map_err(|_| ReplacementWorkflowError::BindingUnavailable)
}
