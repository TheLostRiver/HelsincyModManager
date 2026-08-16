use anyhow::Result;
use hmm_app::{
    SaveBackupBackgroundService, SaveBackupExitDecision, SaveBackupExitGuard,
    SaveBackupExitGuardError, SaveBackupExitReason, SAVE_BACKUP_BACKGROUND_STARTUP_GRACE_MILLIS,
};
use hmm_core::{
    BackupCadence, GameId, Profile, ProfileBackupRetention, ProfileBackupSchedule,
    ProfileDirectoryMode, ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId,
    ProfileSaveSettings, SaveBackupBackgroundRegistrationStatus, SaveBackupBackgroundSettings,
    SaveBackupSchedulerLeaseRequest, SaveBackupSchedulerState, SaveBackupWorkerHeartbeat,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, ProfileRepository, ProfileSaveSettingsRepository,
    SaveBackupBackgroundRegistry, SaveBackupBackgroundRegistryResult,
    SaveBackupBackgroundSettingsRepository, SaveBackupSchedulerStateRepository,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const NOW: u128 = SAVE_BACKUP_BACKGROUND_STARTUP_GRACE_MILLIS + 1_000_000;

#[test]
fn no_auto_profile_can_exit_without_background_protection() {
    let harness = Harness::new(BackgroundScenario::WorkerUnhealthy);
    harness.insert_profile("manual", BackupCadence::Manual);

    assert_eq!(
        harness.guard.evaluate().expect("exit decision"),
        SaveBackupExitDecision::Safe
    );
    assert_eq!(harness.registry.inspect_count(), 0);
}

#[test]
fn profile_without_saved_settings_is_treated_as_manual() {
    let harness = Harness::new(BackgroundScenario::WorkerUnhealthy);
    harness.insert_profile_without_settings("default");

    assert_eq!(
        harness.guard.evaluate().expect("exit decision"),
        SaveBackupExitDecision::Safe
    );
    assert_eq!(harness.registry.inspect_count(), 0);
}

#[test]
fn protected_auto_profile_can_exit_without_confirmation() {
    let harness = Harness::new(BackgroundScenario::Protected);
    harness.insert_profile("default", BackupCadence::Daily);

    assert_eq!(
        harness.guard.evaluate().expect("exit decision"),
        SaveBackupExitDecision::Safe
    );
    assert_eq!(harness.registry.inspect_count(), 1);
}

#[test]
fn starting_auto_profile_requires_confirmation() {
    let harness = Harness::new(BackgroundScenario::Starting);
    harness.insert_profile("default", BackupCadence::Daily);

    assert_eq!(
        harness.guard.evaluate().expect("exit decision"),
        confirmation(SaveBackupExitReason::BackgroundStarting)
    );
}

#[test]
fn every_non_protected_control_status_maps_to_a_stable_reason() {
    let cases = [
        (
            BackgroundScenario::NotEnabled,
            SaveBackupExitReason::BackgroundNotEnabled,
        ),
        (
            BackgroundScenario::RegistrationFailed,
            SaveBackupExitReason::RegistrationFailed,
        ),
        (
            BackgroundScenario::WorkerUnhealthy,
            SaveBackupExitReason::WorkerUnhealthy,
        ),
        (
            BackgroundScenario::PermissionRequired,
            SaveBackupExitReason::PermissionRequired,
        ),
        (
            BackgroundScenario::UnsupportedPlatform,
            SaveBackupExitReason::UnsupportedPlatform,
        ),
    ];

    for (scenario, reason) in cases {
        let harness = Harness::new(scenario);
        harness.insert_profile("default", BackupCadence::Weekly);

        assert_eq!(
            harness.guard.evaluate().expect("exit decision"),
            confirmation(reason),
            "scenario {scenario:?}"
        );
    }
}

#[test]
fn profile_list_failure_requires_confirmation_without_querying_background_status() {
    let harness = Harness::new(BackgroundScenario::Protected);
    harness.fail_profile_list();

    assert_eq!(
        harness.guard.evaluate().expect("exit decision"),
        confirmation(SaveBackupExitReason::StatusUnavailable)
    );
    assert_eq!(harness.registry.inspect_count(), 0);
}

#[test]
fn any_profile_settings_failure_requires_confirmation() {
    let harness = Harness::new(BackgroundScenario::Protected);
    harness.insert_profile("auto", BackupCadence::Daily);
    harness.insert_profile_without_settings("unavailable");
    harness.fail_settings_for("unavailable");

    assert_eq!(
        harness.guard.evaluate().expect("exit decision"),
        confirmation(SaveBackupExitReason::StatusUnavailable)
    );
    assert_eq!(harness.registry.inspect_count(), 0);
}

#[test]
fn background_status_dependency_failure_requires_confirmation() {
    for scenario in [
        BackgroundScenario::StatusUnavailable,
        BackgroundScenario::ControlClockUnavailable,
    ] {
        let harness = Harness::new(scenario);
        harness.insert_profile("default", BackupCadence::Daily);

        assert_eq!(
            harness.guard.evaluate().expect("exit decision"),
            confirmation(SaveBackupExitReason::StatusUnavailable),
            "scenario {scenario:?}"
        );
    }
}

#[test]
fn override_audit_uses_only_stable_whitelisted_fields() {
    let harness = Harness::new(BackgroundScenario::Protected);
    let cases = [
        (SaveBackupExitReason::BackgroundStarting, "starting", ""),
        (
            SaveBackupExitReason::BackgroundNotEnabled,
            "not_enabled",
            "save_backup_background_not_enabled",
        ),
        (
            SaveBackupExitReason::RegistrationFailed,
            "registration_failed",
            "save_backup_background_registration_failed",
        ),
        (
            SaveBackupExitReason::WorkerUnhealthy,
            "worker_unhealthy",
            "save_backup_background_worker_unhealthy",
        ),
        (
            SaveBackupExitReason::PermissionRequired,
            "permission_required",
            "save_backup_background_permission_required",
        ),
        (
            SaveBackupExitReason::UnsupportedPlatform,
            "unsupported_platform",
            "save_backup_background_unsupported_platform",
        ),
        (
            SaveBackupExitReason::StatusUnavailable,
            "status_unavailable",
            "save_backup_background_status_unavailable",
        ),
    ];

    for (reason, _, _) in cases {
        harness
            .guard
            .record_override(reason)
            .expect("override audit");
    }

    let events = harness.audit.events();
    assert_eq!(events.len(), cases.len());
    for (event, (_, protection_status, error_code)) in events.iter().zip(cases) {
        assert_eq!(event.timestamp_unix_millis, NOW);
        assert_eq!(event.category, "save_backup");
        assert_eq!(event.operation, "background_exit_override");
        assert_eq!(event.result, "success");
        assert_eq!(
            event.fields.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["error_code", "protection_status"]
        );
        assert_eq!(event.fields["protection_status"], protection_status);
        assert_eq!(event.fields["error_code"], error_code);

        let serialized = serde_json::to_string(event).expect("serialize audit event");
        for forbidden in [
            "C:/Users",
            "S-1-5-21",
            "profile-list",
            "worker-instance",
            "lease-owner",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }
}

#[test]
fn override_clock_and_audit_failures_have_stable_sanitized_errors() {
    let clock_failure = Harness::new(BackgroundScenario::Protected);
    clock_failure.clock.set_failing(true);
    let error = clock_failure
        .guard
        .record_override(SaveBackupExitReason::WorkerUnhealthy)
        .expect_err("clock failure");
    assert_eq!(error, SaveBackupExitGuardError::ClockUnavailable);
    assert_eq!(error.code(), "save_backup_clock_unavailable");
    assert_eq!(
        error.to_string(),
        "save backup exit guard clock is unavailable"
    );
    assert!(clock_failure.audit.events().is_empty());

    let audit_failure = Harness::new(BackgroundScenario::Protected);
    audit_failure.audit.set_failing(true);
    let error = audit_failure
        .guard
        .record_override(SaveBackupExitReason::WorkerUnhealthy)
        .expect_err("audit failure");
    assert_eq!(error, SaveBackupExitGuardError::AuditUnavailable);
    assert_eq!(error.code(), "save_backup_background_audit_unavailable");
    assert_eq!(
        error.to_string(),
        "save backup exit guard audit is unavailable"
    );
    assert!(audit_failure.audit.events().is_empty());
}

fn confirmation(reason: SaveBackupExitReason) -> SaveBackupExitDecision {
    SaveBackupExitDecision::ConfirmationRequired { reason }
}

#[derive(Debug, Clone, Copy)]
enum BackgroundScenario {
    Protected,
    Starting,
    NotEnabled,
    RegistrationFailed,
    WorkerUnhealthy,
    PermissionRequired,
    UnsupportedPlatform,
    StatusUnavailable,
    ControlClockUnavailable,
}

struct Harness {
    guard: SaveBackupExitGuard,
    profiles: Arc<FakeProfileRepository>,
    profile_settings: Arc<FakeProfileSaveSettingsRepository>,
    registry: Arc<FakeRegistry>,
    audit: Arc<RecordingAuditLog>,
    clock: Arc<FixedClock>,
}

impl Harness {
    fn new(scenario: BackgroundScenario) -> Self {
        let profiles = Arc::new(FakeProfileRepository::default());
        let profile_settings = Arc::new(FakeProfileSaveSettingsRepository::default());
        let (settings, registration, settings_fail, clock_fail) = scenario.configuration();
        let registry = Arc::new(FakeRegistry::new(registration));
        let background_settings = Arc::new(FakeBackgroundSettingsRepository {
            settings,
            fail_load: settings_fail,
        });
        let audit = Arc::new(RecordingAuditLog::default());
        let clock = Arc::new(FixedClock::new(clock_fail));
        let background_service =
            Arc::new(SaveBackupBackgroundService::new_with_settings_repository(
                registry.clone(),
                Arc::new(UnusedSchedulerRepository),
                background_settings,
                audit.clone(),
                clock.clone(),
            ));
        let guard = SaveBackupExitGuard::new(
            profiles.clone(),
            profile_settings.clone(),
            background_service,
            audit.clone(),
            clock.clone(),
        );

        Self {
            guard,
            profiles,
            profile_settings,
            registry,
            audit,
            clock,
        }
    }

    fn insert_profile(&self, profile_id: &str, cadence: BackupCadence) {
        self.insert_profile_without_settings(profile_id);
        self.profile_settings.insert(settings(profile_id, cadence));
    }

    fn insert_profile_without_settings(&self, profile_id: &str) {
        self.profiles.insert(Profile {
            id: profile_id.to_owned(),
            name: "Profile".to_owned(),
            description: None,
            is_active: false,
            created_at: 1,
            updated_at: 1,
        });
    }

    fn fail_profile_list(&self) {
        self.profiles.fail_list.store(true, Ordering::Relaxed);
    }

    fn fail_settings_for(&self, profile_id: &str) {
        *self
            .profile_settings
            .fail_profile_id
            .lock()
            .expect("settings failure lock") = Some(profile_id.to_owned());
    }
}

impl BackgroundScenario {
    fn configuration(
        self,
    ) -> (
        SaveBackupBackgroundSettings,
        SaveBackupBackgroundRegistrationStatus,
        bool,
        bool,
    ) {
        let enabled = |enabled_at, heartbeat| SaveBackupBackgroundSettings {
            desired_enabled: true,
            enabled_at: Some(enabled_at),
            last_worker_heartbeat_at: heartbeat,
            updated_at: heartbeat.unwrap_or(enabled_at),
        };
        match self {
            Self::Protected => (
                enabled(NOW - 1, Some(NOW)),
                SaveBackupBackgroundRegistrationStatus::Registered,
                false,
                false,
            ),
            Self::Starting => (
                enabled(NOW, None),
                SaveBackupBackgroundRegistrationStatus::Registered,
                false,
                false,
            ),
            Self::NotEnabled => (
                SaveBackupBackgroundSettings::disabled(),
                SaveBackupBackgroundRegistrationStatus::Registered,
                false,
                false,
            ),
            Self::RegistrationFailed => (
                enabled(NOW, None),
                SaveBackupBackgroundRegistrationStatus::RegistrationFailed,
                false,
                false,
            ),
            Self::WorkerUnhealthy => (
                enabled(NOW - SAVE_BACKUP_BACKGROUND_STARTUP_GRACE_MILLIS - 1, None),
                SaveBackupBackgroundRegistrationStatus::Registered,
                false,
                false,
            ),
            Self::PermissionRequired => (
                enabled(NOW, None),
                SaveBackupBackgroundRegistrationStatus::PermissionRequired,
                false,
                false,
            ),
            Self::UnsupportedPlatform => (
                enabled(NOW, None),
                SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform,
                false,
                false,
            ),
            Self::StatusUnavailable => (
                SaveBackupBackgroundSettings::disabled(),
                SaveBackupBackgroundRegistrationStatus::Registered,
                true,
                false,
            ),
            Self::ControlClockUnavailable => (
                enabled(NOW, None),
                SaveBackupBackgroundRegistrationStatus::Registered,
                false,
                true,
            ),
        }
    }
}

fn settings(profile_id: &str, cadence: BackupCadence) -> ProfileSaveSettings {
    ProfileSaveSettings {
        profile_id: profile_id.to_owned(),
        save_directory: unset_directory(),
        backup_directory: unset_directory(),
        schedule: if cadence == BackupCadence::Manual {
            ProfileBackupSchedule::manual()
        } else {
            ProfileBackupSchedule {
                cadence,
                hour: Some(3),
                minute: Some(0),
                weekdays: (cadence == BackupCadence::Weekly)
                    .then_some(vec![1])
                    .unwrap_or_default(),
            }
        },
        retention: ProfileBackupRetention::default(),
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 1,
    }
}

fn unset_directory() -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Unset,
        status: ProfileDirectoryStatus::Unset,
        directory: None,
        path_label: None,
        messages: Vec::new(),
    }
}

