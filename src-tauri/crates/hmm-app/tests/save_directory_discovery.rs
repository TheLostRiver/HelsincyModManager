use anyhow::{bail, Result};
use hmm_app::{
    ConfirmProfileSaveDirectoryCandidateRequest, DiscoverProfileSaveDirectoriesRequest,
    ProfileSaveDirectoryDiscoveryService, SaveDirectoryDiscoveryError,
};
use hmm_core::{
    GameDirectoryStatus, GameId, GameInstance, Profile, ProfileBackupRetention,
    ProfileBackupSchedule, ProfileDirectoryMode, ProfileDirectorySelection, ProfileDirectoryStatus,
    ProfileId, ProfileSaveSettings, SaveDirectoryCandidateConfidence, SaveDirectoryCandidateSource,
    SaveDirectoryDiscoveryOutcome, SteamAccountProfileSummary,
};
use hmm_ports::{
    AppClock, GameConfigRepository, GameConfigRepositoryResult, GameSaveDirectoryRule,
    PendingSaveDirectoryCandidate, PendingSaveDirectoryCandidateStore,
    PendingSaveDirectoryDiscovery, ProfileRepository, ProfileSaveDirectoryValidator,
    ProfileSaveSettingsRepository, ScannedSaveDirectoryCandidate, SteamAccountProfileClient,
    SteamUserdataScanRequest, SteamUserdataScanner,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[test]
fn discovery_auto_saves_single_high_confidence_candidate() {
    let harness = Harness::new();
    harness
        .scanner
        .set_scan_candidates(vec![candidate("candidate-a", 1234, 1_000)]);

    let result = harness.discover_default();

    assert_eq!(result.outcome, SaveDirectoryDiscoveryOutcome::AutoSaved);
    assert_eq!(
        result.saved_settings.unwrap().directory.as_deref(),
        Some("C:/Synthetic/Steam/userdata/1234/582010/remote")
    );
    let saved = harness
        .settings_repo
        .get_settings("default")
        .unwrap()
        .unwrap();
    assert_eq!(
        saved.save_directory.path_label.as_deref(),
        Some("Steam/userdata/<account>/582010/remote")
    );
    assert_eq!(
        saved.backup_directory.path_label.as_deref(),
        Some("mhw/HelsincyModManager/Backups")
    );
}

#[test]
fn discovery_requires_confirmation_for_multiple_candidates_and_recommends_newest() {
    let harness = Harness::new();
    harness.scanner.set_scan_candidates(vec![
        candidate("older", 1111, 1_000),
        candidate("newer", 2222, 2_000),
    ]);
    harness.profile_client.insert(
        2222,
        SteamAccountProfileSummary {
            account_name: Some("New Hunter".to_owned()),
            avatar_url: Some("https://avatars.steamstatic.com/newer.jpg".to_owned()),
        },
    );

    let result = harness.discover_default();

    assert_eq!(
        result.outcome,
        SaveDirectoryDiscoveryOutcome::ConfirmationRequired
    );
    assert_eq!(result.recommended_candidate_id.as_deref(), Some("newer"));
    assert!(result.candidates[0].recommended);
    assert_eq!(
        result.candidates[0].account_name.as_deref(),
        Some("New Hunter")
    );
    assert!(harness
        .settings_repo
        .get_settings("default")
        .unwrap()
        .is_none());
    assert_eq!(harness.pending_store.stored_count(), 1);
}

#[test]
fn discovery_does_not_overwrite_existing_valid_setting() {
    let harness = Harness::new();
    harness
        .settings_repo
        .save_settings(&settings_with_save_directory(
            "C:/Synthetic/Steam/userdata/9999/582010/remote",
        ))
        .unwrap();
    harness
        .scanner
        .set_validate_candidate(candidate("existing", 9999, 1_500));

    let result = harness.discover_default();

    assert_eq!(result.outcome, SaveDirectoryDiscoveryOutcome::ExistingValid);
    assert_eq!(
        result.saved_settings.unwrap().directory.as_deref(),
        Some("C:/Synthetic/Steam/userdata/9999/582010/remote")
    );
    assert_eq!(harness.settings_repo.save_count(), 1);
}

#[test]
fn discovery_reports_scan_failed_when_existing_setting_cannot_be_validated() {
    let harness = Harness::new();
    harness
        .settings_repo
        .save_settings(&settings_with_save_directory(
            "C:/Synthetic/Steam/userdata/9999/582010/remote",
        ))
        .unwrap();
    harness.scanner.set_validate_error(true);

    let result = harness.discover_default();

    assert_eq!(result.outcome, SaveDirectoryDiscoveryOutcome::ScanFailed);
    assert_eq!(
        result.error_code.as_deref(),
        Some("save_directory_discovery_scan_failed")
    );
    assert_eq!(harness.settings_repo.save_count(), 1);
}

#[test]
fn discovery_reports_existing_invalid_for_low_confidence_existing_setting() {
    let harness = Harness::new();
    harness
        .settings_repo
        .save_settings(&settings_with_save_directory(
            "C:/Synthetic/Steam/userdata/9999/582010/remote",
        ))
        .unwrap();
    harness
        .scanner
        .set_validate_candidate(candidate_with_confidence(
            "existing-low-confidence",
            9999,
            1_500,
            SaveDirectoryCandidateConfidence::Low,
        ));

    let result = harness.discover_default();

    assert_eq!(
        result.outcome,
        SaveDirectoryDiscoveryOutcome::ExistingInvalid
    );
    assert_eq!(
        result.error_code.as_deref(),
        Some("save_directory_discovery_candidate_invalid")
    );
    assert_eq!(harness.settings_repo.save_count(), 1);
}

#[test]
fn discovery_degrades_when_steam_profile_lookup_fails() {
    let harness = Harness::new();
    harness.scanner.set_scan_candidates(vec![
        candidate("candidate-a", 1234, 1_000),
        candidate("candidate-b", 5678, 2_000),
    ]);
    harness.profile_client.set_fail(true);

    let result = harness.discover_default();

    assert_eq!(
        result.outcome,
        SaveDirectoryDiscoveryOutcome::ConfirmationRequired
    );
    assert!(result
        .candidates
        .iter()
        .all(|candidate| candidate.account_name.is_none() && candidate.avatar_url.is_none()));
}

#[test]
fn confirm_candidate_revalidates_and_saves_selected_directory() {
    let harness = Harness::new();
    let mut pending = pending_discovery(11_000, vec![pending_candidate("candidate-a", 1234)]);
    pending.candidates[0].summary.account_name = Some("Synthetic Hunter".to_owned());
    pending.candidates[0].summary.avatar_url =
        Some("https://avatars.steamstatic.com/fixture.jpg".to_owned());
    harness.pending_store.put(pending, 10_000).unwrap();
    harness
        .scanner
        .set_validate_candidate(candidate("candidate-a", 1234, 3_000));

    let result = harness
        .service
        .confirm_candidate(ConfirmProfileSaveDirectoryCandidateRequest {
            discovery_id: "discovery-a".to_owned(),
            candidate_id: "candidate-a".to_owned(),
        })
        .expect("confirm");

    assert_eq!(result.outcome, SaveDirectoryDiscoveryOutcome::AutoSaved);
    assert_eq!(harness.scanner.validated_count(), 1);
    let saved = harness
        .settings_repo
        .get_settings("default")
        .unwrap()
        .unwrap();
    assert_eq!(
        saved.save_directory.directory.as_deref(),
        Some("C:/Synthetic/Steam/userdata/1234/582010/remote")
    );
    let account = saved.steam_account.expect("confirmed account snapshot");
    assert_eq!(account.account_name.as_deref(), Some("Synthetic Hunter"));
    assert_eq!(
        account.avatar_url.as_deref(),
        Some("https://avatars.steamstatic.com/fixture.jpg")
    );
    assert_eq!(account.account_label, "Steam user ****1234");
}

#[test]
fn confirm_candidate_consumes_pending_candidate_after_success() {
    let harness = Harness::new();
    let pending = pending_discovery(11_000, vec![pending_candidate("candidate-a", 1234)]);
    harness.pending_store.put(pending, 10_000).unwrap();
    harness
        .scanner
        .set_validate_candidate(candidate("candidate-a", 1234, 3_000));

    harness
        .service
        .confirm_candidate(ConfirmProfileSaveDirectoryCandidateRequest {
            discovery_id: "discovery-a".to_owned(),
            candidate_id: "candidate-a".to_owned(),
        })
        .expect("first confirmation succeeds");

    let replay_error = harness
        .service
        .confirm_candidate(ConfirmProfileSaveDirectoryCandidateRequest {
            discovery_id: "discovery-a".to_owned(),
            candidate_id: "candidate-a".to_owned(),
        })
        .expect_err("candidate should be single-use");

    assert_eq!(replay_error, SaveDirectoryDiscoveryError::CandidateExpired);
    assert_eq!(harness.settings_repo.save_count(), 1);
}

#[test]
fn confirm_candidate_rejects_expired_candidate() {
    let harness = Harness::new();
    let pending = pending_discovery(1_000, vec![pending_candidate("candidate-a", 1234)]);
    harness.pending_store.put(pending, 10_000).unwrap();

    let error = harness
        .service
        .confirm_candidate(ConfirmProfileSaveDirectoryCandidateRequest {
            discovery_id: "discovery-a".to_owned(),
            candidate_id: "candidate-a".to_owned(),
        })
        .expect_err("expired candidate");

    assert_eq!(error, SaveDirectoryDiscoveryError::CandidateExpired);
    assert_eq!(error.code(), "save_directory_discovery_candidate_expired");
}

struct Harness {
    service: ProfileSaveDirectoryDiscoveryService,
    settings_repo: Arc<FakeProfileSaveSettingsRepository>,
    scanner: Arc<FakeSteamUserdataScanner>,
    profile_client: Arc<FakeSteamAccountProfileClient>,
    pending_store: Arc<FakePendingSaveDirectoryCandidateStore>,
}

impl Harness {
    fn new() -> Self {
        let game_config_repository = Arc::new(FakeGameConfigRepository::configured());
        let profile_repository = Arc::new(FakeProfileRepository::with_default_profile());
        let settings_repo = Arc::new(FakeProfileSaveSettingsRepository::default());
        let validator = Arc::new(FakeProfileSaveDirectoryValidator);
        let scanner = Arc::new(FakeSteamUserdataScanner::default());
        let profile_client = Arc::new(FakeSteamAccountProfileClient::default());
        let pending_store = Arc::new(FakePendingSaveDirectoryCandidateStore::default());
        let service = ProfileSaveDirectoryDiscoveryService::new(
            game_config_repository,
            profile_repository,
            Arc::clone(&settings_repo) as _,
            validator,
            vec![Arc::new(FakeMhwSaveDirectoryRule)],
            Arc::clone(&scanner) as _,
            Arc::clone(&profile_client) as _,
            Arc::clone(&pending_store) as _,
            Arc::new(FixedClock(10_000)),
        );

        Self {
            service,
            settings_repo,
            scanner,
            profile_client,
            pending_store,
        }
    }

    fn discover_default(&self) -> hmm_core::SaveDirectoryDiscoveryResult {
        self.service
            .discover(DiscoverProfileSaveDirectoriesRequest {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
            })
            .expect("discover")
    }
}

struct FakeGameConfigRepository {
    instance: Mutex<Option<GameInstance>>,
}

impl FakeGameConfigRepository {
    fn configured() -> Self {
        Self {
            instance: Mutex::new(Some(GameInstance {
                id: "mhw".to_owned(),
                game_id: GameId::mhw(),
                display_name: "Monster Hunter: World - Iceborne".to_owned(),
                root_dir: PathBuf::from("C:/Synthetic/Steam/steamapps/common/Monster Hunter World"),
                status: GameDirectoryStatus::Configured,
                configured_at_unix_millis: 1,
            })),
        }
    }
}

impl GameConfigRepository for FakeGameConfigRepository {
    fn load_game_instance(
        &self,
        _game_id: &GameId,
    ) -> GameConfigRepositoryResult<Option<GameInstance>> {
        Ok(self.instance.lock().unwrap().clone())
    }

    fn save_game_instance(&self, instance: &GameInstance) -> GameConfigRepositoryResult<()> {
        *self.instance.lock().unwrap() = Some(instance.clone());
        Ok(())
    }
}

#[derive(Default)]
struct FakeProfileRepository {
    profiles: Mutex<Vec<Profile>>,
}

impl FakeProfileRepository {
    fn with_default_profile() -> Self {
        Self {
            profiles: Mutex::new(vec![Profile {
                id: "default".to_owned(),
                name: "Default".to_owned(),
                description: None,
                is_active: true,
                created_at: 1,
                updated_at: 1,
            }]),
        }
    }
}

impl ProfileRepository for FakeProfileRepository {
    fn get(&self, profile_id: &str) -> Result<Option<Profile>> {
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned())
    }

    fn save(&self, profile: &Profile) -> Result<()> {
        let mut profiles = self.profiles.lock().unwrap();
        profiles.retain(|existing| existing.id != profile.id);
        profiles.push(profile.clone());
        Ok(())
    }

    fn delete(&self, profile_id: &str) -> Result<()> {
        self.profiles
            .lock()
            .unwrap()
            .retain(|profile| profile.id != profile_id);
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<Profile>> {
        Ok(self.profiles.lock().unwrap().clone())
    }

    fn get_active(&self) -> Result<Option<Profile>> {
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.is_active)
            .cloned())
    }

    fn set_active(&self, profile_id: &str, updated_at: u128) -> Result<()> {
        for profile in self.profiles.lock().unwrap().iter_mut() {
            profile.is_active = profile.id == profile_id;
            if profile.is_active {
                profile.updated_at = updated_at;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct FakeProfileSaveSettingsRepository {
    settings: Mutex<Option<ProfileSaveSettings>>,
    save_count: Mutex<usize>,
}

impl FakeProfileSaveSettingsRepository {
    fn save_count(&self) -> usize {
        *self.save_count.lock().unwrap()
    }
}

impl ProfileSaveSettingsRepository for FakeProfileSaveSettingsRepository {
    fn get_settings(&self, _profile_id: &str) -> Result<Option<ProfileSaveSettings>> {
        Ok(self.settings.lock().unwrap().clone())
    }

    fn save_settings(&self, settings: &ProfileSaveSettings) -> Result<()> {
        *self.settings.lock().unwrap() = Some(settings.clone());
        *self.save_count.lock().unwrap() += 1;
        Ok(())
    }
}

struct FakeProfileSaveDirectoryValidator;

impl ProfileSaveDirectoryValidator for FakeProfileSaveDirectoryValidator {
    fn validate_save_directory(
        &self,
        _game_id: &str,
        directory: &str,
    ) -> Result<ProfileDirectorySelection> {
        Ok(valid_selection(directory))
    }

    fn validate_backup_directory(
        &self,
        _game_id: &str,
        directory: &str,
    ) -> Result<ProfileDirectorySelection> {
        Ok(valid_selection(directory))
    }

    fn default_backup_directory(&self, game_id: &str) -> Result<ProfileDirectorySelection> {
        Ok(ProfileDirectorySelection {
            mode: ProfileDirectoryMode::Default,
            status: ProfileDirectoryStatus::Defaulted,
            directory: None,
            path_label: Some(format!("{game_id}/HelsincyModManager/Backups")),
            messages: vec!["使用默认备份目录".to_owned()],
        })
    }
}

struct FakeMhwSaveDirectoryRule;

impl GameSaveDirectoryRule for FakeMhwSaveDirectoryRule {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn steam_app_id(&self) -> u32 {
        582010
    }

    fn steam_remote_relative_path(&self) -> &'static str {
        "582010/remote"
    }

    fn known_save_file_names(&self) -> &'static [&'static str] {
        &["SAVEDATA1000"]
    }

    fn path_label(&self) -> &'static str {
        "Steam/userdata/<account>/582010/remote"
    }
}

