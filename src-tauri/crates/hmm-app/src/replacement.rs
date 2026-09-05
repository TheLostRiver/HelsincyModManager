use hmm_core::{
    FileLayer, GameId, InstallFileProvider, InstallManifestStatusConsumption, InstallPlan,
    InstallPlanValidationError, ModId, ModRevisionId, PackageFileId, ProfileId,
    ReplacementAnalysis, ReplacementBinding, ReplacementBindingId, ReplacementBindingSnapshot,
    ReplacementTarget, ReplacementTargetId, RetargetError, RetargetPlan, RetargetSourceRouting,
};
use hmm_ports::{
    AppClock, InstallManifestRepository, ModImportResultRepository, ModImportSandboxLocator,
    ModPackageInstallFileReadRequest, ModPackageInstallFileReader,
    ModPackageInstallFileScanRequest, ModPackageInstallFileScanner, ReplacementAdapter,
    ReplacementAdapterError, ReplacementAnalysisRequest, ReplacementAsset,
    ReplacementAssetContentReader, ReplacementCatalogError, ReplacementCatalogProvider,
    RetargetPlanRequest, RetargetStagingError, RetargetStagingFile, RetargetStagingMaterializer,
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use crate::install::cross_mod_target_conflicts;
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
    #[error("retarget plan transform facts are invalid")]
    InvalidRetargetPlan(#[from] RetargetError),
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
    #[error("install manifest is unavailable")]
    InstallManifestUnavailable,
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

    /// catalog 解析出的规范目标。调用方要做身份校验时必须比对它，
    /// 而不是请求里那个原样字符串——请求可能带的是 legacy_ids 里的旧 slug。
    pub fn target(&self) -> &ReplacementTarget {
        &self.target
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
    sandbox_root: PathBuf,
    assets: Vec<ReplacementAsset>,
    analysis: ReplacementAnalysis,
}

pub struct ReplacementWorkflowService {
    replacement: ReplacementService,
    catalogs: Vec<Arc<dyn ReplacementCatalogProvider>>,
    result_repository: Arc<dyn ModImportResultRepository>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    file_scanner: Arc<dyn ModPackageInstallFileScanner>,
    file_reader: Arc<dyn ModPackageInstallFileReader>,
    install_status: Arc<dyn InitialRetargetInstallStatusReader>,
    install_manifests: Arc<dyn InstallManifestRepository>,
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

    /// 提交这个计划时「哪些文件该从 staging 读」。提交方拿它路由源文件读取，
    /// 而不是从 `InstallPlan` 反推——见 `RetargetSourceRouting`。
    pub fn source_routing(&self) -> RetargetSourceRouting {
        self.retarget_plan.source_routing()
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

    pub fn build_retarget_plan_with_content(
        &self,
        request: RetargetPlanRequest,
        content_reader: &dyn ReplacementAssetContentReader,
    ) -> Result<RetargetPlan, ReplacementServiceError> {
        let adapter = self.adapter_for(&request.game_id)?;
        adapter
            .build_retarget_plan_with_content(request, content_reader)
            .map_err(Into::into)
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
                let file = RetargetStagingFile::new(
                    action.package_file_id().clone(),
                    action.target_relative_path().clone(),
                );
                match action.content_transform() {
                    Some(invocation) => file.with_content_transform(invocation.clone()),
                    None => file,
                }
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
        plan.validate_transform_facts()?;
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
        file_reader: Arc<dyn ModPackageInstallFileReader>,
        install_status: Arc<dyn InitialRetargetInstallStatusReader>,
        install_manifests: Arc<dyn InstallManifestRepository>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            replacement: ReplacementService::new(replacement_adapters),
            catalogs,
            result_repository,
            sandbox_locator,
            file_scanner,
            file_reader,
            install_status,
            install_manifests,
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

    pub fn list_compatible_targets(
        &self,
        game_id: &GameId,
        mod_id: &ModId,
        query: Option<&str>,
    ) -> Result<Vec<ReplacementTarget>, ReplacementWorkflowError> {
        let resolved = self.resolve_imported_replacement(game_id, mod_id)?;
        let source = resolved
            .analysis
            .single_source()
            .ok_or(ReplacementWorkflowError::SourceNotRetargetable)?;
        let targets = self.list_targets(game_id, query)?;
        Ok(targets
            .into_iter()
            .filter(|target| {
                target.target_type() == source.source_type()
                    && target
                        .metadata()
                        .get("path_family")
                        .and_then(serde_json::Value::as_str)
                        == Some(source.path_family())
            })
            .collect())
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
        let content_reader = ImportedReplacementContentReader {
            reader: self.file_reader.as_ref(),
            package_id: &resolved.package_id,
            sandbox_root: &resolved.sandbox_root,
        };
        let retarget_plan = self
            .replacement
            .build_retarget_plan_with_content(
                RetargetPlanRequest {
                    game_id: game_id.clone(),
                    binding,
                    assets: resolved.assets,
                    // 一次只提交一个绑定，所以它就是包级随行资源的唯一承载者。
                    // 多绑定提交（`#349` 切片③b-3）会在 N 个绑定里指定恰好一个。
                    carries_package_companions: true,
                },
                &content_reader,
            )
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
        let profile_id = request.profile_id.clone();
        let mod_id = request.mod_id.clone();
        self.ensure_initial_install_allowed(&request.game_id, &profile_id, &mod_id)?;
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
        let content_reader = ImportedReplacementContentReader {
            reader: self.file_reader.as_ref(),
            package_id: &resolved.package_id,
            sandbox_root: &resolved.sandbox_root,
        };
        let retarget_plan = self
            .replacement
            .build_retarget_plan_with_content(
                RetargetPlanRequest {
                    game_id: request.game_id,
                    binding,
                    assets: resolved.assets,
                    // 同上：单绑定提交，本计划承载包级随行资源。
                    carries_package_companions: true,
                },
                &content_reader,
            )
            .map_err(ReplacementWorkflowError::Analysis)?;
        let install_plan = self
            .replacement
            .build_retarget_install_plan(
                &retarget_plan,
                request.layer.clone(),
                Some(resolved.revision_id.clone()),
            )
            .map_err(|_| ReplacementWorkflowError::PlanUnavailable)?;
        let install_plan = self.append_cross_mod_target_conflicts(install_plan, &profile_id)?;

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

    /// 初始重定向安装不得覆盖其他 Mod 已管理的目标文件。
    ///
    /// `InstallPlan::from_providers` 只能看见本计划内的 provider，跨 Mod 的目标
    /// 占用记录在 profile 安装清单里；这里把外来 owner 以冲突 provider 的形式
    /// 并入 plan.conflicts，使预览与任务期重建的计划都携带阻断冲突——前端按钮
    /// 门禁与 commit 侧 `PlanHasBlockingConflicts` 因此同时生效。
    /// 清单不存在视为干净目标；读取失败或状态不可信（提交中/待恢复）按
    /// fail-closed 返回错误，绝不放行一次归属未知的写入。
    ///
    /// 占用判定本身在 `cross_mod_target_conflicts`，与常规安装 commit 共用，
    /// 保证「预览说有冲突」和「commit 说有冲突」永远是同一件事。
    fn append_cross_mod_target_conflicts(
        &self,
        mut install_plan: InstallPlan,
        profile_id: &ProfileId,
    ) -> Result<InstallPlan, ReplacementWorkflowError> {
        let manifest = match self.install_manifests.load_manifest(profile_id) {
            Ok(Some(manifest))
                if manifest.status.consumption()
                    == InstallManifestStatusConsumption::TrustEntries =>
            {
                manifest
            }
            Ok(Some(_)) | Err(_) => {
                return Err(ReplacementWorkflowError::InstallManifestUnavailable);
            }
            Ok(None) => return Ok(install_plan),
        };

        install_plan
            .conflicts
            .extend(cross_mod_target_conflicts(Some(&manifest), &install_plan));
        Ok(install_plan)
    }

    /// 返回 `MaterializedRetarget` 而不是裸的 `InstallPlan`：提交方还需要
    /// `source_routing()`——「哪些 `package_file_id` 该从 staging 读」只在这里可知。
    pub fn materialize_initial_install(
        &self,
        staging: &dyn RetargetStagingMaterializer,
        planned: PlannedInitialRetargetInstall,
    ) -> Result<MaterializedRetarget, ReplacementWorkflowError> {
        self.replacement
            .materialize_retarget(
                staging,
                MaterializeRetargetRequest {
                    plan: planned.retarget_plan,
                    layer: planned.layer,
                    revision_id: Some(planned.revision_id),
                },
            )
            .map_err(|_| ReplacementWorkflowError::PlanUnavailable)
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
        let content_reader = ImportedReplacementContentReader {
            reader: self.file_reader.as_ref(),
            package_id: &resolved.package_id,
            sandbox_root: &resolved.sandbox_root,
        };
        let retarget_plan = self
            .replacement
            .build_retarget_plan_with_content(
                RetargetPlanRequest {
                    game_id: request.game_id,
                    binding,
                    assets: resolved.assets,
                    // 同上：单绑定提交，本计划承载包级随行资源。
                    carries_package_companions: true,
                },
                &content_reader,
            )
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
        let assets = self.scan_assets(&revision.package_id, &sandbox_root)?;
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
            sandbox_root,
            assets,
            analysis,
        })
    }

    fn scan_assets(
        &self,
        package_id: &str,
        sandbox_root: &std::path::Path,
    ) -> Result<Vec<ReplacementAsset>, ReplacementWorkflowError> {
        self.file_scanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id,
                sandbox_root,
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

struct ImportedReplacementContentReader<'a> {
    reader: &'a dyn ModPackageInstallFileReader,
    package_id: &'a str,
    sandbox_root: &'a std::path::Path,
}

impl ReplacementAssetContentReader for ImportedReplacementContentReader<'_> {
    fn read_asset_content(
        &self,
        package_file_id: &PackageFileId,
        max_bytes: u64,
    ) -> Result<Vec<u8>, ReplacementAdapterError> {
        self.reader
            .read_install_file(ModPackageInstallFileReadRequest {
                package_id: self.package_id,
                sandbox_root: self.sandbox_root,
                package_file_id,
                max_bytes,
            })
            .map_err(|_| ReplacementAdapterError::SourceContentUnavailable)
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
