use anyhow::{anyhow, ensure, Context, Result};
use hmm_core::{
    ExternalImportBatch, ExternalImportBatchId, ExternalImportBatchImportStatus,
    ExternalImportCandidate, ExternalImportItemResult, ExternalImportScanStatus,
    ExternalImportSelection, ExternalImportSelectionId,
};
use hmm_ports::{
    ExternalImportBatchRepository, ExternalImportCandidatePage, ExternalImportItemResultPage,
    ExternalImportSealAndStartRequest, ExternalImportSealAndStartResult,
    ExternalImportSelectionCompareAndSwapRequest, ExternalImportSelectionCompareAndSwapResult,
};
use rusqlite::{Connection, OptionalExtension, Transaction};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::{Arc, Mutex};

pub struct SqliteExternalImportBatchRepository {
    db: Arc<Mutex<Connection>>,
}

impl SqliteExternalImportBatchRepository {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| anyhow!("external import database lock poisoned"))
    }
}

impl ExternalImportBatchRepository for SqliteExternalImportBatchRepository {
    fn create_batch(&self, batch: &ExternalImportBatch) -> Result<()> {
        let batch_json = serialize(batch, "external import batch")?;
        let conn = self.lock_db()?;
        conn.execute(
            "INSERT INTO external_import_batches (batch_id, batch_json) VALUES (?1, ?2)",
            rusqlite::params![batch.batch_id.as_str(), batch_json],
        )
        .context("failed to create external import batch")?;
        Ok(())
    }

    fn get_batch(&self, batch_id: &ExternalImportBatchId) -> Result<Option<ExternalImportBatch>> {
        let conn = self.lock_db()?;
        let batch_json: Option<String> = conn
            .query_row(
                "SELECT batch_json FROM external_import_batches WHERE batch_id = ?1",
                rusqlite::params![batch_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .context("failed to read external import batch")?;
        batch_json
            .as_deref()
            .map(|value| deserialize(value, "external import batch"))
            .transpose()
    }

    fn update_batch(&self, batch: &ExternalImportBatch) -> Result<()> {
        let batch_json = serialize(batch, "external import batch")?;
        let conn = self.lock_db()?;
        let changed = conn
            .execute(
                "UPDATE external_import_batches SET batch_json = ?2 WHERE batch_id = ?1",
                rusqlite::params![batch.batch_id.as_str(), batch_json],
            )
            .context("failed to update external import batch")?;
        ensure!(changed == 1, "external import batch is unavailable");
        Ok(())
    }

    fn save_scan_result(
        &self,
        batch: &ExternalImportBatch,
        candidates: &[ExternalImportCandidate],
    ) -> Result<()> {
        validate_candidate_batch_ids(&batch.batch_id, candidates)?;
        let batch_json = serialize(batch, "external import batch")?;
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin external import scan result transaction")?;
        let changed = transaction
            .execute(
                "UPDATE external_import_batches SET batch_json = ?2 WHERE batch_id = ?1",
                rusqlite::params![batch.batch_id.as_str(), batch_json],
            )
            .context("failed to update external import scan batch")?;
        ensure!(changed == 1, "external import batch is unavailable");
        replace_candidates_in_transaction(&transaction, &batch.batch_id, candidates)?;
        transaction
            .commit()
            .context("failed to commit external import scan result")?;
        Ok(())
    }

    fn replace_candidates(
        &self,
        batch_id: &ExternalImportBatchId,
        candidates: &[ExternalImportCandidate],
    ) -> Result<()> {
        validate_candidate_batch_ids(batch_id, candidates)?;
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin external import candidate transaction")?;
        ensure!(
            batch_exists(&transaction, batch_id)?,
            "external import batch is unavailable"
        );
        replace_candidates_in_transaction(&transaction, batch_id, candidates)?;
        transaction
            .commit()
            .context("failed to commit external import candidate replacement")?;
        Ok(())
    }

    fn list_candidates(
        &self,
        batch_id: &ExternalImportBatchId,
    ) -> Result<Vec<ExternalImportCandidate>> {
        let conn = self.lock_db()?;
        list_candidates_from_connection(&conn, batch_id)
    }

    fn list_candidates_page(
        &self,
        batch_id: &ExternalImportBatchId,
        offset: usize,
        limit: usize,
    ) -> Result<ExternalImportCandidatePage> {
        ensure!(
            limit > 0,
            "external import candidate page limit must be positive"
        );
        let offset = i64::try_from(offset).context("external import page offset is too large")?;
        let limit = i64::try_from(limit).context("external import page limit is too large")?;
        let conn = self.lock_db()?;
        let total_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM external_import_candidates WHERE batch_id = ?1",
                rusqlite::params![batch_id.as_str()],
                |row| row.get(0),
            )
            .context("failed to count external import candidates")?;
        let total_count =
            usize::try_from(total_count).context("external import candidate count is invalid")?;
        let mut statement = conn
            .prepare(
                "SELECT candidate_json
                 FROM external_import_candidates
                 WHERE batch_id = ?1
                 ORDER BY ordinal ASC
                 LIMIT ?2 OFFSET ?3",
            )
            .context("failed to prepare external import candidate page")?;
        let rows = statement
            .query_map(rusqlite::params![batch_id.as_str(), limit, offset], |row| {
                row.get::<_, String>(0)
            })
            .context("failed to query external import candidate page")?;
        let candidates = deserialize_rows(rows, "external import candidate")?;
        let next_offset = usize::try_from(offset)
            .ok()
            .and_then(|offset| offset.checked_add(candidates.len()))
            .filter(|next| *next < total_count);

        Ok(ExternalImportCandidatePage {
            candidates,
            total_count,
            next_offset,
        })
    }

    fn create_selection(&self, selection: &ExternalImportSelection) -> Result<()> {
        let selection_json = serialize(selection, "external import selection")?;
        let conn = self.lock_db()?;
        conn.execute(
            "INSERT INTO external_import_selections (selection_id, batch_id, selection_json)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![
                selection.selection_id.as_str(),
                selection.batch_id.as_str(),
                selection_json
            ],
        )
        .context("failed to create external import selection")?;
        Ok(())
    }

