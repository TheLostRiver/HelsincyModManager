use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use hmm_core::{
    GameId, ProfileId, SaveBackupSchedulerLeaseRenewalRequest, SaveBackupSchedulerState,
    SaveBackupSummary, SaveBackupTrigger,
};
use hmm_ports::{AppClock, AuditLogEvent, AuditLogWriter, SaveBackupSchedulerStateRepository};

use crate::{
    CreateSaveBackupRequest, CreateSaveBackupResult, SaveBackupError, SaveBackupService,
    SaveBackupWarning, TaskKind, TaskManager, TaskManagerError, TaskProgressEvent, TaskSnapshot,
    TaskStarted, TaskStatus,
};

const SAVE_BACKUP_SCANNING_PHASE: &str = "save_backup.scanning";
const SAVE_BACKUP_ARCHIVING_PHASE: &str = "save_backup.archiving";
const SAVE_BACKUP_MANIFEST_WRITING_PHASE: &str = "save_backup.manifest_writing";
const SAVE_BACKUP_RETENTION_PRUNING_PHASE: &str = "save_backup.retention_pruning";
const SAVE_BACKUP_COMPLETED_PHASE: &str = "save_backup.completed";
const SAVE_BACKUP_FAILED_PHASE: &str = "save_backup.failed";
const SAVE_BACKUP_FAILED_ERROR: &str = "save_backup_failed";
const SAVE_BACKUP_SCHEDULER_LEASE_UNAVAILABLE_ERROR: &str =
    "save_backup_scheduler_lease_unavailable";
const SCHEDULER_LEASE_TTL_MILLIS: u128 = 5 * 60_000;
const DEFAULT_SCHEDULER_LEASE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(60);
const MIN_SCHEDULER_LEASE_KEEPALIVE_INTERVAL: Duration = Duration::from_millis(1);
const MAX_SCHEDULER_LEASE_KEEPALIVE_INTERVAL: Duration = DEFAULT_SCHEDULER_LEASE_KEEPALIVE_INTERVAL;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartSaveBackupTaskRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub trigger: SaveBackupTrigger,
    pub note: Option<String>,
    pub scheduler_lease_owner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SaveBackupTaskScope {
    game_id: String,
    profile_id: String,
}

impl From<&StartSaveBackupTaskRequest> for SaveBackupTaskScope {
    fn from(request: &StartSaveBackupTaskRequest) -> Self {
        Self {
            game_id: request.game_id.as_str().to_owned(),
            profile_id: request.profile_id.as_str().to_owned(),
        }
    }
}

#[derive(Debug, Default)]
pub struct SaveBackupTaskScopeRegistry {
    active_tasks: Mutex<BTreeMap<SaveBackupTaskScope, String>>,
}

impl SaveBackupTaskScopeRegistry {
    pub fn reserve_task(
        &self,
        request: &StartSaveBackupTaskRequest,
        create_task: impl FnOnce() -> Result<TaskSnapshot, TaskManagerError>,
    ) -> Result<TaskSnapshot, TaskManagerError> {
        let scope = SaveBackupTaskScope::from(request);
        let mut active_tasks = self
            .active_tasks
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;

        if let Some(task_id) = active_tasks.get(&scope) {
            return Err(TaskManagerError::TaskScopeBusy {
                kind: TaskKind::SaveBackup,
                task_id: task_id.clone(),
            });
        }

        let task = create_task()?;
        active_tasks.insert(scope, task.task_id.clone());

        Ok(task)
    }

    pub fn release_task(&self, request: &StartSaveBackupTaskRequest, task_id: &str) {
        let scope = SaveBackupTaskScope::from(request);
        let Ok(mut active_tasks) = self.active_tasks.lock() else {
            return;
        };

        if active_tasks
            .get(&scope)
            .is_some_and(|active_task_id| active_task_id == task_id)
        {
            active_tasks.remove(&scope);
        }
    }
}

