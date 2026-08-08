use std::collections::BTreeMap;
use std::sync::Arc;

use hmm_core::{
    FileLayer, GameId, InstallPlan, ModId, ModRevisionId, ProfileId, ReplacementTargetId,
};
use hmm_ports::{AppClock, AuditLogEvent, AuditLogWriter, AuditWriteFailurePolicy};

use crate::replacement_audit::{
    append_adapter_audit_fields, unique_adapter_audit_facts, ReplacementAdapterAuditFacts,
};
use crate::{
    GamePrerequisiteDecision, GameProfileWriteLockRegistry, ImportedModInstallCommitRequest,
    InstallPlanCommitter, InstallWriteAdmission, ReplacementWorkflowError, TaskKind, TaskManager,
    TaskManagerError, TaskProgressEvent, TaskStarted, TaskStatus,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialRetargetInstallPlan {
    pub plan: InstallPlan,
    pub revision_id: ModRevisionId,
}

pub trait InitialRetargetInstallPlanner: Send + Sync {
    fn build_initial_retarget_install_plan(
        &self,
        request: StartRetargetInstallTaskRequest,
    ) -> Result<InitialRetargetInstallPlan, ReplacementWorkflowError>;

    fn revalidate_initial_install(
        &self,
        request: &StartRetargetInstallTaskRequest,
    ) -> Result<(), ReplacementWorkflowError>;

    fn prerequisite_decision(&self, game_id: &GameId) -> GamePrerequisiteDecision;

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
        let prerequisite_decision = self.planner.prerequisite_decision(&request.game_id);
        if prerequisite_decision.is_blocked() {
            return Err(self.fail(task_id, &request, events, "prerequisite", 0, None));
        }
        let planned = match self
            .planner
            .build_initial_retarget_install_plan(request.clone())
        {
            Ok(planned) => planned,
            Err(_) => return Err(self.fail(task_id, &request, events, "planning", 0, None)),
        };
        let revision_id = planned.revision_id;
        let plan = planned.plan;
        let action_count = plan.actions.len();
        let adapter_facts = unique_adapter_audit_facts(&plan.replacement_bindings);
        let cleanup_plan = plan.clone();

        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
            self.planner.discard_initial_retarget_install(&plan);
            return Ok(events);
        }

        let current_prerequisite_decision = self.planner.prerequisite_decision(&request.game_id);
        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
            self.planner.discard_initial_retarget_install(&plan);
            return Ok(events);
        }
        if current_prerequisite_decision.is_blocked()
            || current_prerequisite_decision != prerequisite_decision
        {
            self.planner.discard_initial_retarget_install(&plan);
            return Err(self.fail(
                task_id,
                &request,
                events,
                "prerequisite",
                action_count,
                adapter_facts.as_deref(),
            ));
        }

        let write_lock = self
            .write_locks
            .lock_for(&request.game_id, &request.profile_id);
        let commit_result = {
            let _guard = write_lock.lock().map_err(|_| {
                self.planner.discard_initial_retarget_install(&plan);
                self.fail(
                    task_id,
                    &request,
                    events.clone(),
                    "lock",
                    action_count,
                    adapter_facts.as_deref(),
                )
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
                        adapter_facts.as_deref(),
                    )
                })?;
            self.planner
                .revalidate_initial_install(&request)
                .map_err(|_| {
                    self.planner.discard_initial_retarget_install(&plan);
                    self.fail(
                        task_id,
                        &request,
                        events.clone(),
                        "state",
                        action_count,
                        adapter_facts.as_deref(),
                    )
                })?;
            if self.task_manager.block_task_cancellation(task_id).is_err() {
                self.planner.discard_initial_retarget_install(&plan);
                if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                    return Ok(events);
                }
                return Err(self.fail(
                    task_id,
                    &request,
                    events,
                    "lock",
                    action_count,
                    adapter_facts.as_deref(),
                ));
            }

            events.push(running_event(task_id, COMMIT_PROCESSING_PHASE));
            self.committer
                .commit_install_plan(ImportedModInstallCommitRequest {
                    game_id: request.game_id.clone(),
                    mod_id: request.mod_id.clone(),
                    revision_id: Some(revision_id),
                    profile_id: request.profile_id.clone(),
                    plan,
                })
        };

        self.planner.discard_initial_retarget_install(&cleanup_plan);
        if commit_result.is_err() {
            return Err(self.fail(
                task_id,
                &request,
                events,
                "commit",
                action_count,
                adapter_facts.as_deref(),
            ));
        }
        self.record_audit(
            task_id,
            &request,
            "success",
            action_count,
            None,
            adapter_facts.as_deref(),
        );

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
            Err(_) => Err(self.fail(
                task_id,
                &request,
                events,
                "complete",
                action_count,
                adapter_facts.as_deref(),
            )),
        }
    }

    fn fail(
        &self,
        task_id: &str,
        request: &StartRetargetInstallTaskRequest,
        mut events: Vec<TaskProgressEvent>,
        phase: &str,
        action_count: usize,
        adapter_facts: Option<&ReplacementAdapterAuditFacts>,
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
        self.record_audit(
            task_id,
            request,
            "failure",
            action_count,
            Some(&error_code),
            adapter_facts,
        );
        RetargetInstallTaskRunError { events }
    }

    fn record_audit(
        &self,
        task_id: &str,
        request: &StartRetargetInstallTaskRequest,
        result: &str,
        action_count: usize,
        error_code: Option<&str>,
        adapter_facts: Option<&ReplacementAdapterAuditFacts>,
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
        append_adapter_audit_fields(&mut fields, adapter_facts);
        let policy = AuditWriteFailurePolicy::for_commit_result(result);
        let _ = self.audit_log.record_with_policy(
            AuditLogEvent {
                timestamp_unix_millis: self.clock.now_unix_millis().unwrap_or_default(),
                category: "install".to_owned(),
                operation: "commit_retargeted_mod".to_owned(),
                result: result.to_owned(),
                fields,
            },
            policy,
        );
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

    use hmm_core::{
        ContentTransformerIdentity, InstallFileProvider, InstallManifest, InstallTargetPath,
        PackageFileId, ReplacementAdapterFacts,
    };

    use crate::{InstallCommitError, InstallCommitResult, InstallWriteAdmissionError};

    struct RecordingPlanner {
        revalidate_result: Result<(), ReplacementWorkflowError>,
        preview_decision: GamePrerequisiteDecision,
        revalidation_decision: GamePrerequisiteDecision,
        build_count: Mutex<usize>,
        revalidated: Mutex<bool>,
        discard_count: Mutex<usize>,
        prerequisite_write_lock: Option<Arc<std::sync::Mutex<()>>>,
    }

    impl InitialRetargetInstallPlanner for RecordingPlanner {
        fn build_initial_retarget_install_plan(
            &self,
            request: StartRetargetInstallTaskRequest,
        ) -> Result<InitialRetargetInstallPlan, ReplacementWorkflowError> {
            *self.build_count.lock().expect("build count") += 1;
            Ok(InitialRetargetInstallPlan {
                plan: InstallPlan::from_providers([InstallFileProvider::new(
                    request.mod_id,
                    PackageFileId::new("body"),
                    InstallTargetPath::parse(
                        "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
                        ["nativePC"],
                    )
                    .expect("target path"),
                    request.layer,
                )]),
                revision_id: ModRevisionId::new("revision-a"),
            })
        }

        fn revalidate_initial_install(
            &self,
            _request: &StartRetargetInstallTaskRequest,
        ) -> Result<(), ReplacementWorkflowError> {
            *self.revalidated.lock().expect("revalidated") = true;
            self.revalidate_result.clone()
        }

        fn prerequisite_decision(&self, _game_id: &GameId) -> GamePrerequisiteDecision {
            if let Some(write_lock) = &self.prerequisite_write_lock {
                let _guard = write_lock
                    .try_lock()
                    .expect("prerequisite checks must run outside the write lock");
            }
            if *self.build_count.lock().expect("build count") == 0 {
                self.preview_decision.clone()
            } else {
                self.revalidation_decision.clone()
            }
        }

        fn discard_initial_retarget_install(&self, _plan: &InstallPlan) {
            *self.discard_count.lock().expect("discard count") += 1;
        }
    }

    #[derive(Default)]
    struct RecordingCommitter {
        commit_count: Mutex<usize>,
        revisions: Mutex<Vec<Option<ModRevisionId>>>,
    }

    impl InstallPlanCommitter for RecordingCommitter {
        fn commit_install_plan(
            &self,
            request: ImportedModInstallCommitRequest,
        ) -> Result<InstallCommitResult, InstallCommitError> {
            *self.commit_count.lock().expect("commit count") += 1;
            self.revisions
                .lock()
                .expect("committed revisions")
                .push(request.revision_id);
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
            preview_decision: ready_prerequisite_decision(),
            revalidation_decision: ready_prerequisite_decision(),
            build_count: Mutex::new(0),
            revalidated: Mutex::new(false),
            discard_count: Mutex::new(0),
            prerequisite_write_lock: None,
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
        assert_eq!(
            *committer.revisions.lock().expect("committed revisions"),
            vec![Some(ModRevisionId::new("revision-a"))]
        );
        assert_eq!(events[0].phase, PLAN_BUILDING_PHASE);
        assert_eq!(events[1].phase, COMMIT_PROCESSING_PHASE);
        assert_eq!(events[2].phase, COMPLETED_PHASE);
        let audit = audit.events.lock().expect("audit events");
        assert_eq!(audit[0].operation, "commit_retargeted_mod");
        assert_eq!(audit[0].fields["target_id"], "mhw:armor:fatalis-alpha");
    }

    #[test]
    fn retarget_install_audit_projects_only_stable_transformer_facts() {
        let task_manager = Arc::new(TaskManager::new());
        let planner = Arc::new(RecordingPlanner {
            revalidate_result: Ok(()),
            preview_decision: ready_prerequisite_decision(),
            revalidation_decision: ready_prerequisite_decision(),
            build_count: Mutex::new(0),
            revalidated: Mutex::new(false),
            discard_count: Mutex::new(0),
            prerequisite_write_lock: None,
        });
        let audit = Arc::new(RecordingAudit::default());
        let runner = runner(
            task_manager,
            planner,
            Arc::new(RecordingCommitter::default()),
            Arc::clone(&audit),
        );
        let facts = ReplacementAdapterFacts::new(
            1,
            "mhw.weapon",
            "mrl3-texture-path",
            1,
            "a".repeat(64),
            "b".repeat(64),
            "c".repeat(64),
        )
        .expect("adapter facts")
        .with_transformers(
            vec![
                ContentTransformerIdentity::new("mhw.weapon.mrl3-texture-path.v1", 1)
                    .expect("transformer identity"),
            ],
            1,
            2,
        )
        .expect("transformer facts");

        let audit_facts =
            ReplacementAdapterAuditFacts::from_adapter_facts(&facts).expect("audit projection");
        runner.record_audit(
            "install-1",
            &request(),
            "success",
            2,
            None,
            Some(&audit_facts),
        );

        let event = &audit.events.lock().expect("audit events")[0];
        assert_eq!(event.fields["adapter_id"], "mhw.weapon");
        assert_eq!(event.fields["strategy_id"], "mrl3-texture-path");
        assert_eq!(
            event.fields["transformer_id"],
            "mhw.weapon.mrl3-texture-path.v1"
        );
        assert_eq!(event.fields["transformer_version"], "1");
        assert_eq!(event.fields["part_count"], "1");
        assert_eq!(event.fields["file_count"], "2");
        assert!(!event.fields.values().any(|value| value.len() == 64));
    }

    #[test]
    fn runner_fails_closed_when_locked_revalidation_changes_state() {
        let task_manager = Arc::new(TaskManager::new());
        let planner = Arc::new(RecordingPlanner {
            revalidate_result: Err(ReplacementWorkflowError::InitialInstallBlocked {
                status: crate::InstallRecoveryStatus::Completed,
            }),
            preview_decision: ready_prerequisite_decision(),
            revalidation_decision: ready_prerequisite_decision(),
            build_count: Mutex::new(0),
            revalidated: Mutex::new(false),
            discard_count: Mutex::new(0),
            prerequisite_write_lock: None,
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
    fn runner_blocks_missing_prerequisites_before_materializing_staging() {
        let task_manager = Arc::new(TaskManager::new());
        let planner = Arc::new(RecordingPlanner {
            revalidate_result: Ok(()),
            preview_decision: blocked_prerequisite_decision(),
            revalidation_decision: blocked_prerequisite_decision(),
            build_count: Mutex::new(0),
            revalidated: Mutex::new(false),
            discard_count: Mutex::new(0),
            prerequisite_write_lock: None,
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
        .expect_err("missing prerequisites must block retarget install");

        assert_eq!(*planner.build_count.lock().expect("build count"), 0);
        assert_eq!(*planner.discard_count.lock().expect("discard count"), 0);
        assert_eq!(*committer.commit_count.lock().expect("commit count"), 0);
        assert_eq!(
            error.events.last().and_then(|event| event.error.as_deref()),
            Some("install_retarget_failed:prerequisite")
        );
    }

    #[test]
    fn runner_rejects_prerequisite_drift_before_write_lock() {
        let task_manager = Arc::new(TaskManager::new());
        let write_locks = Arc::new(GameProfileWriteLockRegistry::default());
        let write_lock = write_locks.lock_for(&GameId::mhw(), &ProfileId::new("profile-a"));
        let planner = Arc::new(RecordingPlanner {
            revalidate_result: Ok(()),
            preview_decision: warning_prerequisite_decision(),
            revalidation_decision: blocked_prerequisite_decision(),
            build_count: Mutex::new(0),
            revalidated: Mutex::new(false),
            discard_count: Mutex::new(0),
            prerequisite_write_lock: Some(write_lock),
        });
        let committer = Arc::new(RecordingCommitter::default());
        let audit = Arc::new(RecordingAudit::default());
        let task = RetargetInstallTaskService::new(Arc::clone(&task_manager))
            .start_retarget_install_task(request())
            .expect("start task");

        let error = RetargetInstallTaskRunner::with_write_coordination(
            task_manager,
            planner.clone(),
            committer.clone(),
            audit,
            Arc::new(FixedClock),
            write_locks,
            Arc::new(AllowWrites),
        )
        .run_retarget_install_task(&task.task_id, request())
        .expect_err("prerequisite drift must block retarget install");

        assert_eq!(*planner.build_count.lock().expect("build count"), 1);
        assert_eq!(*planner.discard_count.lock().expect("discard count"), 1);
        assert_eq!(*committer.commit_count.lock().expect("commit count"), 0);
        assert!(!*planner.revalidated.lock().expect("revalidated"));
        assert_eq!(
            error.events.last().and_then(|event| event.error.as_deref()),
            Some("install_retarget_failed:prerequisite")
        );
    }

    #[test]
    fn runner_blocks_cancellation_before_commit_and_completes_consistently() {
        let task_manager = Arc::new(TaskManager::new());
        let planner = Arc::new(RecordingPlanner {
            revalidate_result: Ok(()),
            preview_decision: ready_prerequisite_decision(),
            revalidation_decision: ready_prerequisite_decision(),
            build_count: Mutex::new(0),
            revalidated: Mutex::new(false),
            discard_count: Mutex::new(0),
            prerequisite_write_lock: None,
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

    fn ready_prerequisite_decision() -> GamePrerequisiteDecision {
        GamePrerequisiteDecision {
            game_id: GameId::mhw(),
            status: crate::GamePrerequisiteDecisionStatus::Ready,
            rules_version: Some(1),
            codes: Vec::new(),
        }
    }

    fn warning_prerequisite_decision() -> GamePrerequisiteDecision {
        GamePrerequisiteDecision {
            game_id: GameId::mhw(),
            status: crate::GamePrerequisiteDecisionStatus::Warning,
            rules_version: Some(1),
            codes: vec![crate::GamePrerequisiteDecisionCode::SignatureUnverified],
        }
    }

    fn blocked_prerequisite_decision() -> GamePrerequisiteDecision {
        GamePrerequisiteDecision {
            game_id: GameId::mhw(),
            status: crate::GamePrerequisiteDecisionStatus::Blocked,
            rules_version: Some(1),
            codes: vec![crate::GamePrerequisiteDecisionCode::MissingRequiredFile],
        }
    }
}
