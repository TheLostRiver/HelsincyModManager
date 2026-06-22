mod game_setup;
mod mod_import;
mod preview_image;

pub use game_setup::{
    GameCandidateScan, GameSetupCandidate, GameSetupService, GameSetupServiceError,
};
pub use mod_import::{
    ImportPreviewImage, ImportPreviewImageProcessor, ModImportAnalysisRequest,
    ModImportAnalysisResult, ModImportAnalysisService,
};
pub use preview_image::PreviewImageService;

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
