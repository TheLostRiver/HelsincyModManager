use anyhow::Result;
use hmm_app::{
    CreateSaveBackupRequest, CreateSaveBackupResult, SaveBackupError, SaveBackupExecutor,
    SaveBackupTaskRunner, SaveBackupTaskService, SaveBackupWarning, StartSaveBackupTaskRequest,
    TaskKind, TaskManager, TaskManagerError, TaskStatus,
};
use hmm_core::{
    GameId, ProfileId, SaveBackupBackgroundProtectionStatus, SaveBackupSchedulerLeaseRequest,
    SaveBackupSchedulerState, SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
    SaveBackupWorkerHeartbeat,
};
use hmm_ports::{AppClock, AuditLogEvent, AuditLogWriter, SaveBackupSchedulerStateRepository};
use std::sync::{Arc, Mutex};

#[test]
fn start_save_backup_task_returns_queued_save_backup_task() {
    let task_manager = Arc::new(TaskManager::new());
    let service = SaveBackupTaskService::new(Arc::clone(&task_manager));

    let task = service
        .start_save_backup_task(sample_request())
        .expect("save backup task starts");

    assert!(task.task_id.starts_with("save-backup-"));
    assert_eq!(task.kind, TaskKind::SaveBackup);
    assert_eq!(task.status, TaskStatus::Queued);
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(TaskStatus::Queued)
    );
}

#[test]
fn save_backup_task_scope_rejects_duplicate_profile_work_until_runner_finishes() {
    let task_manager = Arc::new(TaskManager::new());
    let scope_registry = Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default());
    let service = SaveBackupTaskService::with_scope_registry(
        Arc::clone(&task_manager),
        Arc::clone(&scope_registry),
    );
    let task = service
        .start_save_backup_task(sample_request())
        .expect("first save backup task starts");

    let duplicate = service
        .start_save_backup_task(sample_request())
        .expect_err("same game/profile save backup task is already active");

    assert_eq!(
        duplicate,
        TaskManagerError::TaskScopeBusy {
            kind: TaskKind::SaveBackup,
            task_id: task.task_id.clone(),
        }
    );

    let runner = SaveBackupTaskRunner::with_scope_registry(
        Arc::clone(&task_manager),
        Arc::new(RecordingSaveBackupExecutor::ok(sample_result())),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        scope_registry,
    );
    runner
        .run_save_backup_task(&task.task_id, sample_request())
        .expect("finished task releases profile scope");

    let next = service
        .start_save_backup_task(sample_request())
        .expect("same profile can start again after previous task finishes");
    assert_ne!(next.task_id, task.task_id);
}

#[test]
fn run_save_backup_task_releases_profile_scope_when_executor_panics() {
    let task_manager = Arc::new(TaskManager::new());
    let scope_registry = Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default());
    let service = SaveBackupTaskService::with_scope_registry(
        Arc::clone(&task_manager),
        Arc::clone(&scope_registry),
    );
    let task = service
        .start_save_backup_task(sample_request())
        .expect("save backup task starts");
    let runner = SaveBackupTaskRunner::with_scope_registry(
        Arc::clone(&task_manager),
        Arc::new(PanickingSaveBackupExecutor),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        scope_registry,
    );

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = runner.run_save_backup_task(&task.task_id, sample_request());
    }));

    assert!(panic.is_err());
    let next = service
        .start_save_backup_task(sample_request())
        .expect("panic still releases profile scope");
    assert_ne!(next.task_id, task.task_id);
}

#[test]
fn run_save_backup_task_records_scheduler_success_and_releases_auto_lease() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let scheduler_state = Arc::new(RecordingSchedulerStateRepository::with_state(
        sample_scheduler_state("auto-lease"),
    ));
    let runner = SaveBackupTaskRunner::with_scope_registry_and_scheduler_state(
        Arc::clone(&task_manager),
        Arc::new(RecordingSaveBackupExecutor::ok(sample_result())),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default()),
        scheduler_state.clone(),
    );

    let events = runner
        .run_save_backup_task(&task.task_id, auto_request("auto-lease"))
        .expect("save backup task succeeds");

    assert_eq!(
        events.last().map(|event| event.phase.as_str()),
        Some("save_backup.completed")
    );
    let state = scheduler_state
        .latest_state()
        .expect("scheduler state should be updated");
    assert_eq!(state.last_attempt_at, Some(42));
    assert_eq!(state.last_success_at, Some(42));
    assert_eq!(state.last_error_code, None);
    assert_eq!(
        scheduler_state.release_calls(),
        vec!["auto-lease".to_owned()]
    );
}

