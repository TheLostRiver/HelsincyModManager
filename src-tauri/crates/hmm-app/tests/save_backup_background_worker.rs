use anyhow::Result;
use hmm_app::{
    CreateSaveBackupRequest, CreateSaveBackupResult, SaveBackupAutoCheckRequest,
    SaveBackupAutoSchedulerService, SaveBackupBackgroundWorker, SaveBackupExecutor,
    SaveBackupTaskRunner, SaveBackupTaskScopeRegistry, SaveBackupTaskService,
    StartSaveBackupTaskRequest, TaskManager,
};
use hmm_core::{
    BackupCadence, GameId, Profile, ProfileBackupRetention, ProfileBackupSchedule,
    ProfileDirectoryMode, ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId,
    ProfileSaveSettings, SaveBackupBackgroundProtectionStatus, SaveBackupSchedulerLeaseRequest,
    SaveBackupSchedulerState, SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
    SaveBackupWorkerHeartbeat,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, GameRunningDetector, GameRunningStatus,
    ProfileRepository, ProfileSaveSettingsRepository, SaveBackupRepository,
    SaveBackupSchedulerStateRepository,
};
use std::sync::{Arc, Mutex};

const DAY_MS: u128 = 86_400_000;
const HOUR_MS: u128 = 3_600_000;
const NOW: u128 = 2 * DAY_MS + 4 * HOUR_MS;

#[test]
fn due_auto_profile_starts_auto_task_and_records_tray_only_heartbeat() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(daily_settings("default"));

    let summary = harness.worker().run_once("worker-a").expect("worker runs");

    assert_eq!(summary.checked_profiles, 1);
    assert_eq!(summary.started_tasks, 1);
    assert_eq!(summary.deferred_profiles, 0);
    assert_eq!(summary.failed_profiles, 0);
    assert_eq!(harness.executor.triggers(), vec![SaveBackupTrigger::Auto]);
    assert_eq!(
        harness
            .scheduler_state_repository
            .heartbeats()
            .into_iter()
            .map(|heartbeat| heartbeat.status)
            .collect::<Vec<_>>(),
        vec![SaveBackupBackgroundProtectionStatus::TrayOnly]
    );

    let state = harness
        .scheduler_state_repository
        .state(&GameId::mhw(), &ProfileId::new("default"))
        .expect("worker heartbeat keeps scheduler state");
    assert!(!state.background_protection_enabled);
    assert_eq!(
        state.background_status,
        SaveBackupBackgroundProtectionStatus::TrayOnly
    );
}

#[test]
fn running_and_unknown_profiles_are_deferred_without_task_or_second_lease() {
    for status in [GameRunningStatus::Running, GameRunningStatus::Unknown] {
        let harness = Harness::new();
        harness.insert_profile("default");
        harness.insert_settings(daily_settings("default"));
        harness.game_running_detector.set_status(status);

        let summary = harness.worker().run_once("worker-a").expect("worker runs");

        assert_eq!(summary.checked_profiles, 1);
        assert_eq!(summary.started_tasks, 0);
        assert_eq!(summary.deferred_profiles, 1);
        assert_eq!(summary.failed_profiles, 0);
        assert!(harness
            .scheduler_state_repository
            .lease_requests()
            .is_empty());
        assert!(harness.executor.triggers().is_empty());
        assert_eq!(harness.scheduler_state_repository.heartbeats().len(), 1);
    }
}

#[test]
fn manual_and_missing_settings_profiles_are_skipped() {
    let harness = Harness::new();
    harness.insert_profile("manual");
    harness.insert_profile("missing-settings");
    harness.insert_settings(manual_settings("manual"));

    let summary = harness.worker().run_once("worker-a").expect("worker runs");

    assert_eq!(summary.checked_profiles, 0);
    assert_eq!(summary.started_tasks, 0);
    assert_eq!(summary.deferred_profiles, 0);
    assert_eq!(summary.failed_profiles, 0);
    assert!(harness
        .scheduler_state_repository
        .lease_requests()
        .is_empty());
    assert!(harness.scheduler_state_repository.heartbeats().is_empty());
    assert!(harness.executor.triggers().is_empty());
}

