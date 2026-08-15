use anyhow::Result;
use hmm_app::{
    CreateSaveBackupRequest, CreateSaveBackupResult, CrossProcessWriteAdmissionCoordinator,
    SaveBackupError, SaveBackupExecutor, SaveBackupTaskScopeRegistry, SaveBackupTaskService,
    SaveProfileMaintenanceScopeRegistry, SaveRestoreCommitContext, SaveRestorePreviewError,
    StartSaveBackupTaskRequest,
};
use hmm_app::{
    SaveRestoreCommitValidator, SaveRestoreTaskRunner, SaveRestoreTaskScopeRegistry,
    SaveRestoreTaskService, StartSaveRestoreRequest, TaskKind, TaskManager, TaskManagerError,
    TaskSnapshot, TaskStatus,
};
use hmm_core::{
    BackupCadence, GameId, ProfileBackupRetention, ProfileBackupSchedule, ProfileDirectoryMode,
    ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId, ProfileSaveSettings,
    SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger, SaveRestoreTransaction,
    SaveRestoreTransactionStatus,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, CancellationToken, CrossProcessWriteAcquisition,
    CrossProcessWriteAdmission, CrossProcessWriteAdmissionError,
    CrossProcessWriteAdmissionResult, CrossProcessWriteGuard, CrossProcessWriteScope,
    CrossProcessWriteScopeKind, PreparedSaveRestore, SaveRestoreCommitError,
    SaveRestoreCommitRequest, SaveRestoreCommitResult, SaveRestoreFileSystem,
    SaveRestoreFinalizeError, SaveRestoreFinalizeRequest, SaveRestorePrepareError,
    SaveRestorePrepareRequest, SaveRestoreTransactionRepository,
};
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

#[test]
fn runner_persists_transaction_before_finalize_and_commits_pre_restore_backup() {
    let harness = Harness::success();
    let service = harness.service();
    let started = service
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    let events = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect("restore succeeds");

    assert_eq!(
        harness.transactions.statuses(),
        vec![
            SaveRestoreTransactionStatus::Planned,
            SaveRestoreTransactionStatus::Prepared,
            SaveRestoreTransactionStatus::PreRestoreCompleted,
            SaveRestoreTransactionStatus::Committing,
            SaveRestoreTransactionStatus::Committed,
            SaveRestoreTransactionStatus::Completed,
        ]
    );
    assert_eq!(harness.file_system.commit_count(), 1);
    assert_eq!(harness.file_system.finalize_count(), 1);
    assert_eq!(
        harness.backup.triggers(),
        vec![SaveBackupTrigger::PreRestore]
    );
    assert_eq!(harness.backup.notes(), vec![None]);
    assert_eq!(
        harness.task_manager.task_status(&started.task_id),
        Some(TaskStatus::Completed)
    );
    assert!(events
        .iter()
        .any(|event| event.phase == "save_restore.completed"));
}

#[test]
fn restore_holds_save_scope_then_stops_before_commit_when_game_scope_is_busy() {
    let task_manager = Arc::new(TaskManager::new());
    let transactions = Arc::new(RecordingTransactions::default());
    let file_system = Arc::new(RecordingFileSystem::default());
    let backup = Arc::new(RecordingBackupExecutor::default());
    let audit = Arc::new(RecordingAudit::default());
    let validator = Arc::new(StaticValidator::new([Ok(sample_context())]));
    let admission = Arc::new(SaveThenRejectGameAdmission::default());
    let coordinator = Arc::new(CrossProcessWriteAdmissionCoordinator::with_timeout(
        admission.clone(),
        Duration::from_millis(1),
    ));
    let maintenance_registry = Arc::new(
        SaveProfileMaintenanceScopeRegistry::with_cross_process_admission(Arc::clone(
            &coordinator,
        )),
    );
    let scope_registry = Arc::new(SaveRestoreTaskScopeRegistry::with_maintenance_registry(
        maintenance_registry,
    ));
    let write_locks = Arc::new(
        hmm_app::GameProfileWriteLockRegistry::with_cross_process_admission(coordinator),
    );
    let runner = SaveRestoreTaskRunner::with_scope_registry(
        Arc::clone(&task_manager),
        validator,
        file_system.clone(),
        transactions.clone(),
        backup,
        audit,
        Arc::new(FixedClock),
        write_locks,
        Arc::clone(&scope_registry),
    );
    let request = sample_request();
    let task = SaveRestoreTaskService::with_scope_registry(
        Arc::clone(&task_manager),
        scope_registry,
    )
    .start_save_restore_task(&request)
    .expect("restore task starts");

    let error = runner
        .run_save_restore_task(&task.task_id, request)
        .expect_err("busy game scope must reject restore commit");

    assert_eq!(error.error_code, "write_admission_busy");
    assert_eq!(file_system.commit_count(), 0);
    assert_eq!(
        transactions.statuses().last(),
        Some(&SaveRestoreTransactionStatus::Failed)
    );
    assert_eq!(
        admission.calls(),
        vec![
            CrossProcessWriteScopeKind::SaveProfileWrite,
            CrossProcessWriteScopeKind::GameProfileWrite,
        ]
    );
}

#[test]
fn runner_fails_closed_when_pre_restore_backup_fails() {
    let harness = Harness::success();
    harness.backup.set_fail(true);
    let service = harness.service();
    let started = service
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    let error = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect_err("pre-restore backup failure must block commit");

    assert_eq!(error.error_code, "save_backup_history_unavailable");
    assert_eq!(harness.file_system.commit_count(), 0);
    assert_eq!(harness.file_system.finalize_count(), 0);
    assert_eq!(
        harness.transactions.statuses().last(),
        Some(&SaveRestoreTransactionStatus::Failed)
    );
    assert_eq!(
        harness.task_manager.task_status(&started.task_id),
        Some(TaskStatus::Failed)
    );
}

