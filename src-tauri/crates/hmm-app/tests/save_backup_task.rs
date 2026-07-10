use anyhow::Result;
use hmm_app::{
    CreateSaveBackupRequest, CreateSaveBackupResult, SaveBackupError, SaveBackupExecutor,
    SaveBackupTaskRunner, SaveBackupTaskService, SaveBackupWarning, StartSaveBackupTaskRequest,
    TaskKind, TaskManager, TaskManagerError, TaskStatus,
};
use hmm_core::{
    GameId, ProfileId, SaveBackupBackgroundProtectionStatus,
    SaveBackupSchedulerLeaseRenewalRequest, SaveBackupSchedulerLeaseRequest,
    SaveBackupSchedulerState, SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
    SaveBackupWorkerHeartbeat,
};
use hmm_ports::{AppClock, AuditLogEvent, AuditLogWriter, SaveBackupSchedulerStateRepository};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
fn auto_backup_renews_lease_while_executor_is_blocked() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let clock = Arc::new(ControllableClock::new(0));
    let scheduler_state = Arc::new(RecordingSchedulerStateRepository::with_state(
        SaveBackupSchedulerState {
            lease_expires_at: Some(300_000),
            ..sample_scheduler_state("auto-lease")
        },
    ));
    let executor = Arc::new(BlockingSaveBackupExecutor::new());
    let runner = Arc::new(
        SaveBackupTaskRunner::with_scope_registry_and_scheduler_state_and_keepalive_interval(
            Arc::clone(&task_manager),
            executor.clone(),
            Arc::new(RecordingAuditLogWriter::default()),
            clock.clone(),
            Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default()),
            scheduler_state.clone(),
            Duration::from_millis(1),
        ),
    );
    let task_id = task.task_id.clone();
    let run =
        thread::spawn(move || runner.run_save_backup_task(&task_id, auto_request("auto-lease")));

    executor.wait_until_started();
    clock.set(299_999);
    wait_until(|| {
        scheduler_state
            .latest_state()
            .is_some_and(|state| state.lease_expires_at == Some(599_999))
    });
    clock.set(300_001);

    let competing = scheduler_state
        .acquire_due_lease(SaveBackupSchedulerLeaseRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            lease_owner: "competing-worker".to_owned(),
            lease_expires_at: 600_001,
            now_unix_millis: 300_001,
            last_checked_at: Some(300_001),
            next_due_at: Some(400_000),
        })
        .expect("competing due check is not fatal");
    assert!(competing.is_none());

    executor.release();
    assert!(run.join().expect("task thread does not panic").is_ok());
}

#[test]
fn auto_backup_does_not_invoke_executor_when_initial_lease_renewal_fails() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let scheduler_state = Arc::new(RecordingSchedulerStateRepository::with_state(
        sample_scheduler_state("auto-lease"),
    ));
    scheduler_state.set_renewal_available(false);
    let executor = Arc::new(RecordingSaveBackupExecutor::ok(sample_result()));
    let audit_log = Arc::new(RecordingAuditLogWriter::default());
    let runner =
        SaveBackupTaskRunner::with_scope_registry_and_scheduler_state_and_keepalive_interval(
            Arc::clone(&task_manager),
            executor.clone(),
            audit_log.clone(),
            Arc::new(FixedClock),
            Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default()),
            scheduler_state.clone(),
            Duration::from_millis(1),
        );

    let error = runner
        .run_save_backup_task(&task.task_id, auto_request("auto-lease"))
        .expect_err("unconfirmed lease must stop the task before backup execution");

    assert!(executor.take_requests().is_empty());
    assert_eq!(
        error.events.last().and_then(|event| event.error.as_deref()),
        Some("save_backup_failed:save_backup_scheduler_lease_unavailable")
    );
    let audit_events = audit_log.take_events();
    assert_eq!(audit_events.len(), 1);
    assert_eq!(audit_events[0].result, "failure");
    assert_eq!(
        audit_events[0].fields.get("error_code").map(String::as_str),
        Some("save_backup_scheduler_lease_unavailable")
    );
    let state = scheduler_state
        .latest_state()
        .expect("scheduler state exists");
    assert_eq!(state.last_success_at, None);
    assert_eq!(
        state.last_error_code.as_deref(),
        Some("save_backup_scheduler_lease_unavailable")
    );
}