#[derive(Default)]
struct FakeSteamUserdataScanner {
    scan_candidates: Mutex<Vec<ScannedSaveDirectoryCandidate>>,
    scan_error: Mutex<bool>,
    validate_candidate: Mutex<Option<ScannedSaveDirectoryCandidate>>,
    validate_error: Mutex<bool>,
    validated_paths: Mutex<Vec<PathBuf>>,
}

impl FakeSteamUserdataScanner {
    fn set_scan_candidates(&self, candidates: Vec<ScannedSaveDirectoryCandidate>) {
        *self.scan_candidates.lock().unwrap() = candidates;
    }

    fn set_validate_candidate(&self, candidate: ScannedSaveDirectoryCandidate) {
        *self.validate_candidate.lock().unwrap() = Some(candidate);
    }

    fn set_validate_error(&self, value: bool) {
        *self.validate_error.lock().unwrap() = value;
    }

    fn validated_count(&self) -> usize {
        self.validated_paths.lock().unwrap().len()
    }
}

impl SteamUserdataScanner for FakeSteamUserdataScanner {
    fn scan_save_directories(
        &self,
        _request: &SteamUserdataScanRequest,
    ) -> Result<Vec<ScannedSaveDirectoryCandidate>> {
        if *self.scan_error.lock().unwrap() {
            bail!("scan failed");
        }
        Ok(self.scan_candidates.lock().unwrap().clone())
    }

