use super::*;
use crate::{ReinstallCommitError, ReinstallCommitResult, ReinstallTargetCounts};
use hmm_core::{FileLayer, GameId, InstallManifest, ModId, ModRevisionId, ProfileId};
use hmm_ports::{AppClock, AuditLogEvent, AuditLogWriter};
use std::collections::BTreeSet;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone)]
struct FakePrepared {
    audit: ReinstallTaskAuditContext,
}

impl ReinstallTaskPrepared for FakePrepared {
    fn audit_context(&self) -> ReinstallTaskAuditContext {
        self.audit.clone()
    }
}

struct FakeExecutor {
    task_manager: Arc<crate::TaskManager>,
    task_id: String,
    prepare_result: Mutex<Option<Result<FakePrepared, ReinstallTaskPrepareError>>>,
    commit_result: Mutex<Option<Result<ReinstallCommitResult, ReinstallCommitError>>>,
    cancel_during_prepare: bool,
    cancel_during_commit: bool,
    commit_count: Mutex<usize>,
    commit_cancel_error: Mutex<Option<crate::TaskManagerError>>,
}

impl FakeExecutor {
    fn success(task_manager: Arc<crate::TaskManager>, task_id: &str) -> Self {
        Self {
            task_manager,
            task_id: task_id.to_owned(),
            prepare_result: Mutex::new(Some(Ok(FakePrepared {
                audit: audit_context(),
            }))),
            commit_result: Mutex::new(Some(Ok(ReinstallCommitResult {
                manifest: InstallManifest::completed(ProfileId::new("default"), Vec::new()),
            }))),
            cancel_during_prepare: false,
            cancel_during_commit: false,
            commit_count: Mutex::new(0),
            commit_cancel_error: Mutex::new(None),
        }
    }

    fn commit_count(&self) -> usize {
        *self.commit_count.lock().expect("commit count lock")
    }
}

impl ReinstallTaskExecutor for FakeExecutor {
    type Prepared = FakePrepared;

    fn prepare(
        &self,
        _request: crate::ReinstallPreviewRequest,
    ) -> Result<Self::Prepared, ReinstallTaskPrepareError> {
        if self.cancel_during_prepare {
            self.task_manager
                .cancel_task(&self.task_id)
                .expect("prepare cancellation succeeds before barrier");
        }
        self.prepare_result
            .lock()
            .expect("prepare result lock")
            .take()
            .expect("prepare called once")
    }

    fn commit(
        &self,
        _prepared: Self::Prepared,
        _expected_plan_token: &str,
    ) -> Result<ReinstallCommitResult, ReinstallCommitError> {
        *self.commit_count.lock().expect("commit count lock") += 1;
        if self.cancel_during_commit {
            let error = self
                .task_manager
                .cancel_task(&self.task_id)
                .expect_err("commit barrier must reject cancellation");
            *self
                .commit_cancel_error
                .lock()
                .expect("commit cancel error lock") = Some(error);
        }
        self.commit_result
            .lock()
            .expect("commit result lock")
            .take()
            .expect("commit called once")
    }
}

struct BlockingExecutor {
    prepare_started: Mutex<Option<mpsc::Sender<()>>>,
    commit_started: Mutex<Option<mpsc::Sender<()>>>,
    release_commit: Mutex<mpsc::Receiver<()>>,
}

impl ReinstallTaskExecutor for BlockingExecutor {
    type Prepared = FakePrepared;

    fn prepare(
        &self,
        _request: crate::ReinstallPreviewRequest,
    ) -> Result<Self::Prepared, ReinstallTaskPrepareError> {
        if let Some(sender) = self
            .prepare_started
            .lock()
            .expect("prepare sender lock")
            .take()
        {
            sender.send(()).expect("prepare signal receiver");
        }
        Ok(FakePrepared {
            audit: audit_context(),
        })
    }