#[derive(Default)]
struct FakeProfileRepository {
    profiles: Mutex<Vec<Profile>>,
    fail_list: AtomicBool,
}

impl FakeProfileRepository {
    fn insert(&self, profile: Profile) {
        self.profiles.lock().expect("profiles lock").push(profile);
    }
}

impl ProfileRepository for FakeProfileRepository {
    fn get(&self, profile_id: &str) -> Result<Option<Profile>> {
        Ok(self
            .profiles
            .lock()
            .expect("profiles lock")
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned())
    }

    fn save(&self, _profile: &Profile) -> Result<()> {
        panic!("unused")
    }

    fn delete(&self, _profile_id: &str) -> Result<()> {
        panic!("unused")
    }

    fn list_all(&self) -> Result<Vec<Profile>> {
        if self.fail_list.load(Ordering::Relaxed) {
            anyhow::bail!("raw profile-list failure: C:/Users/Alice")
        }
        Ok(self.profiles.lock().expect("profiles lock").clone())
    }

    fn get_active(&self) -> Result<Option<Profile>> {
        panic!("unused")
    }

    fn set_active(&self, _profile_id: &str, _updated_at: u128) -> Result<()> {
        panic!("unused")
    }
}

#[derive(Default)]
struct FakeProfileSaveSettingsRepository {
    settings: Mutex<Vec<ProfileSaveSettings>>,
    fail_profile_id: Mutex<Option<String>>,
}

