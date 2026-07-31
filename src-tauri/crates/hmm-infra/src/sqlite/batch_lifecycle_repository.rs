use anyhow::{anyhow, ensure, Context, Result};
use hmm_core::{
    BatchAttempt, BatchAttemptStatus, BatchId, BatchItemId, BatchItemResult, BatchItemStatus,
    BatchResultSummary, SealedBatch,
};
use hmm_ports::{
    BatchAttemptAdmission, BatchAttemptAdmissionRequest, BatchLifecycleRepository,
    BatchRetryAttemptCreation, BatchRetryAttemptRequest, BatchSealRequest,
};
use rusqlite::{Connection, OptionalExtension, Transaction};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

pub struct SqliteBatchLifecycleRepository {
    db: Arc<Mutex<Connection>>,
}

impl SqliteBatchLifecycleRepository {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| anyhow!("batch lifecycle database lock poisoned"))
    }
}

impl hmm_ports::BatchSealRepository for SqliteBatchLifecycleRepository {
    fn seal_batch(&self, request: BatchSealRequest<'_>) -> Result<()> {
        validate_sealed_batch(request.sealed_batch)?;
        validate_attempt_identity(request.initial_attempt, &request.sealed_batch.batch_id, 0)?;
        ensure!(
            request.initial_attempt.status == BatchAttemptStatus::Sealed
                && request.initial_attempt.task_id.is_none()
                && request.initial_attempt.started_at_unix_millis.is_none()
                && request.initial_attempt.completed_at_unix_millis.is_none()
                && !request.initial_attempt.evidence_health_degraded
                && !request.initial_attempt.plan_token_verifier.is_empty(),
            "batch seal request identity is inconsistent"
        );
        ensure!(
            request.initial_attempt.item_ids
                == request
                    .sealed_batch
                    .items
                    .iter()
                    .map(|item| item.item_id.clone())
                    .collect::<Vec<_>>(),
            "initial batch attempt item set is inconsistent"
        );
        let batch_json = serialize(&request.sealed_batch, "sealed batch")?;
        let attempt_json = serialize(&request.initial_attempt, "batch attempt")?;
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin batch seal transaction")?;
        transaction
            .execute(
                "INSERT INTO hmm_batch_lifecycle_batches
                    (batch_id, sealed_json, created_at_unix_millis)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![
                    request.sealed_batch.batch_id.as_str(),
                    batch_json,
                    i64::try_from(request.sealed_batch.created_at_unix_millis)
                        .context("batch creation timestamp is too large")?,
                ],
            )
            .context("failed to insert sealed batch")?;
        transaction
            .execute(
                "INSERT INTO hmm_batch_lifecycle_attempts
                    (batch_id, attempt_number, attempt_json)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![
                    request.initial_attempt.batch_id.as_str(),
                    i64::from(request.initial_attempt.attempt_number),
                    attempt_json,
                ],
            )
            .context("failed to insert initial batch attempt")?;
        transaction
            .commit()
            .context("failed to commit batch seal transaction")?;
        Ok(())
    }
}