    fn commit(
        &self,
        _prepared: Self::Prepared,
        _expected_plan_token: &str,
    ) -> Result<ReinstallCommitResult, ReinstallCommitError> {
        if let Some(sender) = self
            .commit_started
            .lock()
            .expect("commit sender lock")
            .take()
        {
            sender.send(()).expect("commit signal receiver");
        }
        self.release_commit
            .lock()
            .expect("release commit lock")
            .recv()
            .expect("commit release signal");
        Ok(ReinstallCommitResult {
            manifest: InstallManifest::completed(ProfileId::new("default"), Vec::new()),
        })
    }
}

#[derive(Default)]
struct RecordingAuditLog {
    events: Mutex<Vec<AuditLogEvent>>,
}

impl RecordingAuditLog {
    fn take_one(&self) -> AuditLogEvent {
        let mut events = self.events.lock().expect("audit events lock");
        assert_eq!(events.len(), 1);
        events.remove(0)
    }

    fn is_empty(&self) -> bool {
        self.events.lock().expect("audit events lock").is_empty()
    }
}

impl AuditLogWriter for RecordingAuditLog {
    fn record(&self, event: AuditLogEvent) -> anyhow::Result<()> {
        self.events.lock().expect("audit events lock").push(event);
        Ok(())
    }
}

struct TerminalAuditProbe {
    write_lock: Arc<Mutex<()>>,
    task_manager: Arc<crate::TaskManager>,
    task_id: String,
    expected_status: crate::TaskStatus,
    record_count: Mutex<usize>,
}

impl AuditLogWriter for TerminalAuditProbe {
    fn record(&self, _event: AuditLogEvent) -> anyhow::Result<()> {
        assert_eq!(
            self.task_manager.task_status(&self.task_id),
            Some(self.expected_status),
            "the terminal task state must be durable before Audit"
        );
        let _guard = self
            .write_lock
            .try_lock()
            .expect("game/profile write lock must be released before Audit");
        *self.record_count.lock().expect("record count lock") += 1;
        Ok(())
    }
}

struct FixedClock;

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> anyhow::Result<u128> {
        Ok(1_234)
    }
}

#[test]
fn start_reinstall_task_creates_queued_install_identity_without_request_data() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let service = ReinstallTaskService::new(task_manager);

    let started = service
        .start_reinstall_task(sample_request())
        .expect("task starts");

    assert_eq!(started.kind, crate::TaskKind::Install);
    assert_eq!(started.status, crate::TaskStatus::Queued);
    assert!(started.task_id.starts_with("install-"));
    assert!(!started.task_id.contains("mod-a"));
    assert!(!started.task_id.contains("candidate-v2"));
    assert!(!started.task_id.contains("opaque-plan-token"));
}

#[test]
fn successful_runner_emits_stable_phases_identity_and_sanitized_audit() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    let executor = Arc::new(FakeExecutor::success(
        Arc::clone(&task_manager),
        &task.task_id,
    ));
    let audit = Arc::new(RecordingAuditLog::default());
    let runner = ReinstallTaskRunner::new(
        Arc::clone(&task_manager),
        executor,
        audit.clone(),
        Arc::new(FixedClock),
    );

    let events = runner
        .run_reinstall_task(&task.task_id, sample_request())
        .expect("reinstall succeeds");

    assert_eq!(
        event_phases(&events),
        vec![
            "install.reinstall.plan.building",
            "install.reinstall.preflight.processing",
            "install.reinstall.commit.processing",
            "install.reinstall.completed",
        ]
    );
    assert!(events.iter().all(|event| event.task_id == task.task_id));
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(crate::TaskStatus::Completed)
    );

    let event = audit.take_one();
    assert_eq!(event.category, "install");
    assert_eq!(event.operation, "reinstall_mod");
    assert_eq!(event.result, "success");
    assert_eq!(
        event
            .fields
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "added_count",
            "candidate_revision_id",
            "game_id",
            "mod_id",
            "previous_revision_id",
            "profile_id",
            "replaced_count",
            "retained_count",
            "stale_count",
            "task_id",
        ])
    );
    assert_sanitized_audit(&event);
}

