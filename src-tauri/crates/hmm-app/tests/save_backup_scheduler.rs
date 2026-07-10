use anyhow::Result;
use hmm_app::{
    SaveBackupAutoCheckRequest, SaveBackupAutoCheckStatus, SaveBackupAutoSchedulerService,
};
use hmm_core::{
    BackupCadence, GameId, Profile, ProfileBackupRetention, ProfileBackupSchedule,
    ProfileDirectoryMode, ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId,
    ProfileSaveSettings, SaveBackupBackgroundProtectionStatus, SaveBackupSchedulerLeaseRequest,
    SaveBackupSchedulerPendingReason, SaveBackupSchedulerState, SaveBackupStatus,
    SaveBackupSummary, SaveBackupTrigger, SaveBackupWorkerHeartbeat,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, GameRunningDetector, GameRunningStatus,
    ProfileRepository, ProfileSaveSettingsRepository, SaveBackupRepository,
    SaveBackupSchedulerStateRepository,
};
use std::sync::{Arc, Mutex};

const DAY_MS: u128 = 86_400_000;
const HOUR_MS: u128 = 3_600_000;

#[test]
fn manual_schedule_is_reported_without_starting_auto_backup() {
    let harness = Harness::new(DAY_MS + 4 * HOUR_MS);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule::manual()));

    let result = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("manual schedule should be checked");

    assert_eq!(result.status, SaveBackupAutoCheckStatus::ManualOnly);
    assert!(result.due_task.is_none());
}

#[test]
fn daily_schedule_due_without_prior_auto_backup_returns_auto_task_request() {
    let harness = Harness::new(2 * DAY_MS + 4 * HOUR_MS);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule {
        cadence: BackupCadence::Daily,
        hour: Some(3),
        minute: Some(0),
        weekdays: Vec::new(),
    }));

    let result = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("daily schedule should be due");

    assert_eq!(result.status, SaveBackupAutoCheckStatus::Due);
    assert_eq!(result.next_due_at, Some(3 * DAY_MS + 3 * HOUR_MS));
    let task = result.due_task.expect("due schedule returns task request");
    assert_eq!(task.game_id, GameId::mhw());
    assert_eq!(task.profile_id, ProfileId::new("default"));
    assert_eq!(task.trigger, SaveBackupTrigger::Auto);
    assert!(task.note.is_none());
}

#[test]
fn daily_schedule_due_acquires_scheduler_lease_before_returning_auto_task() {
    let now = 2 * DAY_MS + 4 * HOUR_MS;
    let harness = Harness::new(now);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule {
        cadence: BackupCadence::Daily,
        hour: Some(3),
        minute: Some(0),
        weekdays: Vec::new(),
    }));

    let result = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("daily schedule should be due");

    let lease_requests = harness.scheduler_state_repository.lease_requests();
    assert_eq!(lease_requests.len(), 1);
    let lease_request = &lease_requests[0];
    assert_eq!(lease_request.game_id, GameId::mhw());
    assert_eq!(lease_request.profile_id, ProfileId::new("default"));
    assert_eq!(lease_request.now_unix_millis, now);
    assert_eq!(lease_request.last_checked_at, Some(now));
    assert_eq!(lease_request.next_due_at, Some(3 * DAY_MS + 3 * HOUR_MS));
    assert!(lease_request.lease_expires_at > now);

    let task = result.due_task.expect("lease holder returns auto task");
    assert_eq!(
        task.scheduler_lease_owner.as_deref(),
        Some(lease_request.lease_owner.as_str())
    );

    let state = harness
        .scheduler_state_repository
        .latest_state()
        .expect("scheduler state saved");
    assert!(state.enabled);
    assert!(!state.background_protection_enabled);
    assert_eq!(
        state.background_status,
        SaveBackupBackgroundProtectionStatus::TrayOnly
    );
    assert_eq!(state.last_checked_at, Some(now));
    assert_eq!(state.next_due_at, Some(3 * DAY_MS + 3 * HOUR_MS));
    assert_eq!(state.last_success_at, None);
}