#[test]
fn settings_failure_is_audited_and_does_not_stop_next_due_profile() {
    let harness = Harness::new();
    harness.insert_profile("broken");
    harness.insert_profile("due");
    harness.settings_repository.fail_for("broken");
    harness.insert_settings(daily_settings("due"));

    let summary = harness
        .worker()
        .run_once("worker-a")
        .expect("per-profile failure is isolated");

    assert_eq!(summary.checked_profiles, 1);
    assert_eq!(summary.started_tasks, 1);
    assert_eq!(summary.deferred_profiles, 0);
    assert_eq!(summary.failed_profiles, 1);
    assert_eq!(harness.executor.triggers(), vec![SaveBackupTrigger::Auto]);

    let event = harness
        .audit_log
        .events()
        .into_iter()
        .find(|event| event.operation == "background_worker")
        .expect("worker failure is audited");
    assert_eq!(event.category, "save_backup");
    assert_eq!(event.result, "failure");
    assert_eq!(
        event.fields.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["error_code", "game_id", "profile_id", "trigger"]
    );
    assert_eq!(event.fields["game_id"], "mhw");
    assert_eq!(event.fields["profile_id"], "broken");
    assert_eq!(event.fields["trigger"], "auto");
    assert_eq!(
        event.fields["error_code"],
        "save_backup_auto_settings_unavailable"
    );
}

#[test]
fn task_reservation_failure_releases_the_scheduler_lease() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(daily_settings("default"));
    harness
        .task_service
        .start_save_backup_task(StartSaveBackupTaskRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            trigger: SaveBackupTrigger::Manual,
            note: None,
            scheduler_lease_owner: None,
        })
        .expect("client task reserves the profile scope");

    let summary = harness
        .worker()
        .run_once("worker-a")
        .expect("reservation failure is isolated");

    assert_eq!(summary.checked_profiles, 0);
    assert_eq!(summary.started_tasks, 0);
    assert_eq!(summary.deferred_profiles, 0);
    assert_eq!(summary.failed_profiles, 1);
    assert_eq!(harness.scheduler_state_repository.release_calls().len(), 1);
    let state = harness
        .scheduler_state_repository
        .state(&GameId::mhw(), &ProfileId::new("default"))
        .expect("scheduler state exists");
    assert_eq!(state.lease_owner, None);
    assert_eq!(state.lease_expires_at, None);

    let event = harness
        .audit_log
        .events()
        .into_iter()
        .find(|event| event.operation == "background_worker")
        .expect("task reservation failure is audited");
    assert_eq!(
        event.fields["error_code"],
        "save_backup_background_task_start_failed"
    );
}

#[test]
fn heartbeat_failure_releases_due_lease_without_starting_a_task() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(daily_settings("default"));
    harness.scheduler_state_repository.set_fail_heartbeat(true);

    let summary = harness
        .worker()
        .run_once("worker-a")
        .expect("heartbeat failure is isolated");

    assert_eq!(summary.started_tasks, 0);
    assert_eq!(summary.failed_profiles, 1);
    assert!(harness.executor.triggers().is_empty());

    let lease_owner = harness
        .scheduler_state_repository
        .lease_requests()
        .into_iter()
        .next()
        .expect("due check acquired a lease")
        .lease_owner;
    assert_eq!(
        harness.scheduler_state_repository.release_calls(),
        vec![lease_owner]
    );

    let state = harness
        .scheduler_state_repository
        .state(&GameId::mhw(), &ProfileId::new("default"))
        .expect("scheduler state exists");
    assert_eq!(state.lease_owner, None);
    assert_eq!(state.lease_expires_at, None);

    let event = harness
        .audit_log
        .events()
        .into_iter()
        .find(|event| event.operation == "background_worker")
        .expect("heartbeat failure is audited");
    assert_eq!(event.category, "save_backup");
    assert_eq!(event.result, "failure");
    assert_eq!(
        event.fields.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["error_code", "game_id", "profile_id", "trigger"]
    );
    assert_eq!(
        event.fields["error_code"],
        "save_backup_scheduler_unavailable"
    );
}