struct SaveBackupTaskScopeReleaseGuard<'a> {
    registry: &'a SaveBackupTaskScopeRegistry,
    request: &'a StartSaveBackupTaskRequest,
    task_id: &'a str,
}

impl Drop for SaveBackupTaskScopeReleaseGuard<'_> {
    fn drop(&mut self) {
        self.registry.release_task(self.request, self.task_id);
    }
}

struct SaveBackupSchedulerLeaseReleaseGuard<'a> {
    repository: Option<&'a dyn SaveBackupSchedulerStateRepository>,
    request: &'a StartSaveBackupTaskRequest,
}

impl Drop for SaveBackupSchedulerLeaseReleaseGuard<'_> {
    fn drop(&mut self) {
        let Some(repository) = self.repository else {
            return;
        };
        let Some(lease_owner) = auto_lease_owner(self.request) else {
            return;
        };

        let _ =
            repository.release_lease(&self.request.game_id, &self.request.profile_id, lease_owner);
    }
}

struct SaveBackupSchedulerLeaseKeepalive {
    stop_sender: Option<mpsc::Sender<()>>,
    join_handle: Option<thread::JoinHandle<()>>,
    renewal_failed: Arc<AtomicBool>,
}

impl SaveBackupSchedulerLeaseKeepalive {
    fn stop_and_join(&mut self) -> Result<(), ()> {
        let stop_failed = self
            .stop_sender
            .take()
            .is_none_or(|stop_sender| stop_sender.send(()).is_err());
        let join_failed = self
            .join_handle
            .take()
            .is_none_or(|join_handle| join_handle.join().is_err());
        let renewal_failed = self.renewal_failed.load(Ordering::Acquire);

        if stop_failed || join_failed || renewal_failed {
            Err(())
        } else {
            Ok(())
        }
    }
}