#[test]
fn terminal_task_state_and_audit_happen_after_write_lock_release() {
    assert_terminal_audit_after_lock_release(
        Ok(ReinstallCommitResult {
            manifest: InstallManifest::completed(ProfileId::new("default"), Vec::new()),
        }),
        crate::TaskStatus::Completed,
    );
    assert_terminal_audit_after_lock_release(
        Err(ReinstallCommitError::PostCommit),
        crate::TaskStatus::Failed,
    );
}

#[test]
fn cancellation_during_prepare_stops_before_commit_without_audit() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    let mut fake = FakeExecutor::success(Arc::clone(&task_manager), &task.task_id);
    fake.cancel_during_prepare = true;
    let executor = Arc::new(fake);
    let audit = Arc::new(RecordingAuditLog::default());
    let runner = ReinstallTaskRunner::new(
        Arc::clone(&task_manager),
        Arc::clone(&executor),
        audit.clone(),
        Arc::new(FixedClock),
    );

    let events = runner
        .run_reinstall_task(&task.task_id, sample_request())
        .expect("prepare cancellation is not a task failure");

    assert_eq!(
        event_phases(&events),
        vec![
            "install.reinstall.plan.building",
            "install.reinstall.cancelled",
        ]
    );
    assert_eq!(executor.commit_count(), 0);
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(crate::TaskStatus::Cancelled)
    );
    assert!(audit.is_empty());
}

#[test]
fn queued_cancellation_stops_before_prepare_or_commit() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    task_manager
        .cancel_task(&task.task_id)
        .expect("queued task can be cancelled");
    let executor = Arc::new(FakeExecutor::success(
        Arc::clone(&task_manager),
        &task.task_id,
    ));
    let audit = Arc::new(RecordingAuditLog::default());
    let runner = ReinstallTaskRunner::new(
        Arc::clone(&task_manager),
        Arc::clone(&executor),
        audit.clone(),
        Arc::new(FixedClock),
    );

    let events = runner
        .run_reinstall_task(&task.task_id, sample_request())
        .expect("queued cancellation is not a failure");

    assert_eq!(event_phases(&events), vec!["install.reinstall.cancelled"]);
    assert_eq!(executor.commit_count(), 0);
    assert!(audit.is_empty());
}

#[test]
fn cancellation_during_commit_is_rejected_and_success_stays_authoritative() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    let mut fake = FakeExecutor::success(Arc::clone(&task_manager), &task.task_id);
    fake.cancel_during_commit = true;
    let executor = Arc::new(fake);
    let runner = ReinstallTaskRunner::new(
        Arc::clone(&task_manager),
        Arc::clone(&executor),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock),
    );

    let events = runner
        .run_reinstall_task(&task.task_id, sample_request())
        .expect("commit result wins over cancellation");

    assert_eq!(
        events.last().expect("completed event").status,
        crate::TaskStatus::Completed
    );
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(crate::TaskStatus::Completed)
    );
    assert!(matches!(
        executor
            .commit_cancel_error
            .lock()
            .expect("commit cancel error lock")
            .as_ref(),
        Some(crate::TaskManagerError::TaskCannotBeCancelled {
            status: crate::TaskStatus::Running,
            ..
        })
    ));
}

