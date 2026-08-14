use anyhow::Result;
use hmm_app::{
    PreviewSaveRestoreRequest, SaveRestorePreviewError, SaveRestoreService,
    Sha256SaveRestoreTokenCodec, StartSaveRestoreRequest,
};
use hmm_core::{
    BackupCadence, GameId, Profile, ProfileBackupRetention, ProfileBackupSchedule,
    ProfileDirectoryMode, ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId,
    ProfileSaveSettings, SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
    SaveRestoreTransaction,
};
use hmm_ports::{
    AppClock, GameRunningDetector, GameRunningStatus, ProfileRepository,
    ProfileSaveSettingsRepository, SaveBackupRepository, SaveRestoreSourceError,
    SaveRestoreSourceValidator, SaveRestoreTransactionRepository, ValidatedSaveRestoreSource,
};
use std::sync::{Arc, Mutex};

#[test]
fn preview_and_commit_revalidate_exact_restore_facts() {
    let harness = Harness::new();

    let preview = harness
        .service
        .preview(harness.preview_request())
        .expect("preview");
    assert_eq!(preview.backup.backup_id, "backup-1");
    assert_eq!(preview.file_count, 1);
    assert!(!preview.requires_additional_confirmation);

    let context = harness
        .service
        .validate_for_commit(harness.commit_request(preview.preview_token))
        .expect("commit facts");
    assert_eq!(context.summary.backup_id, "backup-1");
    assert_eq!(context.validated_source.backup_id, "backup-1");
    assert_eq!(
        harness.backups.take_requests(),
        vec![
            (
                "mhw".to_owned(),
                "default".to_owned(),
                "backup-1".to_owned()
            ),
            (
                "mhw".to_owned(),
                "default".to_owned(),
                "backup-1".to_owned()
            ),
        ]
    );
}

#[test]
fn running_and_unknown_game_states_block_restore_preview() {
    let harness = Harness::new();
    harness.running.set(GameRunningStatus::Running);
    assert_eq!(
        harness.service.preview(harness.preview_request()),
        Err(SaveRestorePreviewError::GameRunning)
    );

    harness.running.set(GameRunningStatus::Unknown);
    assert_eq!(
        harness.service.preview(harness.preview_request()),
        Err(SaveRestorePreviewError::GameRunningUnknown)
    );
}

#[test]
fn disabled_pre_restore_backup_requires_explicit_high_risk_confirmation() {
    let harness = Harness::new();
    harness.settings.mutate(|settings| {
        settings.pre_restore_backup_enabled = false;
        settings.updated_at += 1;
    });
    let preview = harness
        .service
        .preview(harness.preview_request())
        .expect("preview");
    assert!(preview.requires_additional_confirmation);
    assert_eq!(
        preview.warning_codes,
        vec!["save_restore_pre_restore_disabled".to_owned()]
    );

    let request = harness.commit_request(preview.preview_token.clone());
    assert_eq!(
        harness.service.validate_for_commit(request),
        Err(SaveRestorePreviewError::HighRiskConfirmationRequired)
    );

    let mut request = harness.commit_request(preview.preview_token);
    request.confirmed_without_pre_restore = true;
    harness
        .service
        .validate_for_commit(request)
        .expect("explicit high-risk confirmation");
}

#[test]
fn settings_drift_invalidates_preview_token() {
    let harness = Harness::new();
    let preview = harness
        .service
        .preview(harness.preview_request())
        .expect("preview");
    harness.settings.mutate(|settings| settings.updated_at += 1);

    assert_eq!(
        harness
            .service
            .validate_for_commit(harness.commit_request(preview.preview_token)),
        Err(SaveRestorePreviewError::StaleToken)
    );
}

#[test]
fn backup_archive_drift_invalidates_preview_token() {
    let harness = Harness::new();
    let preview = harness
        .service
        .preview(harness.preview_request())
        .expect("preview");
    harness.backups.mutate(|summary| {
        summary.archive_sha256 = "sha256:changed".to_owned();
    });
    harness.validator.mutate(|source| {
        source.evidence_digest = "sha256:changed-evidence".to_owned();
    });

    assert_eq!(
        harness
            .service
            .validate_for_commit(harness.commit_request(preview.preview_token)),
        Err(SaveRestorePreviewError::StaleToken)
    );
}