impl Drop for SaveBackupSchedulerLeaseKeepalive {
    fn drop(&mut self) {
        let _ = self.stop_and_join();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupTaskRunError {
    pub events: Vec<TaskProgressEvent>,
}

pub trait SaveBackupExecutor: Send + Sync {
    fn create_backup(
        &self,
        request: CreateSaveBackupRequest,
        trigger: SaveBackupTrigger,
    ) -> Result<CreateSaveBackupResult, SaveBackupError>;
}

impl SaveBackupExecutor for SaveBackupService {
    fn create_backup(
        &self,
        request: CreateSaveBackupRequest,
        trigger: SaveBackupTrigger,
    ) -> Result<CreateSaveBackupResult, SaveBackupError> {
        SaveBackupService::create_backup(self, request, trigger)
    }
}

pub struct SaveBackupTaskService {
    task_manager: Arc<TaskManager>,
    scope_registry: Arc<SaveBackupTaskScopeRegistry>,
}

impl SaveBackupTaskService {
    pub fn new(task_manager: Arc<TaskManager>) -> Self {
        Self::with_scope_registry(
            task_manager,
            Arc::new(SaveBackupTaskScopeRegistry::default()),
        )
    }

    pub fn with_scope_registry(
        task_manager: Arc<TaskManager>,
        scope_registry: Arc<SaveBackupTaskScopeRegistry>,
    ) -> Self {
        Self {
            task_manager,
            scope_registry,
        }
    }

    pub fn start_save_backup_task(
        &self,
        request: StartSaveBackupTaskRequest,
    ) -> Result<TaskStarted, TaskManagerError> {
        let task = self.scope_registry.reserve_task(&request, || {
            self.task_manager.create_task(TaskKind::SaveBackup)
        })?;

        Ok(TaskStarted {
            task_id: task.task_id,
            kind: task.kind,
            status: task.status,
        })
    }
}

pub struct SaveBackupTaskRunner {
    task_manager: Arc<TaskManager>,
    executor: Arc<dyn SaveBackupExecutor>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
    scope_registry: Arc<SaveBackupTaskScopeRegistry>,
    scheduler_state_repository: Option<Arc<dyn SaveBackupSchedulerStateRepository>>,
    scheduler_lease_keepalive_interval: Duration,
}

impl SaveBackupTaskRunner {
    pub fn new(
        task_manager: Arc<TaskManager>,
        executor: Arc<dyn SaveBackupExecutor>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self::with_scope_registry(
            task_manager,
            executor,
            audit_log,
            clock,
            Arc::new(SaveBackupTaskScopeRegistry::default()),
        )
    }

    pub fn with_scope_registry(
        task_manager: Arc<TaskManager>,
        executor: Arc<dyn SaveBackupExecutor>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        scope_registry: Arc<SaveBackupTaskScopeRegistry>,
    ) -> Self {
        Self {
            task_manager,
            executor,
            audit_log,
            clock,
            scope_registry,
            scheduler_state_repository: None,
            scheduler_lease_keepalive_interval: DEFAULT_SCHEDULER_LEASE_KEEPALIVE_INTERVAL,
        }
    }

    pub fn with_scope_registry_and_scheduler_state(
        task_manager: Arc<TaskManager>,
        executor: Arc<dyn SaveBackupExecutor>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        scope_registry: Arc<SaveBackupTaskScopeRegistry>,
        scheduler_state_repository: Arc<dyn SaveBackupSchedulerStateRepository>,
    ) -> Self {
        Self::with_scope_registry_and_scheduler_state_and_keepalive_interval(
            task_manager,
            executor,
            audit_log,
            clock,
            scope_registry,
            scheduler_state_repository,
            DEFAULT_SCHEDULER_LEASE_KEEPALIVE_INTERVAL,
        )
    }

    pub fn with_scope_registry_and_scheduler_state_and_keepalive_interval(
        task_manager: Arc<TaskManager>,
        executor: Arc<dyn SaveBackupExecutor>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        scope_registry: Arc<SaveBackupTaskScopeRegistry>,
        scheduler_state_repository: Arc<dyn SaveBackupSchedulerStateRepository>,
        scheduler_lease_keepalive_interval: Duration,
    ) -> Self {
        Self {
            task_manager,
            executor,
            audit_log,
            clock,
            scope_registry,
            scheduler_state_repository: Some(scheduler_state_repository),
            scheduler_lease_keepalive_interval: scheduler_lease_keepalive_interval
                .max(MIN_SCHEDULER_LEASE_KEEPALIVE_INTERVAL)
                .min(MAX_SCHEDULER_LEASE_KEEPALIVE_INTERVAL),
        }
    }

    pub fn scheduler_lease_keepalive_interval(&self) -> Duration {
        self.scheduler_lease_keepalive_interval
    }

    pub fn run_save_backup_task(
        &self,
        task_id: &str,
        request: StartSaveBackupTaskRequest,
    ) -> Result<Vec<TaskProgressEvent>, SaveBackupTaskRunError> {
        let _scope_release = SaveBackupTaskScopeReleaseGuard {
            registry: self.scope_registry.as_ref(),
            request: &request,
            task_id,
        };
        let _scheduler_lease_release = SaveBackupSchedulerLeaseReleaseGuard {
            repository: self.scheduler_state_repository.as_deref(),
            request: &request,
        };
        self.run_save_backup_task_inner(task_id, request.clone())
    }

    fn run_save_backup_task_inner(
        &self,
        task_id: &str,
        request: StartSaveBackupTaskRequest,
    ) -> Result<Vec<TaskProgressEvent>, SaveBackupTaskRunError> {
        if self.task_manager.start_task(task_id).is_err() {
            return Err(SaveBackupTaskRunError { events: Vec::new() });
        }
        let mut events = vec![
            running_event(task_id, SAVE_BACKUP_SCANNING_PHASE),
            running_event(task_id, SAVE_BACKUP_ARCHIVING_PHASE),
        ];
        self.record_scheduler_attempt(&request);

        let mut lease_keepalive = match self.start_scheduler_lease_keepalive(&request) {
            Ok(lease_keepalive) => lease_keepalive,
            Err(()) => {
                return Err(self.fail_with_audit_code(
                    task_id,
                    &request,
                    events,
                    SAVE_BACKUP_SCHEDULER_LEASE_UNAVAILABLE_ERROR,
                ));
            }
        };

        let result = self.executor.create_backup(
            CreateSaveBackupRequest {
                game_id: request.game_id.clone(),
                profile_id: request.profile_id.clone(),
                note: request.note.clone(),
            },
            request.trigger,
        );

        let scheduler_lease_failed = if let Some(keepalive) = lease_keepalive.as_mut() {
            let keepalive_failed = keepalive.stop_and_join().is_err();
            let final_confirmation_failed = self.renew_scheduler_lease(&request).is_err();
            keepalive_failed || final_confirmation_failed
        } else {
            false
        };
        if scheduler_lease_failed {
            return Err(self.fail_with_audit_code(
                task_id,
                &request,
                events,
                SAVE_BACKUP_SCHEDULER_LEASE_UNAVAILABLE_ERROR,
            ));
        }

        let result = match result {
            Ok(summary) => summary,
            Err(error) => {
                return Err(self.fail_with_audit(task_id, &request, events, error));
            }
        };

        match self.task_manager.complete_task(task_id) {
            Ok(task) => {
                events.push(running_event(task_id, SAVE_BACKUP_MANIFEST_WRITING_PHASE));
                events.push(running_event(task_id, SAVE_BACKUP_RETENTION_PRUNING_PHASE));
                self.record_scheduler_success(&request, &result.summary);
                self.record_success_audit(task_id, &request, &result.summary);
                self.record_warning_audits(task_id, &request, &result.warnings);
                events.push(TaskProgressEvent::new(
                    task.task_id,
                    task.kind,
                    task.status,
                    SAVE_BACKUP_COMPLETED_PHASE,
                ));
                Ok(events)
            }
            Err(_) if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) => {
                self.record_scheduler_success(&request, &result.summary);
                self.record_success_audit(task_id, &request, &result.summary);
                self.record_warning_audits(task_id, &request, &result.warnings);
                Ok(Vec::new())
            }
            Err(_) => Err(self.fail_with_audit(
                task_id,
                &request,
                events,
                SaveBackupError::HistoryUnavailable,
            )),
        }
    }

    fn fail_with_audit(
        &self,
        task_id: &str,
        request: &StartSaveBackupTaskRequest,
        events: Vec<TaskProgressEvent>,
        error: SaveBackupError,
    ) -> SaveBackupTaskRunError {
        self.fail_with_audit_code(task_id, request, events, error.code())
    }

    fn fail_with_audit_code(
        &self,
        task_id: &str,
        request: &StartSaveBackupTaskRequest,
        mut events: Vec<TaskProgressEvent>,
        error_code: &str,
    ) -> SaveBackupTaskRunError {
        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled)
            && error_code != SAVE_BACKUP_SCHEDULER_LEASE_UNAVAILABLE_ERROR
        {
            return SaveBackupTaskRunError { events };
        }

        let _ = self.task_manager.fail_task(task_id);
        let mut event = TaskProgressEvent::new(
            task_id.to_owned(),
            TaskKind::SaveBackup,
            TaskStatus::Failed,
            SAVE_BACKUP_FAILED_PHASE,
        );
        event.error = Some(format!("{}:{}", SAVE_BACKUP_FAILED_ERROR, error_code));
        events.push(event);
        self.record_scheduler_failure(request, error_code);
        self.record_failure_audit(task_id, request, error_code);
        SaveBackupTaskRunError { events }
    }