impl FakeProfileSaveSettingsRepository {
    fn insert(&self, settings: ProfileSaveSettings) {
        self.settings.lock().expect("settings lock").push(settings);
    }
}

impl ProfileSaveSettingsRepository for FakeProfileSaveSettingsRepository {
    fn get_settings(&self, profile_id: &str) -> Result<Option<ProfileSaveSettings>> {
        if self
            .fail_profile_id
            .lock()
            .expect("settings failure lock")
            .as_deref()
            == Some(profile_id)
        {
            anyhow::bail!("raw settings failure: S-1-5-21")
        }
        Ok(self
            .settings
            .lock()
            .expect("settings lock")
            .iter()
            .find(|settings| settings.profile_id == profile_id)
            .cloned())
    }

    fn save_settings(&self, _settings: &ProfileSaveSettings) -> Result<()> {
        panic!("unused")
    }
}

struct FakeRegistry {
    status: SaveBackupBackgroundRegistrationStatus,
    inspect_count: AtomicUsize,
}

impl FakeRegistry {
    fn new(status: SaveBackupBackgroundRegistrationStatus) -> Self {
        Self {
            status,
            inspect_count: AtomicUsize::new(0),
        }
    }

    fn inspect_count(&self) -> usize {
        self.inspect_count.load(Ordering::Relaxed)
    }
}