#[test]
fn post_commit_failure_keeps_distinct_error_and_audit_rollback_result() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    let fake = FakeExecutor::success(Arc::clone(&task_manager), &task.task_id);
    *fake.commit_result.lock().expect("commit result lock") =
        Some(Err(ReinstallCommitError::PostCommit));
    let audit = Arc::new(RecordingAuditLog::default());
    let runner = ReinstallTaskRunner::new(
        Arc::clone(&task_manager),
        Arc::new(fake),
        audit.clone(),
        Arc::new(FixedClock),
    );

    let error = runner
        .run_reinstall_task(&task.task_id, sample_request())
        .expect_err("post-commit bookkeeping failure is not completed");

    assert_eq!(
        event_phases(&error.events),
        vec![
            "install.reinstall.plan.building",
            "install.reinstall.preflight.processing",
            "install.reinstall.commit.processing",
            "install.reinstall.failed",
        ]
    );
    assert_eq!(
        error.events.last().and_then(|event| event.error.as_deref()),
        Some("install_reinstall_failed:post_commit")
    );
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(crate::TaskStatus::Failed)
    );
    let event = audit.take_one();
    assert_eq!(event.result, "failure");
    assert_eq!(
        event.fields["error_code"],
        "install_reinstall_failed:post_commit"
    );
    assert_eq!(event.fields["rollback_result"], "not_attempted_post_commit");
    assert_sanitized_audit(&event);
}

#[test]
fn commit_errors_map_to_stable_task_phases_without_messages() {
    let cases = [
        (
            ReinstallCommitError::PreviewStale,
            "preflight",
            "not_attempted",
            false,
        ),
        (
            ReinstallCommitError::Failed {
                phase: crate::ReinstallCommitPhase::Snapshot,
            },
            "backup",
            "not_attempted",
            false,
        ),
        (
            ReinstallCommitError::Failed {
                phase: crate::ReinstallCommitPhase::Mutation,
            },
            "commit",
            "not_attempted",
            false,
        ),
        (
            ReinstallCommitError::Failed {
                phase: crate::ReinstallCommitPhase::Manifest,
            },
            "manifest",
            "not_attempted",
            false,
        ),
        (
            ReinstallCommitError::RolledBack {
                failed_phase: crate::ReinstallCommitPhase::Mutation,
                cleanup_pending: false,
            },
            "commit",
            "rolled_back",
            true,
        ),
        (
            ReinstallCommitError::RollbackRequired {
                failed_phase: crate::ReinstallCommitPhase::Mutation,
            },
            "rollback",
            "rollback_required",
            true,
        ),
        (
            ReinstallCommitError::RepairRequired {
                failed_phase: crate::ReinstallCommitPhase::Manifest,
            },
            "rollback",
            "repair_required",
            true,
        ),
        (
            ReinstallCommitError::PostCommit,
            "post_commit",
            "not_attempted_post_commit",
            false,
        ),
        (
            ReinstallCommitError::CleanupPending,
            "post_commit",
            "not_attempted_post_commit",
            false,
        ),
    ];

    for (error, phase, rollback_result, emit_rollback) in cases {
        let failure = commit_failure(&error);
        assert_eq!(failure.phase, phase);
        assert_eq!(failure.rollback_result, rollback_result);
        assert_eq!(failure.emit_rollback, emit_rollback);
    }
}

#[test]
fn prepare_runs_outside_same_scope_write_lock_and_commit_waits_for_release() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    let write_locks = Arc::new(crate::GameProfileWriteLockRegistry::default());
    let held_lock = write_locks.lock_for(&GameId::mhw(), &ProfileId::new("default"));
    let held_guard = held_lock.lock().expect("test write lock");
    let (prepare_tx, prepare_rx) = mpsc::channel();
    let (commit_tx, commit_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let runner = ReinstallTaskRunner::with_write_locks(
        Arc::clone(&task_manager),
        Arc::new(BlockingExecutor {
            prepare_started: Mutex::new(Some(prepare_tx)),
            commit_started: Mutex::new(Some(commit_tx)),
            release_commit: Mutex::new(release_rx),
        }),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock),
        Arc::clone(&write_locks),
    );
    let task_id = task.task_id.clone();
    let handle = thread::spawn(move || runner.run_reinstall_task(&task_id, sample_request()));

    prepare_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("prepare completes while write lock is held");
    assert!(
        matches!(commit_rx.try_recv(), Err(mpsc::TryRecvError::Empty)),
        "commit must remain outside the executor while the scope lock is held"
    );

    drop(held_guard);
    commit_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("commit enters after write lock release");
    release_tx.send(()).expect("release commit");
    assert!(handle.join().expect("runner thread").is_ok());
}