#[test]
fn run_save_backup_task_records_scheduler_failure_and_releases_auto_lease() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let scheduler_state = Arc::new(RecordingSchedulerStateRepository::with_state(
        sample_scheduler_state("auto-lease"),
    ));
    let runner = SaveBackupTaskRunner::with_scope_registry_and_scheduler_state(
        Arc::clone(&task_manager),
        Arc::new(RecordingSaveBackupExecutor::err(
            SaveBackupError::SourceUnset,
        )),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default()),
        scheduler_state.clone(),
    );

    let error = runner
        .run_save_backup_task(&task.task_id, auto_request("auto-lease"))
        .expect_err("save backup task fails");

    assert_eq!(
        error.events.last().and_then(|event| event.error.as_deref()),
        Some("save_backup_failed:save_backup_source_unset")
    );
    let state = scheduler_state
        .latest_state()
        .expect("scheduler state should be updated");
    assert_eq!(state.last_attempt_at, Some(42));
    assert_eq!(
        state.last_error_code.as_deref(),
        Some("save_backup_source_unset")
    );
    assert_eq!(
        scheduler_state.release_calls(),
        vec!["auto-lease".to_owned()]
    );
}

#[test]
fn run_save_backup_task_releases_scheduler_lease_when_executor_panics() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let scheduler_state = Arc::new(RecordingSchedulerStateRepository::with_state(
        sample_scheduler_state("auto-lease"),
    ));
    let runner = SaveBackupTaskRunner::with_scope_registry_and_scheduler_state(
        Arc::clone(&task_manager),
        Arc::new(PanickingSaveBackupExecutor),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default()),
        scheduler_state.clone(),
    );

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = runner.run_save_backup_task(&task.task_id, auto_request("auto-lease"));
    }));

    assert!(panic.is_err());
    assert_eq!(
        scheduler_state.release_calls(),
        vec!["auto-lease".to_owned()]
    );
}

#[test]
fn run_save_backup_task_emits_registered_phases_and_records_success_audit() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let executor = Arc::new(RecordingSaveBackupExecutor::ok(sample_result()));
    let audit_log = Arc::new(RecordingAuditLogWriter::default());
    let runner = SaveBackupTaskRunner::new(
        Arc::clone(&task_manager),
        executor.clone(),
        audit_log.clone(),
        Arc::new(FixedClock),
    );

    let events = runner
        .run_save_backup_task(&task.task_id, sample_request())
        .expect("save backup task succeeds");

    assert_eq!(
        events
            .iter()
            .map(|event| event.phase.as_str())
            .collect::<Vec<_>>(),
        vec![
            "save_backup.scanning",
            "save_backup.archiving",
            "save_backup.manifest_writing",
            "save_backup.retention_pruning",
            "save_backup.completed",
        ]
    );
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(TaskStatus::Completed)
    );
    assert_eq!(
        executor.take_requests()[0].0.note.as_deref(),
        Some("manual note")
    );

    let event = audit_log.take_events().pop().expect("success audit event");
    assert_eq!(event.timestamp_unix_millis, 42);
    assert_eq!(event.category, "save_backup");
    assert_eq!(event.operation, "manual_backup");
    assert_eq!(event.result, "success");
    assert_eq!(event.fields["task_id"], task.task_id);
    assert_eq!(event.fields["game_id"], "mhw");
    assert_eq!(event.fields["profile_id"], "default");
    assert_eq!(event.fields["backup_id"], "backup-1");
    assert_eq!(event.fields["trigger"], "manual");
    assert_eq!(event.fields["file_count"], "1");
    assert_eq!(event.fields["archive_size_bytes"], "128");
    assert!(!serde_json::to_string(&event.fields)
        .expect("serialize audit fields")
        .contains("C:/"));
}

#[test]
fn run_save_backup_task_does_not_replay_running_events_after_concurrent_cancel() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let executor = Arc::new(CancellingSaveBackupExecutor {
        task_manager: Arc::clone(&task_manager),
        task_id: task.task_id.clone(),
    });
    let audit_log = Arc::new(RecordingAuditLogWriter::default());
    let runner = SaveBackupTaskRunner::new(
        Arc::clone(&task_manager),
        executor,
        audit_log.clone(),
        Arc::new(FixedClock),
    );

    let events = runner
        .run_save_backup_task(&task.task_id, sample_request())
        .expect("cancelled task should not be treated as runner failure");

    assert!(events.is_empty());
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(TaskStatus::Cancelled)
    );

    let events = audit_log.take_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].result, "success");
    assert_eq!(events[0].fields["backup_id"], "backup-1");
}