#[test]
fn prepared_transaction_persistence_failure_requires_recovery() {
    let harness = Harness::success();
    harness
        .transactions
        .fail_on_status(SaveRestoreTransactionStatus::Prepared);
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    let error = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect_err("prepared transaction persistence failure requires recovery");

    assert_eq!(error.error_code, "save_restore_transaction_unavailable");
    let terminal = error
        .events
        .last()
        .expect("recovery-required terminal event");
    assert_eq!(terminal.status, TaskStatus::Failed);
    assert_eq!(terminal.phase, "save_restore.recovery_required");
    assert_eq!(
        terminal.error.as_deref(),
        Some("save_restore_transaction_unavailable")
    );
    assert_eq!(
        harness.transactions.statuses().last(),
        Some(&SaveRestoreTransactionStatus::Planned)
    );
    assert_eq!(harness.file_system.discard_count(), 1);
}

#[test]
fn failed_transaction_persistence_overrides_volatile_cancellation_projection() {
    let harness = Harness::success();
    harness
        .transactions
        .fail_on_status(SaveRestoreTransactionStatus::Failed);
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");
    harness
        .file_system
        .cancel_and_fail_prepare(Arc::clone(&harness.task_manager), started.task_id.clone());

    let error = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect_err("failed transaction persistence failure requires recovery");

    assert_eq!(error.error_code, "save_restore_transaction_unavailable");
    let terminal = error
        .events
        .last()
        .expect("recovery-required terminal event");
    assert_eq!(terminal.status, TaskStatus::Failed);
    assert_eq!(terminal.phase, "save_restore.recovery_required");
    assert_eq!(
        terminal.error.as_deref(),
        Some("save_restore_transaction_unavailable")
    );
    assert!(!error
        .events
        .iter()
        .any(|event| event.phase == "save_restore.cancelled"));
}

#[test]
fn runner_blocks_game_running_before_any_transaction_is_created() {
    let harness = Harness::blocked(SaveRestorePreviewError::GameRunning);
    let service = harness.service();
    let started = service
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    let error = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect_err("running game must block restore");

    assert_eq!(error.error_code, "save_restore_game_running");
    assert!(harness.transactions.statuses().is_empty());
    assert_eq!(harness.file_system.commit_count(), 0);
    assert_eq!(
        harness.task_manager.task_status(&started.task_id),
        Some(TaskStatus::Failed)
    );
}

#[test]
fn task_service_rejects_same_profile_until_runner_releases_scope() {
    let harness = Harness::success();
    let service = harness.service();
    let first = service
        .start_save_restore_task(&harness.request)
        .expect("first task starts");
    assert!(matches!(
        service.start_save_restore_task(&harness.request),
        Err(TaskManagerError::TaskScopeBusy {
            kind: hmm_app::TaskKind::SaveRestore,
            ..
        })
    ));

    harness
        .runner
        .run_save_restore_task(&first.task_id, harness.request.clone())
        .expect("first restore completes");

    service
        .start_save_restore_task(&harness.request)
        .expect("scope released after runner terminal state");
}

#[test]
fn aborting_an_unpublished_queued_task_releases_scope() {
    let harness = Harness::success();
    let service = harness.service();
    let first = service
        .start_save_restore_task(&harness.request)
        .expect("first task starts");

    service
        .abort_queued_save_restore_task(&harness.request, &first.task_id)
        .expect("unpublished task aborts");

    assert_eq!(
        harness.task_manager.task_status(&first.task_id),
        Some(TaskStatus::Failed)
    );
    service
        .start_save_restore_task(&harness.request)
        .expect("scope is released after abort");
}

#[test]
fn backup_restore_and_retention_share_the_same_profile_maintenance_scope() {
    let task_manager = Arc::new(TaskManager::new());
    let maintenance_registry = Arc::new(SaveProfileMaintenanceScopeRegistry::default());
    let backup_registry = Arc::new(SaveBackupTaskScopeRegistry::with_maintenance_registry(
        Arc::clone(&maintenance_registry),
    ));
    let restore_registry = Arc::new(SaveRestoreTaskScopeRegistry::with_maintenance_registry(
        maintenance_registry,
    ));
    let backup_service = SaveBackupTaskService::with_scope_registry(
        Arc::clone(&task_manager),
        Arc::clone(&backup_registry),
    );
    let restore_service = SaveRestoreTaskService::with_scope_registry(
        Arc::clone(&task_manager),
        Arc::clone(&restore_registry),
    );
    let restore_request = sample_request();
    let backup_request = StartSaveBackupTaskRequest {
        game_id: restore_request.game_id.clone(),
        profile_id: restore_request.profile_id.clone(),
        trigger: SaveBackupTrigger::Auto,
        note: None,
        scheduler_lease_owner: None,
    };

    let restore = restore_service
        .start_save_restore_task(&restore_request)
        .expect("restore reserves shared maintenance scope");
    restore_registry.release_task(&restore_request, "not-the-active-task");
    assert_eq!(
        backup_service
            .start_save_backup_task(backup_request.clone())
            .expect_err("backup cannot race an active restore"),
        TaskManagerError::TaskScopeBusy {
            kind: TaskKind::SaveBackup,
            task_id: restore.task_id.clone(),
        }
    );
    assert!(matches!(
        backup_registry.reserve_maintenance(
            &restore_request.game_id,
            &restore_request.profile_id
        ),
        Err(TaskManagerError::TaskScopeBusy {
            kind: TaskKind::SaveBackup,
            task_id,
        }) if task_id == restore.task_id
    ));
    restore_service
        .abort_queued_save_restore_task(&restore_request, &restore.task_id)
        .expect("aborting restore releases both scope registries");

    let backup = backup_service
        .start_save_backup_task(backup_request.clone())
        .expect("backup reserves shared maintenance scope");
    assert_eq!(
        restore_service
            .start_save_restore_task(&restore_request)
            .expect_err("restore cannot race backup retention"),
        TaskManagerError::TaskScopeBusy {
            kind: TaskKind::SaveRestore,
            task_id: backup.task_id.clone(),
        }
    );
    backup_registry.release_task(&backup_request, &backup.task_id);

    let retention = backup_registry
        .reserve_maintenance(&restore_request.game_id, &restore_request.profile_id)
        .expect("explicit retention reserves shared maintenance scope");
    assert_eq!(
        restore_service
            .start_save_restore_task(&restore_request)
            .expect_err("restore cannot race explicit retention"),
        TaskManagerError::TaskScopeBusy {
            kind: TaskKind::SaveRestore,
            task_id: "retention-maintenance".to_owned(),
        }
    );
    drop(retention);

    let next = restore_service
        .start_save_restore_task(&restore_request)
        .expect("restore can start after retention releases shared scope");
    restore_service
        .abort_queued_save_restore_task(&restore_request, &next.task_id)
        .expect("cleanup queued restore");
}