    fn validate_save_directory(
        &self,
        _request: &SteamUserdataScanRequest,
        directory: &Path,
    ) -> Result<ScannedSaveDirectoryCandidate> {
        self.validated_paths
            .lock()
            .unwrap()
            .push(directory.to_path_buf());
        if *self.validate_error.lock().unwrap() {
            bail!("invalid");
        }
        self.validate_candidate
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("missing validation candidate"))
    }
}

#[derive(Default)]
struct FakeSteamAccountProfileClient {
    summaries: Mutex<HashMap<u32, SteamAccountProfileSummary>>,
    fail: Mutex<bool>,
}

impl FakeSteamAccountProfileClient {
    fn insert(&self, account_id_32: u32, summary: SteamAccountProfileSummary) {
        self.summaries
            .lock()
            .unwrap()
            .insert(account_id_32, summary);
    }

    fn set_fail(&self, value: bool) {
        *self.fail.lock().unwrap() = value;
    }
}

impl SteamAccountProfileClient for FakeSteamAccountProfileClient {
    fn fetch_profile(
        &self,
        account_id_32: u32,
        _timeout: Duration,
    ) -> Result<SteamAccountProfileSummary> {
        if *self.fail.lock().unwrap() {
            bail!("profile unavailable");
        }
        Ok(self
            .summaries
            .lock()
            .unwrap()
            .get(&account_id_32)
            .cloned()
            .unwrap_or(SteamAccountProfileSummary {
                account_name: None,
                avatar_url: None,
            }))
    }
}

