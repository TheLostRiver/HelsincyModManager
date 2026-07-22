use anyhow::Result;
use hmm_core::{
    ExternalImportBatch, ExternalImportBatchId, ExternalImportCandidate, ExternalImportCandidateId,
    ExternalImportItemResult, ExternalImportResourceBudget, ExternalImportResourceUsage,
    ExternalImportSelection, ExternalImportSelectionId, ExternalImportSource,
    ExternalImportSourceId,
};

pub struct ExternalImportScanRequest<'a> {
    /// The source is an opaque, short-lived handle. It deliberately contains no filesystem path.
    pub source: &'a ExternalImportSource,
    pub batch: &'a ExternalImportBatch,
    pub resource_budget: &'a ExternalImportResourceBudget,
    pub cancellation_token: &'a dyn crate::CancellationToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportScanResult {
    pub candidates: Vec<ExternalImportCandidate>,
    pub observed_resource_usage: ExternalImportResourceUsage,
}

pub trait ExternalImportScanner: Send + Sync {
    fn scan(&self, request: ExternalImportScanRequest<'_>) -> Result<ExternalImportScanResult>;
}

pub struct ExternalImportMaterializeRequest<'a> {
    /// The adapter resolves this opaque source handle internally; app code never receives a path.
    pub source_id: &'a ExternalImportSourceId,
    pub batch_id: &'a ExternalImportBatchId,
    pub candidate: &'a ExternalImportCandidate,
    pub expected_content_fingerprint: &'a str,
    pub task_id: &'a str,
    pub resource_budget: &'a ExternalImportResourceBudget,
    pub cancellation_token: &'a dyn crate::CancellationToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportMaterializedPackage {
    pub candidate_id: ExternalImportCandidateId,
    /// An opaque internal package identity. It is not a filesystem path.
    pub package_id: String,
    pub content_fingerprint: String,
    pub resource_usage: ExternalImportResourceUsage,
}

pub trait ExternalImportMaterializer: Send + Sync {
    fn materialize(
        &self,
        request: ExternalImportMaterializeRequest<'_>,
    ) -> Result<ExternalImportMaterializedPackage>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportSelectionCompareAndSwapRequest<'a> {
    pub selection: &'a ExternalImportSelection,
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalImportSelectionCompareAndSwapResult {
    Applied(ExternalImportSelection),
    RevisionConflict { current_revision: u64 },
}

pub trait ExternalImportBatchRepository: Send + Sync {
    fn create_batch(&self, batch: &ExternalImportBatch) -> Result<()>;

    fn get_batch(&self, batch_id: &ExternalImportBatchId) -> Result<Option<ExternalImportBatch>>;

    fn replace_candidates(
        &self,
        batch_id: &ExternalImportBatchId,
        candidates: &[ExternalImportCandidate],
    ) -> Result<()>;

    fn list_candidates(
        &self,
        batch_id: &ExternalImportBatchId,
    ) -> Result<Vec<ExternalImportCandidate>>;

    fn create_selection(&self, selection: &ExternalImportSelection) -> Result<()>;

    fn get_selection(
        &self,
        selection_id: &ExternalImportSelectionId,
    ) -> Result<Option<ExternalImportSelection>>;

    /// The repository, not a caller-side read/modify/write sequence, owns the durable CAS check.
    fn compare_and_swap_selection(
        &self,
        request: ExternalImportSelectionCompareAndSwapRequest<'_>,
    ) -> Result<ExternalImportSelectionCompareAndSwapResult>;

    fn append_item_results(
        &self,
        batch_id: &ExternalImportBatchId,
        results: &[ExternalImportItemResult],
    ) -> Result<()>;

    fn list_item_results(
        &self,
        batch_id: &ExternalImportBatchId,
    ) -> Result<Vec<ExternalImportItemResult>>;
}