#[test]
fn task_creation_failure_or_panic_releases_shared_maintenance_scope() {
    let task_manager = Arc::new(TaskManager::new());
    let maintenance_registry = Arc::new(SaveProfileMaintenanceScopeRegistry::default());
    let backup_registry = Arc::new(SaveBackupTaskScopeRegistry::with_maintenance_registry(
        Arc::clone(&maintenance_registry),
    ));
    let restore_registry = Arc::new(SaveRestoreTaskScopeRegistry::with_maintenance_registry(
        maintenance_registry,
    ));
    let restore_request = sample_request();
    let backup_request = StartSaveBackupTaskRequest {
        game_id: restore_request.game_id.clone(),
        profile_id: restore_request.profile_id.clone(),
        trigger: SaveBackupTrigger::Manual,
        note: None,
        scheduler_lease_owner: None,
    };

    assert_eq!(
        backup_registry.reserve_task(&backup_request, || {
            Err(TaskManagerError::TaskStoreUnavailable)
        }),
        Err(TaskManagerError::TaskStoreUnavailable)
    );
    let backup_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = backup_registry.reserve_task(
            &backup_request,
            || -> std::result::Result<TaskSnapshot, TaskManagerError> {
                panic!("injected backup task creation panic")
            },
        );
    }));
    assert!(backup_panic.is_err());

    let restore_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = restore_registry.reserve_task(
            &restore_request,
            || -> std::result::Result<TaskSnapshot, TaskManagerError> {
                panic!("injected restore task creation panic")
            },
        );
    }));
    assert!(restore_panic.is_err());
    assert!(!restore_registry
        .has_active_task()
        .expect("restore registry remains usable after panic"));

    let restore_service =
        SaveRestoreTaskService::with_scope_registry(task_manager, Arc::clone(&restore_registry));
    let next = restore_service
        .start_save_restore_task(&restore_request)
        .expect("shared maintenance scope is released after failure and panic");
    restore_service
        .abort_queued_save_restore_task(&restore_request, &next.task_id)
        .expect("cleanup queued restore");
}

#[test]
fn exit_admission_stays_closed_only_after_the_active_restore_releases_its_scope() {
    let harness = Harness::success();
    let service = harness.service();
    let started = service
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    assert!(!harness
        .scope_registry
        .begin_exit_if_idle()
        .expect("active restore blocks exit admission"));
    assert!(harness
        .scope_registry
        .has_active_task()
        .expect("scope status is available"));

    harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect("restore completes");

    assert!(harness
        .scope_registry
        .begin_exit_if_idle()
        .expect("idle restore scope can close admission"));
    assert!(matches!(
        service.start_save_restore_task(&harness.request),
        Err(TaskManagerError::TaskCreationBlocked {
            kind: hmm_app::TaskKind::SaveRestore,
        })
    ));
}

#[test]
fn runner_prepares_safety_backup_before_waiting_for_shared_profile_write_lock() {
    let harness = Harness::success();
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");
    let write_lock = harness
        .write_locks
        .lock_for(&harness.request.game_id, &harness.request.profile_id);
    let guard = write_lock.lock().expect("hold shared write lock");
    let runner = Arc::clone(&harness.runner);
    let request = harness.request.clone();
    let task_id = started.task_id.clone();
    let join = thread::spawn(move || runner.run_save_restore_task(&task_id, request));

    harness.file_system.wait_for_prepare();
    harness.backup.wait_for_backup();
    assert_eq!(harness.backup.triggers(), [SaveBackupTrigger::PreRestore]);
    assert_eq!(harness.file_system.commit_count(), 0);
    drop(guard);

    join.join()
        .expect("runner thread")
        .expect("restore completes after shared lock is released");
    assert_eq!(harness.file_system.commit_count(), 1);
}

#[test]
fn runner_emits_cancelled_terminal_after_cancellation_while_waiting_for_write_lock() {
    let harness = Harness::success();
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");
    let write_lock = harness
        .write_locks
        .lock_for(&harness.request.game_id, &harness.request.profile_id);
    let guard = write_lock.lock().expect("hold shared write lock");
    let runner = Arc::clone(&harness.runner);
    let request = harness.request.clone();
    let task_id = started.task_id.clone();
    let join = thread::spawn(move || runner.run_save_restore_task(&task_id, request));

    harness.backup.wait_for_backup();
    harness
        .task_manager
        .cancel_task(&started.task_id)
        .expect("cancel waiting restore");
    drop(guard);

    let error = join
        .join()
        .expect("runner thread")
        .expect_err("cancelled restore stops before commit");
    assert_eq!(error.error_code, "save_restore_cancelled");
    let cancelled = error.events.last().expect("cancelled terminal event");
    assert_eq!(cancelled.status, TaskStatus::Cancelled);
    assert_eq!(cancelled.phase, "save_restore.cancelled");
    assert_eq!(harness.file_system.commit_count(), 0);
    assert_eq!(harness.file_system.discard_count(), 1);
    assert_eq!(
        harness.transactions.statuses().last(),
        Some(&SaveRestoreTransactionStatus::Failed)
    );
}