#[test]
fn different_profile_commit_is_not_blocked_by_another_scope_lock() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    let write_locks = Arc::new(crate::GameProfileWriteLockRegistry::default());
    let held_lock = write_locks.lock_for(&GameId::mhw(), &ProfileId::new("default"));
    let held_guard = held_lock.lock().expect("test write lock");
    let (prepare_tx, _prepare_rx) = mpsc::channel();
    let (commit_tx, commit_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let runner = ReinstallTaskRunner::with_write_locks(
        Arc::clone(&task_manager),
        Arc::new(BlockingExecutor {
            prepare_started: Mutex::new(Some(prepare_tx)),
            commit_started: Mutex::new(Some(commit_tx)),
            release_commit: Mutex::new(release_rx),
        }),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock),
        Arc::clone(&write_locks),
    );
    let mut request = sample_request();
    request.profile_id = ProfileId::new("other-profile");
    let task_id = task.task_id.clone();
    let handle = thread::spawn(move || runner.run_reinstall_task(&task_id, request));

    commit_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("different profile commit enters while first scope remains locked");
    release_tx.send(()).expect("release commit");
    assert!(handle.join().expect("runner thread").is_ok());
    drop(held_guard);
}

#[test]
fn install_uninstall_and_controlled_recovery_wait_for_reinstall_shared_lock() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let reinstall_task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    let install_task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    let uninstall_task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    let recovery_task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    let write_locks = Arc::new(crate::GameProfileWriteLockRegistry::default());

    let (reinstall_commit_tx, reinstall_commit_rx) = mpsc::channel();
    let (release_reinstall_tx, release_reinstall_rx) = mpsc::channel();
    let reinstall_runner = ReinstallTaskRunner::with_write_locks(
        Arc::clone(&task_manager),
        Arc::new(BlockingExecutor {
            prepare_started: Mutex::new(None),
            commit_started: Mutex::new(Some(reinstall_commit_tx)),
            release_commit: Mutex::new(release_reinstall_rx),
        }),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock),
        Arc::clone(&write_locks),
    );
    let reinstall_task_id = reinstall_task.task_id.clone();
    let reinstall_handle = thread::spawn(move || {
        reinstall_runner.run_reinstall_task(&reinstall_task_id, sample_request())
    });
    reinstall_commit_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("reinstall holds shared write lock");

    let (install_planned_tx, install_planned_rx) = mpsc::channel();
    let (install_entered_tx, install_entered_rx) = mpsc::channel();
    let install_runner = crate::InstallTaskRunner::with_write_locks(
        Arc::clone(&task_manager),
        Arc::new(NotifyingInstallPlanner {
            planned: Mutex::new(Some(install_planned_tx)),
        }),
        Arc::new(NotifyingInstallCommitter {
            entered: Mutex::new(Some(install_entered_tx)),
        }),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock),
        Arc::clone(&write_locks),
    );
    let install_task_id = install_task.task_id.clone();
    let install_handle =
        thread::spawn(move || install_runner.run_install_task(&install_task_id, install_request()));
    install_planned_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("install reaches lock boundary");

    let (uninstall_entered_tx, uninstall_entered_rx) = mpsc::channel();
    let uninstall_runner = crate::UninstallTaskRunner::with_write_locks(
        Arc::clone(&task_manager),
        Arc::new(NotifyingUninstaller {
            entered: Mutex::new(Some(uninstall_entered_tx)),
        }),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock),
        Arc::clone(&write_locks),
    );
    let uninstall_task_id = uninstall_task.task_id.clone();
    let uninstall_handle = thread::spawn(move || {
        uninstall_runner.run_uninstall_task(&uninstall_task_id, uninstall_request())
    });

    let (recovery_entered_tx, recovery_entered_rx) = mpsc::channel();
    let recovery_runner = crate::RecoveryActionTaskRunner::with_write_locks(
        Arc::clone(&task_manager),
        Arc::new(NotifyingRecoveryExecutor {
            entered: Mutex::new(Some(recovery_entered_tx)),
        }),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock),
        Arc::clone(&write_locks),
    );
    let recovery_task_id = recovery_task.task_id.clone();
    let recovery_handle = thread::spawn(move || {
        recovery_runner.run_recovery_action_task(&recovery_task_id, recovery_request())
    });

    wait_until_running(&task_manager, &uninstall_task.task_id);
    wait_until_running(&task_manager, &recovery_task.task_id);
    assert!(matches!(
        install_entered_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert!(matches!(
        uninstall_entered_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert!(matches!(
        recovery_entered_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));

    release_reinstall_tx.send(()).expect("release reinstall");
    install_entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("install enters after reinstall release");
    uninstall_entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("uninstall enters after reinstall release");
    recovery_entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("recovery enters after reinstall release");

    assert!(reinstall_handle.join().expect("reinstall thread").is_ok());
    assert!(install_handle.join().expect("install thread").is_ok());
    assert!(uninstall_handle.join().expect("uninstall thread").is_ok());
    assert!(recovery_handle.join().expect("recovery thread").is_ok());
}

#[test]
fn controlled_reinstall_reconciliation_records_sanitized_audit() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    let (entered_tx, entered_rx) = mpsc::channel();
    let audit = Arc::new(RecordingAuditLog::default());
    let runner = crate::RecoveryActionTaskRunner::new(
        Arc::clone(&task_manager),
        Arc::new(NotifyingRecoveryExecutor {
            entered: Mutex::new(Some(entered_tx)),
        }),
        audit.clone(),
        Arc::new(FixedClock),
    );

    runner
        .run_recovery_action_task(&task.task_id, recovery_request())
        .expect("reconciliation action succeeds");
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("reconciliation executor entered");

    let event = audit.take_one();
    assert_eq!(event.operation, "reconcile_reinstall");
    assert_eq!(event.result, "success");
    assert_eq!(
        event
            .fields
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "backup_count",
            "game_id",
            "mod_id",
            "profile_id",
            "remove_file_count",
            "restore_file_count",
            "task_id",
        ])
    );
    assert_sanitized_audit(&event);
}

