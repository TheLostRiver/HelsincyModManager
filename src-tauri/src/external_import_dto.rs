use crate::dto::TaskStartedDto;
use hmm_app::{ExternalImportPreviewPage, ExternalImportScanLaunch};
use hmm_core::{
    ExternalImportBatch, ExternalImportBatchImportStatus, ExternalImportCandidate,
    ExternalImportCandidateStatus, ExternalImportConflictKind, ExternalImportMetadataHint,
    ExternalImportScanStatus, ExternalImportSource,
};
use serde::Serialize;

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
pub struct ExternalImportPreviewPageDto {
    pub batch: ExternalImportPreviewBatchDto,
    pub candidates: Vec<ExternalImportPreviewCandidateDto>,
    pub total_count: usize,
    pub next_cursor: Option<String>,
}

impl From<ExternalImportPreviewPage> for ExternalImportPreviewPageDto {
    fn from(page: ExternalImportPreviewPage) -> Self {
        Self {
            batch: page.batch.into(),
            candidates: page.candidates.into_iter().map(Into::into).collect(),
            total_count: page.total_count,
            next_cursor: page.next_offset.map(|offset| offset.to_string()),
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
}

impl From<ExternalImportCandidate> for ExternalImportPreviewCandidateDto {
    fn from(candidate: ExternalImportCandidate) -> Self {
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
        ExternalImportAdapterId, ExternalImportBatchId, ExternalImportCandidateId,
        ExternalImportConflictKind, ExternalImportResourceUsage,
    };

    #[test]
    fn preview_dto_omits_source_paths_and_private_digests() {
        let page = ExternalImportPreviewPage {
            batch: ExternalImportBatch {
                batch_id: ExternalImportBatchId::new("external-import-batch-1"),
                adapter_id: ExternalImportAdapterId::new("hunting_box_directory_v1"),
                source_fingerprint: "C:/private/source-fingerprint".to_owned(),
                scan_status: ExternalImportScanStatus::Completed,
                import_status: ExternalImportBatchImportStatus::Pending,
                created_at_unix_millis: 1,
            },
            candidates: vec![ExternalImportCandidate {
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
            }],
            total_count: 1,
            next_offset: None,
        };

        let value = serde_json::to_string(&ExternalImportPreviewPageDto::from(page))
            .expect("serialize external import preview dto");

        assert!(value.contains("external-import-candidate-1"));
        assert!(!value.contains("C:/private"));
        assert!(!value.contains("private-content-fingerprint"));
        assert!(!value.contains("sourceFingerprint"));
        assert!(!value.contains("sourceItemKeyHash"));
        assert!(!value.contains("contentFingerprint"));
    }
}