#[test]
fn cancellation_persistence_failure_retains_prepared_evidence_and_requires_recovery() {
    let harness = Harness::success();
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");
    let write_lock = harness
        .write_locks
        .lock_for(&harness.request.game_id, &harness.request.profile_id);
    let guard = write_lock.lock().expect("hold shared write lock");
    let runner = Arc::clone(&harness.runner);
    let request = harness.request.clone();
    let task_id = started.task_id.clone();
    let join = thread::spawn(move || runner.run_save_restore_task(&task_id, request));

    harness.backup.wait_for_backup();
    harness
        .transactions
        .fail_on_status(SaveRestoreTransactionStatus::Failed);
    harness
        .task_manager
        .cancel_task(&started.task_id)
        .expect("cancel waiting restore");
    drop(guard);

    let error = join
        .join()
        .expect("runner thread")
        .expect_err("cancel terminal persistence failure requires recovery");
    assert_eq!(error.error_code, "save_restore_transaction_unavailable");
    let terminal = error
        .events
        .last()
        .expect("recovery-required terminal event");
    assert_eq!(terminal.status, TaskStatus::Failed);
    assert_eq!(terminal.phase, "save_restore.recovery_required");
    assert_eq!(
        terminal.error.as_deref(),
        Some("save_restore_transaction_unavailable")
    );
    assert_eq!(harness.file_system.commit_count(), 0);
    assert_eq!(harness.file_system.discard_count(), 0);
    assert_eq!(
        harness.transactions.statuses().last(),
        Some(&SaveRestoreTransactionStatus::PreRestoreCompleted)
    );
    assert!(harness
        .audit
        .error_codes()
        .contains(&"save_restore_transaction_unavailable".to_owned()));
}

#[test]
fn cancellation_requested_inside_commit_barrier_cannot_reclassify_success() {
    let harness = Harness::success();
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");
    harness
        .file_system
        .cancel_during_commit(Arc::clone(&harness.task_manager), started.task_id.clone());

    harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect("commit barrier preserves success");

    assert!(harness.file_system.commit_cancel_was_rejected());
    assert_eq!(
        harness.task_manager.task_status(&started.task_id),
        Some(TaskStatus::Completed)
    );
}

#[test]
fn durable_restore_success_is_not_reclassified_when_task_projection_fails() {
    let harness = Harness::success();
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");
    harness
        .file_system
        .fail_task_during_commit(Arc::clone(&harness.task_manager), started.task_id.clone());

    let events = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect("durable restore remains successful");

    assert_eq!(
        harness.transactions.statuses().last(),
        Some(&SaveRestoreTransactionStatus::Completed)
    );
    assert_eq!(harness.file_system.commit_count(), 1);
    assert_eq!(harness.file_system.finalize_count(), 1);
    assert_eq!(
        harness.task_manager.task_status(&started.task_id),
        Some(TaskStatus::Failed),
        "the injected volatile projection fault remains visible"
    );
    let completed = events.last().expect("completed event");
    assert_eq!(completed.status, TaskStatus::Completed);
    assert_eq!(completed.phase, "save_restore.completed");
    assert_eq!(
        completed.error.as_deref(),
        Some("save_restore_evidence_degraded")
    );
    assert!(!events
        .iter()
        .any(|event| event.phase == "save_restore.failed"));
    assert!(!harness
        .audit
        .results()
        .iter()
        .any(|result| result == "failure"));
}

#[test]
fn durable_restore_success_reports_audit_evidence_degradation() {
    let harness = Harness::success();
    harness.audit.set_fail(true);
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    let events = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect("audit failure cannot reclassify a durable restore");

    assert_eq!(
        harness.transactions.statuses().last(),
        Some(&SaveRestoreTransactionStatus::Completed)
    );
    assert_eq!(
        harness.task_manager.task_status(&started.task_id),
        Some(TaskStatus::Completed)
    );
    let completed = events.last().expect("completed event");
    assert_eq!(completed.status, TaskStatus::Completed);
    assert_eq!(
        completed.error.as_deref(),
        Some("save_restore_evidence_degraded")
    );
}

#[test]
fn committed_persistence_failure_keeps_rollback_evidence_and_blocks_completion() {
    let harness = Harness::success();
    harness
        .transactions
        .fail_on_status(SaveRestoreTransactionStatus::Committed);
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    let error = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect_err("durable commit fact failure must remain incomplete");

    assert_eq!(error.error_code, "save_restore_transaction_unavailable");
    assert_eq!(
        harness.transactions.statuses().last(),
        Some(&SaveRestoreTransactionStatus::Committing)
    );
    assert_eq!(harness.file_system.commit_count(), 1);
    assert_eq!(harness.file_system.finalize_count(), 0);
}

#[test]
fn completed_persistence_failure_keeps_committed_fact_and_real_audit_identity() {
    let harness = Harness::success();
    harness
        .transactions
        .fail_on_status(SaveRestoreTransactionStatus::Completed);
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    let error = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect_err("durable completion failure must remain incomplete");

    assert_eq!(error.error_code, "save_restore_transaction_unavailable");
    assert_eq!(
        harness.transactions.statuses().last(),
        Some(&SaveRestoreTransactionStatus::Committed)
    );
    assert_eq!(harness.file_system.finalize_count(), 1);
    let transaction_id = harness.transactions.transaction_id();
    assert!(harness
        .audit
        .transaction_ids()
        .iter()
        .any(|value| value == &transaction_id));
    assert!(!harness
        .audit
        .transaction_ids()
        .contains(&"unavailable".to_owned()));
}

