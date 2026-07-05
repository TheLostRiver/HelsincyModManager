use hmm_core::{
    GameDirectoryStatus, GameDirectoryValidation, GameId, GameInstance, GameSetupErrorCode,
    GameSetupStatus,
};
use hmm_ports::{
    AppClock, GameAdapter, GameCandidate, GameConfigRepository, GameConfigRepositoryError,
    GameDirectoryProbeFactory, GameDiscoveryError, GameDiscoveryRequest, GameDiscoveryService,
    GamePrerequisiteReport,
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
    #[error("candidate scan failed: {0}")]
    ScanFailed(String),
    #[error("scan not implemented")]
    ScanNotImplemented,
    #[error("clock failed: {0}")]
    ClockFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameCandidateScan {
    pub game_id: GameId,
    pub candidates: Vec<GameSetupCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSetupCandidate {
    pub candidate: GameCandidate,
    pub validation: GameDirectoryValidation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameAutoDetectionOutcome {
    AlreadyConfigured,
    DetectedAndSaved,
    NotFound,
    InvalidCandidate,
    ScanFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameAutoDetection {
    pub game_id: GameId,
    pub outcome: GameAutoDetectionOutcome,
    pub status: GameSetupStatus,
    pub error_code: Option<GameSetupErrorCode>,
    pub candidate_count: usize,
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
            Some(instance) => self.status_for_loaded_instance(instance)?,
            None => GameSetupStatus::not_configured(game_id),
        })
    }

    pub fn get_prerequisite_status(
        &self,
        game_id: GameId,
    ) -> Result<GamePrerequisiteReport, GameSetupServiceError> {
        let status = self.get_status(game_id.clone())?;

        match status.status {
            GameDirectoryStatus::NotConfigured => {
                Ok(GamePrerequisiteReport::not_configured(game_id))
            }
            GameDirectoryStatus::Invalid => Ok(GamePrerequisiteReport::game_directory_invalid(
                game_id,
                status.error_code.unwrap_or(GameSetupErrorCode::Unknown),
                status
                    .message
                    .unwrap_or_else(|| "saved game directory is no longer valid".to_owned()),
            )),
            GameDirectoryStatus::Configured => {
                let instance = status
                    .instance
                    .expect("configured status should include a game instance");
                let adapter = self.require_adapter(&game_id)?;
                let probe = self.probe_factory.create(instance.root_dir);
                Ok(adapter.inspect_prerequisites(probe.as_ref()))
            }
        }
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

    pub fn scan_candidates(
        &self,
        game_id: GameId,
    ) -> Result<GameCandidateScan, GameSetupServiceError> {
        let adapter = self.require_adapter(&game_id)?;
        let request = GameDiscoveryRequest {
            game_id: game_id.clone(),
            display_name: adapter.display_name().to_owned(),
            steam_app_id: adapter.steam_app_id(),
        };

        let raw_candidates =
            self.discovery
                .scan_candidates(&request)
                .map_err(|error| match error {
                    GameDiscoveryError::ScanNotImplemented => {
                        GameSetupServiceError::ScanNotImplemented
                    }
                    GameDiscoveryError::ScanFailed(message) => {
                        GameSetupServiceError::ScanFailed(message)
                    }
                })?;
        let mut candidates = raw_candidates
            .into_iter()
            .map(|candidate| {
                let validation =
                    self.validate_with_adapter(adapter.as_ref(), candidate.root_dir.clone());

                GameSetupCandidate {
                    candidate,
                    validation,
                }
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|left, right| {
            right
                .validation
                .is_valid
                .cmp(&left.validation.is_valid)
                .then_with(|| right.validation.confidence.cmp(&left.validation.confidence))
                .then_with(|| {
                    normalize_candidate_path(&left.candidate.root_dir)
                        .cmp(&normalize_candidate_path(&right.candidate.root_dir))
                })
        });

        Ok(GameCandidateScan {
            game_id,
            candidates,
        })
    }

    pub fn auto_detect_game_directory(
        &self,
        game_id: GameId,
    ) -> Result<GameAutoDetection, GameSetupServiceError> {
        let current_status = self.get_status(game_id.clone())?;

        if current_status.status == GameDirectoryStatus::Configured {
            return Ok(GameAutoDetection {
                game_id,
                outcome: GameAutoDetectionOutcome::AlreadyConfigured,
                status: current_status,
                error_code: None,
                candidate_count: 0,
            });
        }

        let scan = match self.scan_candidates(game_id.clone()) {
            Ok(scan) => scan,
            Err(GameSetupServiceError::ScanFailed(_)) => {
                return Ok(GameAutoDetection {
                    game_id,
                    outcome: GameAutoDetectionOutcome::ScanFailed,
                    status: current_status,
                    error_code: Some(GameSetupErrorCode::ScanFailed),
                    candidate_count: 0,
                });
            }
            Err(GameSetupServiceError::ScanNotImplemented) => {
                return Ok(GameAutoDetection {
                    game_id,
                    outcome: GameAutoDetectionOutcome::ScanFailed,
                    status: current_status,
                    error_code: Some(GameSetupErrorCode::ScanNotImplemented),
                    candidate_count: 0,
                });
            }
            Err(error) => return Err(error),
        };

        if let Some(valid_candidate) = scan
            .candidates
            .iter()
            .find(|candidate| candidate.validation.is_valid)
        {
            let status = self
                .save_game_directory(game_id.clone(), valid_candidate.candidate.root_dir.clone())?;

            return Ok(GameAutoDetection {
                game_id,
                outcome: GameAutoDetectionOutcome::DetectedAndSaved,
                status,
                error_code: None,
                candidate_count: scan.candidates.len(),
            });
        }

        let error_code = scan
            .candidates
            .iter()
            .find_map(|candidate| candidate.validation.errors.first().cloned())
            .unwrap_or(GameSetupErrorCode::DirectoryNotFound);
        let outcome = if scan.candidates.is_empty() {
            GameAutoDetectionOutcome::NotFound
        } else {
            GameAutoDetectionOutcome::InvalidCandidate
        };

        Ok(GameAutoDetection {
            game_id,
            outcome,
            status: current_status,
            error_code: Some(error_code),
            candidate_count: scan.candidates.len(),
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

    fn status_for_loaded_instance(
        &self,
        instance: GameInstance,
    ) -> Result<GameSetupStatus, GameSetupServiceError> {
        let adapter = self.require_adapter(&instance.game_id)?;
        let validation = self.validate_with_adapter(adapter.as_ref(), instance.root_dir.clone());

        if validation.is_valid {
            return Ok(GameSetupStatus::configured(instance));
        }

        let error_code = validation
            .errors
            .first()
            .cloned()
            .unwrap_or(GameSetupErrorCode::Unknown);

        Ok(GameSetupStatus::invalid(
            validation.game_id,
            error_code,
            "saved game directory is no longer valid",
        ))
    }

    fn validate_with_adapter(
        &self,
        adapter: &dyn GameAdapter,
        directory: PathBuf,
    ) -> GameDirectoryValidation {
        let probe = self.probe_factory.create(directory);
        adapter.validate_directory(probe.as_ref())
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

fn normalize_candidate_path(path: &std::path::Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if cfg!(any(target_os = "windows", target_os = "macos")) {
        normalized.to_lowercase()
    } else {
        normalized
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
            Self::ScanFailed(_) => GameSetupErrorCode::ScanFailed,
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
        GameCandidate, GameCandidateSource, GameConfigRepositoryResult, GameDirectoryProbe,
        GameDiscoveryRequest, GameDiscoveryService, GamePrerequisiteItem,
        GamePrerequisiteItemStatus, GamePrerequisiteReport, GamePrerequisiteSummaryStatus,
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

        fn read_text_file(&self, _relative_path: &str) -> anyhow::Result<String> {
            unreachable!("game setup tests do not read prerequisite text files")
        }

        fn sha256_hex(&self, _relative_path: &str) -> anyhow::Result<String> {
            unreachable!("game setup tests do not hash prerequisite files")
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
        invalid_roots: Vec<PathBuf>,
        prerequisite_report: Option<GamePrerequisiteReport>,
    }

    impl FakeAdapter {
        fn valid() -> Self {
            Self {
                valid: true,
                invalid_roots: Vec::new(),
                prerequisite_report: None,
            }
        }

        fn invalid() -> Self {
            Self {
                valid: false,
                invalid_roots: Vec::new(),
                prerequisite_report: None,
            }
        }

        fn with_invalid_roots(roots: Vec<&str>) -> Self {
            Self {
                valid: true,
                invalid_roots: roots.into_iter().map(PathBuf::from).collect(),
                prerequisite_report: None,
            }
        }

        fn with_prerequisite_report(report: GamePrerequisiteReport) -> Self {
            Self {
                valid: true,
                invalid_roots: Vec::new(),
                prerequisite_report: Some(report),
            }
        }
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
            let is_invalid = !self.valid
                || self
                    .invalid_roots
                    .iter()
                    .any(|root| root == probe.root_dir());
            validation.confidence = if is_invalid { 20 } else { 90 };
            if is_invalid {
                validation.add_error(GameSetupErrorCode::MissingExecutable);
            }
            validation
        }

        fn inspect_prerequisites(&self, _probe: &dyn GameDirectoryProbe) -> GamePrerequisiteReport {
            self.prerequisite_report
                .clone()
                .expect("game setup prerequisite tests must configure a fake report")
        }
    }

    struct NoopDiscovery;

    impl GameDiscoveryService for NoopDiscovery {
        fn scan_candidates(
            &self,
            _request: &GameDiscoveryRequest,
        ) -> Result<Vec<GameCandidate>, GameDiscoveryError> {
            Err(GameDiscoveryError::ScanNotImplemented)
        }
    }

    struct FakeDiscovery {
        result: Result<Vec<GameCandidate>, GameDiscoveryError>,
    }

    impl FakeDiscovery {
        fn ok(candidates: Vec<GameCandidate>) -> Self {
            Self {
                result: Ok(candidates),
            }
        }

        fn error(error: GameDiscoveryError) -> Self {
            Self { result: Err(error) }
        }
    }

    impl GameDiscoveryService for FakeDiscovery {
        fn scan_candidates(
            &self,
            _request: &GameDiscoveryRequest,
        ) -> Result<Vec<GameCandidate>, GameDiscoveryError> {
            self.result.clone()
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

    fn service_with_discovery(
        adapter: FakeAdapter,
        discovery: Arc<dyn GameDiscoveryService>,
    ) -> GameSetupService {
        GameSetupService::new(
            vec![Arc::new(adapter)],
            Arc::new(FakeRepository::empty()),
            Arc::new(FakeProbeFactory),
            discovery,
            Arc::new(FakeClock),
        )
    }

    fn service_with_repository(
        adapter: FakeAdapter,
        repository: Arc<dyn GameConfigRepository>,
    ) -> GameSetupService {
        GameSetupService::new(
            vec![Arc::new(adapter)],
            repository,
            Arc::new(FakeProbeFactory),
            Arc::new(NoopDiscovery),
            Arc::new(FakeClock),
        )
    }

    fn service_with_repository_and_discovery(
        adapter: FakeAdapter,
        repository: Arc<dyn GameConfigRepository>,
        discovery: Arc<dyn GameDiscoveryService>,
    ) -> GameSetupService {
        GameSetupService::new(
            vec![Arc::new(adapter)],
            repository,
            Arc::new(FakeProbeFactory),
            discovery,
            Arc::new(FakeClock),
        )
    }

    fn stored_instance(root: &str) -> GameInstance {
        GameInstance {
            id: "mhw-default".to_owned(),
            game_id: GameId::mhw(),
            display_name: "Monster Hunter: World - Iceborne".to_owned(),
            root_dir: PathBuf::from(root),
            status: GameDirectoryStatus::Configured,
            configured_at_unix_millis: 42,
        }
    }

    #[test]
    fn status_is_not_configured_without_saved_instance() {
        let service = service_with(FakeAdapter::valid());

        let status = service
            .get_status(GameId::mhw())
            .expect("status should load");

        assert_eq!(status.status, GameDirectoryStatus::NotConfigured);
    }

    #[test]
    fn prerequisite_report_returns_not_configured_when_game_is_not_saved() {
        let service = service_with(FakeAdapter::valid());

        let report = service
            .get_prerequisite_status(GameId::mhw())
            .expect("report should load");

        assert_eq!(
            report.state,
            hmm_ports::GamePrerequisiteReportState::NotConfigured
        );
    }

    #[test]
    fn save_directory_validates_before_persisting() {
        let service = service_with(FakeAdapter::valid());

        let status = service
            .save_game_directory(GameId::mhw(), PathBuf::from("C:/MHW"))
            .expect("valid directory should save");

        assert_eq!(status.status, GameDirectoryStatus::Configured);
        assert_eq!(
            status.instance.expect("instance").configured_at_unix_millis,
            42
        );
    }

    #[test]
    fn save_directory_rejects_invalid_validation() {
        let service = service_with(FakeAdapter::invalid());

        let error = service
            .save_game_directory(GameId::mhw(), PathBuf::from("C:/Wrong"))
            .expect_err("invalid directory should fail");

        assert_eq!(error.error_code(), GameSetupErrorCode::MissingExecutable);
    }

    #[test]
    fn status_revalidates_saved_instance_before_reporting_configured() {
        let repository = Arc::new(FakeRepository {
            stored: Mutex::new(Some(stored_instance("C:/Moved"))),
        });
        let service = service_with_repository(FakeAdapter::invalid(), repository);

        let status = service
            .get_status(GameId::mhw())
            .expect("status should load");

        assert_eq!(status.status, GameDirectoryStatus::Invalid);
        assert_eq!(
            status.error_code,
            Some(GameSetupErrorCode::MissingExecutable)
        );
        assert!(status.instance.is_none());
    }

    #[test]
    fn prerequisite_report_returns_game_directory_invalid_when_saved_directory_is_invalid() {
        let repository = Arc::new(FakeRepository {
            stored: Mutex::new(Some(stored_instance("C:/Moved"))),
        });
        let service = service_with_repository(FakeAdapter::invalid(), repository);

        let report = service
            .get_prerequisite_status(GameId::mhw())
            .expect("report should load");

        assert_eq!(
            report.state,
            hmm_ports::GamePrerequisiteReportState::GameDirectoryInvalid
        );
        assert_eq!(
            report.error_code,
            Some(GameSetupErrorCode::MissingExecutable)
        );
    }

    #[test]
    fn prerequisite_report_uses_adapter_when_saved_directory_is_valid() {
        let repository = Arc::new(FakeRepository {
            stored: Mutex::new(Some(stored_instance("C:/MHW"))),
        });
        let service = service_with_repository(
            FakeAdapter::with_prerequisite_report(GamePrerequisiteReport::ready(
                GameId::mhw(),
                GamePrerequisiteSummaryStatus::Warning,
                vec![GamePrerequisiteItem::new(
                    "crc_bypass",
                    "CRCBypass",
                    GamePrerequisiteItemStatus::InstalledUnverified,
                )],
            )),
            repository,
        );

        let report = service
            .get_prerequisite_status(GameId::mhw())
            .expect("report should load");

        assert_eq!(
            report.summary_status,
            Some(GamePrerequisiteSummaryStatus::Warning)
        );
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].id, "crc_bypass");
    }

    #[test]
    fn scan_candidates_validates_discovered_directories() {
        let service = service_with_discovery(
            FakeAdapter::valid(),
            Arc::new(FakeDiscovery::ok(vec![steam_candidate("C:/MHW")])),
        );

        let scan = service
            .scan_candidates(GameId::mhw())
            .expect("scan should return candidates");

        assert_eq!(scan.candidates.len(), 1);
        assert_eq!(
            scan.candidates[0].candidate.root_dir,
            PathBuf::from("C:/MHW")
        );
        assert!(scan.candidates[0].validation.is_valid);
        assert_eq!(scan.candidates[0].validation.confidence, 90);
    }

    #[test]
    fn scan_candidates_sorts_valid_candidates_first() {
        let service = service_with_discovery(
            FakeAdapter::with_invalid_roots(vec!["C:/Broken"]),
            Arc::new(FakeDiscovery::ok(vec![
                steam_candidate("C:/Broken"),
                steam_candidate("C:/MHW"),
            ])),
        );

        let scan = service
            .scan_candidates(GameId::mhw())
            .expect("scan should return candidates");

        assert_eq!(
            scan.candidates[0].candidate.root_dir,
            PathBuf::from("C:/MHW")
        );
        assert!(scan.candidates[0].validation.is_valid);
        assert!(!scan.candidates[1].validation.is_valid);
    }

    #[test]
    fn scan_candidates_maps_discovery_failure() {
        let service = service_with_discovery(
            FakeAdapter::valid(),
            Arc::new(FakeDiscovery::error(GameDiscoveryError::ScanFailed(
                "boom".to_owned(),
            ))),
        );

        let error = service
            .scan_candidates(GameId::mhw())
            .expect_err("scan failure should map to service error");

        assert_eq!(error.error_code(), GameSetupErrorCode::ScanFailed);
    }

    #[test]
    fn auto_detect_saves_first_valid_discovered_directory() {
        let repository = Arc::new(FakeRepository::empty());
        let service = service_with_repository_and_discovery(
            FakeAdapter::with_invalid_roots(vec!["C:/Broken"]),
            repository.clone(),
            Arc::new(FakeDiscovery::ok(vec![
                steam_candidate("C:/Broken"),
                steam_candidate("C:/MHW"),
            ])),
        );

        let detection = service
            .auto_detect_game_directory(GameId::mhw())
            .expect("valid candidate should auto-save");

        assert_eq!(
            detection.outcome,
            GameAutoDetectionOutcome::DetectedAndSaved
        );
        assert_eq!(detection.candidate_count, 2);
        assert_eq!(detection.status.status, GameDirectoryStatus::Configured);
        assert_eq!(
            repository
                .load_game_instance(&GameId::mhw())
                .expect("repo should load")
                .expect("saved instance")
                .root_dir,
            PathBuf::from("C:/MHW")
        );
    }

    #[test]
    fn auto_detect_reports_not_found_without_persisting_when_scan_has_no_candidates() {
        let repository = Arc::new(FakeRepository::empty());
        let service = service_with_repository_and_discovery(
            FakeAdapter::valid(),
            repository.clone(),
            Arc::new(FakeDiscovery::ok(Vec::new())),
        );

        let detection = service
            .auto_detect_game_directory(GameId::mhw())
            .expect("empty scan should be reported as recoverable detection result");

        assert_eq!(detection.outcome, GameAutoDetectionOutcome::NotFound);
        assert_eq!(detection.candidate_count, 0);
        assert_eq!(
            detection.error_code,
            Some(GameSetupErrorCode::DirectoryNotFound)
        );
        assert_eq!(detection.status.status, GameDirectoryStatus::NotConfigured);
        assert!(repository
            .load_game_instance(&GameId::mhw())
            .expect("repo should load")
            .is_none());
    }

    #[test]
    fn auto_detect_reports_already_configured_without_scanning() {
        let repository = Arc::new(FakeRepository {
            stored: Mutex::new(Some(stored_instance("C:/MHW"))),
        });
        let service = service_with_repository_and_discovery(
            FakeAdapter::valid(),
            repository,
            Arc::new(FakeDiscovery::error(GameDiscoveryError::ScanFailed(
                "should not scan".to_owned(),
            ))),
        );

        let detection = service
            .auto_detect_game_directory(GameId::mhw())
            .expect("configured directory should short-circuit detection");

        assert_eq!(
            detection.outcome,
            GameAutoDetectionOutcome::AlreadyConfigured
        );
        assert_eq!(detection.candidate_count, 0);
        assert_eq!(detection.error_code, None);
        assert_eq!(detection.status.status, GameDirectoryStatus::Configured);
    }

    #[test]
    fn auto_detect_reports_invalid_candidate_without_persisting() {
        let repository = Arc::new(FakeRepository::empty());
        let service = service_with_repository_and_discovery(
            FakeAdapter::with_invalid_roots(vec!["C:/Broken"]),
            repository.clone(),
            Arc::new(FakeDiscovery::ok(vec![steam_candidate("C:/Broken")])),
        );

        let detection = service
            .auto_detect_game_directory(GameId::mhw())
            .expect("invalid candidates should be reported as recoverable detection result");

        assert_eq!(
            detection.outcome,
            GameAutoDetectionOutcome::InvalidCandidate
        );
        assert_eq!(detection.candidate_count, 1);
        assert_eq!(
            detection.error_code,
            Some(GameSetupErrorCode::MissingExecutable)
        );
        assert_eq!(detection.status.status, GameDirectoryStatus::NotConfigured);
        assert!(repository
            .load_game_instance(&GameId::mhw())
            .expect("repo should load")
            .is_none());
    }

    #[test]
    fn auto_detect_reports_scan_failed_without_persisting() {
        let repository = Arc::new(FakeRepository::empty());
        let service = service_with_repository_and_discovery(
            FakeAdapter::valid(),
            repository.clone(),
            Arc::new(FakeDiscovery::error(GameDiscoveryError::ScanFailed(
                "boom".to_owned(),
            ))),
        );

        let detection = service
            .auto_detect_game_directory(GameId::mhw())
            .expect("scan failure should be reported as recoverable detection result");

        assert_eq!(detection.outcome, GameAutoDetectionOutcome::ScanFailed);
        assert_eq!(detection.candidate_count, 0);
        assert_eq!(detection.error_code, Some(GameSetupErrorCode::ScanFailed));
        assert_eq!(detection.status.status, GameDirectoryStatus::NotConfigured);
        assert!(repository
            .load_game_instance(&GameId::mhw())
            .expect("repo should load")
            .is_none());
    }

    #[test]
    fn auto_detect_reports_scan_not_implemented_without_persisting() {
        let repository = Arc::new(FakeRepository::empty());
        let service = service_with_repository_and_discovery(
            FakeAdapter::valid(),
            repository.clone(),
            Arc::new(FakeDiscovery::error(GameDiscoveryError::ScanNotImplemented)),
        );

        let detection = service
            .auto_detect_game_directory(GameId::mhw())
            .expect("unimplemented scan should be reported as recoverable detection result");

        assert_eq!(detection.outcome, GameAutoDetectionOutcome::ScanFailed);
        assert_eq!(detection.candidate_count, 0);
        assert_eq!(
            detection.error_code,
            Some(GameSetupErrorCode::ScanNotImplemented)
        );
        assert_eq!(detection.status.status, GameDirectoryStatus::NotConfigured);
        assert!(repository
            .load_game_instance(&GameId::mhw())
            .expect("repo should load")
            .is_none());
    }

    fn steam_candidate(root: &str) -> GameCandidate {
        GameCandidate {
            game_id: GameId::mhw(),
            display_name: "Monster Hunter: World - Iceborne".to_owned(),
            root_dir: PathBuf::from(root),
            source: GameCandidateSource::Steam,
            source_label: "Steam".to_owned(),
        }
    }
}
