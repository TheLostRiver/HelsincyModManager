use std::collections::BTreeMap;
use std::sync::Arc;

use hmm_core::{FileLayer, GameId, InstallPlan, ModId, ProfileId, ReplacementTargetId};
use hmm_ports::{AppClock, AuditLogEvent, AuditLogWriter, AuditWriteFailurePolicy};

use crate::{
    GameProfileWriteLockRegistry, ImportedModInstallCommitRequest, InstallPlanCommitter,
    InstallWriteAdmission, ReplacementWorkflowError, TaskKind, TaskManager, TaskManagerError,
    TaskProgressEvent, TaskStarted, TaskStatus,
};

const PLAN_BUILDING_PHASE: &str = "install.retarget.plan.building";
const COMMIT_PROCESSING_PHASE: &str = "install.retarget.commit.processing";
const COMPLETED_PHASE: &str = "install.retarget.completed";
const FAILED_PHASE: &str = "install.retarget.failed";
const FAILED_ERROR: &str = "install_retarget_failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRetargetInstallTaskRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub target_id: ReplacementTargetId,
    pub layer: FileLayer,
}

pub trait InitialRetargetInstallPlanner: Send + Sync {
    fn build_initial_retarget_install_plan(
        &self,
        request: StartRetargetInstallTaskRequest,
    ) -> Result<InstallPlan, ReplacementWorkflowError>;

    fn revalidate_initial_install(
        &self,
        request: &StartRetargetInstallTaskRequest,
    ) -> Result<(), ReplacementWorkflowError>;

    fn discard_initial_retarget_install(&self, plan: &InstallPlan);
}

pub struct RetargetInstallTaskService {
    task_manager: Arc<TaskManager>,
}

pub struct RetargetInstallTaskRunner {
    task_manager: Arc<TaskManager>,
    planner: Arc<dyn InitialRetargetInstallPlanner>,
    committer: Arc<dyn InstallPlanCommitter>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
    write_locks: Arc<GameProfileWriteLockRegistry>,
    write_admission: Arc<dyn InstallWriteAdmission>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetargetInstallTaskRunError {
    pub events: Vec<TaskProgressEvent>,
}

impl RetargetInstallTaskService {
    pub fn new(task_manager: Arc<TaskManager>) -> Self {
        Self { task_manager }
    }

    pub fn start_retarget_install_task(
        &self,
        _request: StartRetargetInstallTaskRequest,
    ) -> Result<TaskStarted, TaskManagerError> {
        let task = self.task_manager.create_task(TaskKind::Install)?;
        Ok(TaskStarted {
            task_id: task.task_id,
            kind: task.kind,
            status: task.status,
        })
    }
}

impl RetargetInstallTaskRunner {
    #[allow(clippy::too_many_arguments)]
    pub fn with_write_coordination(
        task_manager: Arc<TaskManager>,
        planner: Arc<dyn InitialRetargetInstallPlanner>,
        committer: Arc<dyn InstallPlanCommitter>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
        write_admission: Arc<dyn InstallWriteAdmission>,
    ) -> Self {
        Self {
            task_manager,
            planner,
            committer,
            audit_log,
            clock,
            write_locks,
            write_admission,
        }
    }

    pub fn run_retarget_install_task(
        &self,
        task_id: &str,
        request: StartRetargetInstallTaskRequest,
    ) -> Result<Vec<TaskProgressEvent>, RetargetInstallTaskRunError> {
        if self.task_manager.start_task(task_id).is_err() {
            return Err(RetargetInstallTaskRunError { events: Vec::new() });
        }

        let mut events = vec![running_event(task_id, PLAN_BUILDING_PHASE)];
        let plan = match self
            .planner
            .build_initial_retarget_install_plan(request.clone())
        {
            Ok(plan) => plan,
            Err(_) => return Err(self.fail(task_id, &request, events, "planning", 0)),
        };
        let action_count = plan.actions.len();
        let cleanup_plan = plan.clone();

        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
            self.planner.discard_initial_retarget_install(&plan);
            return Ok(events);
        }