#[test]
fn committed_restore_finalize_failure_requires_recovery_and_is_not_completed() {
    let harness = Harness::success();
    harness
        .file_system
        .set_finalize_error(SaveRestoreFinalizeError::CleanupFailed);
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    let error = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect_err("unsafe finalization must require recovery");

    assert_eq!(error.error_code, "save_restore_recovery_cleanup_failed");
    assert_eq!(
        harness.transactions.statuses().last(),
        Some(&SaveRestoreTransactionStatus::RecoveryRequired)
    );
    assert_eq!(harness.file_system.commit_count(), 1);
    assert_eq!(harness.file_system.finalize_count(), 1);
    let terminal = error.events.last().expect("recovery-required event");
    assert_eq!(terminal.status, TaskStatus::Failed);
    assert_eq!(terminal.phase, "save_restore.recovery_required");
    assert_eq!(
        terminal.error.as_deref(),
        Some("save_restore_recovery_cleanup_failed")
    );
    assert!(harness
        .audit
        .error_codes()
        .contains(&"save_restore_recovery_cleanup_failed".to_owned()));
}

#[test]
fn rolled_back_terminal_is_durable_before_finalize() {
    let harness = Harness::success();
    harness
        .file_system
        .set_commit_error(SaveRestoreCommitError::RolledBack);
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    let error = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect_err("rolled back restore remains a failed task");

    assert_eq!(error.error_code, "save_restore_rolled_back");
    assert_eq!(
        harness.transactions.statuses().last(),
        Some(&SaveRestoreTransactionStatus::RolledBack)
    );
    assert_eq!(harness.file_system.finalize_count(), 1);
}

#[test]
fn rolled_back_finalize_failure_is_projected_as_warning() {
    let harness = Harness::success();
    harness
        .file_system
        .set_commit_error(SaveRestoreCommitError::RolledBack);
    harness
        .file_system
        .set_finalize_error(SaveRestoreFinalizeError::CleanupFailed);
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    let error = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect_err("rolled back restore remains failed with cleanup warning");

    assert_eq!(error.error_code, "save_restore_rolled_back");
    assert_eq!(
        harness.transactions.statuses().last(),
        Some(&SaveRestoreTransactionStatus::RolledBack)
    );
    let terminal = error.events.last().expect("failed terminal event");
    assert_eq!(terminal.status, TaskStatus::Failed);
    assert_eq!(terminal.phase, "save_restore.failed");
    assert_eq!(
        terminal.message.as_deref(),
        Some("save_restore_recovery_cleanup_failed")
    );
    assert!(harness
        .audit
        .error_codes()
        .contains(&"save_restore_recovery_cleanup_failed".to_owned()));
}

#[test]
fn rolled_back_persistence_failure_does_not_finalize() {
    let harness = Harness::success();
    harness
        .file_system
        .set_commit_error(SaveRestoreCommitError::RolledBack);
    harness
        .transactions
        .fail_on_status(SaveRestoreTransactionStatus::RolledBack);
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    let error = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect_err("rolled back terminal write failure stays incomplete");

    assert_eq!(error.error_code, "save_restore_transaction_unavailable");
    let terminal = error
        .events
        .last()
        .expect("recovery-required terminal event");
    assert_eq!(terminal.status, TaskStatus::Failed);
    assert_eq!(terminal.phase, "save_restore.recovery_required");
    assert_eq!(
        terminal.error.as_deref(),
        Some("save_restore_transaction_unavailable")
    );
    assert_eq!(
        harness.transactions.statuses().last(),
        Some(&SaveRestoreTransactionStatus::Committing)
    );
    assert_eq!(harness.file_system.finalize_count(), 0);
}

#[test]
fn recovery_required_never_finalizes_evidence() {
    let harness = Harness::success();
    harness
        .file_system
        .set_commit_error(SaveRestoreCommitError::RecoveryRequired);
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    let error = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect_err("recovery required must be explicit");

    assert_eq!(error.error_code, "save_restore_recovery_required");
    assert_eq!(
        harness.transactions.statuses().last(),
        Some(&SaveRestoreTransactionStatus::RecoveryRequired)
    );
    assert_eq!(harness.file_system.finalize_count(), 0);
    assert!(error
        .events
        .iter()
        .any(|event| event.phase == "save_restore.recovery_required"));
}

#[test]
fn pre_restore_backup_identity_mismatch_fails_closed() {
    let harness = Harness::success();
    harness.backup.set_profile_id("other-profile");
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    let error = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect_err("mismatched safety backup must block commit");

    assert_eq!(error.error_code, "save_restore_pre_restore_backup_invalid");
    assert_eq!(harness.file_system.commit_count(), 0);
    assert_eq!(harness.file_system.finalize_count(), 0);
}

#[test]
fn revalidation_fact_drift_blocks_commit() {
    let initial = sample_context();
    let mut changed = initial.clone();
    changed.facts_digest = "sha256:changed".to_owned();
    let harness = Harness::with_validations([Ok(initial), Ok(changed)]);
    let started = harness
        .service()
        .start_save_restore_task(&harness.request)
        .expect("task starts");

    let error = harness
        .runner
        .run_save_restore_task(&started.task_id, harness.request.clone())
        .expect_err("facts changed under lock");

    assert_eq!(error.error_code, "save_restore_facts_changed");
    assert_eq!(harness.file_system.commit_count(), 0);
    assert_eq!(harness.validator.excluded_transaction_ids().len(), 1);
}

struct Harness {
    runner: Arc<SaveRestoreTaskRunner>,
    task_manager: Arc<TaskManager>,
    transactions: Arc<RecordingTransactions>,
    file_system: Arc<RecordingFileSystem>,
    backup: Arc<RecordingBackupExecutor>,
    audit: Arc<RecordingAudit>,
    validator: Arc<StaticValidator>,
    scope_registry: Arc<SaveRestoreTaskScopeRegistry>,
    write_locks: Arc<hmm_app::GameProfileWriteLockRegistry>,
    request: StartSaveRestoreRequest,
}

