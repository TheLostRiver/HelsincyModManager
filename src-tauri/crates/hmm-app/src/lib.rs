mod game_setup;
mod mod_import;
mod mod_import_task;
mod preview_image;
mod task_manager;

pub use game_setup::{
    GameCandidateScan, GameSetupCandidate, GameSetupService, GameSetupServiceError,
};
pub use mod_import::{
    ImportPreviewImage, ImportPreviewImageProcessor, ModImportAnalysisRequest,
    ModImportAnalysisResult, ModImportAnalysisService,
};
pub use mod_import_task::{
    ModImportTaskError, ModImportTaskService, StartImportModTaskRequest, TaskStarted,
};
pub use preview_image::PreviewImageService;
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
