mod app_settings;
mod category;
mod game_launch;
mod game_setup;
mod install;
mod install_manifest_query;
mod install_recovery;
mod install_task;
mod log_diagnostics;
mod mod_dependency_graph;
mod mod_import;
mod mod_import_diagnostics;
mod mod_import_task;
mod mod_metadata;
mod preview_image;
mod profile;
mod reinstall;
mod reinstall_commit;
mod reinstall_task;
mod replacement;
mod replacement_task;
mod save_backup;
mod save_backup_background;
mod save_backup_background_worker;
mod save_backup_exit_guard;
mod save_backup_scheduler;
mod save_backup_task;
mod save_directory_discovery;
mod support_diagnostics;
mod task_manager;

pub use app_settings::{AppSettingsService, AppSettingsServiceError};
pub use category::{CategoryService, CategoryWithCount};
pub use game_launch::{GameLaunchService, GameLaunchServiceError};
pub use game_setup::{
    GameAutoDetection, GameAutoDetectionOutcome, GameCandidateScan, GameSetupCandidate,
    GameSetupService, GameSetupServiceError,
};
pub use install::{
    BuildImportedModInstallPlanRequest, BuildInstallPlanRequest, CommitInstallPlanRequest,
    InstallCommitError, InstallCommitPhase, InstallCommitResult, InstallCommitService,
    InstallPlanFile, InstallPlanningError, InstallPlanningService, UninstallModError,
    UninstallModRequest, UninstallModResult, UninstallModService,
};
pub use install_manifest_query::{
    InstallManifestQueryError, InstallManifestQueryRequest, InstallManifestQueryService,
    InstallManifestStatus, InstallManifestStatusSummary,
};
pub use install_recovery::{
    InstallRecoveryActionAvailability, InstallRecoveryActionBlockReason,
    InstallRecoveryActionBlockReasonSummary, InstallRecoveryActionError, InstallRecoveryActionKind,
    InstallRecoveryActionPhase, InstallRecoveryActionPreview, InstallRecoveryActionPreviewError,
    InstallRecoveryActionPreviewRequest, InstallRecoveryActionPreviewService,
    InstallRecoveryActionRequest, InstallRecoveryActionResult, InstallRecoveryActionService,
    InstallRecoveryIssue, InstallRecoveryIssueSummary, InstallRecoveryScanError,
    InstallRecoveryScanRequest, InstallRecoveryScanService, InstallRecoveryStatus,
    InstallRecoverySummary,
};
pub use install_task::{
    GameProfileWriteLockRegistry, ImportedModInstallCommitRequest, ImportedModInstallPlanner,
    InstallPlanCommitter, InstallRecoveryActionExecutor, InstallTaskRunError, InstallTaskRunner,
    InstallTaskService, InstallWriteAdmission, InstallWriteAdmissionError, ModUninstaller,
    RecoveryActionTaskRunError, RecoveryActionTaskRunner, RecoveryActionTaskService,
    ReinstallRecoveryWriteAdmission, StartInstallTaskRequest, StartRecoveryActionTaskRequest,
    StartUninstallTaskRequest, UninstallTaskRunError, UninstallTaskRunner, UninstallTaskService,
};
pub use log_diagnostics::{
    AuditLogDiagnosticsExport, AuditLogDiagnosticsExportService, MAX_AUDIT_LOG_DIAGNOSTIC_EVENTS,
};
pub use mod_dependency_graph::{
    ModDependencyGraph, ModDependencyGraphEdge, ModDependencyGraphNode, ModDependencyGraphService,
};
pub use mod_import::{
    ImportPreviewImage, ImportPreviewImageProcessor, ModDetail, ModImportAnalysisRequest,
    ModImportAnalysisResult, ModImportAnalysisService, ModImportPrepareRequest,
    ModImportPrepareResult, ModImportPrepareService, ModImportTaskRunError, ModImportTaskRunner,
    ModLibraryItem, ModLibraryService, ModLibraryStatus, ModPackageMetadataSummary,
    ModRevisionList, ThumbnailCacheMaintenanceScheduler,
    DEFAULT_THUMBNAIL_CACHE_MAINTENANCE_INTERVAL,
};
pub use mod_import_diagnostics::{
    PreviewImageDiagnosticExportCategory, PreviewImageDiagnosticExportCategoryId,
    PreviewImageDiagnosticExportCategoryStatus, PreviewImageDiagnosticExportExclusionReason,
    PreviewImageDiagnosticsExport, PreviewImageDiagnosticsExportService,
    PreviewImageDiagnosticsSummary, PreviewImageFallbackDiagnostic,
};
pub use mod_import_task::{
    ModImportTaskError, ModImportTaskService, StartImportModRevisionTaskRequest,
    StartImportModTaskRequest, TaskStarted,
};
pub use mod_metadata::{ModMetadataService, UpdateModMetadataRequest};
pub use preview_image::{
    LimitedPreviewImageProcessor, PreviewImageCandidateList, PreviewImageCandidateListService,
    PreviewImageCandidateSelectionService, PreviewImageCandidateSummary, PreviewImageDetailService,
    PreviewImageService, DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY,
};
pub use profile::{
    CreateProfileRequest, ProfileService, SetProfileSaveSettingsRequest, UpdateProfileRequest,
};
pub use reinstall::{
    InstalledReplacementReinstallContext, InstalledReplacementReinstallResolution,
    PreparedReinstall, ReinstallBlockingReason, ReinstallBlockingReasonSummary,
    ReinstallCandidatePlanError, ReinstallCandidatePlanRequest, ReinstallCandidatePlanner,
    ReinstallCandidateSourceReader, ReinstallPlanPreview, ReinstallPreparation,
    ReinstallPreviewError, ReinstallPreviewRequest, ReinstallPreviewService,
    ReinstallPreviewStatus, ReinstallRevisionSummary, ReinstallTargetCounts,
};
pub use reinstall_commit::{
    ReinstallCommitError, ReinstallCommitPhase, ReinstallCommitResult, ReinstallCommitService,
};
pub use reinstall_task::{
    ReinstallTaskAuditContext, ReinstallTaskExecutor, ReinstallTaskExecutorService,
    ReinstallTaskPrepareError, ReinstallTaskPrepared, ReinstallTaskRunError, ReinstallTaskRunner,
    ReinstallTaskService, RetargetReinstallTaskExecutor, StartReinstallTaskRequest,
    StartRetargetReinstallTaskRequest,
};
pub use replacement::{
    AnalyzeImportedReplacementRequest, InitialRetargetInstallStatusError,
    InitialRetargetInstallStatusReader, MaterializeRetargetRequest, MaterializedRetarget,
    PlannedInitialRetargetInstall, PlannedRetargetReinstall, PreviewInitialRetargetInstallRequest,
    PreviewRetargetReinstallRequest, ReplacementService, ReplacementServiceError,
    ReplacementWorkflowError, ReplacementWorkflowService, RetargetMaterializeError,
    RetargetReinstallRequest,
};
pub use replacement_task::{
    InitialRetargetInstallPlanner, RetargetInstallTaskRunError, RetargetInstallTaskRunner,
    RetargetInstallTaskService, StartRetargetInstallTaskRequest,
};
pub use save_backup::{
    CreateSaveBackupRequest, CreateSaveBackupResult, SaveBackupError, SaveBackupService,
    SaveBackupWarning,
};
pub use save_backup_background::{
    SaveBackupBackgroundControlStatus, SaveBackupBackgroundRegistrationResult,
    SaveBackupBackgroundService, SaveBackupBackgroundServiceError, SaveBackupBackgroundStatus,
    SAVE_BACKUP_BACKGROUND_HEARTBEAT_TTL_MILLIS, SAVE_BACKUP_BACKGROUND_STARTUP_GRACE_MILLIS,
};
pub use save_backup_background_worker::{
    SaveBackupBackgroundWorker, SaveBackupBackgroundWorkerError,
    SaveBackupBackgroundWorkerRunSummary,
};
pub use save_backup_exit_guard::{
    SaveBackupExitDecision, SaveBackupExitGuard, SaveBackupExitGuardError, SaveBackupExitReason,
};
pub use save_backup_scheduler::{
    SaveBackupAutoCheckRequest, SaveBackupAutoCheckResult, SaveBackupAutoCheckStatus,
    SaveBackupAutoSchedulerError, SaveBackupAutoSchedulerService,
};
pub use save_backup_task::{
    SaveBackupExecutor, SaveBackupTaskRunError, SaveBackupTaskRunner, SaveBackupTaskScopeRegistry,
    SaveBackupTaskService, StartSaveBackupTaskRequest,
};
pub use save_directory_discovery::{
    ConfirmProfileSaveDirectoryCandidateRequest, DiscoverProfileSaveDirectoriesRequest,
    ProfileSaveDirectoryDiscoveryService, SaveDirectoryDiscoveryError,
};
pub use support_diagnostics::{
    DiagnosticsPageSnapshot, SupportDiagnosticsExport, SupportDiagnosticsExportService,
    MAX_DIAGNOSTICS_PAGE_ITEMS, MAX_SUPPORT_DIAGNOSTIC_TEXT_LOG_LINES,
};
pub use task_manager::{
    TaskKind, TaskManager, TaskManagerError, TaskProgressEvent, TaskSnapshot, TaskStatus,
};

pub fn app_name() -> &'static str {
    "Helsincy Mod Manager"
}

#[cfg(test)]
mod tests {
    use super::app_name;

    #[test]
    fn app_name_is_stable() {
        assert_eq!(app_name(), "Helsincy Mod Manager");
    }
}

#[cfg(test)]
#[path = "reinstall_tests.rs"]
mod reinstall_tests;
