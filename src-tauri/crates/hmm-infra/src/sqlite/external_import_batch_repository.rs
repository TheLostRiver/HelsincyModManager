use anyhow::{anyhow, ensure, Context, Result};
use hmm_core::{
    ExternalImportBatch, ExternalImportBatchId, ExternalImportBatchImportStatus,
    ExternalImportCandidate, ExternalImportItemResult, ExternalImportItemStatus,
    ExternalImportItemStatusCounts, ExternalImportScanStatus, ExternalImportSelection,
    ExternalImportSelectionId,
};
use hmm_ports::{
    ExternalImportBatchHistoryEntry, ExternalImportBatchHistoryPage, ExternalImportBatchRepository,
    ExternalImportBatchRetentionOutcome, ExternalImportBatchRetentionRequest,
    ExternalImportCandidatePage, ExternalImportItemResultDetailPage, ExternalImportItemResultPage,
    ExternalImportItemResultRecord, ExternalImportSealAndStartRequest,
    ExternalImportSealAndStartResult, ExternalImportSelectionCompareAndSwapRequest,
    ExternalImportSelectionCompareAndSwapResult,
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
            "INSERT INTO external_import_batches (batch_id, batch_json, created_at)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![
                batch.batch_id.as_str(),
                batch_json,
                batch_created_at(batch)?
            ],
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
                "UPDATE external_import_batches
                 SET batch_json = ?2, created_at = ?3
                 WHERE batch_id = ?1",
                rusqlite::params![
                    batch.batch_id.as_str(),
                    batch_json,
                    batch_created_at(batch)?
                ],
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
                "UPDATE external_import_batches
                 SET batch_json = ?2, created_at = ?3
                 WHERE batch_id = ?1",
                rusqlite::params![
                    batch.batch_id.as_str(),
                    batch_json,
                    batch_created_at(batch)?
                ],
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
                "UPDATE external_import_batches
                 SET batch_json = ?2, created_at = ?3
                 WHERE batch_id = ?1",
                rusqlite::params![
                    batch.batch_id.as_str(),
                    batch_json,
                    batch_created_at(&batch)?
                ],
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
                "UPDATE external_import_batches
                 SET batch_json = ?2, created_at = ?3
                 WHERE batch_id = ?1",
                rusqlite::params![
                    batch.batch_id.as_str(),
                    batch_json,
                    batch_created_at(&batch)?
                ],
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
                    (batch_id, candidate_id, ordinal, result_json, status)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(batch_id, candidate_id) DO UPDATE SET
                    ordinal = excluded.ordinal,
                    result_json = excluded.result_json,
                    status = excluded.status",
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
                    result.status.as_str(),
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

    fn list_item_result_details_page(
        &self,
        batch_id: &ExternalImportBatchId,
        offset: usize,
        limit: usize,
    ) -> Result<ExternalImportItemResultDetailPage> {
        ensure!(
            limit > 0,
            "external import result detail page limit must be positive"
        );
        let offset_param = i64::try_from(offset)
            .context("external import result detail page offset is too large")?;
        let limit_param = i64::try_from(limit)
            .context("external import result detail page limit is too large")?;
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
        // LEFT JOIN:候选行意外缺失时该结果仍在页内(显示名降级为空),
        // 保证页覆盖与 total_count 一致,不静默丢行。
        let mut statement = conn
            .prepare(
                "SELECT r.result_json, c.candidate_json
                 FROM external_import_item_results r
                 LEFT JOIN external_import_candidates c
                   ON c.batch_id = r.batch_id AND c.candidate_id = r.candidate_id
                 WHERE r.batch_id = ?1
                 ORDER BY r.ordinal ASC
                 LIMIT ?2 OFFSET ?3",
            )
            .context("failed to prepare external import result detail page")?;
        let rows = statement
            .query_map(
                rusqlite::params![batch_id.as_str(), limit_param, offset_param],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .context("failed to query external import result detail page")?;
        let mut records = Vec::new();
        for row in rows {
            let (result_json, candidate_json) =
                row.context("failed to read external import result detail row")?;
            let result: ExternalImportItemResult =
                deserialize(&result_json, "external import item result")?;
            // 只携带受限显示名;候选整体(含 digest/key)不出仓储层。
            let display_name = candidate_json
                .as_deref()
                .map(|value| {
                    deserialize::<ExternalImportCandidate>(value, "external import candidate")
                })
                .transpose()?
                .and_then(|candidate| candidate.metadata_hint.display_name);
            records.push(ExternalImportItemResultRecord {
                result,
                display_name,
            });
        }
        let next_offset = offset
            .checked_add(records.len())
            .filter(|next| *next < total_count);

        Ok(ExternalImportItemResultDetailPage {
            records,
            total_count,
            next_offset,
        })
    }

    fn list_batch_history_page(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<ExternalImportBatchHistoryPage> {
        ensure!(
            limit > 0,
            "external import history page limit must be positive"
        );
        let offset_param =
            i64::try_from(offset).context("external import history page offset is too large")?;
        let limit_param =
            i64::try_from(limit).context("external import history page limit is too large")?;
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin external import history transaction")?;
        let total_count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM external_import_batches", [], |row| {
                row.get(0)
            })
            .context("failed to count external import batches")?;
        let total_count =
            usize::try_from(total_count).context("external import batch count is invalid")?;
        let mut statement = transaction
            .prepare(
                "SELECT batch_json
                 FROM external_import_batches
                 ORDER BY created_at IS NULL ASC, created_at DESC, batch_id ASC
                 LIMIT ?1 OFFSET ?2",
            )
            .context("failed to prepare external import history page")?;
        let rows = statement
            .query_map(rusqlite::params![limit_param, offset_param], |row| {
                row.get::<_, String>(0)
            })
            .context("failed to query external import history page")?;
        let batches: Vec<ExternalImportBatch> = deserialize_rows(rows, "external import batch")?;
        drop(statement);

        let batch_ids: Vec<&str> = batches
            .iter()
            .map(|batch| batch.batch_id.as_str())
            .collect();
        let candidate_counts = candidate_counts_by_batch(&transaction, &batch_ids)?;
        let result_counts = item_result_counts_by_batch(&transaction, &batch_ids)?;
        transaction
            .commit()
            .context("failed to commit external import history read")?;

        let next_offset = offset
            .checked_add(batches.len())
            .filter(|next| *next < total_count);
        let entries = batches
            .into_iter()
            .map(|batch| {
                let candidate_count = candidate_counts
                    .get(batch.batch_id.as_str())
                    .copied()
                    .unwrap_or(0);
                let result_counts = result_counts
                    .get(batch.batch_id.as_str())
                    .copied()
                    .unwrap_or_default();
                ExternalImportBatchHistoryEntry {
                    batch,
                    candidate_count,
                    result_counts,
                }
            })
            .collect();

        Ok(ExternalImportBatchHistoryPage {
            entries,
            total_count,
            next_offset,
        })
    }

    fn prune_batches(
        &self,
        request: ExternalImportBatchRetentionRequest,
    ) -> Result<ExternalImportBatchRetentionOutcome> {
        let conn = self.lock_db()?;
        let transaction = conn
            .unchecked_transaction()
            .context("failed to begin external import retention transaction")?;
        let mut statement = transaction
            .prepare(
                "SELECT batch_json
                 FROM external_import_batches
                 ORDER BY created_at IS NULL ASC, created_at DESC, batch_id ASC",
            )
            .context("failed to prepare external import retention query")?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .context("failed to query external import retention batches")?;
        let batches: Vec<ExternalImportBatch> = deserialize_rows(rows, "external import batch")?;
        drop(statement);

        let mut imported_kept = 0_usize;
        let mut scan_only_kept = 0_usize;
        let mut removable: Vec<String> = Vec::new();
        for batch in batches {
            match batch.import_status {
                // 进行中的批次永不清理:结果仍在增量落库。
                ExternalImportBatchImportStatus::Running => {}
                ExternalImportBatchImportStatus::Completed
                | ExternalImportBatchImportStatus::CompletedWithErrors
                | ExternalImportBatchImportStatus::Failed
                | ExternalImportBatchImportStatus::Cancelled => {
                    if imported_kept < request.max_imported_batches {
                        imported_kept += 1;
                    } else {
                        removable.push(batch.batch_id.as_str().to_owned());
                    }
                }
                ExternalImportBatchImportStatus::Pending => {
                    let expired =
                        batch.created_at_unix_millis < request.scan_only_expires_before_unix_millis;
                    if !expired && scan_only_kept < request.max_scan_only_batches {
                        scan_only_kept += 1;
                    } else {
                        removable.push(batch.batch_id.as_str().to_owned());
                    }
                }
            }
        }

        let mut removed_batches = 0_usize;
        for chunk in removable.chunks(RETENTION_DELETE_CHUNK) {
            let sql = format!(
                "DELETE FROM external_import_batches WHERE batch_id IN ({})",
                vec!["?"; chunk.len()].join(", ")
            );
            let removed = transaction
                .execute(&sql, rusqlite::params_from_iter(chunk.iter()))
                .context("failed to delete external import retention batches")?;
            ensure!(
                removed == chunk.len(),
                "external import retention delete count mismatch"
            );
            removed_batches = removed_batches
                .checked_add(removed)
                .ok_or_else(|| anyhow!("external import retention count overflow"))?;
        }
        transaction
            .commit()
            .context("failed to commit external import retention")?;

        Ok(ExternalImportBatchRetentionOutcome { removed_batches })
    }
}

