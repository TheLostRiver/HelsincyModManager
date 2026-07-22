use anyhow::Result;
use hmm_core::{
    ExternalImportBatch, ExternalImportBatchId, ExternalImportCandidate, ExternalImportCandidateId,
    ExternalImportItemResult, ExternalImportResourceBudget, ExternalImportResourceUsage,
    ExternalImportSelection, ExternalImportSelectionError, ExternalImportSelectionId,
    ExternalImportSource, ExternalImportSourceId,
};

/// A path-free lookup result for an ephemeral external source registration.
///
/// `source_fingerprint` is deliberately retained inside application and infrastructure
/// code. It must never be copied into a DTO, task event, log, or diagnostic package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportSourceRegistration {
    pub source: ExternalImportSource,
    pub source_fingerprint: String,
}

pub trait ExternalImportSourceRegistry: Send + Sync {
    fn resolve_source(
        &self,
        source_id: &ExternalImportSourceId,
    ) -> Result<Option<ExternalImportSourceRegistration>>;

    /// Finds a newly selected ephemeral source that exactly matches a durable, protected batch
    /// fingerprint. Neither a source root nor the fingerprint crosses the Tauri boundary.
    fn resolve_matching_source(
        &self,
        _source_fingerprint: &str,
    ) -> Result<Option<ExternalImportSourceRegistration>> {
        Ok(None)
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportCandidatePage {
    pub candidates: Vec<ExternalImportCandidate>,
    pub total_count: usize,
    pub next_offset: Option<usize>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalImportMaterializationOutcome {
    Materialized(ExternalImportMaterializedPackage),
    /// The source no longer matches the preview fingerprint or safe structure.
    SourceChanged,
}

pub trait ExternalImportMaterializer: Send + Sync {
    fn materialize(
        &self,
        request: ExternalImportMaterializeRequest<'_>,
    ) -> Result<ExternalImportMaterializationOutcome>;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportSealAndStartRequest<'a> {
    pub selection_id: &'a ExternalImportSelectionId,
    pub expected_revision: u64,
    pub now_unix_millis: u64,
    pub resource_budget: &'a ExternalImportResourceBudget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalImportSealAndStartResult {
    Started {
        batch: ExternalImportBatch,
        selection: Box<ExternalImportSelection>,
    },
    RevisionConflict {
        current_revision: u64,
    },
    SelectionRejected {
        error: ExternalImportSelectionError,
    },
    BatchNotStartable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportItemResultPage {
    pub results: Vec<ExternalImportItemResult>,
    pub total_count: usize,
    pub next_offset: Option<usize>,
}

pub trait ExternalImportBatchRepository: Send + Sync {
    fn create_batch(&self, batch: &ExternalImportBatch) -> Result<()>;

    fn get_batch(&self, batch_id: &ExternalImportBatchId) -> Result<Option<ExternalImportBatch>>;

    fn update_batch(&self, batch: &ExternalImportBatch) -> Result<()>;

    /// Persists the terminal scan state and its preview candidates together.
    ///
    /// Implementations must keep this write short and atomic. Directory traversal,
    /// XML parsing, and hashing must have completed before this method is called.
    fn save_scan_result(
        &self,
        batch: &ExternalImportBatch,
        candidates: &[ExternalImportCandidate],
    ) -> Result<()>;

    fn replace_candidates(
        &self,
        batch_id: &ExternalImportBatchId,
        candidates: &[ExternalImportCandidate],
    ) -> Result<()>;

    fn list_candidates(
        &self,
        batch_id: &ExternalImportBatchId,
    ) -> Result<Vec<ExternalImportCandidate>>;

    /// Reads one stable preview page. `offset` is an application-owned cursor value,
    /// never a filesystem path or source identifier.
    fn list_candidates_page(
        &self,
        batch_id: &ExternalImportBatchId,
        offset: usize,
        limit: usize,
    ) -> Result<ExternalImportCandidatePage>;

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

    /// Seals an editing selection and transitions the containing batch to `running` inside one
    /// short repository transaction. File I/O must happen only after this call returns.
    fn seal_selection_and_start(
        &self,
        _request: ExternalImportSealAndStartRequest<'_>,
    ) -> Result<ExternalImportSealAndStartResult> {
        anyhow::bail!("external import sealed start is not supported by this repository")
    }

    /// Restarts a terminal import batch while preserving its already sealed selection fact.
    fn restart_batch(
        &self,
        _batch_id: &ExternalImportBatchId,
    ) -> Result<Option<ExternalImportBatch>> {
        anyhow::bail!("external import batch retry is not supported by this repository")
    }

    /// Marks batches left `running` by a previous process as failed while preserving their sealed
    /// selection and already durable item results. A later explicit retry performs source
    /// re-association and reconciliation; startup must never resume source I/O automatically.
    fn recover_interrupted_batches(&self) -> Result<usize> {
        anyhow::bail!(
            "external import interrupted batch recovery is not supported by this repository"
        )
    }

    fn append_item_results(
        &self,
        batch_id: &ExternalImportBatchId,
        results: &[ExternalImportItemResult],
    ) -> Result<()>;

    fn list_item_results(
        &self,
        batch_id: &ExternalImportBatchId,
    ) -> Result<Vec<ExternalImportItemResult>>;

    /// Reads a bounded result page in stable scanner candidate order.
    fn list_item_results_page(
        &self,
        _batch_id: &ExternalImportBatchId,
        _offset: usize,
        _limit: usize,
    ) -> Result<ExternalImportItemResultPage> {
        anyhow::bail!("external import result paging is not supported by this repository")
    }
}