#[test]
fn run_save_backup_task_records_retention_warning_audit_without_failing_task() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let executor = Arc::new(RecordingSaveBackupExecutor::ok(CreateSaveBackupResult {
        summary: sample_summary(),
        warnings: vec![SaveBackupWarning::RetentionFailed],
    }));
    let audit_log = Arc::new(RecordingAuditLogWriter::default());
    let runner = SaveBackupTaskRunner::new(
        Arc::clone(&task_manager),
        executor,
        audit_log.clone(),
        Arc::new(FixedClock),
    );

    let events = runner
        .run_save_backup_task(&task.task_id, sample_request())
        .expect("retention warning should not fail save backup task");

    assert_eq!(
        events.last().map(|event| event.phase.as_str()),
        Some("save_backup.completed")
    );
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(TaskStatus::Completed)
    );

    let events = audit_log.take_events();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].result, "success");
    assert_eq!(events[1].operation, "retention_pruning");
    assert_eq!(events[1].result, "warning");
    assert_eq!(
        events[1].fields["error_code"],
        "save_backup_retention_failed"
    );
}

#[test]
fn run_save_backup_task_records_failure_audit_with_stable_error_code() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let executor = Arc::new(RecordingSaveBackupExecutor::err(
        SaveBackupError::SourceUnset,
    ));
    let audit_log = Arc::new(RecordingAuditLogWriter::default());
    let runner = SaveBackupTaskRunner::new(
        Arc::clone(&task_manager),
        executor,
        audit_log.clone(),
        Arc::new(FixedClock),
    );

    let error = runner
        .run_save_backup_task(&task.task_id, sample_request())
        .expect_err("save backup task fails");

    assert_eq!(
        error
            .events
            .iter()
            .map(|event| (event.phase.as_str(), event.error.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            ("save_backup.scanning", None),
            ("save_backup.archiving", None),
            (
                "save_backup.failed",
                Some("save_backup_failed:save_backup_source_unset")
            ),
        ]
    );
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(TaskStatus::Failed)
    );

    let event = audit_log.take_events().pop().expect("failure audit event");
    assert_eq!(event.result, "failure");
    assert_eq!(event.fields["task_id"], task.task_id);
    assert_eq!(event.fields["error_code"], "save_backup_source_unset");
    assert!(!serde_json::to_string(&event.fields)
        .expect("serialize audit fields")
        .contains("C:/"));
}

fn sample_request() -> StartSaveBackupTaskRequest {
    StartSaveBackupTaskRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        trigger: SaveBackupTrigger::Manual,
        note: Some("manual note".to_owned()),
        scheduler_lease_owner: None,
    }
}

fn auto_request(lease_owner: &str) -> StartSaveBackupTaskRequest {
    StartSaveBackupTaskRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        trigger: SaveBackupTrigger::Auto,
        note: None,
        scheduler_lease_owner: Some(lease_owner.to_owned()),
    }
}

fn sample_scheduler_state(lease_owner: &str) -> SaveBackupSchedulerState {
    SaveBackupSchedulerState {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        enabled: true,
        background_protection_enabled: false,
        background_status: SaveBackupBackgroundProtectionStatus::TrayOnly,
        last_checked_at: Some(40),
        last_attempt_at: None,
        last_success_at: None,
        next_due_at: Some(80),
        pending_reason: None,
        last_error_code: None,
        worker_instance_id: None,
        lease_owner: Some(lease_owner.to_owned()),
        lease_expires_at: Some(120),
        updated_at: 40,
    }
}

fn sample_summary() -> SaveBackupSummary {
    SaveBackupSummary {
        backup_id: "backup-1".to_owned(),
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        trigger: SaveBackupTrigger::Manual,
        status: SaveBackupStatus::Completed,
        archive_file_name: "20260704-221530_mhw_profile-default_manual.zip".to_owned(),
        manifest_file_name: "20260704-221530_mhw_profile-default_manual.manifest.json".to_owned(),
        archive_size_bytes: 128,
        archive_sha256: "sha256:test".to_owned(),
        file_count: 1,
        created_at: 42,
        source_path_label: Some("Saves".to_owned()),
        source_path_hash: "sha256:source".to_owned(),
        backup_directory: hmm_core::ProfileDirectorySelection {
            mode: hmm_core::ProfileDirectoryMode::Default,
            status: hmm_core::ProfileDirectoryStatus::Defaulted,
            directory: None,
            path_label: Some("HelsincyModManager/backups/saves/mhw/profile-default".to_owned()),
            messages: Vec::new(),
        },
        notes: Some("manual note".to_owned()),
    }
}

