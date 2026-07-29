use hmm_app::{
    ModUninstaller, StartUninstallTaskRequest, UninstallModError, UninstallModRequest,
    UninstallModResult, UninstallModService,
};
use hmm_infra::{
    FileSystemInstallBackupStore, FileSystemInstallGameFileSystem, JsonInstallManifestRepository,
};
use hmm_ports::GameConfigRepository;
use std::path::PathBuf;
use std::sync::Arc;

pub(super) fn mod_uninstaller(
    game_config_repository: Arc<dyn GameConfigRepository>,
    app_data_dir: PathBuf,
) -> Arc<dyn ModUninstaller> {
    Arc::new(ConfiguredModUninstaller::new(
        game_config_repository,
        app_data_dir,
    ))
}

struct ConfiguredModUninstaller {
    game_config_repository: Arc<dyn GameConfigRepository>,
    app_data_dir: PathBuf,
}

impl ConfiguredModUninstaller {
    fn new(game_config_repository: Arc<dyn GameConfigRepository>, app_data_dir: PathBuf) -> Self {
        Self {
            game_config_repository,
            app_data_dir,
        }
    }
}

impl ModUninstaller for ConfiguredModUninstaller {
    fn uninstall_mod(
        &self,
        request: StartUninstallTaskRequest,
    ) -> Result<UninstallModResult, UninstallModError> {
        let game_instance = self
            .game_config_repository
            .load_game_instance(&request.game_id)
            .map_err(|_| UninstallModError::GameInstanceUnavailable)?
            .ok_or(UninstallModError::GameInstanceUnavailable)?;
        let service = UninstallModService::new(
            Arc::new(FileSystemInstallGameFileSystem::new(game_instance.root_dir)),
            Arc::new(FileSystemInstallBackupStore::new(
                self.app_data_dir.join("install").join("backups"),
            )),
            Arc::new(JsonInstallManifestRepository::new(
                self.app_data_dir.join("install").join("manifests"),
            )),
        );

        service.uninstall_mod(UninstallModRequest {
            profile_id: request.profile_id,
            mod_id: request.mod_id,
        })
    }
}