#[derive(Default)]
struct FakePendingSaveDirectoryCandidateStore {
    discoveries: Mutex<Vec<PendingSaveDirectoryDiscovery>>,
}

impl FakePendingSaveDirectoryCandidateStore {
    fn stored_count(&self) -> usize {
        self.discoveries.lock().unwrap().len()
    }
}

impl PendingSaveDirectoryCandidateStore for FakePendingSaveDirectoryCandidateStore {
    fn put(&self, discovery: PendingSaveDirectoryDiscovery, _now_unix_millis: u128) -> Result<()> {
        let mut discoveries = self.discoveries.lock().unwrap();
        discoveries.retain(|existing| existing.discovery_id != discovery.discovery_id);
        discoveries.push(discovery);
        Ok(())
    }

    fn get_candidate(
        &self,
        discovery_id: &str,
        candidate_id: &str,
        now_unix_millis: u128,
    ) -> Result<Option<PendingSaveDirectoryCandidate>> {
        Ok(self
            .discoveries
            .lock()
            .unwrap()
            .iter()
            .find(|discovery| {
                discovery.discovery_id == discovery_id
                    && discovery.expires_at_unix_millis > now_unix_millis
            })
            .and_then(|discovery| {
                discovery
                    .candidates
                    .iter()
                    .find(|candidate| candidate.summary.candidate_id == candidate_id)
                    .cloned()
            }))
    }

