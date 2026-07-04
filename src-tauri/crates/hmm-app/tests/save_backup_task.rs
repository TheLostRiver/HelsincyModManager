use anyhow::Result;
use hmm_app::{
    CreateSaveBackupRequest, SaveBackupError, SaveBackupExecutor, SaveBackupTaskRunner,
    SaveBackupTaskService, StartSaveBackupTaskRequest, TaskKind, TaskManager, TaskStatus,
};
use hmm_core::{GameId, ProfileId, SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger};
use hmm_ports::{AppClock, AuditLogEvent, AuditLogWriter};
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
fn run_save_backup_task_emits_registered_phases_and_records_success_audit() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let executor = Arc::new(RecordingSaveBackupExecutor::ok(sample_summary()));
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
        executor.take_requests()[0].note.as_deref(),
        Some("manual note")
    );

    let event = audit_log.take_event().expect("success audit event");
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

    let event = audit_log.take_event().expect("failure audit event");
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
        note: Some("manual note".to_owned()),
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
        notes: Some("manual note".to_owned()),
    }
}

struct RecordingSaveBackupExecutor {
    result: Mutex<Result<SaveBackupSummary, SaveBackupError>>,
    requests: Mutex<Vec<CreateSaveBackupRequest>>,
}

impl RecordingSaveBackupExecutor {
    fn ok(summary: SaveBackupSummary) -> Self {
        Self {
            result: Mutex::new(Ok(summary)),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn err(error: SaveBackupError) -> Self {
        Self {
            result: Mutex::new(Err(error)),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn take_requests(&self) -> Vec<CreateSaveBackupRequest> {
        std::mem::take(&mut *self.requests.lock().unwrap())
    }
}

impl SaveBackupExecutor for RecordingSaveBackupExecutor {
    fn create_manual_backup(
        &self,
        request: CreateSaveBackupRequest,
    ) -> Result<SaveBackupSummary, SaveBackupError> {
        self.requests.lock().unwrap().push(request);
        self.result.lock().unwrap().clone()
    }
}

#[derive(Default)]
struct RecordingAuditLogWriter {
    event: Mutex<Option<AuditLogEvent>>,
}

impl RecordingAuditLogWriter {
    fn take_event(&self) -> Option<AuditLogEvent> {
        self.event.lock().unwrap().take()
    }
}

impl AuditLogWriter for RecordingAuditLogWriter {
    fn record(&self, event: AuditLogEvent) -> Result<()> {
        *self.event.lock().unwrap() = Some(event);
        Ok(())
    }
}

struct FixedClock;

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(42)
    }
}
