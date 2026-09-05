use hmm_core::{
    FileLayer, GameId, InstallFileProvider, InstallManifestStatusConsumption, InstallPlan,
    InstallPlanValidationError, ModId, ModRevisionId, PackageFileId, ProfileId,
    ReplacementAnalysis, ReplacementBinding, ReplacementBindingId, ReplacementBindingSnapshot,
    ReplacementSourceId, ReplacementTarget, ReplacementTargetId, RetargetError, RetargetPlan,
    RetargetSourceRouting,
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
use std::collections::BTreeSet;
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
    /// 同一个源槽位在一次提交里被给了两条意图。谁生效都可能是错的，所以拒绝。
    #[error("one replacement source carries two slot intents")]
    DuplicateSlotIntent,
    /// 一次提交里两个源槽位指向了同一个目标。
    ///
    /// 不拦的话它照样装不上（两个 provider 撞同一个 `target_path`，动作全进 `conflicts`、
    /// `actions` 为空，绑定校验随后报 `ReplacementBindingOwnerMissing`），但报出来的是
    /// 「计划不可用」——玩家看不出是自己把两件装备指到了一处。这里提前具名拒绝，
    /// 让 `#349` 切片④ 的文案有准确的根因可讲。
    #[error("two replacement sources aim at one target")]
    DuplicateSlotTarget,
    /// 「保持原位」要求源槽位本身在 catalog 里能唯一解析成一个目标（`#349` D2）。
    /// 解析不出时那个槽位只能选「换到 X」或「不装」——猜会把文件装到别的装备上。
    #[error("keeping this replacement source in place is unavailable")]
    KeepInPlaceUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzeImportedReplacementRequest {
    pub game_id: GameId,
    pub mod_id: ModId,
}

/// 玩家对**一个源槽位**的意图（`#349` D2 三态）。
///
/// 「不装」不在这里出现——它就是**不把这个槽位放进 `slots`**，于是它的文件不进计划。
/// 三态因此只需要两个变体加上「缺席」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitialRetargetSlotIntent {
    /// 换到某个目标槽位：创建重定向绑定，产出走 staging。
    Retarget {
        source_id: ReplacementSourceId,
        target_id: ReplacementTargetId,
    },
    /// 保持原位：文件按**原路径**照常安装。
    ///
    /// 内部解析成一个指向源槽位自己的 identity 绑定（`created_at == 0` 且源目标同一，
    /// 与 canonical source install 用的是同一套机制）。它不进源路由，所以提交时直接读
    /// 沙箱原包——「不重定向」在字节层面就是「不经 staging」。
    ///
    /// 源槽位本身必须在 catalog 里能唯一解析成一个目标；解析不出时返回
    /// `KeepInPlaceUnavailable`，那个槽位只能选「换到 X」或「不装」。
    KeepInPlace { source_id: ReplacementSourceId },
}

impl InitialRetargetSlotIntent {
    pub fn source_id(&self) -> &ReplacementSourceId {
        match self {
            Self::Retarget { source_id, .. } | Self::KeepInPlace { source_id } => source_id,
        }
    }
}

/// 一次初始重定向安装要装什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitialRetargetSelection {
    /// `#349` 切片③b 之前唯一的形状：整包**恰好一个**源槽位，重定向到一个目标，
    /// 源由分析推断。多槽位包在这一档下仍报 `SourceNotRetargetable`——与切片③b 之前
    /// 逐字相同的行为。前端要装多槽位包得改发 [`Self::PerSlot`]（`#349` 切片④）。
    SoleSource { target_id: ReplacementTargetId },
    /// 逐槽位意图（D2 三态）。不在列表里的槽位就是「不装」。
    PerSlot(Vec<InitialRetargetSlotIntent>),
}

