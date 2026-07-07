use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use hmm_core::{GameId, ProfileId, SaveBackupSummary, SaveBackupTrigger};
use hmm_ports::{AppClock, AuditLogEvent, AuditLogWriter};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartSaveBackupTaskRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub trigger: SaveBackupTrigger,
    pub note: Option<String>,
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
        }
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

        let result = match self.executor.create_backup(
            CreateSaveBackupRequest {
                game_id: request.game_id.clone(),
                profile_id: request.profile_id.clone(),
                note: request.note.clone(),
            },
            request.trigger,
        ) {
            Ok(summary) => summary,
            Err(error) => {
                return Err(self.fail_with_audit(task_id, &request, events, error));
            }
        };

        match self.task_manager.complete_task(task_id) {
            Ok(task) => {
                events.push(running_event(task_id, SAVE_BACKUP_MANIFEST_WRITING_PHASE));
                events.push(running_event(task_id, SAVE_BACKUP_RETENTION_PRUNING_PHASE));
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
        mut events: Vec<TaskProgressEvent>,
        error: SaveBackupError,
    ) -> SaveBackupTaskRunError {
        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
            return SaveBackupTaskRunError { events };
        }

        let _ = self.task_manager.fail_task(task_id);
        let mut event = TaskProgressEvent::new(
            task_id.to_owned(),
            TaskKind::SaveBackup,
            TaskStatus::Failed,
            SAVE_BACKUP_FAILED_PHASE,
        );
        event.error = Some(format!("{}:{}", SAVE_BACKUP_FAILED_ERROR, error.code()));
        events.push(event);
        self.record_failure_audit(task_id, request, error.code());
        SaveBackupTaskRunError { events }
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
