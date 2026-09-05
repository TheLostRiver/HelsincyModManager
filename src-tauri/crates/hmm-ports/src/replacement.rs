use hmm_core::{
    GameId, PackageFileId, ReplacementAnalysis, ReplacementBinding, ReplacementCatalog,
    ReplacementTarget, ReplacementTargetId, RetargetPlan,
};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplacementCatalogError {
    #[error("replacement catalog is unavailable")]
    CatalogUnavailable,
    #[error("replacement catalog is invalid")]
    CatalogInvalid,
    #[error("unsupported replacement catalog schema version: {schema_version}")]
    UnsupportedSchemaVersion { schema_version: u32 },
    #[error("replacement target not found: {target_id}")]
    TargetNotFound { target_id: ReplacementTargetId },
}

pub type ReplacementCatalogResult<T> = Result<T, ReplacementCatalogError>;

pub trait ReplacementCatalogProvider: Send + Sync {
    fn game_id(&self) -> GameId;

    fn replacement_catalog(&self) -> ReplacementCatalogResult<ReplacementCatalog>;

    fn find_replacement_target(
        &self,
        target_id: &ReplacementTargetId,
    ) -> ReplacementCatalogResult<ReplacementTarget> {
        self.replacement_catalog()?
            .find(target_id)
            .cloned()
            .ok_or_else(|| ReplacementCatalogError::TargetNotFound {
                target_id: target_id.clone(),
            })
    }

    fn search_replacement_targets(
        &self,
        query: &str,
    ) -> ReplacementCatalogResult<Vec<ReplacementTarget>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementAsset {
    package_file_id: PackageFileId,
    relative_path: String,
}

impl ReplacementAsset {
    pub fn new(package_file_id: PackageFileId, relative_path: impl Into<String>) -> Self {
        Self {
            package_file_id,
            relative_path: relative_path.into(),
        }
    }

    pub fn package_file_id(&self) -> &PackageFileId {
        &self.package_file_id
    }

    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementAnalysisRequest {
    pub game_id: GameId,
    pub assets: Vec<ReplacementAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetargetPlanRequest {
    pub game_id: GameId,
    pub binding: ReplacementBinding,
    pub assets: Vec<ReplacementAsset>,
    /// 这次计划是否承载**包级**随行资源（族级作者目录、族级 `epv/` `sound/`）。
    ///
    /// `#349` 切片③b：那些文件属于包、不属于任何槽位，一个包只该装一次。多槽位包一次
    /// 提交 N 个绑定时，组装方在其中**恰好一个**上置 `true`；单槽位包恒为 `true`。
    ///
    /// 之所以是组装方的决定而不是适配器的：只有组装方知道用户对每个槽位的意图
    /// （换到 X / 保持原位 / 不装），而承载者必须是一个**真的要装**的绑定。
    ///
    /// 置错的后果是可发现的：多个绑定都承载会让同一个 `target_path` 出现多个 provider、
    /// 在 `InstallPlan` 里撞成阻断冲突；一个都不承载则这些文件不进计划。
    pub carries_package_companions: bool,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplacementAdapterError {
    #[error("replacement adapter does not support the requested game")]
    UnsupportedGame,
    #[error("replacement source slot was not recognized")]
    UnrecognizedSourceSlot,
    /// 保留但**不再由防具侧产生**（`#349` 切片②）：它原先同时承担两件事——「包里有多个源
    /// 槽位」与「唯一的那个槽位不受支持」。前者不是错误（多槽位包现在逐槽位绑定），后者
    /// 由 [`Self::SourceHasNoAvailableTargets`] 具名。存量 manifest 与日志仍可解析。
    #[error("replacement source is ambiguous or unsupported")]
    AmbiguousSourceSlot,
    /// 源槽位**识别得出，但本游戏当前没有可选的替换目标**。
    ///
    /// `#356`：MHW 的防具 catalog 只覆盖女性装备（实测 269 条目标全是 `pl/f_equip`），
    /// 男装包因此一件都换不了。此前它掉进 `AmbiguousSourceSlot`，玩家看到「源槽位有歧义」
    /// ——而包里明明只有一个槽位，诊断与事实相反。
    ///
    /// 与 [`Self::UnrecognizedSourceSlot`] 的区别是**认不认得**：那条是「这不像个源槽位」，
    /// 这条是「认得，但没地方可换」。前者要玩家换个包，后者是本工具的覆盖面限制。
    #[error("replacement source has no available targets")]
    SourceHasNoAvailableTargets,
    #[error("retarget path is unsafe")]
    UnsafeRetargetPath,
    #[error("replacement binding does not reference the analyzed source")]
    SourceBindingMismatch,
    #[error("replacement target is not supported")]
    UnsupportedReplacementTarget,
    #[error("replacement target catalog is unavailable")]
    TargetCatalogUnavailable,
    #[error("replacement target is missing from the catalog: {target_id}")]
    TargetCatalogMissing { target_id: ReplacementTargetId },
    #[error("retarget plan is invalid")]
    InvalidRetargetPlan,
    #[error("replacement source content is unavailable")]
    SourceContentUnavailable,
    #[error("replacement analysis was rejected: {code}")]
    AnalysisRejected { code: &'static str },
}

pub type ReplacementAdapterResult<T> = Result<T, ReplacementAdapterError>;

pub trait ReplacementAssetContentReader: Send + Sync {
    fn read_asset_content(
        &self,
        package_file_id: &PackageFileId,
        max_bytes: u64,
    ) -> ReplacementAdapterResult<Vec<u8>>;
}

pub trait ReplacementAdapter: Send + Sync {
    fn game_id(&self) -> GameId;

    fn analyze_replacement_assets(
        &self,
        request: ReplacementAnalysisRequest,
    ) -> ReplacementAdapterResult<ReplacementAnalysis>;

    fn build_retarget_plan(
        &self,
        request: RetargetPlanRequest,
    ) -> ReplacementAdapterResult<RetargetPlan>;

    fn build_retarget_plan_with_content(
        &self,
        request: RetargetPlanRequest,
        _content_reader: &dyn ReplacementAssetContentReader,
    ) -> ReplacementAdapterResult<RetargetPlan> {
        self.build_retarget_plan(request)
    }
}