    fn consume_candidate(
        &self,
        discovery_id: &str,
        candidate_id: &str,
        now_unix_millis: u128,
    ) -> Result<Option<PendingSaveDirectoryCandidate>> {
        let mut discoveries = self.discoveries.lock().unwrap();
        discoveries.retain(|discovery| discovery.expires_at_unix_millis > now_unix_millis);

        let candidate = discoveries
            .iter()
            .find(|discovery| discovery.discovery_id == discovery_id)
            .and_then(|discovery| {
                discovery
                    .candidates
                    .iter()
                    .find(|candidate| candidate.summary.candidate_id == candidate_id)
                    .cloned()
            });

        if candidate.is_some() {
            discoveries.retain(|discovery| discovery.discovery_id != discovery_id);
        }

        Ok(candidate)
    }
}

struct FixedClock(u128);

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(self.0)
    }
}

fn candidate(
    candidate_id: &str,
    account_id_32: u32,
    last_modified_at: u128,
) -> ScannedSaveDirectoryCandidate {
    candidate_with_confidence(
        candidate_id,
        account_id_32,
        last_modified_at,
        SaveDirectoryCandidateConfidence::High,
    )
}

fn candidate_with_confidence(
    candidate_id: &str,
    account_id_32: u32,
    last_modified_at: u128,
    confidence: SaveDirectoryCandidateConfidence,
) -> ScannedSaveDirectoryCandidate {
    ScannedSaveDirectoryCandidate {
        candidate_id: candidate_id.to_owned(),
        account_id_32,
        directory: PathBuf::from(format!(
            "C:/Synthetic/Steam/userdata/{account_id_32}/582010/remote"
        )),
        confidence,
        last_modified_at: Some(last_modified_at),
        evidence: vec!["Found MHW:I save file".to_owned()],
        account_label: format!("Steam user ****{account_id_32}"),
        path_label: "Steam/userdata/<account>/582010/remote".to_owned(),
    }
}