struct NotifyingInstallPlanner {
    planned: Mutex<Option<mpsc::Sender<()>>>,
}

impl crate::ImportedModInstallPlanner for NotifyingInstallPlanner {
    fn build_imported_mod_install_plan(
        &self,
        _request: crate::BuildImportedModInstallPlanRequest,
    ) -> Result<hmm_core::InstallPlan, crate::InstallPlanningError> {
        if let Some(sender) = self.planned.lock().expect("planned sender lock").take() {
            sender.send(()).expect("planned signal receiver");
        }
        Ok(hmm_core::InstallPlan::from_providers(vec![
            hmm_core::InstallFileProvider::new(
                ModId::new("mod-a"),
                hmm_core::PackageFileId::new("nativePC/a.bin"),
                sample_target(),
                FileLayer::new("base", 0),
            ),
        ]))
    }
}

struct NotifyingInstallCommitter {
    entered: Mutex<Option<mpsc::Sender<()>>>,
}

impl crate::InstallPlanCommitter for NotifyingInstallCommitter {
    fn commit_install_plan(
        &self,
        _request: crate::ImportedModInstallCommitRequest,
    ) -> Result<crate::InstallCommitResult, crate::InstallCommitError> {
        if let Some(sender) = self.entered.lock().expect("install sender lock").take() {
            sender.send(()).expect("install signal receiver");
        }
        Ok(crate::InstallCommitResult {
            manifest: InstallManifest::completed(ProfileId::new("default"), Vec::new()),
        })
    }
}

