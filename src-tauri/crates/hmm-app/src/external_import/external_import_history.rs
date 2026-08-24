use super::{ExternalImportBatch, ExternalImportBatchError, ExternalImportBatchService};
use hmm_core::{
    ExternalImportItemStatusCounts, EXTERNAL_IMPORT_HISTORY_MAX_IMPORTED_BATCHES,
    EXTERNAL_IMPORT_HISTORY_MAX_SCAN_ONLY_BATCHES, EXTERNAL_IMPORT_HISTORY_SCAN_ONLY_TTL_MILLIS,
};
use hmm_ports::ExternalImportBatchRetentionRequest;

pub const DEFAULT_EXTERNAL_IMPORT_HISTORY_LIMIT: usize = 20;
pub const MAX_EXTERNAL_IMPORT_HISTORY_LIMIT: usize = 50;

/// 跨批次历史条目。`batch` 仍含 `source_fingerprint`,只允许在 app 内流转,
/// DTO 层负责剥离。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportHistoryEntry {
    pub batch: ExternalImportBatch,
    pub candidate_count: usize,
    pub result_counts: ExternalImportItemStatusCounts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportHistoryPage {
    pub entries: Vec<ExternalImportHistoryEntry>,
    pub total_count: usize,
    pub next_offset: Option<usize>,
}

impl ExternalImportBatchService {
    pub fn list_history(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<ExternalImportHistoryPage, ExternalImportBatchError> {
        if !(1..=MAX_EXTERNAL_IMPORT_HISTORY_LIMIT).contains(&limit) {
            return Err(ExternalImportBatchError::HistoryPageInvalid);
        }
        let page = self
            .batch_repository
            .list_batch_history_page(offset, limit)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;

        Ok(ExternalImportHistoryPage {
            entries: page
                .entries
                .into_iter()
                .map(|entry| ExternalImportHistoryEntry {
                    batch: entry.batch,
                    candidate_count: entry.candidate_count,
                    result_counts: entry.result_counts,
                })
                .collect(),
            total_count: page.total_count,
            next_offset: page.next_offset,
        })
    }

    /// 启动期一次性保留清理:不做来源 I/O、不创建任务、绝不触碰 running 批次。
    /// 失败由调用方降级处理,不得阻断启动。
    pub fn prune_batch_history(&self) -> Result<usize, ExternalImportBatchError> {
        let now = self.now_unix_millis()?;
        let outcome = self
            .batch_repository
            .prune_batches(ExternalImportBatchRetentionRequest {
                max_imported_batches: EXTERNAL_IMPORT_HISTORY_MAX_IMPORTED_BATCHES,
                max_scan_only_batches: EXTERNAL_IMPORT_HISTORY_MAX_SCAN_ONLY_BATCHES,
                scan_only_expires_before_unix_millis: now
                    .saturating_sub(EXTERNAL_IMPORT_HISTORY_SCAN_ONLY_TTL_MILLIS),
            })
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        Ok(outcome.removed_batches)
    }
}