fn pending_discovery(
    expires_at_unix_millis: u128,
    candidates: Vec<PendingSaveDirectoryCandidate>,
) -> PendingSaveDirectoryDiscovery {
    PendingSaveDirectoryDiscovery {
        discovery_id: "discovery-a".to_owned(),
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        expires_at_unix_millis,
        candidates,
    }
}

fn pending_candidate(candidate_id: &str, account_id_32: u32) -> PendingSaveDirectoryCandidate {
    let scanned = candidate(candidate_id, account_id_32, 1_000);
    PendingSaveDirectoryCandidate {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        summary: hmm_core::SaveDirectoryCandidateSummary {
            candidate_id: scanned.candidate_id.clone(),
            source: SaveDirectoryCandidateSource::SteamUserdata,
            confidence: scanned.confidence,
            recommended: true,
            account_name: None,
            avatar_url: None,
            account_label: scanned.account_label.clone(),
            path_label: scanned.path_label.clone(),
            last_modified_at: scanned.last_modified_at,
            evidence: scanned.evidence.clone(),
        },
        account_id_32,
        directory: scanned.directory,
    }
}

fn settings_with_save_directory(directory: &str) -> ProfileSaveSettings {
    ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: valid_selection(directory),
        backup_directory: ProfileDirectorySelection {
            mode: ProfileDirectoryMode::Default,
            status: ProfileDirectoryStatus::Defaulted,
            directory: None,
            path_label: Some("mhw/HelsincyModManager/Backups".to_owned()),
            messages: Vec::new(),
        },
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention::default(),
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 1,
    }
}

fn valid_selection(directory: &str) -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Custom,
        status: ProfileDirectoryStatus::Valid,
        directory: Some(directory.to_owned()),
        path_label: Some("Steam/userdata/<account>/582010/remote".to_owned()),
        messages: Vec::new(),
    }
}