#[derive(Default)]
struct SaveThenRejectGameAdmission {
    calls: Mutex<Vec<CrossProcessWriteScopeKind>>,
}

impl SaveThenRejectGameAdmission {
    fn calls(&self) -> Vec<CrossProcessWriteScopeKind> {
        self.calls.lock().expect("admission calls").clone()
    }
}

impl CrossProcessWriteAdmission for SaveThenRejectGameAdmission {
    fn acquire(
        &self,
        scope: &CrossProcessWriteScope,
        _timeout: Duration,
        _cancellation: &dyn CancellationToken,
    ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>> {
        self.calls
            .lock()
            .expect("admission calls")
            .push(scope.kind());
        if scope.kind() == CrossProcessWriteScopeKind::GameProfileWrite {
            return Err(CrossProcessWriteAdmissionError::Busy);
        }
        Ok(Box::new(RecordingCrossProcessGuard {
            scope: scope.clone(),
        }))
    }
}

struct RecordingCrossProcessGuard {
    scope: CrossProcessWriteScope,
}

impl CrossProcessWriteGuard for RecordingCrossProcessGuard {
    fn scope(&self) -> &CrossProcessWriteScope {
        &self.scope
    }

    fn acquisition(&self) -> CrossProcessWriteAcquisition {
        CrossProcessWriteAcquisition::default()
    }
}

impl Harness {
    fn success() -> Self {
        Self::with_validations([Ok(sample_context())])
    }

    fn blocked(error: SaveRestorePreviewError) -> Self {
        Self::with_validations([Err(error)])
    }

    fn with_validations(
        validations: impl IntoIterator<
            Item = std::result::Result<SaveRestoreCommitContext, SaveRestorePreviewError>,
        >,
    ) -> Self {
        let task_manager = Arc::new(TaskManager::new());
        let transactions = Arc::new(RecordingTransactions::default());
        let file_system = Arc::new(RecordingFileSystem::default());
        let backup = Arc::new(RecordingBackupExecutor::default());
        let audit = Arc::new(RecordingAudit::default());
        let validator = Arc::new(StaticValidator::new(validations));
        let scope_registry = Arc::new(SaveRestoreTaskScopeRegistry::default());
        let write_locks = Arc::new(hmm_app::GameProfileWriteLockRegistry::default());
        let runner = Arc::new(SaveRestoreTaskRunner::with_scope_registry(
            task_manager.clone(),
            validator.clone(),
            file_system.clone(),
            transactions.clone(),
            backup.clone(),
            audit.clone(),
            Arc::new(FixedClock),
            write_locks.clone(),
            scope_registry.clone(),
        ));
        Self {
            runner,
            task_manager,
            transactions,
            file_system,
            backup,
            audit,
            validator,
            scope_registry,
            write_locks,
            request: sample_request(),
        }
    }

    fn service(&self) -> SaveRestoreTaskService {
        SaveRestoreTaskService::with_scope_registry(
            Arc::clone(&self.task_manager),
            Arc::clone(&self.scope_registry),
        )
    }
}

struct StaticValidator {
    validations:
        Mutex<VecDeque<std::result::Result<SaveRestoreCommitContext, SaveRestorePreviewError>>>,
    last: std::result::Result<SaveRestoreCommitContext, SaveRestorePreviewError>,
    excluded_transaction_ids: Mutex<Vec<String>>,
}

impl StaticValidator {
    fn new(
        validations: impl IntoIterator<
            Item = std::result::Result<SaveRestoreCommitContext, SaveRestorePreviewError>,
        >,
    ) -> Self {
        let validations = validations.into_iter().collect::<VecDeque<_>>();
        let last = validations.back().expect("at least one validation").clone();
        Self {
            validations: Mutex::new(validations),
            last,
            excluded_transaction_ids: Mutex::new(Vec::new()),
        }
    }

    fn next(&self) -> std::result::Result<SaveRestoreCommitContext, SaveRestorePreviewError> {
        self.validations
            .lock()
            .expect("validations")
            .pop_front()
            .unwrap_or_else(|| self.last.clone())
    }

    fn excluded_transaction_ids(&self) -> Vec<String> {
        self.excluded_transaction_ids
            .lock()
            .expect("excluded transaction ids")
            .clone()
    }
}

impl SaveRestoreCommitValidator for StaticValidator {
    fn validate_for_commit(
        &self,
        _request: StartSaveRestoreRequest,
    ) -> std::result::Result<SaveRestoreCommitContext, SaveRestorePreviewError> {
        self.next()
    }

    fn validate_for_commit_excluding_transaction(
        &self,
        _request: StartSaveRestoreRequest,
        transaction_id: &str,
    ) -> std::result::Result<SaveRestoreCommitContext, SaveRestorePreviewError> {
        self.excluded_transaction_ids
            .lock()
            .expect("excluded transaction ids")
            .push(transaction_id.to_owned());
        self.next()
    }
}

#[derive(Default)]
struct RecordingTransactions {
    values: Mutex<Vec<SaveRestoreTransaction>>,
    fail_status: Mutex<Option<SaveRestoreTransactionStatus>>,
}

impl RecordingTransactions {
    fn statuses(&self) -> Vec<SaveRestoreTransactionStatus> {
        self.values
            .lock()
            .expect("transactions")
            .iter()
            .map(|transaction| transaction.status)
            .collect()
    }

    fn fail_on_status(&self, status: SaveRestoreTransactionStatus) {
        *self.fail_status.lock().expect("fail status") = Some(status);
    }