        let write_lock = self
            .write_locks
            .lock_for(&request.game_id, &request.profile_id);
        let commit_result = {
            let _guard = write_lock.lock().map_err(|_| {
                self.planner.discard_initial_retarget_install(&plan);
                self.fail(task_id, &request, events.clone(), "lock", action_count)
            })?;
            if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                self.planner.discard_initial_retarget_install(&plan);
                return Ok(events);
            }
            self.write_admission
                .ensure_write_allowed(&request.game_id, &request.profile_id)
                .map_err(|error| {
                    self.planner.discard_initial_retarget_install(&plan);
                    self.fail(
                        task_id,
                        &request,
                        events.clone(),
                        error.failure_phase(),
                        action_count,
                    )
                })?;
            self.planner
                .revalidate_initial_install(&request)
                .map_err(|_| {
                    self.planner.discard_initial_retarget_install(&plan);
                    self.fail(task_id, &request, events.clone(), "state", action_count)
                })?;
            if self.task_manager.block_task_cancellation(task_id).is_err() {
                self.planner.discard_initial_retarget_install(&plan);
                if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                    return Ok(events);
                }
                return Err(self.fail(task_id, &request, events, "lock", action_count));
            }

            events.push(running_event(task_id, COMMIT_PROCESSING_PHASE));
            self.committer
                .commit_install_plan(ImportedModInstallCommitRequest {
                    game_id: request.game_id.clone(),
                    mod_id: request.mod_id.clone(),
                    profile_id: request.profile_id.clone(),
                    plan,
                })
        };

        self.planner.discard_initial_retarget_install(&cleanup_plan);
        if commit_result.is_err() {
            return Err(self.fail(task_id, &request, events, "commit", action_count));
        }
        self.record_audit(task_id, &request, "success", action_count, None);

        match self.task_manager.complete_task(task_id) {
            Ok(task) => {
                events.push(TaskProgressEvent::new(
                    task.task_id,
                    task.kind,
                    task.status,
                    COMPLETED_PHASE,
                ));
                Ok(events)
            }
            Err(_) if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) => {
                Ok(events)
            }
            Err(_) => Err(self.fail(task_id, &request, events, "complete", action_count)),
        }
    }

    fn fail(
        &self,
        task_id: &str,
        request: &StartRetargetInstallTaskRequest,
        mut events: Vec<TaskProgressEvent>,
        phase: &str,
        action_count: usize,
    ) -> RetargetInstallTaskRunError {
        if matches!(
            self.task_manager.fail_task(task_id),
            Err(TaskManagerError::TaskCannotTransition {
                from: TaskStatus::Cancelled,
                to: TaskStatus::Failed,
                ..
            })
        ) {
            return RetargetInstallTaskRunError { events };
        }
        let error_code = format!("{FAILED_ERROR}:{phase}");
        events.push(failed_event(task_id, phase));
        self.record_audit(task_id, request, "failure", action_count, Some(&error_code));
        RetargetInstallTaskRunError { events }
    }

    fn record_audit(
        &self,
        task_id: &str,
        request: &StartRetargetInstallTaskRequest,
        result: &str,
        action_count: usize,
        error_code: Option<&str>,
    ) {
        let mut fields = BTreeMap::from([
            ("task_id".to_owned(), task_id.to_owned()),
            ("game_id".to_owned(), request.game_id.as_str().to_owned()),
            ("mod_id".to_owned(), request.mod_id.as_str().to_owned()),
            (
                "profile_id".to_owned(),
                request.profile_id.as_str().to_owned(),
            ),
            (
                "target_id".to_owned(),
                request.target_id.as_str().to_owned(),
            ),
            ("action_count".to_owned(), action_count.to_string()),
        ]);
        if let Some(error_code) = error_code {
            fields.insert("error_code".to_owned(), error_code.to_owned());
        }
        let policy = if result == "success" { AuditWriteFailurePolicy::ReportAfterCommit } else { AuditWriteFailurePolicy::BestEffort };
        let _ = self.audit_log.record_with_policy(AuditLogEvent {
            timestamp_unix_millis: self.clock.now_unix_millis().unwrap_or_default(),
            category: "install".to_owned(),
            operation: "commit_retargeted_mod".to_owned(),
            result: result.to_owned(),
            fields,
        }, policy);
    }
}

fn running_event(task_id: &str, phase: &str) -> TaskProgressEvent {
    TaskProgressEvent::new(
        task_id.to_owned(),
        TaskKind::Install,
        TaskStatus::Running,
        phase,
    )
}