#[test]
fn client_due_lease_prevents_a_second_worker_from_starting_the_same_backup() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(daily_settings("default"));

    let client_check = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("client acquires the due lease");
    let client_lease_owner = client_check
        .due_task
        .and_then(|request| request.scheduler_lease_owner)
        .expect("client owns the due lease");

    let summary = harness
        .worker()
        .run_once("worker-a")
        .expect("worker checks the profile");

    assert_eq!(summary.checked_profiles, 1);
    assert_eq!(summary.started_tasks, 0);
    assert_eq!(summary.deferred_profiles, 0);
    assert_eq!(summary.failed_profiles, 0);
    assert_eq!(harness.scheduler_state_repository.lease_requests().len(), 2);
    assert!(harness.executor.triggers().is_empty());
    let state = harness
        .scheduler_state_repository
        .state(&GameId::mhw(), &ProfileId::new("default"))
        .expect("client lease remains active");
    assert_eq!(
        state.lease_owner.as_deref(),
        Some(client_lease_owner.as_str())
    );
}

#[test]
fn runner_failure_is_isolated_and_the_next_due_profile_still_runs() {
    let harness = Harness::new();
    harness.insert_profile("failing-runner");
    harness.insert_profile("due");
    harness.insert_settings(daily_settings("failing-runner"));
    harness.insert_settings(daily_settings("due"));
    harness.executor.fail_for("failing-runner");

    let summary = harness
        .worker()
        .run_once("worker-a")
        .expect("runner failure is isolated");

    assert_eq!(summary.checked_profiles, 1);
    assert_eq!(summary.started_tasks, 1);
    assert_eq!(summary.deferred_profiles, 0);
    assert_eq!(summary.failed_profiles, 1);
    assert_eq!(
        harness.executor.profile_ids(),
        vec!["failing-runner".to_owned(), "due".to_owned()]
    );
    assert!(harness.audit_log.events().iter().any(|event| {
        event.operation == "background_worker"
            && event.fields.get("error_code").map(String::as_str)
                == Some("save_backup_background_task_run_failed")
    }));
}

#[test]
fn list_and_worker_clock_failures_are_the_only_top_level_errors() {
    let list_failure = Harness::new();
    list_failure.profile_repository.set_fail_list(true);

    let list_error = list_failure
        .worker()
        .run_once("worker-a")
        .expect_err("profile listing failure is top-level");
    assert_eq!(
        list_error.code(),
        "save_backup_background_profile_list_unavailable"
    );

    let clock_failure = Harness::new();
    let clock_error = clock_failure
        .worker_with_clock(Arc::new(FixedClock::failing()))
        .run_once("worker-a")
        .expect_err("worker clock failure is top-level");
    assert_eq!(
        clock_error.code(),
        "save_backup_background_clock_unavailable"
    );
}

struct Harness {
    profile_repository: Arc<FakeProfileRepository>,
    settings_repository: Arc<FakeProfileSaveSettingsRepository>,
    scheduler_state_repository: Arc<FakeSaveBackupSchedulerStateRepository>,
    game_running_detector: Arc<FakeGameRunningDetector>,
    audit_log: Arc<RecordingAuditLogWriter>,
    executor: Arc<RecordingSaveBackupExecutor>,
    scheduler: Arc<SaveBackupAutoSchedulerService>,
    task_service: Arc<SaveBackupTaskService>,
    task_runner: Arc<SaveBackupTaskRunner>,
    worker_clock: Arc<FixedClock>,
}

impl Harness {
    fn new() -> Self {
        let profile_repository = Arc::new(FakeProfileRepository::default());
        let settings_repository = Arc::new(FakeProfileSaveSettingsRepository::default());
        let backup_repository = Arc::new(FakeSaveBackupRepository::default());
        let scheduler_state_repository =
            Arc::new(FakeSaveBackupSchedulerStateRepository::default());
        let game_running_detector = Arc::new(FakeGameRunningDetector::default());
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let scheduler_clock = Arc::new(FixedClock::available(NOW));
        let worker_clock = Arc::new(FixedClock::available(NOW));
        let scheduler = Arc::new(SaveBackupAutoSchedulerService::new(
            profile_repository.clone(),
            settings_repository.clone(),
            backup_repository,
            scheduler_state_repository.clone(),
            game_running_detector.clone(),
            audit_log.clone(),
            scheduler_clock.clone(),
        ));
        let executor = Arc::new(RecordingSaveBackupExecutor::default());
        let task_manager = Arc::new(TaskManager::new());
        let task_scope_registry = Arc::new(SaveBackupTaskScopeRegistry::default());
        let task_service = Arc::new(SaveBackupTaskService::with_scope_registry(
            task_manager.clone(),
            task_scope_registry.clone(),
        ));
        let task_runner = Arc::new(
            SaveBackupTaskRunner::with_scope_registry_and_scheduler_state(
                task_manager,
                executor.clone(),
                audit_log.clone(),
                scheduler_clock,
                task_scope_registry,
                scheduler_state_repository.clone(),
            ),
        );

        Self {
            profile_repository,
            settings_repository,
            scheduler_state_repository,
            game_running_detector,
            audit_log,
            executor,
            scheduler,
            task_service,
            task_runner,
            worker_clock,
        }
    }