#[test]
fn auto_backup_fails_when_keepalive_cannot_renew_lease() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let scheduler_state = Arc::new(RecordingSchedulerStateRepository::with_state(
        sample_scheduler_state("auto-lease"),
    ));
    let executor = Arc::new(BlockingSaveBackupExecutor::new());
    let audit_log = Arc::new(RecordingAuditLogWriter::default());
    let runner = Arc::new(
        SaveBackupTaskRunner::with_scope_registry_and_scheduler_state_and_keepalive_interval(
            Arc::clone(&task_manager),
            executor.clone(),
            audit_log.clone(),
            Arc::new(FixedClock),
            Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default()),
            scheduler_state.clone(),
            Duration::from_millis(1),
        ),
    );
    let task_id = task.task_id.clone();
    let run =
        thread::spawn(move || runner.run_save_backup_task(&task_id, auto_request("auto-lease")));

    executor.wait_until_started();
    let initial_renewal_calls = scheduler_state.renewal_calls();
    scheduler_state.set_renewal_available(false);
    wait_until(|| scheduler_state.renewal_calls() > initial_renewal_calls);

    executor.release();
    let error = run
        .join()
        .expect("task thread does not panic")
        .expect_err("a lost scheduler lease must prevent a successful task result");

    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(TaskStatus::Failed)
    );
    assert_eq!(
        error.events.last().and_then(|event| event.error.as_deref()),
        Some("save_backup_failed:save_backup_scheduler_lease_unavailable")
    );
    let audit_events = audit_log.take_events();
    assert_eq!(audit_events.len(), 1);
    assert_eq!(audit_events[0].result, "failure");
    assert_eq!(
        audit_events[0].fields.get("error_code").map(String::as_str),
        Some("save_backup_scheduler_lease_unavailable")
    );
    let state = scheduler_state
        .latest_state()
        .expect("scheduler state exists");
    assert_eq!(state.last_success_at, None);
    assert_eq!(
        state.last_error_code.as_deref(),
        Some("save_backup_scheduler_lease_unavailable")
    );
}

#[test]
fn auto_backup_without_lease_owner_fails_before_invoking_executor() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let scheduler_state = Arc::new(RecordingSchedulerStateRepository::with_state(
        sample_scheduler_state("auto-lease"),
    ));
    let executor = Arc::new(RecordingSaveBackupExecutor::ok(sample_result()));
    let audit_log = Arc::new(RecordingAuditLogWriter::default());
    let runner = SaveBackupTaskRunner::with_scope_registry_and_scheduler_state(
        Arc::clone(&task_manager),
        executor.clone(),
        audit_log.clone(),
        Arc::new(FixedClock),
        Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default()),
        scheduler_state.clone(),
    );
    let mut request = auto_request("unused-owner");
    request.scheduler_lease_owner = None;

    let error = runner
        .run_save_backup_task(&task.task_id, request)
        .expect_err("auto backup without a persisted lease owner must fail closed");

    assert!(executor.take_requests().is_empty());
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(TaskStatus::Failed)
    );
    assert_eq!(
        error.events.last().and_then(|event| event.error.as_deref()),
        Some("save_backup_failed:save_backup_scheduler_lease_unavailable")
    );
    assert_eq!(scheduler_state.renewal_calls(), 0);
    assert!(scheduler_state.release_calls().is_empty());
    let audit_events = audit_log.take_events();
    assert_eq!(audit_events.len(), 1);
    assert_eq!(audit_events[0].result, "failure");
    assert_eq!(
        audit_events[0].fields.get("error_code").map(String::as_str),
        Some("save_backup_scheduler_lease_unavailable")
    );
    let state = scheduler_state
        .latest_state()
        .expect("scheduler state exists");
    assert_eq!(state.last_success_at, None);
    assert_eq!(
        state.last_error_code.as_deref(),
        Some("save_backup_scheduler_lease_unavailable")
    );
}

#[test]
fn auto_backup_with_blank_lease_owner_fails_without_renew_or_release() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let scheduler_state = Arc::new(RecordingSchedulerStateRepository::with_state(
        sample_scheduler_state("auto-lease"),
    ));
    let executor = Arc::new(RecordingSaveBackupExecutor::ok(sample_result()));
    let runner = SaveBackupTaskRunner::with_scope_registry_and_scheduler_state(
        Arc::clone(&task_manager),
        executor.clone(),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default()),
        scheduler_state.clone(),
    );

    let error = runner
        .run_save_backup_task(&task.task_id, auto_request("   "))
        .expect_err("blank scheduler lease ownership is invalid");

    assert!(executor.take_requests().is_empty());
    assert_eq!(scheduler_state.renewal_calls(), 0);
    assert!(scheduler_state.release_calls().is_empty());
    assert_eq!(
        error.events.last().and_then(|event| event.error.as_deref()),
        Some("save_backup_failed:save_backup_scheduler_lease_unavailable")
    );
}