    fn start_scheduler_lease_keepalive(
        &self,
        request: &StartSaveBackupTaskRequest,
    ) -> Result<Option<SaveBackupSchedulerLeaseKeepalive>, ()> {
        if request.trigger != SaveBackupTrigger::Auto {
            return Ok(None);
        }
        self.renew_scheduler_lease(request)?;

        let Some(lease_owner) = auto_lease_owner(request) else {
            return Err(());
        };
        let Some(repository) = self.scheduler_state_repository.as_ref() else {
            return Err(());
        };

        let (stop_sender, stop_receiver) = mpsc::channel();
        let renewal_failed = Arc::new(AtomicBool::new(false));
        let renewal_failed_for_thread = Arc::clone(&renewal_failed);
        let repository = Arc::clone(repository);
        let clock = Arc::clone(&self.clock);
        let game_id = request.game_id.clone();
        let profile_id = request.profile_id.clone();
        let lease_owner = lease_owner.to_owned();
        let interval = self.scheduler_lease_keepalive_interval;
        let join_handle = thread::spawn(move || loop {
            match stop_receiver.recv_timeout(interval) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }

            let Ok(now_unix_millis) = clock.now_unix_millis() else {
                renewal_failed_for_thread.store(true, Ordering::Release);
                break;
            };
            let renewed = repository
                .renew_lease(SaveBackupSchedulerLeaseRenewalRequest {
                    game_id: game_id.clone(),
                    profile_id: profile_id.clone(),
                    lease_owner: lease_owner.clone(),
                    lease_expires_at: now_unix_millis.saturating_add(SCHEDULER_LEASE_TTL_MILLIS),
                    now_unix_millis,
                })
                .unwrap_or(false);
            if !renewed {
                renewal_failed_for_thread.store(true, Ordering::Release);
                break;
            }
        });