/// IN 子句删除的分块上限,远低于 SQLite 变量数限制。
const RETENTION_DELETE_CHUNK: usize = 500;

fn batch_created_at(batch: &ExternalImportBatch) -> Result<i64> {
    i64::try_from(batch.created_at_unix_millis)
        .context("external import batch creation time is invalid")
}

fn candidate_counts_by_batch(
    transaction: &Transaction<'_>,
    batch_ids: &[&str],
) -> Result<std::collections::BTreeMap<String, usize>> {
    let mut counts = std::collections::BTreeMap::new();
    if batch_ids.is_empty() {
        return Ok(counts);
    }
    let sql = format!(
        "SELECT batch_id, COUNT(*)
         FROM external_import_candidates
         WHERE batch_id IN ({})
         GROUP BY batch_id",
        vec!["?"; batch_ids.len()].join(", ")
    );
    let mut statement = transaction
        .prepare(&sql)
        .context("failed to prepare external import candidate count query")?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(batch_ids.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .context("failed to query external import candidate counts")?;
    for row in rows {
        let (batch_id, count) =
            row.context("failed to read external import candidate count row")?;
        let count = usize::try_from(count).context("external import candidate count is invalid")?;
        counts.insert(batch_id, count);
    }
    Ok(counts)
}

fn item_result_counts_by_batch(
    transaction: &Transaction<'_>,
    batch_ids: &[&str],
) -> Result<std::collections::BTreeMap<String, ExternalImportItemStatusCounts>> {
    let mut counts: std::collections::BTreeMap<String, ExternalImportItemStatusCounts> =
        std::collections::BTreeMap::new();
    if batch_ids.is_empty() {
        return Ok(counts);
    }
    let sql = format!(
        "SELECT batch_id, status, COUNT(*)
         FROM external_import_item_results
         WHERE batch_id IN ({})
         GROUP BY batch_id, status",
        vec!["?"; batch_ids.len()].join(", ")
    );
    let mut statement = transaction
        .prepare(&sql)
        .context("failed to prepare external import result count query")?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(batch_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .context("failed to query external import result counts")?;
    let mut null_status_batches: Vec<String> = Vec::new();
    for row in rows {
        let (batch_id, status, count) =
            row.context("failed to read external import result count row")?;
        let Some(status) = status else {
            null_status_batches.push(batch_id);
            continue;
        };
        // 无法映射的状态字符串整体报错:静默漏计会伪造「计数与明细一致」的信任基础。
        let status = ExternalImportItemStatus::parse(&status)
            .ok_or_else(|| anyhow!("external import item result status is invalid"))?;
        let count = u64::try_from(count).context("external import result count is invalid")?;
        counts
            .entry(batch_id)
            .or_default()
            .add(status, count)
            .ok_or_else(|| anyhow!("external import result count overflow"))?;
    }
    drop(statement);
    for batch_id in null_status_batches {
        add_null_status_results_from_json(transaction, &batch_id, &mut counts)?;
    }
    Ok(counts)
}

/// 派生 status 列残留 NULL 的行(预期为零)按 result_json 权威事实归类,不静默丢弃。
fn add_null_status_results_from_json(
    transaction: &Transaction<'_>,
    batch_id: &str,
    counts: &mut std::collections::BTreeMap<String, ExternalImportItemStatusCounts>,
) -> Result<()> {
    let mut statement = transaction
        .prepare(
            "SELECT result_json
             FROM external_import_item_results
             WHERE batch_id = ?1 AND status IS NULL",
        )
        .context("failed to prepare external import result fallback query")?;
    let rows = statement
        .query_map(rusqlite::params![batch_id], |row| row.get::<_, String>(0))
        .context("failed to query external import result fallback rows")?;
    let results: Vec<ExternalImportItemResult> =
        deserialize_rows(rows, "external import item result")?;
    for result in results {
        counts
            .entry(batch_id.to_owned())
            .or_default()
            .add(result.status, 1)
            .ok_or_else(|| anyhow!("external import result count overflow"))?;
    }
    Ok(())
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

    #[test]
    fn history_page_orders_by_creation_time_and_breaks_ties_by_batch_id() {
        let temporary = tempfile::tempdir().expect("temporary database directory");
        let db = Arc::new(Mutex::new(
            crate::open_database(&temporary.path().join("hmm.db")).expect("open database"),
        ));
        let repository = SqliteExternalImportBatchRepository::new(db);
        let mut tie_b = batch("batch-b");
        tie_b.created_at_unix_millis = 100;
        let mut tie_a = batch("batch-a");
        tie_a.created_at_unix_millis = 100;
        let mut newest = batch("batch-c");
        newest.created_at_unix_millis = 200;
        for seeded in [&tie_b, &tie_a, &newest] {
            repository.create_batch(seeded).expect("create batch");
        }

        let first_page = repository
            .list_batch_history_page(0, 2)
            .expect("first history page");
        let second_page = repository
            .list_batch_history_page(2, 2)
            .expect("second history page");

        assert_eq!(first_page.total_count, 3);
        assert_eq!(first_page.next_offset, Some(2));
        assert_eq!(
            first_page
                .entries
                .iter()
                .map(|entry| entry.batch.batch_id.as_str())
                .collect::<Vec<_>>(),
            ["batch-c", "batch-a"]
        );
        assert_eq!(second_page.next_offset, None);
        assert_eq!(
            second_page
                .entries
                .iter()
                .map(|entry| entry.batch.batch_id.as_str())
                .collect::<Vec<_>>(),
            ["batch-b"]
        );
    }

    #[test]
    fn history_page_counts_match_a_rust_recount_of_item_results() {
        let temporary = tempfile::tempdir().expect("temporary database directory");
        let db = Arc::new(Mutex::new(
            crate::open_database(&temporary.path().join("hmm.db")).expect("open database"),
        ));
        let repository = SqliteExternalImportBatchRepository::new(db);
        let mut batch = batch("batch-counts");
        batch.scan_status = ExternalImportScanStatus::Completed;
        let candidates = vec![
            candidate(&batch.batch_id, "candidate-1"),
            candidate(&batch.batch_id, "candidate-2"),
            candidate(&batch.batch_id, "candidate-3"),
        ];
        repository.create_batch(&batch).expect("create batch");
        repository
            .save_scan_result(&batch, &candidates)
            .expect("save scan result");
        repository
            .append_item_results(
                &batch.batch_id,
                &[
                    item_result("candidate-1", ExternalImportItemStatus::Imported),
                    item_result("candidate-2", ExternalImportItemStatus::Failed),
                ],
            )
            .expect("append first chunk");
        repository
            .append_item_results(
                &batch.batch_id,
                &[item_result(
                    "candidate-3",
                    ExternalImportItemStatus::Skipped,
                )],
            )
            .expect("append second chunk");

        let page = repository
            .list_batch_history_page(0, 10)
            .expect("history page");

        // 这条断言是「允许 migration 用 json_extract」的对价:派生列聚合必须与
        // result_json 权威事实的逐行重算完全一致,serde 字段名一旦漂移立即翻红。
        let mut recounted = ExternalImportItemStatusCounts::default();
        for result in repository
            .list_item_results(&batch.batch_id)
            .expect("list item results")
        {
            recounted.add(result.status, 1).expect("recount");
        }
        assert_eq!(page.entries[0].result_counts, recounted);
        assert_eq!(page.entries[0].result_counts.total(), 3);
        assert_eq!(page.entries[0].candidate_count, 3);
    }

    #[test]
    fn history_counts_fall_back_to_result_json_when_the_status_column_is_null() {
        let temporary = tempfile::tempdir().expect("temporary database directory");
        let db = Arc::new(Mutex::new(
            crate::open_database(&temporary.path().join("hmm.db")).expect("open database"),
        ));
        let repository = SqliteExternalImportBatchRepository::new(Arc::clone(&db));
        let mut batch = batch("batch-null-status");
        batch.scan_status = ExternalImportScanStatus::Completed;
        let candidates = vec![
            candidate(&batch.batch_id, "candidate-1"),
            candidate(&batch.batch_id, "candidate-2"),
        ];
        repository.create_batch(&batch).expect("create batch");
        repository
            .save_scan_result(&batch, &candidates)
            .expect("save scan result");
        repository
            .append_item_results(
                &batch.batch_id,
                &[
                    item_result("candidate-1", ExternalImportItemStatus::Imported),
                    item_result("candidate-2", ExternalImportItemStatus::Blocked),
                ],
            )
            .expect("append results");
        db.lock()
            .expect("database lock")
            .execute(
                "UPDATE external_import_item_results SET status = NULL WHERE batch_id = ?1",
                rusqlite::params![batch.batch_id.as_str()],
            )
            .expect("null out derived status");

        let page = repository
            .list_batch_history_page(0, 10)
            .expect("history page");

        assert_eq!(page.entries[0].result_counts.imported, 1);
        assert_eq!(page.entries[0].result_counts.blocked, 1);
        assert_eq!(page.entries[0].result_counts.total(), 2);
    }

    #[test]
    fn retry_upsert_moves_the_result_between_status_buckets() {
        let temporary = tempfile::tempdir().expect("temporary database directory");
        let db = Arc::new(Mutex::new(
            crate::open_database(&temporary.path().join("hmm.db")).expect("open database"),
        ));
        let repository = SqliteExternalImportBatchRepository::new(db);
        let mut batch = batch("batch-retry-buckets");
        batch.scan_status = ExternalImportScanStatus::Completed;
        let candidate = candidate(&batch.batch_id, "candidate-1");
        repository.create_batch(&batch).expect("create batch");
        repository
            .save_scan_result(&batch, std::slice::from_ref(&candidate))
            .expect("save scan result");
        repository
            .append_item_results(
                &batch.batch_id,
                &[item_result("candidate-1", ExternalImportItemStatus::Failed)],
            )
            .expect("append failed result");
        repository
            .append_item_results(
                &batch.batch_id,
                &[item_result(
                    "candidate-1",
                    ExternalImportItemStatus::Imported,
                )],
            )
            .expect("upsert retried result");

        let page = repository
            .list_batch_history_page(0, 10)
            .expect("history page");

        assert_eq!(page.entries[0].result_counts.failed, 0);
        assert_eq!(page.entries[0].result_counts.imported, 1);
        assert_eq!(page.entries[0].result_counts.total(), 1);
    }

    #[test]
    fn result_detail_page_joins_candidate_display_names_in_scan_order() {
        let temporary = tempfile::tempdir().expect("temporary database directory");
        let db = Arc::new(Mutex::new(
            crate::open_database(&temporary.path().join("hmm.db")).expect("open database"),
        ));
        let repository = SqliteExternalImportBatchRepository::new(Arc::clone(&db));
        let mut batch = batch("batch-detail-names");
        batch.scan_status = ExternalImportScanStatus::Completed;
        let mut first = candidate(&batch.batch_id, "candidate-1");
        first.metadata_hint.display_name = Some("第一项".to_owned());
        let second = candidate(&batch.batch_id, "candidate-2");
        repository.create_batch(&batch).expect("create batch");
        repository
            .save_scan_result(&batch, &[first, second])
            .expect("save scan result");
        // 乱序落库,明细页仍须按候选扫描序返回。
        repository
            .append_item_results(
                &batch.batch_id,
                &[item_result("candidate-2", ExternalImportItemStatus::Failed)],
            )
            .expect("append second result");
        repository
            .append_item_results(
                &batch.batch_id,
                &[item_result(
                    "candidate-1",
                    ExternalImportItemStatus::Imported,
                )],
            )
            .expect("append first result");

        let page = repository
            .list_item_result_details_page(&batch.batch_id, 0, 10)
            .expect("result detail page");

        assert_eq!(page.total_count, 2);
        assert_eq!(
            page.records
                .iter()
                .map(|record| {
                    (
                        record.result.candidate_id.as_str(),
                        record.display_name.as_deref(),
                    )
                })
                .collect::<Vec<_>>(),
            [("candidate-1", Some("第一项")), ("candidate-2", None)]
        );

        // 候选行意外缺失时结果行不得静默消失,显示名降级为空。
        db.lock()
            .expect("database lock")
            .execute(
                "DELETE FROM external_import_candidates
                 WHERE batch_id = ?1 AND candidate_id = 'candidate-1'",
                rusqlite::params![batch.batch_id.as_str()],
            )
            .expect("drop candidate row");
        let degraded = repository
            .list_item_result_details_page(&batch.batch_id, 0, 10)
            .expect("degraded result detail page");
        assert_eq!(degraded.total_count, 2);
        assert_eq!(degraded.records.len(), 2);
        assert_eq!(degraded.records[0].display_name, None);
    }

    #[test]
    fn retention_keeps_running_batches_and_leaves_no_orphans() {
        let temporary = tempfile::tempdir().expect("temporary database directory");
        let db = Arc::new(Mutex::new(
            crate::open_database(&temporary.path().join("hmm.db")).expect("open database"),
        ));
        let repository = SqliteExternalImportBatchRepository::new(Arc::clone(&db));
        let mut running = batch("batch-running");
        running.created_at_unix_millis = 10;
        running.import_status = ExternalImportBatchImportStatus::Running;
        let mut kept_imported = batch("batch-imported-kept");
        kept_imported.created_at_unix_millis = 300;
        kept_imported.import_status = ExternalImportBatchImportStatus::Completed;
        let mut removed_imported = batch("batch-imported-removed");
        removed_imported.created_at_unix_millis = 200;
        removed_imported.scan_status = ExternalImportScanStatus::Completed;
        removed_imported.import_status = ExternalImportBatchImportStatus::CompletedWithErrors;
        let mut expired_scan_only = batch("batch-scan-only-expired");
        expired_scan_only.created_at_unix_millis = 1;
        for seeded in [
            &running,
            &kept_imported,
            &removed_imported,
            &expired_scan_only,
        ] {
            repository.create_batch(seeded).expect("create batch");
        }
        let removed_candidate = candidate(&removed_imported.batch_id, "candidate-1");
        repository
            .save_scan_result(&removed_imported, std::slice::from_ref(&removed_candidate))
            .expect("save scan result");
        repository
            .create_selection(&ExternalImportSelection::new(
                ExternalImportSelectionId::new("selection-removed"),
                removed_imported.batch_id.clone(),
                1000,
            ))
            .expect("create selection");
        repository
            .append_item_results(
                &removed_imported.batch_id,
                &[item_result("candidate-1", ExternalImportItemStatus::Failed)],
            )
            .expect("append result");

        let outcome = repository
            .prune_batches(ExternalImportBatchRetentionRequest {
                max_imported_batches: 1,
                max_scan_only_batches: 10,
                scan_only_expires_before_unix_millis: 5,
            })
            .expect("prune batches");

        assert_eq!(outcome.removed_batches, 2);
        assert!(repository
            .get_batch(&running.batch_id)
            .expect("read running batch")
            .is_some());
        assert!(repository
            .get_batch(&kept_imported.batch_id)
            .expect("read kept batch")
            .is_some());
        assert!(repository
            .get_batch(&removed_imported.batch_id)
            .expect("read removed batch")
            .is_none());
        assert!(repository
            .get_batch(&expired_scan_only.batch_id)
            .expect("read expired batch")
            .is_none());
        // 级联必须把候选/selection/结果一并清掉,不留孤儿行。
        let orphan_rows: (i64, i64, i64) = db
            .lock()
            .expect("database lock")
            .query_row(
                "SELECT (SELECT COUNT(*) FROM external_import_candidates WHERE batch_id = ?1),
                        (SELECT COUNT(*) FROM external_import_selections WHERE batch_id = ?1),
                        (SELECT COUNT(*) FROM external_import_item_results WHERE batch_id = ?1)",
                rusqlite::params![removed_imported.batch_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("count orphan rows");
        assert_eq!(orphan_rows, (0, 0, 0));
    }

    #[test]
    fn retention_never_removes_within_the_imported_batch_cap() {
        let temporary = tempfile::tempdir().expect("temporary database directory");
        let db = Arc::new(Mutex::new(
            crate::open_database(&temporary.path().join("hmm.db")).expect("open database"),
        ));
        let repository = SqliteExternalImportBatchRepository::new(db);
        for (index, id) in ["batch-1", "batch-2", "batch-3"].iter().enumerate() {
            let mut seeded = batch(id);
            seeded.created_at_unix_millis = 100 + index as u64;
            seeded.import_status = ExternalImportBatchImportStatus::Completed;
            repository.create_batch(&seeded).expect("create batch");
        }

        let outcome = repository
            .prune_batches(ExternalImportBatchRetentionRequest {
                max_imported_batches: 50,
                max_scan_only_batches: 10,
                scan_only_expires_before_unix_millis: u64::MAX,
            })
            .expect("prune batches");

        assert_eq!(outcome.removed_batches, 0);
        assert_eq!(
            repository
                .list_batch_history_page(0, 10)
                .expect("history page")
                .total_count,
            3
        );
    }

    fn item_result(
        candidate_id: &str,
        status: ExternalImportItemStatus,
    ) -> ExternalImportItemResult {
        ExternalImportItemResult {
            candidate_id: ExternalImportCandidateId::new(candidate_id),
            status,
            reason_code: None,
            imported_mod_id: None,
            retryable: false,
        }
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