#[test]
fn incomplete_transaction_blocks_new_restore() {
    let harness = Harness::new();
    harness.transactions.set_incomplete(true);

    assert_eq!(
        harness.service.preview(harness.preview_request()),
        Err(SaveRestorePreviewError::RecoveryRequired)
    );
}

#[test]
fn commit_revalidation_excludes_only_the_current_transaction() {
    let harness = Harness::new();
    let preview = harness
        .service
        .preview(harness.preview_request())
        .expect("preview");
    harness.transactions.set_incomplete_ids(["restore-current"]);

    harness
        .service
        .validate_for_commit_excluding_transaction(
            harness.commit_request(preview.preview_token.clone()),
            "restore-current",
        )
        .expect("current transaction is excluded from its own revalidation");
    assert_eq!(
        harness.service.validate_for_commit_excluding_transaction(
            harness.commit_request(preview.preview_token),
            "restore-other",
        ),
        Err(SaveRestorePreviewError::RecoveryRequired)
    );
}

#[test]
fn backup_and_settings_location_drift_invalidates_preview_token() {
    let harness = Harness::new();
    let preview = harness
        .service
        .preview(harness.preview_request())
        .expect("preview");
    harness.backups.mutate(|summary| {
        summary.backup_directory = custom_directory("C:/HMMFixtures/other-backup");
    });

    assert_eq!(
        harness
            .service
            .validate_for_commit(harness.commit_request(preview.preview_token)),
        Err(SaveRestorePreviewError::StaleToken)
    );

    let harness = Harness::new();
    let preview = harness
        .service
        .preview(harness.preview_request())
        .expect("preview");
    harness.settings.mutate(|settings| {
        settings.backup_directory = custom_directory("C:/HMMFixtures/other-backup");
    });
    assert_eq!(
        harness
            .service
            .validate_for_commit(harness.commit_request(preview.preview_token)),
        Err(SaveRestorePreviewError::StaleToken)
    );
}

#[test]
fn repository_and_validator_identity_mismatches_are_rejected() {
    let harness = Harness::new();
    harness.backups.mutate(|summary| {
        summary.backup_id = "wrong-backup".to_owned();
    });
    assert_eq!(
        harness.service.preview(harness.preview_request()),
        Err(SaveRestorePreviewError::BackupUnavailable)
    );

    harness.backups.mutate(|summary| {
        summary.backup_id = "backup-1".to_owned();
    });
    harness.validator.mutate(|source| {
        source.backup_id = "wrong-source".to_owned();
    });
    assert_eq!(
        harness.service.preview(harness.preview_request()),
        Err(SaveRestorePreviewError::SourceIdentityMismatch)
    );
}

struct Harness {
    service: SaveRestoreService,
    settings: Arc<FakeSettingsRepository>,
    backups: Arc<FakeBackupRepository>,
    validator: Arc<FakeSourceValidator>,
    transactions: Arc<FakeTransactionRepository>,
    running: Arc<FakeGameRunningDetector>,
}

impl Harness {
    fn new() -> Self {
        let profiles = Arc::new(FakeProfileRepository::new(sample_profile()));
        let settings = Arc::new(FakeSettingsRepository::new(sample_settings()));
        let backups = Arc::new(FakeBackupRepository::new(sample_backup()));
        let validator = Arc::new(FakeSourceValidator::new(sample_source()));
        let transactions = Arc::new(FakeTransactionRepository::default());
        let running = Arc::new(FakeGameRunningDetector::default());
        let service = SaveRestoreService::new(
            profiles,
            settings.clone(),
            backups.clone(),
            validator.clone(),
            transactions.clone(),
            running.clone(),
            Arc::new(FixedClock(1_000)),
            Arc::new(Sha256SaveRestoreTokenCodec::new("fixture-secret").expect("codec")),
        );
        Self {
            service,
            settings,
            backups,
            validator,
            transactions,
            running,
        }
    }

    fn preview_request(&self) -> PreviewSaveRestoreRequest {
        PreviewSaveRestoreRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            backup_id: "backup-1".to_owned(),
        }
    }

    fn commit_request(&self, preview_token: String) -> StartSaveRestoreRequest {
        StartSaveRestoreRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            backup_id: "backup-1".to_owned(),
            preview_token,
            confirmed: true,
            confirmed_without_pre_restore: false,
        }
    }
}