impl SaveBackupBackgroundRegistry for FakeRegistry {
    fn inspect(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.inspect_count.fetch_add(1, Ordering::Relaxed);
        Ok(self.status)
    }

    fn register(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        panic!("unused")
    }

    fn unregister(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        panic!("unused")
    }
}

struct FakeBackgroundSettingsRepository {
    settings: SaveBackupBackgroundSettings,
    fail_load: bool,
}

impl SaveBackupBackgroundSettingsRepository for FakeBackgroundSettingsRepository {
    fn load(&self) -> Result<SaveBackupBackgroundSettings> {
        if self.fail_load {
            anyhow::bail!("raw background status failure: worker-instance")
        }
        Ok(self.settings.clone())
    }

    fn begin_enable(&self, _enabled_at: u128) -> Result<()> {
        panic!("unused")
    }

    fn finish_disable(&self, _updated_at: u128) -> Result<()> {
        panic!("unused")
    }

    fn record_worker_heartbeat(&self, _heartbeat_at: u128) -> Result<()> {
        panic!("unused")
    }
}

struct UnusedSchedulerRepository;

impl SaveBackupSchedulerStateRepository for UnusedSchedulerRepository {
    fn get_state(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
    ) -> Result<Option<SaveBackupSchedulerState>> {
        panic!("unused")
    }