#[test]
fn due_schedule_without_scheduler_lease_does_not_return_auto_task() {
    let harness = Harness::new(2 * DAY_MS + 4 * HOUR_MS);
    harness
        .scheduler_state_repository
        .set_lease_available(false);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule {
        cadence: BackupCadence::Daily,
        hour: Some(3),
        minute: Some(0),
        weekdays: Vec::new(),
    }));

    let result = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("daily schedule should be checked");

    assert_eq!(result.status, SaveBackupAutoCheckStatus::Due);
    assert!(result.due_task.is_none());
    assert_eq!(harness.scheduler_state_repository.lease_requests().len(), 1);
}

#[test]
fn due_schedule_with_running_game_defers_without_lease_or_task() {
    let harness = Harness::new(2 * DAY_MS + 4 * HOUR_MS);
    harness
        .game_running_detector
        .set_status(GameRunningStatus::Running);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule {
        cadence: BackupCadence::Daily,
        hour: Some(3),
        minute: Some(0),
        weekdays: Vec::new(),
    }));

    let result = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("daily schedule should be checked");

    assert_eq!(result.status, SaveBackupAutoCheckStatus::Due);
    assert_eq!(
        result.pending_reason,
        Some(SaveBackupSchedulerPendingReason::GameRunning)
    );
    assert!(result.due_task.is_none());
    assert!(harness
        .scheduler_state_repository
        .lease_requests()
        .is_empty());

    let state = harness
        .scheduler_state_repository
        .latest_state()
        .expect("scheduler state saved");
    assert_eq!(
        state.pending_reason,
        Some(SaveBackupSchedulerPendingReason::GameRunning)
    );

    let events = harness.audit_log.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].category, "save_backup");
    assert_eq!(events[0].operation, "auto_backup");
    assert_eq!(events[0].result, "deferred");
    assert_eq!(
        events[0].fields.get("error_code").map(String::as_str),
        Some("save_backup_auto_skipped_game_running")
    );
    assert!(events[0]
        .fields
        .values()
        .all(|value| !value.contains(":\\") && !value.contains('/')));
}

#[test]
fn due_schedule_with_unknown_game_state_defers_conservatively() {
    let harness = Harness::new(2 * DAY_MS + 4 * HOUR_MS);
    harness
        .game_running_detector
        .set_status(GameRunningStatus::Unknown);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule {
        cadence: BackupCadence::Daily,
        hour: Some(3),
        minute: Some(0),
        weekdays: Vec::new(),
    }));

    let result = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("daily schedule should be checked");

    assert_eq!(
        result.pending_reason,
        Some(SaveBackupSchedulerPendingReason::GameRunningUnknown)
    );
    assert!(result.due_task.is_none());
    assert!(harness
        .scheduler_state_repository
        .lease_requests()
        .is_empty());
    assert_eq!(
        harness.audit_log.events()[0]
            .fields
            .get("error_code")
            .map(String::as_str),
        Some("save_backup_auto_skipped_game_running_unknown")
    );
}

#[test]
fn repeated_running_checks_record_deferred_audit_once() {
    let harness = Harness::new(2 * DAY_MS + 4 * HOUR_MS);
    harness
        .game_running_detector
        .set_status(GameRunningStatus::Running);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule {
        cadence: BackupCadence::Daily,
        hour: Some(3),
        minute: Some(0),
        weekdays: Vec::new(),
    }));

    for _ in 0..3 {
        harness
            .scheduler
            .check_profile(SaveBackupAutoCheckRequest {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
            })
            .expect("check succeeds");
    }

    assert_eq!(harness.audit_log.events().len(), 1);
}