#[test]
fn oversized_keepalive_interval_is_bounded_below_scheduler_lease_ttl() {
    let runner =
        SaveBackupTaskRunner::with_scope_registry_and_scheduler_state_and_keepalive_interval(
            Arc::new(TaskManager::new()),
            Arc::new(RecordingSaveBackupExecutor::ok(sample_result())),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default()),
            Arc::new(RecordingSchedulerStateRepository::with_state(
                sample_scheduler_state("auto-lease"),
            )),
            Duration::MAX,
        );

    let interval = runner.scheduler_lease_keepalive_interval();
    assert!(interval >= Duration::from_millis(1));
    assert!(interval < Duration::from_millis(300_000));
}

#[test]
fn auto_backup_fails_when_expired_lease_is_acquired_by_competing_owner_before_completion() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let clock = Arc::new(ControllableClock::new(0));
    let scheduler_state = Arc::new(RecordingSchedulerStateRepository::with_state(
        SaveBackupSchedulerState {
            lease_expires_at: Some(300_000),
            ..sample_scheduler_state("auto-lease")
        },
    ));
    let executor = Arc::new(BlockingSaveBackupExecutor::new());
    let audit_log = Arc::new(RecordingAuditLogWriter::default());
    let runner = Arc::new(
        SaveBackupTaskRunner::with_scope_registry_and_scheduler_state_and_keepalive_interval(
            Arc::clone(&task_manager),
            executor.clone(),
            audit_log.clone(),
            clock.clone(),
            Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default()),
            scheduler_state.clone(),
            Duration::from_secs(60),
        ),
    );
    let task_id = task.task_id.clone();
    let run =
        thread::spawn(move || runner.run_save_backup_task(&task_id, auto_request("auto-lease")));

    executor.wait_until_started();
    clock.set(300_001);
    let competing = scheduler_state
        .acquire_due_lease(SaveBackupSchedulerLeaseRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            lease_owner: "competing-worker".to_owned(),
            lease_expires_at: 600_001,
            now_unix_millis: 300_001,
            last_checked_at: Some(300_001),
            next_due_at: Some(400_000),
        })
        .expect("competing owner can inspect the expired lease");
    assert!(competing.is_some());

    executor.release();
    let error = run
        .join()
        .expect("task thread does not panic")
        .expect_err("ownership lost before completion must fail closed");

    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(TaskStatus::Failed)
    );
    assert_eq!(
        error.events.last().and_then(|event| event.error.as_deref()),
        Some("save_backup_failed:save_backup_scheduler_lease_unavailable")
    );
    let audit_events = audit_log.take_events();
    assert_eq!(audit_events.len(), 1);
    assert_eq!(audit_events[0].result, "failure");
    assert_eq!(
        audit_events[0].fields.get("error_code").map(String::as_str),
        Some("save_backup_scheduler_lease_unavailable")
    );
    let state = scheduler_state
        .latest_state()
        .expect("scheduler state exists");
    assert_eq!(state.lease_owner.as_deref(), Some("competing-worker"));
    assert_eq!(state.last_success_at, None);
    assert_eq!(
        state.last_error_code.as_deref(),
        Some("save_backup_scheduler_lease_unavailable")
    );
}

#[test]
fn auto_backup_fails_when_keepalive_thread_panics() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let clock = Arc::new(PanickingKeepaliveClock::default());
    let scheduler_state = Arc::new(RecordingSchedulerStateRepository::with_state(
        sample_scheduler_state("auto-lease"),
    ));
    let executor = Arc::new(BlockingSaveBackupExecutor::new());
    let audit_log = Arc::new(RecordingAuditLogWriter::default());
    let runner = Arc::new(
        SaveBackupTaskRunner::with_scope_registry_and_scheduler_state_and_keepalive_interval(
            Arc::clone(&task_manager),
            executor.clone(),
            audit_log.clone(),
            clock.clone(),
            Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default()),
            scheduler_state.clone(),
            Duration::from_millis(1),
        ),
    );
    let task_id = task.task_id.clone();
    let run =
        thread::spawn(move || runner.run_save_backup_task(&task_id, auto_request("auto-lease")));

    executor.wait_until_started();
    wait_until(|| clock.keepalive_panicked.load(Ordering::Acquire));
    executor.release();
    let error = run
        .join()
        .expect("task runner contains the keepalive panic")
        .expect_err("keepalive join failure must fail closed");

    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(TaskStatus::Failed)
    );
    assert_eq!(
        error.events.last().and_then(|event| event.error.as_deref()),
        Some("save_backup_failed:save_backup_scheduler_lease_unavailable")
    );
    let audit_events = audit_log.take_events();
    assert_eq!(audit_events.len(), 1);
    assert_eq!(audit_events[0].result, "failure");
    let state = scheduler_state
        .latest_state()
        .expect("scheduler state exists");
    assert_eq!(state.last_success_at, None);
    assert_eq!(
        state.last_error_code.as_deref(),
        Some("save_backup_scheduler_lease_unavailable")
    );
}