    fn transaction_id(&self) -> String {
        self.values
            .lock()
            .expect("transactions")
            .first()
            .expect("transaction")
            .transaction_id
            .clone()
    }
}

impl SaveRestoreTransactionRepository for RecordingTransactions {
    fn save_transaction(&self, transaction: &SaveRestoreTransaction) -> Result<()> {
        if self
            .fail_status
            .lock()
            .expect("fail status")
            .is_some_and(|status| status == transaction.status)
        {
            anyhow::bail!("injected transaction persistence failure");
        }
        self.values
            .lock()
            .expect("transactions")
            .push(transaction.clone());
        Ok(())
    }

    fn get_transaction(&self, _transaction_id: &str) -> Result<Option<SaveRestoreTransaction>> {
        Ok(None)
    }

    fn has_incomplete_transaction_excluding(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
        _excluded_transaction_id: Option<&str>,
    ) -> Result<bool> {
        Ok(false)
    }
}

#[derive(Default)]
struct RecordingFileSystem {
    commits: Mutex<usize>,
    finalizes: Mutex<usize>,
    discards: Mutex<usize>,
    prepare_signal: (Mutex<bool>, Condvar),
    commit_error: Mutex<Option<SaveRestoreCommitError>>,
    finalize_error: Mutex<Option<SaveRestoreFinalizeError>>,
    cancel_and_fail_prepare: Mutex<Option<(Arc<TaskManager>, String)>>,
    cancel_during_commit: Mutex<Option<(Arc<TaskManager>, String)>>,
    fail_task_during_commit: Mutex<Option<(Arc<TaskManager>, String)>>,
    commit_cancel_rejected: Mutex<bool>,
}

impl RecordingFileSystem {
    fn commit_count(&self) -> usize {
        *self.commits.lock().expect("commits")
    }

    fn finalize_count(&self) -> usize {
        *self.finalizes.lock().expect("finalizes")
    }

    fn discard_count(&self) -> usize {
        *self.discards.lock().expect("discards")
    }

    fn wait_for_prepare(&self) {
        let (ready, signal) = &self.prepare_signal;
        let prepared = ready.lock().expect("prepare signal");
        let (prepared, wait) = signal
            .wait_timeout_while(prepared, Duration::from_secs(2), |prepared| !*prepared)
            .expect("wait for prepare");
        assert!(
            !wait.timed_out() && *prepared,
            "restore prepare did not finish"
        );
    }

    fn set_commit_error(&self, error: SaveRestoreCommitError) {
        *self.commit_error.lock().expect("commit error") = Some(error);
    }

    fn set_finalize_error(&self, error: SaveRestoreFinalizeError) {
        *self.finalize_error.lock().expect("finalize error") = Some(error);
    }

    fn cancel_and_fail_prepare(&self, task_manager: Arc<TaskManager>, task_id: String) {
        *self
            .cancel_and_fail_prepare
            .lock()
            .expect("cancel and fail prepare") = Some((task_manager, task_id));
    }

    fn cancel_during_commit(&self, task_manager: Arc<TaskManager>, task_id: String) {
        *self
            .cancel_during_commit
            .lock()
            .expect("cancel during commit") = Some((task_manager, task_id));
    }

    fn fail_task_during_commit(&self, task_manager: Arc<TaskManager>, task_id: String) {
        *self
            .fail_task_during_commit
            .lock()
            .expect("fail task during commit") = Some((task_manager, task_id));
    }

    fn commit_cancel_was_rejected(&self) -> bool {
        *self
            .commit_cancel_rejected
            .lock()
            .expect("commit cancel result")
    }
}

impl SaveRestoreFileSystem for RecordingFileSystem {
    fn prepare_restore(
        &self,
        _request: SaveRestorePrepareRequest,
    ) -> std::result::Result<PreparedSaveRestore, SaveRestorePrepareError> {
        if let Some((task_manager, task_id)) = self
            .cancel_and_fail_prepare
            .lock()
            .expect("cancel and fail prepare")
            .take()
        {
            task_manager
                .cancel_task(&task_id)
                .expect("inject volatile cancellation projection");
            return Err(SaveRestorePrepareError::TargetUnavailable);
        }
        let (ready, signal) = &self.prepare_signal;
        *ready.lock().expect("prepare signal") = true;
        signal.notify_all();
        Ok(PreparedSaveRestore {
            prepared_id: "prepared-1".to_owned(),
            evidence_digest: "sha256:evidence".to_owned(),
            file_count: 1,
            total_uncompressed_bytes: 36,
        })
    }

    fn discard_prepared(&self, _prepared_id: &str) {
        *self.discards.lock().expect("discards") += 1;
    }

    fn commit_restore(
        &self,
        _request: SaveRestoreCommitRequest,
    ) -> std::result::Result<SaveRestoreCommitResult, SaveRestoreCommitError> {
        *self.commits.lock().expect("commits") += 1;
        if let Some((task_manager, task_id)) = self
            .cancel_during_commit
            .lock()
            .expect("cancel during commit")
            .take()
        {
            *self
                .commit_cancel_rejected
                .lock()
                .expect("commit cancel result") = task_manager.cancel_task(&task_id).is_err();
        }
        if let Some((task_manager, task_id)) = self
            .fail_task_during_commit
            .lock()
            .expect("fail task during commit")
            .take()
        {
            task_manager
                .fail_task(&task_id)
                .expect("inject volatile task projection failure");
        }
        if let Some(error) = self.commit_error.lock().expect("commit error").take() {
            return Err(error);
        }
        Ok(SaveRestoreCommitResult {
            restored_file_count: 1,
            rollback_performed: false,
        })
    }

