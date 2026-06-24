use hmm_app::{
    GameCandidateScan, GameSetupCandidate, GameSetupServiceError, ImportPreviewImage, ModDetail,
    ModImportTaskError, ModLibraryItem, ModLibraryStatus, TaskKind, TaskManagerError,
    TaskProgressEvent, TaskStarted, TaskStatus,
};
use hmm_core::{
    GameDirectoryEvidence, GameDirectoryEvidenceKind, GameDirectoryStatus, GameDirectoryValidation,
    GameInstance, GameSetupErrorCode, GameSetupStatus, PreviewImageRejectionReason,
};
use hmm_ports::GameCandidateSource;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandErrorDto {
    pub code: String,
    pub message: String,
}

impl CommandErrorDto {
    pub fn from_service_error(error: GameSetupServiceError) -> Self {
        let code = error_code_to_string(error.error_code());

        Self {
            code,
            message: error.to_string(),
        }
    }

    pub fn from_mod_import_task_error(error: ModImportTaskError) -> Self {
        Self {
            code: error.error_code().to_owned(),
            message: error.to_string(),
        }
    }

    pub fn from_task_manager_error(error: TaskManagerError) -> Self {
        let code = match error {
            TaskManagerError::TaskIdGenerationFailed(_) => "task_id_generation_failed",
            TaskManagerError::TaskNotFound(_) => "task_not_found",
            TaskManagerError::TaskCannotBeCancelled { .. } => "task_cannot_be_cancelled",
            TaskManagerError::TaskCannotTransition { .. } => "task_cannot_transition",
            TaskManagerError::TaskStoreUnavailable => "task_store_unavailable",
        };

        Self {
            code: code.to_owned(),
            message: error.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStartedDto {
    pub task_id: String,
    pub kind: TaskKindDto,
    pub status: TaskStatusDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgressEventDto {
    pub task_id: String,
    pub kind: TaskKindDto,
    pub status: TaskStatusDto,
    pub phase: String,
    pub current: Option<u64>,
    pub total: Option<u64>,
    pub message: Option<String>,
    pub error: Option<String>,
    pub result_ref: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKindDto {
    ModImport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatusDto {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSetupStatusDto {
    pub game_id: String,
    pub kind: String,
    pub display_name: Option<String>,
    pub path_label: Option<String>,
    pub error_code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDirectoryValidationDto {
    pub game_id: String,
    pub is_valid: bool,
    pub confidence: u8,
    pub evidence: Vec<GameDirectoryEvidenceDto>,
    pub errors: Vec<String>,
    pub path_label: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDirectoryEvidenceDto {
    pub kind: String,
    pub label: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCandidateScanDto {
    pub game_id: String,
    pub candidates: Vec<GameCandidateDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCandidateDto {
    pub game_id: String,
    pub display_name: String,
    pub directory: String,
    pub path_label: String,
    pub source: String,
    pub source_label: String,
    pub is_valid: bool,
    pub confidence: u8,
    pub evidence: Vec<GameDirectoryEvidenceDto>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModLibraryItemDto {
    pub id: String,
    pub name: String,
    pub size_label: String,
    pub status: ModInstallStatusDto,
    pub category_labels: Vec<String>,
    pub preview_image: PreviewImageDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDetailDto {
    pub id: String,
    pub name: String,
    pub package_id: String,
    pub preview_image: PreviewImageDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModInstallStatusDto {
    Disabled,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum PreviewImageDto {
    Thumbnail {
        thumbnail_url: String,
        width: u32,
        height: u32,
        content_hash: String,
    },
    Fallback {
        reason: PreviewImageFallbackReasonDto,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewImageFallbackReasonDto {
    Missing,
    TooLarge,
    TooManyCandidates,
    UnsupportedFormat,
    DecodeFailed,
    PixelLimitExceeded,
    CacheWriteFailed,
}

impl From<PreviewImageRejectionReason> for PreviewImageFallbackReasonDto {
    fn from(reason: PreviewImageRejectionReason) -> Self {
        match reason {
            PreviewImageRejectionReason::Missing => Self::Missing,
            PreviewImageRejectionReason::TooLarge => Self::TooLarge,
            PreviewImageRejectionReason::TooManyCandidates => Self::TooManyCandidates,
            PreviewImageRejectionReason::UnsupportedFormat => Self::UnsupportedFormat,
            PreviewImageRejectionReason::DecodeFailed => Self::DecodeFailed,
            PreviewImageRejectionReason::PixelLimitExceeded => Self::PixelLimitExceeded,
            PreviewImageRejectionReason::CacheWriteFailed => Self::CacheWriteFailed,
        }
    }
}

impl From<ImportPreviewImage> for PreviewImageDto {
    fn from(preview_image: ImportPreviewImage) -> Self {
        match preview_image {
            ImportPreviewImage::Thumbnail {
                thumbnail_url,
                width,
                height,
                content_hash,
            } => Self::Thumbnail {
                thumbnail_url,
                width,
                height,
                content_hash,
            },
            ImportPreviewImage::Fallback { reason } => Self::Fallback {
                reason: reason.into(),
            },
        }
    }
}

impl From<ModLibraryItem> for ModLibraryItemDto {
    fn from(item: ModLibraryItem) -> Self {
        Self {
            id: item.id,
            name: item.name,
            size_label: item.size_label,
            status: item.status.into(),
            category_labels: item.category_labels,
            preview_image: item.preview_image.into(),
        }
    }
}

impl From<ModDetail> for ModDetailDto {
    fn from(detail: ModDetail) -> Self {
        Self {
            id: detail.id,
            name: detail.name,
            package_id: detail.package_id,
            preview_image: detail.preview_image.into(),
        }
    }
}

impl From<ModLibraryStatus> for ModInstallStatusDto {
    fn from(status: ModLibraryStatus) -> Self {
        match status {
            ModLibraryStatus::Disabled => Self::Disabled,
        }
    }
}

impl From<TaskStarted> for TaskStartedDto {
    fn from(task: TaskStarted) -> Self {
        Self {
            task_id: task.task_id,
            kind: task.kind.into(),
            status: task.status.into(),
        }
    }
}

impl From<TaskProgressEvent> for TaskProgressEventDto {
    fn from(event: TaskProgressEvent) -> Self {
        Self {
            task_id: event.task_id,
            kind: event.kind.into(),
            status: event.status.into(),
            phase: event.phase,
            current: event.current,
            total: event.total,
            message: event.message,
            error: event.error,
            result_ref: event.result_ref,
        }
    }
}

impl From<TaskKind> for TaskKindDto {
    fn from(kind: TaskKind) -> Self {
        match kind {
            TaskKind::ModImport => Self::ModImport,
        }
    }
}

impl From<TaskStatus> for TaskStatusDto {
    fn from(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Queued => Self::Queued,
            TaskStatus::Running => Self::Running,
            TaskStatus::Completed => Self::Completed,
            TaskStatus::Failed => Self::Failed,
            TaskStatus::Cancelled => Self::Cancelled,
        }
    }
}

pub fn status_to_dto(status: GameSetupStatus) -> GameSetupStatusDto {
    let kind = match status.status {
        GameDirectoryStatus::NotConfigured => "not_configured",
        GameDirectoryStatus::Invalid => "invalid",
        GameDirectoryStatus::Configured => "configured",
    }
    .to_owned();

    let (display_name, path_label) = status
        .instance
        .map(instance_to_display_parts)
        .unwrap_or((None, None));

    GameSetupStatusDto {
        game_id: status.game_id.as_str().to_owned(),
        kind,
        display_name,
        path_label,
        error_code: status.error_code.map(error_code_to_string),
        message: status.message,
    }
}

pub fn candidate_scan_to_dto(scan: GameCandidateScan) -> GameCandidateScanDto {
    GameCandidateScanDto {
        game_id: scan.game_id.as_str().to_owned(),
        candidates: scan.candidates.into_iter().map(candidate_to_dto).collect(),
    }
}

pub fn validation_to_dto(validation: GameDirectoryValidation) -> GameDirectoryValidationDto {
    GameDirectoryValidationDto {
        game_id: validation.game_id.as_str().to_owned(),
        is_valid: validation.is_valid,
        confidence: validation.confidence,
        evidence: validation
            .evidence
            .into_iter()
            .map(evidence_to_dto)
            .collect(),
        errors: validation
            .errors
            .into_iter()
            .map(error_code_to_string)
            .collect(),
        path_label: path_label_from_path(&validation.directory),
    }
}

fn candidate_to_dto(candidate: GameSetupCandidate) -> GameCandidateDto {
    GameCandidateDto {
        game_id: candidate.candidate.game_id.as_str().to_owned(),
        display_name: candidate.candidate.display_name,
        directory: candidate.candidate.root_dir.to_string_lossy().to_string(),
        path_label: path_label_from_path(&candidate.candidate.root_dir),
        source: candidate_source_to_string(candidate.candidate.source),
        source_label: candidate.candidate.source_label,
        is_valid: candidate.validation.is_valid,
        confidence: candidate.validation.confidence,
        evidence: candidate
            .validation
            .evidence
            .into_iter()
            .map(evidence_to_dto)
            .collect(),
        errors: candidate
            .validation
            .errors
            .into_iter()
            .map(error_code_to_string)
            .collect(),
    }
}

fn instance_to_display_parts(instance: GameInstance) -> (Option<String>, Option<String>) {
    (
        Some(instance.display_name),
        Some(path_label_from_path(&instance.root_dir)),
    )
}

fn candidate_source_to_string(source: GameCandidateSource) -> String {
    match source {
        GameCandidateSource::Steam => "steam",
    }
    .to_owned()
}

fn evidence_to_dto(evidence: GameDirectoryEvidence) -> GameDirectoryEvidenceDto {
    GameDirectoryEvidenceDto {
        kind: evidence_kind_to_string(evidence.kind),
        label: evidence.label,
    }
}

fn evidence_kind_to_string(kind: GameDirectoryEvidenceKind) -> String {
    match kind {
        GameDirectoryEvidenceKind::DirectoryExists => "directory_exists",
        GameDirectoryEvidenceKind::DirectoryMissing => "directory_missing",
        GameDirectoryEvidenceKind::FoundExecutable => "found_executable",
        GameDirectoryEvidenceKind::MissingExecutable => "missing_executable",
        GameDirectoryEvidenceKind::FoundNativePc => "found_native_pc",
    }
    .to_owned()
}

fn error_code_to_string(error: GameSetupErrorCode) -> String {
    match error {
        GameSetupErrorCode::UnsupportedGame => "unsupported_game",
        GameSetupErrorCode::DirectoryNotFound => "directory_not_found",
        GameSetupErrorCode::DirectoryNotAbsolute => "directory_not_absolute",
        GameSetupErrorCode::MissingExecutable => "missing_executable",
        GameSetupErrorCode::StorageFailed => "storage_failed",
        GameSetupErrorCode::StorageCorrupted => "storage_corrupted",
        GameSetupErrorCode::ScanFailed => "scan_failed",
        GameSetupErrorCode::ScanNotImplemented => "scan_not_implemented",
        GameSetupErrorCode::Unknown => "unknown",
    }
    .to_owned()
}

fn path_label_from_path(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!(".../{name}"))
        .unwrap_or_else(|| ".../selected-directory".to_owned())
}

#[cfg(test)]
mod preview_image_tests {
    use super::*;

    #[test]
    fn serializes_thumbnail_dto_with_camel_case_fields() {
        let dto = PreviewImageDto::Thumbnail {
            thumbnail_url: "thumbnail://pkg/preview/hash".to_owned(),
            width: 512,
            height: 768,
            content_hash: "abc123".to_owned(),
        };

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["kind"], "thumbnail");
        assert_eq!(value["thumbnailUrl"], "thumbnail://pkg/preview/hash");
        assert_eq!(value["contentHash"], "abc123");
    }

    #[test]
    fn serializes_fallback_reason_as_snake_case() {
        let dto = PreviewImageDto::Fallback {
            reason: PreviewImageRejectionReason::PixelLimitExceeded.into(),
        };

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["kind"], "fallback");
        assert_eq!(value["reason"], "pixel_limit_exceeded");
    }

    #[test]
    fn maps_all_domain_fallback_reasons_to_dto() {
        let cases = [
            (PreviewImageRejectionReason::Missing, "missing"),
            (PreviewImageRejectionReason::TooLarge, "too_large"),
            (
                PreviewImageRejectionReason::TooManyCandidates,
                "too_many_candidates",
            ),
            (
                PreviewImageRejectionReason::UnsupportedFormat,
                "unsupported_format",
            ),
            (PreviewImageRejectionReason::DecodeFailed, "decode_failed"),
            (
                PreviewImageRejectionReason::PixelLimitExceeded,
                "pixel_limit_exceeded",
            ),
            (
                PreviewImageRejectionReason::CacheWriteFailed,
                "cache_write_failed",
            ),
        ];

        for (reason, expected) in cases {
            let dto_reason: PreviewImageFallbackReasonDto = reason.into();
            let value = serde_json::to_value(dto_reason).expect("serialize reason");
            assert_eq!(value, expected);
        }
    }

    #[test]
    fn maps_import_preview_thumbnail_to_dto() {
        let dto: PreviewImageDto = ImportPreviewImage::Thumbnail {
            thumbnail_url: "thumbnail://pkg-1/preview-768/hash".to_owned(),
            width: 320,
            height: 180,
            content_hash: "hash".to_owned(),
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["kind"], "thumbnail");
        assert_eq!(value["thumbnailUrl"], "thumbnail://pkg-1/preview-768/hash");
        assert_eq!(value["width"], 320);
        assert_eq!(value["height"], 180);
        assert_eq!(value["contentHash"], "hash");
    }

    #[test]
    fn maps_import_preview_fallback_to_dto() {
        let dto: PreviewImageDto = ImportPreviewImage::Fallback {
            reason: PreviewImageRejectionReason::DecodeFailed,
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["kind"], "fallback");
        assert_eq!(value["reason"], "decode_failed");
    }

    #[test]
    fn serializes_mod_library_item_with_preview_image() {
        let dto: ModLibraryItemDto = hmm_app::ModLibraryItem {
            id: "pkg-1".to_owned(),
            name: "pkg-1".to_owned(),
            size_label: "导入完成".to_owned(),
            status: hmm_app::ModLibraryStatus::Disabled,
            category_labels: Vec::new(),
            preview_image: ImportPreviewImage::Thumbnail {
                thumbnail_url: "thumbnail://pkg-1/preview/hash".to_owned(),
                width: 320,
                height: 180,
                content_hash: "hash".to_owned(),
            },
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["id"], "pkg-1");
        assert_eq!(value["name"], "pkg-1");
        assert_eq!(value["sizeLabel"], "导入完成");
        assert_eq!(value["status"], "disabled");
        assert_eq!(value["previewImage"]["kind"], "thumbnail");
        assert_eq!(
            value["previewImage"]["thumbnailUrl"],
            "thumbnail://pkg-1/preview/hash"
        );
    }

    #[test]
    fn serializes_mod_detail_with_preview_image() {
        let dto: ModDetailDto = hmm_app::ModDetail {
            id: "pkg-1".to_owned(),
            name: "pkg-1".to_owned(),
            package_id: "pkg-1".to_owned(),
            preview_image: ImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::Missing,
            },
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["id"], "pkg-1");
        assert_eq!(value["packageId"], "pkg-1");
        assert_eq!(value["previewImage"]["kind"], "fallback");
        assert_eq!(value["previewImage"]["reason"], "missing");
    }
}

#[cfg(test)]
mod task_dto_tests {
    use super::*;

    #[test]
    fn serializes_task_started_dto_with_camel_case_fields() {
        let dto = TaskStartedDto {
            task_id: "mod-import-123".to_owned(),
            kind: TaskKindDto::ModImport,
            status: TaskStatusDto::Queued,
        };

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["taskId"], "mod-import-123");
        assert_eq!(value["kind"], "mod_import");
        assert_eq!(value["status"], "queued");
    }

    #[test]
    fn serializes_task_progress_event_dto_with_camel_case_fields() {
        let dto: TaskProgressEventDto = TaskProgressEvent::new(
            "mod-import-123",
            TaskKind::ModImport,
            TaskStatus::Queued,
            "mod_import.queued",
        )
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["taskId"], "mod-import-123");
        assert_eq!(value["kind"], "mod_import");
        assert_eq!(value["status"], "queued");
        assert_eq!(value["phase"], "mod_import.queued");
        assert!(value["current"].is_null());
        assert!(value["total"].is_null());
        assert!(value["message"].is_null());
        assert!(value["error"].is_null());
        assert!(value["resultRef"].is_null());
    }

    #[test]
    fn maps_mod_import_task_error_to_command_error_code() {
        let dto =
            CommandErrorDto::from_mod_import_task_error(ModImportTaskError::ArchivePathNotAbsolute);

        assert_eq!(dto.code, "archive_path_not_absolute");
    }
}