#[test]
fn due_task_resumes_after_game_exits() {
    let harness = Harness::new(2 * DAY_MS + 4 * HOUR_MS);
    harness
        .game_running_detector
        .set_status(GameRunningStatus::Running);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule {
        cadence: BackupCadence::Daily,
        hour: Some(3),
        minute: Some(0),
        weekdays: Vec::new(),
    }));

    let deferred = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("deferred check succeeds");
    assert!(deferred.due_task.is_none());

    harness
        .game_running_detector
        .set_status(GameRunningStatus::NotRunning);
    let resumed = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("resumed check succeeds");

    assert_eq!(resumed.pending_reason, None);
    let task = resumed.due_task.expect("game exit resumes auto task");
    assert_eq!(task.trigger, SaveBackupTrigger::Auto);

    let state = harness
        .scheduler_state_repository
        .latest_state()
        .expect("scheduler state saved");
    assert_eq!(state.pending_reason, None);
}

#[test]
fn daily_schedule_not_due_after_auto_backup_in_current_slot() {
    let harness = Harness::new(2 * DAY_MS + 4 * HOUR_MS);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule {
        cadence: BackupCadence::Daily,
        hour: Some(3),
        minute: Some(0),
        weekdays: Vec::new(),
    }));
    harness.insert_backup(auto_summary("auto-current", 2 * DAY_MS + 3 * HOUR_MS + 1));

    let result = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("daily schedule should be checked");

    assert_eq!(result.status, SaveBackupAutoCheckStatus::NotDue);
    assert!(result.due_task.is_none());
    assert_eq!(result.next_due_at, Some(3 * DAY_MS + 3 * HOUR_MS));
}

#[test]
fn manual_schedule_records_disabled_scheduler_state() {
    let now = DAY_MS + 4 * HOUR_MS;
    let harness = Harness::new(now);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule::manual()));

    let result = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("manual schedule should be checked");

    assert_eq!(result.status, SaveBackupAutoCheckStatus::ManualOnly);
    assert!(result.due_task.is_none());
    assert!(harness
        .scheduler_state_repository
        .lease_requests()
        .is_empty());

    let state = harness
        .scheduler_state_repository
        .latest_state()
        .expect("disabled scheduler state saved");
    assert_eq!(state.game_id, GameId::mhw());
    assert_eq!(state.profile_id, ProfileId::new("default"));
    assert!(!state.enabled);
    assert!(!state.background_protection_enabled);
    assert_eq!(
        state.background_status,
        SaveBackupBackgroundProtectionStatus::NotEnabled
    );
    assert_eq!(state.last_checked_at, Some(now));
    assert_eq!(state.next_due_at, None);
}

#[test]
fn switching_to_manual_preserves_in_flight_scheduler_lease() {
    let now = DAY_MS + 4 * HOUR_MS;
    let harness = Harness::new(now);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule::manual()));
    harness
        .scheduler_state_repository
        .upsert_state(&SaveBackupSchedulerState {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            enabled: true,
            background_protection_enabled: false,
            background_status: SaveBackupBackgroundProtectionStatus::TrayOnly,
            last_checked_at: Some(now - HOUR_MS),
            last_attempt_at: Some(now - HOUR_MS),
            last_success_at: None,
            next_due_at: Some(now + DAY_MS),
            pending_reason: None,
            last_error_code: None,
            worker_instance_id: None,
            worker_heartbeat_at: Some(now - HOUR_MS),
            lease_owner: Some("client-runtime:in-flight".to_owned()),
            lease_expires_at: Some(now + HOUR_MS),
            updated_at: now - HOUR_MS,
        })
        .expect("in-flight lease state seeded");

    let result = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("manual schedule should be checked");

    assert_eq!(result.status, SaveBackupAutoCheckStatus::ManualOnly);
    let state = harness
        .scheduler_state_repository
        .latest_state()
        .expect("disabled scheduler state saved");
    assert!(!state.enabled);
    // 在途租约必须保留，只允许 release_lease 按 owner 清理，
    // 否则重新启用后会并发启动第二个 auto 备份任务。
    assert_eq!(
        state.lease_owner.as_deref(),
        Some("client-runtime:in-flight")
    );
    assert_eq!(state.lease_expires_at, Some(now + HOUR_MS));
    assert_eq!(state.worker_heartbeat_at, Some(now - HOUR_MS));
}