impl InitialRetargetSelection {
    /// 单目标形状下的那个目标。`PerSlot` 返回 `None`——多槽位选择没有单值「目标」，
    /// 这正是 `#349` 之前的模型不成立的地方。
    pub fn sole_target_id(&self) -> Option<&ReplacementTargetId> {
        match self {
            Self::SoleSource { target_id } => Some(target_id),
            Self::PerSlot(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewInitialRetargetInstallRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub selection: InitialRetargetSelection,
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
    /// 与 `retarget_plans` 一一对应、同序。「保持原位」的槽位在这里是它自己解析出的目标。
    targets: Vec<ReplacementTarget>,
    retarget_plans: Vec<RetargetPlan>,
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

    pub fn targets(&self) -> &[ReplacementTarget] {
        &self.targets
    }

    pub fn retarget_plans(&self) -> &[RetargetPlan] {
        &self.retarget_plans
    }

    pub fn install_plan(&self) -> &InstallPlan {
        &self.install_plan
    }

    /// 需要各自 staging 根的绑定——**只有重定向的**。
    ///
    /// 「保持原位」的 identity 绑定不在其中：它的文件不经字节改写，提交时直接读沙箱原包。
    pub fn staged_binding_ids(&self) -> Vec<ReplacementBindingId> {
        self.retarget_plans
            .iter()
            .filter(|plan| !is_identity_retarget_plan(plan))
            .map(|plan| plan.binding().id().clone())
            .collect()
    }

    /// 提交时的源路由：**覆盖每一个动作**，两种来源都显式记录。
    ///
    /// 「保持原位」的槽位记 `ImportedPackage` 而不是干脆不记——不记会让「组装方漏了一个
    /// 文件」与「这个文件本来就该读原包」在提交侧无法区分，漏记的文件会拿未重定向的原包
    /// 字节写进重定向后的目标路径。提交侧因此能逐动作核对覆盖面（见
    /// `ConfiguredInstallCommitter`）。
    pub fn source_routing(&self) -> Result<RetargetSourceRouting, ReplacementWorkflowError> {
        let mut routing = RetargetSourceRouting::empty();
        for plan in &self.retarget_plans {
            if is_identity_retarget_plan(plan) {
                for action in plan.actions() {
                    routing
                        .read_from_package(action.package_file_id().clone())
                        .map_err(|_| ReplacementWorkflowError::PlanUnavailable)?;
                }
                continue;
            }
            routing
                .merge(plan.source_routing())
                .map_err(|_| ReplacementWorkflowError::PlanUnavailable)?;
        }
        Ok(routing)
    }
}

/// 「保持原位」产出的计划：目标就是源槽位自己，所以字节不用改、不用进 staging。
///
/// 判据与 `is_identity_replacement_binding` 对齐（源与目标的 internal_id 及 path_family
/// 同一），只是这里看的是尚未转成快照的计划。
fn is_identity_retarget_plan(plan: &RetargetPlan) -> bool {
    plan.binding().created_at_unix_millis() == 0
        && plan.actions().iter().all(|action| {
            action.source_internal_id() == action.target_internal_id()
                && action.source_path_family() == action.target_path_family()
        })
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

/// 每个重定向绑定有自己的 staging 根，所以 materializer 要按绑定现取（`#349` 切片③b）。
///
/// 定义成 trait 而不是让 `ReplacementWorkflowService` 自己拼路径：staging 根的磁盘布局
/// （`install/retarget-staging/<binding uuid>/`）是运行时组装层的知识，app 层不该知道。
pub trait RetargetStagingMaterializerFactory {
    fn materializer_for(
        &self,
        binding_id: &ReplacementBindingId,
    ) -> Result<Box<dyn RetargetStagingMaterializer>, ReplacementWorkflowError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedInitialRetargetInstall {
    install_plan: InstallPlan,
    source_routing: RetargetSourceRouting,
}

impl MaterializedInitialRetargetInstall {
    pub fn install_plan(&self) -> &InstallPlan {
        &self.install_plan
    }

    pub fn source_routing(&self) -> &RetargetSourceRouting {
        &self.source_routing
    }

    pub fn into_parts(self) -> (InstallPlan, RetargetSourceRouting) {
        (self.install_plan, self.source_routing)
    }
}

/// 一个重定向计划要落进 staging 的文件清单。与 `materialize_retarget` 逐字同构。
fn retarget_staging_files(plan: &RetargetPlan) -> Vec<RetargetStagingFile> {
    plan.actions()
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
        .collect()
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
        self.build_retarget_install_plan_for_all(std::slice::from_ref(plan), layer, revision_id)
    }

    /// 把 N 个重定向计划合并成**一个**安装计划。
    ///
    /// `#349` 切片③b：一个包里的多件装备各自绑定、各自产出，但只提交一次。合并发生在
    /// provider 层而不是「拼接两个 `InstallPlan`」——`InstallPlan::from_providers` 的冲突
    /// 判定要看见全部 provider 才能发现「两个槽位重定向到了同一个目标」，分开构造再拼
    /// 会让那种撞车漏过去。
    ///
    /// 单计划输入与切片③b 之前**逐字等价**：provider 顺序、绑定快照、`plan_hash` 都不变。
    pub fn build_retarget_install_plan_for_all(
        &self,
        plans: &[RetargetPlan],
        layer: FileLayer,
        revision_id: Option<ModRevisionId>,
    ) -> Result<InstallPlan, RetargetMaterializeError> {
        let mut snapshots = Vec::with_capacity(plans.len());
        let mut providers = Vec::new();
        for plan in plans {
            plan.validate_transform_facts()?;
            snapshots.push(ReplacementBindingSnapshot::from_retarget_plan(
                plan,
                revision_id.clone(),
            ));
            providers.extend(plan.actions().iter().map(|action| {
                InstallFileProvider::new(
                    plan.binding().mod_id().clone(),
                    action.package_file_id().clone(),
                    action.target_relative_path().clone(),
                    layer.clone(),
                )
            }));
        }
        Ok(InstallPlan::from_providers(providers).with_replacement_bindings(snapshots)?)
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
        let slots = match &request.selection {
            // 源由分析推断，多槽位包在这里就报错——切片③b 之前逐字相同的行为。
            InitialRetargetSelection::SoleSource { target_id } => {
                let source = resolved
                    .analysis
                    .single_source()
                    .ok_or(ReplacementWorkflowError::SourceNotRetargetable)?;
                vec![InitialRetargetSlotIntent::Retarget {
                    source_id: source.id().clone(),
                    target_id: target_id.clone(),
                }]
            }
            InitialRetargetSelection::PerSlot(slots) => slots.clone(),
        };
        if slots.is_empty() {
            return Err(ReplacementWorkflowError::SourceNotRetargetable);
        }
        let mut claimed_sources = BTreeSet::new();
        for slot in &slots {
            if !claimed_sources.insert(slot.source_id().clone()) {
                return Err(ReplacementWorkflowError::DuplicateSlotIntent);
            }
        }
        let catalog = self.catalog_for(&request.game_id)?;
        let content_reader = ImportedReplacementContentReader {
            reader: self.file_reader.as_ref(),
            package_id: &resolved.package_id,
            sandbox_root: &resolved.sandbox_root,
        };

        /*
         * 包级随行资源（族级作者目录、族级 `epv/` `sound/`）属于包、不属于任何槽位，
         * 一个包只装一次，所以在 N 个绑定里指定**恰好一个**承载者（`#349` 切片③b-2）。
         *
         * 承载者是谁不影响正确性：包级文件的目标路径恒等于原路径，两档绑定都能把它装到位
         * （重定向绑定经 staging 转一手，identity 绑定直接读沙箱原包）。承重的只有「恰好
         * 一个」——多于一个会让同一个 `target_path` 出现多个 provider、撞成阻断冲突；
         * 一个都没有则这些文件不进计划。承载者必然在 `slots` 里，所以不会因为槽位被
         * 「不装」而丢。
         *
         * 取第一个，不做偏好：曾经这里选「第一个重定向的槽位」并注释成「否则会撞」，
         * 那个理由是错的（反向验证里把它改成恒取 0 之后一条用例都没转红），留着只会误导。
         */
        let carrier_index = 0;

        let mut targets = Vec::with_capacity(slots.len());
        let mut retarget_plans = Vec::with_capacity(slots.len());
        for (index, slot) in slots.iter().enumerate() {
            let source = resolved
                .analysis
                .sources()
                .iter()
                .find(|source| source.id() == slot.source_id())
                .ok_or(ReplacementWorkflowError::SourceNotRetargetable)?;
            let (target, binding) = match slot {
                InitialRetargetSlotIntent::Retarget { target_id, .. } => {
                    let target = catalog
                        .find_replacement_target(target_id)
                        .map_err(map_catalog_error)?;
                    let binding = ReplacementBinding::new(
                        ReplacementBindingId::parse(format!("binding-{}", Uuid::new_v4()))
                            .map_err(|_| ReplacementWorkflowError::BindingUnavailable)?,
                        mod_id.clone(),
                        profile_id.clone(),
                        source.id().clone(),
                        target.id().clone(),
                        self.clock
                            .now_unix_millis()
                            .map_err(|_| ReplacementWorkflowError::BindingUnavailable)?,
                    )
                    .map_err(|_| ReplacementWorkflowError::BindingUnavailable)?;
                    (target, binding)
                }
                InitialRetargetSlotIntent::KeepInPlace { .. } => {
                    let target = self.self_target_for(&request.game_id, source)?;
                    let binding = ReplacementBinding::new(
                        canonical_source_binding_id(
                            &request.game_id,
                            &profile_id,
                            &mod_id,
                            source.id(),
                            target.id(),
                        )?,
                        mod_id.clone(),
                        profile_id.clone(),
                        source.id().clone(),
                        target.id().clone(),
                        0,
                    )
                    .map_err(|_| ReplacementWorkflowError::BindingUnavailable)?;
                    (target, binding)
                }
            };
            let retarget_plan = self
                .replacement
                .build_retarget_plan_with_content(
                    RetargetPlanRequest {
                        game_id: request.game_id.clone(),
                        binding,
                        assets: resolved.assets.clone(),
                        carries_package_companions: index == carrier_index,
                    },
                    &content_reader,
                )
                .map_err(ReplacementWorkflowError::Analysis)?;
            targets.push(target);
            retarget_plans.push(retarget_plan);
        }

        // 「保持原位」的自身目标与别的槽位的重定向目标同样可能撞（把 A 换到 B 的位置、
        // 同时让 B 保持原位），所以检查放在**全部**目标解析完之后，而不是只看 Retarget。
        let mut claimed_targets = BTreeSet::new();
        for target in &targets {
            if !claimed_targets.insert(target.id().clone()) {
                return Err(ReplacementWorkflowError::DuplicateSlotTarget);
            }
        }

        let install_plan = self
            .replacement
            .build_retarget_install_plan_for_all(
                &retarget_plans,
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
            targets,
            retarget_plans,
            install_plan,
        })
    }

    /// 「保持原位」需要的那个目标：源槽位自己。
    ///
    /// 与 `preview_canonical_source_install_plan` 同一套解析（catalog 里 target_type /
    /// internal_id / path_family 三者都匹配且**唯一**）。解析不出就明确报错，不猜——
    /// 猜错会把这个槽位的文件装到别的装备上。
    fn self_target_for(
        &self,
        game_id: &GameId,
        source: &hmm_core::ReplacementSource,
    ) -> Result<ReplacementTarget, ReplacementWorkflowError> {
        let catalog = self
            .catalog_for(game_id)?
            .replacement_catalog()
            .map_err(map_catalog_error)?;
        let mut matching = catalog.targets().iter().filter(|target| {
            target.target_type() == source.source_type()
                && target.internal_id() == source.internal_id()
                && target
                    .metadata()
                    .get("path_family")
                    .and_then(serde_json::Value::as_str)
                    == Some(source.path_family())
        });
        match (matching.next(), matching.next()) {
            (Some(target), None) => Ok(target.clone()),
            _ => Err(ReplacementWorkflowError::KeepInPlaceUnavailable),
        }
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

    /// 逐个绑定 materialize，然后把 N 个计划合并成一次提交。
    ///
    /// `#349` 切片③b：每个重定向绑定有自己的 staging 根，所以 materializer 要**按绑定现取**
    /// （`factory`）。「保持原位」的 identity 绑定跳过 staging——它的文件不经字节改写，
    /// 提交时直接读沙箱原包。
    ///
    /// 中途失败时，已经建好的 staging 目录由调用方按 `planned.staged_binding_ids()` 清理：
    /// 那份清单在调用前就能取到，不依赖本函数的返回值。
    pub fn materialize_initial_install(
        &self,
        factory: &dyn RetargetStagingMaterializerFactory,
        planned: PlannedInitialRetargetInstall,
    ) -> Result<MaterializedInitialRetargetInstall, ReplacementWorkflowError> {
        let source_routing = planned.source_routing()?;
        let layer = planned.layer.clone();
        let revision_id = planned.revision_id.clone();
        for plan in &planned.retarget_plans {
            if is_identity_retarget_plan(plan) {
                continue;
            }
            let materializer = factory.materializer_for(plan.binding().id())?;
            let staging_files = retarget_staging_files(plan);
            materializer
                .materialize(&staging_files)
                .map_err(|_| ReplacementWorkflowError::PlanUnavailable)?;
        }
        // staging 落盘之后再重建安装计划：与 `materialize_retarget` 同序（先算计划、再落盘、
        // 计划不变），保证 `plan_hash` 与预览阶段逐字一致。
        let install_plan = self
            .replacement
            .build_retarget_install_plan_for_all(&planned.retarget_plans, layer, Some(revision_id))
            .map_err(|_| ReplacementWorkflowError::PlanUnavailable)?;
        Ok(MaterializedInitialRetargetInstall {
            install_plan,
            source_routing,
        })
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