#[test]
fn auto_backup_lease_failure_prevents_cancelled_success_audit() {
    let task_manager = Arc::new(TaskManager::new());
    let task = task_manager
        .create_task(TaskKind::SaveBackup)
        .expect("task can be created");
    let scheduler_state = Arc::new(RecordingSchedulerStateRepository::with_state(
        sample_scheduler_state("auto-lease"),
    ));
    let audit_log = Arc::new(RecordingAuditLogWriter::default());
    let runner = SaveBackupTaskRunner::with_scope_registry_and_scheduler_state(
        Arc::clone(&task_manager),
        Arc::new(CancellingAndLosingLeaseExecutor {
            task_manager: Arc::clone(&task_manager),
            task_id: task.task_id.clone(),
            scheduler_state: scheduler_state.clone(),
        }),
        audit_log.clone(),
        Arc::new(FixedClock),
        Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default()),
        scheduler_state.clone(),
    );

    let error = runner
        .run_save_backup_task(&task.task_id, auto_request("auto-lease"))
        .expect_err("lease loss must take precedence over cancelled-success handling");

    assert_eq!(
        error.events.last().and_then(|event| event.error.as_deref()),
        Some("save_backup_failed:save_backup_scheduler_lease_unavailable")
    );
    let audit_events = audit_log.take_events();
    assert_eq!(audit_events.len(), 1);
    assert_eq!(audit_events[0].result, "failure");
    assert_eq!(
        audit_events[0].fields.get("error_code").map(String::as_str),
        Some("save_backup_scheduler_lease_unavailable")
    );
    let state = scheduler_state
        .latest_state()
        .expect("scheduler state exists");
    assert_eq!(state.last_success_at, None);
    assert_eq!(
        state.last_error_code.as_deref(),
        Some("save_backup_scheduler_lease_unavailable")
    );
}

