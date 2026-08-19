use std::collections::BTreeMap;

use super::{
    ExternalImportBatch, ExternalImportBatchError, ExternalImportBatchId,
    ExternalImportBatchService, ExternalImportCandidate, ExternalImportSelection,
    ExternalImportSelectionDecision, ExternalImportSelectionId, ExternalImportSelectionStatus,
    MAX_EXTERNAL_IMPORT_PREVIEW_LIMIT,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportPreviewPage {
    pub batch: ExternalImportBatch,
    pub selection: Option<ExternalImportSelection>,
    pub candidates: Vec<ExternalImportPreviewCandidate>,
    pub total_count: usize,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportPreviewCandidate {
    pub candidate: ExternalImportCandidate,
    pub selected: bool,
    pub selection_decision: Option<ExternalImportSelectionDecision>,
}

impl ExternalImportBatchService {
    pub fn get_preview(
        &self,
        batch_id: &ExternalImportBatchId,
        selection_id: Option<&ExternalImportSelectionId>,
        offset: usize,
        limit: usize,
    ) -> Result<ExternalImportPreviewPage, ExternalImportBatchError> {
        if !(1..=MAX_EXTERNAL_IMPORT_PREVIEW_LIMIT).contains(&limit) {
            return Err(ExternalImportBatchError::PreviewPageInvalid);
        }

        let batch = self.get_batch(batch_id)?;
        let selection = selection_id
            .map(|selection_id| {
                let mut selection = self.get_selection(selection_id)?;
                if selection.batch_id != batch.batch_id {
                    return Err(ExternalImportBatchError::SelectionUnavailable);
                }
                if selection.status == ExternalImportSelectionStatus::Editing
                    && self.now_unix_millis()? >= selection.expires_at_unix_millis
                {
                    selection.status = ExternalImportSelectionStatus::Expired;
                }
                Ok(selection)
            })
            .transpose()?;
        let selection_entries = selection
            .as_ref()
            .map(|selection| {
                selection
                    .entries
                    .iter()
                    .map(|entry| (entry.candidate_id.clone(), entry.decision.clone()))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let page = self
            .batch_repository
            .list_candidates_page(batch_id, offset, limit)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        let candidates = page
            .candidates
            .into_iter()
            .map(|candidate| {
                let selection_decision = selection_entries.get(&candidate.candidate_id).cloned();
                ExternalImportPreviewCandidate {
                    selected: selection_decision.is_some(),
                    selection_decision: selection_decision.flatten(),
                    candidate,
                }
            })
            .collect();

        Ok(ExternalImportPreviewPage {
            batch,
            selection,
            candidates,
            total_count: page.total_count,
            next_offset: page.next_offset,
        })
    }
}