impl BatchLifecycleRepository for SqliteBatchLifecycleRepository {
    fn load_batch(&self, batch_id: &BatchId) -> Result<Option<SealedBatch>> {
        let conn = self.lock_db()?;
        let value: Option<String> = conn
            .query_row(
                "SELECT sealed_json
                 FROM hmm_batch_lifecycle_batches
                 WHERE batch_id = ?1",
                rusqlite::params![batch_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .context("failed to load sealed batch")?;
        let batch = value
            .as_deref()
            .map(|json| deserialize(json, "sealed batch"))
            .transpose()?;
        if let Some(batch) = &batch {
            validate_sealed_batch(batch)?;
            ensure!(
                batch.batch_id == *batch_id,
                "sealed batch row identity mismatch"
            );
        }
        Ok(batch)
    }

    fn load_attempt(
        &self,
        batch_id: &BatchId,
        attempt_number: u32,
    ) -> Result<Option<BatchAttempt>> {
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin batch attempt read transaction")?;
        let attempt = load_attempt_from_transaction(&transaction, batch_id, attempt_number)?;
        if let Some(attempt) = &attempt {
            let batch = load_batch_from_transaction(&transaction, batch_id)?
                .ok_or_else(|| anyhow!("sealed batch is unavailable"))?;
            validate_attempt_selection_chain(&transaction, &batch, attempt)?;
        }
        transaction
            .commit()
            .context("failed to finish batch attempt read transaction")?;
        Ok(attempt)
    }

    fn admit_attempt(
        &self,
        request: BatchAttemptAdmissionRequest<'_>,
    ) -> Result<BatchAttemptAdmission> {
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin batch admission transaction")?;
        let Some(mut attempt) =
            load_attempt_from_transaction(&transaction, request.batch_id, request.attempt_number)?
        else {
            return Ok(BatchAttemptAdmission::Rejected);
        };
        let batch = load_batch_from_transaction(&transaction, request.batch_id)?
            .ok_or_else(|| anyhow!("sealed batch is unavailable"))?;
        validate_attempt_selection_chain(&transaction, &batch, &attempt)?;

        if attempt.plan_token_verifier != request.presented_plan_token_verifier {
            return Ok(BatchAttemptAdmission::Rejected);
        }

        if attempt.status != BatchAttemptStatus::Sealed {
            transaction
                .commit()
                .context("failed to finish idempotent batch admission")?;
            return Ok(BatchAttemptAdmission::AlreadyAdmitted(attempt));
        }
        if request.now_unix_millis >= attempt.expires_at_unix_millis {
            return Ok(BatchAttemptAdmission::Rejected);
        }

        attempt.status = BatchAttemptStatus::Queued;
        attempt.task_id = Some(request.task_id.to_owned());
        update_attempt(&transaction, &attempt)?;
        transaction
            .commit()
            .context("failed to commit batch admission")?;
        Ok(BatchAttemptAdmission::Admitted(attempt))
    }

    fn mark_attempt_running(
        &self,
        batch_id: &BatchId,
        attempt_number: u32,
        now_unix_millis: u128,
    ) -> Result<BatchAttempt> {
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin batch running transition")?;
        let mut attempt = load_attempt_from_transaction(&transaction, batch_id, attempt_number)?
            .ok_or_else(|| anyhow!("batch attempt is unavailable"))?;
        let batch = load_batch_from_transaction(&transaction, batch_id)?
            .ok_or_else(|| anyhow!("sealed batch is unavailable"))?;
        validate_attempt_selection_chain(&transaction, &batch, &attempt)?;
        match attempt.status {
            BatchAttemptStatus::Queued => {
                attempt.status = BatchAttemptStatus::Running;
                attempt.started_at_unix_millis = Some(now_unix_millis);
                update_attempt(&transaction, &attempt)?;
            }
            BatchAttemptStatus::Running | BatchAttemptStatus::Stopping => {}
            _ => anyhow::bail!("batch attempt cannot become running"),
        }
        transaction
            .commit()
            .context("failed to commit batch running transition")?;
        Ok(attempt)
    }

    fn mark_item_running(
        &self,
        batch_id: &BatchId,
        attempt_number: u32,
        item_id: &BatchItemId,
    ) -> Result<()> {
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin batch item intent transaction")?;
        let attempt = load_attempt_from_transaction(&transaction, batch_id, attempt_number)?
            .ok_or_else(|| anyhow!("batch attempt is unavailable"))?;
        ensure!(
            matches!(
                attempt.status,
                BatchAttemptStatus::Running | BatchAttemptStatus::Stopping
            ),
            "batch attempt is not running"
        );
        ensure!(
            attempt.item_ids.contains(item_id),
            "batch item is outside the attempt selection"
        );
        let batch = load_batch_from_transaction(&transaction, batch_id)?
            .ok_or_else(|| anyhow!("sealed batch is unavailable"))?;
        validate_attempt_selection_chain(&transaction, &batch, &attempt)?;
        let item = batch
            .items
            .iter()
            .find(|item| item.item_id == *item_id)
            .ok_or_else(|| anyhow!("batch item is unavailable"))?;
        let result = BatchItemResult {
            batch_id: batch_id.clone(),
            attempt_number,
            item_id: item.item_id.clone(),
            ordinal: item.ordinal,
            mod_id: item.mod_id.clone(),
            status: BatchItemStatus::Running,
            reason_code: None,
            retryable: false,
        };
        let current: Option<String> = transaction
            .query_row(
                "SELECT result_json
                 FROM hmm_batch_lifecycle_item_results
                 WHERE batch_id = ?1 AND attempt_number = ?2 AND item_id = ?3",
                rusqlite::params![
                    batch_id.as_str(),
                    i64::from(attempt_number),
                    item.item_id.as_str(),
                ],
                |row| row.get(0),
            )
            .optional()
            .context("failed to read current batch item intent")?;
        if let Some(current) = current {
            let current: BatchItemResult = deserialize(&current, "batch item result")?;
            validate_item_result_row(
                &current,
                batch_id,
                attempt_number,
                i64::try_from(item.ordinal).context("batch item ordinal is too large")?,
            )?;
            ensure!(
                current.item_id == item.item_id && current.mod_id == item.mod_id,
                "running batch item identity does not match sealed item"
            );
            ensure!(
                !current.status.is_terminal(),
                "terminal batch item result cannot return to running"
            );
            transaction
                .commit()
                .context("failed to finish idempotent batch item intent")?;
            return Ok(());
        }
        let result_json = serialize(&result, "running batch item")?;
        transaction
            .execute(
                "INSERT INTO hmm_batch_lifecycle_item_results
                    (batch_id, attempt_number, item_id, ordinal, result_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    batch_id.as_str(),
                    i64::from(attempt_number),
                    item.item_id.as_str(),
                    i64::try_from(item.ordinal).context("batch item ordinal is too large")?,
                    result_json,
                ],
            )
            .context("failed to persist running batch item")?;
        transaction
            .commit()
            .context("failed to commit batch item intent")?;
        Ok(())
    }

    fn record_item_result(&self, result: &BatchItemResult) -> Result<()> {
        validate_terminal_item_result(result)?;
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin batch item result transaction")?;
        let batch = load_batch_from_transaction(&transaction, &result.batch_id)?
            .ok_or_else(|| anyhow!("sealed batch is unavailable"))?;
        let item = batch
            .items
            .iter()
            .find(|item| item.item_id == result.item_id)
            .ok_or_else(|| anyhow!("batch item is unavailable"))?;
        let attempt =
            load_attempt_from_transaction(&transaction, &result.batch_id, result.attempt_number)?
                .ok_or_else(|| anyhow!("batch attempt is unavailable"))?;
        validate_attempt_selection_chain(&transaction, &batch, &attempt)?;
        ensure!(
            attempt.item_ids.contains(&result.item_id),
            "batch item is outside the attempt selection"
        );
        ensure!(
            matches!(
                attempt.status,
                BatchAttemptStatus::Running | BatchAttemptStatus::Stopping
            ),
            "batch attempt is not running"
        );
        ensure!(
            item.ordinal == result.ordinal && item.mod_id == result.mod_id,
            "batch item result identity does not match sealed item"
        );
        let current: Option<String> = transaction
            .query_row(
                "SELECT result_json
                 FROM hmm_batch_lifecycle_item_results
                 WHERE batch_id = ?1 AND attempt_number = ?2 AND item_id = ?3",
                rusqlite::params![
                    result.batch_id.as_str(),
                    i64::from(result.attempt_number),
                    result.item_id.as_str(),
                ],
                |row| row.get(0),
            )
            .optional()
            .context("failed to read current batch item result")?;
        if let Some(current) = current {
            let current: BatchItemResult = deserialize(&current, "batch item result")?;
            validate_item_result_row(
                &current,
                &result.batch_id,
                result.attempt_number,
                i64::try_from(item.ordinal).context("batch item ordinal is too large")?,
            )?;
            ensure!(
                current.item_id == item.item_id && current.mod_id == item.mod_id,
                "running batch item identity does not match sealed item"
            );
            ensure!(
                !current.status.is_terminal(),
                "terminal batch item result cannot be overwritten"
            );
        } else {
            ensure!(
                result.status == BatchItemStatus::Skipped,
                "batch item terminal result requires a running intent"
            );
        }
        let result_json = serialize(result, "batch item result")?;
        transaction
            .execute(
                "INSERT INTO hmm_batch_lifecycle_item_results
                    (batch_id, attempt_number, item_id, ordinal, result_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(batch_id, attempt_number, item_id) DO UPDATE SET
                    ordinal = excluded.ordinal,
                    result_json = excluded.result_json",
                rusqlite::params![
                    result.batch_id.as_str(),
                    i64::from(result.attempt_number),
                    result.item_id.as_str(),
                    i64::try_from(result.ordinal).context("batch item ordinal is too large")?,
                    result_json,
                ],
            )
            .context("failed to persist batch item result")?;
        transaction
            .commit()
            .context("failed to commit batch item result")?;
        Ok(())
    }

    fn list_item_results(
        &self,
        batch_id: &BatchId,
        attempt_number: u32,
    ) -> Result<Vec<BatchItemResult>> {
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin batch item result read transaction")?;
        let batch = load_batch_from_transaction(&transaction, batch_id)?
            .ok_or_else(|| anyhow!("sealed batch is unavailable"))?;
        let attempt = load_attempt_from_transaction(&transaction, batch_id, attempt_number)?
            .ok_or_else(|| anyhow!("batch attempt is unavailable"))?;
        let results = list_item_results_from_transaction(&transaction, batch_id, attempt_number)?;
        validate_attempt_selection_chain(&transaction, &batch, &attempt)?;
        validate_results_against_batch_attempt(&batch, &attempt, &results)?;
        transaction
            .commit()
            .context("failed to finish batch item result read transaction")?;
        Ok(results)
    }

    fn finish_attempt(
        &self,
        batch_id: &BatchId,
        attempt_number: u32,
        status: BatchAttemptStatus,
        evidence_health_degraded: bool,
        completed_at_unix_millis: u128,
    ) -> Result<BatchAttempt> {
        ensure!(status.is_terminal(), "batch attempt status is not terminal");
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin batch finish transaction")?;
        let mut attempt = load_attempt_from_transaction(&transaction, batch_id, attempt_number)?
            .ok_or_else(|| anyhow!("batch attempt is unavailable"))?;
        ensure!(
            !attempt.status.is_terminal(),
            "batch attempt is already terminal"
        );
        ensure!(
            matches!(
                attempt.status,
                BatchAttemptStatus::Queued
                    | BatchAttemptStatus::Running
                    | BatchAttemptStatus::Stopping
            ),
            "batch attempt cannot become terminal from its current state"
        );
        let results = list_item_results_from_transaction(&transaction, batch_id, attempt_number)?;
        let batch = load_batch_from_transaction(&transaction, batch_id)?
            .ok_or_else(|| anyhow!("sealed batch is unavailable"))?;
        validate_attempt_selection_chain(&transaction, &batch, &attempt)?;
        validate_results_against_batch_attempt(&batch, &attempt, &results)?;
        validate_attempt_completion(&attempt, status, evidence_health_degraded, &results)?;
        attempt.status = status;
        attempt.evidence_health_degraded = evidence_health_degraded;
        attempt.completed_at_unix_millis = Some(completed_at_unix_millis);
        update_attempt(&transaction, &attempt)?;
        transaction
            .commit()
            .context("failed to commit batch finish")?;
        Ok(attempt)
    }

    fn create_retry_attempt(
        &self,
        request: BatchRetryAttemptRequest<'_>,
    ) -> Result<BatchRetryAttemptCreation> {
        let retry_attempt = request.retry_attempt;
        let expected_retry_number = request
            .expected_attempt_number
            .checked_add(1)
            .ok_or_else(|| anyhow!("batch attempt number overflow"))?;
        validate_attempt_identity(retry_attempt, request.batch_id, expected_retry_number)?;
        ensure!(
            retry_attempt.status == BatchAttemptStatus::Sealed
                && retry_attempt.task_id.is_none()
                && retry_attempt.started_at_unix_millis.is_none()
                && retry_attempt.completed_at_unix_millis.is_none()
                && !retry_attempt.evidence_health_degraded
                && !retry_attempt.plan_token_verifier.is_empty()
                && !retry_attempt.item_ids.is_empty(),
            "batch retry attempt is invalid"
        );
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin batch retry transaction")?;
        let latest: Option<i64> = transaction
            .query_row(
                "SELECT MAX(attempt_number)
                 FROM hmm_batch_lifecycle_attempts
                 WHERE batch_id = ?1",
                rusqlite::params![request.batch_id.as_str()],
                |row| row.get(0),
            )
            .context("failed to read latest batch attempt")?;
        if latest != Some(i64::from(request.expected_attempt_number)) {
            return Ok(BatchRetryAttemptCreation::Stale);
        }
        let Some(previous_attempt) = load_attempt_from_transaction(
            &transaction,
            request.batch_id,
            request.expected_attempt_number,
        )?
        else {
            return Ok(BatchRetryAttemptCreation::Unavailable);
        };
        if !previous_attempt.status.is_terminal() {
            return Ok(BatchRetryAttemptCreation::Stale);
        }
        if matches!(
            previous_attempt.status,
            BatchAttemptStatus::RecoveryRequired | BatchAttemptStatus::Interrupted
        ) || previous_attempt.evidence_health_degraded
        {
            return Ok(BatchRetryAttemptCreation::Unavailable);
        }
        let batch = load_batch_from_transaction(&transaction, request.batch_id)?
            .ok_or_else(|| anyhow!("sealed batch is unavailable"))?;
        validate_attempt_selection_chain(&transaction, &batch, &previous_attempt)?;
        let selected = retry_attempt
            .item_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        ensure!(
            selected.len() == retry_attempt.item_ids.len()
                && selected
                    .iter()
                    .all(|item_id| batch.items.iter().any(|item| item.item_id == *item_id)),
            "batch retry item set is invalid"
        );
        let results = list_item_results_from_transaction(
            &transaction,
            request.batch_id,
            request.expected_attempt_number,
        )?;
        validate_results_against_batch_attempt(&batch, &previous_attempt, &results)?;
        validate_attempt_completion(
            &previous_attempt,
            previous_attempt.status,
            previous_attempt.evidence_health_degraded,
            &results,
        )?;
        let retryable = results
            .into_iter()
            .filter(|result| result.retryable)
            .map(|result| result.item_id)
            .collect::<Vec<_>>();
        ensure!(
            retry_attempt.item_ids == retryable,
            "batch retry item set does not match terminal results"
        );
        let attempt_json = serialize(retry_attempt, "retry batch attempt")?;
        transaction
            .execute(
                "INSERT INTO hmm_batch_lifecycle_attempts
                    (batch_id, attempt_number, attempt_json)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![
                    request.batch_id.as_str(),
                    i64::from(retry_attempt.attempt_number),
                    attempt_json,
                ],
            )
            .context("failed to insert retry batch attempt")?;
        transaction
            .commit()
            .context("failed to commit batch retry attempt")?;
        Ok(BatchRetryAttemptCreation::Created(retry_attempt.clone()))
    }
}

fn update_attempt(transaction: &Transaction<'_>, attempt: &BatchAttempt) -> Result<()> {
    validate_attempt_state(attempt)?;
    let attempt_json = serialize(attempt, "batch attempt")?;
    let changed = transaction
        .execute(
            "UPDATE hmm_batch_lifecycle_attempts
             SET attempt_json = ?3
             WHERE batch_id = ?1 AND attempt_number = ?2",
            rusqlite::params![
                attempt.batch_id.as_str(),
                i64::from(attempt.attempt_number),
                attempt_json,
            ],
        )
        .context("failed to update batch attempt")?;
    ensure!(changed == 1, "batch attempt disappeared");
    Ok(())
}

fn load_batch_from_transaction(
    transaction: &Transaction<'_>,
    batch_id: &BatchId,
) -> Result<Option<SealedBatch>> {
    let value: Option<String> = transaction
        .query_row(
            "SELECT sealed_json
             FROM hmm_batch_lifecycle_batches
             WHERE batch_id = ?1",
            rusqlite::params![batch_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .context("failed to load sealed batch in transaction")?;
    let batch = value
        .as_deref()
        .map(|json| deserialize(json, "sealed batch"))
        .transpose()?;
    if let Some(batch) = &batch {
        validate_sealed_batch(batch)?;
        ensure!(
            batch.batch_id == *batch_id,
            "sealed batch row identity mismatch"
        );
    }
    Ok(batch)
}

fn load_attempt_from_transaction(
    transaction: &Transaction<'_>,
    batch_id: &BatchId,
    attempt_number: u32,
) -> Result<Option<BatchAttempt>> {
    let value: Option<String> = transaction
        .query_row(
            "SELECT attempt_json
             FROM hmm_batch_lifecycle_attempts
             WHERE batch_id = ?1 AND attempt_number = ?2",
            rusqlite::params![batch_id.as_str(), i64::from(attempt_number)],
            |row| row.get(0),
        )
        .optional()
        .context("failed to load batch attempt in transaction")?;
    let attempt = value
        .as_deref()
        .map(|json| deserialize(json, "batch attempt"))
        .transpose()?;
    if let Some(attempt) = &attempt {
        validate_attempt_identity(attempt, batch_id, attempt_number)?;
    }
    Ok(attempt)
}

fn list_item_results_from_transaction(
    transaction: &Transaction<'_>,
    batch_id: &BatchId,
    attempt_number: u32,
) -> Result<Vec<BatchItemResult>> {
    let mut statement = transaction
        .prepare(
            "SELECT ordinal, result_json
             FROM hmm_batch_lifecycle_item_results
             WHERE batch_id = ?1 AND attempt_number = ?2
             ORDER BY ordinal ASC",
        )
        .context("failed to prepare batch item results in transaction")?;
    let rows = statement
        .query_map(
            rusqlite::params![batch_id.as_str(), i64::from(attempt_number)],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .context("failed to query batch item results in transaction")?;
    rows.map(|row| {
        let (ordinal, json) = row.context("failed to read batch item result")?;
        let result: BatchItemResult = deserialize(&json, "batch item result")?;
        validate_item_result_row(&result, batch_id, attempt_number, ordinal)?;
        Ok(result)
    })
    .collect()
}

fn validate_sealed_batch(batch: &SealedBatch) -> Result<()> {
    ensure!(!batch.batch_id.as_str().is_empty(), "batch id is empty");
    batch
        .request
        .validate_integrity(batch.plan.resource_limits.max_items)
        .context("sealed batch request integrity check failed")?;
    batch
        .plan
        .validate_integrity()
        .context("sealed batch plan integrity check failed")?;
    ensure!(
        batch.items.len() == batch.plan.items.len()
            && batch.items.len() == batch.request.items.len(),
        "sealed batch item mapping is incomplete"
    );
    ensure!(
        batch.request.schema_version == batch.plan.plan_schema_version
            && batch.request.operation == batch.plan.operation
            && batch.request.game_id == batch.plan.game_id
            && batch.request.profile_id == batch.plan.profile_id
            && batch.request.execution_policy == batch.plan.execution_policy,
        "sealed batch plan identity is inconsistent"
    );
    let mut item_ids = BTreeSet::new();
    for (expected_ordinal, ((sealed, planned), requested)) in batch
        .items
        .iter()
        .zip(&batch.plan.items)
        .zip(&batch.request.items)
        .enumerate()
    {
        ensure!(
            !sealed.item_id.as_str().is_empty()
                && item_ids.insert(sealed.item_id.clone())
                && sealed.ordinal == expected_ordinal
                && sealed.ordinal == planned.ordinal
                && sealed.mod_id == *planned.input_snapshot.mod_id()
                && planned.input_snapshot == *requested,
            "sealed batch item mapping is inconsistent"
        );
    }
    Ok(())
}

fn validate_attempt_identity(
    attempt: &BatchAttempt,
    batch_id: &BatchId,
    attempt_number: u32,
) -> Result<()> {
    ensure!(
        attempt.batch_id == *batch_id && attempt.attempt_number == attempt_number,
        "batch attempt row identity mismatch"
    );
    let selected = attempt.item_ids.iter().cloned().collect::<BTreeSet<_>>();
    ensure!(
        !attempt.item_ids.is_empty()
            && selected.len() == attempt.item_ids.len()
            && attempt
                .item_ids
                .iter()
                .all(|item| !item.as_str().is_empty()),
        "batch attempt item selection is invalid"
    );
    validate_attempt_state(attempt)?;
    Ok(())
}

fn validate_attempt_selection_chain(
    transaction: &Transaction<'_>,
    batch: &SealedBatch,
    target_attempt: &BatchAttempt,
) -> Result<()> {
    let mut expected_item_ids = batch
        .items
        .iter()
        .map(|item| item.item_id.clone())
        .collect::<Vec<_>>();

    for attempt_number in 0..=target_attempt.attempt_number {
        let attempt = if attempt_number == target_attempt.attempt_number {
            target_attempt.clone()
        } else {
            load_attempt_from_transaction(transaction, &batch.batch_id, attempt_number)?
                .ok_or_else(|| anyhow!("batch attempt selection chain is incomplete"))?
        };
        ensure!(
            attempt.item_ids == expected_item_ids,
            "batch attempt item selection does not match durable retry facts"
        );
        if attempt_number == target_attempt.attempt_number {
            break;
        }
        ensure!(
            attempt.status.is_terminal()
                && !matches!(
                    attempt.status,
                    BatchAttemptStatus::RecoveryRequired | BatchAttemptStatus::Interrupted
                )
                && !attempt.evidence_health_degraded,
            "batch retry selection follows an ineligible attempt"
        );
        let results =
            list_item_results_from_transaction(transaction, &batch.batch_id, attempt_number)?;
        validate_results_against_batch_attempt(batch, &attempt, &results)?;
        validate_attempt_completion(
            &attempt,
            attempt.status,
            attempt.evidence_health_degraded,
            &results,
        )?;
        expected_item_ids = results
            .into_iter()
            .filter(|result| result.retryable)
            .map(|result| result.item_id)
            .collect();
        ensure!(
            !expected_item_ids.is_empty(),
            "batch retry selection follows an attempt without retryable items"
        );
    }
    Ok(())
}

fn validate_attempt_state(attempt: &BatchAttempt) -> Result<()> {
    let timeline_is_ordered = match (
        attempt.started_at_unix_millis,
        attempt.completed_at_unix_millis,
    ) {
        (Some(started_at), Some(completed_at)) => completed_at >= started_at,
        _ => true,
    };
    let valid = !attempt.plan_token_verifier.is_empty()
        && attempt.expires_at_unix_millis > 0
        && timeline_is_ordered
        && match attempt.status {
            BatchAttemptStatus::Sealed => {
                attempt.task_id.is_none()
                    && attempt.started_at_unix_millis.is_none()
                    && attempt.completed_at_unix_millis.is_none()
                    && !attempt.evidence_health_degraded
            }
            BatchAttemptStatus::Queued => {
                attempt
                    .task_id
                    .as_deref()
                    .is_some_and(|task_id| !task_id.is_empty())
                    && attempt.started_at_unix_millis.is_none()
                    && attempt.completed_at_unix_millis.is_none()
                    && !attempt.evidence_health_degraded
            }
            BatchAttemptStatus::Running | BatchAttemptStatus::Stopping => {
                attempt
                    .task_id
                    .as_deref()
                    .is_some_and(|task_id| !task_id.is_empty())
                    && attempt.started_at_unix_millis.is_some()
                    && attempt.completed_at_unix_millis.is_none()
            }
            status if status.is_terminal() => {
                attempt
                    .task_id
                    .as_deref()
                    .is_some_and(|task_id| !task_id.is_empty())
                    && attempt.completed_at_unix_millis.is_some()
                    && (attempt.started_at_unix_millis.is_some()
                        || status == BatchAttemptStatus::Failed)
                    && (!matches!(
                        status,
                        BatchAttemptStatus::Interrupted | BatchAttemptStatus::Failed
                    ) || attempt.evidence_health_degraded)
            }
            _ => false,
        };
    ensure!(valid, "batch attempt state fields are inconsistent");
    Ok(())
}

fn validate_terminal_item_result(result: &BatchItemResult) -> Result<()> {
    ensure!(
        result.status.is_terminal(),
        "batch item result is not terminal"
    );
    let valid_reason = result
        .reason_code
        .as_deref()
        .map(is_stable_reason_code)
        .unwrap_or(false);
    let valid = match result.status {
        BatchItemStatus::Running => false,
        BatchItemStatus::Succeeded => result.reason_code.is_none() && !result.retryable,
        BatchItemStatus::Blocked | BatchItemStatus::Failed => valid_reason,
        BatchItemStatus::RecoveryRequired => valid_reason && !result.retryable,
        BatchItemStatus::Cancelled | BatchItemStatus::Skipped => valid_reason && result.retryable,
    };
    ensure!(valid, "batch item result status fields are inconsistent");
    Ok(())
}

fn is_stable_reason_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn validate_item_result_row(
    result: &BatchItemResult,
    batch_id: &BatchId,
    attempt_number: u32,
    stored_ordinal: i64,
) -> Result<()> {
    ensure!(
        result.batch_id == *batch_id
            && result.attempt_number == attempt_number
            && i64::try_from(result.ordinal).ok() == Some(stored_ordinal),
        "batch item result row identity mismatch"
    );
    if result.status.is_terminal() {
        validate_terminal_item_result(result)?;
    } else {
        ensure!(
            result.reason_code.is_none() && !result.retryable,
            "running batch item result is inconsistent"
        );
    }
    Ok(())
}

fn validate_attempt_completion(
    attempt: &BatchAttempt,
    status: BatchAttemptStatus,
    evidence_health_degraded: bool,
    results: &[BatchItemResult],
) -> Result<()> {
    let result_ids = results
        .iter()
        .map(|result| result.item_id.clone())
        .collect::<BTreeSet<_>>();
    ensure!(
        result_ids.len() == results.len()
            && result_ids
                .iter()
                .all(|item| attempt.item_ids.contains(item)),
        "batch attempt results are outside the selected item set"
    );
    if status == BatchAttemptStatus::Interrupted {
        ensure!(
            matches!(
                attempt.status,
                BatchAttemptStatus::Running | BatchAttemptStatus::Stopping
            ) && evidence_health_degraded,
            "interrupted batch attempt must report degraded evidence"
        );
        return Ok(());
    }
    if status == BatchAttemptStatus::Failed {
        ensure!(
            evidence_health_degraded,
            "failed batch attempt must report degraded evidence"
        );
        return Ok(());
    }
    ensure!(
        results.len() == attempt.item_ids.len()
            && results.iter().all(|result| result.status.is_terminal()),
        "normal batch terminal status requires every selected item result"
    );
    let summary = BatchResultSummary::from_item_results(attempt.item_ids.len(), results);
    let consistent = match status {
        BatchAttemptStatus::Completed => summary.succeeded_count == summary.item_count,
        BatchAttemptStatus::CompletedWithErrors => {
            summary.recovery_required_count == 0
                && summary.cancelled_count == 0
                && summary.succeeded_count < summary.item_count
                && summary.blocked_count + summary.failed_count + summary.skipped_count > 0
        }
        BatchAttemptStatus::Blocked => {
            summary.succeeded_count == 0
                && summary.failed_count == 0
                && summary.cancelled_count == 0
                && summary.recovery_required_count == 0
                && summary.blocked_count > 0
        }
        BatchAttemptStatus::Cancelled => {
            summary.recovery_required_count == 0
                && summary.succeeded_count < summary.item_count
                && summary.cancelled_count + summary.skipped_count > 0
        }
        BatchAttemptStatus::RecoveryRequired => summary.recovery_required_count > 0,
        BatchAttemptStatus::Sealed
        | BatchAttemptStatus::Queued
        | BatchAttemptStatus::Running
        | BatchAttemptStatus::Stopping
        | BatchAttemptStatus::Interrupted
        | BatchAttemptStatus::Failed => false,
    };
    ensure!(
        consistent,
        "batch terminal status does not match item results"
    );
    Ok(())
}

fn validate_results_against_batch_attempt(
    batch: &SealedBatch,
    attempt: &BatchAttempt,
    results: &[BatchItemResult],
) -> Result<()> {
    for result in results {
        let item = batch
            .items
            .iter()
            .find(|item| item.item_id == result.item_id)
            .ok_or_else(|| anyhow!("batch item result is outside the sealed batch"))?;
        ensure!(
            attempt.item_ids.contains(&result.item_id)
                && result.ordinal == item.ordinal
                && result.mod_id == item.mod_id,
            "batch item result identity does not match sealed attempt"
        );
    }
    Ok(())
}

fn serialize<T: Serialize>(value: &T, label: &str) -> Result<String> {
    serde_json::to_string(value).with_context(|| format!("failed to serialize {label}"))
}

fn deserialize<T: DeserializeOwned>(value: &str, label: &str) -> Result<T> {
    serde_json::from_str(value).with_context(|| format!("failed to deserialize {label}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        build_batch_plan, BatchActionSummary, BatchExecutionPolicy, BatchItemFacts, BatchItemInput,
        BatchOperation, BatchPlanFacts, BatchPlanRequest, BatchPreflightDecision,
        BatchPreflightStatus, BatchResourceLimits, BatchTargetClaim, BatchTargetWriteKind,
        FileLayer, GameId, InstallBatchItemInput, InstallTargetPath, ModId, ModRevisionId,
        ProfileId, SealedBatchItem,
    };
    use hmm_ports::{BatchRetryAttemptCreation, BatchRetryAttemptRequest, BatchSealRepository};
    use rusqlite::Connection;

    fn repository() -> SqliteBatchLifecycleRepository {
        let mut connection = Connection::open_in_memory().expect("database");
        rusqlite_migration::Migrations::new(vec![
            rusqlite_migration::M::up(include_str!("migrations/001_metadata_categories.sql")),
            rusqlite_migration::M::up(include_str!("migrations/002_profiles.sql")),
            rusqlite_migration::M::up(include_str!("migrations/003_profile_save_settings.sql")),
            rusqlite_migration::M::up(include_str!("migrations/004_save_backups.sql")),
            rusqlite_migration::M::up(include_str!(
                "migrations/005_save_backup_directory_snapshot.sql"
            )),
            rusqlite_migration::M::up(include_str!(
                "migrations/006_save_backup_scheduler_state.sql"
            )),
            rusqlite_migration::M::up(include_str!(
                "migrations/007_save_backup_worker_heartbeat.sql"
            )),
            rusqlite_migration::M::up(include_str!(
                "migrations/008_save_backup_background_settings.sql"
            )),
            rusqlite_migration::M::up(include_str!("migrations/009_mod_library_projection.sql")),
            rusqlite_migration::M::up(include_str!("migrations/010_external_import_preview.sql")),
            rusqlite_migration::M::up(include_str!("migrations/011_batch_lifecycle.sql")),
        ])
        .to_latest(&mut connection)
        .expect("schema");
        SqliteBatchLifecycleRepository::new(Arc::new(Mutex::new(connection)))
    }

    fn sealed_batch() -> (SealedBatch, BatchAttempt) {
        let request = BatchPlanRequest {
            schema_version: 1,
            operation: BatchOperation::Install,
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            execution_policy: BatchExecutionPolicy::StopOnFailure,
            items: vec![
                BatchItemInput::Install(InstallBatchItemInput {
                    mod_id: ModId::new("mod-a"),
                    revision_id: ModRevisionId::new("revision-a"),
                    layer: FileLayer::new("default", 1),
                    replacement_binding_snapshot: None,
                }),
                BatchItemInput::Install(InstallBatchItemInput {
                    mod_id: ModId::new("mod-b"),
                    revision_id: ModRevisionId::new("revision-b"),
                    layer: FileLayer::new("default", 1),
                    replacement_binding_snapshot: None,
                }),
            ],
        }
        .normalize()
        .expect("request");
        let plan = build_batch_plan(
            request.clone(),
            BatchPlanFacts {
                environment_digest: "env".to_owned(),
                prerequisite_rules_version: Some(1),
                items: [
                    ("mod-a", "revision-a", "fact-a", "plan-a", "nativepc/a"),
                    ("mod-b", "revision-b", "fact-b", "plan-b", "nativepc/b"),
                ]
                .into_iter()
                .map(
                    |(mod_id, revision_id, fact_digest, plan_digest, target)| BatchItemFacts {
                        mod_id: ModId::new(mod_id),
                        source_revision_id: Some(ModRevisionId::new(revision_id)),
                        installed_revision_id: None,
                        fact_digest: fact_digest.to_owned(),
                        single_plan_digest: plan_digest.to_owned(),
                        target_claims: vec![BatchTargetClaim {
                            target_path: InstallTargetPath::parse(target, ["nativepc"])
                                .expect("target"),
                            kind: BatchTargetWriteKind::Install,
                        }],
                        action_summary: BatchActionSummary {
                            actions: 1,
                            ..Default::default()
                        },
                        prerequisite: BatchPreflightDecision {
                            status: BatchPreflightStatus::Ready,
                            rules_version: Some(1),
                            codes: Vec::new(),
                        },
                        blocking_reasons: Vec::new(),
                        warning_codes: Vec::new(),
                    },
                )
                .collect(),
            },
            BatchResourceLimits::default(),
        )
        .expect("plan");
        let batch_id = BatchId::new("batch-test");
        let sealed = SealedBatch {
            batch_id: batch_id.clone(),
            request,
            plan,
            items: vec![
                SealedBatchItem {
                    item_id: BatchItemId::new("item-a"),
                    ordinal: 0,
                    mod_id: ModId::new("mod-a"),
                },
                SealedBatchItem {
                    item_id: BatchItemId::new("item-b"),
                    ordinal: 1,
                    mod_id: ModId::new("mod-b"),
                },
            ],
            created_at_unix_millis: 1,
        };
        let attempt = BatchAttempt {
            batch_id,
            attempt_number: 0,
            item_ids: sealed
                .items
                .iter()
                .map(|item| item.item_id.clone())
                .collect(),
            status: BatchAttemptStatus::Sealed,
            task_id: None,
            plan_token_verifier: "verifier".to_owned(),
            expires_at_unix_millis: 100,
            started_at_unix_millis: None,
            completed_at_unix_millis: None,
            evidence_health_degraded: false,
        };
        (sealed, attempt)
    }

    fn start_initial_attempt(repository: &SqliteBatchLifecycleRepository, batch: &SealedBatch) {
        repository
            .admit_attempt(BatchAttemptAdmissionRequest {
                batch_id: &batch.batch_id,
                attempt_number: 0,
                task_id: "task-a",
                presented_plan_token_verifier: "verifier",
                now_unix_millis: 2,
            })
            .expect("admit");
        repository
            .mark_attempt_running(&batch.batch_id, 0, 2)
            .expect("running");
    }

    fn item_result(
        batch: &SealedBatch,
        index: usize,
        status: BatchItemStatus,
        reason_code: Option<&str>,
        retryable: bool,
    ) -> BatchItemResult {
        let item = &batch.items[index];
        BatchItemResult {
            batch_id: batch.batch_id.clone(),
            attempt_number: 0,
            item_id: item.item_id.clone(),
            ordinal: item.ordinal,
            mod_id: item.mod_id.clone(),
            status,
            reason_code: reason_code.map(str::to_owned),
            retryable,
        }
    }

    #[test]
    fn seal_and_admission_are_once_only() {
        let repository = repository();
        let (batch, attempt) = sealed_batch();
        repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .expect("seal");
        assert!(repository
            .admit_attempt(BatchAttemptAdmissionRequest {
                batch_id: &batch.batch_id,
                attempt_number: 0,
                task_id: "",
                presented_plan_token_verifier: "verifier",
                now_unix_millis: 2,
            })
            .is_err());
        let first = repository
            .admit_attempt(BatchAttemptAdmissionRequest {
                batch_id: &batch.batch_id,
                attempt_number: 0,
                task_id: "task-a",
                presented_plan_token_verifier: "verifier",
                now_unix_millis: 2,
            })
            .expect("admit");
        assert!(matches!(first, BatchAttemptAdmission::Admitted(_)));
        let second = repository
            .admit_attempt(BatchAttemptAdmissionRequest {
                batch_id: &batch.batch_id,
                attempt_number: 0,
                task_id: "task-b",
                presented_plan_token_verifier: "verifier",
                now_unix_millis: 100,
            })
            .expect("repeat admit");
        assert!(matches!(
            second,
            BatchAttemptAdmission::AlreadyAdmitted(BatchAttempt {
                task_id: Some(ref task_id),
                ..
            }) if task_id == "task-a"
        ));
        assert_eq!(
            repository
                .admit_attempt(BatchAttemptAdmissionRequest {
                    batch_id: &batch.batch_id,
                    attempt_number: 0,
                    task_id: "task-c",
                    presented_plan_token_verifier: "different-verifier",
                    now_unix_millis: 100,
                })
                .expect("wrong verifier decision"),
            BatchAttemptAdmission::Rejected
        );
    }

    #[test]
    fn item_terminal_result_and_retry_attempt_are_durable_and_compare_and_swap() {
        let repository = repository();
        let (batch, attempt) = sealed_batch();
        repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .expect("seal");
        repository
            .admit_attempt(BatchAttemptAdmissionRequest {
                batch_id: &batch.batch_id,
                attempt_number: 0,
                task_id: "task-a",
                presented_plan_token_verifier: "verifier",
                now_unix_millis: 2,
            })
            .expect("admit");
        repository
            .mark_attempt_running(&batch.batch_id, 0, 2)
            .expect("running");
        let item = &batch.items[0];
        repository
            .mark_item_running(&batch.batch_id, 0, &item.item_id)
            .expect("item intent");
        let terminal = BatchItemResult {
            batch_id: batch.batch_id.clone(),
            attempt_number: 0,
            item_id: item.item_id.clone(),
            ordinal: item.ordinal,
            mod_id: item.mod_id.clone(),
            status: BatchItemStatus::Failed,
            reason_code: Some("commit_failed".to_owned()),
            retryable: true,
        };
        repository
            .record_item_result(&terminal)
            .expect("terminal result");
        let skipped_item = &batch.items[1];
        repository
            .record_item_result(&BatchItemResult {
                batch_id: batch.batch_id.clone(),
                attempt_number: 0,
                item_id: skipped_item.item_id.clone(),
                ordinal: skipped_item.ordinal,
                mod_id: skipped_item.mod_id.clone(),
                status: BatchItemStatus::Skipped,
                reason_code: Some("batch_stopped".to_owned()),
                retryable: true,
            })
            .expect("skipped item does not require a running intent");
        assert!(
            repository
                .mark_item_running(&batch.batch_id, 0, &item.item_id)
                .is_err(),
            "a terminal item can never return to running"
        );
        repository
            .finish_attempt(
                &batch.batch_id,
                0,
                BatchAttemptStatus::CompletedWithErrors,
                false,
                3,
            )
            .expect("finish");
        let retry = BatchAttempt {
            batch_id: batch.batch_id.clone(),
            attempt_number: 1,
            item_ids: vec![item.item_id.clone(), skipped_item.item_id.clone()],
            status: BatchAttemptStatus::Sealed,
            task_id: None,
            plan_token_verifier: "retry-verifier".to_owned(),
            expires_at_unix_millis: 100,
            started_at_unix_millis: None,
            completed_at_unix_millis: None,
            evidence_health_degraded: false,
        };
        let mut invalid_retry = retry.clone();
        invalid_retry.expires_at_unix_millis = 0;
        assert!(repository
            .create_retry_attempt(BatchRetryAttemptRequest {
                batch_id: &batch.batch_id,
                expected_attempt_number: 0,
                retry_attempt: &invalid_retry,
            })
            .is_err());
        assert!(matches!(
            repository
                .create_retry_attempt(BatchRetryAttemptRequest {
                    batch_id: &batch.batch_id,
                    expected_attempt_number: 0,
                    retry_attempt: &retry,
                })
                .expect("create retry"),
            BatchRetryAttemptCreation::Created(_)
        ));
        assert!(matches!(
            repository
                .create_retry_attempt(BatchRetryAttemptRequest {
                    batch_id: &batch.batch_id,
                    expected_attempt_number: 0,
                    retry_attempt: &retry,
                })
                .expect("repeat retry"),
            BatchRetryAttemptCreation::Stale
        ));
        repository
            .admit_attempt(BatchAttemptAdmissionRequest {
                batch_id: &batch.batch_id,
                attempt_number: 1,
                task_id: "task-retry",
                presented_plan_token_verifier: "retry-verifier",
                now_unix_millis: 4,
            })
            .expect("admit retry");
        repository
            .mark_attempt_running(&batch.batch_id, 1, 4)
            .expect("retry running");
        let unknown_item_id = BatchItemId::new("item-outside-attempt");
        assert!(repository
            .mark_item_running(&batch.batch_id, 1, &unknown_item_id)
            .is_err());
        assert!(repository
            .record_item_result(&BatchItemResult {
                batch_id: batch.batch_id.clone(),
                attempt_number: 1,
                item_id: unknown_item_id,
                ordinal: 2,
                mod_id: ModId::new("outside-attempt"),
                status: BatchItemStatus::Skipped,
                reason_code: Some("outside_attempt".to_owned()),
                retryable: true,
            })
            .is_err());
    }

    #[test]
    fn seal_rejects_mismatched_ordinal_and_mod_mapping() {
        let ordinal_repository = repository();
        let (mut batch, attempt) = sealed_batch();
        batch.items[0].ordinal = 1;
        assert!(ordinal_repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .is_err());

        let mod_repository = repository();
        let (mut batch, attempt) = sealed_batch();
        batch.items[0].mod_id = ModId::new("wrong-mod");
        assert!(mod_repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .is_err());
    }

    #[test]
    fn normal_terminal_status_requires_complete_consistent_item_results() {
        let repository = repository();
        let (batch, attempt) = sealed_batch();
        repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .expect("seal");
        start_initial_attempt(&repository, &batch);

        repository
            .mark_item_running(&batch.batch_id, 0, &batch.items[0].item_id)
            .expect("first item intent");
        repository
            .record_item_result(&item_result(
                &batch,
                0,
                BatchItemStatus::Succeeded,
                None,
                false,
            ))
            .expect("first item result");
        assert!(repository
            .finish_attempt(&batch.batch_id, 0, BatchAttemptStatus::Completed, false, 3,)
            .is_err());

        repository
            .mark_item_running(&batch.batch_id, 0, &batch.items[1].item_id)
            .expect("second item intent");
        repository
            .record_item_result(&item_result(
                &batch,
                1,
                BatchItemStatus::Failed,
                Some("commit_failed"),
                true,
            ))
            .expect("second item result");
        assert!(repository
            .finish_attempt(&batch.batch_id, 0, BatchAttemptStatus::Completed, false, 3,)
            .is_err());
        assert_eq!(
            repository
                .finish_attempt(
                    &batch.batch_id,
                    0,
                    BatchAttemptStatus::CompletedWithErrors,
                    false,
                    3,
                )
                .expect("consistent finish")
                .status,
            BatchAttemptStatus::CompletedWithErrors
        );
    }

    #[test]
    fn non_skipped_terminal_result_requires_running_intent_and_valid_fields() {
        let repository = repository();
        let (batch, attempt) = sealed_batch();
        repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .expect("seal");
        start_initial_attempt(&repository, &batch);

        assert!(repository
            .record_item_result(&item_result(
                &batch,
                0,
                BatchItemStatus::Succeeded,
                None,
                false,
            ))
            .is_err());
        assert!(repository
            .record_item_result(&item_result(
                &batch,
                1,
                BatchItemStatus::Skipped,
                Some("batch_stopped"),
                false,
            ))
            .is_err());

        repository
            .mark_item_running(&batch.batch_id, 0, &batch.items[0].item_id)
            .expect("item intent");
        assert!(repository
            .record_item_result(&item_result(
                &batch,
                0,
                BatchItemStatus::Succeeded,
                Some("unexpected_reason"),
                false,
            ))
            .is_err());
        repository
            .record_item_result(&item_result(
                &batch,
                0,
                BatchItemStatus::Succeeded,
                None,
                false,
            ))
            .expect("valid terminal result");
    }

    #[test]
    fn interrupted_attempt_requires_degraded_evidence_and_cannot_retry() {
        let repository = repository();
        let (batch, attempt) = sealed_batch();
        repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .expect("seal");
        start_initial_attempt(&repository, &batch);
        repository
            .mark_item_running(&batch.batch_id, 0, &batch.items[0].item_id)
            .expect("item intent");

        assert!(repository
            .finish_attempt(
                &batch.batch_id,
                0,
                BatchAttemptStatus::Interrupted,
                false,
                3,
            )
            .is_err());
        repository
            .finish_attempt(&batch.batch_id, 0, BatchAttemptStatus::Interrupted, true, 3)
            .expect("interrupted finish");
        let retry = BatchAttempt {
            batch_id: batch.batch_id.clone(),
            attempt_number: 1,
            item_ids: vec![batch.items[0].item_id.clone()],
            status: BatchAttemptStatus::Sealed,
            task_id: None,
            plan_token_verifier: "retry-verifier".to_owned(),
            expires_at_unix_millis: 100,
            started_at_unix_millis: None,
            completed_at_unix_millis: None,
            evidence_health_degraded: false,
        };
        assert!(matches!(
            repository
                .create_retry_attempt(BatchRetryAttemptRequest {
                    batch_id: &batch.batch_id,
                    expected_attempt_number: 0,
                    retry_attempt: &retry,
                })
                .expect("retry decision"),
            BatchRetryAttemptCreation::Unavailable
        ));
    }

    #[test]
    fn load_rejects_persisted_row_identity_mismatch() {
        let repository = repository();
        let (batch, attempt) = sealed_batch();
        repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .expect("seal");

        let mut corrupted = attempt;
        corrupted.batch_id = BatchId::new("other-batch");
        let corrupted_json = serialize(&corrupted, "corrupted attempt").expect("serialize");
        repository
            .db
            .lock()
            .expect("database")
            .execute(
                "UPDATE hmm_batch_lifecycle_attempts
                 SET attempt_json = ?3
                 WHERE batch_id = ?1 AND attempt_number = ?2",
                rusqlite::params![batch.batch_id.as_str(), 0_i64, corrupted_json],
            )
            .expect("corrupt row");

        assert!(repository.load_attempt(&batch.batch_id, 0).is_err());
    }

    #[test]
    fn load_and_admission_reject_persisted_sealed_plan_digest_mismatch() {
        let repository = repository();
        let (batch, attempt) = sealed_batch();
        repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .expect("seal");

        let mut corrupted = batch.clone();
        let BatchItemInput::Install(request_input) = &mut corrupted.request.items[0] else {
            panic!("install request");
        };
        request_input.revision_id = ModRevisionId::new("replacement-revision");
        let BatchItemInput::Install(plan_input) = &mut corrupted.plan.items[0].input_snapshot
        else {
            panic!("install plan");
        };
        plan_input.revision_id = ModRevisionId::new("replacement-revision");
        let corrupted_json = serialize(&corrupted, "corrupted sealed batch").expect("serialize");
        repository
            .db
            .lock()
            .expect("database")
            .execute(
                "UPDATE hmm_batch_lifecycle_batches
                 SET sealed_json = ?2
                 WHERE batch_id = ?1",
                rusqlite::params![batch.batch_id.as_str(), corrupted_json],
            )
            .expect("corrupt sealed batch");

        assert!(repository.load_batch(&batch.batch_id).is_err());
        assert!(repository.load_attempt(&batch.batch_id, 0).is_err());
        assert!(repository
            .admit_attempt(BatchAttemptAdmissionRequest {
                batch_id: &batch.batch_id,
                attempt_number: 0,
                task_id: "task-corrupted",
                presented_plan_token_verifier: "verifier",
                now_unix_millis: 2,
            })
            .is_err());
    }

    #[test]
    fn load_and_admission_reject_persisted_initial_attempt_selection_mismatch() {
        let repository = repository();
        let (batch, attempt) = sealed_batch();
        repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .expect("seal");
        let mut corrupted = attempt;
        corrupted.item_ids = vec![batch.items[1].item_id.clone()];
        let corrupted_json = serde_json::to_string(&corrupted).expect("corrupted json");
        repository
            .db
            .lock()
            .expect("database")
            .execute(
                "UPDATE hmm_batch_lifecycle_attempts
                 SET attempt_json = ?3
                 WHERE batch_id = ?1 AND attempt_number = ?2",
                rusqlite::params![batch.batch_id.as_str(), 0_i64, corrupted_json],
            )
            .expect("corrupt selection");

        assert!(repository.load_attempt(&batch.batch_id, 0).is_err());
        assert!(repository
            .admit_attempt(BatchAttemptAdmissionRequest {
                batch_id: &batch.batch_id,
                attempt_number: 0,
                task_id: "task-corrupted",
                presented_plan_token_verifier: "verifier",
                now_unix_millis: 2,
            })
            .is_err());
    }

    #[test]
    fn load_rejects_persisted_attempt_state_corruption() {
        let verifier_repository = repository();
        let (batch, mut attempt) = sealed_batch();
        verifier_repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .expect("seal");
        attempt.plan_token_verifier.clear();
        let corrupted_json = serialize(&attempt, "corrupted attempt").expect("serialize");
        verifier_repository
            .db
            .lock()
            .expect("database")
            .execute(
                "UPDATE hmm_batch_lifecycle_attempts
                 SET attempt_json = ?3
                 WHERE batch_id = ?1 AND attempt_number = ?2",
                rusqlite::params![batch.batch_id.as_str(), 0_i64, corrupted_json],
            )
            .expect("corrupt verifier");
        assert!(verifier_repository
            .load_attempt(&batch.batch_id, 0)
            .is_err());

        let state_repository = repository();
        let (batch, mut attempt) = sealed_batch();
        state_repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .expect("seal");
        attempt.status = BatchAttemptStatus::Running;
        let corrupted_json = serialize(&attempt, "corrupted attempt").expect("serialize");
        state_repository
            .db
            .lock()
            .expect("database")
            .execute(
                "UPDATE hmm_batch_lifecycle_attempts
                 SET attempt_json = ?3
                 WHERE batch_id = ?1 AND attempt_number = ?2",
                rusqlite::params![batch.batch_id.as_str(), 0_i64, corrupted_json],
            )
            .expect("corrupt state");
        assert!(state_repository.load_attempt(&batch.batch_id, 0).is_err());
    }

    #[test]
    fn load_rejects_persisted_attempt_with_reversed_timeline() {
        let repository = repository();
        let (batch, attempt) = sealed_batch();
        repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .expect("seal");
        start_initial_attempt(&repository, &batch);

        let mut corrupted = repository
            .load_attempt(&batch.batch_id, 0)
            .expect("load attempt")
            .expect("attempt");
        corrupted.status = BatchAttemptStatus::Completed;
        corrupted.completed_at_unix_millis = Some(1);
        let corrupted_json = serialize(&corrupted, "corrupted attempt").expect("serialize");
        repository
            .db
            .lock()
            .expect("database")
            .execute(
                "UPDATE hmm_batch_lifecycle_attempts
                 SET attempt_json = ?3
                 WHERE batch_id = ?1 AND attempt_number = ?2",
                rusqlite::params![batch.batch_id.as_str(), 0_i64, corrupted_json],
            )
            .expect("corrupt timeline");

        assert!(repository.load_attempt(&batch.batch_id, 0).is_err());
    }
}