fn sample_profile() -> Profile {
    Profile {
        id: "default".to_owned(),
        name: "Default".to_owned(),
        description: None,
        is_active: true,
        created_at: 1,
        updated_at: 1,
    }
}

fn sample_settings() -> ProfileSaveSettings {
    ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory("C:/HMMFixtures/save"),
        backup_directory: custom_directory("C:/HMMFixtures/backup"),
        schedule: ProfileBackupSchedule {
            cadence: BackupCadence::Manual,
            hour: None,
            minute: None,
            weekdays: Vec::new(),
        },
        retention: ProfileBackupRetention::default(),
        pre_restore_backup_enabled: true,
        updated_at: 10,
    }
}

fn sample_backup() -> SaveBackupSummary {
    SaveBackupSummary {
        backup_id: "backup-1".to_owned(),
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        trigger: SaveBackupTrigger::Manual,
        status: SaveBackupStatus::Completed,
        archive_file_name: "backup-1.zip".to_owned(),
        manifest_file_name: "backup-1.manifest.json".to_owned(),
        archive_size_bytes: 36,
        archive_sha256: "sha256:archive".to_owned(),
        file_count: 1,
        created_at: 5,
        source_path_label: Some("fixture".to_owned()),
        source_path_hash: "sha256:source".to_owned(),
        backup_directory: custom_directory("C:/HMMFixtures/backup"),
        notes: None,
    }
}

fn sample_source() -> ValidatedSaveRestoreSource {
    ValidatedSaveRestoreSource {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        backup_id: "backup-1".to_owned(),
        evidence_digest: "sha256:evidence".to_owned(),
        file_count: 1,
        total_uncompressed_bytes: 36,
    }
}

fn custom_directory(path: &str) -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Custom,
        status: ProfileDirectoryStatus::Valid,
        directory: Some(path.to_owned()),
        path_label: Some("fixture".to_owned()),
        messages: Vec::new(),
    }
}

struct FakeProfileRepository {
    profile: Mutex<Option<Profile>>,
}

impl FakeProfileRepository {
    fn new(profile: Profile) -> Self {
        Self {
            profile: Mutex::new(Some(profile)),
        }
    }
}

impl ProfileRepository for FakeProfileRepository {
    fn get(&self, profile_id: &str) -> Result<Option<Profile>> {
        Ok(self
            .profile
            .lock()
            .expect("profile")
            .clone()
            .filter(|profile| profile.id == profile_id))
    }

    fn save(&self, profile: &Profile) -> Result<()> {
        *self.profile.lock().expect("profile") = Some(profile.clone());
        Ok(())
    }

    fn delete(&self, profile_id: &str) -> Result<()> {
        let mut profile = self.profile.lock().expect("profile");
        if profile
            .as_ref()
            .is_some_and(|profile| profile.id == profile_id)
        {
            *profile = None;
        }
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<Profile>> {
        Ok(self
            .profile
            .lock()
            .expect("profile")
            .clone()
            .into_iter()
            .collect())
    }

    fn get_active(&self) -> Result<Option<Profile>> {
        Ok(self
            .profile
            .lock()
            .expect("profile")
            .clone()
            .filter(|profile| profile.is_active))
    }

    fn set_active(&self, profile_id: &str, updated_at: u128) -> Result<()> {
        if let Some(profile) = self.profile.lock().expect("profile").as_mut() {
            profile.is_active = profile.id == profile_id;
            profile.updated_at = updated_at;
        }
        Ok(())
    }
}

struct FakeSettingsRepository {
    settings: Mutex<ProfileSaveSettings>,
}

impl FakeSettingsRepository {
    fn new(settings: ProfileSaveSettings) -> Self {
        Self {
            settings: Mutex::new(settings),
        }
    }

    fn mutate(&self, update: impl FnOnce(&mut ProfileSaveSettings)) {
        update(&mut self.settings.lock().expect("settings"));
    }
}

impl ProfileSaveSettingsRepository for FakeSettingsRepository {
    fn get_settings(&self, profile_id: &str) -> Result<Option<ProfileSaveSettings>> {
        let settings = self.settings.lock().expect("settings");
        Ok((settings.profile_id == profile_id).then(|| settings.clone()))
    }