    fn upsert_state(&self, _state: &SaveBackupSchedulerState) -> Result<()> {
        panic!("unused")
    }

    fn acquire_due_lease(
        &self,
        _request: SaveBackupSchedulerLeaseRequest,
    ) -> Result<Option<SaveBackupSchedulerState>> {
        panic!("unused")
    }

    fn release_lease(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
        _lease_owner: &str,
    ) -> Result<()> {
        panic!("unused")
    }

    fn record_worker_heartbeat(&self, _heartbeat: SaveBackupWorkerHeartbeat) -> Result<()> {
        panic!("unused")
    }
}

struct FixedClock {
    fail: AtomicBool,
}

impl FixedClock {
    fn new(fail: bool) -> Self {
        Self {
            fail: AtomicBool::new(fail),
        }
    }

    fn set_failing(&self, fail: bool) {
        self.fail.store(fail, Ordering::Relaxed);
    }
}

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        if self.fail.load(Ordering::Relaxed) {
            anyhow::bail!("raw clock failure: C:/Users/Alice/save")
        }
        Ok(NOW)
    }
}

#[derive(Default)]
struct RecordingAuditLog {
    events: Mutex<Vec<AuditLogEvent>>,
    fail: AtomicBool,
}

impl RecordingAuditLog {
    fn events(&self) -> Vec<AuditLogEvent> {
        self.events.lock().expect("audit lock").clone()
    }

    fn set_failing(&self, fail: bool) {
        self.fail.store(fail, Ordering::Relaxed);
    }
}

impl AuditLogWriter for RecordingAuditLog {
    fn record(&self, event: AuditLogEvent) -> Result<()> {
        if self.fail.load(Ordering::Relaxed) {
            anyhow::bail!("raw audit failure: lease-owner S-1-5-21")
        }
        self.events.lock().expect("audit lock").push(event);
        Ok(())
    }
}
