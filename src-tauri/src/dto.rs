use crate::thumbnail_protocol::to_loadable_thumbnail_url;
use hmm_app::{
    AppSettingsServiceError, CategoryWithCount, GameAutoDetection, GameAutoDetectionOutcome,
    GameCandidateScan, GamePrerequisiteDecision, GamePrerequisiteDecisionCode,
    GamePrerequisiteDecisionStatus, GameSetupCandidate, GameSetupServiceError, ImportPreviewImage,
    ImportedModInstallPreflight, InstallManifestStatusSummary, InstallRecoveryActionAvailability,
    InstallRecoveryActionBlockReason, InstallRecoveryActionBlockReasonSummary,
    InstallRecoveryActionPreview, InstallRecoveryIssue, InstallRecoveryIssueSummary,
    InstallRecoverySummary, ModDetail, ModImportTaskError, TaskKind, TaskManagerError,
    TaskProgressEvent, TaskStarted, TaskStatus,
};
use hmm_core::{
    BackupCadence, GameDirectoryEvidence, GameDirectoryEvidenceKind, GameDirectoryStatus,
    GameDirectoryValidation, GameInstance, GameSetupErrorCode, GameSetupStatus, InstallAction,
    InstallConflict, InstallFileProvider, InstallPlan, PreviewImageRejectionReason,
    ProfileBackupRetention, ProfileBackupSchedule, ProfileDirectoryMode, ProfileDirectorySelection,
    ProfileDirectoryStatus, ProfileSaveSettings,
};
use hmm_ports::{AppSettings, GameCandidateSource};
use serde::{Deserialize, Serialize};

pub use crate::mod_library_dto::ModLibraryItemDto;
pub use crate::reinstall_dto::{
    InstallManifestStatusDto, InstallRecoveryActionKindDto, InstallRecoveryStatusDto,
};
pub use crate::replacement_dto::{
    AnalyzeImportedModReplacementRequestDto, InitialRetargetInstallPreviewDto,
    ListReplacementTargetsRequestDto, PreviewInitialRetargetInstallRequestDto,
    PreviewRetargetReinstallRequestDto, ReplacementAnalysisDto, ReplacementSourceDto,
    ReplacementTargetDto, ReplacementWarningDto, RetargetActionPreviewDto,
    StartRetargetInstallTaskRequestDto, StartRetargetReinstallTaskRequestDto,
};

mod game_prerequisites;