    fn finalize_restore(
        &self,
        _request: SaveRestoreFinalizeRequest,
    ) -> std::result::Result<(), SaveRestoreFinalizeError> {
        *self.finalizes.lock().expect("finalizes") += 1;
        if let Some(error) = self.finalize_error.lock().expect("finalize error").take() {
            return Err(error);
        }
        Ok(())
    }
}

#[derive(Default)]
struct RecordingBackupExecutor {
    fail: Mutex<bool>,
    requests: Mutex<Vec<SaveBackupTrigger>>,
    notes: Mutex<Vec<Option<String>>>,
    profile_id: Mutex<Option<String>>,
    backup_signal: (Mutex<bool>, Condvar),
}

impl RecordingBackupExecutor {
    fn triggers(&self) -> Vec<SaveBackupTrigger> {
        self.requests.lock().expect("backup requests").clone()
    }

    fn notes(&self) -> Vec<Option<String>> {
        self.notes.lock().expect("backup notes").clone()
    }

    fn set_fail(&self, fail: bool) {
        *self.fail.lock().expect("backup failure") = fail;
    }

    fn set_profile_id(&self, profile_id: &str) {
        *self.profile_id.lock().expect("backup profile id") = Some(profile_id.to_owned());
    }

    fn wait_for_backup(&self) {
        let (ready, signal) = &self.backup_signal;
        let completed = ready.lock().expect("backup signal");
        let (completed, wait) = signal
            .wait_timeout_while(completed, Duration::from_secs(2), |completed| !*completed)
            .expect("wait for backup");
        assert!(
            !wait.timed_out() && *completed,
            "pre-restore backup did not finish before the write lock"
        );
    }
}

impl SaveBackupExecutor for RecordingBackupExecutor {
    fn create_backup(
        &self,
        request: CreateSaveBackupRequest,
        trigger: SaveBackupTrigger,
    ) -> std::result::Result<CreateSaveBackupResult, SaveBackupError> {
        self.requests.lock().expect("backup requests").push(trigger);
        self.notes.lock().expect("backup notes").push(request.note);
        let (ready, signal) = &self.backup_signal;
        *ready.lock().expect("backup signal") = true;
        signal.notify_all();
        if *self.fail.lock().expect("backup failure") {
            return Err(SaveBackupError::HistoryUnavailable);
        }
        Ok(CreateSaveBackupResult {
            summary: SaveBackupSummary {
                backup_id: "pre-restore-1".to_owned(),
                game_id: GameId::mhw(),
                profile_id: ProfileId::new(
                    self.profile_id
                        .lock()
                        .expect("backup profile id")
                        .clone()
                        .unwrap_or_else(|| "default".to_owned()),
                ),
                trigger: SaveBackupTrigger::PreRestore,
                status: SaveBackupStatus::Completed,
                archive_file_name: "pre-restore-1.zip".to_owned(),
                manifest_file_name: "pre-restore-1.manifest.json".to_owned(),
                archive_size_bytes: 36,
                retention_released_bytes: 0,
                archive_sha256: "sha256:pre".to_owned(),
                file_count: 1,
                created_at: 2,
                source_path_label: Some("fixture".to_owned()),
                source_path_hash: "sha256:source".to_owned(),
                backup_directory: custom_directory("C:/HMMFixtures/backup"),
                notes: None,
            },
            warnings: Vec::new(),
            retention_report: None,
        })
    }
}

#[derive(Default)]
struct RecordingAudit {
    events: Mutex<Vec<AuditLogEvent>>,
    fail: Mutex<bool>,
}

impl RecordingAudit {
    fn set_fail(&self, fail: bool) {
        *self.fail.lock().expect("audit failure") = fail;
    }

    fn transaction_ids(&self) -> Vec<String> {
        self.events
            .lock()
            .expect("audit events")
            .iter()
            .filter_map(|event| event.fields.get("transaction_id").cloned())
            .collect()
    }

    fn results(&self) -> Vec<String> {
        self.events
            .lock()
            .expect("audit events")
            .iter()
            .map(|event| event.result.clone())
            .collect()
    }

    fn error_codes(&self) -> Vec<String> {
        self.events
            .lock()
            .expect("audit events")
            .iter()
            .filter_map(|event| event.fields.get("error_code").cloned())
            .collect()
    }
}

impl AuditLogWriter for RecordingAudit {
    fn record(&self, event: AuditLogEvent) -> Result<()> {
        if *self.fail.lock().expect("audit failure") {
            anyhow::bail!("injected audit failure");
        }
        self.events.lock().expect("audit events").push(event);
        Ok(())
    }
}

struct FixedClock;

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(100)
    }
}

fn sample_request() -> StartSaveRestoreRequest {
    StartSaveRestoreRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        backup_id: "backup-1".to_owned(),
        preview_token: "fixture-token".to_owned(),
        confirmed: true,
        confirmed_without_pre_restore: false,
    }
}

fn sample_context() -> SaveRestoreCommitContext {
    SaveRestoreCommitContext {
        request: sample_request(),
        summary: SaveBackupSummary {
            backup_id: "backup-1".to_owned(),
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            trigger: SaveBackupTrigger::Manual,
            status: SaveBackupStatus::Completed,
            archive_file_name: "backup-1.zip".to_owned(),
            manifest_file_name: "backup-1.manifest.json".to_owned(),
            archive_size_bytes: 36,
            retention_released_bytes: 0,
            archive_sha256: "sha256:archive".to_owned(),
            file_count: 1,
            created_at: 1,
            source_path_label: Some("fixture".to_owned()),
            source_path_hash: "sha256:source".to_owned(),
            backup_directory: custom_directory("C:/HMMFixtures/backup"),
            notes: None,
        },
        settings: ProfileSaveSettings {
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
            steam_account: None,
            pre_restore_backup_enabled: true,
            updated_at: 1,
        },
        validated_source: hmm_ports::ValidatedSaveRestoreSource {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            backup_id: "backup-1".to_owned(),
            evidence_digest: "sha256:evidence".to_owned(),
            file_count: 1,
            total_uncompressed_bytes: 36,
        },
        facts_digest: "sha256:facts".to_owned(),
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