struct NotifyingUninstaller {
    entered: Mutex<Option<mpsc::Sender<()>>>,
}

impl crate::ModUninstaller for NotifyingUninstaller {
    fn uninstall_mod(
        &self,
        _request: crate::StartUninstallTaskRequest,
    ) -> Result<crate::UninstallModResult, crate::UninstallModError> {
        if let Some(sender) = self.entered.lock().expect("uninstall sender lock").take() {
            sender.send(()).expect("uninstall signal receiver");
        }
        Ok(crate::UninstallModResult {
            manifest: InstallManifest::completed(ProfileId::new("default"), Vec::new()),
            removed_file_count: 0,
            restored_file_count: 0,
        })
    }
}

struct NotifyingRecoveryExecutor {
    entered: Mutex<Option<mpsc::Sender<()>>>,
}

struct BlockedWriteAdmission;

impl crate::InstallWriteAdmission for BlockedWriteAdmission {
    fn ensure_write_allowed(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
    ) -> Result<(), crate::InstallWriteAdmissionError> {
        Err(crate::InstallWriteAdmissionError::RecoveryPending)
    }
}

#[test]
fn unsafe_reinstall_recovery_gate_blocks_install_and_uninstall_before_writes() {
    let task_manager = Arc::new(crate::TaskManager::new());
    let install_task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    let uninstall_task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    let write_locks = Arc::new(crate::GameProfileWriteLockRegistry::default());
    let gate = Arc::new(BlockedWriteAdmission);

    let (install_planned_tx, install_planned_rx) = mpsc::channel();
    let (install_entered_tx, install_entered_rx) = mpsc::channel();
    let install_runner = crate::InstallTaskRunner::with_write_coordination(
        Arc::clone(&task_manager),
        Arc::new(NotifyingInstallPlanner {
            planned: Mutex::new(Some(install_planned_tx)),
        }),
        Arc::new(NotifyingInstallCommitter {
            entered: Mutex::new(Some(install_entered_tx)),
        }),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock),
        Arc::clone(&write_locks),
        gate.clone(),
    );
    let install_result = install_runner.run_install_task(&install_task.task_id, install_request());
    install_planned_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("install planning completes");
    assert!(install_result.is_err());
    assert!(matches!(
        install_entered_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));

    let (uninstall_entered_tx, uninstall_entered_rx) = mpsc::channel();
    let uninstall_runner = crate::UninstallTaskRunner::with_write_coordination(
        Arc::clone(&task_manager),
        Arc::new(NotifyingUninstaller {
            entered: Mutex::new(Some(uninstall_entered_tx)),
        }),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock),
        write_locks,
        gate,
    );
    let uninstall_result =
        uninstall_runner.run_uninstall_task(&uninstall_task.task_id, uninstall_request());
    assert!(uninstall_result.is_err());
    assert!(matches!(
        uninstall_entered_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

impl crate::InstallRecoveryActionExecutor for NotifyingRecoveryExecutor {
    fn run_recovery_action(
        &self,
        request: crate::StartRecoveryActionTaskRequest,
    ) -> Result<crate::InstallRecoveryActionResult, crate::InstallRecoveryActionError> {
        if let Some(sender) = self.entered.lock().expect("recovery sender lock").take() {
            sender.send(()).expect("recovery signal receiver");
        }
        Ok(crate::InstallRecoveryActionResult {
            profile_id: request.profile_id,
            mod_id: request.mod_id,
            action_kind: request.action_kind,
            remove_file_count: 0,
            restore_file_count: 0,
            backup_count: 0,
        })
    }
}

fn install_request() -> crate::StartInstallTaskRequest {
    crate::StartInstallTaskRequest {
        game_id: GameId::mhw(),
        mod_id: ModId::new("mod-a"),
        profile_id: ProfileId::new("default"),
        layer: FileLayer::new("base", 0),
    }
}

fn uninstall_request() -> crate::StartUninstallTaskRequest {
    crate::StartUninstallTaskRequest {
        game_id: GameId::mhw(),
        mod_id: ModId::new("mod-a"),
        profile_id: ProfileId::new("default"),
    }
}

fn recovery_request() -> crate::StartRecoveryActionTaskRequest {
    crate::StartRecoveryActionTaskRequest {
        game_id: GameId::mhw(),
        mod_id: ModId::new("mod-a"),
        profile_id: ProfileId::new("default"),
        action_kind: crate::InstallRecoveryActionKind::ReconcileReinstall,
    }
}

fn sample_target() -> hmm_core::InstallTargetPath {
    hmm_core::InstallTargetPath::parse("nativePC/a.bin", ["nativePC"]).expect("sample target")
}

fn wait_until_running(task_manager: &crate::TaskManager, task_id: &str) {
    for _ in 0..100_000 {
        if task_manager.task_status(task_id) == Some(crate::TaskStatus::Running) {
            return;
        }
        thread::yield_now();
    }
    panic!("task {task_id} did not reach running");
}

fn sample_request() -> StartReinstallTaskRequest {
    StartReinstallTaskRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        mod_id: ModId::new("mod-a"),
        candidate_revision_id: ModRevisionId::new("candidate-v2"),
        layer: FileLayer::new("base", 0),
        plan_token: "opaque-plan-token".to_owned(),
    }
}

fn audit_context() -> ReinstallTaskAuditContext {
    ReinstallTaskAuditContext {
        previous_revision_id: Some(ModRevisionId::new("installed-v1")),
        candidate_revision_id: ModRevisionId::new("candidate-v2"),
        counts: ReinstallTargetCounts {
            retained: 1,
            replaced: 2,
            added: 1,
            stale: 1,
        },
    }
}

fn event_phases(events: &[crate::TaskProgressEvent]) -> Vec<&str> {
    events.iter().map(|event| event.phase.as_str()).collect()
}

fn assert_sanitized_audit(event: &AuditLogEvent) {
    let payload = serde_json::to_string(event).expect("serialize audit event");
    for forbidden in [
        "opaque-plan-token",
        "nativePC/",
        "backup-ref",
        "snapshot-ref",
        "C:/Users/",
        "manifest",
        "sha256",
    ] {
        assert!(!payload.contains(forbidden), "audit leaked {forbidden}");
    }
}

fn assert_terminal_audit_after_lock_release(
    commit_result: Result<ReinstallCommitResult, ReinstallCommitError>,
    expected_status: crate::TaskStatus,
) {
    let task_manager = Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::Install)
        .expect("task can be created");
    let fake = FakeExecutor::success(Arc::clone(&task_manager), &task.task_id);
    *fake.commit_result.lock().expect("commit result lock") = Some(commit_result);
    let write_locks = Arc::new(crate::GameProfileWriteLockRegistry::default());
    let write_lock = write_locks.lock_for(&GameId::mhw(), &ProfileId::new("default"));
    let audit = Arc::new(TerminalAuditProbe {
        write_lock,
        task_manager: Arc::clone(&task_manager),
        task_id: task.task_id.clone(),
        expected_status,
        record_count: Mutex::new(0),
    });
    let runner = ReinstallTaskRunner::with_write_locks(
        task_manager,
        Arc::new(fake),
        audit.clone(),
        Arc::new(FixedClock),
        write_locks,
    );

    let result = runner.run_reinstall_task(&task.task_id, sample_request());

    assert_eq!(
        result.is_ok(),
        expected_status == crate::TaskStatus::Completed
    );
    assert_eq!(*audit.record_count.lock().expect("record count lock"), 1);
}
