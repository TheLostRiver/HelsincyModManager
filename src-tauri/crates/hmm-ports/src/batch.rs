use anyhow::Result;
use hmm_core::{
    BatchAttempt, BatchAttemptStatus, BatchId, BatchItemId, BatchItemResult, GameId, ProfileId,
    SealedBatch,
};
use hmm_core::{BatchPlanFacts, NormalizedBatchPlanRequest};

pub trait BatchPlanFactsProvider: Send + Sync {
    /// Reads the current batch facts without mutating repositories or creating artifacts.
    fn read_batch_plan_facts(&self, request: &NormalizedBatchPlanRequest)
        -> Result<BatchPlanFacts>;
}

pub struct BatchSealRequest<'a> {
    pub sealed_batch: &'a SealedBatch,
    pub initial_attempt: &'a BatchAttempt,
}

pub trait BatchSealRepository: Send + Sync {
    /// Atomically persists the immutable input/plan snapshot and attempt 0.
    ///
    /// Implementations must not leave partial batch state when returning an error.
    fn seal_batch(&self, request: BatchSealRequest<'_>) -> Result<()>;
}

pub struct BatchAttemptAdmissionRequest<'a> {
    pub batch_id: &'a BatchId,
    pub attempt_number: u32,
    pub task_id: &'a str,
    /// SHA-256 verifier of the raw caller-supplied plan token. The raw token never crosses this
    /// port or reaches durable storage.
    pub presented_plan_token_verifier: &'a str,
    pub now_unix_millis: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchAttemptAdmission {
    Admitted(BatchAttempt),
    AlreadyAdmitted(BatchAttempt),
    Rejected,
}

pub struct BatchRetryAttemptRequest<'a> {
    pub batch_id: &'a BatchId,
    pub expected_attempt_number: u32,
    pub retry_attempt: &'a BatchAttempt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchRetryAttemptCreation {
    Created(BatchAttempt),
    Stale,
    Unavailable,
}

/// Durable control-state boundary for one sealed batch. File writes remain in the existing
/// single-item install transaction; this repository only records orchestration facts.
pub trait BatchLifecycleRepository: BatchSealRepository {
    fn load_batch(&self, batch_id: &BatchId) -> Result<Option<SealedBatch>>;

    fn load_attempt(&self, batch_id: &BatchId, attempt_number: u32)
        -> Result<Option<BatchAttempt>>;

    /// Finds a durable execution attempt that cannot be resumed safely after process loss.
    ///
    /// `Sealed` is intentionally excluded because no execution admission has occurred yet.
    fn find_active_attempt_for_scope(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<Option<BatchAttempt>>;

    /// Performs the once-only sealed -> queued compare-and-swap and binds the admitted task id.
    fn admit_attempt(
        &self,
        request: BatchAttemptAdmissionRequest<'_>,
    ) -> Result<BatchAttemptAdmission>;

    fn mark_attempt_running(
        &self,
        batch_id: &BatchId,
        attempt_number: u32,
        now_unix_millis: u128,
    ) -> Result<BatchAttempt>;

    /// Persists a pre-write running intent for one sealed item.
    fn mark_item_running(
        &self,
        batch_id: &BatchId,
        attempt_number: u32,
        item_id: &BatchItemId,
    ) -> Result<()>;

    /// Persists the terminal fact for exactly one item. Implementations must reject item IDs
    /// outside the sealed ordinal mapping and must not overwrite a terminal result.
    fn record_item_result(&self, result: &BatchItemResult) -> Result<()>;

    fn list_item_results(
        &self,
        batch_id: &BatchId,
        attempt_number: u32,
    ) -> Result<Vec<BatchItemResult>>;

    fn finish_attempt(
        &self,
        batch_id: &BatchId,
        attempt_number: u32,
        status: BatchAttemptStatus,
        evidence_health_degraded: bool,
        completed_at_unix_millis: u128,
    ) -> Result<BatchAttempt>;

    /// Creates exactly one next attempt after the expected terminal attempt. The repository owns
    /// the expected-attempt compare-and-swap so concurrent retry clicks cannot replay writes.
    fn create_retry_attempt(
        &self,
        request: BatchRetryAttemptRequest<'_>,
    ) -> Result<BatchRetryAttemptCreation>;
}