    fn worker(&self) -> SaveBackupBackgroundWorker {
        self.worker_with_clock(self.worker_clock.clone())
    }

    fn worker_with_clock(&self, clock: Arc<dyn AppClock>) -> SaveBackupBackgroundWorker {
        SaveBackupBackgroundWorker::new(
            vec![GameId::mhw()],
            self.profile_repository.clone(),
            self.settings_repository.clone(),
            self.scheduler.clone(),
            self.task_service.clone(),
            self.task_runner.clone(),
            self.scheduler_state_repository.clone(),
            self.audit_log.clone(),
            clock,
        )
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
}

#[derive(Default)]
struct FakeProfileRepository {
    profiles: Mutex<Vec<Profile>>,
    fail_list: Mutex<bool>,
}

impl FakeProfileRepository {
    fn set_fail_list(&self, fail: bool) {
        *self.fail_list.lock().expect("profile list lock") = fail;
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

    fn save(&self, profile: &Profile) -> Result<()> {
        let mut profiles = self.profiles.lock().expect("profiles lock");
        profiles.retain(|existing| existing.id != profile.id);
        profiles.push(profile.clone());
        Ok(())
    }

    fn delete(&self, profile_id: &str) -> Result<()> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .retain(|profile| profile.id != profile_id);
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<Profile>> {
        if *self.fail_list.lock().expect("profile list lock") {
            anyhow::bail!("profile list unavailable");
        }
        Ok(self.profiles.lock().expect("profiles lock").clone())
    }

    fn get_active(&self) -> Result<Option<Profile>> {
        Ok(self
            .profiles
            .lock()
            .expect("profiles lock")
            .iter()
            .find(|profile| profile.is_active)
            .cloned())
    }

    fn set_active(&self, profile_id: &str, updated_at: u128) -> Result<()> {
        for profile in self.profiles.lock().expect("profiles lock").iter_mut() {
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
    failing_profile_ids: Mutex<Vec<String>>,
}

impl FakeProfileSaveSettingsRepository {
    fn fail_for(&self, profile_id: &str) {
        self.failing_profile_ids
            .lock()
            .expect("failing settings lock")
            .push(profile_id.to_owned());
    }
}

impl ProfileSaveSettingsRepository for FakeProfileSaveSettingsRepository {
    fn get_settings(&self, profile_id: &str) -> Result<Option<ProfileSaveSettings>> {
        if self
            .failing_profile_ids
            .lock()
            .expect("failing settings lock")
            .iter()
            .any(|failing_profile_id| failing_profile_id == profile_id)
        {
            anyhow::bail!("settings unavailable");
        }

        Ok(self
            .settings
            .lock()
            .expect("settings lock")
            .iter()
            .find(|settings| settings.profile_id == profile_id)
            .cloned())
    }

    fn save_settings(&self, settings: &ProfileSaveSettings) -> Result<()> {
        let mut all_settings = self.settings.lock().expect("settings lock");
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
        self.saved
            .lock()
            .expect("backups lock")
            .push(summary.clone());
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
            .expect("backups lock")
            .iter()
            .filter(|summary| &summary.game_id == game_id && &summary.profile_id == profile_id)
            .cloned()
            .collect())
    }

    fn mark_status(&self, backup_id: &str, status: SaveBackupStatus) -> Result<()> {
        for summary in self.saved.lock().expect("backups lock").iter_mut() {
            if summary.backup_id == backup_id {
                summary.status = status;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct FakeSaveBackupSchedulerStateRepository {
    states: Mutex<Vec<SaveBackupSchedulerState>>,
    lease_requests: Mutex<Vec<SaveBackupSchedulerLeaseRequest>>,
    release_calls: Mutex<Vec<String>>,
    heartbeats: Mutex<Vec<SaveBackupWorkerHeartbeat>>,
    fail_heartbeat: Mutex<bool>,
}

impl FakeSaveBackupSchedulerStateRepository {
    fn state(&self, game_id: &GameId, profile_id: &ProfileId) -> Option<SaveBackupSchedulerState> {
        self.states
            .lock()
            .expect("scheduler state lock")
            .iter()
            .rev()
            .find(|state| &state.game_id == game_id && &state.profile_id == profile_id)
            .cloned()
    }

    fn lease_requests(&self) -> Vec<SaveBackupSchedulerLeaseRequest> {
        self.lease_requests
            .lock()
            .expect("lease requests lock")
            .clone()
    }

    fn release_calls(&self) -> Vec<String> {
        self.release_calls
            .lock()
            .expect("release calls lock")
            .clone()
    }

    fn heartbeats(&self) -> Vec<SaveBackupWorkerHeartbeat> {
        self.heartbeats.lock().expect("heartbeats lock").clone()
    }

    fn set_fail_heartbeat(&self, fail: bool) {
        *self.fail_heartbeat.lock().expect("heartbeat failure lock") = fail;
    }
}

impl SaveBackupSchedulerStateRepository for FakeSaveBackupSchedulerStateRepository {
    fn get_state(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<Option<SaveBackupSchedulerState>> {
        Ok(self.state(game_id, profile_id))
    }

    fn upsert_state(&self, state: &SaveBackupSchedulerState) -> Result<()> {
        let mut states = self.states.lock().expect("scheduler state lock");
        states.retain(|existing| {
            existing.game_id != state.game_id || existing.profile_id != state.profile_id
        });
        states.push(state.clone());
        Ok(())
    }

    fn acquire_due_lease(
        &self,
        request: SaveBackupSchedulerLeaseRequest,
    ) -> Result<Option<SaveBackupSchedulerState>> {
        self.lease_requests
            .lock()
            .expect("lease requests lock")
            .push(request.clone());

        let mut states = self.states.lock().expect("scheduler state lock");
        let Some(state) = states.iter_mut().rev().find(|state| {
            state.game_id == request.game_id && state.profile_id == request.profile_id
        }) else {
            return Ok(None);
        };

        if state
            .lease_expires_at
            .is_some_and(|expires_at| expires_at > request.now_unix_millis)
        {
            return Ok(None);
        }

        state.lease_owner = Some(request.lease_owner);
        state.lease_expires_at = Some(request.lease_expires_at);
        state.last_checked_at = request.last_checked_at;
        state.next_due_at = request.next_due_at;
        state.updated_at = request.now_unix_millis;
        Ok(Some(state.clone()))
    }

    fn release_lease(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        lease_owner: &str,
    ) -> Result<()> {
        if let Some(state) = self
            .states
            .lock()
            .expect("scheduler state lock")
            .iter_mut()
            .rev()
            .find(|state| &state.game_id == game_id && &state.profile_id == profile_id)
        {
            if state.lease_owner.as_deref() == Some(lease_owner) {
                state.lease_owner = None;
                state.lease_expires_at = None;
            }
        }
        self.release_calls
            .lock()
            .expect("release calls lock")
            .push(lease_owner.to_owned());
        Ok(())
    }

    fn record_worker_heartbeat(&self, heartbeat: SaveBackupWorkerHeartbeat) -> Result<()> {
        if *self.fail_heartbeat.lock().expect("heartbeat failure lock") {
            anyhow::bail!("heartbeat write unavailable");
        }

        if let Some(state) = self
            .states
            .lock()
            .expect("scheduler state lock")
            .iter_mut()
            .rev()
            .find(|state| {
                state.game_id == heartbeat.game_id && state.profile_id == heartbeat.profile_id
            })
        {
            state.worker_instance_id = Some(heartbeat.worker_instance_id.clone());
            state.last_checked_at = Some(heartbeat.checked_at);
            state.background_status = heartbeat.status;
            state.updated_at = heartbeat.checked_at;
        }
        self.heartbeats
            .lock()
            .expect("heartbeats lock")
            .push(heartbeat);
        Ok(())
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
        *self.status.lock().expect("game status lock") = status;
    }
}

impl GameRunningDetector for FakeGameRunningDetector {
    fn game_running_status(&self, _game_id: &GameId) -> GameRunningStatus {
        *self.status.lock().expect("game status lock")
    }
}

#[derive(Default)]
struct RecordingAuditLogWriter {
    events: Mutex<Vec<AuditLogEvent>>,
}

impl RecordingAuditLogWriter {
    fn events(&self) -> Vec<AuditLogEvent> {
        self.events.lock().expect("audit events lock").clone()
    }
}

impl AuditLogWriter for RecordingAuditLogWriter {
    fn record(&self, event: AuditLogEvent) -> Result<()> {
        self.events.lock().expect("audit events lock").push(event);
        Ok(())
    }
}

#[derive(Default)]
struct RecordingSaveBackupExecutor {
    failing_profile_ids: Mutex<Vec<String>>,
    requests: Mutex<Vec<(CreateSaveBackupRequest, SaveBackupTrigger)>>,
}

impl RecordingSaveBackupExecutor {
    fn fail_for(&self, profile_id: &str) {
        self.failing_profile_ids
            .lock()
            .expect("failing executor lock")
            .push(profile_id.to_owned());
    }

    fn triggers(&self) -> Vec<SaveBackupTrigger> {
        self.requests
            .lock()
            .expect("backup requests lock")
            .iter()
            .map(|(_, trigger)| *trigger)
            .collect()
    }

    fn profile_ids(&self) -> Vec<String> {
        self.requests
            .lock()
            .expect("backup requests lock")
            .iter()
            .map(|(request, _)| request.profile_id.as_str().to_owned())
            .collect()
    }
}

impl SaveBackupExecutor for RecordingSaveBackupExecutor {
    fn create_backup(
        &self,
        request: CreateSaveBackupRequest,
        trigger: SaveBackupTrigger,
    ) -> Result<CreateSaveBackupResult, hmm_app::SaveBackupError> {
        self.requests
            .lock()
            .expect("backup requests lock")
            .push((request.clone(), trigger));

        if self
            .failing_profile_ids
            .lock()
            .expect("failing executor lock")
            .iter()
            .any(|profile_id| profile_id == request.profile_id.as_str())
        {
            return Err(hmm_app::SaveBackupError::SourceUnset);
        }

        Ok(CreateSaveBackupResult {
            summary: SaveBackupSummary {
                backup_id: format!("backup-{}", request.profile_id.as_str()),
                game_id: request.game_id,
                profile_id: request.profile_id,
                trigger,
                status: SaveBackupStatus::Completed,
                archive_file_name: "backup.zip".to_owned(),
                manifest_file_name: "backup.manifest.json".to_owned(),
                archive_size_bytes: 1,
                archive_sha256: "sha256:test".to_owned(),
                file_count: 1,
                created_at: NOW,
                source_path_label: None,
                source_path_hash: "sha256:source".to_owned(),
                backup_directory: directory_selection("backup-root"),
                notes: None,
            },
            warnings: Vec::new(),
        })
    }
}

struct FixedClock {
    now_unix_millis: u128,
    fail: bool,
}

impl FixedClock {
    fn available(now_unix_millis: u128) -> Self {
        Self {
            now_unix_millis,
            fail: false,
        }
    }

    fn failing() -> Self {
        Self {
            now_unix_millis: 0,
            fail: true,
        }
    }
}

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        if self.fail {
            anyhow::bail!("clock unavailable");
        }
        Ok(self.now_unix_millis)
    }
}

fn daily_settings(profile_id: &str) -> ProfileSaveSettings {
    settings(profile_id, BackupCadence::Daily)
}

fn manual_settings(profile_id: &str) -> ProfileSaveSettings {
    settings(profile_id, BackupCadence::Manual)
}

fn settings(profile_id: &str, cadence: BackupCadence) -> ProfileSaveSettings {
    ProfileSaveSettings {
        profile_id: profile_id.to_owned(),
        save_directory: directory_selection("save-root"),
        backup_directory: directory_selection("backup-root"),
        schedule: ProfileBackupSchedule {
            cadence,
            hour: Some(3),
            minute: Some(0),
            weekdays: Vec::new(),
        },
        retention: ProfileBackupRetention::default(),
        updated_at: 1,
    }
}

fn directory_selection(directory: &str) -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Default,
        status: ProfileDirectoryStatus::Valid,
        directory: Some(directory.to_owned()),
        path_label: None,
        messages: Vec::new(),
    }
}
