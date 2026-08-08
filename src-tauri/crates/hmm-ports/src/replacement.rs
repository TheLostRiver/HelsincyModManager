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
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplacementAdapterError {
    #[error("replacement adapter does not support the requested game")]
    UnsupportedGame,
    #[error("replacement source slot was not recognized")]
    UnrecognizedSourceSlot,
    #[error("replacement source is ambiguous or unsupported")]
    AmbiguousSourceSlot,
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
