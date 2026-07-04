use std::collections::BTreeMap;
use std::sync::Arc;

use hmm_core::{GameId, ProfileId, SaveBackupSummary};
use hmm_ports::{AppClock, AuditLogEvent, AuditLogWriter};

use crate::{
    CreateSaveBackupRequest, SaveBackupError, SaveBackupService, TaskKind, TaskManager,
    TaskManagerError, TaskProgressEvent, TaskStarted, TaskStatus,
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
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupTaskRunError {
    pub events: Vec<TaskProgressEvent>,
}

pub trait SaveBackupExecutor: Send + Sync {
    fn create_manual_backup(
        &self,
        request: CreateSaveBackupRequest,
    ) -> Result<SaveBackupSummary, SaveBackupError>;
}

impl SaveBackupExecutor for SaveBackupService {
    fn create_manual_backup(
        &self,
        request: CreateSaveBackupRequest,
    ) -> Result<SaveBackupSummary, SaveBackupError> {
        SaveBackupService::create_manual_backup(self, request)
    }
}

pub struct SaveBackupTaskService {
    task_manager: Arc<TaskManager>,
}

impl SaveBackupTaskService {
    pub fn new(task_manager: Arc<TaskManager>) -> Self {
        Self { task_manager }
    }

    pub fn start_save_backup_task(
        &self,
        _request: StartSaveBackupTaskRequest,
    ) -> Result<TaskStarted, TaskManagerError> {
        let task = self.task_manager.create_task(TaskKind::SaveBackup)?;

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
}

impl SaveBackupTaskRunner {
    pub fn new(
        task_manager: Arc<TaskManager>,
        executor: Arc<dyn SaveBackupExecutor>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            task_manager,
            executor,
            audit_log,
            clock,
        }
    }

    pub fn run_save_backup_task(
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

        let summary = match self.executor.create_manual_backup(CreateSaveBackupRequest {
            game_id: request.game_id.clone(),
            profile_id: request.profile_id.clone(),
            note: request.note.clone(),
        }) {
            Ok(summary) => summary,
            Err(error) => {
                return Err(self.fail_with_audit(task_id, &request, events, error));
            }
        };

        events.push(running_event(task_id, SAVE_BACKUP_MANIFEST_WRITING_PHASE));
        events.push(running_event(task_id, SAVE_BACKUP_RETENTION_PRUNING_PHASE));
        self.record_success_audit(task_id, &request, &summary);

        match self.task_manager.complete_task(task_id) {
            Ok(task) => {
                events.push(TaskProgressEvent::new(
                    task.task_id,
                    task.kind,
                    task.status,
                    SAVE_BACKUP_COMPLETED_PHASE,
                ));
                Ok(events)
            }
            Err(_) if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) => {
                Ok(events)
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

        self.record_audit("success", fields);
    }

    fn record_failure_audit(
        &self,
        task_id: &str,
        request: &StartSaveBackupTaskRequest,
        error_code: &str,
    ) {
        let mut fields = audit_fields(task_id, request);
        fields.insert("error_code".to_owned(), error_code.to_owned());

        self.record_audit("failure", fields);
    }

    fn record_audit(&self, result: &str, fields: BTreeMap<String, String>) {
        let timestamp_unix_millis = self.clock.now_unix_millis().unwrap_or_default();
        let _ = self.audit_log.record(AuditLogEvent {
            timestamp_unix_millis,
            category: "save_backup".to_owned(),
            operation: "manual_backup".to_owned(),
            result: result.to_owned(),
            fields,
        });
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
    ])
}
