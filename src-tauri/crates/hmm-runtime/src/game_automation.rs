use crate::{production_app_data_dir, RuntimeEnvironment};
use hmm_app::{GameSetupService, GameSetupServiceError};
use hmm_core::{
    GameDirectoryEvidenceKind, GameDirectoryStatus, GameId, GameInstance, GameSetupErrorCode,
};
use hmm_games_mhw::MonsterHunterWorldAdapter;
use hmm_infra::steam_discovery::SteamRootProvider;
use hmm_infra::{
    JsonGameConfigRepository, PlatformSteamRootProvider,
    ReadOnlyJsonGamePrerequisiteRuleRepository, RealGameDirectoryProbeFactory,
    SteamGameDiscoveryService, SystemClock,
};
use hmm_ports::{
    GameAdapter, GameConfigRepository, GameConfigRepositoryError, GameDiscoveryService,
    GamePrerequisiteIssueCode, GamePrerequisiteItemStatus, GamePrerequisiteReport,
    GamePrerequisiteReportState, GamePrerequisiteRuleRepository, GamePrerequisiteSummaryStatus,
};
use serde::Serialize;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStatusSnapshot {
    pub game_id: GameId,
    pub status: GameDirectoryStatus,
    pub error_code: Option<GameSetupErrorCode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameScanSnapshot {
    pub game_id: GameId,
    pub candidate_count: usize,
    pub valid_candidate_count: usize,
    pub invalid_candidate_count: usize,
    pub max_confidence: Option<u8>,
    pub issue_codes: Vec<GameSetupErrorCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GameValidationState {
    NotConfigured,
    Validated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameValidationSnapshot {
    pub game_id: GameId,
    pub state: GameValidationState,
    pub valid: Option<bool>,
    pub confidence: Option<u8>,
    pub evidence: Vec<GameDirectoryEvidenceKind>,
    pub issue_codes: Vec<GameSetupErrorCode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePrerequisiteItemSnapshot {
    pub code: String,
    pub status: GamePrerequisiteItemStatus,
    pub issue_codes: Vec<GamePrerequisiteIssueCode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePrerequisiteSnapshot {
    pub game_id: GameId,
    pub state: GamePrerequisiteReportState,
    pub status: Option<GamePrerequisiteSummaryStatus>,
    pub item_count: usize,
    pub items: Vec<GamePrerequisiteItemSnapshot>,
    pub issue_codes: Vec<GamePrerequisiteIssueCode>,
    pub error_code: Option<GameSetupErrorCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnlyGameAutomationError {
    AppDataUnavailable,
    UnsupportedGame,
    ConfiguredGamePathRejected,
    SandboxGamePathRejected,
    StorageCorrupted,
    StorageUnavailable,
    ScanUnavailable,
    InternalUnavailable,
}

impl ReadOnlyGameAutomationError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::AppDataUnavailable => "app_data_unavailable",
            Self::UnsupportedGame => "unsupported_game",
            Self::ConfiguredGamePathRejected => "configured_game_path_rejected",
            Self::SandboxGamePathRejected => "sandbox_game_path_rejected",
            Self::StorageCorrupted => "game_config_corrupted",
            Self::StorageUnavailable => "game_config_unavailable",
            Self::ScanUnavailable => "game_scan_unavailable",
            Self::InternalUnavailable => "game_automation_unavailable",
        }
    }
}

pub struct ReadOnlyGameAutomation {
    service: GameSetupService,
    repository: Arc<dyn GameConfigRepository>,
    sandbox_fixture_root: Option<PathBuf>,
}

impl ReadOnlyGameAutomation {
    pub fn from_environment(
        environment: &RuntimeEnvironment,
    ) -> Result<Self, ReadOnlyGameAutomationError> {
        let (data_dir, discovery, sandbox_fixture_root): (
            PathBuf,
            Arc<dyn GameDiscoveryService>,
            Option<PathBuf>,
        ) = match environment {
            RuntimeEnvironment::Production => (
                production_app_data_dir().ok_or(ReadOnlyGameAutomationError::AppDataUnavailable)?,
                Arc::new(SteamGameDiscoveryService::new(Arc::new(
                    PlatformSteamRootProvider,
                ))),
                None,
            ),
            RuntimeEnvironment::Sandbox { data_dir } => {
                let fixture_root = data_dir.join("fixtures");
                let steam_root = fixture_root.join("steam");
                (
                    data_dir.clone(),
                    Arc::new(SteamGameDiscoveryService::new_contained(
                        Arc::new(FixedSteamRootProvider::new(steam_root)),
                        fixture_root.clone(),
                    )),
                    Some(fixture_root),
                )
            }
        };

        let repository: Arc<dyn GameConfigRepository> = Arc::new(JsonGameConfigRepository::new(
            data_dir.join("config").join("games.json"),
        ));
        let prerequisite_rules: Arc<dyn GamePrerequisiteRuleRepository> =
            Arc::new(ReadOnlyJsonGamePrerequisiteRuleRepository::new(
                data_dir
                    .join("config")
                    .join("prerequisite-rules")
                    .join("mhw.json"),
            ));
        let adapter: Arc<dyn GameAdapter> =
            Arc::new(MonsterHunterWorldAdapter::new(prerequisite_rules));
        let service = GameSetupService::new(
            vec![adapter],
            Arc::clone(&repository),
            Arc::new(RealGameDirectoryProbeFactory),
            discovery,
            Arc::new(SystemClock),
        );

        Ok(Self {
            service,
            repository,
            sandbox_fixture_root,
        })
    }

    pub fn status(&self, game_id: &str) -> Result<GameStatusSnapshot, ReadOnlyGameAutomationError> {
        let game_id = parse_game_id(game_id)?;
        let Some(instance) = self.load_configured_instance(&game_id)? else {
            return Ok(GameStatusSnapshot {
                game_id,
                status: GameDirectoryStatus::NotConfigured,
                error_code: None,
            });
        };
        let validation = self
            .service
            .validate_directory(game_id.clone(), instance.root_dir)
            .map_err(map_service_error)?;

        Ok(GameStatusSnapshot {
            game_id,
            status: if validation.is_valid {
                GameDirectoryStatus::Configured
            } else {
                GameDirectoryStatus::Invalid
            },
            error_code: validation.errors.first().cloned(),
        })
    }

    pub fn scan(&self, game_id: &str) -> Result<GameScanSnapshot, ReadOnlyGameAutomationError> {
        let game_id = parse_game_id(game_id)?;
        let scan = self
            .service
            .scan_candidates(game_id)
            .map_err(map_service_error)?;
        let valid_candidate_count = scan
            .candidates
            .iter()
            .filter(|candidate| candidate.validation.is_valid)
            .count();
        let mut issue_codes = Vec::new();

        for candidate in &scan.candidates {
            for code in &candidate.validation.errors {
                push_unique(&mut issue_codes, code.clone());
            }
        }

        Ok(GameScanSnapshot {
            game_id: scan.game_id,
            candidate_count: scan.candidates.len(),
            valid_candidate_count,
            invalid_candidate_count: scan.candidates.len() - valid_candidate_count,
            max_confidence: scan
                .candidates
                .iter()
                .map(|candidate| candidate.validation.confidence)
                .max(),
            issue_codes,
        })
    }

    pub fn validate(
        &self,
        game_id: &str,
    ) -> Result<GameValidationSnapshot, ReadOnlyGameAutomationError> {
        let game_id = parse_game_id(game_id)?;
        let Some(instance) = self.load_configured_instance(&game_id)? else {
            return Ok(GameValidationSnapshot {
                game_id,
                state: GameValidationState::NotConfigured,
                valid: None,
                confidence: None,
                evidence: Vec::new(),
                issue_codes: Vec::new(),
            });
        };
        let validation = self
            .service
            .validate_directory(game_id.clone(), instance.root_dir)
            .map_err(map_service_error)?;

        Ok(GameValidationSnapshot {
            game_id,
            state: GameValidationState::Validated,
            valid: Some(validation.is_valid),
            confidence: Some(validation.confidence),
            evidence: validation
                .evidence
                .into_iter()
                .map(|evidence| evidence.kind)
                .collect(),
            issue_codes: validation.errors,
        })
    }

    pub fn prerequisites(
        &self,
        game_id: &str,
    ) -> Result<GamePrerequisiteSnapshot, ReadOnlyGameAutomationError> {
        let game_id = parse_game_id(game_id)?;
        let report = match self.load_configured_instance(&game_id)? {
            Some(instance) => self
                .service
                .get_prerequisite_status_for_directory(game_id, instance.root_dir)
                .map_err(map_service_error)?,
            None => GamePrerequisiteReport::not_configured(game_id),
        };

        Ok(project_prerequisite_report(report))
    }

    fn load_configured_instance(
        &self,
        game_id: &GameId,
    ) -> Result<Option<GameInstance>, ReadOnlyGameAutomationError> {
        let instance = self
            .repository
            .load_game_instance(game_id)
            .map_err(map_repository_error)?;

        if let Some(instance) = &instance {
            self.admit_configured_path(&instance.root_dir)?;
        }

        Ok(instance)
    }

    fn admit_configured_path(&self, path: &Path) -> Result<(), ReadOnlyGameAutomationError> {
        if !is_safe_absolute_path(path) {
            return Err(ReadOnlyGameAutomationError::ConfiguredGamePathRejected);
        }

        let Some(fixture_root) = &self.sandbox_fixture_root else {
            return Ok(());
        };

        if !is_canonically_within(path, fixture_root) {
            return Err(ReadOnlyGameAutomationError::SandboxGamePathRejected);
        }

        Ok(())
    }
}

fn parse_game_id(value: &str) -> Result<GameId, ReadOnlyGameAutomationError> {
    GameId::parse(value).map_err(|_| ReadOnlyGameAutomationError::UnsupportedGame)
}

fn map_repository_error(error: GameConfigRepositoryError) -> ReadOnlyGameAutomationError {
    match error {
        GameConfigRepositoryError::StorageCorrupted => {
            ReadOnlyGameAutomationError::StorageCorrupted
        }
        GameConfigRepositoryError::StorageFailed(_) => {
            ReadOnlyGameAutomationError::StorageUnavailable
        }
    }
}

fn map_service_error(error: GameSetupServiceError) -> ReadOnlyGameAutomationError {
    match error {
        GameSetupServiceError::UnsupportedGame => ReadOnlyGameAutomationError::UnsupportedGame,
        GameSetupServiceError::StorageCorrupted => ReadOnlyGameAutomationError::StorageCorrupted,
        GameSetupServiceError::StorageFailed(_) => ReadOnlyGameAutomationError::StorageUnavailable,
        GameSetupServiceError::ScanFailed(_) | GameSetupServiceError::ScanNotImplemented => {
            ReadOnlyGameAutomationError::ScanUnavailable
        }
        GameSetupServiceError::ValidationFailed(_) | GameSetupServiceError::ClockFailed(_) => {
            ReadOnlyGameAutomationError::InternalUnavailable
        }
    }
}

fn project_prerequisite_report(report: GamePrerequisiteReport) -> GamePrerequisiteSnapshot {
    let mut issue_codes = Vec::new();
    let items = report
        .items
        .into_iter()
        .map(|item| {
            let item_issue_codes = item
                .issues
                .into_iter()
                .map(|issue| issue.code)
                .collect::<Vec<_>>();
            for code in &item_issue_codes {
                push_unique(&mut issue_codes, code.clone());
            }

            GamePrerequisiteItemSnapshot {
                code: item.id,
                status: item.status,
                issue_codes: item_issue_codes,
            }
        })
        .collect::<Vec<_>>();

    GamePrerequisiteSnapshot {
        game_id: report.game_id,
        state: report.state,
        status: report.summary_status,
        item_count: items.len(),
        items,
        issue_codes,
        error_code: report.error_code,
    }
}

fn push_unique<T: PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}

pub(crate) fn is_safe_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path.file_name().is_some()
        && !path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        && !path
            .as_os_str()
            .to_string_lossy()
            .split(['/', '\\'])
            .any(|component| matches!(component, "." | ".."))
}

pub(crate) fn is_canonically_within(path: &Path, parent: &Path) -> bool {
    if !is_lexically_within_or_equal(path, parent) {
        return false;
    }

    let Ok(canonical_parent) = parent.canonicalize() else {
        return false;
    };
    let Some(existing_ancestor) = nearest_existing_ancestor(path) else {
        return false;
    };
    let Ok(canonical_ancestor) = existing_ancestor.canonicalize() else {
        return false;
    };

    is_lexically_within_or_equal(&canonical_ancestor, &canonical_parent)
}

fn nearest_existing_ancestor(path: &Path) -> Option<&Path> {
    path.ancestors().find(|ancestor| ancestor.exists())
}

fn is_lexically_within_or_equal(path: &Path, parent: &Path) -> bool {
    let path = normalized_path_key(path);
    let parent = normalized_path_key(parent);
    path == parent
        || path
            .strip_prefix(&parent)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn normalized_path_key(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if cfg!(any(target_os = "windows", target_os = "macos")) {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

struct FixedSteamRootProvider {
    root: PathBuf,
}

impl FixedSteamRootProvider {
    fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl SteamRootProvider for FixedSteamRootProvider {
    fn steam_roots(&self) -> Vec<PathBuf> {
        vec![self.root.clone()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;

    fn create_sandbox() -> tempfile::TempDir {
        tempfile::tempdir().expect("sandbox")
    }

    fn write_game_config(sandbox: &Path, game_root: &Path) {
        let config_dir = sandbox.join("config");
        fs::create_dir_all(&config_dir).expect("config dir");
        fs::write(
            config_dir.join("games.json"),
            serde_json::json!({
                "version": 1,
                "games": [{
                    "id": "mhw-default",
                    "game_id": "mhw",
                    "display_name": "Monster Hunter: World - Iceborne",
                    "root_dir": game_root,
                    "status": "configured",
                    "configured_at_unix_millis": 42
                }]
            })
            .to_string(),
        )
        .expect("game config");
    }

    fn create_valid_game_fixture(sandbox: &Path) -> PathBuf {
        let game_root = sandbox.join("fixtures").join("games").join("mhw-minimal");
        fs::create_dir_all(&game_root).expect("game fixture");
        fs::write(game_root.join("MonsterHunterWorld.exe"), b"fixture").expect("game exe");
        game_root
    }

    #[test]
    fn sandbox_status_and_validation_return_path_free_snapshots() {
        let sandbox = create_sandbox();
        let game_root = create_valid_game_fixture(sandbox.path());
        write_game_config(sandbox.path(), &game_root);
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let automation =
            ReadOnlyGameAutomation::from_environment(&environment).expect("automation");

        let status = automation.status("mhw").expect("status");
        let validation = automation.validate("mhw").expect("validation");
        let serialized =
            serde_json::to_string(&(status.clone(), validation.clone())).expect("serialize");

        assert_eq!(status.status, GameDirectoryStatus::Configured);
        assert_eq!(validation.valid, Some(true));
        assert!(!serialized.contains(&game_root.to_string_lossy().to_string()));
        assert!(!serialized.contains("root_dir"));
    }

    #[test]
    fn sandbox_rejects_configured_game_outside_fixture_root() {
        let sandbox = create_sandbox();
        let outside = tempfile::tempdir().expect("outside");
        fs::write(outside.path().join("MonsterHunterWorld.exe"), b"fixture").expect("outside exe");
        write_game_config(sandbox.path(), outside.path());
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let automation =
            ReadOnlyGameAutomation::from_environment(&environment).expect("automation");

        let error = automation
            .validate("mhw")
            .expect_err("outside path must fail closed");

        assert_eq!(error, ReadOnlyGameAutomationError::SandboxGamePathRejected);
    }

    #[test]
    fn prerequisites_use_bundled_rules_without_creating_override() {
        let sandbox = create_sandbox();
        let game_root = create_valid_game_fixture(sandbox.path());
        write_game_config(sandbox.path(), &game_root);
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let automation =
            ReadOnlyGameAutomation::from_environment(&environment).expect("automation");
        let override_path = sandbox
            .path()
            .join("config")
            .join("prerequisite-rules")
            .join("mhw.json");

        let snapshot = automation.prerequisites("mhw").expect("prerequisites");

        assert_eq!(snapshot.state, GamePrerequisiteReportState::Ready);
        assert!(!override_path.exists());
        assert!(!override_path.parent().expect("override parent").exists());
    }

    #[test]
    fn scan_returns_only_aggregates_without_paths() {
        let sandbox = create_sandbox();
        let steam_root = sandbox.path().join("fixtures").join("steam");
        let game_root = steam_root
            .join("steamapps")
            .join("common")
            .join("Monster Hunter World");
        fs::create_dir_all(&game_root).expect("game fixture");
        fs::write(game_root.join("MonsterHunterWorld.exe"), b"fixture").expect("game exe");
        fs::write(
            steam_root.join("steamapps").join("libraryfolders.vdf"),
            format!(
                r#""libraryfolders" {{ "0" {{ "path" "{}" "apps" {{ "582010" "1" }} }} }}"#,
                steam_root.display()
            ),
        )
        .expect("library folders");
        fs::write(
            steam_root.join("steamapps").join("appmanifest_582010.acf"),
            r#""AppState" { "appid" "582010" "installdir" "Monster Hunter World" }"#,
        )
        .expect("app manifest");
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let automation =
            ReadOnlyGameAutomation::from_environment(&environment).expect("automation");

        let snapshot = automation.scan("mhw").expect("scan");
        let serialized = serde_json::to_string(&snapshot).expect("serialize");

        assert_eq!(snapshot.candidate_count, 1);
        assert_eq!(snapshot.valid_candidate_count, 1);
        assert!(!serialized.contains(&sandbox.path().to_string_lossy().to_string()));
        assert!(!serialized.contains("root"));
    }

    #[test]
    fn snapshots_do_not_include_free_text_or_path_fields() {
        let snapshot = GamePrerequisiteSnapshot {
            game_id: GameId::mhw(),
            state: GamePrerequisiteReportState::Ready,
            status: Some(GamePrerequisiteSummaryStatus::Warning),
            item_count: 1,
            items: vec![GamePrerequisiteItemSnapshot {
                code: "loader".to_owned(),
                status: GamePrerequisiteItemStatus::InstalledUnverified,
                issue_codes: vec![GamePrerequisiteIssueCode::SignatureUnverified],
            }],
            issue_codes: vec![GamePrerequisiteIssueCode::SignatureUnverified],
            error_code: None,
        };

        let value: Value = serde_json::to_value(snapshot).expect("snapshot");
        let text = value.to_string();

        assert!(value.get("message").is_none());
        assert!(value["items"][0].get("path").is_none());
        assert!(!text.contains("displayName"));
    }
}
