use anyhow::Result;
use hmm_core::{BatchPlan, BatchPlanFacts, NormalizedBatchPlanRequest};

pub trait BatchPlanFactsProvider: Send + Sync {
    /// Reads the current batch facts without mutating repositories or creating artifacts.
    fn read_batch_plan_facts(&self, request: &NormalizedBatchPlanRequest)
        -> Result<BatchPlanFacts>;
}

pub struct BatchSealRequest<'a> {
    pub request: &'a NormalizedBatchPlanRequest,
    pub plan: &'a BatchPlan,
    pub plan_token_verifier: &'a str,
    pub expires_at_unix_millis: u128,
}

pub trait BatchSealRepository: Send + Sync {
    /// Atomically persists the normalized request, plan snapshot, and initial attempt.
    ///
    /// Implementations must not leave partial batch state when returning an error.
    fn seal_batch(&self, request: BatchSealRequest<'_>) -> Result<String>;
}
