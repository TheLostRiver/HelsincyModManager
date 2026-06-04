use hmm_core::{
    GameDirectoryStatus, GameDirectoryValidation, GameId, GameInstance, GameSetupErrorCode,
    GameSetupStatus,
};
use hmm_ports::{
    AppClock, GameAdapter, GameConfigRepository, GameConfigRepositoryError,
    GameDirectoryProbeFactory, GameDiscoveryError, GameDiscoveryService,
};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GameSetupServiceError {
    #[error("unsupported game")]
    UnsupportedGame,
    #[error("directory validation failed")]
    ValidationFailed(GameDirectoryValidation),
    #[error("storage corrupted")]
    StorageCorrupted,
    #[error("storage failed: {0}")]
    StorageFailed(String),
    #[error("scan not implemented")]
    ScanNotImplemented,
    #[error("clock failed: {0}")]
    ClockFailed(String),
}

pub struct GameSetupService {
    adapters: Vec<Arc<dyn GameAdapter>>,
    repository: Arc<dyn GameConfigRepository>,
    probe_factory: Arc<dyn GameDirectoryProbeFactory>,
    discovery: Arc<dyn GameDiscoveryService>,
    clock: Arc<dyn AppClock>,
}

impl GameSetupService {
    pub fn new(
        adapters: Vec<Arc<dyn GameAdapter>>,
        repository: Arc<dyn GameConfigRepository>,
        probe_factory: Arc<dyn GameDirectoryProbeFactory>,
        discovery: Arc<dyn GameDiscoveryService>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            adapters,
            repository,
            probe_factory,
            discovery,
            clock,
        }
    }

    pub fn get_status(&self, game_id: GameId) -> Result<GameSetupStatus, GameSetupServiceError> {
        self.require_adapter(&game_id)?;

        let instance = self
            .repository
            .load_game_instance(&game_id)
            .map_err(Self::map_storage_error)?;

        Ok(match instance {
            Some(instance) => GameSetupStatus::configured(instance),
            None => GameSetupStatus::not_configured(game_id),
        })
    }

    pub fn validate_directory(
        &self,
        game_id: GameId,
        directory: PathBuf,
    ) -> Result<GameDirectoryValidation, GameSetupServiceError> {
        let adapter = self.require_adapter(&game_id)?;
        let probe = self.probe_factory.create(directory);
        Ok(adapter.validate_directory(probe.as_ref()))
    }

    pub fn save_game_directory(
        &self,
        game_id: GameId,
        directory: PathBuf,
    ) -> Result<GameSetupStatus, GameSetupServiceError> {
        let adapter = self.require_adapter(&game_id)?;
        let validation = self.validate_directory(game_id.clone(), directory.clone())?;

        if !validation.is_valid {
            return Err(GameSetupServiceError::ValidationFailed(validation));
        }

        let instance = GameInstance {
            id: format!("{}-default", game_id.as_str()),
            game_id,
            display_name: adapter.display_name().to_owned(),
            root_dir: directory,
            status: GameDirectoryStatus::Configured,
            configured_at_unix_millis: self
                .clock
                .now_unix_millis()
                .map_err(|error| GameSetupServiceError::ClockFailed(error.to_string()))?,
        };

        self.repository
            .save_game_instance(&instance)
            .map_err(Self::map_storage_error)?;

        Ok(GameSetupStatus::configured(instance))
    }

    pub fn scan_candidates(&self, game_id: GameId) -> Result<(), GameSetupServiceError> {
        self.require_adapter(&game_id)?;
        self.discovery
            .scan_candidates(&game_id)
            .map(|_| ())
            .map_err(|error| match error {
                GameDiscoveryError::ScanNotImplemented => GameSetupServiceError::ScanNotImplemented,
                GameDiscoveryError::ScanFailed(message) => {
                    GameSetupServiceError::StorageFailed(message)
                }
            })
    }

    fn require_adapter(
        &self,
        game_id: &GameId,
    ) -> Result<Arc<dyn GameAdapter>, GameSetupServiceError> {
        self.adapters
            .iter()
            .find(|adapter| adapter.game_id() == *game_id)
            .cloned()
            .ok_or(GameSetupServiceError::UnsupportedGame)
    }

    fn map_storage_error(error: GameConfigRepositoryError) -> GameSetupServiceError {
        match error {
            GameConfigRepositoryError::StorageCorrupted => GameSetupServiceError::StorageCorrupted,
            GameConfigRepositoryError::StorageFailed(message) => {
                GameSetupServiceError::StorageFailed(message)
            }
        }
    }
}