fn failed_event(task_id: &str, failed_phase: &str) -> TaskProgressEvent {
    let mut event = TaskProgressEvent::new(
        task_id.to_owned(),
        TaskKind::Install,
        TaskStatus::Failed,
        FAILED_PHASE,
    );
    event.error = Some(format!("{FAILED_ERROR}:{failed_phase}"));
    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use hmm_core::{InstallFileProvider, InstallManifest, InstallTargetPath, PackageFileId};

    use crate::{InstallCommitError, InstallCommitResult, InstallWriteAdmissionError};

    struct RecordingPlanner {
        revalidate_result: Result<(), ReplacementWorkflowError>,
        revalidated: Mutex<bool>,
        discard_count: Mutex<usize>,
    }

    impl InitialRetargetInstallPlanner for RecordingPlanner {
        fn build_initial_retarget_install_plan(
            &self,
            request: StartRetargetInstallTaskRequest,
        ) -> Result<InstallPlan, ReplacementWorkflowError> {
            Ok(InstallPlan::from_providers([InstallFileProvider::new(
                request.mod_id,
                PackageFileId::new("body"),
                InstallTargetPath::parse(
                    "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
                    ["nativePC"],
                )
                .expect("target path"),
                request.layer,
            )]))
        }

        fn revalidate_initial_install(
            &self,
            _request: &StartRetargetInstallTaskRequest,
        ) -> Result<(), ReplacementWorkflowError> {
            *self.revalidated.lock().expect("revalidated") = true;
            self.revalidate_result.clone()
        }

        fn discard_initial_retarget_install(&self, _plan: &InstallPlan) {
            *self.discard_count.lock().expect("discard count") += 1;
        }
    }

    #[derive(Default)]
    struct RecordingCommitter {
        commit_count: Mutex<usize>,
    }

    impl InstallPlanCommitter for RecordingCommitter {
        fn commit_install_plan(
            &self,
            request: ImportedModInstallCommitRequest,
        ) -> Result<InstallCommitResult, InstallCommitError> {
            *self.commit_count.lock().expect("commit count") += 1;
            Ok(InstallCommitResult {
                manifest: InstallManifest::completed(request.profile_id, Vec::new()),
            })
        }
    }

    struct CancellingCommitter {
        task_manager: Arc<TaskManager>,
        task_id: String,
        cancel_result: Mutex<Option<Result<TaskStatus, TaskManagerError>>>,
    }

    impl InstallPlanCommitter for CancellingCommitter {
        fn commit_install_plan(
            &self,
            request: ImportedModInstallCommitRequest,
        ) -> Result<InstallCommitResult, InstallCommitError> {
            let result = self
                .task_manager
                .cancel_task(&self.task_id)
                .map(|task| task.status);
            *self.cancel_result.lock().expect("cancel result") = Some(result);
            Ok(InstallCommitResult {
                manifest: InstallManifest::completed(request.profile_id, Vec::new()),
            })
        }
    }

    #[derive(Default)]
    struct RecordingAudit {
        events: Mutex<Vec<AuditLogEvent>>,
    }

    impl AuditLogWriter for RecordingAudit {
        fn record(&self, event: AuditLogEvent) -> anyhow::Result<()> {
            self.events.lock().expect("audit events").push(event);
            Ok(())
        }
    }

    struct FixedClock;

    impl AppClock for FixedClock {
        fn now_unix_millis(&self) -> anyhow::Result<u128> {
            Ok(42)
        }
    }

    struct AllowWrites;

    impl InstallWriteAdmission for AllowWrites {
        fn ensure_write_allowed(
            &self,
            _game_id: &GameId,
            _profile_id: &ProfileId,
        ) -> Result<(), InstallWriteAdmissionError> {
            Ok(())
        }
    }

    fn request() -> StartRetargetInstallTaskRequest {
        StartRetargetInstallTaskRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("profile-a"),
            mod_id: ModId::new("mod-a"),
            target_id: ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target id"),
            layer: FileLayer::new("base", 0),
        }
    }

    fn runner(
        task_manager: Arc<TaskManager>,
        planner: Arc<RecordingPlanner>,
        committer: Arc<RecordingCommitter>,
        audit: Arc<RecordingAudit>,
    ) -> RetargetInstallTaskRunner {
        RetargetInstallTaskRunner::with_write_coordination(
            task_manager,
            planner,
            committer,
            audit,
            Arc::new(FixedClock),
            Arc::new(GameProfileWriteLockRegistry::default()),
            Arc::new(AllowWrites),
        )
    }

    #[test]
    fn runner_revalidates_under_the_write_lock_before_committing() {
        let task_manager = Arc::new(TaskManager::new());
        let planner = Arc::new(RecordingPlanner {
            revalidate_result: Ok(()),
            revalidated: Mutex::new(false),
            discard_count: Mutex::new(0),
        });
        let committer = Arc::new(RecordingCommitter::default());
        let audit = Arc::new(RecordingAudit::default());
        let task = RetargetInstallTaskService::new(Arc::clone(&task_manager))
            .start_retarget_install_task(request())
            .expect("start task");

        let events = runner(
            task_manager,
            Arc::clone(&planner),
            Arc::clone(&committer),
            Arc::clone(&audit),
        )
        .run_retarget_install_task(&task.task_id, request())
        .expect("run task");

        assert!(*planner.revalidated.lock().expect("revalidated"));
        assert_eq!(*committer.commit_count.lock().expect("commit count"), 1);
        assert_eq!(events[0].phase, PLAN_BUILDING_PHASE);
        assert_eq!(events[1].phase, COMMIT_PROCESSING_PHASE);
        assert_eq!(events[2].phase, COMPLETED_PHASE);
        let audit = audit.events.lock().expect("audit events");
        assert_eq!(audit[0].operation, "commit_retargeted_mod");
        assert_eq!(audit[0].fields["target_id"], "mhw:armor:fatalis-alpha");
    }

    #[test]
    fn runner_fails_closed_when_locked_revalidation_changes_state() {
        let task_manager = Arc::new(TaskManager::new());
        let planner = Arc::new(RecordingPlanner {
            revalidate_result: Err(ReplacementWorkflowError::InitialInstallBlocked {
                status: crate::InstallRecoveryStatus::Completed,
            }),
            revalidated: Mutex::new(false),
            discard_count: Mutex::new(0),
        });
        let committer = Arc::new(RecordingCommitter::default());
        let audit = Arc::new(RecordingAudit::default());
        let task = RetargetInstallTaskService::new(Arc::clone(&task_manager))
            .start_retarget_install_task(request())
            .expect("start task");

        let error = runner(
            task_manager,
            Arc::clone(&planner),
            Arc::clone(&committer),
            audit,
        )
        .run_retarget_install_task(&task.task_id, request())
        .expect_err("state change blocks commit");

        assert_eq!(*committer.commit_count.lock().expect("commit count"), 0);
        assert_eq!(*planner.discard_count.lock().expect("discard count"), 1);
        assert_eq!(
            error.events.last().expect("failed event").phase,
            FAILED_PHASE
        );
        assert_eq!(
            error.events.last().expect("failed event").error.as_deref(),
            Some("install_retarget_failed:state")
        );
    }

    #[test]
    fn runner_blocks_cancellation_before_commit_and_completes_consistently() {
        let task_manager = Arc::new(TaskManager::new());
        let planner = Arc::new(RecordingPlanner {
            revalidate_result: Ok(()),
            revalidated: Mutex::new(false),
            discard_count: Mutex::new(0),
        });
        let audit = Arc::new(RecordingAudit::default());
        let task = RetargetInstallTaskService::new(Arc::clone(&task_manager))
            .start_retarget_install_task(request())
            .expect("start task");
        let committer = Arc::new(CancellingCommitter {
            task_manager: Arc::clone(&task_manager),
            task_id: task.task_id.clone(),
            cancel_result: Mutex::new(None),
        });
        let runner = RetargetInstallTaskRunner::with_write_coordination(
            Arc::clone(&task_manager),
            planner,
            committer.clone(),
            audit,
            Arc::new(FixedClock),
            Arc::new(GameProfileWriteLockRegistry::default()),
            Arc::new(AllowWrites),
        );

        let events = runner
            .run_retarget_install_task(&task.task_id, request())
            .expect("committed task completes");

        let cancel_result = committer.cancel_result.lock().expect("cancel result");
        assert!(matches!(
            cancel_result
                .as_ref()
                .expect("commit attempted cancellation"),
            Err(TaskManagerError::TaskCannotBeCancelled {
                status: TaskStatus::Running,
                ..
            })
        ));
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(TaskStatus::Completed)
        );
        assert_eq!(
            events.last().expect("completed event").phase,
            COMPLETED_PHASE
        );
    }
}