pub use game_prerequisites::{prerequisite_report_to_dto, GamePrerequisiteReportDto};

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
            TaskManagerError::TaskScopeBusy { .. } => "task_scope_busy",
            TaskManagerError::TaskCreationBlocked { .. } => "task_creation_blocked",
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
            AppSettingsServiceError::InvalidLogStorageMaxBytes => "log_storage_max_bytes_invalid",
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogStorageSettingsDto {
    pub max_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugLogSettingsDto {
    pub enabled: bool,
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
    #[serde(default)]
    pub game_id: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecoveryActionPreviewRequestDto {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub action_kind: InstallRecoveryActionKindDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRecoveryActionTaskRequestDto {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub action_kind: InstallRecoveryActionKindDto,
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
pub struct ImportedModInstallPreflightDto {
    #[serde(flatten)]
    pub plan: InstallPlanPreviewDto,
    pub prerequisite_decision: GamePrerequisiteDecisionDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePrerequisiteDecisionDto {
    pub status: GamePrerequisiteDecisionStatusDto,
    pub rules_version: Option<u32>,
    pub codes: Vec<GamePrerequisiteDecisionCodeDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePrerequisiteDecisionStatusDto {
    Ready,
    Warning,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePrerequisiteDecisionCodeDto {
    GameNotConfigured,
    GameDirectoryInvalid,
    GameDirectoryNotWritable,
    RulesUnavailable,
    RulesCorrupted,
    StorageUnavailable,
    StorageCorrupted,
    UnsupportedGame,
    MissingRequiredFile,
    SignatureUnverified,
    ConfigReadFailed,
    ConfigInvalidJson,
    ConfigFieldMismatch,
    PrerequisiteDecisionInvalid,
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
    /// Exact installed revision from revisioned manifest facts (schema v2); `null` for
    /// legacy manifests, not-installed mods and recovery-derived summaries.
    pub installed_revision_id: Option<String>,
    /// Entries claimed from an external installation (#286 adopt); uninstalling deletes
    /// them with nothing to restore. Both command paths report it; the key is omitted only
    /// when the summary source does not carry the fact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adopted_file_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecoverySummaryDto {
    pub profile_id: String,
    pub mod_id: String,
    pub status: InstallRecoveryStatusDto,
    pub managed_file_count: usize,
    pub backup_count: usize,
    pub adopted_file_count: usize,
    pub issue_count: usize,
    pub issues: Vec<InstallRecoveryIssueSummaryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecoveryIssueSummaryDto {
    pub issue: InstallRecoveryIssueDto,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecoveryActionPreviewDto {
    pub profile_id: String,
    pub mod_id: String,
    pub action_kind: InstallRecoveryActionKindDto,
    pub availability: InstallRecoveryActionAvailabilityDto,
    pub remove_file_count: usize,
    pub restore_file_count: usize,
    pub backup_count: usize,
    pub blocking_issue_count: usize,
    pub blocking_reasons: Vec<InstallRecoveryActionBlockReasonSummaryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecoveryActionBlockReasonSummaryDto {
    pub reason: InstallRecoveryActionBlockReasonDto,
    pub count: usize,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallRecoveryActionAvailabilityDto {
    Available,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallRecoveryActionBlockReasonDto {
    RollbackStateMissing,
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

impl From<AppSettings> for LogStorageSettingsDto {
    fn from(settings: AppSettings) -> Self {
        Self {
            max_bytes: settings.log_storage_max_bytes,
        }
    }
}

impl From<AppSettings> for DebugLogSettingsDto {
    fn from(settings: AppSettings) -> Self {
        Self {
            enabled: settings.debug_log_enabled,
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

impl From<ImportedModInstallPreflight> for ImportedModInstallPreflightDto {
    fn from(preflight: ImportedModInstallPreflight) -> Self {
        Self {
            plan: preflight.plan.into(),
            prerequisite_decision: preflight.prerequisite_decision.into(),
        }
    }
}

impl From<GamePrerequisiteDecision> for GamePrerequisiteDecisionDto {
    fn from(decision: GamePrerequisiteDecision) -> Self {
        Self {
            status: decision.status.into(),
            rules_version: decision.rules_version,
            codes: decision.codes.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<GamePrerequisiteDecisionStatus> for GamePrerequisiteDecisionStatusDto {
    fn from(status: GamePrerequisiteDecisionStatus) -> Self {
        match status {
            GamePrerequisiteDecisionStatus::Ready => Self::Ready,
            GamePrerequisiteDecisionStatus::Warning => Self::Warning,
            GamePrerequisiteDecisionStatus::Blocked => Self::Blocked,
        }
    }
}

impl From<GamePrerequisiteDecisionCode> for GamePrerequisiteDecisionCodeDto {
    fn from(code: GamePrerequisiteDecisionCode) -> Self {
        match code {
            GamePrerequisiteDecisionCode::GameNotConfigured => Self::GameNotConfigured,
            GamePrerequisiteDecisionCode::GameDirectoryInvalid => Self::GameDirectoryInvalid,
            GamePrerequisiteDecisionCode::GameDirectoryNotWritable => {
                Self::GameDirectoryNotWritable
            }
            GamePrerequisiteDecisionCode::RulesUnavailable => Self::RulesUnavailable,
            GamePrerequisiteDecisionCode::RulesCorrupted => Self::RulesCorrupted,
            GamePrerequisiteDecisionCode::StorageUnavailable => Self::StorageUnavailable,
            GamePrerequisiteDecisionCode::StorageCorrupted => Self::StorageCorrupted,
            GamePrerequisiteDecisionCode::UnsupportedGame => Self::UnsupportedGame,
            GamePrerequisiteDecisionCode::MissingRequiredFile => Self::MissingRequiredFile,
            GamePrerequisiteDecisionCode::SignatureUnverified => Self::SignatureUnverified,
            GamePrerequisiteDecisionCode::ConfigReadFailed => Self::ConfigReadFailed,
            GamePrerequisiteDecisionCode::ConfigInvalidJson => Self::ConfigInvalidJson,
            GamePrerequisiteDecisionCode::ConfigFieldMismatch => Self::ConfigFieldMismatch,
            GamePrerequisiteDecisionCode::DecisionInvalid => Self::PrerequisiteDecisionInvalid,
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
            installed_revision_id: summary
                .installed_revision_id
                .map(|revision| revision.as_str().to_owned()),
            adopted_file_count: summary.adopted_file_count,
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
            adopted_file_count: summary.adopted_file_count,
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

impl From<InstallRecoveryActionPreview> for InstallRecoveryActionPreviewDto {
    fn from(preview: InstallRecoveryActionPreview) -> Self {
        Self {
            profile_id: preview.profile_id.as_str().to_owned(),
            mod_id: preview.mod_id.as_str().to_owned(),
            action_kind: preview.action_kind.into(),
            availability: preview.availability.into(),
            remove_file_count: preview.remove_file_count,
            restore_file_count: preview.restore_file_count,
            backup_count: preview.backup_count,
            blocking_issue_count: preview.blocking_issue_count,
            blocking_reasons: preview
                .blocking_reasons
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<InstallRecoveryActionBlockReasonSummary> for InstallRecoveryActionBlockReasonSummaryDto {
    fn from(summary: InstallRecoveryActionBlockReasonSummary) -> Self {
        Self {
            reason: summary.reason.into(),
            count: summary.count,
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

impl From<InstallRecoveryActionAvailability> for InstallRecoveryActionAvailabilityDto {
    fn from(availability: InstallRecoveryActionAvailability) -> Self {
        match availability {
            InstallRecoveryActionAvailability::Available => Self::Available,
            InstallRecoveryActionAvailability::Blocked => Self::Blocked,
        }
    }
}

impl From<InstallRecoveryActionBlockReason> for InstallRecoveryActionBlockReasonDto {
    fn from(reason: InstallRecoveryActionBlockReason) -> Self {
        match reason {
            InstallRecoveryActionBlockReason::RollbackStateMissing => Self::RollbackStateMissing,
            InstallRecoveryActionBlockReason::MissingInstalledFileSummary => {
                Self::MissingInstalledFileSummary
            }
            InstallRecoveryActionBlockReason::TargetMissing => Self::TargetMissing,
            InstallRecoveryActionBlockReason::TargetChanged => Self::TargetChanged,
            InstallRecoveryActionBlockReason::TargetReadFailed => Self::TargetReadFailed,
            InstallRecoveryActionBlockReason::BackupMissing => Self::BackupMissing,
            InstallRecoveryActionBlockReason::BackupReadFailed => Self::BackupReadFailed,
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
    SaveBackup,
    SaveRestore,
    ExternalStateScan,
    ExternalModAdopt,
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
pub struct GameAutoDetectionDto {
    pub game_id: String,
    pub outcome: String,
    pub status: GameSetupStatusDto,
    pub error_code: Option<String>,
    pub candidate_count: usize,
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
pub struct CategoryLabelDto {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDto {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryWithCountDto {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i32,
    pub mod_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupCadenceDto {
    Manual,
    Daily,
    Weekly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileDirectoryModeDto {
    Unset,
    Custom,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileDirectoryStatusDto {
    Unset,
    Valid,
    Invalid,
    Defaulted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDirectorySelectionDto {
    pub mode: ProfileDirectoryModeDto,
    pub status: ProfileDirectoryStatusDto,
    pub path_label: Option<String>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileBackupScheduleDto {
    pub cadence: BackupCadenceDto,
    pub hour: Option<u8>,
    pub minute: Option<u8>,
    pub weekdays: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileBackupRetentionDto {
    pub max_count: u32,
    pub max_age_days: Option<u32>,
    pub max_total_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamAccountDisplaySummaryDto {
    pub account_name: Option<String>,
    pub avatar_url: Option<String>,
    pub account_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSaveSettingsDto {
    pub profile_id: String,
    pub save_directory: ProfileDirectorySelectionDto,
    pub backup_directory: ProfileDirectorySelectionDto,
    pub schedule: ProfileBackupScheduleDto,
    pub retention: ProfileBackupRetentionDto,
    pub steam_account: Option<SteamAccountDisplaySummaryDto>,
    pub pre_restore_backup_enabled: bool,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetProfileSaveSettingsRequestDto {
    pub game_id: String,
    pub profile_id: String,
    #[serde(default)]
    pub save_directory: Option<String>,
    #[serde(default)]
    pub backup_directory: Option<String>,
    pub schedule: ProfileBackupScheduleDto,
    pub retention: ProfileBackupRetentionDto,
    #[serde(default = "default_pre_restore_backup_enabled")]
    pub pre_restore_backup_enabled: bool,
}

const fn default_pre_restore_backup_enabled() -> bool {
    true
}

impl From<hmm_core::Category> for CategoryDto {
    fn from(c: hmm_core::Category) -> Self {
        Self {
            id: c.id,
            name: c.name,
            color: c.color,
            sort_order: c.sort_order,
        }
    }
}

impl From<CategoryWithCount> for CategoryWithCountDto {
    fn from(c: CategoryWithCount) -> Self {
        Self {
            id: c.category.id,
            name: c.category.name,
            color: c.category.color,
            sort_order: c.category.sort_order,
            mod_count: c.mod_count,
        }
    }
}

impl From<hmm_core::Profile> for ProfileDto {
    fn from(profile: hmm_core::Profile) -> Self {
        Self {
            id: profile.id,
            name: profile.name,
            description: profile.description,
            is_active: profile.is_active,
            created_at: profile.created_at as u64,
            updated_at: profile.updated_at as u64,
        }
    }
}

impl From<BackupCadence> for BackupCadenceDto {
    fn from(cadence: BackupCadence) -> Self {
        match cadence {
            BackupCadence::Manual => Self::Manual,
            BackupCadence::Daily => Self::Daily,
            BackupCadence::Weekly => Self::Weekly,
        }
    }
}

impl From<BackupCadenceDto> for BackupCadence {
    fn from(cadence: BackupCadenceDto) -> Self {
        match cadence {
            BackupCadenceDto::Manual => Self::Manual,
            BackupCadenceDto::Daily => Self::Daily,
            BackupCadenceDto::Weekly => Self::Weekly,
        }
    }
}

impl From<ProfileDirectoryMode> for ProfileDirectoryModeDto {
    fn from(mode: ProfileDirectoryMode) -> Self {
        match mode {
            ProfileDirectoryMode::Unset => Self::Unset,
            ProfileDirectoryMode::Custom => Self::Custom,
            ProfileDirectoryMode::Default => Self::Default,
        }
    }
}

impl From<ProfileDirectoryStatus> for ProfileDirectoryStatusDto {
    fn from(status: ProfileDirectoryStatus) -> Self {
        match status {
            ProfileDirectoryStatus::Unset => Self::Unset,
            ProfileDirectoryStatus::Valid => Self::Valid,
            ProfileDirectoryStatus::Invalid => Self::Invalid,
            ProfileDirectoryStatus::Defaulted => Self::Defaulted,
        }
    }
}

impl From<ProfileDirectorySelection> for ProfileDirectorySelectionDto {
    fn from(selection: ProfileDirectorySelection) -> Self {
        Self {
            mode: selection.mode.into(),
            status: selection.status.into(),
            path_label: selection.path_label,
            messages: selection.messages,
        }
    }
}

impl From<ProfileBackupSchedule> for ProfileBackupScheduleDto {
    fn from(schedule: ProfileBackupSchedule) -> Self {
        Self {
            cadence: schedule.cadence.into(),
            hour: schedule.hour,
            minute: schedule.minute,
            weekdays: schedule.weekdays,
        }
    }
}

impl From<ProfileBackupScheduleDto> for ProfileBackupSchedule {
    fn from(schedule: ProfileBackupScheduleDto) -> Self {
        Self {
            cadence: schedule.cadence.into(),
            hour: schedule.hour,
            minute: schedule.minute,
            weekdays: schedule.weekdays,
        }
    }
}

impl From<ProfileBackupRetention> for ProfileBackupRetentionDto {
    fn from(retention: ProfileBackupRetention) -> Self {
        Self {
            max_count: retention.max_count,
            max_age_days: retention.max_age_days,
            max_total_bytes: retention.max_total_bytes,
        }
    }
}

impl From<ProfileBackupRetentionDto> for ProfileBackupRetention {
    fn from(retention: ProfileBackupRetentionDto) -> Self {
        Self {
            max_count: retention.max_count,
            max_age_days: retention.max_age_days.filter(|value| *value != 0),
            max_total_bytes: retention.max_total_bytes.filter(|value| *value != 0),
        }
    }
}

impl From<ProfileSaveSettings> for ProfileSaveSettingsDto {
    fn from(settings: ProfileSaveSettings) -> Self {
        Self {
            profile_id: settings.profile_id,
            save_directory: settings.save_directory.into(),
            backup_directory: settings.backup_directory.into(),
            schedule: settings.schedule.into(),
            retention: settings.retention.into(),
            steam_account: settings.steam_account.map(Into::into),
            pre_restore_backup_enabled: settings.pre_restore_backup_enabled,
            updated_at: settings.updated_at as u64,
        }
    }
}

impl From<hmm_core::SteamAccountDisplaySummary> for SteamAccountDisplaySummaryDto {
    fn from(summary: hmm_core::SteamAccountDisplaySummary) -> Self {
        Self {
            account_name: summary.account_name,
            avatar_url: summary.avatar_url,
            account_label: summary.account_label,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDetailDto {
    pub id: String,
    pub name: String,
    pub package_id: String,
    pub metadata: ModPackageMetadataDto,
    pub description: Option<String>,
    pub nexus_mod_id: Option<u64>,
    pub preview_image: PreviewImageDto,
    pub origin: ModOriginDto,
}

/// 脱敏来源摘要:只出 adapter/batch 的稳定 ID 与导入时间,
/// `sourceItemKeyHash`/`contentFingerprint` 等私有摘要不得出现。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModOriginDto {
    pub kind: ModOriginKindDto,
    pub adapter_id: Option<String>,
    pub batch_id: Option<String>,
    pub imported_at_unix_millis: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModOriginKindDto {
    Imported,
    ExternalImport,
    MigratedV1,
}

impl From<hmm_app::ModOriginSummary> for ModOriginDto {
    fn from(origin: hmm_app::ModOriginSummary) -> Self {
        match origin {
            hmm_app::ModOriginSummary::Imported => Self {
                kind: ModOriginKindDto::Imported,
                adapter_id: None,
                batch_id: None,
                imported_at_unix_millis: None,
            },
            hmm_app::ModOriginSummary::ExternalImport {
                adapter_id,
                batch_id,
                imported_at_unix_millis,
            } => Self {
                kind: ModOriginKindDto::ExternalImport,
                adapter_id: Some(adapter_id),
                batch_id: Some(batch_id),
                imported_at_unix_millis: Some(imported_at_unix_millis),
            },
            hmm_app::ModOriginSummary::MigratedV1 => Self {
                kind: ModOriginKindDto::MigratedV1,
                adapter_id: None,
                batch_id: None,
                imported_at_unix_millis: None,
            },
        }
    }
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
    pub debug_log_line_count: usize,
    pub task_log_line_count: usize,
    pub audit_event_count: usize,
    pub debug_log_status: String,
    pub task_log_status: String,
    pub audit_log_status: String,
    pub log_storage_status: String,
    pub debug_log_event_rejected_count: u64,
    pub debug_log_write_failure_count: u64,
    pub debug_log_retention_failure_count: u64,
    pub task_log_write_failure_count: u64,
    pub task_log_retention_failure_count: u64,
    pub audit_write_failure_count: u64,
    pub audit_write_failure_after_commit_count: u64,
    pub audit_log_retention_failure_count: u64,
    pub log_storage_failure_count: u64,
    pub log_storage_unsatisfied_count: u64,
    pub log_storage_settings_failure_count: u64,
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
                // WebView2 cannot load `thumbnail://`; rewrite only on the way out.
                thumbnail_url: to_loadable_thumbnail_url(&thumbnail_url),
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

impl From<ModDetail> for ModDetailDto {
    fn from(detail: ModDetail) -> Self {
        Self {
            id: detail.id,
            name: detail.name,
            package_id: detail.package_id,
            metadata: detail.metadata.into(),
            description: detail.description,
            nexus_mod_id: detail.nexus_mod_id,
            preview_image: detail.preview_image.into(),
            origin: detail.origin.into(),
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
            debug_log_line_count: export.debug_log_line_count,
            task_log_line_count: export.task_log_line_count,
            audit_event_count: export.audit_event_count,
            debug_log_status: export.evidence_health.debug_log_status,
            task_log_status: export.evidence_health.task_log_status,
            audit_log_status: export.evidence_health.audit_log_status,
            log_storage_status: export.evidence_health.log_storage_status,
            debug_log_event_rejected_count: export.evidence_health.debug_log_event_rejected_count,
            debug_log_write_failure_count: export.evidence_health.debug_log_write_failure_count,
            debug_log_retention_failure_count: export
                .evidence_health
                .debug_log_retention_failure_count,
            task_log_write_failure_count: export.evidence_health.task_log_write_failure_count,
            task_log_retention_failure_count: export
                .evidence_health
                .task_log_retention_failure_count,
            audit_write_failure_count: export.evidence_health.audit_write_failure_count,
            audit_write_failure_after_commit_count: export
                .evidence_health
                .audit_write_failure_after_commit_count,
            audit_log_retention_failure_count: export
                .evidence_health
                .audit_log_retention_failure_count,
            log_storage_failure_count: export.evidence_health.log_storage_failure_count,
            log_storage_unsatisfied_count: export.evidence_health.log_storage_unsatisfied_count,
            log_storage_settings_failure_count: export
                .evidence_health
                .log_storage_settings_failure_count,
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
            TaskKind::SaveBackup => Self::SaveBackup,
            TaskKind::SaveRestore => Self::SaveRestore,
            TaskKind::ExternalStateScan => Self::ExternalStateScan,
            TaskKind::ExternalModAdopt => Self::ExternalModAdopt,
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

pub fn auto_detection_to_dto(detection: GameAutoDetection) -> GameAutoDetectionDto {
    GameAutoDetectionDto {
        game_id: detection.game_id.as_str().to_owned(),
        outcome: auto_detection_outcome_to_string(detection.outcome),
        status: status_to_dto(detection.status),
        error_code: detection.error_code.map(error_code_to_string),
        candidate_count: detection.candidate_count,
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

fn auto_detection_outcome_to_string(outcome: GameAutoDetectionOutcome) -> String {
    match outcome {
        GameAutoDetectionOutcome::AlreadyConfigured => "already_configured",
        GameAutoDetectionOutcome::DetectedAndSaved => "detected_and_saved",
        GameAutoDetectionOutcome::NotFound => "not_found",
        GameAutoDetectionOutcome::InvalidCandidate => "invalid_candidate",
        GameAutoDetectionOutcome::ScanFailed => "scan_failed",
    }
    .to_owned()
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
        GameSetupErrorCode::DirectoryOverlapsModStorage => "directory_overlaps_mod_storage",
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
#[path = "dto_tests.rs"]
mod tests;