impl GameSetupServiceError {
    pub fn error_code(&self) -> GameSetupErrorCode {
        match self {
            Self::UnsupportedGame => GameSetupErrorCode::UnsupportedGame,
            Self::ValidationFailed(validation) => validation
                .errors
                .first()
                .cloned()
                .unwrap_or(GameSetupErrorCode::Unknown),
            Self::StorageCorrupted => GameSetupErrorCode::StorageCorrupted,
            Self::StorageFailed(_) => GameSetupErrorCode::StorageFailed,
            Self::ScanNotImplemented => GameSetupErrorCode::ScanNotImplemented,
            Self::ClockFailed(_) => GameSetupErrorCode::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{GameDirectoryEvidence, GameDirectoryEvidenceKind};
    use hmm_ports::{
        GameCandidate, GameConfigRepositoryResult, GameDirectoryProbe, GameDiscoveryService,
    };
    use std::path::Path;
    use std::sync::Mutex;

    struct FakeClock;

    impl AppClock for FakeClock {
        fn now_unix_millis(&self) -> anyhow::Result<u128> {
            Ok(42)
        }
    }

    struct FakeRepository {
        stored: Mutex<Option<GameInstance>>,
    }

    impl FakeRepository {
        fn empty() -> Self {
            Self {
                stored: Mutex::new(None),
            }
        }
    }

    impl GameConfigRepository for FakeRepository {
        fn load_game_instance(
            &self,
            _game_id: &GameId,
        ) -> GameConfigRepositoryResult<Option<GameInstance>> {
            Ok(self.stored.lock().expect("fake repo lock").clone())
        }

        fn save_game_instance(&self, instance: &GameInstance) -> GameConfigRepositoryResult<()> {
            *self.stored.lock().expect("fake repo lock") = Some(instance.clone());
            Ok(())
        }
    }

    struct FakeProbe {
        root_dir: PathBuf,
    }

    impl GameDirectoryProbe for FakeProbe {
        fn root_dir(&self) -> &Path {
            &self.root_dir
        }

        fn root_exists(&self) -> bool {
            true
        }

        fn exists(&self, _relative_path: &str) -> bool {
            true
        }

        fn is_file(&self, _relative_path: &str) -> bool {
            true
        }

        fn is_dir(&self, _relative_path: &str) -> bool {
            false
        }
    }

    struct FakeProbeFactory;

    impl GameDirectoryProbeFactory for FakeProbeFactory {
        fn create(&self, directory: PathBuf) -> Box<dyn GameDirectoryProbe> {
            Box::new(FakeProbe {
                root_dir: directory,
            })
        }
    }

    struct FakeAdapter {
        valid: bool,
    }

    impl GameAdapter for FakeAdapter {
        fn game_id(&self) -> GameId {
            GameId::mhw()
        }

        fn display_name(&self) -> &'static str {
            "Monster Hunter: World - Iceborne"
        }

        fn validate_directory(&self, probe: &dyn GameDirectoryProbe) -> GameDirectoryValidation {
            let mut validation =
                GameDirectoryValidation::new(self.game_id(), probe.root_dir().to_path_buf());
            validation.add_evidence(GameDirectoryEvidence::new(
                GameDirectoryEvidenceKind::DirectoryExists,
                "目录存在",
            ));
            if !self.valid {
                validation.add_error(GameSetupErrorCode::MissingExecutable);
            }
            validation
        }
    }

    struct NoopDiscovery;

    impl GameDiscoveryService for NoopDiscovery {
        fn scan_candidates(
            &self,
            _game_id: &GameId,
        ) -> Result<Vec<GameCandidate>, GameDiscoveryError> {
            Err(GameDiscoveryError::ScanNotImplemented)
        }
    }

    fn service_with(adapter: FakeAdapter) -> GameSetupService {
        GameSetupService::new(
            vec![Arc::new(adapter)],
            Arc::new(FakeRepository::empty()),
            Arc::new(FakeProbeFactory),
            Arc::new(NoopDiscovery),
            Arc::new(FakeClock),
        )
    }

    #[test]
    fn status_is_not_configured_without_saved_instance() {
        let service = service_with(FakeAdapter { valid: true });

        let status = service.get_status(GameId::mhw()).expect("status should load");

        assert_eq!(status.status, GameDirectoryStatus::NotConfigured);
    }

    #[test]
    fn save_directory_validates_before_persisting() {
        let service = service_with(FakeAdapter { valid: true });

        let status = service
            .save_game_directory(GameId::mhw(), PathBuf::from("C:/MHW"))
            .expect("valid directory should save");

        assert_eq!(status.status, GameDirectoryStatus::Configured);
        assert_eq!(
            status
                .instance
                .expect("instance")
                .configured_at_unix_millis,
            42
        );
    }

    #[test]
    fn save_directory_rejects_invalid_validation() {
        let service = service_with(FakeAdapter { valid: false });

        let error = service
            .save_game_directory(GameId::mhw(), PathBuf::from("C:/Wrong"))
            .expect_err("invalid directory should fail");

        assert_eq!(error.error_code(), GameSetupErrorCode::MissingExecutable);
    }

    #[test]
    fn scan_candidates_returns_explicit_not_implemented() {
        let service = service_with(FakeAdapter { valid: true });

        let error = service
            .scan_candidates(GameId::mhw())
            .expect_err("scan should be disabled in first version");

        assert_eq!(error.error_code(), GameSetupErrorCode::ScanNotImplemented);
    }
}