#[test]
fn weekly_schedule_uses_latest_elapsed_weekday_slot() {
    let wednesday_day = 6;
    let harness = Harness::new(wednesday_day * DAY_MS + 4 * HOUR_MS);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule {
        cadence: BackupCadence::Weekly,
        hour: Some(3),
        minute: Some(0),
        weekdays: vec![1, 3],
    }));
    harness.insert_backup(auto_summary("auto-before-slot", wednesday_day * DAY_MS));

    let result = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("weekly schedule should be due");

    assert_eq!(result.status, SaveBackupAutoCheckStatus::Due);
    assert_eq!(
        result.last_due_at,
        Some(wednesday_day * DAY_MS + 3 * HOUR_MS)
    );
    assert_eq!(result.next_due_at, Some(11 * DAY_MS + 3 * HOUR_MS));
    assert_eq!(
        result
            .due_task
            .expect("due weekly schedule returns task")
            .trigger,
        SaveBackupTrigger::Auto
    );
}

#[test]
fn background_status_returns_saved_state_or_none() {
    let harness = Harness::new(DAY_MS);

    let missing = harness
        .scheduler
        .background_status(&GameId::mhw(), &ProfileId::new("default"))
        .expect("missing state is not an error");
    assert_eq!(missing, None);

    let state = SaveBackupSchedulerState {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        enabled: true,
        background_protection_enabled: false,
        background_status: SaveBackupBackgroundProtectionStatus::TrayOnly,
        last_checked_at: Some(DAY_MS),
        last_attempt_at: None,
        last_success_at: None,
        next_due_at: Some(2 * DAY_MS),
        pending_reason: None,
        last_error_code: None,
        worker_instance_id: None,
        worker_heartbeat_at: None,
        lease_owner: None,
        lease_expires_at: None,
        updated_at: DAY_MS,
    };
    harness
        .scheduler_state_repository
        .upsert_state(&state)
        .expect("state seeded");

    let loaded = harness
        .scheduler
        .background_status(&GameId::mhw(), &ProfileId::new("default"))
        .expect("saved state can be read");
    assert_eq!(loaded, Some(state));
}

#[test]
fn background_status_maps_repository_failure_to_stable_error_code() {
    let harness = Harness::new(DAY_MS);
    harness.scheduler_state_repository.set_fail_get_state(true);

    let error = harness
        .scheduler
        .background_status(&GameId::mhw(), &ProfileId::new("default"))
        .expect_err("repository failure surfaces as scheduler error");
    assert_eq!(error.code(), "save_backup_scheduler_unavailable");
}

struct Harness {
    scheduler: SaveBackupAutoSchedulerService,
    profile_repository: Arc<FakeProfileRepository>,
    settings_repository: Arc<FakeProfileSaveSettingsRepository>,
    backup_repository: Arc<FakeSaveBackupRepository>,
    scheduler_state_repository: Arc<FakeSaveBackupSchedulerStateRepository>,
    game_running_detector: Arc<FakeGameRunningDetector>,
    audit_log: Arc<RecordingAuditLogWriter>,
}

impl Harness {
    fn new(now_unix_millis: u128) -> Self {
        let profile_repository = Arc::new(FakeProfileRepository::default());
        let settings_repository = Arc::new(FakeProfileSaveSettingsRepository::default());
        let backup_repository = Arc::new(FakeSaveBackupRepository::default());
        let scheduler_state_repository =
            Arc::new(FakeSaveBackupSchedulerStateRepository::default());
        let game_running_detector = Arc::new(FakeGameRunningDetector::default());
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let scheduler = SaveBackupAutoSchedulerService::new(
            profile_repository.clone(),
            settings_repository.clone(),
            backup_repository.clone(),
            scheduler_state_repository.clone(),
            game_running_detector.clone(),
            audit_log.clone(),
            Arc::new(FixedClock { now_unix_millis }),
        );

        Self {
            scheduler,
            profile_repository,
            settings_repository,
            backup_repository,
            scheduler_state_repository,
            game_running_detector,
            audit_log,
        }
    }

