mod app_settings;
mod game_setup;
mod install;
mod log_diagnostics;
mod mod_dependency_graph;
mod mod_import;
mod mod_import_diagnostics;
mod mod_import_task;
mod preview_image;
mod support_diagnostics;
mod task_manager;

pub use app_settings::{AppSettingsService, AppSettingsServiceError};
pub use game_setup::{
    GameCandidateScan, GameSetupCandidate, GameSetupService, GameSetupServiceError,
};
pub use install::{
    BuildImportedModInstallPlanRequest, BuildInstallPlanRequest, CommitInstallPlanRequest,
    InstallCommitError, InstallCommitPhase, InstallCommitResult, InstallCommitService,
    InstallPlanFile, InstallPlanningError, InstallPlanningService,
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
    ThumbnailCacheMaintenanceScheduler, DEFAULT_THUMBNAIL_CACHE_MAINTENANCE_INTERVAL,
};
pub use mod_import_diagnostics::{
    PreviewImageDiagnosticExportCategory, PreviewImageDiagnosticExportCategoryId,
    PreviewImageDiagnosticExportCategoryStatus, PreviewImageDiagnosticExportExclusionReason,
    PreviewImageDiagnosticsExport, PreviewImageDiagnosticsExportService,
    PreviewImageDiagnosticsSummary, PreviewImageFallbackDiagnostic,
};
pub use mod_import_task::{
    ModImportTaskError, ModImportTaskService, StartImportModTaskRequest, TaskStarted,
};
pub use preview_image::{
    LimitedPreviewImageProcessor, PreviewImageCandidateList, PreviewImageCandidateListService,
    PreviewImageCandidateSelectionService, PreviewImageCandidateSummary, PreviewImageDetailService,
    PreviewImageService, DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY,
};
pub use support_diagnostics::{
    SupportDiagnosticsExport, SupportDiagnosticsExportService,
    MAX_SUPPORT_DIAGNOSTIC_TEXT_LOG_LINES,
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