    fn get_selection(
        &self,
        selection_id: &ExternalImportSelectionId,
    ) -> Result<Option<ExternalImportSelection>> {
        let conn = self.lock_db()?;
        let selection_json: Option<String> = conn
            .query_row(
                "SELECT selection_json FROM external_import_selections WHERE selection_id = ?1",
                rusqlite::params![selection_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .context("failed to read external import selection")?;
        selection_json
            .as_deref()
            .map(|value| deserialize(value, "external import selection"))
            .transpose()
    }

    fn compare_and_swap_selection(
        &self,
        request: ExternalImportSelectionCompareAndSwapRequest<'_>,
    ) -> Result<ExternalImportSelectionCompareAndSwapResult> {
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin external import selection transaction")?;
        let current_json: Option<String> = transaction
            .query_row(
                "SELECT selection_json FROM external_import_selections WHERE selection_id = ?1",
                rusqlite::params![request.selection.selection_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .context("failed to read external import selection for compare-and-swap")?;
        let Some(current_json) = current_json else {
            anyhow::bail!("external import selection is unavailable");
        };
        let current: ExternalImportSelection =
            deserialize(&current_json, "external import selection")?;
        if current.revision != request.expected_revision {
            return Ok(
                ExternalImportSelectionCompareAndSwapResult::RevisionConflict {
                    current_revision: current.revision,
                },
            );
        }
        ensure!(
            current.batch_id == request.selection.batch_id,
            "external import selection batch is invalid"
        );
        let next_json = serialize(request.selection, "external import selection")?;
        let changed = transaction
            .execute(
                "UPDATE external_import_selections
                 SET selection_json = ?2
                 WHERE selection_id = ?1",
                rusqlite::params![request.selection.selection_id.as_str(), next_json],
            )
            .context("failed to write external import selection compare-and-swap")?;
        ensure!(changed == 1, "external import selection is unavailable");
        transaction
            .commit()
            .context("failed to commit external import selection compare-and-swap")?;
        Ok(ExternalImportSelectionCompareAndSwapResult::Applied(
            request.selection.clone(),
        ))
    }

    fn seal_selection_and_start(
        &self,
        request: ExternalImportSealAndStartRequest<'_>,
    ) -> Result<ExternalImportSealAndStartResult> {
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin external import sealed start transaction")?;
        let selection_json: Option<String> = transaction
            .query_row(
                "SELECT selection_json FROM external_import_selections WHERE selection_id = ?1",
                rusqlite::params![request.selection_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .context("failed to read external import selection for sealed start")?;
        let Some(selection_json) = selection_json else {
            anyhow::bail!("external import selection is unavailable");
        };
        let mut selection: ExternalImportSelection =
            deserialize(&selection_json, "external import selection")?;
        if selection.revision != request.expected_revision {
            return Ok(ExternalImportSealAndStartResult::RevisionConflict {
                current_revision: selection.revision,
            });
        }

        let batch_json: Option<String> = transaction
            .query_row(
                "SELECT batch_json FROM external_import_batches WHERE batch_id = ?1",
                rusqlite::params![selection.batch_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .context("failed to read external import batch for sealed start")?;
        let Some(batch_json) = batch_json else {
            anyhow::bail!("external import batch is unavailable");
        };
        let mut batch: ExternalImportBatch = deserialize(&batch_json, "external import batch")?;
        if batch.scan_status != ExternalImportScanStatus::Completed
            || batch.import_status != ExternalImportBatchImportStatus::Pending
        {
            return Ok(ExternalImportSealAndStartResult::BatchNotStartable);
        }

        let candidates = list_candidates_from_transaction(&transaction, &batch.batch_id)?;
        if let Err(error) = selection.seal(
            request.expected_revision,
            &candidates,
            request.resource_budget,
            request.now_unix_millis,
        ) {
            return Ok(ExternalImportSealAndStartResult::SelectionRejected { error });
        }
        batch.import_status = ExternalImportBatchImportStatus::Running;

        let selection_json = serialize(&selection, "external import sealed selection")?;
        let batch_json = serialize(&batch, "external import running batch")?;
        let selection_changed = transaction
            .execute(
                "UPDATE external_import_selections SET selection_json = ?2 WHERE selection_id = ?1",
                rusqlite::params![selection.selection_id.as_str(), selection_json],
            )
            .context("failed to write external import sealed selection")?;
        ensure!(
            selection_changed == 1,
            "external import selection is unavailable"
        );
        let batch_changed = transaction
            .execute(
                "UPDATE external_import_batches SET batch_json = ?2 WHERE batch_id = ?1",
                rusqlite::params![batch.batch_id.as_str(), batch_json],
            )
            .context("failed to write external import running batch")?;
        ensure!(batch_changed == 1, "external import batch is unavailable");
        transaction
            .commit()
            .context("failed to commit external import sealed start")?;

        Ok(ExternalImportSealAndStartResult::Started {
            batch,
            selection: Box::new(selection),
        })
    }

    fn restart_batch(
        &self,
        batch_id: &ExternalImportBatchId,
    ) -> Result<Option<ExternalImportBatch>> {
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin external import batch retry transaction")?;
        let batch_json: Option<String> = transaction
            .query_row(
                "SELECT batch_json FROM external_import_batches WHERE batch_id = ?1",
                rusqlite::params![batch_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .context("failed to read external import batch for retry")?;
        let Some(batch_json) = batch_json else {
            return Ok(None);
        };
        let mut batch: ExternalImportBatch = deserialize(&batch_json, "external import batch")?;
        if !matches!(
            batch.import_status,
            ExternalImportBatchImportStatus::CompletedWithErrors
                | ExternalImportBatchImportStatus::Failed
                | ExternalImportBatchImportStatus::Cancelled
        ) {
            return Ok(None);
        }
        batch.import_status = ExternalImportBatchImportStatus::Running;
        let batch_json = serialize(&batch, "external import retried batch")?;
        let changed = transaction
            .execute(
                "UPDATE external_import_batches SET batch_json = ?2 WHERE batch_id = ?1",
                rusqlite::params![batch.batch_id.as_str(), batch_json],
            )
            .context("failed to write external import retried batch")?;
        ensure!(changed == 1, "external import batch is unavailable");
        transaction
            .commit()
            .context("failed to commit external import batch retry")?;
        Ok(Some(batch))
    }

    fn recover_interrupted_batches(&self) -> Result<usize> {
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin external import interrupted batch recovery")?;
        let mut statement = transaction
            .prepare("SELECT batch_json FROM external_import_batches")
            .context("failed to prepare external import interrupted batch query")?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .context("failed to query external import interrupted batches")?;
        let mut batches: Vec<ExternalImportBatch> =
            deserialize_rows(rows, "external import batch")?;
        drop(statement);

        let mut recovered = 0_usize;
        for batch in &mut batches {
            if batch.import_status != ExternalImportBatchImportStatus::Running {
                continue;
            }
            batch.import_status = ExternalImportBatchImportStatus::Failed;
            let batch_json = serialize(batch, "external import recovered batch")?;
            let changed = transaction
                .execute(
                    "UPDATE external_import_batches SET batch_json = ?2 WHERE batch_id = ?1",
                    rusqlite::params![batch.batch_id.as_str(), batch_json],
                )
                .context("failed to write external import recovered batch")?;
            ensure!(changed == 1, "external import batch is unavailable");
            recovered = recovered
                .checked_add(1)
                .ok_or_else(|| anyhow!("external import recovered batch count overflow"))?;
        }
        transaction
            .commit()
            .context("failed to commit external import interrupted batch recovery")?;
        Ok(recovered)
    }

    fn append_item_results(
        &self,
        batch_id: &ExternalImportBatchId,
        results: &[ExternalImportItemResult],
    ) -> Result<()> {
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin external import item result transaction")?;
        ensure!(
            batch_exists(&transaction, batch_id)?,
            "external import batch is unavailable"
        );
        let mut statement = transaction
            .prepare(
                "INSERT INTO external_import_item_results
                    (batch_id, candidate_id, ordinal, result_json)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(batch_id, candidate_id) DO UPDATE SET
                    ordinal = excluded.ordinal,
                    result_json = excluded.result_json",
            )
            .context("failed to prepare external import item result write")?;
        for result in results {
            let result_json = serialize(result, "external import item result")?;
            let ordinal: Option<i64> = transaction
                .query_row(
                    "SELECT ordinal FROM external_import_candidates
                     WHERE batch_id = ?1 AND candidate_id = ?2",
                    rusqlite::params![batch_id.as_str(), result.candidate_id.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .context("failed to resolve external import item result candidate order")?;
            let ordinal = ordinal
                .ok_or_else(|| anyhow!("external import item result candidate is unavailable"))?;
            statement
                .execute(rusqlite::params![
                    batch_id.as_str(),
                    result.candidate_id.as_str(),
                    ordinal,
                    result_json,
                ])
                .context("failed to append external import item result")?;
        }
        drop(statement);
        transaction
            .commit()
            .context("failed to commit external import item results")?;
        Ok(())
    }

    fn list_item_results(
        &self,
        batch_id: &ExternalImportBatchId,
    ) -> Result<Vec<ExternalImportItemResult>> {
        let conn = self.lock_db()?;
        let mut statement = conn
            .prepare(
                "SELECT result_json
                 FROM external_import_item_results
                 WHERE batch_id = ?1
                 ORDER BY ordinal ASC",
            )
            .context("failed to prepare external import item result query")?;
        let rows = statement
            .query_map(rusqlite::params![batch_id.as_str()], |row| {
                row.get::<_, String>(0)
            })
            .context("failed to query external import item results")?;
        deserialize_rows(rows, "external import item result")
    }

    fn list_item_results_page(
        &self,
        batch_id: &ExternalImportBatchId,
        offset: usize,
        limit: usize,
    ) -> Result<ExternalImportItemResultPage> {
        ensure!(
            limit > 0,
            "external import item result page limit must be positive"
        );
        let offset =
            i64::try_from(offset).context("external import result page offset is too large")?;
        let limit =
            i64::try_from(limit).context("external import result page limit is too large")?;
        let conn = self.lock_db()?;
        let total_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM external_import_item_results WHERE batch_id = ?1",
                rusqlite::params![batch_id.as_str()],
                |row| row.get(0),
            )
            .context("failed to count external import item results")?;
        let total_count =
            usize::try_from(total_count).context("external import item result count is invalid")?;
        let mut statement = conn
            .prepare(
                "SELECT result_json
                 FROM external_import_item_results
                 WHERE batch_id = ?1
                 ORDER BY ordinal ASC
                 LIMIT ?2 OFFSET ?3",
            )
            .context("failed to prepare external import item result page")?;
        let rows = statement
            .query_map(rusqlite::params![batch_id.as_str(), limit, offset], |row| {
                row.get::<_, String>(0)
            })
            .context("failed to query external import item result page")?;
        let results = deserialize_rows(rows, "external import item result")?;
        let next_offset = usize::try_from(offset)
            .ok()
            .and_then(|offset| offset.checked_add(results.len()))
            .filter(|next| *next < total_count);

        Ok(ExternalImportItemResultPage {
            results,
            total_count,
            next_offset,
        })
    }
}

fn batch_exists(transaction: &Transaction<'_>, batch_id: &ExternalImportBatchId) -> Result<bool> {
    transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM external_import_batches WHERE batch_id = ?1)",
            rusqlite::params![batch_id.as_str()],
            |row| row.get(0),
        )
        .context("failed to verify external import batch")
}

fn replace_candidates_in_transaction(
    transaction: &Transaction<'_>,
    batch_id: &ExternalImportBatchId,
    candidates: &[ExternalImportCandidate],
) -> Result<()> {
    transaction
        .execute(
            "DELETE FROM external_import_candidates WHERE batch_id = ?1",
            rusqlite::params![batch_id.as_str()],
        )
        .context("failed to clear external import candidates")?;
    let mut statement = transaction
        .prepare(
            "INSERT INTO external_import_candidates
                (batch_id, candidate_id, ordinal, candidate_json)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .context("failed to prepare external import candidate write")?;
    for (ordinal, candidate) in candidates.iter().enumerate() {
        statement
            .execute(rusqlite::params![
                batch_id.as_str(),
                candidate.candidate_id.as_str(),
                i64::try_from(ordinal).context("external import candidate count is too large")?,
                serialize(candidate, "external import candidate")?,
            ])
            .context("failed to write external import candidate")?;
    }
    Ok(())
}

fn list_candidates_from_connection(
    connection: &Connection,
    batch_id: &ExternalImportBatchId,
) -> Result<Vec<ExternalImportCandidate>> {
    let mut statement = connection
        .prepare(
            "SELECT candidate_json
             FROM external_import_candidates
             WHERE batch_id = ?1
             ORDER BY ordinal ASC",
        )
        .context("failed to prepare external import candidate query")?;
    let rows = statement
        .query_map(rusqlite::params![batch_id.as_str()], |row| {
            row.get::<_, String>(0)
        })
        .context("failed to query external import candidates")?;
    deserialize_rows(rows, "external import candidate")
}

fn list_candidates_from_transaction(
    transaction: &Transaction<'_>,
    batch_id: &ExternalImportBatchId,
) -> Result<Vec<ExternalImportCandidate>> {
    let mut statement = transaction
        .prepare(
            "SELECT candidate_json
             FROM external_import_candidates
             WHERE batch_id = ?1
             ORDER BY ordinal ASC",
        )
        .context("failed to prepare external import candidate transaction query")?;
    let rows = statement
        .query_map(rusqlite::params![batch_id.as_str()], |row| {
            row.get::<_, String>(0)
        })
        .context("failed to query external import candidate transaction rows")?;
    deserialize_rows(rows, "external import candidate")
}

fn validate_candidate_batch_ids(
    batch_id: &ExternalImportBatchId,
    candidates: &[ExternalImportCandidate],
) -> Result<()> {
    ensure!(
        candidates
            .iter()
            .all(|candidate| candidate.batch_id == *batch_id),
        "external import candidate batch is invalid"
    );
    Ok(())
}

fn serialize<T: Serialize>(value: &T, label: &str) -> Result<String> {
    serde_json::to_string(value).with_context(|| format!("failed to serialize {label}"))
}

fn deserialize<T: DeserializeOwned>(value: &str, label: &str) -> Result<T> {
    serde_json::from_str(value).with_context(|| format!("failed to deserialize {label}"))
}

fn deserialize_rows<T: DeserializeOwned>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<String>>,
    label: &str,
) -> Result<Vec<T>> {
    let mut values = Vec::new();
    for row in rows {
        let value = row.with_context(|| format!("failed to read {label} row"))?;
        values.push(deserialize(&value, label)?);
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        ExternalImportAdapterId, ExternalImportBatchImportStatus, ExternalImportCandidateId,
        ExternalImportCandidateStatus, ExternalImportConflictKind, ExternalImportItemResult,
        ExternalImportItemStatus, ExternalImportMetadataHint, ExternalImportResourceBudget,
        ExternalImportResourceUsage, ExternalImportScanStatus, ExternalImportSelectionMutation,
    };

    #[test]
    fn scan_result_is_durable_and_pages_in_scanner_order() {
        let temporary = tempfile::tempdir().expect("temporary database directory");
        let db = Arc::new(Mutex::new(
            crate::open_database(&temporary.path().join("hmm.db")).expect("open database"),
        ));
        let repository = SqliteExternalImportBatchRepository::new(Arc::clone(&db));
        let mut batch = batch("batch-page");
        repository.create_batch(&batch).expect("create batch");
        batch.scan_status = ExternalImportScanStatus::Completed;
        let candidates = vec![
            candidate(&batch.batch_id, "candidate-1"),
            candidate(&batch.batch_id, "candidate-2"),
            candidate(&batch.batch_id, "candidate-3"),
        ];

        repository
            .save_scan_result(&batch, &candidates)
            .expect("persist scan result");
        let first_page = repository
            .list_candidates_page(&batch.batch_id, 0, 2)
            .expect("first page");
        let second_page = repository
            .list_candidates_page(&batch.batch_id, 2, 2)
            .expect("second page");

        assert_eq!(first_page.total_count, 3);
        assert_eq!(first_page.next_offset, Some(2));
        assert_eq!(
            first_page
                .candidates
                .iter()
                .map(|candidate| candidate.candidate_id.as_str())
                .collect::<Vec<_>>(),
            ["candidate-1", "candidate-2"]
        );
        assert_eq!(second_page.total_count, 3);
        assert_eq!(second_page.next_offset, None);
        assert_eq!(
            second_page.candidates[0].candidate_id.as_str(),
            "candidate-3"
        );
        assert_eq!(
            repository
                .get_batch(&batch.batch_id)
                .expect("get batch")
                .expect("batch exists")
                .scan_status,
            ExternalImportScanStatus::Completed
        );
    }

    #[test]
    fn invalid_scan_result_leaves_existing_batch_and_candidates_unchanged() {
        let temporary = tempfile::tempdir().expect("temporary database directory");
        let db = Arc::new(Mutex::new(
            crate::open_database(&temporary.path().join("hmm.db")).expect("open database"),
        ));
        let repository = SqliteExternalImportBatchRepository::new(db);
        let mut batch = batch("batch-atomic");
        repository.create_batch(&batch).expect("create batch");
        let existing = candidate(&batch.batch_id, "candidate-existing");
        repository
            .replace_candidates(&batch.batch_id, std::slice::from_ref(&existing))
            .expect("write existing candidate");
        batch.scan_status = ExternalImportScanStatus::Completed;
        let invalid = candidate(
            &ExternalImportBatchId::new("different-batch"),
            "candidate-invalid",
        );

        let error = repository
            .save_scan_result(&batch, &[invalid])
            .expect_err("mismatched candidate is rejected before a transaction writes");

        assert!(error.to_string().contains("candidate batch"));
        assert_eq!(
            repository
                .get_batch(&batch.batch_id)
                .expect("get batch")
                .expect("batch exists")
                .scan_status,
            ExternalImportScanStatus::Pending
        );
        assert_eq!(
            repository
                .list_candidates(&batch.batch_id)
                .expect("list candidates"),
            vec![existing]
        );
    }

    #[test]
    fn selection_compare_and_swap_detects_stale_revisions() {
        let temporary = tempfile::tempdir().expect("temporary database directory");
        let db = Arc::new(Mutex::new(
            crate::open_database(&temporary.path().join("hmm.db")).expect("open database"),
        ));
        let repository = SqliteExternalImportBatchRepository::new(db);
        let batch = batch("batch-selection");
        repository.create_batch(&batch).expect("create batch");
        let selection = ExternalImportSelection::new(
            ExternalImportSelectionId::new("selection-1"),
            batch.batch_id.clone(),
            1000,
        );
        repository
            .create_selection(&selection)
            .expect("create selection");
        let mut next = selection.clone();
        next.revision = 1;

        let applied = repository
            .compare_and_swap_selection(ExternalImportSelectionCompareAndSwapRequest {
                selection: &next,
                expected_revision: 0,
            })
            .expect("apply compare-and-swap");
        let stale = repository
            .compare_and_swap_selection(ExternalImportSelectionCompareAndSwapRequest {
                selection: &next,
                expected_revision: 0,
            })
            .expect("report stale revision");

        assert_eq!(
            applied,
            ExternalImportSelectionCompareAndSwapResult::Applied(next)
        );
        assert_eq!(
            stale,
            ExternalImportSelectionCompareAndSwapResult::RevisionConflict {
                current_revision: 1
            }
        );
    }

    #[test]
    fn sealed_start_persists_selection_and_running_batch_together() {
        let temporary = tempfile::tempdir().expect("temporary database directory");
        let db = Arc::new(Mutex::new(
            crate::open_database(&temporary.path().join("hmm.db")).expect("open database"),
        ));
        let repository = SqliteExternalImportBatchRepository::new(db);
        let mut batch = batch("batch-sealed-start");
        batch.scan_status = ExternalImportScanStatus::Completed;
        let candidate = candidate(&batch.batch_id, "candidate-1");
        repository.create_batch(&batch).expect("create batch");
        repository
            .save_scan_result(&batch, std::slice::from_ref(&candidate))
            .expect("save scan result");
        let mut selection = ExternalImportSelection::new(
            ExternalImportSelectionId::new("selection-sealed-start"),
            batch.batch_id.clone(),
            1000,
        );
        selection
            .apply_mutation(
                0,
                &[ExternalImportSelectionMutation {
                    candidate_id: candidate.candidate_id.clone(),
                    selected: true,
                    decision: None,
                }],
                std::slice::from_ref(&candidate),
                &ExternalImportResourceBudget::default(),
                1,
            )
            .expect("select candidate");
        repository
            .create_selection(&selection)
            .expect("create selection");

        let result = repository
            .seal_selection_and_start(ExternalImportSealAndStartRequest {
                selection_id: &selection.selection_id,
                expected_revision: selection.revision,
                now_unix_millis: 2,
                resource_budget: &ExternalImportResourceBudget::default(),
            })
            .expect("sealed start transaction");

        let ExternalImportSealAndStartResult::Started {
            batch: started_batch,
            selection: sealed_selection,
        } = result
        else {
            panic!("selection should seal and batch should start");
        };
        assert_eq!(
            started_batch.import_status,
            ExternalImportBatchImportStatus::Running
        );
        assert_eq!(
            sealed_selection.status,
            hmm_core::ExternalImportSelectionStatus::Sealed
        );
        assert_eq!(
            repository
                .get_batch(&batch.batch_id)
                .expect("read batch")
                .expect("batch exists")
                .import_status,
            ExternalImportBatchImportStatus::Running
        );
        assert_eq!(
            repository
                .get_selection(&selection.selection_id)
                .expect("read selection")
                .expect("selection exists")
                .status,
            hmm_core::ExternalImportSelectionStatus::Sealed
        );
    }

    #[test]
    fn startup_recovery_marks_running_batches_failed_without_losing_the_sealed_selection() {
        let temporary = tempfile::tempdir().expect("temporary database directory");
        let db = Arc::new(Mutex::new(
            crate::open_database(&temporary.path().join("hmm.db")).expect("open database"),
        ));
        let repository = SqliteExternalImportBatchRepository::new(db);
        let mut batch = batch("batch-interrupted");
        batch.scan_status = ExternalImportScanStatus::Completed;
        let candidate = candidate(&batch.batch_id, "candidate-1");
        repository.create_batch(&batch).expect("create batch");
        repository
            .save_scan_result(&batch, std::slice::from_ref(&candidate))
            .expect("save scan result");
        let mut selection = ExternalImportSelection::new(
            ExternalImportSelectionId::new("selection-interrupted"),
            batch.batch_id.clone(),
            1000,
        );
        selection
            .apply_mutation(
                0,
                &[ExternalImportSelectionMutation {
                    candidate_id: candidate.candidate_id.clone(),
                    selected: true,
                    decision: None,
                }],
                std::slice::from_ref(&candidate),
                &ExternalImportResourceBudget::default(),
                1,
            )
            .expect("select candidate");
        repository
            .create_selection(&selection)
            .expect("create selection");
        repository
            .seal_selection_and_start(ExternalImportSealAndStartRequest {
                selection_id: &selection.selection_id,
                expected_revision: selection.revision,
                now_unix_millis: 2,
                resource_budget: &ExternalImportResourceBudget::default(),
            })
            .expect("seal selection");

        assert_eq!(
            repository
                .recover_interrupted_batches()
                .expect("recover interrupted batches"),
            1
        );
        assert_eq!(
            repository
                .get_batch(&batch.batch_id)
                .expect("read recovered batch")
                .expect("batch exists")
                .import_status,
            ExternalImportBatchImportStatus::Failed
        );
        assert_eq!(
            repository
                .get_selection(&selection.selection_id)
                .expect("read sealed selection")
                .expect("selection exists")
                .status,
            hmm_core::ExternalImportSelectionStatus::Sealed
        );
    }

    #[test]
    fn item_result_pages_follow_candidate_scan_order_across_multiple_appends() {
        let temporary = tempfile::tempdir().expect("temporary database directory");
        let db = Arc::new(Mutex::new(
            crate::open_database(&temporary.path().join("hmm.db")).expect("open database"),
        ));
        let repository = SqliteExternalImportBatchRepository::new(db);
        let mut batch = batch("batch-result-page");
        batch.scan_status = ExternalImportScanStatus::Completed;
        let first = candidate(&batch.batch_id, "candidate-1");
        let second = candidate(&batch.batch_id, "candidate-2");
        repository.create_batch(&batch).expect("create batch");
        repository
            .save_scan_result(&batch, &[first.clone(), second.clone()])
            .expect("save scan result");

        repository
            .append_item_results(
                &batch.batch_id,
                &[ExternalImportItemResult {
                    candidate_id: second.candidate_id.clone(),
                    status: ExternalImportItemStatus::Imported,
                    reason_code: None,
                    imported_mod_id: None,
                    retryable: false,
                }],
            )
            .expect("append later candidate result");
        repository
            .append_item_results(
                &batch.batch_id,
                &[ExternalImportItemResult {
                    candidate_id: first.candidate_id.clone(),
                    status: ExternalImportItemStatus::Failed,
                    reason_code: None,
                    imported_mod_id: None,
                    retryable: true,
                }],
            )
            .expect("append first candidate result");

        let page = repository
            .list_item_results_page(&batch.batch_id, 0, 1)
            .expect("read result page");
        assert_eq!(page.total_count, 2);
        assert_eq!(page.next_offset, Some(1));
        assert_eq!(page.results[0].candidate_id, first.candidate_id);
    }

    fn batch(id: &str) -> ExternalImportBatch {
        ExternalImportBatch {
            batch_id: ExternalImportBatchId::new(id),
            source_id: None,
            adapter_id: ExternalImportAdapterId::new("hunting_box_directory_v1"),
            source_fingerprint: "private-fingerprint".to_owned(),
            scan_status: ExternalImportScanStatus::Pending,
            import_status: ExternalImportBatchImportStatus::Pending,
            created_at_unix_millis: 1,
        }
    }

    fn candidate(batch_id: &ExternalImportBatchId, id: &str) -> ExternalImportCandidate {
        ExternalImportCandidate {
            batch_id: batch_id.clone(),
            candidate_id: ExternalImportCandidateId::new(id),
            source_item_key_hash: format!("private-item-key-{id}"),
            content_fingerprint: format!("sha256:{id}"),
            metadata_hint: ExternalImportMetadataHint::default(),
            resource_usage: ExternalImportResourceUsage::default(),
            preview_status: ExternalImportCandidateStatus::Ready,
            conflict_kind: ExternalImportConflictKind::None,
        }
    }
}