    fn insert_profile(&self, profile_id: &str) {
        self.profile_repository
            .save(&Profile {
                id: profile_id.to_owned(),
                name: "Profile".to_owned(),
                description: None,
                is_active: profile_id == "default",
                created_at: 1,
                updated_at: 1,
            })
            .expect("profile saved");
    }

    fn insert_settings(&self, settings: ProfileSaveSettings) {
        self.settings_repository
            .save_settings(&settings)
            .expect("settings saved");
    }

    fn insert_backup(&self, summary: SaveBackupSummary) {
        self.backup_repository.save(&summary).expect("backup saved");
    }
}

#[derive(Default)]
struct FakeProfileRepository {
    profiles: Mutex<Vec<Profile>>,
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
        let mut profiles = self.profiles.lock().unwrap();
        for profile in profiles.iter_mut() {
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
    settings: Mutex<Vec<ProfileSaveSettings>>,
}

impl ProfileSaveSettingsRepository for FakeProfileSaveSettingsRepository {
    fn get_settings(&self, profile_id: &str) -> Result<Option<ProfileSaveSettings>> {
        Ok(self
            .settings
            .lock()
            .unwrap()
            .iter()
            .find(|settings| settings.profile_id == profile_id)
            .cloned())
    }

    fn save_settings(&self, settings: &ProfileSaveSettings) -> Result<()> {
        let mut all_settings = self.settings.lock().unwrap();
        all_settings.retain(|existing| existing.profile_id != settings.profile_id);
        all_settings.push(settings.clone());
        Ok(())
    }
}

#[derive(Default)]
struct FakeSaveBackupRepository {
    saved: Mutex<Vec<SaveBackupSummary>>,
}

impl SaveBackupRepository for FakeSaveBackupRepository {
    fn save(&self, summary: &SaveBackupSummary) -> Result<()> {
        self.saved.lock().unwrap().push(summary.clone());
        Ok(())
    }

    fn list_for_profile(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        _limit: Option<usize>,
    ) -> Result<Vec<SaveBackupSummary>> {
        Ok(self
            .saved
            .lock()
            .unwrap()
            .iter()
            .filter(|summary| &summary.game_id == game_id && &summary.profile_id == profile_id)
            .cloned()
            .collect())
    }

    fn mark_status(&self, backup_id: &str, status: SaveBackupStatus) -> Result<()> {
        for summary in self.saved.lock().unwrap().iter_mut() {
            if summary.backup_id == backup_id {
                summary.status = status;
            }
        }
        Ok(())
    }
}

struct FakeSaveBackupSchedulerStateRepository {
    states: Mutex<Vec<SaveBackupSchedulerState>>,
    lease_requests: Mutex<Vec<SaveBackupSchedulerLeaseRequest>>,
    lease_available: Mutex<bool>,
    fail_get_state: Mutex<bool>,
}

impl Default for FakeSaveBackupSchedulerStateRepository {
    fn default() -> Self {
        Self {
            states: Mutex::new(Vec::new()),
            lease_requests: Mutex::new(Vec::new()),
            lease_available: Mutex::new(true),
            fail_get_state: Mutex::new(false),
        }
    }
}

impl FakeSaveBackupSchedulerStateRepository {
    fn latest_state(&self) -> Option<SaveBackupSchedulerState> {
        self.states.lock().unwrap().last().cloned()
    }

    fn lease_requests(&self) -> Vec<SaveBackupSchedulerLeaseRequest> {
        self.lease_requests.lock().unwrap().clone()
    }

    fn set_lease_available(&self, available: bool) {
        *self.lease_available.lock().unwrap() = available;
    }

    fn set_fail_get_state(&self, fail: bool) {
        *self.fail_get_state.lock().unwrap() = fail;
    }

