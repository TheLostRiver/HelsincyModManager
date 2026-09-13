use hmm_app::{PluginSelectionService, PluginSelectionSources};
use hmm_infra::{JsonPluginSelectionRepository, SandboxModPackageInstallFileScanner};
use hmm_ports::{
    InstallManifestRepository, ModImportResultRepository, ModImportSandboxLocator,
    ModPackageContentRootRepository, ModPackageFileSelectionRepository, NoStoredFileSelection,
};
use std::path::Path;
use std::sync::Arc;

pub(crate) fn record_verified_plan_plugins(
    app_data: &Path,
    game_id: &hmm_core::GameId,
    profile_id: &hmm_core::ProfileId,
    mod_id: &hmm_core::ModId,
    plan: &hmm_core::InstallPlan,
) -> Result<(), hmm_app::InstallWriteAdmissionError> {
    use hmm_ports::PluginSelectionRepository;
    plan.validate_plugin_selections(game_id, profile_id)
        .map_err(|_| hmm_app::InstallWriteAdmissionError::SafetyRejected)?;
    let repository = JsonPluginSelectionRepository::new(app_data.join("install/plugin-selections"));
    for selection in &plan.plugin_selections {
        if selection.scope().mod_id != *mod_id {
            return Err(hmm_app::InstallWriteAdmissionError::SafetyRejected);
        }
        repository
            .save_selection(selection)
            .map_err(|_| hmm_app::InstallWriteAdmissionError::SafetyRejected)?;
    }
    Ok(())
}

pub(crate) fn record_verified_reinstall_plugins(
    app_data: &Path,
    approval: &hmm_app::ReinstallPluginApproval<'_>,
) -> Result<(), hmm_app::InstallWriteAdmissionError> {
    use hmm_ports::PluginSelectionRepository;
    let repository = JsonPluginSelectionRepository::new(app_data.join("install/plugin-selections"));
    for selection in approval.selections {
        let scope = selection.scope();
        if &scope.game_id != approval.game_id
            || &scope.profile_id != approval.profile_id
            || &scope.mod_id != approval.mod_id
        {
            return Err(hmm_app::InstallWriteAdmissionError::SafetyRejected);
        }
        repository
            .save_selection(selection)
            .map_err(|_| hmm_app::InstallWriteAdmissionError::SafetyRejected)?;
    }
    Ok(())
}

pub(crate) fn plugin_selection_service(
    app_data: &Path,
    catalog: Arc<dyn ModImportResultRepository>,
    sandboxes: Arc<dyn ModImportSandboxLocator>,
    content_roots: Arc<dyn ModPackageContentRootRepository>,
    package_selection: Arc<dyn ModPackageFileSelectionRepository>,
    manifests: Arc<dyn InstallManifestRepository>,
    read_only: bool,
) -> Arc<PluginSelectionService> {
    let raw = Arc::new(SandboxModPackageInstallFileScanner::new(
        content_roots,
        Arc::new(NoStoredFileSelection),
    ));
    let root = app_data.join("install/plugin-selections");
    let selections = if read_only {
        JsonPluginSelectionRepository::read_only(root)
    } else {
        JsonPluginSelectionRepository::new(root)
    };
    Arc::new(PluginSelectionService::new(PluginSelectionSources {
        catalog,
        sandboxes,
        scanner: raw.clone(),
        reader: raw,
        package_selection,
        selections: Arc::new(selections),
        manifests,
        policies: vec![Arc::new(hmm_games_mhw::MhwPluginPolicy)],
    }))
}
