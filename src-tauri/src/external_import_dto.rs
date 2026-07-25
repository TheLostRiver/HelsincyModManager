use crate::dto::TaskStartedDto;
use hmm_app::{
    ExternalImportBatchLaunch, ExternalImportPreviewCandidate, ExternalImportPreviewPage,
    ExternalImportResultPage, ExternalImportScanLaunch,
};
use hmm_core::{
    ExternalImportBatch, ExternalImportBatchImportStatus, ExternalImportCandidateStatus,
    ExternalImportConflictKind, ExternalImportConflictResolution, ExternalImportItemResult,
    ExternalImportItemStatus, ExternalImportMetadataHint, ExternalImportReasonCode,
    ExternalImportResourceUsage, ExternalImportScanStatus, ExternalImportSelection,
    ExternalImportSelectionDecision, ExternalImportSelectionMutationResult,
    ExternalImportSelectionStatus, ExternalImportSource,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportSourceDto {
    pub source_id: String,
    pub adapter_id: String,
    pub display_label: String,
    pub expires_at_unix_millis: u64,
}

impl From<ExternalImportSource> for ExternalImportSourceDto {
    fn from(source: ExternalImportSource) -> Self {
        Self {
            source_id: source.source_id.as_str().to_owned(),
            adapter_id: source.adapter_id.as_str().to_owned(),
            display_label: source.display_label,
            expires_at_unix_millis: source.expires_at_unix_millis,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportScanStartedDto {
    pub task: TaskStartedDto,
    pub batch_id: String,
}

impl From<&ExternalImportScanLaunch> for ExternalImportScanStartedDto {
    fn from(launch: &ExternalImportScanLaunch) -> Self {
        Self {
            task: launch.task.clone().into(),
            batch_id: launch.batch_id.as_str().to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportBatchStartedDto {
    pub task: TaskStartedDto,
    pub batch_id: String,
}

impl From<&ExternalImportBatchLaunch> for ExternalImportBatchStartedDto {
    fn from(launch: &ExternalImportBatchLaunch) -> Self {
        Self {
            task: launch.task.clone().into(),
            batch_id: launch.batch_id.as_str().to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportSelectionDto {
    pub selection_id: String,
    pub revision: u64,
    pub status: ExternalImportSelectionStatusDto,
    pub selected_count: usize,
    pub selected_resource_usage: ExternalImportResourceUsageDto,
    pub expires_at_unix_millis: u64,
}

impl From<ExternalImportSelection> for ExternalImportSelectionDto {
    fn from(selection: ExternalImportSelection) -> Self {
        Self {
            selection_id: selection.selection_id.as_str().to_owned(),
            revision: selection.revision,
            status: selection.status.into(),
            selected_count: selection.selected_count(),
            selected_resource_usage: selection.selected_resource_usage.into(),
            expires_at_unix_millis: selection.expires_at_unix_millis,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportSelectionMutationResultDto {
    pub revision: u64,
    pub selected_count: usize,
    pub selected_resource_usage: ExternalImportResourceUsageDto,
}

impl From<ExternalImportSelectionMutationResult> for ExternalImportSelectionMutationResultDto {
    fn from(result: ExternalImportSelectionMutationResult) -> Self {
        Self {
            revision: result.revision,
            selected_count: result.selected_count,
            selected_resource_usage: result.selected_resource_usage.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportSelectionMutationInputDto {
    pub candidate_id: String,
    pub selected: bool,
    pub decision: Option<ExternalImportSelectionDecisionInputDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportSelectionDecisionInputDto {
    pub conflict_resolution: Option<ExternalImportConflictResolutionInputDto>,
    pub category_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportConflictResolutionInputDto {
    KeepBoth,
    IgnoreInvalidMetadata,
}

impl From<ExternalImportConflictResolutionInputDto> for ExternalImportConflictResolution {
    fn from(value: ExternalImportConflictResolutionInputDto) -> Self {
        match value {
            ExternalImportConflictResolutionInputDto::KeepBoth => Self::KeepBoth,
            ExternalImportConflictResolutionInputDto::IgnoreInvalidMetadata => {
                Self::IgnoreInvalidMetadata
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportSelectionDecisionDto {
    pub conflict_resolution: Option<ExternalImportConflictResolutionDto>,
    pub category_id: Option<String>,
}

impl From<ExternalImportSelectionDecision> for ExternalImportSelectionDecisionDto {
    fn from(decision: ExternalImportSelectionDecision) -> Self {
        Self {
            conflict_resolution: decision.conflict_resolution.map(Into::into),
            category_id: decision.category_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportConflictResolutionDto {
    KeepBoth,
    IgnoreInvalidMetadata,
}

impl From<ExternalImportConflictResolution> for ExternalImportConflictResolutionDto {
    fn from(value: ExternalImportConflictResolution) -> Self {
        match value {
            ExternalImportConflictResolution::KeepBoth => Self::KeepBoth,
            ExternalImportConflictResolution::IgnoreInvalidMetadata => {
                Self::IgnoreInvalidMetadata
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportSelectionStatusDto {
    Editing,
    Sealed,
    Expired,
}

impl From<ExternalImportSelectionStatus> for ExternalImportSelectionStatusDto {
    fn from(status: ExternalImportSelectionStatus) -> Self {
        match status {
            ExternalImportSelectionStatus::Editing => Self::Editing,
            ExternalImportSelectionStatus::Sealed => Self::Sealed,
            ExternalImportSelectionStatus::Expired => Self::Expired,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportResourceUsageDto {
    pub file_count: u64,
    pub source_bytes: u64,
    pub materialization_bytes: u64,
}

impl From<ExternalImportResourceUsage> for ExternalImportResourceUsageDto {
    fn from(usage: ExternalImportResourceUsage) -> Self {
        Self {
            file_count: usage.file_count,
            source_bytes: usage.source_bytes,
            materialization_bytes: usage.materialization_bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportPreviewPageDto {
    pub batch: ExternalImportPreviewBatchDto,
    pub selection: Option<ExternalImportSelectionDto>,
    pub candidates: Vec<ExternalImportPreviewCandidateDto>,
    pub total_count: usize,
    pub next_cursor: Option<String>,
}

impl From<ExternalImportPreviewPage> for ExternalImportPreviewPageDto {
    fn from(page: ExternalImportPreviewPage) -> Self {
        Self {
            batch: page.batch.into(),
            selection: page.selection.map(Into::into),
            candidates: page.candidates.into_iter().map(Into::into).collect(),
            total_count: page.total_count,
            next_cursor: page.next_offset.map(|offset| offset.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportBatchResultPageDto {
    pub batch: ExternalImportPreviewBatchDto,
    pub results: Vec<ExternalImportItemResultDto>,
    pub total_count: usize,
    pub next_cursor: Option<String>,
}

impl From<ExternalImportResultPage> for ExternalImportBatchResultPageDto {
    fn from(page: ExternalImportResultPage) -> Self {
        Self {
            batch: page.batch.into(),
            results: page.results.into_iter().map(Into::into).collect(),
            total_count: page.total_count,
            next_cursor: page.next_offset.map(|offset| offset.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportItemResultDto {
    pub candidate_id: String,
    pub status: ExternalImportItemStatusDto,
    pub reason_code: Option<String>,
    pub imported_mod_id: Option<String>,
    pub retryable: bool,
}

impl From<ExternalImportItemResult> for ExternalImportItemResultDto {
    fn from(result: ExternalImportItemResult) -> Self {
        Self {
            candidate_id: result.candidate_id.as_str().to_owned(),
            status: result.status.into(),
            reason_code: result
                .reason_code
                .map(ExternalImportReasonCode::as_str)
                .map(str::to_owned),
            imported_mod_id: result
                .imported_mod_id
                .map(|mod_id| mod_id.as_str().to_owned()),
            retryable: result.retryable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportItemStatusDto {
    Imported,
    AlreadyImported,
    Skipped,
    Blocked,
    Failed,
    Cancelled,
}

impl From<ExternalImportItemStatus> for ExternalImportItemStatusDto {
    fn from(status: ExternalImportItemStatus) -> Self {
        match status {
            ExternalImportItemStatus::Imported => Self::Imported,
            ExternalImportItemStatus::AlreadyImported => Self::AlreadyImported,
            ExternalImportItemStatus::Skipped => Self::Skipped,
            ExternalImportItemStatus::Blocked => Self::Blocked,
            ExternalImportItemStatus::Failed => Self::Failed,
            ExternalImportItemStatus::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportPreviewBatchDto {
    pub batch_id: String,
    pub adapter_id: String,
    pub scan_status: ExternalImportScanStatusDto,
    pub import_status: ExternalImportBatchImportStatusDto,
}

impl From<ExternalImportBatch> for ExternalImportPreviewBatchDto {
    fn from(batch: ExternalImportBatch) -> Self {
        Self {
            batch_id: batch.batch_id.as_str().to_owned(),
            adapter_id: batch.adapter_id.as_str().to_owned(),
            scan_status: batch.scan_status.into(),
            import_status: batch.import_status.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportPreviewCandidateDto {
    pub candidate_id: String,
    pub metadata: ExternalImportMetadataHintDto,
    pub file_count: u64,
    pub total_bytes: u64,
    pub preview_status: ExternalImportCandidateStatusDto,
    pub conflict_kind: ExternalImportConflictKindDto,
    pub reason_code: Option<String>,
    pub selected: bool,
    pub selection_decision: Option<ExternalImportSelectionDecisionDto>,
}

impl From<ExternalImportPreviewCandidate> for ExternalImportPreviewCandidateDto {
    fn from(preview: ExternalImportPreviewCandidate) -> Self {
        let candidate = preview.candidate;
        Self {
            candidate_id: candidate.candidate_id.as_str().to_owned(),
            metadata: candidate.metadata_hint.into(),
            file_count: candidate.resource_usage.file_count,
            total_bytes: candidate.resource_usage.source_bytes,
            preview_status: candidate.preview_status.into(),
            conflict_kind: candidate.conflict_kind.into(),
            reason_code: candidate
                .preview_status
                .reason_code()
                .map(|reason| reason.as_str().to_owned()),
            selected: preview.selected,
            selection_decision: preview.selection_decision.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImportMetadataHintDto {
    pub display_name: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub source_mod_type: Option<String>,
}

impl From<ExternalImportMetadataHint> for ExternalImportMetadataHintDto {
    fn from(metadata: ExternalImportMetadataHint) -> Self {
        Self {
            display_name: metadata.display_name,
            author: metadata.author,
            version: metadata.version,
            source_mod_type: metadata.source_mod_type,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportScanStatusDto {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl From<ExternalImportScanStatus> for ExternalImportScanStatusDto {
    fn from(status: ExternalImportScanStatus) -> Self {
        match status {
            ExternalImportScanStatus::Pending => Self::Pending,
            ExternalImportScanStatus::Running => Self::Running,
            ExternalImportScanStatus::Completed => Self::Completed,
            ExternalImportScanStatus::Failed => Self::Failed,
            ExternalImportScanStatus::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportBatchImportStatusDto {
    Pending,
    Running,
    Completed,
    CompletedWithErrors,
    Failed,
    Cancelled,
}

impl From<ExternalImportBatchImportStatus> for ExternalImportBatchImportStatusDto {
    fn from(status: ExternalImportBatchImportStatus) -> Self {
        match status {
            ExternalImportBatchImportStatus::Pending => Self::Pending,
            ExternalImportBatchImportStatus::Running => Self::Running,
            ExternalImportBatchImportStatus::Completed => Self::Completed,
            ExternalImportBatchImportStatus::CompletedWithErrors => Self::CompletedWithErrors,
            ExternalImportBatchImportStatus::Failed => Self::Failed,
            ExternalImportBatchImportStatus::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportCandidateStatusDto {
    Ready,
    AlreadyImported,
    DuplicateInBatch,
    NameCollision,
    StructureInvalid,
    MetadataInvalid,
    UnsupportedEntry,
    ResourceLimitExceeded,
    SourceUnreadable,
}

impl From<ExternalImportCandidateStatus> for ExternalImportCandidateStatusDto {
    fn from(status: ExternalImportCandidateStatus) -> Self {
        match status {
            ExternalImportCandidateStatus::Ready => Self::Ready,
            ExternalImportCandidateStatus::AlreadyImported => Self::AlreadyImported,
            ExternalImportCandidateStatus::DuplicateInBatch => Self::DuplicateInBatch,
            ExternalImportCandidateStatus::NameCollision => Self::NameCollision,
            ExternalImportCandidateStatus::StructureInvalid => Self::StructureInvalid,
            ExternalImportCandidateStatus::MetadataInvalid => Self::MetadataInvalid,
            ExternalImportCandidateStatus::UnsupportedEntry => Self::UnsupportedEntry,
            ExternalImportCandidateStatus::ResourceLimitExceeded => Self::ResourceLimitExceeded,
            ExternalImportCandidateStatus::SourceUnreadable => Self::SourceUnreadable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportConflictKindDto {
    None,
    ContentDuplicate,
    NameCollision,
}

impl From<ExternalImportConflictKind> for ExternalImportConflictKindDto {
    fn from(kind: ExternalImportConflictKind) -> Self {
        match kind {
            ExternalImportConflictKind::None => Self::None,
            ExternalImportConflictKind::ContentDuplicate => Self::ContentDuplicate,
            ExternalImportConflictKind::NameCollision => Self::NameCollision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        ExternalImportAdapterId, ExternalImportBatchId, ExternalImportCandidate,
        ExternalImportCandidateId, ExternalImportConflictKind, ExternalImportConflictResolution,
        ExternalImportItemResult, ExternalImportItemStatus, ExternalImportReasonCode,
        ExternalImportResourceUsage, ExternalImportSelectionDecision,
        ExternalImportSelectionEntry, ExternalImportSelectionId, ExternalImportSourceId, ModId,
    };

    #[test]
    fn preview_dto_omits_source_paths_and_private_digests() {
        let decision = ExternalImportSelectionDecision {
            conflict_resolution: Some(ExternalImportConflictResolution::KeepBoth),
            category_id: Some("category-safe".to_owned()),
        };
        let page = ExternalImportPreviewPage {
            batch: ExternalImportBatch {
                batch_id: ExternalImportBatchId::new("external-import-batch-1"),
                source_id: None,
                adapter_id: ExternalImportAdapterId::new("hunting_box_directory_v1"),
                source_fingerprint: "C:/private/source-fingerprint".to_owned(),
                scan_status: ExternalImportScanStatus::Completed,
                import_status: ExternalImportBatchImportStatus::Pending,
                created_at_unix_millis: 1,
            },
            selection: Some(ExternalImportSelection {
                selection_id: ExternalImportSelectionId::new("external-import-selection-1"),
                batch_id: ExternalImportBatchId::new("private-selection-batch-id"),
                revision: 2,
                status: ExternalImportSelectionStatus::Editing,
                entries: vec![ExternalImportSelectionEntry {
                    candidate_id: ExternalImportCandidateId::new("private-selection-entry"),
                    decision: Some(decision.clone()),
                    updated_at_unix_millis: 99,
                }],
                selected_resource_usage: ExternalImportResourceUsage {
                    file_count: 2,
                    source_bytes: 3,
                    materialization_bytes: 3,
                },
                expires_at_unix_millis: 100,
            }),
            candidates: vec![ExternalImportPreviewCandidate {
                candidate: ExternalImportCandidate {
                    batch_id: ExternalImportBatchId::new("external-import-batch-1"),
                    candidate_id: ExternalImportCandidateId::new("external-import-candidate-1"),
                    source_item_key_hash: "C:/private/source-item-key".to_owned(),
                    content_fingerprint: "sha256:private-content-fingerprint".to_owned(),
                    metadata_hint: ExternalImportMetadataHint {
                        display_name: Some("Fixture Mod".to_owned()),
                        author: None,
                        version: None,
                        source_mod_type: None,
                    },
                    resource_usage: ExternalImportResourceUsage {
                        file_count: 2,
                        source_bytes: 3,
                        materialization_bytes: 3,
                    },
                    preview_status: ExternalImportCandidateStatus::Ready,
                    conflict_kind: ExternalImportConflictKind::None,
                },
                selected: true,
                selection_decision: Some(decision),
            }],
            total_count: 1,
            next_offset: None,
        };

        let value = serde_json::to_value(ExternalImportPreviewPageDto::from(page))
            .expect("serialize external import preview dto");
        let serialized = value.to_string();

        assert_eq!(value["selection"]["selectionId"], "external-import-selection-1");
        assert_eq!(value["selection"]["selectedCount"], 1);
        assert_eq!(value["candidates"][0]["selected"], true);
        assert_eq!(
            value["candidates"][0]["selectionDecision"]["conflictResolution"],
            "keep_both"
        );
        assert_eq!(
            value["candidates"][0]["selectionDecision"]["categoryId"],
            "category-safe"
        );
        assert!(value["selection"].get("batchId").is_none());
        assert!(value["selection"].get("entries").is_none());
        assert!(value["selection"].get("updatedAtUnixMillis").is_none());
        assert!(serialized.contains("external-import-candidate-1"));
        assert!(!serialized.contains("C:/private"));
        assert!(!serialized.contains("private-content-fingerprint"));
        assert!(!serialized.contains("private-selection-batch-id"));
        assert!(!serialized.contains("private-selection-entry"));
        assert!(!serialized.contains("sourceFingerprint"));
        assert!(!serialized.contains("sourceItemKeyHash"));
        assert!(!serialized.contains("contentFingerprint"));
    }

    #[test]
    fn result_dto_omits_source_paths_and_private_digests() {
        let page = ExternalImportResultPage {
            batch: ExternalImportBatch {
                batch_id: ExternalImportBatchId::new("external-import-batch-1"),
                source_id: Some(ExternalImportSourceId::new("private-source-id")),
                adapter_id: ExternalImportAdapterId::new("hunting_box_directory_v1"),
                source_fingerprint: "C:/private/source-fingerprint".to_owned(),
                scan_status: ExternalImportScanStatus::Completed,
                import_status: ExternalImportBatchImportStatus::CompletedWithErrors,
                created_at_unix_millis: 1,
            },
            results: vec![ExternalImportItemResult {
                candidate_id: ExternalImportCandidateId::new("external-import-candidate-1"),
                status: ExternalImportItemStatus::Blocked,
                reason_code: Some(ExternalImportReasonCode::SourceChanged),
                imported_mod_id: Some(ModId::new("imported-mod-1")),
                retryable: false,
            }],
            total_count: 1,
            next_offset: None,
        };

        let value = serde_json::to_string(&ExternalImportBatchResultPageDto::from(page))
            .expect("serialize external import result dto");

        assert!(value.contains("external-import-candidate-1"));
        assert!(value.contains("imported-mod-1"));
        assert!(!value.contains("C:/private"));
        assert!(!value.contains("private-source-id"));
        assert!(!value.contains("sourceFingerprint"));
        assert!(!value.contains("sourceItemKeyHash"));
        assert!(!value.contains("contentFingerprint"));
    }
}