#[test]
fn manual_and_pre_install_backups_ignore_scheduler_lease_repository() {
    for trigger in [SaveBackupTrigger::Manual, SaveBackupTrigger::PreInstall] {
        let task_manager = Arc::new(TaskManager::new());
        let task = task_manager
            .create_task(TaskKind::SaveBackup)
            .expect("task can be created");
        let initial_state = sample_scheduler_state("auto-lease");
        let scheduler_state = Arc::new(RecordingSchedulerStateRepository::with_state(
            initial_state.clone(),
        ));
        let runner = SaveBackupTaskRunner::with_scope_registry_and_scheduler_state(
            Arc::clone(&task_manager),
            Arc::new(RecordingSaveBackupExecutor::ok(sample_result())),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            Arc::new(hmm_app::SaveBackupTaskScopeRegistry::default()),
            scheduler_state.clone(),
        );
        let request = StartSaveBackupTaskRequest {
            trigger,
            scheduler_lease_owner: Some("must-be-ignored".to_owned()),
            ..sample_request()
        };

        runner
            .run_save_backup_task(&task.task_id, request)
            .expect("non-auto backup remains outside scheduler lease lifecycle");

        assert_eq!(scheduler_state.renewal_calls(), 0);
        assert!(scheduler_state.release_calls().is_empty());
        assert_eq!(scheduler_state.latest_state(), Some(initial_state));
    }
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

struct CancellingAndLosingLeaseExecutor {
    task_manager: Arc<TaskManager>,
    task_id: String,
    scheduler_state: Arc<RecordingSchedulerStateRepository>,
}

impl SaveBackupExecutor for CancellingAndLosingLeaseExecutor {
    fn create_backup(
        &self,
        _request: CreateSaveBackupRequest,
        _trigger: SaveBackupTrigger,
    ) -> Result<CreateSaveBackupResult, SaveBackupError> {
        self.task_manager
            .cancel_task(&self.task_id)
            .expect("running task can be cancelled");
        self.scheduler_state.set_renewal_available(false);
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
    renewal_available: Mutex<bool>,
    renewal_calls: Mutex<usize>,
}

impl RecordingSchedulerStateRepository {
    fn with_state(state: SaveBackupSchedulerState) -> Self {
        Self {
            states: Mutex::new(vec![state]),
            release_calls: Mutex::new(Vec::new()),
            renewal_available: Mutex::new(true),
            renewal_calls: Mutex::new(0),
        }
    }

    fn latest_state(&self) -> Option<SaveBackupSchedulerState> {
        self.states.lock().unwrap().last().cloned()
    }

    fn release_calls(&self) -> Vec<String> {
        self.release_calls.lock().unwrap().clone()
    }

    fn set_renewal_available(&self, available: bool) {
        *self.renewal_available.lock().unwrap() = available;
    }

    fn renewal_calls(&self) -> usize {
        *self.renewal_calls.lock().unwrap()
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
        request: SaveBackupSchedulerLeaseRequest,
    ) -> Result<Option<SaveBackupSchedulerState>> {
        let mut states = self.states.lock().unwrap();
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

    fn renew_lease(&self, request: SaveBackupSchedulerLeaseRenewalRequest) -> Result<bool> {
        *self.renewal_calls.lock().unwrap() += 1;
        if !*self.renewal_available.lock().unwrap() {
            return Ok(false);
        }
        let mut states = self.states.lock().unwrap();
        let Some(state) = states.iter_mut().rev().find(|state| {
            state.game_id == request.game_id && state.profile_id == request.profile_id
        }) else {
            return Ok(false);
        };
        if state.lease_owner.as_deref() != Some(request.lease_owner.as_str())
            || state
                .lease_expires_at
                .is_none_or(|expires_at| expires_at <= request.now_unix_millis)
            || state
                .lease_expires_at
                .is_some_and(|expires_at| expires_at > request.lease_expires_at)
        {
            return Ok(false);
        }

        state.lease_expires_at = Some(request.lease_expires_at);
        state.updated_at = request.now_unix_millis;
        Ok(true)
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

struct ControllableClock {
    now_unix_millis: Mutex<u128>,
}

impl ControllableClock {
    fn new(now_unix_millis: u128) -> Self {
        Self {
            now_unix_millis: Mutex::new(now_unix_millis),
        }
    }

    fn set(&self, now_unix_millis: u128) {
        *self.now_unix_millis.lock().unwrap() = now_unix_millis;
    }
}

impl AppClock for ControllableClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(*self.now_unix_millis.lock().unwrap())
    }
}

#[derive(Default)]
struct PanickingKeepaliveClock {
    calls: AtomicUsize,
    keepalive_panicked: AtomicBool,
}

impl AppClock for PanickingKeepaliveClock {
    fn now_unix_millis(&self) -> Result<u128> {
        let call = self.calls.fetch_add(1, Ordering::AcqRel);
        if call == 2 {
            self.keepalive_panicked.store(true, Ordering::Release);
            panic!("simulated keepalive clock panic");
        }
        Ok(42)
    }
}

struct BlockingSaveBackupExecutor {
    started: Mutex<mpsc::Receiver<()>>,
    started_sender: Mutex<Option<mpsc::Sender<()>>>,
    release: Mutex<mpsc::Receiver<()>>,
    release_sender: Mutex<Option<mpsc::Sender<()>>>,
}

impl BlockingSaveBackupExecutor {
    fn new() -> Self {
        let (started_sender, started) = mpsc::channel();
        let (release_sender, release) = mpsc::channel();
        Self {
            started: Mutex::new(started),
            started_sender: Mutex::new(Some(started_sender)),
            release: Mutex::new(release),
            release_sender: Mutex::new(Some(release_sender)),
        }
    }

    fn wait_until_started(&self) {
        self.started
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(1))
            .expect("executor starts within timeout");
    }

    fn release(&self) {
        self.release_sender
            .lock()
            .unwrap()
            .take()
            .expect("executor release sender is available")
            .send(())
            .expect("executor receives release");
    }
}

impl SaveBackupExecutor for BlockingSaveBackupExecutor {
    fn create_backup(
        &self,
        _request: CreateSaveBackupRequest,
        _trigger: SaveBackupTrigger,
    ) -> Result<CreateSaveBackupResult, SaveBackupError> {
        self.started_sender
            .lock()
            .unwrap()
            .take()
            .expect("executor start sender is available")
            .send(())
            .expect("test receives executor start");
        self.release
            .lock()
            .unwrap()
            .recv()
            .expect("test releases executor");
        Ok(sample_result())
    }
}

fn wait_until(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(1);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "condition was not met within timeout"
        );
        thread::sleep(Duration::from_millis(1));
    }
}