fn sample_result() -> CreateSaveBackupResult {
    CreateSaveBackupResult {
        summary: sample_summary(),
        warnings: Vec::new(),
    }
}

struct RecordingSaveBackupExecutor {
    result: Mutex<Result<CreateSaveBackupResult, SaveBackupError>>,
    requests: Mutex<Vec<(CreateSaveBackupRequest, SaveBackupTrigger)>>,
}

impl RecordingSaveBackupExecutor {
    fn ok(result: CreateSaveBackupResult) -> Self {
        Self {
            result: Mutex::new(Ok(result)),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn err(error: SaveBackupError) -> Self {
        Self {
            result: Mutex::new(Err(error)),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn take_requests(&self) -> Vec<(CreateSaveBackupRequest, SaveBackupTrigger)> {
        std::mem::take(&mut *self.requests.lock().unwrap())
    }
}

impl SaveBackupExecutor for RecordingSaveBackupExecutor {
    fn create_backup(
        &self,
        request: CreateSaveBackupRequest,
        trigger: SaveBackupTrigger,
    ) -> Result<CreateSaveBackupResult, SaveBackupError> {
        self.requests.lock().unwrap().push((request, trigger));
        self.result.lock().unwrap().clone()
    }
}

struct CancellingSaveBackupExecutor {
    task_manager: Arc<TaskManager>,
    task_id: String,
}

impl SaveBackupExecutor for CancellingSaveBackupExecutor {
    fn create_backup(
        &self,
        _request: CreateSaveBackupRequest,
        _trigger: SaveBackupTrigger,
    ) -> Result<CreateSaveBackupResult, SaveBackupError> {
        self.task_manager
            .cancel_task(&self.task_id)
            .expect("running task can be cancelled");
        Ok(sample_result())
    }
}

struct PanickingSaveBackupExecutor;

impl SaveBackupExecutor for PanickingSaveBackupExecutor {
    fn create_backup(
        &self,
        _request: CreateSaveBackupRequest,
        _trigger: SaveBackupTrigger,
    ) -> Result<CreateSaveBackupResult, SaveBackupError> {
        panic!("simulated save backup executor panic");
    }
}

#[derive(Default)]
struct RecordingAuditLogWriter {
    events: Mutex<Vec<AuditLogEvent>>,
}

impl RecordingAuditLogWriter {
    fn take_events(&self) -> Vec<AuditLogEvent> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}

impl AuditLogWriter for RecordingAuditLogWriter {
    fn record(&self, event: AuditLogEvent) -> Result<()> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }
}

struct RecordingSchedulerStateRepository {
    states: Mutex<Vec<SaveBackupSchedulerState>>,
    release_calls: Mutex<Vec<String>>,
}

impl RecordingSchedulerStateRepository {
    fn with_state(state: SaveBackupSchedulerState) -> Self {
        Self {
            states: Mutex::new(vec![state]),
            release_calls: Mutex::new(Vec::new()),
        }
    }

    fn latest_state(&self) -> Option<SaveBackupSchedulerState> {
        self.states.lock().unwrap().last().cloned()
    }

    fn release_calls(&self) -> Vec<String> {
        self.release_calls.lock().unwrap().clone()
    }
}

impl SaveBackupSchedulerStateRepository for RecordingSchedulerStateRepository {
    fn get_state(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<Option<SaveBackupSchedulerState>> {
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
        let mut states = self.states.lock().unwrap();
        states.retain(|existing| {
            existing.game_id != state.game_id || existing.profile_id != state.profile_id
        });
        states.push(state.clone());
        Ok(())
    }

    fn acquire_due_lease(
        &self,
        _request: SaveBackupSchedulerLeaseRequest,
    ) -> Result<Option<SaveBackupSchedulerState>> {
        panic!("task runner tests do not acquire leases")
    }

    fn release_lease(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
        lease_owner: &str,
    ) -> Result<()> {
        self.release_calls
            .lock()
            .unwrap()
            .push(lease_owner.to_owned());
        Ok(())
    }

    fn record_worker_heartbeat(&self, _heartbeat: SaveBackupWorkerHeartbeat) -> Result<()> {
        Ok(())
    }
}

struct FixedClock;

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(42)
    }
}