        Ok(Some(SaveBackupSchedulerLeaseKeepalive {
            stop_sender: Some(stop_sender),
            join_handle: Some(join_handle),
            renewal_failed,
        }))
    }

    fn renew_scheduler_lease(&self, request: &StartSaveBackupTaskRequest) -> Result<(), ()> {
        let lease_owner = auto_lease_owner(request).ok_or(())?;
        let repository = self.scheduler_state_repository.as_ref().ok_or(())?;
        let now_unix_millis = self.clock.now_unix_millis().map_err(|_| ())?;
        let renewed = repository
            .renew_lease(SaveBackupSchedulerLeaseRenewalRequest {
                game_id: request.game_id.clone(),
                profile_id: request.profile_id.clone(),
                lease_owner: lease_owner.to_owned(),
                lease_expires_at: now_unix_millis.saturating_add(SCHEDULER_LEASE_TTL_MILLIS),
                now_unix_millis,
            })
            .unwrap_or(false);

        renewed.then_some(()).ok_or(())
    }

    fn record_scheduler_attempt(&self, request: &StartSaveBackupTaskRequest) {
        self.update_scheduler_state(request, |state, now| {
            state.last_attempt_at = Some(now);
            state.pending_reason = None;
            state.last_error_code = None;
            state.updated_at = now;
        });
    }

    fn record_scheduler_success(
        &self,
        request: &StartSaveBackupTaskRequest,
        summary: &SaveBackupSummary,
    ) {
        self.update_scheduler_state(request, |state, now| {
            state.last_attempt_at = Some(now);
            state.last_success_at = Some(summary.created_at);
            state.pending_reason = None;
            state.last_error_code = None;
            state.updated_at = now;
        });
    }

    fn record_scheduler_failure(&self, request: &StartSaveBackupTaskRequest, error_code: &str) {
        self.update_scheduler_state(request, |state, now| {
            state.last_attempt_at = Some(now);
            state.last_error_code = Some(error_code.to_owned());
            state.updated_at = now;
        });
    }

    fn update_scheduler_state(
        &self,
        request: &StartSaveBackupTaskRequest,
        update: impl FnOnce(&mut SaveBackupSchedulerState, u128),
    ) {
        if request.trigger != SaveBackupTrigger::Auto {
            return;
        }
        let Some(repository) = self.scheduler_state_repository.as_deref() else {
            return;
        };
        let Ok(now) = self.clock.now_unix_millis() else {
            return;
        };
        let Ok(Some(mut state)) = repository.get_state(&request.game_id, &request.profile_id)
        else {
            return;
        };

        update(&mut state, now);
        let _ = repository.upsert_state(&state);
    }

    fn record_success_audit(
        &self,
        task_id: &str,
        request: &StartSaveBackupTaskRequest,
        summary: &SaveBackupSummary,
    ) {
        let mut fields = audit_fields(task_id, request);
        fields.insert("backup_id".to_owned(), summary.backup_id.clone());
        fields.insert("trigger".to_owned(), summary.trigger.as_str().to_owned());
        fields.insert("file_count".to_owned(), summary.file_count.to_string());
        fields.insert(
            "archive_size_bytes".to_owned(),
            summary.archive_size_bytes.to_string(),
        );

        self.record_audit(request.trigger, "success", fields);
    }

    fn record_warning_audits(
        &self,
        task_id: &str,
        request: &StartSaveBackupTaskRequest,
        warnings: &[SaveBackupWarning],
    ) {
        for warning in warnings {
            let mut fields = audit_fields(task_id, request);
            fields.insert("error_code".to_owned(), warning.code().to_owned());
            self.record_audit_for_operation("retention_pruning", "warning", fields);
        }
    }

    fn record_failure_audit(
        &self,
        task_id: &str,
        request: &StartSaveBackupTaskRequest,
        error_code: &str,
    ) {
        let mut fields = audit_fields(task_id, request);
        fields.insert("error_code".to_owned(), error_code.to_owned());

        self.record_audit(request.trigger, "failure", fields);
    }

    fn record_audit(
        &self,
        trigger: SaveBackupTrigger,
        result: &str,
        fields: BTreeMap<String, String>,
    ) {
        self.record_audit_for_operation(backup_operation(trigger), result, fields);
    }

    fn record_audit_for_operation(
        &self,
        operation: &str,
        result: &str,
        fields: BTreeMap<String, String>,
    ) {
        let timestamp_unix_millis = self.clock.now_unix_millis().unwrap_or_default();
        let _ = self.audit_log.record(AuditLogEvent {
            timestamp_unix_millis,
            category: "save_backup".to_owned(),
            operation: operation.to_owned(),
            result: result.to_owned(),
            fields,
        });
    }
}

fn backup_operation(trigger: SaveBackupTrigger) -> &'static str {
    match trigger {
        SaveBackupTrigger::Manual => "manual_backup",
        SaveBackupTrigger::Auto => "auto_backup",
        SaveBackupTrigger::PreInstall => "pre_install_backup",
    }
}

fn auto_lease_owner(request: &StartSaveBackupTaskRequest) -> Option<&str> {
    (request.trigger == SaveBackupTrigger::Auto)
        .then_some(request.scheduler_lease_owner.as_deref())
        .flatten()
        .filter(|lease_owner| !lease_owner.trim().is_empty())
}

fn running_event(task_id: &str, phase: &str) -> TaskProgressEvent {
    TaskProgressEvent::new(
        task_id.to_owned(),
        TaskKind::SaveBackup,
        TaskStatus::Running,
        phase,
    )
}

fn audit_fields(task_id: &str, request: &StartSaveBackupTaskRequest) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("task_id".to_owned(), task_id.to_owned()),
        ("game_id".to_owned(), request.game_id.as_str().to_owned()),
        (
            "profile_id".to_owned(),
            request.profile_id.as_str().to_owned(),
        ),
        ("trigger".to_owned(), request.trigger.as_str().to_owned()),
    ])
}
