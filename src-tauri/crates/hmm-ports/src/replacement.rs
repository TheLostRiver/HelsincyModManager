use hmm_core::{GameId, ReplacementCatalog, ReplacementTarget, ReplacementTargetId};
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