    fn replace_state(&self, state: SaveBackupSchedulerState) {
        let mut states = self.states.lock().unwrap();
        states.retain(|existing| {
            existing.game_id != state.game_id || existing.profile_id != state.profile_id
        });
        states.push(state);
    }
}

impl SaveBackupSchedulerStateRepository for FakeSaveBackupSchedulerStateRepository {
    fn get_state(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<Option<SaveBackupSchedulerState>> {
        if *self.fail_get_state.lock().unwrap() {
            anyhow::bail!("scheduler state unavailable");
        }
        Ok(self
            .states
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|state| &state.game_id == game_id && &state.profile_id == profile_id)
            .cloned())
    }

    fn upsert_state(&self, state: &SaveBackupSchedulerState) -> Result<()> {
        self.replace_state(state.clone());
        Ok(())
    }

    fn acquire_due_lease(
        &self,
        request: SaveBackupSchedulerLeaseRequest,
    ) -> Result<Option<SaveBackupSchedulerState>> {
        self.lease_requests.lock().unwrap().push(request.clone());
        if !*self.lease_available.lock().unwrap() {
            return Ok(None);
        }

        let Some(mut state) = self.get_state(&request.game_id, &request.profile_id)? else {
            return Ok(None);
        };
        state.lease_owner = Some(request.lease_owner);
        state.lease_expires_at = Some(request.lease_expires_at);
        state.last_checked_at = request.last_checked_at;
        state.next_due_at = request.next_due_at;
        state.updated_at = request.now_unix_millis;
        self.replace_state(state.clone());
        Ok(Some(state))
    }

    fn release_lease(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        lease_owner: &str,
    ) -> Result<()> {
        if let Some(mut state) = self.get_state(game_id, profile_id)? {
            if state.lease_owner.as_deref() == Some(lease_owner) {
                state.lease_owner = None;
                state.lease_expires_at = None;
                self.replace_state(state);
            }
        }
        Ok(())
    }

    fn record_worker_heartbeat(&self, _heartbeat: SaveBackupWorkerHeartbeat) -> Result<()> {
        Ok(())
    }
}

struct FixedClock {
    now_unix_millis: u128,
}

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(self.now_unix_millis)
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
    fn set_status(&self, status: GameRunningStatus) {
        *self.status.lock().unwrap() = status;
    }
}

impl GameRunningDetector for FakeGameRunningDetector {
    fn game_running_status(&self, _game_id: &GameId) -> GameRunningStatus {
        *self.status.lock().unwrap()
    }
}

#[derive(Default)]
struct RecordingAuditLogWriter {
    events: Mutex<Vec<AuditLogEvent>>,
}

impl RecordingAuditLogWriter {
    fn events(&self) -> Vec<AuditLogEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl AuditLogWriter for RecordingAuditLogWriter {
    fn record(&self, event: AuditLogEvent) -> Result<()> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }
}

fn settings_with_schedule(schedule: ProfileBackupSchedule) -> ProfileSaveSettings {
    ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: directory_selection("C:/Users/Test/Saves"),
        backup_directory: directory_selection("D:/HMM/Backups"),
        schedule,
        retention: ProfileBackupRetention::default(),
        updated_at: 10,
    }
}

fn auto_summary(backup_id: &str, created_at: u128) -> SaveBackupSummary {
    SaveBackupSummary {
        backup_id: backup_id.to_owned(),
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        trigger: SaveBackupTrigger::Auto,
        status: SaveBackupStatus::Completed,
        archive_file_name: format!("{backup_id}.zip"),
        manifest_file_name: format!("{backup_id}.manifest.json"),
        archive_size_bytes: 128,
        archive_sha256: "sha256:test".to_owned(),
        file_count: 1,
        created_at,
        source_path_label: Some("remote".to_owned()),
        source_path_hash: "sha256:source".to_owned(),
        backup_directory: directory_selection("D:/HMM/Backups"),
        notes: None,
    }
}

fn directory_selection(directory: &str) -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Custom,
        status: ProfileDirectoryStatus::Valid,
        directory: Some(directory.to_owned()),
        path_label: Some("Saves".to_owned()),
        messages: Vec::new(),
    }
}
