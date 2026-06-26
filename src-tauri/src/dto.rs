use hmm_app::{
    AppSettingsServiceError, GameCandidateScan, GameSetupCandidate, GameSetupServiceError,
    ImportPreviewImage, InstallManifestStatus, InstallManifestStatusSummary, InstallRecoveryIssue,
    InstallRecoveryIssueSummary, InstallRecoveryStatus, InstallRecoverySummary, ModDetail,
    ModImportTaskError, ModLibraryItem, ModLibraryStatus, TaskKind, TaskManagerError,
    TaskProgressEvent, TaskStarted, TaskStatus,
};
use hmm_core::{
    GameDirectoryEvidence, GameDirectoryEvidenceKind, GameDirectoryStatus, GameDirectoryValidation,
    GameInstance, GameSetupErrorCode, GameSetupStatus, InstallAction, InstallConflict,
    InstallFileProvider, InstallPlan, PreviewImageRejectionReason,
};
use hmm_ports::{AppSettings, GameCandidateSource};
use serde::{Deserialize, Serialize};

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

    pub fn from_app_settings_service_error(error: AppSettingsServiceError) -> Self {
        let code = match error {
            AppSettingsServiceError::InvalidThumbnailCacheMaxBytes => {
                "thumbnail_cache_max_bytes_invalid"
            }
            AppSettingsServiceError::InvalidThumbnailCacheMaxAgeDays => {
                "thumbnail_cache_max_age_days_invalid"
            }
            AppSettingsServiceError::SettingsUnavailable => "app_settings_unavailable",
        };

        Self {
            code: code.to_owned(),
            message: error.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsDto {
    pub thumbnail_cache_max_bytes: Option<u64>,
    pub thumbnail_cache_max_age_days: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewInstallPlanRequestDto {
    pub allowed_target_roots: Vec<String>,
    pub files: Vec<PreviewInstallPlanFileInputDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewInstallPlanFileInputDto {
    pub mod_id: String,
    pub package_file_id: String,
    pub target_path: String,
    pub layer_name: String,
    pub layer_priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewImportedModInstallPlanRequestDto {
    pub game_id: String,
    pub mod_id: String,
    pub layer_name: String,
    pub layer_priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartInstallTaskRequestDto {
    pub game_id: String,
    pub mod_id: String,
    pub profile_id: String,
    pub layer_name: String,
    pub layer_priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartUninstallTaskRequestDto {
    pub game_id: String,
    pub mod_id: String,
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallManifestStatusRequestDto {
    pub profile_id: String,
    pub mod_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecoveryScanRequestDto {
    pub game_id: String,
    pub profile_id: String,
    pub mod_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlanPreviewDto {
    pub actions: Vec<InstallPlanActionDto>,
    pub conflicts: Vec<InstallPlanConflictDto>,
    pub has_blocking_conflicts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlanActionDto {
    pub target_path: String,
    pub mod_id: String,
    pub package_file_id: String,
    pub layer_name: String,
    pub layer_priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlanConflictDto {
    pub target_path: String,
    pub providers: Vec<InstallPlanProviderDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlanProviderDto {
    pub mod_id: String,
    pub package_file_id: String,
    pub layer_name: String,
    pub layer_priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallManifestStatusSummaryDto {
    pub profile_id: String,
    pub mod_id: String,
    pub status: InstallManifestStatusDto,
    pub managed_file_count: usize,
    pub backup_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecoverySummaryDto {
    pub profile_id: String,
    pub mod_id: String,
    pub status: InstallRecoveryStatusDto,
    pub managed_file_count: usize,
    pub backup_count: usize,
    pub issue_count: usize,
    pub issues: Vec<InstallRecoveryIssueSummaryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecoveryIssueSummaryDto {
    pub issue: InstallRecoveryIssueDto,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallManifestStatusDto {
    NotInstalled,
    Installed,
    RepairRequired,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallRecoveryStatusDto {
    NotInstalled,
    Completed,
    RepairRequired,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallRecoveryIssueDto {
    MissingInstalledFileSummary,
    TargetMissing,
    TargetChanged,
    TargetReadFailed,
    BackupMissing,
    BackupReadFailed,
}

impl From<AppSettings> for AppSettingsDto {
    fn from(settings: AppSettings) -> Self {
        Self {
            thumbnail_cache_max_bytes: settings.thumbnail_cache_max_bytes,
            thumbnail_cache_max_age_days: settings.thumbnail_cache_max_age_days,
        }
    }
}

impl From<InstallPlan> for InstallPlanPreviewDto {
    fn from(plan: InstallPlan) -> Self {
        let has_blocking_conflicts = plan.has_blocking_conflicts();

        Self {
            actions: plan.actions.into_iter().map(Into::into).collect(),
            conflicts: plan.conflicts.into_iter().map(Into::into).collect(),
            has_blocking_conflicts,
        }
    }
}

impl From<InstallAction> for InstallPlanActionDto {
    fn from(action: InstallAction) -> Self {
        let provider = action.provider;

        Self {
            target_path: action.target_path.as_str().to_owned(),
            mod_id: provider.mod_id.as_str().to_owned(),
            package_file_id: provider.package_file_id.as_str().to_owned(),
            layer_name: provider.layer.name,
            layer_priority: provider.layer.priority,
        }
    }
}

impl From<InstallConflict> for InstallPlanConflictDto {
    fn from(conflict: InstallConflict) -> Self {
        Self {
            target_path: conflict.target_path.as_str().to_owned(),
            providers: conflict.providers.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<InstallFileProvider> for InstallPlanProviderDto {
    fn from(provider: InstallFileProvider) -> Self {
        Self {
            mod_id: provider.mod_id.as_str().to_owned(),
            package_file_id: provider.package_file_id.as_str().to_owned(),
            layer_name: provider.layer.name,
            layer_priority: provider.layer.priority,
        }
    }
}

impl From<InstallManifestStatusSummary> for InstallManifestStatusSummaryDto {
    fn from(summary: InstallManifestStatusSummary) -> Self {
        Self {
            profile_id: summary.profile_id.as_str().to_owned(),
            mod_id: summary.mod_id.as_str().to_owned(),
            status: summary.status.into(),
            managed_file_count: summary.managed_file_count,
            backup_count: summary.backup_count,
        }
    }
}

impl From<InstallRecoverySummary> for InstallRecoverySummaryDto {
    fn from(summary: InstallRecoverySummary) -> Self {
        Self {
            profile_id: summary.profile_id.as_str().to_owned(),
            mod_id: summary.mod_id.as_str().to_owned(),
            status: summary.status.into(),
            managed_file_count: summary.managed_file_count,
            backup_count: summary.backup_count,
            issue_count: summary.issue_count,
            issues: summary.issues.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<InstallRecoveryIssueSummary> for InstallRecoveryIssueSummaryDto {
    fn from(summary: InstallRecoveryIssueSummary) -> Self {
        Self {
            issue: summary.issue.into(),
            count: summary.count,
        }
    }
}

impl From<InstallManifestStatus> for InstallManifestStatusDto {
    fn from(status: InstallManifestStatus) -> Self {
        match status {
            InstallManifestStatus::NotInstalled => Self::NotInstalled,
            InstallManifestStatus::Installed => Self::Installed,
            InstallManifestStatus::RepairRequired => Self::RepairRequired,
            InstallManifestStatus::Unknown => Self::Unknown,
        }
    }
}

impl From<InstallRecoveryStatus> for InstallRecoveryStatusDto {
    fn from(status: InstallRecoveryStatus) -> Self {
        match status {
            InstallRecoveryStatus::NotInstalled => Self::NotInstalled,
            InstallRecoveryStatus::Completed => Self::Completed,
            InstallRecoveryStatus::RepairRequired => Self::RepairRequired,
            InstallRecoveryStatus::Unknown => Self::Unknown,
        }
    }
}

impl From<InstallRecoveryIssue> for InstallRecoveryIssueDto {
    fn from(issue: InstallRecoveryIssue) -> Self {
        match issue {
            InstallRecoveryIssue::MissingInstalledFileSummary => Self::MissingInstalledFileSummary,
            InstallRecoveryIssue::TargetMissing => Self::TargetMissing,
            InstallRecoveryIssue::TargetChanged => Self::TargetChanged,
            InstallRecoveryIssue::TargetReadFailed => Self::TargetReadFailed,
            InstallRecoveryIssue::BackupMissing => Self::BackupMissing,
            InstallRecoveryIssue::BackupReadFailed => Self::BackupReadFailed,
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
    Install,
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
    pub author: Option<String>,
    pub version_label: Option<String>,
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
    pub metadata: ModPackageMetadataDto,
    pub preview_image: PreviewImageDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModPackageMetadataDto {
    pub version: Option<String>,
    pub author: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDependencyGraphDto {
    pub nodes: Vec<ModDependencyGraphNodeDto>,
    pub edges: Vec<ModDependencyGraphEdgeDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDependencyGraphNodeDto {
    pub mod_id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDependencyGraphEdgeDto {
    pub source_mod_id: String,
    pub dependency: String,
    pub matched_imported_mod_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewImageDiagnosticsDto {
    pub total_imported_mods: usize,
    pub thumbnail_count: usize,
    pub fallback_count: usize,
    pub fallback_reasons: Vec<PreviewImageFallbackDiagnosticsDto>,
    pub export_categories: Vec<PreviewImageDiagnosticExportCategoryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewImageDiagnosticsExportDto {
    pub export_id: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub diagnostics: PreviewImageDiagnosticsDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogDiagnosticsExportDto {
    pub export_id: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub audit_event_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportDiagnosticsExportDto {
    pub export_id: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub app_log_line_count: usize,
    pub task_log_line_count: usize,
    pub audit_event_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewImageDiagnosticExportCategoryDto {
    pub category: PreviewImageDiagnosticExportCategoryIdDto,
    pub status: PreviewImageDiagnosticExportCategoryStatusDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<PreviewImageDiagnosticExportExclusionReasonDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewImageDiagnosticExportCategoryIdDto {
    PreviewImageSummary,
    ThumbnailFiles,
    ThumbnailUrls,
    RawPackageContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewImageDiagnosticExportCategoryStatusDto {
    Included,
    Excluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewImageDiagnosticExportExclusionReasonDto {
    DerivedImageContent,
    OpaqueResourceReference,
    ThirdPartyModContent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewImageCandidateListDto {
    pub mod_id: String,
    pub candidates: Vec<PreviewImageCandidateDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewImageCandidateDto {
    pub candidate_index: usize,
    pub file_name: String,
    pub compressed_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewImageFallbackDiagnosticsDto {
    pub reason: PreviewImageFallbackReasonDto,
    pub count: usize,
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
                variant: _,
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
            author: item.author,
            version_label: item.version_label,
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
            metadata: detail.metadata.into(),
            preview_image: detail.preview_image.into(),
        }
    }
}

impl From<hmm_app::ModPackageMetadataSummary> for ModPackageMetadataDto {
    fn from(metadata: hmm_app::ModPackageMetadataSummary) -> Self {
        Self {
            version: metadata.version,
            author: metadata.author,
            category: metadata.category,
            tags: metadata.tags,
            dependencies: metadata.dependencies,
        }
    }
}

impl From<hmm_app::ModDependencyGraph> for ModDependencyGraphDto {
    fn from(graph: hmm_app::ModDependencyGraph) -> Self {
        Self {
            nodes: graph.nodes.into_iter().map(Into::into).collect(),
            edges: graph.edges.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<hmm_app::ModDependencyGraphNode> for ModDependencyGraphNodeDto {
    fn from(node: hmm_app::ModDependencyGraphNode) -> Self {
        Self {
            mod_id: node.mod_id,
            name: node.name,
        }
    }
}

impl From<hmm_app::ModDependencyGraphEdge> for ModDependencyGraphEdgeDto {
    fn from(edge: hmm_app::ModDependencyGraphEdge) -> Self {
        Self {
            source_mod_id: edge.source_mod_id,
            dependency: edge.dependency,
            matched_imported_mod_id: edge.matched_imported_mod_id,
        }
    }
}

impl From<hmm_app::PreviewImageDiagnosticsSummary> for PreviewImageDiagnosticsDto {
    fn from(summary: hmm_app::PreviewImageDiagnosticsSummary) -> Self {
        Self {
            total_imported_mods: summary.total_imported_mods,
            thumbnail_count: summary.thumbnail_count,
            fallback_count: summary.fallback_count,
            fallback_reasons: summary
                .fallback_reasons
                .into_iter()
                .map(Into::into)
                .collect(),
            export_categories: summary
                .export_categories
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<hmm_app::PreviewImageDiagnosticsExport> for PreviewImageDiagnosticsExportDto {
    fn from(export: hmm_app::PreviewImageDiagnosticsExport) -> Self {
        Self {
            export_id: export.export_id,
            file_name: export.file_name,
            size_bytes: export.size_bytes,
            diagnostics: export.diagnostics.into(),
        }
    }
}

impl From<hmm_app::AuditLogDiagnosticsExport> for AuditLogDiagnosticsExportDto {
    fn from(export: hmm_app::AuditLogDiagnosticsExport) -> Self {
        Self {
            export_id: export.export_id,
            file_name: export.file_name,
            size_bytes: export.size_bytes,
            audit_event_count: export.audit_event_count,
        }
    }
}

impl From<hmm_app::SupportDiagnosticsExport> for SupportDiagnosticsExportDto {
    fn from(export: hmm_app::SupportDiagnosticsExport) -> Self {
        Self {
            export_id: export.export_id,
            file_name: export.file_name,
            size_bytes: export.size_bytes,
            app_log_line_count: export.app_log_line_count,
            task_log_line_count: export.task_log_line_count,
            audit_event_count: export.audit_event_count,
        }
    }
}

impl From<hmm_app::PreviewImageDiagnosticExportCategory>
    for PreviewImageDiagnosticExportCategoryDto
{
    fn from(category: hmm_app::PreviewImageDiagnosticExportCategory) -> Self {
        Self {
            category: category.category.into(),
            status: category.status.into(),
            reason: category.reason.map(Into::into),
        }
    }
}

impl From<hmm_app::PreviewImageDiagnosticExportCategoryId>
    for PreviewImageDiagnosticExportCategoryIdDto
{
    fn from(category: hmm_app::PreviewImageDiagnosticExportCategoryId) -> Self {
        match category {
            hmm_app::PreviewImageDiagnosticExportCategoryId::PreviewImageSummary => {
                Self::PreviewImageSummary
            }
            hmm_app::PreviewImageDiagnosticExportCategoryId::ThumbnailFiles => Self::ThumbnailFiles,
            hmm_app::PreviewImageDiagnosticExportCategoryId::ThumbnailUrls => Self::ThumbnailUrls,
            hmm_app::PreviewImageDiagnosticExportCategoryId::RawPackageContent => {
                Self::RawPackageContent
            }
        }
    }
}

impl From<hmm_app::PreviewImageDiagnosticExportCategoryStatus>
    for PreviewImageDiagnosticExportCategoryStatusDto
{
    fn from(status: hmm_app::PreviewImageDiagnosticExportCategoryStatus) -> Self {
        match status {
            hmm_app::PreviewImageDiagnosticExportCategoryStatus::Included => Self::Included,
            hmm_app::PreviewImageDiagnosticExportCategoryStatus::Excluded => Self::Excluded,
        }
    }
}

impl From<hmm_app::PreviewImageDiagnosticExportExclusionReason>
    for PreviewImageDiagnosticExportExclusionReasonDto
{
    fn from(reason: hmm_app::PreviewImageDiagnosticExportExclusionReason) -> Self {
        match reason {
            hmm_app::PreviewImageDiagnosticExportExclusionReason::DerivedImageContent => {
                Self::DerivedImageContent
            }
            hmm_app::PreviewImageDiagnosticExportExclusionReason::OpaqueResourceReference => {
                Self::OpaqueResourceReference
            }
            hmm_app::PreviewImageDiagnosticExportExclusionReason::ThirdPartyModContent => {
                Self::ThirdPartyModContent
            }
        }
    }
}

impl From<hmm_app::PreviewImageFallbackDiagnostic> for PreviewImageFallbackDiagnosticsDto {
    fn from(reason: hmm_app::PreviewImageFallbackDiagnostic) -> Self {
        Self {
            reason: reason.reason.into(),
            count: reason.count,
        }
    }
}

impl From<hmm_app::PreviewImageCandidateList> for PreviewImageCandidateListDto {
    fn from(list: hmm_app::PreviewImageCandidateList) -> Self {
        Self {
            mod_id: list.mod_id,
            candidates: list.candidates.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<hmm_app::PreviewImageCandidateSummary> for PreviewImageCandidateDto {
    fn from(candidate: hmm_app::PreviewImageCandidateSummary) -> Self {
        Self {
            candidate_index: candidate.candidate_index,
            file_name: candidate.file_name,
            compressed_size_bytes: candidate.compressed_size_bytes,
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
            TaskKind::Install => Self::Install,
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
mod app_settings_dto_tests {
    use super::*;

    #[test]
    fn serializes_app_settings_dto_with_camel_case_fields() {
        let dto: AppSettingsDto = AppSettings {
            thumbnail_cache_max_bytes: Some(128 * 1024 * 1024),
            thumbnail_cache_max_age_days: Some(14),
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize settings");

        assert_eq!(value["thumbnailCacheMaxBytes"], 128 * 1024 * 1024);
        assert_eq!(value["thumbnailCacheMaxAgeDays"], 14);
    }

    #[test]
    fn maps_invalid_thumbnail_cache_setting_to_stable_error_code() {
        let error = CommandErrorDto::from_app_settings_service_error(
            AppSettingsServiceError::InvalidThumbnailCacheMaxBytes,
        );

        assert_eq!(error.code, "thumbnail_cache_max_bytes_invalid");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
    }

    #[test]
    fn maps_invalid_thumbnail_cache_age_setting_to_stable_error_code() {
        let error = CommandErrorDto::from_app_settings_service_error(
            AppSettingsServiceError::InvalidThumbnailCacheMaxAgeDays,
        );

        assert_eq!(error.code, "thumbnail_cache_max_age_days_invalid");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
    }
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
            variant: "preview-768".to_owned(),
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
    fn serializes_preview_image_diagnostics_without_thumbnail_urls() {
        let dto: PreviewImageDiagnosticsDto = hmm_app::PreviewImageDiagnosticsSummary {
            total_imported_mods: 4,
            thumbnail_count: 1,
            fallback_count: 3,
            fallback_reasons: vec![hmm_app::PreviewImageFallbackDiagnostic {
                reason: PreviewImageRejectionReason::DecodeFailed,
                count: 2,
            }],
            export_categories: vec![
                hmm_app::PreviewImageDiagnosticExportCategory {
                    category: hmm_app::PreviewImageDiagnosticExportCategoryId::PreviewImageSummary,
                    status: hmm_app::PreviewImageDiagnosticExportCategoryStatus::Included,
                    reason: None,
                },
                hmm_app::PreviewImageDiagnosticExportCategory {
                    category: hmm_app::PreviewImageDiagnosticExportCategoryId::ThumbnailFiles,
                    status: hmm_app::PreviewImageDiagnosticExportCategoryStatus::Excluded,
                    reason: Some(
                        hmm_app::PreviewImageDiagnosticExportExclusionReason::DerivedImageContent,
                    ),
                },
            ],
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize diagnostics");

        assert_eq!(value["totalImportedMods"], 4);
        assert_eq!(value["thumbnailCount"], 1);
        assert_eq!(value["fallbackCount"], 3);
        assert_eq!(value["fallbackReasons"][0]["reason"], "decode_failed");
        assert_eq!(value["fallbackReasons"][0]["count"], 2);
        assert_eq!(
            value["exportCategories"][0]["category"],
            "preview_image_summary"
        );
        assert_eq!(value["exportCategories"][0]["status"], "included");
        assert!(value["exportCategories"][0].get("reason").is_none());
        assert_eq!(value["exportCategories"][1]["category"], "thumbnail_files");
        assert_eq!(value["exportCategories"][1]["status"], "excluded");
        assert_eq!(
            value["exportCategories"][1]["reason"],
            "derived_image_content"
        );
        assert!(value.get("thumbnailUrl").is_none());
        assert!(value.get("contentHash").is_none());
        assert!(value.get("path").is_none());
    }

    #[test]
    fn serializes_preview_image_diagnostics_export_without_paths_or_thumbnail_urls() {
        let dto: PreviewImageDiagnosticsExportDto = hmm_app::PreviewImageDiagnosticsExport {
            export_id: "preview-image-diagnostics-42.zip".to_owned(),
            file_name: "preview-image-diagnostics-42.zip".to_owned(),
            size_bytes: 1234,
            diagnostics: hmm_app::PreviewImageDiagnosticsSummary {
                total_imported_mods: 2,
                thumbnail_count: 1,
                fallback_count: 1,
                fallback_reasons: vec![hmm_app::PreviewImageFallbackDiagnostic {
                    reason: PreviewImageRejectionReason::DecodeFailed,
                    count: 1,
                }],
                export_categories: vec![hmm_app::PreviewImageDiagnosticExportCategory {
                    category: hmm_app::PreviewImageDiagnosticExportCategoryId::PreviewImageSummary,
                    status: hmm_app::PreviewImageDiagnosticExportCategoryStatus::Included,
                    reason: None,
                }],
            },
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize export");

        assert_eq!(value["exportId"], "preview-image-diagnostics-42.zip");
        assert_eq!(value["fileName"], "preview-image-diagnostics-42.zip");
        assert_eq!(value["sizeBytes"], 1234);
        assert_eq!(value["diagnostics"]["totalImportedMods"], 2);
        assert_eq!(value["diagnostics"]["thumbnailCount"], 1);
        assert!(!value.to_string().contains("thumbnailUrl"));
        assert!(!value.to_string().contains("contentHash"));
        assert!(!value.to_string().contains("thumbnail://"));
        assert!(!value.to_string().contains("C:/"));
        assert!(!value.to_string().contains("sandbox"));
    }

    #[test]
    fn serializes_audit_log_diagnostics_export_without_paths_or_raw_events() {
        let dto: AuditLogDiagnosticsExportDto = hmm_app::AuditLogDiagnosticsExport {
            export_id: "audit-log-diagnostics-42.zip".to_owned(),
            file_name: "audit-log-diagnostics-42.zip".to_owned(),
            size_bytes: 1234,
            audit_event_count: 2,
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize audit diagnostics export");

        assert_eq!(value["exportId"], "audit-log-diagnostics-42.zip");
        assert_eq!(value["fileName"], "audit-log-diagnostics-42.zip");
        assert_eq!(value["sizeBytes"], 1234);
        assert_eq!(value["auditEventCount"], 2);
        assert!(value.get("events").is_none());
        assert!(!value.to_string().contains("thumbnail://"));
        assert!(!value.to_string().contains("contentHash"));
        assert!(!value.to_string().contains("raw_path"));
        assert!(!value.to_string().contains("C:/"));
        assert!(!value.to_string().contains("sandbox"));
    }

    #[test]
    fn serializes_support_diagnostics_export_without_paths_or_raw_logs() {
        let dto: SupportDiagnosticsExportDto = hmm_app::SupportDiagnosticsExport {
            export_id: "support-diagnostics-42.zip".to_owned(),
            file_name: "support-diagnostics-42.zip".to_owned(),
            size_bytes: 4096,
            app_log_line_count: 2,
            task_log_line_count: 3,
            audit_event_count: 4,
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize support diagnostics export");

        assert_eq!(value["exportId"], "support-diagnostics-42.zip");
        assert_eq!(value["fileName"], "support-diagnostics-42.zip");
        assert_eq!(value["sizeBytes"], 4096);
        assert_eq!(value["appLogLineCount"], 2);
        assert_eq!(value["taskLogLineCount"], 3);
        assert_eq!(value["auditEventCount"], 4);
        assert!(value.get("appLogLines").is_none());
        assert!(value.get("taskLogLines").is_none());
        assert!(value.get("events").is_none());
        assert!(value.get("path").is_none());
        assert!(!value.to_string().contains("thumbnail://"));
        assert!(!value.to_string().contains("contentHash"));
        assert!(!value.to_string().contains("raw_path"));
        assert!(!value.to_string().contains("C:/"));
        assert!(!value.to_string().contains("sandbox"));
    }

    #[test]
    fn serializes_preview_image_candidate_list_without_paths_or_urls() {
        let dto: PreviewImageCandidateListDto = hmm_app::PreviewImageCandidateList {
            mod_id: "mod-1".to_owned(),
            candidates: vec![hmm_app::PreviewImageCandidateSummary {
                candidate_index: 0,
                file_name: "preview.png".to_owned(),
                compressed_size_bytes: 1234,
            }],
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize candidate list");

        assert_eq!(value["modId"], "mod-1");
        assert_eq!(value["candidates"][0]["candidateIndex"], 0);
        assert_eq!(value["candidates"][0]["fileName"], "preview.png");
        assert_eq!(value["candidates"][0]["compressedSizeBytes"], 1234);
        assert!(value["candidates"][0].get("logicalPath").is_none());
        assert!(value["candidates"][0].get("thumbnailUrl").is_none());
        assert!(value["candidates"][0].get("path").is_none());
    }

    #[test]
    fn serializes_mod_library_item_with_preview_image() {
        let dto: ModLibraryItemDto = hmm_app::ModLibraryItem {
            id: "pkg-1".to_owned(),
            name: "pkg-1".to_owned(),
            author: Some("A Hunter".to_owned()),
            version_label: Some("v1.2.3".to_owned()),
            size_label: "导入完成".to_owned(),
            status: hmm_app::ModLibraryStatus::Disabled,
            category_labels: Vec::new(),
            preview_image: ImportPreviewImage::Thumbnail {
                thumbnail_url: "thumbnail://pkg-1/preview/hash".to_owned(),
                width: 320,
                height: 180,
                content_hash: "hash".to_owned(),
                variant: "preview".to_owned(),
            },
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["id"], "pkg-1");
        assert_eq!(value["name"], "pkg-1");
        assert_eq!(value["author"], "A Hunter");
        assert_eq!(value["versionLabel"], "v1.2.3");
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
            metadata: hmm_app::ModPackageMetadataSummary {
                version: Some("1.2.3".to_owned()),
                author: Some("A Hunter".to_owned()),
                category: Some("Visual".to_owned()),
                tags: vec!["armor".to_owned(), "hd".to_owned()],
                dependencies: vec!["stracker-loader".to_owned()],
            },
            preview_image: ImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::Missing,
            },
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["id"], "pkg-1");
        assert_eq!(value["packageId"], "pkg-1");
        assert_eq!(value["metadata"]["version"], "1.2.3");
        assert_eq!(value["metadata"]["author"], "A Hunter");
        assert_eq!(value["metadata"]["category"], "Visual");
        assert_eq!(value["metadata"]["tags"][0], "armor");
        assert_eq!(value["metadata"]["dependencies"][0], "stracker-loader");
        assert_eq!(value["previewImage"]["kind"], "fallback");
        assert_eq!(value["previewImage"]["reason"], "missing");
    }

    #[test]
    fn serializes_mod_dependency_graph_without_install_status_or_paths() {
        let dto: ModDependencyGraphDto = hmm_app::ModDependencyGraph {
            nodes: vec![hmm_app::ModDependencyGraphNode {
                mod_id: "armor-pack".to_owned(),
                name: "Armor Pack".to_owned(),
            }],
            edges: vec![hmm_app::ModDependencyGraphEdge {
                source_mod_id: "armor-pack".to_owned(),
                dependency: "stracker-loader".to_owned(),
                matched_imported_mod_id: Some("stracker-loader".to_owned()),
            }],
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["nodes"][0]["modId"], "armor-pack");
        assert_eq!(value["nodes"][0]["name"], "Armor Pack");
        assert_eq!(value["edges"][0]["sourceModId"], "armor-pack");
        assert_eq!(value["edges"][0]["dependency"], "stracker-loader");
        assert_eq!(value["edges"][0]["matchedImportedModId"], "stracker-loader");
        assert!(value["edges"][0].get("installed").is_none());
        assert!(value["edges"][0].get("path").is_none());
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
    fn serializes_install_task_kind_as_stable_snake_case() {
        let dto: TaskStartedDto = TaskStarted {
            task_id: "install-123".to_owned(),
            kind: TaskKind::Install,
            status: TaskStatus::Queued,
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["taskId"], "install-123");
        assert_eq!(value["kind"], "install");
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

#[cfg(test)]
mod install_recovery_dto_tests {
    use super::*;
    use hmm_core::{ModId, ProfileId};

    #[test]
    fn serializes_install_recovery_summary_without_paths_or_backup_refs() {
        let dto: InstallRecoverySummaryDto = hmm_app::InstallRecoverySummary {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            status: hmm_app::InstallRecoveryStatus::RepairRequired,
            managed_file_count: 1,
            backup_count: 1,
            issue_count: 1,
            issues: vec![hmm_app::InstallRecoveryIssueSummary {
                issue: hmm_app::InstallRecoveryIssue::BackupMissing,
                count: 1,
            }],
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize recovery summary");

        assert_eq!(value["profileId"], "default");
        assert_eq!(value["modId"], "mod-a");
        assert_eq!(value["status"], "repair_required");
        assert_eq!(value["managedFileCount"], 1);
        assert_eq!(value["backupCount"], 1);
        assert_eq!(value["issueCount"], 1);
        assert_eq!(value["issues"][0]["issue"], "backup_missing");
        assert_eq!(value["issues"][0]["count"], 1);
        assert!(value.get("targetPath").is_none());
        assert!(value.get("backupRef").is_none());
        assert!(!value.to_string().contains("nativePC"));
        assert!(!value.to_string().contains("backup-original"));
    }
}
