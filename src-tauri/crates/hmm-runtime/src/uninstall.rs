use hmm_app::{
    ModUninstaller, StartUninstallTaskRequest, UninstallModError, UninstallModRequest,
    UninstallModResult, UninstallModService,
};
use hmm_core::ModRevisionId;
use hmm_infra::{
    FileSystemInstallBackupStore, FileSystemInstallGameFileSystem, JsonInstallManifestRepository,
};
use hmm_ports::{GameConfigRepository, GameRunningDetector};
use std::path::PathBuf;
use std::sync::Arc;

pub(super) fn mod_uninstaller(
    game_config_repository: Arc<dyn GameConfigRepository>,
    app_data_dir: PathBuf,
    game_running_detector: Arc<dyn GameRunningDetector>,
) -> Arc<dyn ModUninstaller> {
    Arc::new(ConfiguredModUninstaller::new(
        game_config_repository,
        app_data_dir,
        game_running_detector,
    ))
}

struct ConfiguredModUninstaller {
    game_config_repository: Arc<dyn GameConfigRepository>,
    app_data_dir: PathBuf,
    game_running_detector: Arc<dyn GameRunningDetector>,
}

impl ConfiguredModUninstaller {
    fn new(
        game_config_repository: Arc<dyn GameConfigRepository>,
        app_data_dir: PathBuf,
        game_running_detector: Arc<dyn GameRunningDetector>,
    ) -> Self {
        Self {
            game_config_repository,
            app_data_dir,
            game_running_detector,
        }
    }

    fn service_for_request(
        &self,
        request: &StartUninstallTaskRequest,
    ) -> Result<UninstallModService, UninstallModError> {
        let game_instance = self
            .game_config_repository
            .load_game_instance(&request.game_id)
            .map_err(|_| UninstallModError::GameInstanceUnavailable)?
            .ok_or(UninstallModError::GameInstanceUnavailable)?;
        Ok(UninstallModService::new(
            Arc::new(FileSystemInstallGameFileSystem::new(game_instance.root_dir)),
            Arc::new(FileSystemInstallBackupStore::new(
                self.app_data_dir.join("install").join("backups"),
            )),
            Arc::new(JsonInstallManifestRepository::new(
                self.app_data_dir.join("install").join("manifests"),
            )),
        )
        .with_game_running_detector(Arc::clone(&self.game_running_detector)))
    }
}

impl ModUninstaller for ConfiguredModUninstaller {
    fn uninstall_mod(
        &self,
        request: StartUninstallTaskRequest,
    ) -> Result<UninstallModResult, UninstallModError> {
        let service = self.service_for_request(&request)?;

        service.uninstall_mod(UninstallModRequest {
            game_id: request.game_id,
            profile_id: request.profile_id,
            mod_id: request.mod_id,
        })
    }

    fn uninstall_mod_for_revision(
        &self,
        request: StartUninstallTaskRequest,
        expected_installed_revision_id: ModRevisionId,
    ) -> Result<UninstallModResult, UninstallModError> {
        let service = self.service_for_request(&request)?;
        service.uninstall_mod_for_revision(
            UninstallModRequest {
                game_id: request.game_id,
                profile_id: request.profile_id,
                mod_id: request.mod_id,
            },
            expected_installed_revision_id,
        )
    }

    fn uninstall_mod_for_revision_and_manifest(
        &self,
        request: StartUninstallTaskRequest,
        expected_installed_revision_id: ModRevisionId,
        expected_manifest_digest: &str,
    ) -> Result<UninstallModResult, UninstallModError> {
        let service = self.service_for_request(&request)?;
        service.uninstall_mod_for_revision_and_manifest(
            UninstallModRequest {
                game_id: request.game_id,
                profile_id: request.profile_id,
                mod_id: request.mod_id,
            },
            expected_installed_revision_id,
            expected_manifest_digest,
        )
    }
}
