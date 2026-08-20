use crate::batch::{execution_token_digest, BatchTokenCodec, BatchTokenError};
use crate::install_task::{
    InstallTaskOrchestrationError, InstallTaskRunner, StartInstallTaskRequest,
};
use crate::task_manager::{TaskManager, TaskProgressObserver, TaskStatus};
use crate::TaskKind;
use hmm_core::{
    BatchAttemptStatus, BatchExecutionPolicy, BatchId, BatchItemResult, BatchItemStatus,
    BatchOperation, BatchPlan, BatchPlanStatus, BatchResultSummary, SealedBatch, SealedBatchItem,
    DEFAULT_BATCH_PREVIEW_TOKEN_TTL_MILLIS,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, AuditWriteFailurePolicy, BatchAttemptAdmission,
    BatchAttemptAdmissionRequest, BatchLifecycleRepository, BatchPlanFactsProvider,
    BatchRetryAttemptCreation, BatchRetryAttemptRequest,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;

const INSTALL_ITEM_COMMIT_PHASE: &str = "install.commit.processing";
const UNINSTALL_ITEM_COMMIT_PHASE: &str = "install.uninstall.processing";
const REINSTALL_ITEM_COMMIT_PHASE: &str = "install.reinstall.commit.processing";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchInstallItemRequest {
    pub batch_id: BatchId,
    pub attempt_number: u32,
    pub item: SealedBatchItem,
    pub plan: BatchPlan,
    pub parent_task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchInstallItemExecution {
    Succeeded {
        evidence_health_degraded: bool,
    },
    Blocked {
        reason_code: String,
    },
    Failed {
        reason_code: String,
        retryable: bool,
        evidence_health_degraded: bool,
    },
    RecoveryRequired {
        reason_code: String,
    },
    Cancelled,
}

pub trait BatchInstallItemExecutor: Send + Sync {
    fn execute(&self, request: BatchInstallItemRequest) -> BatchInstallItemExecution;
}

enum BatchItemFactsCheck {
    Current,
    Stale,
    GlobalBlocked(String),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchInstallRunResult {
    pub task_id: String,
    pub batch_id: BatchId,
    pub attempt_number: u32,
    pub status: BatchAttemptStatus,
    pub summary: BatchResultSummary,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BatchInstallRunError {
    #[error("batch is unavailable")]
    BatchUnavailable,
    #[error("batch plan token is invalid")]
    InvalidToken,
    #[error("batch plan is blocked")]
    PlanBlocked,
    #[error("batch operation is not install")]
    OperationMismatch,
    #[error("batch start admission was rejected")]
    AdmissionRejected,
    #[error("batch scope has an active attempt that requires reconciliation")]
    ScopeReconciliationRequired,
    #[error("batch journal is unavailable")]
    JournalUnavailable,
    #[error("batch task is unavailable")]
    TaskUnavailable,
}

#[derive(Debug, Clone, Copy)]
struct BatchAuditOutcome {
    status: BatchAttemptStatus,
    error_code: Option<&'static str>,
}

impl BatchAuditOutcome {
    fn from_status(status: BatchAttemptStatus) -> Self {
        Self {
            status,
            error_code: None,
        }
    }

    fn failure(error_code: &'static str) -> Self {
        Self {
            status: BatchAttemptStatus::Failed,
            error_code: Some(error_code),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchInstallRetryResult {
    pub batch_id: BatchId,
    pub attempt_number: u32,
    pub plan_token: String,
    pub expires_at_unix_millis: u128,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BatchInstallRetryError {
    #[error("batch is unavailable")]
    BatchUnavailable,
    #[error("batch retry is unavailable")]
    RetryUnavailable,
    #[error("batch attempt is stale")]
    AttemptStale,
    #[error("batch clock is unavailable")]
    ClockUnavailable,
    #[error("batch plan token could not be issued")]
    TokenIssueFailed,
    #[error("batch journal is unavailable")]
    JournalUnavailable,
}

pub struct BatchInstallRetryService {
    repository: Arc<dyn BatchLifecycleRepository>,
    clock: Arc<dyn AppClock>,
    token_codec: Arc<dyn BatchTokenCodec>,
    expected_operation: BatchOperation,
}

impl BatchInstallRetryService {
    pub fn new(
        repository: Arc<dyn BatchLifecycleRepository>,
        clock: Arc<dyn AppClock>,
        token_codec: Arc<dyn BatchTokenCodec>,
    ) -> Self {
        Self::for_operation(BatchOperation::Install, repository, clock, token_codec)
    }

    pub fn for_operation(
        expected_operation: BatchOperation,
        repository: Arc<dyn BatchLifecycleRepository>,
        clock: Arc<dyn AppClock>,
        token_codec: Arc<dyn BatchTokenCodec>,
    ) -> Self {
        Self {
            repository,
            clock,
            token_codec,
            expected_operation,
        }
    }

    pub fn retry(
        &self,
        batch_id: &BatchId,
        expected_attempt_number: u32,
    ) -> Result<BatchInstallRetryResult, BatchInstallRetryError> {
        let batch = self
            .repository
            .load_batch(batch_id)
            .map_err(|_| BatchInstallRetryError::BatchUnavailable)?
            .ok_or(BatchInstallRetryError::BatchUnavailable)?;
        if batch.plan.operation != self.expected_operation {
            return Err(BatchInstallRetryError::RetryUnavailable);
        }
        let previous = self
            .repository
            .load_attempt(batch_id, expected_attempt_number)
            .map_err(|_| BatchInstallRetryError::BatchUnavailable)?
            .ok_or(BatchInstallRetryError::AttemptStale)?;
        if !previous.status.is_terminal() {
            return Err(BatchInstallRetryError::RetryUnavailable);
        }
        if matches!(
            previous.status,
            BatchAttemptStatus::RecoveryRequired | BatchAttemptStatus::Interrupted
        ) || previous.evidence_health_degraded
        {
            return Err(BatchInstallRetryError::RetryUnavailable);
        }
        let results = self
            .repository
            .list_item_results(batch_id, expected_attempt_number)
            .map_err(|_| BatchInstallRetryError::JournalUnavailable)?;
        let retryable_item_ids = batch
            .items
            .iter()
            .filter(|item| {
                results
                    .iter()
                    .any(|result| result.item_id == item.item_id && result.retryable)
            })
            .map(|item| item.item_id.clone())
            .collect::<Vec<_>>();
        if retryable_item_ids.is_empty() {
            return Err(BatchInstallRetryError::RetryUnavailable);
        }
        let attempt_number = expected_attempt_number
            .checked_add(1)
            .ok_or(BatchInstallRetryError::AttemptStale)?;
        let now = self
            .clock
            .now_unix_millis()
            .map_err(|_| BatchInstallRetryError::ClockUnavailable)?;
        let expires_at = now.saturating_add(DEFAULT_BATCH_PREVIEW_TOKEN_TTL_MILLIS);
        let token = self
            .token_codec
            .issue(
                crate::batch::BatchTokenKind::Plan,
                &execution_token_digest(
                    batch_id,
                    attempt_number,
                    &retryable_item_ids,
                    &batch.plan.batch_digest,
                    &batch.plan.environment_digest,
                ),
                &batch.plan.environment_digest,
                now,
                expires_at,
            )
            .map_err(|_| BatchInstallRetryError::TokenIssueFailed)?;
        let retry_attempt = hmm_core::BatchAttempt {
            batch_id: batch_id.clone(),
            attempt_number,
            item_ids: retryable_item_ids,
            status: BatchAttemptStatus::Sealed,
            task_id: None,
            plan_token_verifier: token.verifier,
            expires_at_unix_millis: expires_at,
            started_at_unix_millis: None,
            completed_at_unix_millis: None,
            evidence_health_degraded: false,
        };
        match self
            .repository
            .create_retry_attempt(BatchRetryAttemptRequest {
                batch_id,
                expected_attempt_number,
                retry_attempt: &retry_attempt,
            })
            .map_err(|_| BatchInstallRetryError::JournalUnavailable)?
        {
            BatchRetryAttemptCreation::Created(_) => Ok(BatchInstallRetryResult {
                batch_id: batch_id.clone(),
                attempt_number,
                plan_token: token.token,
                expires_at_unix_millis: expires_at,
            }),
            BatchRetryAttemptCreation::Stale => Err(BatchInstallRetryError::AttemptStale),
            BatchRetryAttemptCreation::Unavailable => Err(BatchInstallRetryError::RetryUnavailable),
        }
    }
}

pub struct BatchInstallTaskRunner {
    task_manager: Arc<TaskManager>,
    repository: Arc<dyn BatchLifecycleRepository>,
    executor: Arc<dyn BatchInstallItemExecutor>,
    facts_provider: Arc<dyn BatchPlanFactsProvider>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
    token_codec: Arc<dyn BatchTokenCodec>,
    expected_operation: BatchOperation,
}

impl BatchInstallTaskRunner {
    pub fn new(
        task_manager: Arc<TaskManager>,
        repository: Arc<dyn BatchLifecycleRepository>,
        executor: Arc<dyn BatchInstallItemExecutor>,
        facts_provider: Arc<dyn BatchPlanFactsProvider>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        token_codec: Arc<dyn BatchTokenCodec>,
    ) -> Self {
        Self::for_operation(
            BatchOperation::Install,
            task_manager,
            repository,
            executor,
            facts_provider,
            audit_log,
            clock,
            token_codec,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn for_operation(
        expected_operation: BatchOperation,
        task_manager: Arc<TaskManager>,
        repository: Arc<dyn BatchLifecycleRepository>,
        executor: Arc<dyn BatchInstallItemExecutor>,
        facts_provider: Arc<dyn BatchPlanFactsProvider>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        token_codec: Arc<dyn BatchTokenCodec>,
    ) -> Self {
        Self {
            task_manager,
            repository,
            executor,
            facts_provider,
            audit_log,
            clock,
            token_codec,
            expected_operation,
        }
    }

    pub fn run(
        &self,
        batch_id: &BatchId,
        plan_token: &str,
    ) -> Result<BatchInstallRunResult, BatchInstallRunError> {
        self.run_attempt(batch_id, 0, plan_token)
    }

    pub fn run_attempt(
        &self,
        batch_id: &BatchId,
        attempt_number: u32,
        plan_token: &str,
    ) -> Result<BatchInstallRunResult, BatchInstallRunError> {
        let batch = self
            .repository
            .load_batch(batch_id)
            .map_err(|_| BatchInstallRunError::BatchUnavailable)?
            .ok_or(BatchInstallRunError::BatchUnavailable)?;
        if batch.plan.operation != self.expected_operation {
            return Err(BatchInstallRunError::OperationMismatch);
        }
        let attempt = self
            .repository
            .load_attempt(batch_id, attempt_number)
            .map_err(|_| BatchInstallRunError::BatchUnavailable)?
            .ok_or(BatchInstallRunError::BatchUnavailable)?;
        let now = self
            .clock
            .now_unix_millis()
            .map_err(|_| BatchInstallRunError::JournalUnavailable)?;
        let token_digest = execution_token_digest(
            batch_id,
            attempt.attempt_number,
            &attempt.item_ids,
            &batch.plan.batch_digest,
            &batch.plan.environment_digest,
        );
        let presented_verifier = sha256_hex(plan_token.as_bytes());
        if attempt.status == BatchAttemptStatus::Sealed {
            match self.token_codec.verify(
                crate::batch::BatchTokenKind::Plan,
                plan_token,
                &token_digest,
                &batch.plan.environment_digest,
                now,
            ) {
                Ok(()) => {}
                Err(
                    BatchTokenError::Expired | BatchTokenError::Invalid | BatchTokenError::Mismatch,
                ) => return Err(BatchInstallRunError::InvalidToken),
            }
        } else if attempt.plan_token_verifier != presented_verifier {
            return Err(BatchInstallRunError::InvalidToken);
        }
        if batch.plan.status() == BatchPlanStatus::Blocked {
            return Err(BatchInstallRunError::PlanBlocked);
        }

        let task = self
            .task_manager
            .create_task(TaskKind::Install)
            .map_err(|_| BatchInstallRunError::TaskUnavailable)?;
        let task_id = task.task_id.clone();
        let admission = self
            .repository
            .admit_attempt(BatchAttemptAdmissionRequest {
                batch_id,
                attempt_number: attempt.attempt_number,
                task_id: &task_id,
                presented_plan_token_verifier: &presented_verifier,
                now_unix_millis: now,
            })
            .map_err(|_| {
                let _ = self.task_manager.fail_task(&task_id);
                BatchInstallRunError::JournalUnavailable
            })?;
        let admitted_attempt = match admission {
            BatchAttemptAdmission::Admitted(attempt) => attempt,
            BatchAttemptAdmission::AlreadyAdmitted(attempt) => {
                let _ = self.task_manager.cancel_task(&task_id);
                let results = self
                    .repository
                    .list_item_results(batch_id, attempt.attempt_number)
                    .map_err(|_| BatchInstallRunError::JournalUnavailable)?;
                return Ok(BatchInstallRunResult {
                    task_id: attempt.task_id.unwrap_or(task_id),
                    batch_id: batch_id.clone(),
                    attempt_number: attempt.attempt_number,
                    status: attempt.status,
                    summary: BatchResultSummary::from_item_results(
                        attempt.item_ids.len(),
                        &results,
                    ),
                });
            }
            BatchAttemptAdmission::ScopeActive => {
                let cleanup = if attempt.attempt_number == 0 {
                    Ok(true)
                } else {
                    self.repository.discard_unadmitted_retry_attempt(
                        batch_id,
                        attempt.attempt_number,
                        &presented_verifier,
                    )
                };
                let _ = self.task_manager.fail_task(&task_id);
                let cleanup_error_code = match cleanup {
                    Ok(true) => return Err(BatchInstallRunError::ScopeReconciliationRequired),
                    Ok(false) => "batch_retry_cleanup_ineligible",
                    Err(_) => "batch_retry_cleanup_failed",
                };
                let _ = self.record_batch_admission_failure_audit(
                    &task_id,
                    &batch,
                    attempt.attempt_number,
                    attempt.item_ids.len(),
                    cleanup_error_code,
                    now,
                );
                return Err(BatchInstallRunError::JournalUnavailable);
            }
            BatchAttemptAdmission::Rejected => {
                let _ = self.task_manager.fail_task(&task_id);
                return Err(BatchInstallRunError::AdmissionRejected);
            }
        };

        self.task_manager.start_task(&task_id).map_err(|_| {
            self.fail_for_journal_error(batch_id, admitted_attempt.attempt_number, &task_id, now)
        })?;
        self.repository
            .mark_attempt_running(batch_id, admitted_attempt.attempt_number, now)
            .map_err(|_| {
                self.fail_for_journal_error(
                    batch_id,
                    admitted_attempt.attempt_number,
                    &task_id,
                    now,
                )
            })?;

        let mut evidence_health_degraded = false;
        let selected_item_count = attempt.item_ids.len();
        let mut results = Vec::with_capacity(selected_item_count);
        let mut stop_after_item = false;
        let mut pre_write_blocked = false;
        let mut any_item_started = false;
        let facts_snapshot = self
            .facts_provider
            .read_batch_plan_facts(&batch.request)
            .ok();
        let initial_plan_blocker = if admitted_attempt.attempt_number == 0 {
            self.find_initial_plan_blocker(&batch, &attempt, facts_snapshot.as_ref())
        } else {
            None
        };
        let preflight_blocker = initial_plan_blocker.or_else(|| {
            if batch.plan.execution_policy == BatchExecutionPolicy::StopOnFailure {
                self.find_stop_policy_blocker(&batch, &attempt, facts_snapshot.as_ref())
            } else {
                None
            }
        });
        for item in &batch.items {
            if !attempt
                .item_ids
                .iter()
                .any(|item_id| item_id == &item.item_id)
            {
                continue;
            }
            if stop_after_item
                || self.task_manager.task_status(&task_id) == Some(TaskStatus::Cancelled)
            {
                let result = skipped_result(batch_id, admitted_attempt.attempt_number, item);
                self.repository.record_item_result(&result).map_err(|_| {
                    self.journal_error(
                        &batch,
                        admitted_attempt.attempt_number,
                        &task_id,
                        selected_item_count,
                        &results,
                        any_item_started,
                        now,
                    )
                })?;
                results.push(result);
                continue;
            }
            if let Some((blocked_item_id, reason_code)) = &preflight_blocker {
                let result = if item.item_id == *blocked_item_id {
                    let result = terminal_result(
                        batch_id,
                        admitted_attempt.attempt_number,
                        item,
                        BatchItemStatus::Blocked,
                        Some(reason_code.clone()),
                        false,
                    );
                    self.repository
                        .mark_item_running(batch_id, admitted_attempt.attempt_number, &item.item_id)
                        .and_then(|_| self.repository.record_item_result(&result))
                        .map_err(|_| {
                            self.journal_error(
                                &batch,
                                admitted_attempt.attempt_number,
                                &task_id,
                                selected_item_count,
                                &results,
                                any_item_started,
                                now,
                            )
                        })?;
                    result
                } else {
                    let result = skipped_result(batch_id, admitted_attempt.attempt_number, item);
                    self.repository.record_item_result(&result).map_err(|_| {
                        self.journal_error(
                            &batch,
                            admitted_attempt.attempt_number,
                            &task_id,
                            selected_item_count,
                            &results,
                            any_item_started,
                            now,
                        )
                    })?;
                    result
                };
                results.push(result);
                pre_write_blocked = true;
                continue;
            }
            let plan_item = batch
                .plan
                .items
                .iter()
                .find(|candidate| candidate.ordinal == item.ordinal)
                .ok_or(BatchInstallRunError::BatchUnavailable)?;

            if !plan_item.is_ready() {
                let result = terminal_result(
                    batch_id,
                    admitted_attempt.attempt_number,
                    item,
                    BatchItemStatus::Blocked,
                    plan_item.blocking_reasons.first().cloned(),
                    false,
                );
                self.repository
                    .mark_item_running(batch_id, admitted_attempt.attempt_number, &item.item_id)
                    .and_then(|_| self.repository.record_item_result(&result))
                    .map_err(|_| {
                        self.journal_error(
                            &batch,
                            admitted_attempt.attempt_number,
                            &task_id,
                            selected_item_count,
                            &results,
                            any_item_started,
                            now,
                        )
                    })?;
                results.push(result);
                pre_write_blocked |= !any_item_started;
                stop_after_item =
                    batch.plan.execution_policy == BatchExecutionPolicy::StopOnFailure;
                continue;
            }

            let facts_check = self.check_item_facts(&batch, plan_item);
            if !matches!(facts_check, BatchItemFactsCheck::Current) {
                let facts_unavailable = matches!(facts_check, BatchItemFactsCheck::Unavailable);
                let global_blocker = match &facts_check {
                    BatchItemFactsCheck::GlobalBlocked(reason_code) => Some(reason_code.clone()),
                    _ => None,
                };
                let result = terminal_result(
                    batch_id,
                    admitted_attempt.attempt_number,
                    item,
                    BatchItemStatus::Blocked,
                    Some(global_blocker.unwrap_or_else(|| {
                        if facts_unavailable {
                            "batch_facts_unavailable"
                        } else {
                            "batch_item_plan_stale"
                        }
                        .to_owned()
                    })),
                    false,
                );
                self.repository
                    .mark_item_running(batch_id, admitted_attempt.attempt_number, &item.item_id)
                    .and_then(|_| self.repository.record_item_result(&result))
                    .map_err(|_| {
                        self.journal_error(
                            &batch,
                            admitted_attempt.attempt_number,
                            &task_id,
                            selected_item_count,
                            &results,
                            any_item_started,
                            now,
                        )
                    })?;
                results.push(result);
                pre_write_blocked |= !any_item_started;
                stop_after_item = facts_unavailable
                    || matches!(facts_check, BatchItemFactsCheck::GlobalBlocked(_))
                    || batch.plan.execution_policy == BatchExecutionPolicy::StopOnFailure;
                continue;
            }

            self.repository
                .mark_item_running(batch_id, admitted_attempt.attempt_number, &item.item_id)
                .map_err(|_| {
                    self.journal_error(
                        &batch,
                        admitted_attempt.attempt_number,
                        &task_id,
                        selected_item_count,
                        &results,
                        any_item_started,
                        now,
                    )
                })?;
            any_item_started = true;
            let execution = self.executor.execute(BatchInstallItemRequest {
                batch_id: batch_id.clone(),
                attempt_number: admitted_attempt.attempt_number,
                item: item.clone(),
                plan: batch.plan.clone(),
                parent_task_id: task_id.clone(),
            });
            let (status, reason_code, retryable, degraded) = map_execution(&execution);
            evidence_health_degraded |= degraded;
            let result = terminal_result(
                batch_id,
                admitted_attempt.attempt_number,
                item,
                status,
                reason_code,
                retryable,
            );
            self.repository.record_item_result(&result).map_err(|_| {
                self.interrupt_for_journal_error(
                    &batch,
                    admitted_attempt.attempt_number,
                    &task_id,
                    selected_item_count,
                    &results,
                    now,
                )
            })?;
            results.push(result);
            let _ = self.task_manager.unblock_task_cancellation(&task_id);
            match self.task_manager.apply_deferred_cancellation(&task_id) {
                Ok(Some(_)) => stop_after_item = true,
                Ok(None) => {}
                Err(_) => {
                    evidence_health_degraded = true;
                    stop_after_item = true;
                }
            }
            if matches!(execution, BatchInstallItemExecution::Cancelled) {
                let _ = self.task_manager.cancel_task(&task_id);
            }
            if self.task_manager.task_status(&task_id) == Some(TaskStatus::Cancelled) {
                stop_after_item = true;
            }
            if degraded {
                stop_after_item = true;
            }
            if matches!(
                execution,
                BatchInstallItemExecution::RecoveryRequired { .. }
            ) {
                stop_after_item = true;
            }
            if batch.plan.execution_policy == BatchExecutionPolicy::StopOnFailure
                && !matches!(execution, BatchInstallItemExecution::Succeeded { .. })
            {
                stop_after_item = true;
            }
        }

        let accepted_cancellation = if self.task_manager.task_status(&task_id)
            == Some(TaskStatus::Cancelled)
        {
            true
        } else {
            match self.task_manager.block_task_cancellation(&task_id) {
                Ok(_) => false,
                Err(_)
                    if self.task_manager.task_status(&task_id) == Some(TaskStatus::Cancelled) =>
                {
                    true
                }
                Err(_) => {
                    return Err(self.journal_error(
                        &batch,
                        admitted_attempt.attempt_number,
                        &task_id,
                        selected_item_count,
                        &results,
                        any_item_started,
                        now,
                    ))
                }
            }
        };
        let final_status = if results
            .iter()
            .any(|result| result.status == BatchItemStatus::RecoveryRequired)
        {
            BatchAttemptStatus::RecoveryRequired
        } else if accepted_cancellation {
            BatchAttemptStatus::Cancelled
        } else if pre_write_blocked && !any_item_started {
            BatchAttemptStatus::Blocked
        } else if results.iter().any(|result| {
            matches!(
                result.status,
                BatchItemStatus::Blocked | BatchItemStatus::Failed | BatchItemStatus::Skipped
            )
        }) {
            BatchAttemptStatus::CompletedWithErrors
        } else {
            BatchAttemptStatus::Completed
        };
        let completed_at = self.clock.now_unix_millis().map_err(|_| {
            self.interrupt_for_journal_error(
                &batch,
                admitted_attempt.attempt_number,
                &task_id,
                selected_item_count,
                &results,
                now,
            )
        })?;
        let summary = BatchResultSummary::from_item_results(selected_item_count, &results);
        if !self.record_batch_audit(
            &task_id,
            &batch,
            admitted_attempt.attempt_number,
            final_status,
            &summary,
            completed_at,
        ) {
            evidence_health_degraded = true;
        }
        let _attempt = self
            .repository
            .finish_attempt(
                batch_id,
                admitted_attempt.attempt_number,
                final_status,
                evidence_health_degraded,
                completed_at,
            )
            .map_err(|_| {
                self.interrupt_for_journal_error(
                    &batch,
                    admitted_attempt.attempt_number,
                    &task_id,
                    selected_item_count,
                    &results,
                    now,
                )
            })?;
        if final_status == BatchAttemptStatus::RecoveryRequired {
            let _ = self.task_manager.fail_task(&task_id);
        } else if final_status == BatchAttemptStatus::Cancelled {
            // TaskManager already contains the accepted cancellation.
        } else {
            self.task_manager
                .complete_task(&task_id)
                .map_err(|_| BatchInstallRunError::TaskUnavailable)?;
        }
        Ok(BatchInstallRunResult {
            task_id,
            batch_id: batch_id.clone(),
            attempt_number: admitted_attempt.attempt_number,
            status: final_status,
            summary,
        })
    }

    fn check_item_facts(
        &self,
        batch: &SealedBatch,
        plan_item: &hmm_core::BatchItemPlan,
    ) -> BatchItemFactsCheck {
        let facts = match self.facts_provider.read_batch_plan_facts(&batch.request) {
            Ok(facts) => facts,
            Err(_) => return BatchItemFactsCheck::Unavailable,
        };
        if let Some(reason_code) = first_global_blocker(&facts) {
            return BatchItemFactsCheck::GlobalBlocked(reason_code);
        }
        if batch_item_facts_are_current(batch, plan_item, &facts) {
            BatchItemFactsCheck::Current
        } else {
            BatchItemFactsCheck::Stale
        }
    }

    fn find_initial_plan_blocker(
        &self,
        batch: &SealedBatch,
        attempt: &hmm_core::BatchAttempt,
        facts: Option<&hmm_core::BatchPlanFacts>,
    ) -> Option<(hmm_core::BatchItemId, String)> {
        let item_id = attempt.item_ids.first()?.clone();
        let Some(facts) = facts else {
            return Some((item_id, "batch_facts_unavailable".to_owned()));
        };
        if let Some(reason_code) = first_global_blocker(facts) {
            return Some((item_id, reason_code));
        }
        if batch_plan_facts_are_current(batch, facts) {
            None
        } else {
            Some((item_id, "batch_plan_stale".to_owned()))
        }
    }

    fn find_stop_policy_blocker(
        &self,
        batch: &SealedBatch,
        attempt: &hmm_core::BatchAttempt,
        facts: Option<&hmm_core::BatchPlanFacts>,
    ) -> Option<(hmm_core::BatchItemId, String)> {
        let Some(facts) = facts else {
            return attempt
                .item_ids
                .first()
                .cloned()
                .map(|item_id| (item_id, "batch_facts_unavailable".to_owned()));
        };
        if let Some(reason_code) = first_global_blocker(facts) {
            return attempt
                .item_ids
                .first()
                .cloned()
                .map(|item_id| (item_id, reason_code));
        }
        for item in &batch.items {
            if !attempt.item_ids.contains(&item.item_id) {
                continue;
            }
            let Some(plan_item) = batch
                .plan
                .items
                .iter()
                .find(|candidate| candidate.ordinal == item.ordinal)
            else {
                return Some((item.item_id.clone(), "batch_plan_invalid".to_owned()));
            };
            if !plan_item.is_ready() {
                return Some((
                    item.item_id.clone(),
                    plan_item
                        .blocking_reasons
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "batch_item_blocked".to_owned()),
                ));
            }
            if !batch_item_facts_are_current(batch, plan_item, facts) {
                return Some((item.item_id.clone(), "batch_item_plan_stale".to_owned()));
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn journal_error(
        &self,
        batch: &SealedBatch,
        attempt_number: u32,
        task_id: &str,
        selected_item_count: usize,
        results: &[BatchItemResult],
        any_item_started: bool,
        fallback_unix_millis: u128,
    ) -> BatchInstallRunError {
        if any_item_started {
            self.interrupt_for_journal_error(
                batch,
                attempt_number,
                task_id,
                selected_item_count,
                results,
                fallback_unix_millis,
            )
        } else {
            self.fail_for_journal_error(
                &batch.batch_id,
                attempt_number,
                task_id,
                fallback_unix_millis,
            )
        }
    }

    fn fail_for_journal_error(
        &self,
        batch_id: &BatchId,
        attempt_number: u32,
        task_id: &str,
        fallback_unix_millis: u128,
    ) -> BatchInstallRunError {
        let now = self
            .clock
            .now_unix_millis()
            .map(|timestamp| timestamp.max(fallback_unix_millis))
            .unwrap_or(fallback_unix_millis);
        let _ = self.repository.finish_attempt(
            batch_id,
            attempt_number,
            BatchAttemptStatus::Failed,
            true,
            now,
        );
        let _ = self.task_manager.unblock_task_cancellation(task_id);
        let _ = self.task_manager.fail_task(task_id);
        BatchInstallRunError::JournalUnavailable
    }

    fn interrupt_for_journal_error(
        &self,
        batch: &SealedBatch,
        attempt_number: u32,
        task_id: &str,
        selected_item_count: usize,
        results: &[BatchItemResult],
        fallback_unix_millis: u128,
    ) -> BatchInstallRunError {
        let now = self
            .clock
            .now_unix_millis()
            .map(|timestamp| timestamp.max(fallback_unix_millis))
            .unwrap_or(fallback_unix_millis);
        let _ = self.repository.finish_attempt(
            &batch.batch_id,
            attempt_number,
            BatchAttemptStatus::Interrupted,
            true,
            now,
        );
        let summary = BatchResultSummary::from_item_results(selected_item_count, results);
        let _ = self.record_batch_audit(
            task_id,
            batch,
            attempt_number,
            BatchAttemptStatus::Interrupted,
            &summary,
            now,
        );
        let _ = self.task_manager.unblock_task_cancellation(task_id);
        let _ = self.task_manager.fail_task(task_id);
        BatchInstallRunError::JournalUnavailable
    }

    fn record_batch_admission_failure_audit(
        &self,
        task_id: &str,
        batch: &SealedBatch,
        attempt_number: u32,
        item_count: usize,
        error_code: &'static str,
        timestamp_unix_millis: u128,
    ) -> bool {
        self.record_batch_audit_with_error(
            task_id,
            batch,
            attempt_number,
            BatchAuditOutcome::failure(error_code),
            &BatchResultSummary::from_item_results(item_count, &[]),
            timestamp_unix_millis,
        )
    }

    fn record_batch_audit(
        &self,
        task_id: &str,
        batch: &SealedBatch,
        attempt_number: u32,
        status: BatchAttemptStatus,
        summary: &BatchResultSummary,
        timestamp_unix_millis: u128,
    ) -> bool {
        self.record_batch_audit_with_error(
            task_id,
            batch,
            attempt_number,
            BatchAuditOutcome::from_status(status),
            summary,
            timestamp_unix_millis,
        )
    }

    fn record_batch_audit_with_error(
        &self,
        task_id: &str,
        batch: &SealedBatch,
        attempt_number: u32,
        outcome: BatchAuditOutcome,
        summary: &BatchResultSummary,
        timestamp_unix_millis: u128,
    ) -> bool {
        let mut fields = BTreeMap::new();
        fields.insert("task_id".to_owned(), short_audit_id(task_id));
        fields.insert(
            "batch_id".to_owned(),
            short_audit_id(batch.batch_id.as_str()),
        );
        fields.insert(
            "execution_policy".to_owned(),
            batch.plan.execution_policy.as_str().to_owned(),
        );
        fields.insert("attempt_number".to_owned(), attempt_number.to_string());
        fields.insert("item_count".to_owned(), summary.item_count.to_string());
        fields.insert(
            "succeeded_count".to_owned(),
            summary.succeeded_count.to_string(),
        );
        fields.insert(
            "blocked_count".to_owned(),
            summary.blocked_count.to_string(),
        );
        fields.insert("failed_count".to_owned(), summary.failed_count.to_string());
        fields.insert(
            "cancelled_count".to_owned(),
            summary.cancelled_count.to_string(),
        );
        fields.insert(
            "skipped_count".to_owned(),
            summary.skipped_count.to_string(),
        );
        fields.insert(
            "recovery_required_count".to_owned(),
            summary.recovery_required_count.to_string(),
        );
        if let Some(error_code) = outcome
            .error_code
            .or_else(|| batch_status_error_code(outcome.status))
        {
            fields.insert("error_code".to_owned(), error_code.to_owned());
        }

        let policy =
            if summary.succeeded_count > 0 || outcome.status == BatchAttemptStatus::Interrupted {
                AuditWriteFailurePolicy::ReportAfterCommit
            } else {
                AuditWriteFailurePolicy::BestEffort
            };
        self.audit_log
            .record_with_policy(
                AuditLogEvent {
                    timestamp_unix_millis,
                    category: "install".to_owned(),
                    operation: format!("batch_{}", batch.plan.operation.as_str()),
                    result: batch_status_result(outcome.status).to_owned(),
                    fields,
                },
                policy,
            )
            .is_ok()
    }
}

fn batch_item_facts_are_current(
    batch: &SealedBatch,
    plan_item: &hmm_core::BatchItemPlan,
    facts: &hmm_core::BatchPlanFacts,
) -> bool {
    if facts.environment_digest != batch.plan.environment_digest
        || facts.prerequisite_rules_version != batch.plan.prerequisite_rules_version
    {
        return false;
    }
    let Some(fact) = facts
        .items
        .iter()
        .find(|fact| fact.mod_id == plan_item.input_snapshot.mod_id().clone())
    else {
        return false;
    };
    fact.source_revision_id == plan_item.source_revision_id
        && fact.installed_revision_id == plan_item.installed_revision_id
        && fact.fact_digest == plan_item.fact_digest
        && fact.single_plan_digest == plan_item.single_plan_digest
        && fact.prerequisite == plan_item.prerequisite
        && fact.target_claims == plan_item.target_claims
        && fact.action_summary == plan_item.action_summary
        && fact.blocking_reasons == plan_item.blocking_reasons
        && fact.warning_codes == plan_item.warning_codes
}

fn batch_plan_facts_are_current(batch: &SealedBatch, facts: &hmm_core::BatchPlanFacts) -> bool {
    let Ok(current_plan) = hmm_core::build_batch_plan(
        batch.request.clone(),
        facts.clone(),
        batch.plan.resource_limits.clone(),
    ) else {
        return false;
    };
    current_plan.environment_digest == batch.plan.environment_digest
        && current_plan.prerequisite_rules_version == batch.plan.prerequisite_rules_version
        && current_plan.global_blocking_reasons == batch.plan.global_blocking_reasons
        && current_plan.warning_codes == batch.plan.warning_codes
        && current_plan.items == batch.plan.items
}

fn first_global_blocker(facts: &hmm_core::BatchPlanFacts) -> Option<String> {
    facts
        .global_blocking_reasons
        .first()
        .map(|reason| reason.code.clone())
}

fn short_audit_id(value: &str) -> String {
    sha256_hex(value.as_bytes())[..12].to_owned()
}

fn batch_status_result(status: BatchAttemptStatus) -> &'static str {
    match status {
        BatchAttemptStatus::Completed => "success",
        BatchAttemptStatus::CompletedWithErrors => "partial_failure",
        BatchAttemptStatus::Blocked => "blocked",
        BatchAttemptStatus::Cancelled => "cancelled",
        BatchAttemptStatus::RecoveryRequired => "recovery_required",
        BatchAttemptStatus::Interrupted => "interrupted",
        BatchAttemptStatus::Failed => "failure",
        BatchAttemptStatus::Sealed
        | BatchAttemptStatus::Queued
        | BatchAttemptStatus::Running
        | BatchAttemptStatus::Stopping => "failure",
    }
}

fn batch_status_error_code(status: BatchAttemptStatus) -> Option<&'static str> {
    match status {
        BatchAttemptStatus::Completed => None,
        BatchAttemptStatus::CompletedWithErrors => Some("batch_items_failed"),
        BatchAttemptStatus::Blocked => Some("batch_blocked"),
        BatchAttemptStatus::Cancelled => Some("batch_cancelled"),
        BatchAttemptStatus::RecoveryRequired => Some("batch_recovery_required"),
        BatchAttemptStatus::Interrupted => Some("batch_journal_interrupted"),
        BatchAttemptStatus::Failed
        | BatchAttemptStatus::Sealed
        | BatchAttemptStatus::Queued
        | BatchAttemptStatus::Running
        | BatchAttemptStatus::Stopping => Some("batch_failed"),
    }
}

fn map_execution(
    execution: &BatchInstallItemExecution,
) -> (BatchItemStatus, Option<String>, bool, bool) {
    match execution {
        BatchInstallItemExecution::Succeeded {
            evidence_health_degraded,
        } => (
            BatchItemStatus::Succeeded,
            None,
            false,
            *evidence_health_degraded,
        ),
        BatchInstallItemExecution::Blocked { reason_code } => (
            BatchItemStatus::Blocked,
            Some(reason_code.clone()),
            false,
            false,
        ),
        BatchInstallItemExecution::Failed {
            reason_code,
            retryable,
            evidence_health_degraded,
        } => (
            BatchItemStatus::Failed,
            Some(reason_code.clone()),
            *retryable,
            *evidence_health_degraded,
        ),
        BatchInstallItemExecution::RecoveryRequired { reason_code } => (
            BatchItemStatus::RecoveryRequired,
            Some(reason_code.clone()),
            false,
            false,
        ),
        BatchInstallItemExecution::Cancelled => (
            BatchItemStatus::Cancelled,
            Some("cancelled".to_owned()),
            true,
            false,
        ),
    }
}

fn terminal_result(
    batch_id: &BatchId,
    attempt_number: u32,
    item: &SealedBatchItem,
    status: BatchItemStatus,
    reason_code: Option<String>,
    retryable: bool,
) -> BatchItemResult {
    BatchItemResult {
        batch_id: batch_id.clone(),
        attempt_number,
        item_id: item.item_id.clone(),
        ordinal: item.ordinal,
        mod_id: item.mod_id.clone(),
        status,
        reason_code,
        retryable,
    }
}

fn skipped_result(
    batch_id: &BatchId,
    attempt_number: u32,
    item: &SealedBatchItem,
) -> BatchItemResult {
    terminal_result(
        batch_id,
        attempt_number,
        item,
        BatchItemStatus::Skipped,
        Some("batch_stopped".to_owned()),
        true,
    )
}

/// Adapter retaining the existing single-item plan/backup/commit/recovery chain. Child task
/// events are intentionally not forwarded; the batch runner exposes only aggregate state.
pub struct InstallTaskBatchItemExecutor {
    runner: Arc<InstallTaskRunner>,
    task_manager: Arc<TaskManager>,
}

impl InstallTaskBatchItemExecutor {
    pub fn new(runner: Arc<InstallTaskRunner>, task_manager: Arc<TaskManager>) -> Self {
        Self {
            runner,
            task_manager,
        }
    }
}

impl BatchInstallItemExecutor for InstallTaskBatchItemExecutor {
    fn execute(&self, request: BatchInstallItemRequest) -> BatchInstallItemExecution {
        let input = match request
            .plan
            .items
            .iter()
            .find(|item| item.ordinal == request.item.ordinal)
            .map(|item| &item.input_snapshot)
        {
            Some(hmm_core::BatchItemInput::Install(input)) => input,
            _ => {
                return BatchInstallItemExecution::Blocked {
                    reason_code: "batch_operation_not_install".to_owned(),
                };
            }
        };
        if input
            .replacement_binding_snapshot
            .as_ref()
            .is_some_and(|binding| !crate::is_identity_replacement_binding(binding))
        {
            return BatchInstallItemExecution::Blocked {
                reason_code: "batch_retarget_install_unsupported".to_owned(),
            };
        }
        let child = match self.task_manager.create_task(TaskKind::Install) {
            Ok(task) => task,
            Err(_) => {
                return BatchInstallItemExecution::Failed {
                    reason_code: "task_unavailable".to_owned(),
                    retryable: true,
                    evidence_health_degraded: true,
                }
            }
        };
        let observer = ParentTaskObserver {
            task_manager: Arc::clone(&self.task_manager),
            parent_task_id: request.parent_task_id,
            child_task_id: child.task_id.clone(),
        };
        let result = self
            .runner
            .run_install_revision_task_for_orchestration_with_observer(
                &child.task_id,
                StartInstallTaskRequest {
                    game_id: request.plan.game_id.clone(),
                    mod_id: input.mod_id.clone(),
                    profile_id: request.plan.profile_id.clone(),
                    layer: input.layer.clone(),
                },
                input.revision_id.clone(),
                input.replacement_binding_snapshot.clone(),
                &observer,
            );
        match result {
            Ok(ref events)
                if self.task_manager.task_status(&child.task_id) == Some(TaskStatus::Completed) =>
            {
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: events_contain_audit_degradation(events),
                }
            }
            Ok(_) => BatchInstallItemExecution::Cancelled,
            Err(_)
                if self.task_manager.task_status(&child.task_id) == Some(TaskStatus::Cancelled) =>
            {
                BatchInstallItemExecution::Cancelled
            }
            Err(error) => classify_install_task_failure(&error),
        }
    }
}

fn classify_install_task_failure(
    error: &InstallTaskOrchestrationError,
) -> BatchInstallItemExecution {
    if let Some(commit_error) = &error.commit_error {
        return match commit_error {
            crate::InstallCommitError::RollbackFailed { .. } => {
                BatchInstallItemExecution::RecoveryRequired {
                    reason_code: "install_rollback_failed".to_owned(),
                }
            }
            // 单独给码：批量安装里这是唯一一个"关掉游戏再重试即可"的阻断原因，
            // 混进 install_plan_blocked 会让用户去查计划冲突。
            crate::InstallCommitError::GameRunning
            | crate::InstallCommitError::GameRunningUnknown => BatchInstallItemExecution::Blocked {
                reason_code: "install_blocked_game_running".to_owned(),
            },
            crate::InstallCommitError::PlanHasBlockingConflicts
            | crate::InstallCommitError::PlanHasInvalidReplacementBindings
            | crate::InstallCommitError::PlanHasInvalidRevisionIdentity => {
                BatchInstallItemExecution::Blocked {
                    reason_code: "install_plan_blocked".to_owned(),
                }
            }
            crate::InstallCommitError::RollbackSucceeded { .. } => {
                BatchInstallItemExecution::Failed {
                    reason_code: "install_rollback_succeeded".to_owned(),
                    retryable: true,
                    evidence_health_degraded: false,
                }
            }
            crate::InstallCommitError::Failed { .. } => BatchInstallItemExecution::Failed {
                reason_code: "install_commit_failed".to_owned(),
                retryable: true,
                evidence_health_degraded: false,
            },
        };
    }

    let reason = error
        .events
        .last()
        .and_then(|event| event.error.as_deref())
        .unwrap_or("install_failed");
    if reason.ends_with(":prerequisite") {
        BatchInstallItemExecution::Blocked {
            reason_code: "prerequisite_blocked".to_owned(),
        }
    } else {
        BatchInstallItemExecution::Failed {
            reason_code: "install_failed".to_owned(),
            retryable: true,
            evidence_health_degraded: false,
        }
    }
}

pub(crate) fn events_contain_audit_degradation(events: &[crate::TaskProgressEvent]) -> bool {
    events
        .iter()
        .any(|event| event.error.as_deref() == Some("install_audit_unavailable"))
}

pub(crate) struct ParentTaskObserver {
    task_manager: Arc<TaskManager>,
    parent_task_id: String,
    child_task_id: String,
}

impl ParentTaskObserver {
    pub(crate) fn new(
        task_manager: Arc<TaskManager>,
        parent_task_id: String,
        child_task_id: String,
    ) -> Self {
        Self {
            task_manager,
            parent_task_id,
            child_task_id,
        }
    }
}

impl TaskProgressObserver for ParentTaskObserver {
    type Error = ();

    fn observe(&self, event: &crate::TaskProgressEvent) -> Result<(), Self::Error> {
        if self.task_manager.task_status(&self.parent_task_id) == Some(TaskStatus::Cancelled) {
            let _ = self.task_manager.cancel_task(&self.child_task_id);
        }
        if matches!(
            event.phase.as_str(),
            INSTALL_ITEM_COMMIT_PHASE | UNINSTALL_ITEM_COMMIT_PHASE | REINSTALL_ITEM_COMMIT_PHASE
        ) && self
            .task_manager
            .block_tasks_cancellation(&[&self.parent_task_id, &self.child_task_id])
            .is_err()
        {
            let _ = self.task_manager.cancel_task(&self.child_task_id);
        }
        Ok(())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
#[path = "batch_install_tests.rs"]
mod tests;