    fn save_settings(&self, settings: &ProfileSaveSettings) -> Result<()> {
        *self.settings.lock().expect("settings") = settings.clone();
        Ok(())
    }
}

struct FakeBackupRepository {
    summary: Mutex<SaveBackupSummary>,
    requests: Mutex<Vec<(String, String, String)>>,
}

impl FakeBackupRepository {
    fn new(summary: SaveBackupSummary) -> Self {
        Self {
            summary: Mutex::new(summary),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn mutate(&self, update: impl FnOnce(&mut SaveBackupSummary)) {
        update(&mut self.summary.lock().expect("summary"));
    }

    fn take_requests(&self) -> Vec<(String, String, String)> {
        std::mem::take(&mut *self.requests.lock().expect("requests"))
    }
}

impl SaveBackupRepository for FakeBackupRepository {
    fn save(&self, summary: &SaveBackupSummary) -> Result<()> {
        *self.summary.lock().expect("summary") = summary.clone();
        Ok(())
    }

    fn get_for_restore(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        backup_id: &str,
    ) -> Result<Option<SaveBackupSummary>> {
        self.requests.lock().expect("requests").push((
            game_id.as_str().to_owned(),
            profile_id.as_str().to_owned(),
            backup_id.to_owned(),
        ));
        Ok(Some(self.summary.lock().expect("summary").clone()))
    }

    fn list_for_profile(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
        _limit: Option<usize>,
    ) -> Result<Vec<SaveBackupSummary>> {
        Ok(vec![self.summary.lock().expect("summary").clone()])
    }

    fn mark_status(&self, _backup_id: &str, status: SaveBackupStatus) -> Result<()> {
        self.summary.lock().expect("summary").status = status;
        Ok(())
    }
}

struct FakeSourceValidator {
    source: Mutex<ValidatedSaveRestoreSource>,
}

impl FakeSourceValidator {
    fn new(source: ValidatedSaveRestoreSource) -> Self {
        Self {
            source: Mutex::new(source),
        }
    }

    fn mutate(&self, update: impl FnOnce(&mut ValidatedSaveRestoreSource)) {
        update(&mut self.source.lock().expect("source"));
    }
}

impl SaveRestoreSourceValidator for FakeSourceValidator {
    fn validate_source(
        &self,
        _summary: &SaveBackupSummary,
    ) -> std::result::Result<ValidatedSaveRestoreSource, SaveRestoreSourceError> {
        Ok(self.source.lock().expect("source").clone())
    }
}

#[derive(Default)]
struct FakeTransactionRepository {
    incomplete_ids: Mutex<Vec<String>>,
}

impl FakeTransactionRepository {
    fn set_incomplete(&self, incomplete: bool) {
        self.set_incomplete_ids(incomplete.then_some("restore-other"));
    }

    fn set_incomplete_ids(&self, ids: impl IntoIterator<Item = impl Into<String>>) {
        *self.incomplete_ids.lock().expect("incomplete ids") =
            ids.into_iter().map(Into::into).collect();
    }
}

impl SaveRestoreTransactionRepository for FakeTransactionRepository {
    fn save_transaction(&self, _transaction: &SaveRestoreTransaction) -> Result<()> {
        Ok(())
    }

    fn get_transaction(&self, _transaction_id: &str) -> Result<Option<SaveRestoreTransaction>> {
        Ok(None)
    }

    fn has_incomplete_transaction_excluding(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
        excluded_transaction_id: Option<&str>,
    ) -> Result<bool> {
        Ok(self
            .incomplete_ids
            .lock()
            .expect("incomplete ids")
            .iter()
            .any(|transaction_id| Some(transaction_id.as_str()) != excluded_transaction_id))
    }
}

struct FakeGameRunningDetector {
    status: Mutex<GameRunningStatus>,
}

impl Default for FakeGameRunningDetector {
    fn default() -> Self {
        Self {
            status: Mutex::new(GameRunningStatus::NotRunning),
        }
    }
}

impl FakeGameRunningDetector {
    fn set(&self, status: GameRunningStatus) {
        *self.status.lock().expect("running status") = status;
    }
}

impl GameRunningDetector for FakeGameRunningDetector {
    fn game_running_status(&self, _game_id: &GameId) -> GameRunningStatus {
        *self.status.lock().expect("running status")
    }
}

struct FixedClock(u128);

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(self.0)
    }
}
