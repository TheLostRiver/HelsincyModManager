use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use hmm_core::{
    FileLayer, GameId, InstallPlan, ModId, ModRevisionId, ProfileId, ReplacementBindingSnapshot,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, AuditWriteFailurePolicy,
    CrossProcessWriteAdmissionResult, CrossProcessWriteGuard, NeverCancelled,
    ReinstallRecoveryTransactionRepository,
};
use thiserror::Error;

use crate::task_manager::{noop_task_progress_observer, observe_task_progress};
use crate::{
    is_identity_replacement_binding, BuildImportedModInstallPlanRequest, CommitInstallPlanRequest,
    GamePrerequisiteDecision, ImportedModInstallPreflight, ImportedModInstallPreflightService,
    InstallCommitError, InstallCommitResult, InstallCommitService, InstallPlanningError,
    InstallRecoveryActionError, InstallRecoveryActionKind, InstallRecoveryActionRequest,
    InstallRecoveryActionResult, InstallRecoveryActionService, TaskKind, TaskManager,
    TaskManagerError, TaskProgressEvent, TaskProgressObserver, TaskStarted, TaskStatus,
    UninstallModError, UninstallModRequest, UninstallModResult, UninstallModService,
    CrossProcessWriteAdmissionCoordinator,
};

const INSTALL_PLAN_BUILDING_PHASE: &str = "install.plan.building";
const INSTALL_COMMIT_PROCESSING_PHASE: &str = "install.commit.processing";
const INSTALL_COMPLETED_PHASE: &str = "install.completed";
const INSTALL_FAILED_PHASE: &str = "install.failed";
const INSTALL_FAILED_ERROR: &str = "install_failed";
const INSTALL_UNINSTALL_PROCESSING_PHASE: &str = "install.uninstall.processing";
const INSTALL_UNINSTALL_COMPLETED_PHASE: &str = "install.uninstall.completed";
const INSTALL_UNINSTALL_FAILED_PHASE: &str = "install.uninstall.failed";
const INSTALL_UNINSTALL_FAILED_ERROR: &str = "install_uninstall_failed";
const INSTALL_RECOVERY_PLANNING_PHASE: &str = "install.recovery.planning";
const INSTALL_RECOVERY_PROCESSING_PHASE: &str = "install.recovery.processing";
const INSTALL_RECOVERY_COMPLETED_PHASE: &str = "install.recovery.completed";
const INSTALL_RECOVERY_FAILED_PHASE: &str = "install.recovery.failed";
const INSTALL_RECOVERY_FAILED_ERROR: &str = "install_recovery_failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartInstallTaskRequest {
    pub game_id: GameId,
    pub mod_id: ModId,
    pub profile_id: ProfileId,
    pub layer: FileLayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartUninstallTaskRequest {
    pub game_id: GameId,
    pub mod_id: ModId,
    pub profile_id: ProfileId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRecoveryActionTaskRequest {
    pub game_id: GameId,
    pub mod_id: ModId,
    pub profile_id: ProfileId,
    pub action_kind: InstallRecoveryActionKind,
}

pub struct InstallTaskService {
    task_manager: Arc<TaskManager>,
}

pub struct UninstallTaskService {
    task_manager: Arc<TaskManager>,
}

pub struct RecoveryActionTaskService {
    task_manager: Arc<TaskManager>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallTaskRunError {
    pub events: Vec<TaskProgressEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallTaskOrchestrationError {
    pub events: Vec<TaskProgressEvent>,
    pub commit_error: Option<InstallCommitError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallTaskRunError {
    pub events: Vec<TaskProgressEvent>,
    pub(crate) uninstall_error: Option<UninstallModError>,
    pub(crate) committed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryActionTaskRunError {
    pub events: Vec<TaskProgressEvent>,
}

pub struct ImportedModInstallCommitRequest {
    pub game_id: GameId,
    pub mod_id: ModId,
    pub revision_id: Option<ModRevisionId>,
    pub profile_id: ProfileId,
    pub plan: InstallPlan,
}

pub trait ImportedModInstallPlanner: Send + Sync {
    fn build_imported_mod_install_plan(
        &self,
        request: BuildImportedModInstallPlanRequest,
    ) -> Result<ImportedModInstallPreflight, InstallPlanningError>;

    fn build_imported_mod_revision_install_plan(
        &self,
        game_id: &GameId,
        mod_id: &ModId,
        revision_id: &ModRevisionId,
        layer: &FileLayer,
    ) -> Result<ImportedModInstallPreflight, InstallPlanningError>;

    fn prerequisite_decision(&self, game_id: &GameId) -> GamePrerequisiteDecision;
}

impl ImportedModInstallPlanner for ImportedModInstallPreflightService {
    fn build_imported_mod_install_plan(
        &self,
        request: BuildImportedModInstallPlanRequest,
    ) -> Result<ImportedModInstallPreflight, InstallPlanningError> {
        self.preview(request)
    }

    fn build_imported_mod_revision_install_plan(
        &self,
        game_id: &GameId,
        mod_id: &ModId,
        revision_id: &ModRevisionId,
        layer: &FileLayer,
    ) -> Result<ImportedModInstallPreflight, InstallPlanningError> {
        self.preview_revision(game_id, mod_id, revision_id, layer)
    }

    fn prerequisite_decision(&self, game_id: &GameId) -> GamePrerequisiteDecision {
        ImportedModInstallPreflightService::prerequisite_decision(self, game_id)
    }
}

pub trait InstallPlanCommitter: Send + Sync {
    fn commit_install_plan(
        &self,
        request: ImportedModInstallCommitRequest,
    ) -> Result<InstallCommitResult, InstallCommitError>;
}

impl InstallPlanCommitter for InstallCommitService {
    fn commit_install_plan(
        &self,
        request: ImportedModInstallCommitRequest,
    ) -> Result<InstallCommitResult, InstallCommitError> {
        let commit_request = CommitInstallPlanRequest {
            profile_id: request.profile_id,
            plan: request.plan,
        };
        match request.revision_id {
            Some(revision_id) => {
                self.commit_plan_for_revision(commit_request, request.mod_id, revision_id)
            }
            None => self.commit_plan(commit_request),
        }
    }
}

pub trait ModUninstaller: Send + Sync {
    fn uninstall_mod(
        &self,
        request: StartUninstallTaskRequest,
    ) -> Result<UninstallModResult, UninstallModError>;

    fn uninstall_mod_for_revision(
        &self,
        request: StartUninstallTaskRequest,
        expected_installed_revision_id: ModRevisionId,
    ) -> Result<UninstallModResult, UninstallModError>;

    fn uninstall_mod_for_revision_and_manifest(
        &self,
        request: StartUninstallTaskRequest,
        expected_installed_revision_id: ModRevisionId,
        expected_manifest_digest: &str,
    ) -> Result<UninstallModResult, UninstallModError>;
}

impl ModUninstaller for UninstallModService {
    fn uninstall_mod(
        &self,
        request: StartUninstallTaskRequest,
    ) -> Result<UninstallModResult, UninstallModError> {
        UninstallModService::uninstall_mod(
            self,
            UninstallModRequest {
                profile_id: request.profile_id,
                mod_id: request.mod_id,
            },
        )
    }

    fn uninstall_mod_for_revision(
        &self,
        request: StartUninstallTaskRequest,
        expected_installed_revision_id: ModRevisionId,
    ) -> Result<UninstallModResult, UninstallModError> {
        UninstallModService::uninstall_mod_for_revision(
            self,
            UninstallModRequest {
                profile_id: request.profile_id,
                mod_id: request.mod_id,
            },
            expected_installed_revision_id,
        )
    }

    fn uninstall_mod_for_revision_and_manifest(
        &self,
        request: StartUninstallTaskRequest,
        expected_installed_revision_id: ModRevisionId,
        expected_manifest_digest: &str,
    ) -> Result<UninstallModResult, UninstallModError> {
        UninstallModService::uninstall_mod_for_revision_and_manifest(
            self,
            UninstallModRequest {
                profile_id: request.profile_id,
                mod_id: request.mod_id,
            },
            expected_installed_revision_id,
            expected_manifest_digest,
        )
    }
}

pub trait InstallRecoveryActionExecutor: Send + Sync {
    fn run_recovery_action(
        &self,
        request: StartRecoveryActionTaskRequest,
    ) -> Result<InstallRecoveryActionResult, InstallRecoveryActionError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallWriteAdmissionError {
    #[error("reinstall recovery state is unavailable")]
    RecoveryUnavailable,
    #[error("reinstall recovery is pending")]
    RecoveryPending,
    #[error("write safety admission rejected the operation")]
    SafetyRejected,
}

impl InstallWriteAdmissionError {
    pub(crate) fn failure_phase(&self) -> &'static str {
        match self {
            Self::RecoveryUnavailable => "recovery_unavailable",
            Self::RecoveryPending => "recovery_pending",
            Self::SafetyRejected => "write_safety_rejected",
        }
    }
}

pub trait InstallWriteAdmission: Send + Sync {
    fn ensure_write_allowed(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<(), InstallWriteAdmissionError>;

    fn ensure_install_plan_allowed(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        _mod_id: &ModId,
        _plan: &InstallPlan,
        _prerequisite_decision: &GamePrerequisiteDecision,
    ) -> Result<(), InstallWriteAdmissionError> {
        self.ensure_write_allowed(game_id, profile_id)
    }
}

pub(crate) struct AllowInstallWriteAdmission;

impl InstallWriteAdmission for AllowInstallWriteAdmission {
    fn ensure_write_allowed(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
    ) -> Result<(), InstallWriteAdmissionError> {
        Ok(())
    }
}

pub struct ReinstallRecoveryWriteAdmission {
    repository: Arc<dyn ReinstallRecoveryTransactionRepository>,
}

impl ReinstallRecoveryWriteAdmission {
    pub fn new(repository: Arc<dyn ReinstallRecoveryTransactionRepository>) -> Self {
        Self { repository }
    }
}

impl InstallWriteAdmission for ReinstallRecoveryWriteAdmission {
    fn ensure_write_allowed(
        &self,
        _game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<(), InstallWriteAdmissionError> {
        let transactions = self
            .repository
            .list_transactions(profile_id)
            .map_err(|_| InstallWriteAdmissionError::RecoveryUnavailable)?;
        if transactions.is_empty() {
            Ok(())
        } else {
            Err(InstallWriteAdmissionError::RecoveryPending)
        }
    }
}

impl InstallRecoveryActionExecutor for InstallRecoveryActionService {
    fn run_recovery_action(
        &self,
        request: StartRecoveryActionTaskRequest,
    ) -> Result<InstallRecoveryActionResult, InstallRecoveryActionError> {
        self.run(InstallRecoveryActionRequest {
            profile_id: request.profile_id,
            mod_id: request.mod_id,
            action_kind: request.action_kind,
        })
    }
}

pub struct InstallTaskRunner {
    task_manager: Arc<TaskManager>,
    planner: Arc<dyn ImportedModInstallPlanner>,
    committer: Arc<dyn InstallPlanCommitter>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
    write_locks: Arc<GameProfileWriteLockRegistry>,
    write_admission: Arc<dyn InstallWriteAdmission>,
}

pub struct UninstallTaskRunner {
    task_manager: Arc<TaskManager>,
    uninstaller: Arc<dyn ModUninstaller>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
    write_locks: Arc<GameProfileWriteLockRegistry>,
    write_admission: Arc<dyn InstallWriteAdmission>,
}

pub struct RecoveryActionTaskRunner {
    task_manager: Arc<TaskManager>,
    action_executor: Arc<dyn InstallRecoveryActionExecutor>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
    write_locks: Arc<GameProfileWriteLockRegistry>,
    write_admission: Arc<dyn InstallWriteAdmission>,
}

impl InstallTaskService {
    pub fn new(task_manager: Arc<TaskManager>) -> Self {
        Self { task_manager }
    }

    pub fn start_install_task(
        &self,
        _request: StartInstallTaskRequest,
    ) -> Result<TaskStarted, TaskManagerError> {
        let task = self.task_manager.create_task(TaskKind::Install)?;

        Ok(TaskStarted {
            task_id: task.task_id,
            kind: task.kind,
            status: task.status,
        })
    }
}

impl UninstallTaskService {
    pub fn new(task_manager: Arc<TaskManager>) -> Self {
        Self { task_manager }
    }

    pub fn start_uninstall_task(
        &self,
        _request: StartUninstallTaskRequest,
    ) -> Result<TaskStarted, TaskManagerError> {
        let task = self.task_manager.create_task(TaskKind::Install)?;

        Ok(TaskStarted {
            task_id: task.task_id,
            kind: task.kind,
            status: task.status,
        })
    }
}

impl RecoveryActionTaskService {
    pub fn new(task_manager: Arc<TaskManager>) -> Self {
        Self { task_manager }
    }

    pub fn start_recovery_action_task(
        &self,
        _request: StartRecoveryActionTaskRequest,
    ) -> Result<TaskStarted, TaskManagerError> {
        let task = self.task_manager.create_task(TaskKind::Install)?;

        Ok(TaskStarted {
            task_id: task.task_id,
            kind: task.kind,
            status: task.status,
        })
    }
}

impl InstallTaskRunner {
    pub fn new(
        task_manager: Arc<TaskManager>,
        planner: Arc<dyn ImportedModInstallPlanner>,
        committer: Arc<dyn InstallPlanCommitter>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self::with_write_locks(
            task_manager,
            planner,
            committer,
            audit_log,
            clock,
            Arc::new(GameProfileWriteLockRegistry::default()),
        )
    }

    pub fn with_write_locks(
        task_manager: Arc<TaskManager>,
        planner: Arc<dyn ImportedModInstallPlanner>,
        committer: Arc<dyn InstallPlanCommitter>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
    ) -> Self {
        Self::with_write_coordination(
            task_manager,
            planner,
            committer,
            audit_log,
            clock,
            write_locks,
            Arc::new(AllowInstallWriteAdmission),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_write_coordination(
        task_manager: Arc<TaskManager>,
        planner: Arc<dyn ImportedModInstallPlanner>,
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

    pub fn run_install_task(
        &self,
        task_id: &str,
        request: StartInstallTaskRequest,
    ) -> Result<Vec<TaskProgressEvent>, InstallTaskRunError> {
        let observer = noop_task_progress_observer();
        self.run_install_task_with_observer(task_id, request, &observer)
    }

    pub fn run_install_task_with_observer<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: StartInstallTaskRequest,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, InstallTaskRunError> {
        self.run_install_task_for_orchestration_with_observer(task_id, request, observer)
            .map_err(|error| InstallTaskRunError {
                events: error.events,
            })
    }

    pub(crate) fn run_install_task_for_orchestration_with_observer<
        O: TaskProgressObserver + ?Sized,
    >(
        &self,
        task_id: &str,
        request: StartInstallTaskRequest,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, InstallTaskOrchestrationError> {
        self.run_install_task_for_orchestration_with_revision(
            task_id, request, None, None, observer,
        )
    }

    pub(crate) fn run_install_revision_task_for_orchestration_with_observer<
        O: TaskProgressObserver + ?Sized,
    >(
        &self,
        task_id: &str,
        request: StartInstallTaskRequest,
        revision_id: ModRevisionId,
        replacement_binding_snapshot: Option<ReplacementBindingSnapshot>,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, InstallTaskOrchestrationError> {
        self.run_install_task_for_orchestration_with_revision(
            task_id,
            request,
            Some(revision_id),
            replacement_binding_snapshot,
            observer,
        )
    }

    fn run_install_task_for_orchestration_with_revision<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: StartInstallTaskRequest,
        revision_id: Option<ModRevisionId>,
        replacement_binding_snapshot: Option<ReplacementBindingSnapshot>,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, InstallTaskOrchestrationError> {
        if self.task_manager.start_task(task_id).is_err() {
            return Err(InstallTaskOrchestrationError {
                events: Vec::new(),
                commit_error: None,
            });
        }

        let mut events = Vec::new();
        observe_task_progress(
            &mut events,
            observer,
            running_event(task_id, INSTALL_PLAN_BUILDING_PHASE),
        );
        let preflight = match revision_id.as_ref() {
            Some(revision_id) => self.planner.build_imported_mod_revision_install_plan(
                &request.game_id,
                &request.mod_id,
                revision_id,
                &request.layer,
            ),
            None => {
                self.planner
                    .build_imported_mod_install_plan(BuildImportedModInstallPlanRequest {
                        game_id: request.game_id.clone(),
                        mod_id: request.mod_id.clone(),
                        layer: request.layer.clone(),
                    })
            }
        };
        let preflight = match preflight {
            Ok(preflight) => preflight,
            Err(_) => {
                return Err(self.fail_with_audit(task_id, &request, events, observer, "planning", 0))
            }
        };
        let action_count = preflight.plan.actions.len();
        if preflight.prerequisite_decision.is_blocked() {
            return Err(self.fail_with_audit(
                task_id,
                &request,
                events,
                observer,
                "prerequisite",
                action_count,
            ));
        }
        let prerequisite_decision = preflight.prerequisite_decision;
        let mut plan = preflight.plan;
        if let Some(binding) = replacement_binding_snapshot {
            let binding_is_valid = revision_id.as_ref() == binding.revision_id()
                && binding.mod_id() == &request.mod_id
                && binding.profile_id() == &request.profile_id
                && is_identity_replacement_binding(&binding);
            if !binding_is_valid {
                return Err(self.fail_with_audit(
                    task_id,
                    &request,
                    events,
                    observer,
                    "planning",
                    action_count,
                ));
            }
            plan = match plan.with_replacement_bindings(vec![binding]) {
                Ok(plan) => plan,
                Err(_) => {
                    return Err(self.fail_with_audit(
                        task_id,
                        &request,
                        events,
                        observer,
                        "planning",
                        action_count,
                    ))
                }
            };
        }
        let action_count = plan.actions.len();

        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
            return Ok(events);
        }

        let current_prerequisite_decision = self.planner.prerequisite_decision(&request.game_id);
        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
            return Ok(events);
        }
        if current_prerequisite_decision.is_blocked()
            || current_prerequisite_decision != prerequisite_decision
        {
            return Err(self.fail_with_audit(
                task_id,
                &request,
                events,
                observer,
                "prerequisite",
                action_count,
            ));
        }

        let commit_result = {
            let _cross_process_guard = match self.write_locks.acquire_cross_process_for_task(
                &request.game_id,
                &request.profile_id,
                &self.task_manager,
                task_id,
            ) {
                Ok(guard) => guard,
                Err(error) => {
                    if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                        return Ok(events);
                    }
                    return Err(self.fail_with_audit(
                        task_id,
                        &request,
                        events,
                        observer,
                        error.code(),
                        action_count,
                    ));
                }
            };
            let write_lock = self
                .write_locks
                .lock_for(&request.game_id, &request.profile_id);
            let _guard = write_lock.lock().map_err(|_| {
                self.fail_with_audit(
                    task_id,
                    &request,
                    events.clone(),
                    observer,
                    "lock",
                    action_count,
                )
            })?;
            if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                return Ok(events);
            }
            match self.write_admission.ensure_install_plan_allowed(
                &request.game_id,
                &request.profile_id,
                &request.mod_id,
                &plan,
                &current_prerequisite_decision,
            ) {
                Ok(()) => {
                    observe_task_progress(
                        &mut events,
                        observer,
                        running_event(task_id, INSTALL_COMMIT_PROCESSING_PHASE),
                    );
                    if self.task_manager.block_task_cancellation(task_id).is_err() {
                        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                            return Ok(events);
                        }
                        return Err(self.fail_with_audit(
                            task_id,
                            &request,
                            events,
                            observer,
                            "lock",
                            action_count,
                        ));
                    }
                    Ok(self
                        .committer
                        .commit_install_plan(ImportedModInstallCommitRequest {
                            game_id: request.game_id.clone(),
                            mod_id: request.mod_id.clone(),
                            revision_id: revision_id.clone(),
                            profile_id: request.profile_id.clone(),
                            plan,
                        }))
                }
                Err(error) => Err(error),
            }
        };
        let commit_result = match commit_result {
            Ok(result) => result,
            Err(error) => {
                return Err(self.fail_with_audit(
                    task_id,
                    &request,
                    events,
                    observer,
                    error.failure_phase(),
                    action_count,
                ))
            }
        };

        let audit_ok = match commit_result {
            Ok(_) => self.record_audit(task_id, &request, "success", action_count, None, None),
            Err(error) => {
                return Err(self.fail_with_commit_error(
                    task_id,
                    &request,
                    events,
                    observer,
                    action_count,
                    error,
                ))
            }
        };

        match self.task_manager.complete_task(task_id) {
            Ok(task) => {
                let mut event = TaskProgressEvent::new(
                    task.task_id,
                    task.kind,
                    task.status,
                    INSTALL_COMPLETED_PHASE,
                );
                if !audit_ok {
                    event.error = Some("install_audit_unavailable".to_owned());
                }
                observe_task_progress(&mut events, observer, event);
                Ok(events)
            }
            Err(_) if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) => {
                Ok(events)
            }
            Err(_) => Err(self.fail_with_audit(
                task_id,
                &request,
                events,
                observer,
                "complete",
                action_count,
            )),
        }
    }

    fn fail_with_audit<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &StartInstallTaskRequest,
        events: Vec<TaskProgressEvent>,
        observer: &O,
        phase: &str,
        action_count: usize,
    ) -> InstallTaskOrchestrationError {
        self.fail_with_audit_details(
            task_id,
            request,
            events,
            observer,
            phase,
            action_count,
            None,
        )
    }

    fn fail_with_commit_error<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &StartInstallTaskRequest,
        events: Vec<TaskProgressEvent>,
        observer: &O,
        action_count: usize,
        commit_error: InstallCommitError,
    ) -> InstallTaskOrchestrationError {
        let rollback_result = match &commit_error {
            InstallCommitError::RollbackSucceeded { .. } => "rollback_succeeded",
            InstallCommitError::RollbackFailed { .. } => "rollback_failed",
            InstallCommitError::PlanHasBlockingConflicts
            | InstallCommitError::PlanHasInvalidReplacementBindings
            | InstallCommitError::PlanHasInvalidRevisionIdentity
            | InstallCommitError::Failed { .. } => "not_attempted",
        };
        let mut error = self.fail_with_audit_details(
            task_id,
            request,
            events,
            observer,
            "commit",
            action_count,
            Some(rollback_result),
        );
        error.commit_error = Some(commit_error);
        error
    }

    #[allow(clippy::too_many_arguments)]
    fn fail_with_audit_details<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &StartInstallTaskRequest,
        mut events: Vec<TaskProgressEvent>,
        observer: &O,
        phase: &str,
        action_count: usize,
        rollback_result: Option<&str>,
    ) -> InstallTaskOrchestrationError {
        if matches!(
            self.task_manager.fail_task(task_id),
            Err(TaskManagerError::TaskCannotTransition {
                from: TaskStatus::Cancelled,
                to: TaskStatus::Failed,
                ..
            })
        ) {
            return InstallTaskOrchestrationError {
                events,
                commit_error: None,
            };
        }
        let error_code = format!("{INSTALL_FAILED_ERROR}:{phase}");
        observe_task_progress(&mut events, observer, failed_event(task_id, phase));
        self.record_audit(
            task_id,
            request,
            "failure",
            action_count,
            Some(&error_code),
            rollback_result,
        );
        InstallTaskOrchestrationError {
            events,
            commit_error: None,
        }
    }

    fn record_audit(
        &self,
        task_id: &str,
        request: &StartInstallTaskRequest,
        result: &str,
        action_count: usize,
        error_code: Option<&str>,
        rollback_result: Option<&str>,
    ) -> bool {
        let timestamp_unix_millis = self.clock.now_unix_millis().unwrap_or_default();
        let mut fields = BTreeMap::new();
        fields.insert("task_id".to_owned(), task_id.to_owned());
        fields.insert("game_id".to_owned(), request.game_id.as_str().to_owned());
        fields.insert("mod_id".to_owned(), request.mod_id.as_str().to_owned());
        fields.insert(
            "profile_id".to_owned(),
            request.profile_id.as_str().to_owned(),
        );
        fields.insert("action_count".to_owned(), action_count.to_string());
        if let Some(error_code) = error_code {
            fields.insert("error_code".to_owned(), error_code.to_owned());
        }
        if let Some(rollback_result) = rollback_result {
            fields.insert("rollback_result".to_owned(), rollback_result.to_owned());
        }

        let policy = AuditWriteFailurePolicy::for_commit_result(result);
        self.audit_log
            .record_with_policy(
                AuditLogEvent {
                    timestamp_unix_millis,
                    category: "install".to_owned(),
                    operation: "commit_imported_mod".to_owned(),
                    result: result.to_owned(),
                    fields,
                },
                policy,
            )
            .is_ok()
    }
}

impl UninstallTaskRunner {
    pub fn new(
        task_manager: Arc<TaskManager>,
        uninstaller: Arc<dyn ModUninstaller>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self::with_write_locks(
            task_manager,
            uninstaller,
            audit_log,
            clock,
            Arc::new(GameProfileWriteLockRegistry::default()),
        )
    }

    pub fn with_write_locks(
        task_manager: Arc<TaskManager>,
        uninstaller: Arc<dyn ModUninstaller>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
    ) -> Self {
        Self::with_write_coordination(
            task_manager,
            uninstaller,
            audit_log,
            clock,
            write_locks,
            Arc::new(AllowInstallWriteAdmission),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_write_coordination(
        task_manager: Arc<TaskManager>,
        uninstaller: Arc<dyn ModUninstaller>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
        write_admission: Arc<dyn InstallWriteAdmission>,
    ) -> Self {
        Self {
            task_manager,
            uninstaller,
            audit_log,
            clock,
            write_locks,
            write_admission,
        }
    }

    pub fn run_uninstall_task(
        &self,
        task_id: &str,
        request: StartUninstallTaskRequest,
    ) -> Result<Vec<TaskProgressEvent>, UninstallTaskRunError> {
        let observer = noop_task_progress_observer();
        self.run_uninstall_task_with_observer(task_id, request, &observer)
    }

    pub fn run_uninstall_task_with_observer<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: StartUninstallTaskRequest,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, UninstallTaskRunError> {
        self.run_uninstall_task_internal(task_id, request, None, None, observer)
    }

    pub(crate) fn run_uninstall_revision_task_for_orchestration_with_observer<
        O: TaskProgressObserver + ?Sized,
    >(
        &self,
        task_id: &str,
        request: StartUninstallTaskRequest,
        expected_installed_revision_id: ModRevisionId,
        expected_manifest_digest: String,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, UninstallTaskRunError> {
        self.run_uninstall_task_internal(
            task_id,
            request,
            Some(expected_installed_revision_id),
            Some(expected_manifest_digest),
            observer,
        )
    }

    fn run_uninstall_task_internal<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: StartUninstallTaskRequest,
        expected_installed_revision_id: Option<ModRevisionId>,
        expected_manifest_digest: Option<String>,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, UninstallTaskRunError> {
        if self.task_manager.start_task(task_id).is_err() {
            return Err(UninstallTaskRunError {
                events: Vec::new(),
                uninstall_error: None,
                committed: false,
            });
        }

        let mut events = Vec::new();
        let uninstall_result = {
            let _cross_process_guard = match self.write_locks.acquire_cross_process_for_task(
                &request.game_id,
                &request.profile_id,
                &self.task_manager,
                task_id,
            ) {
                Ok(guard) => guard,
                Err(error) => {
                    if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                        return Ok(events);
                    }
                    return Err(self.fail_uninstall_with_audit(
                        task_id,
                        &request,
                        events,
                        observer,
                        error.code(),
                        None,
                        None,
                    ));
                }
            };
            let write_lock = self
                .write_locks
                .lock_for(&request.game_id, &request.profile_id);
            let _guard = write_lock.lock().map_err(|_| {
                self.fail_uninstall_with_audit(
                    task_id,
                    &request,
                    events.clone(),
                    observer,
                    "lock",
                    None,
                    None,
                )
            })?;
            if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                return Ok(events);
            }
            match self
                .write_admission
                .ensure_write_allowed(&request.game_id, &request.profile_id)
            {
                Ok(()) => {
                    observe_task_progress(
                        &mut events,
                        observer,
                        running_event(task_id, INSTALL_UNINSTALL_PROCESSING_PHASE),
                    );
                    if self.task_manager.block_task_cancellation(task_id).is_err() {
                        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                            return Ok(events);
                        }
                        return Err(self.fail_uninstall_with_audit(
                            task_id, &request, events, observer, "lock", None, None,
                        ));
                    }
                    Ok(
                        match (
                            expected_installed_revision_id.clone(),
                            expected_manifest_digest.as_deref(),
                        ) {
                            (Some(revision_id), Some(manifest_digest)) => {
                                self.uninstaller.uninstall_mod_for_revision_and_manifest(
                                    request.clone(),
                                    revision_id,
                                    manifest_digest,
                                )
                            }
                            (Some(revision_id), None) => self
                                .uninstaller
                                .uninstall_mod_for_revision(request.clone(), revision_id),
                            (None, _) => self.uninstaller.uninstall_mod(request.clone()),
                        },
                    )
                }
                Err(error) => Err(error),
            }
        };
        let uninstall_result = match uninstall_result {
            Ok(result) => result,
            Err(error) => {
                return Err(self.fail_uninstall_with_audit(
                    task_id,
                    &request,
                    events,
                    observer,
                    error.failure_phase(),
                    None,
                    None,
                ))
            }
        };

        let result = match uninstall_result {
            Ok(result) => result,
            Err(error) => {
                return Err(self.fail_uninstall_with_audit(
                    task_id,
                    &request,
                    events,
                    observer,
                    "uninstall",
                    None,
                    Some(error),
                ))
            }
        };

        match self.task_manager.complete_task(task_id) {
            Ok(task) => {
                let audit_ok =
                    self.record_uninstall_audit(task_id, &request, "success", Some(&result), None);
                let mut event = TaskProgressEvent::new(
                    task.task_id,
                    task.kind,
                    task.status,
                    INSTALL_UNINSTALL_COMPLETED_PHASE,
                );
                if !audit_ok {
                    event.error = Some("install_audit_unavailable".to_owned());
                }
                observe_task_progress(&mut events, observer, event);
                Ok(events)
            }
            Err(_) if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) => {
                Ok(events)
            }
            Err(_) => Err(self.fail_uninstall_with_audit(
                task_id,
                &request,
                events,
                observer,
                "complete",
                Some(&result),
                None,
            )),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn fail_uninstall_with_audit<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &StartUninstallTaskRequest,
        mut events: Vec<TaskProgressEvent>,
        observer: &O,
        phase: &str,
        result: Option<&UninstallModResult>,
        uninstall_error: Option<UninstallModError>,
    ) -> UninstallTaskRunError {
        if matches!(
            self.task_manager.fail_task(task_id),
            Err(TaskManagerError::TaskCannotTransition {
                from: TaskStatus::Cancelled,
                to: TaskStatus::Failed,
                ..
            })
        ) {
            return UninstallTaskRunError {
                events,
                uninstall_error,
                committed: result.is_some(),
            };
        }
        let error_code = format!("{INSTALL_UNINSTALL_FAILED_ERROR}:{phase}");
        observe_task_progress(
            &mut events,
            observer,
            failed_uninstall_event(task_id, phase),
        );
        self.record_uninstall_audit(task_id, request, "failure", result, Some(&error_code));
        UninstallTaskRunError {
            events,
            uninstall_error,
            committed: result.is_some(),
        }
    }

    fn record_uninstall_audit(
        &self,
        task_id: &str,
        request: &StartUninstallTaskRequest,
        result: &str,
        uninstall_result: Option<&UninstallModResult>,
        error_code: Option<&str>,
    ) -> bool {
        let timestamp_unix_millis = self.clock.now_unix_millis().unwrap_or_default();
        let mut fields = BTreeMap::new();
        fields.insert("task_id".to_owned(), task_id.to_owned());
        fields.insert("game_id".to_owned(), request.game_id.as_str().to_owned());
        fields.insert("mod_id".to_owned(), request.mod_id.as_str().to_owned());
        fields.insert(
            "profile_id".to_owned(),
            request.profile_id.as_str().to_owned(),
        );
        fields.insert(
            "removed_file_count".to_owned(),
            uninstall_result
                .map(|result| result.removed_file_count)
                .unwrap_or_default()
                .to_string(),
        );
        fields.insert(
            "restored_file_count".to_owned(),
            uninstall_result
                .map(|result| result.restored_file_count)
                .unwrap_or_default()
                .to_string(),
        );
        if let Some(error_code) = error_code {
            fields.insert("error_code".to_owned(), error_code.to_owned());
        }

        let policy = AuditWriteFailurePolicy::for_commit_result(result);
        self.audit_log
            .record_with_policy(
                AuditLogEvent {
                    timestamp_unix_millis,
                    category: "install".to_owned(),
                    operation: "uninstall_mod".to_owned(),
                    result: result.to_owned(),
                    fields,
                },
                policy,
            )
            .is_ok()
    }
}

impl RecoveryActionTaskRunner {
    pub fn new(
        task_manager: Arc<TaskManager>,
        action_executor: Arc<dyn InstallRecoveryActionExecutor>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self::with_write_locks(
            task_manager,
            action_executor,
            audit_log,
            clock,
            Arc::new(GameProfileWriteLockRegistry::default()),
        )
    }

    pub fn with_write_locks(
        task_manager: Arc<TaskManager>,
        action_executor: Arc<dyn InstallRecoveryActionExecutor>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
    ) -> Self {
        Self::with_write_coordination(
            task_manager,
            action_executor,
            audit_log,
            clock,
            write_locks,
            Arc::new(AllowInstallWriteAdmission),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_write_coordination(
        task_manager: Arc<TaskManager>,
        action_executor: Arc<dyn InstallRecoveryActionExecutor>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
        write_admission: Arc<dyn InstallWriteAdmission>,
    ) -> Self {
        Self {
            task_manager,
            action_executor,
            audit_log,
            clock,
            write_locks,
            write_admission,
        }
    }

    pub fn run_recovery_action_task(
        &self,
        task_id: &str,
        request: StartRecoveryActionTaskRequest,
    ) -> Result<Vec<TaskProgressEvent>, RecoveryActionTaskRunError> {
        let observer = noop_task_progress_observer();
        self.run_recovery_action_task_with_observer(task_id, request, &observer)
    }

    pub fn run_recovery_action_task_with_observer<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: StartRecoveryActionTaskRequest,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, RecoveryActionTaskRunError> {
        if self.task_manager.start_task(task_id).is_err() {
            return Err(RecoveryActionTaskRunError { events: Vec::new() });
        }

        let mut events = Vec::new();
        observe_task_progress(
            &mut events,
            observer,
            running_event(task_id, INSTALL_RECOVERY_PLANNING_PHASE),
        );
        let action_result = {
            let _cross_process_guard = match self.write_locks.acquire_cross_process_for_task(
                &request.game_id,
                &request.profile_id,
                &self.task_manager,
                task_id,
            ) {
                Ok(guard) => guard,
                Err(error) => {
                    if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                        return Ok(events);
                    }
                    return Err(self.fail_recovery_action_with_audit(
                        task_id,
                        &request,
                        events,
                        observer,
                        error.code(),
                        None,
                    ));
                }
            };
            let write_lock = self
                .write_locks
                .lock_for(&request.game_id, &request.profile_id);
            let _guard = write_lock.lock().map_err(|_| {
                self.fail_recovery_action_with_audit(
                    task_id,
                    &request,
                    events.clone(),
                    observer,
                    "lock",
                    None,
                )
            })?;
            if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                return Ok(events);
            }
            if let Err(error) = self
                .write_admission
                .ensure_write_allowed(&request.game_id, &request.profile_id)
            {
                return Err(self.fail_recovery_action_with_audit(
                    task_id,
                    &request,
                    events,
                    observer,
                    error.failure_phase(),
                    None,
                ));
            }
            if self.task_manager.block_task_cancellation(task_id).is_err() {
                if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                    return Ok(events);
                }
                return Err(self.fail_recovery_action_with_audit(
                    task_id, &request, events, observer, "lock", None,
                ));
            }
            self.action_executor.run_recovery_action(request.clone())
        };

        let result = match action_result {
            Ok(result) => {
                observe_task_progress(
                    &mut events,
                    observer,
                    running_event(task_id, INSTALL_RECOVERY_PROCESSING_PHASE),
                );
                result
            }
            Err(error) => {
                if recovery_action_failed_phase(&error) == "processing" {
                    observe_task_progress(
                        &mut events,
                        observer,
                        running_event(task_id, INSTALL_RECOVERY_PROCESSING_PHASE),
                    );
                }
                return Err(self.fail_recovery_action_with_audit(
                    task_id,
                    &request,
                    events,
                    observer,
                    recovery_action_failed_phase(&error),
                    None,
                ));
            }
        };

        self.record_recovery_action_audit(task_id, &request, "success", Some(&result), None);
        match self.task_manager.complete_task(task_id) {
            Ok(task) => {
                observe_task_progress(
                    &mut events,
                    observer,
                    TaskProgressEvent::new(
                        task.task_id,
                        task.kind,
                        task.status,
                        INSTALL_RECOVERY_COMPLETED_PHASE,
                    ),
                );
                Ok(events)
            }
            Err(_) if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) => {
                Ok(events)
            }
            Err(_) => Err(self.fail_recovery_action_with_audit(
                task_id,
                &request,
                events,
                observer,
                "complete",
                Some(&result),
            )),
        }
    }

    fn fail_recovery_action_with_audit<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &StartRecoveryActionTaskRequest,
        mut events: Vec<TaskProgressEvent>,
        observer: &O,
        phase: &str,
        result: Option<&InstallRecoveryActionResult>,
    ) -> RecoveryActionTaskRunError {
        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
            return RecoveryActionTaskRunError { events };
        }

        let _ = self.task_manager.fail_task(task_id);
        let error_code = format!("{INSTALL_RECOVERY_FAILED_ERROR}:{phase}");
        observe_task_progress(
            &mut events,
            observer,
            failed_recovery_action_event(task_id, phase),
        );
        self.record_recovery_action_audit(task_id, request, "failure", result, Some(&error_code));
        RecoveryActionTaskRunError { events }
    }

    fn record_recovery_action_audit(
        &self,
        task_id: &str,
        request: &StartRecoveryActionTaskRequest,
        result: &str,
        action_result: Option<&InstallRecoveryActionResult>,
        error_code: Option<&str>,
    ) {
        let timestamp_unix_millis = self.clock.now_unix_millis().unwrap_or_default();
        let mut fields = BTreeMap::new();
        fields.insert("task_id".to_owned(), task_id.to_owned());
        fields.insert("game_id".to_owned(), request.game_id.as_str().to_owned());
        fields.insert("mod_id".to_owned(), request.mod_id.as_str().to_owned());
        fields.insert(
            "profile_id".to_owned(),
            request.profile_id.as_str().to_owned(),
        );
        fields.insert(
            "remove_file_count".to_owned(),
            action_result
                .map(|result| result.remove_file_count)
                .unwrap_or_default()
                .to_string(),
        );
        fields.insert(
            "restore_file_count".to_owned(),
            action_result
                .map(|result| result.restore_file_count)
                .unwrap_or_default()
                .to_string(),
        );
        fields.insert(
            "backup_count".to_owned(),
            action_result
                .map(|result| result.backup_count)
                .unwrap_or_default()
                .to_string(),
        );
        if let Some(error_code) = error_code {
            fields.insert("error_code".to_owned(), error_code.to_owned());
        }

        let policy = AuditWriteFailurePolicy::for_commit_result(result);
        let _ = self.audit_log.record_with_policy(
            AuditLogEvent {
                timestamp_unix_millis,
                category: "install".to_owned(),
                operation: recovery_action_operation(request.action_kind).to_owned(),
                result: result.to_owned(),
                fields,
            },
            policy,
        );
    }
}

pub struct GameProfileWriteLockRegistry {
    locks: Mutex<HashMap<GameProfileLockKey, GameProfileLock>>,
    cross_process: Arc<CrossProcessWriteAdmissionCoordinator>,
}

type GameProfileLockKey = (String, String);
type GameProfileLock = Arc<Mutex<()>>;

impl GameProfileWriteLockRegistry {
    pub fn with_cross_process_admission(
        cross_process: Arc<CrossProcessWriteAdmissionCoordinator>,
    ) -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
            cross_process,
        }
    }

    pub fn lock_for(&self, game_id: &GameId, profile_id: &ProfileId) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().expect("write lock registry");
        locks
            .entry((game_id.as_str().to_owned(), profile_id.as_str().to_owned()))
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    pub fn acquire_cross_process_for_task(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        task_manager: &TaskManager,
        task_id: &str,
    ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>> {
        self.cross_process.acquire_game_profile_for_task(
            game_id,
            profile_id,
            task_manager,
            task_id,
        )
    }

    pub fn acquire_cross_process(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>> {
        self.cross_process
            .acquire_game_profile(game_id, profile_id, &NeverCancelled)
    }
}

impl Default for GameProfileWriteLockRegistry {
    fn default() -> Self {
        Self::with_cross_process_admission(Arc::new(
            CrossProcessWriteAdmissionCoordinator::process_local_compatibility(),
        ))
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
        INSTALL_FAILED_PHASE,
    );
    event.error = Some(format!("{INSTALL_FAILED_ERROR}:{failed_phase}"));
    event
}

fn failed_uninstall_event(task_id: &str, failed_phase: &str) -> TaskProgressEvent {
    let mut event = TaskProgressEvent::new(
        task_id.to_owned(),
        TaskKind::Install,
        TaskStatus::Failed,
        INSTALL_UNINSTALL_FAILED_PHASE,
    );
    event.error = Some(format!("{INSTALL_UNINSTALL_FAILED_ERROR}:{failed_phase}"));
    event
}

fn failed_recovery_action_event(task_id: &str, failed_phase: &str) -> TaskProgressEvent {
    let mut event = TaskProgressEvent::new(
        task_id.to_owned(),
        TaskKind::Install,
        TaskStatus::Failed,
        INSTALL_RECOVERY_FAILED_PHASE,
    );
    event.error = Some(format!("{INSTALL_RECOVERY_FAILED_ERROR}:{failed_phase}"));
    event
}

fn recovery_action_operation(action_kind: InstallRecoveryActionKind) -> &'static str {
    match action_kind {
        InstallRecoveryActionKind::RollbackInstall => "rollback_install",
        InstallRecoveryActionKind::ReconcileReinstall => "reconcile_reinstall",
    }
}

fn recovery_action_failed_phase(error: &InstallRecoveryActionError) -> &'static str {
    match error {
        InstallRecoveryActionError::ActionUnavailable
        | InstallRecoveryActionError::Blocked { .. } => "planning",
        InstallRecoveryActionError::RemoveFailed
        | InstallRecoveryActionError::RestoreFailed
        | InstallRecoveryActionError::ManifestSaveFailed
        | InstallRecoveryActionError::RecoveryRecordSaveFailed
        | InstallRecoveryActionError::RollbackFailed { .. }
        | InstallRecoveryActionError::ReinstallRepairRequired
        | InstallRecoveryActionError::ReinstallPostCommitFailed
        | InstallRecoveryActionError::ReinstallCleanupFailed => "processing",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildImportedModInstallPlanRequest, ImportedModInstallCommitRequest, InstallCommitError,
        InstallCommitResult, InstallPlanningError,
    };
    use hmm_core::{
        InstallFileProvider, InstallManifest, InstallManifestEntry, InstallPlan, InstallTargetPath,
        ModRevisionId, PackageFileId, ReinstallRecoveryTransaction,
        ReinstallRecoveryTransactionStatus,
    };
    use hmm_ports::{
        AppClock, AuditLogEvent, AuditLogWriter, CancellationToken,
        CrossProcessWriteAdmission, CrossProcessWriteAdmissionError,
        CrossProcessWriteAdmissionResult, CrossProcessWriteGuard, CrossProcessWriteScope,
    };
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn start_install_task_returns_queued_install_task_without_leaking_inputs() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let service = InstallTaskService::new(Arc::clone(&task_manager));

        let task = service
            .start_install_task(StartInstallTaskRequest {
                game_id: GameId::mhw(),
                mod_id: ModId::new("visible-mod-id"),
                profile_id: ProfileId::new("default-profile"),
                layer: FileLayer::new("base", 0),
            })
            .expect("install task starts");

        assert!(task.task_id.starts_with("install-"));
        assert!(!task.task_id.contains("visible-mod-id"));
        assert!(!task.task_id.contains("default-profile"));
        assert_eq!(task.kind, crate::TaskKind::Install);
        assert_eq!(task.status, crate::TaskStatus::Queued);
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Queued)
        );
    }

    #[test]
    fn start_uninstall_task_returns_queued_install_task_without_leaking_inputs() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let service = UninstallTaskService::new(Arc::clone(&task_manager));

        let task = service
            .start_uninstall_task(StartUninstallTaskRequest {
                game_id: GameId::mhw(),
                mod_id: ModId::new("visible-mod-id"),
                profile_id: ProfileId::new("default-profile"),
            })
            .expect("uninstall task starts");

        assert!(task.task_id.starts_with("install-"));
        assert!(!task.task_id.contains("visible-mod-id"));
        assert!(!task.task_id.contains("default-profile"));
        assert_eq!(task.kind, crate::TaskKind::Install);
        assert_eq!(task.status, crate::TaskStatus::Queued);
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Queued)
        );
    }

    #[test]
    fn start_recovery_action_task_returns_queued_install_task_without_leaking_inputs() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let service = RecoveryActionTaskService::new(Arc::clone(&task_manager));

        let task = service
            .start_recovery_action_task(StartRecoveryActionTaskRequest {
                game_id: GameId::mhw(),
                mod_id: ModId::new("visible-mod-id"),
                profile_id: ProfileId::new("default-profile"),
                action_kind: crate::InstallRecoveryActionKind::RollbackInstall,
            })
            .expect("recovery action task starts");

        assert!(task.task_id.starts_with("install-"));
        assert!(!task.task_id.contains("visible-mod-id"));
        assert!(!task.task_id.contains("default-profile"));
        assert_eq!(task.kind, crate::TaskKind::Install);
        assert_eq!(task.status, crate::TaskStatus::Queued);
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Queued)
        );
    }

    #[test]
    fn reinstall_recovery_write_admission_distinguishes_empty_pending_and_unavailable() {
        let cases = [
            (AdmissionRepositoryMode::Empty, Ok(())),
            (
                AdmissionRepositoryMode::Pending,
                Err(InstallWriteAdmissionError::RecoveryPending),
            ),
            (
                AdmissionRepositoryMode::Unavailable,
                Err(InstallWriteAdmissionError::RecoveryUnavailable),
            ),
        ];

        for (mode, expected) in cases {
            let admission =
                ReinstallRecoveryWriteAdmission::new(Arc::new(AdmissionRecoveryRepository {
                    mode,
                }));

            assert_eq!(
                admission.ensure_write_allowed(&GameId::mhw(), &ProfileId::new("default")),
                expected
            );
        }
    }

    #[test]
    fn cancelled_install_and_uninstall_are_not_finalized_as_failures() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let install_task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("install task can be created");
        task_manager
            .start_task(&install_task.task_id)
            .expect("task starts");
        task_manager
            .cancel_task(&install_task.task_id)
            .expect("cancellation wins");
        let install_audit = Arc::new(RecordingAuditLogWriter::default());
        let install_runner = InstallTaskRunner::new(
            Arc::clone(&task_manager),
            Arc::new(RecordingInstallPlanner::new(sample_plan())),
            Arc::new(RecordingInstallCommitter::new(sample_manifest())),
            install_audit.clone(),
            Arc::new(FixedClock),
        );

        let install_observer = noop_task_progress_observer();
        let install_error = install_runner.fail_with_audit(
            &install_task.task_id,
            &sample_request(),
            Vec::new(),
            &install_observer,
            "planning",
            0,
        );
        assert!(install_error.events.is_empty());
        assert!(install_audit.take_event().is_none());

        let uninstall_task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("uninstall task can be created");
        task_manager
            .start_task(&uninstall_task.task_id)
            .expect("task starts");
        task_manager
            .cancel_task(&uninstall_task.task_id)
            .expect("cancellation wins");
        let uninstall_audit = Arc::new(RecordingAuditLogWriter::default());
        let uninstall_runner = UninstallTaskRunner::new(
            Arc::clone(&task_manager),
            Arc::new(RecordingUninstaller::new(UninstallModResult {
                manifest: sample_manifest(),
                removed_file_count: 0,
                restored_file_count: 0,
            })),
            uninstall_audit.clone(),
            Arc::new(FixedClock),
        );

        let uninstall_observer = noop_task_progress_observer();
        let uninstall_error = uninstall_runner.fail_uninstall_with_audit(
            &uninstall_task.task_id,
            &sample_uninstall_request(),
            Vec::new(),
            &uninstall_observer,
            "uninstall",
            None,
            None,
        );
        assert!(uninstall_error.events.is_empty());
        assert!(uninstall_audit.take_event().is_none());
    }

    #[test]
    fn run_install_task_commits_imported_mod_plan_and_records_sanitized_success_audit() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let planner = Arc::new(RecordingInstallPlanner::new(sample_plan()));
        let committer = Arc::new(RecordingInstallCommitter::new(sample_manifest()));
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let runner = InstallTaskRunner::new(
            Arc::clone(&task_manager),
            planner.clone(),
            committer.clone(),
            audit_log.clone(),
            Arc::new(FixedClock),
        );
        let request = sample_request();

        let events = runner
            .run_install_task(&task.task_id, request.clone())
            .expect("install task succeeds");

        assert_eq!(
            events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec![
                "install.plan.building",
                "install.commit.processing",
                "install.completed"
            ]
        );
        assert!(events.iter().all(|event| event.task_id == task.task_id));
        assert!(events
            .iter()
            .all(|event| event.kind == crate::TaskKind::Install));
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Completed)
        );
        assert_eq!(
            planner.take_requests(),
            vec![BuildImportedModInstallPlanRequest {
                game_id: request.game_id.clone(),
                mod_id: request.mod_id.clone(),
                layer: request.layer.clone(),
            }]
        );
        assert_eq!(committer.take_profiles(), vec!["default".to_owned()]);

        let event = audit_log.take_event().expect("audit event recorded");
        assert_eq!(event.timestamp_unix_millis, 42);
        assert_eq!(event.category, "install");
        assert_eq!(event.operation, "commit_imported_mod");
        assert_eq!(event.result, "success");
        assert_eq!(event.fields["task_id"], task.task_id);
        assert_eq!(event.fields["game_id"], "mhw");
        assert_eq!(event.fields["mod_id"], "mod-a");
        assert_eq!(event.fields["profile_id"], "default");
        assert_eq!(event.fields["action_count"], "1");
        let serialized = serde_json::to_string(&event).expect("serialize audit event");
        assert!(!serialized.contains("nativePC/models/player.mod3"));
        assert!(!serialized.contains("C:/"));
        assert!(!serialized.contains('\\'));
    }

    #[test]
    fn cross_process_busy_stops_install_before_committer() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let committer = Arc::new(RecordingInstallCommitter::new(sample_manifest()));
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let coordinator = Arc::new(CrossProcessWriteAdmissionCoordinator::with_timeout(
            Arc::new(RejectingCrossProcessAdmission(
                CrossProcessWriteAdmissionError::Busy,
            )),
            Duration::from_millis(1),
        ));
        let runner = InstallTaskRunner::with_write_locks(
            Arc::clone(&task_manager),
            Arc::new(RecordingInstallPlanner::new(sample_plan())),
            committer.clone(),
            audit_log.clone(),
            Arc::new(FixedClock),
            Arc::new(GameProfileWriteLockRegistry::with_cross_process_admission(
                coordinator,
            )),
        );

        let error = runner
            .run_install_task(&task.task_id, sample_request())
            .expect_err("busy admission must reject install");

        assert!(committer.take_profiles().is_empty());
        assert_eq!(
            error.events.last().and_then(|event| event.error.as_deref()),
            Some("install_failed:write_admission_busy")
        );
        assert_eq!(
            audit_log
                .take_event()
                .expect("failure audit")
                .fields["error_code"],
            "install_failed:write_admission_busy"
        );
    }

    #[test]
    fn successful_install_keeps_file_fact_and_reports_audit_degradation() {
        struct FailingAuditLogWriter;

        impl AuditLogWriter for FailingAuditLogWriter {
            fn record(&self, _event: AuditLogEvent) -> anyhow::Result<()> {
                anyhow::bail!("audit unavailable")
            }
        }

        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let runner = InstallTaskRunner::new(
            Arc::clone(&task_manager),
            Arc::new(RecordingInstallPlanner::new(sample_plan())),
            Arc::new(RecordingInstallCommitter::new(sample_manifest())),
            Arc::new(FailingAuditLogWriter),
            Arc::new(FixedClock),
        );

        let events = runner
            .run_install_task(&task.task_id, sample_request())
            .expect("audit failure cannot rewrite a committed install as a failure");

        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Completed)
        );
        assert_eq!(
            events.last().and_then(|event| event.error.as_deref()),
            Some("install_audit_unavailable")
        );
    }

    #[test]
    fn run_install_task_passes_current_plan_to_admission_before_commit() {
        struct RejectingPlanAdmission {
            expected_plan: InstallPlan,
            expected_decision: GamePrerequisiteDecision,
        }

        impl InstallWriteAdmission for RejectingPlanAdmission {
            fn ensure_write_allowed(
                &self,
                _game_id: &GameId,
                _profile_id: &ProfileId,
            ) -> Result<(), InstallWriteAdmissionError> {
                panic!("install runner must use plan-aware admission")
            }

            fn ensure_install_plan_allowed(
                &self,
                game_id: &GameId,
                profile_id: &ProfileId,
                mod_id: &ModId,
                plan: &InstallPlan,
                prerequisite_decision: &GamePrerequisiteDecision,
            ) -> Result<(), InstallWriteAdmissionError> {
                assert_eq!(game_id, &GameId::mhw());
                assert_eq!(profile_id, &ProfileId::new("default"));
                assert_eq!(mod_id, &ModId::new("mod-a"));
                assert_eq!(plan, &self.expected_plan);
                assert_eq!(prerequisite_decision, &self.expected_decision);
                Err(InstallWriteAdmissionError::SafetyRejected)
            }
        }

        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("install task can be created");
        let plan = sample_plan();
        let prerequisite_decision = ready_prerequisite_decision();
        let committer = Arc::new(RecordingInstallCommitter::new(sample_manifest()));
        let runner = InstallTaskRunner::with_write_coordination(
            Arc::clone(&task_manager),
            Arc::new(RecordingInstallPlanner::with_decisions(
                plan.clone(),
                prerequisite_decision.clone(),
                prerequisite_decision.clone(),
            )),
            committer.clone(),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            Arc::new(GameProfileWriteLockRegistry::default()),
            Arc::new(RejectingPlanAdmission {
                expected_plan: plan,
                expected_decision: prerequisite_decision,
            }),
        );

        let error = runner
            .run_install_task(&task.task_id, sample_request())
            .expect_err("admission rejection must stop commit");

        assert_eq!(
            error
                .events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec!["install.plan.building", "install.failed"]
        );
        assert!(committer.take_profiles().is_empty());
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Failed)
        );
    }

    #[test]
    fn run_install_task_with_observer_streams_events_and_ignores_observer_failure() {
        #[derive(Default)]
        struct FailingObserver {
            phases: Mutex<Vec<String>>,
        }

        impl TaskProgressObserver for FailingObserver {
            type Error = &'static str;

            fn observe(&self, event: &TaskProgressEvent) -> Result<(), Self::Error> {
                self.phases
                    .lock()
                    .expect("observer lock")
                    .push(event.phase.clone());
                Err("fixture observer failure")
            }
        }

        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("install task can be created");
        let committer = Arc::new(RecordingInstallCommitter::new(sample_manifest()));
        let runner = InstallTaskRunner::new(
            Arc::clone(&task_manager),
            Arc::new(RecordingInstallPlanner::new(sample_plan())),
            committer.clone(),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
        );
        let observer = FailingObserver::default();

        let events = runner
            .run_install_task_with_observer(&task.task_id, sample_request(), &observer)
            .expect("observer failure does not fail install");

        let phases = observer.phases.lock().expect("observer lock").clone();
        assert_eq!(
            phases,
            vec![
                "install.plan.building",
                "install.commit.processing",
                "install.completed"
            ]
        );
        assert_eq!(
            events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            phases.iter().map(String::as_str).collect::<Vec<_>>()
        );
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Completed)
        );
        assert_eq!(committer.take_profiles(), vec!["default".to_owned()]);
    }

    #[test]
    fn run_uninstall_task_executes_uninstall_and_records_sanitized_success_audit() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let uninstaller = Arc::new(RecordingUninstaller::new(UninstallModResult {
            manifest: sample_manifest(),
            removed_file_count: 1,
            restored_file_count: 0,
        }));
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let runner = UninstallTaskRunner::new(
            Arc::clone(&task_manager),
            uninstaller.clone(),
            audit_log.clone(),
            Arc::new(FixedClock),
        );
        let request = StartUninstallTaskRequest {
            game_id: GameId::mhw(),
            mod_id: ModId::new("mod-a"),
            profile_id: ProfileId::new("default"),
        };

        let events = runner
            .run_uninstall_task(&task.task_id, request.clone())
            .expect("uninstall task succeeds");

        assert_eq!(
            events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec![
                "install.uninstall.processing",
                "install.uninstall.completed"
            ]
        );
        assert!(events.iter().all(|event| event.task_id == task.task_id));
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Completed)
        );
        assert_eq!(uninstaller.take_requests(), vec![request.clone()]);

        let event = audit_log.take_event().expect("audit event recorded");
        assert_eq!(event.timestamp_unix_millis, 42);
        assert_eq!(event.category, "install");
        assert_eq!(event.operation, "uninstall_mod");
        assert_eq!(event.result, "success");
        assert_eq!(event.fields["task_id"], task.task_id);
        assert_eq!(event.fields["game_id"], "mhw");
        assert_eq!(event.fields["mod_id"], "mod-a");
        assert_eq!(event.fields["profile_id"], "default");
        assert_eq!(event.fields["removed_file_count"], "1");
        assert_eq!(event.fields["restored_file_count"], "0");
        let serialized = serde_json::to_string(&event).expect("serialize audit event");
        assert!(!serialized.contains("nativePC/models/player.mod3"));
        assert!(!serialized.contains("C:/"));
        assert!(!serialized.contains('\\'));
    }

    #[test]
    fn uninstall_terminal_transition_failure_records_only_failure_audit() {
        struct TerminalTransitionFailingUninstaller {
            task_manager: Arc<crate::TaskManager>,
            task_id: String,
            result: UninstallModResult,
        }

        impl ModUninstaller for TerminalTransitionFailingUninstaller {
            fn uninstall_mod(
                &self,
                _request: StartUninstallTaskRequest,
            ) -> Result<UninstallModResult, UninstallModError> {
                self.task_manager
                    .fail_task(&self.task_id)
                    .expect("injected terminal transition");
                Ok(self.result.clone())
            }

            fn uninstall_mod_for_revision(
                &self,
                request: StartUninstallTaskRequest,
                _expected_installed_revision_id: ModRevisionId,
            ) -> Result<UninstallModResult, UninstallModError> {
                self.uninstall_mod(request)
            }

            fn uninstall_mod_for_revision_and_manifest(
                &self,
                request: StartUninstallTaskRequest,
                _expected_installed_revision_id: ModRevisionId,
                _expected_manifest_digest: &str,
            ) -> Result<UninstallModResult, UninstallModError> {
                self.uninstall_mod(request)
            }
        }

        #[derive(Default)]
        struct CollectingAuditLogWriter {
            events: Mutex<Vec<AuditLogEvent>>,
        }

        impl AuditLogWriter for CollectingAuditLogWriter {
            fn record(&self, event: AuditLogEvent) -> anyhow::Result<()> {
                self.events.lock().expect("events").push(event);
                Ok(())
            }
        }

        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let audit_log = Arc::new(CollectingAuditLogWriter::default());
        let runner = UninstallTaskRunner::new(
            Arc::clone(&task_manager),
            Arc::new(TerminalTransitionFailingUninstaller {
                task_manager: Arc::clone(&task_manager),
                task_id: task.task_id.clone(),
                result: UninstallModResult {
                    manifest: sample_manifest(),
                    removed_file_count: 1,
                    restored_file_count: 0,
                },
            }),
            audit_log.clone(),
            Arc::new(FixedClock),
        );

        runner
            .run_uninstall_task(&task.task_id, sample_uninstall_request())
            .expect_err("completion transition must fail");

        let events = std::mem::take(&mut *audit_log.events.lock().expect("events"));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].operation, "uninstall_mod");
        assert_eq!(events[0].result, "failure");
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Failed)
        );
    }

    #[test]
    fn uninstall_processing_is_emitted_only_after_write_admission() {
        struct RecordingObserver {
            phases: Arc<Mutex<Vec<String>>>,
        }

        impl TaskProgressObserver for RecordingObserver {
            type Error = ();

            fn observe(&self, event: &TaskProgressEvent) -> Result<(), Self::Error> {
                self.phases
                    .lock()
                    .expect("phases")
                    .push(event.phase.clone());
                Ok(())
            }
        }

        struct AdmissionBeforeProcessing {
            phases: Arc<Mutex<Vec<String>>>,
        }

        impl InstallWriteAdmission for AdmissionBeforeProcessing {
            fn ensure_write_allowed(
                &self,
                _game_id: &GameId,
                _profile_id: &ProfileId,
            ) -> Result<(), InstallWriteAdmissionError> {
                assert!(
                    self.phases.lock().expect("phases").is_empty(),
                    "processing must describe the write-locked commit, not lock waiting"
                );
                Ok(())
            }
        }

        let phases = Arc::new(Mutex::new(Vec::new()));
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let runner = UninstallTaskRunner::with_write_coordination(
            Arc::clone(&task_manager),
            Arc::new(RecordingUninstaller::new(UninstallModResult {
                manifest: sample_manifest(),
                removed_file_count: 1,
                restored_file_count: 0,
            })),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            Arc::new(GameProfileWriteLockRegistry::default()),
            Arc::new(AdmissionBeforeProcessing {
                phases: Arc::clone(&phases),
            }),
        );

        runner
            .run_uninstall_task_with_observer(
                &task.task_id,
                sample_uninstall_request(),
                &RecordingObserver {
                    phases: Arc::clone(&phases),
                },
            )
            .expect("uninstall succeeds");

        assert_eq!(
            *phases.lock().expect("phases"),
            vec![
                "install.uninstall.processing".to_owned(),
                "install.uninstall.completed".to_owned()
            ]
        );
    }

    #[test]
    fn run_recovery_action_task_executes_action_and_records_sanitized_success_audit() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let action_executor = Arc::new(RecordingRecoveryActionExecutor::new(
            crate::InstallRecoveryActionResult {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                action_kind: crate::InstallRecoveryActionKind::RollbackInstall,
                remove_file_count: 1,
                restore_file_count: 1,
                backup_count: 1,
            },
        ));
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let runner = RecoveryActionTaskRunner::new(
            Arc::clone(&task_manager),
            action_executor.clone(),
            audit_log.clone(),
            Arc::new(FixedClock),
        );
        let request = sample_recovery_action_request();

        let events = runner
            .run_recovery_action_task(&task.task_id, request.clone())
            .expect("recovery action task succeeds");

        assert_eq!(
            events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec![
                "install.recovery.planning",
                "install.recovery.processing",
                "install.recovery.completed"
            ]
        );
        assert!(events.iter().all(|event| event.task_id == task.task_id));
        assert!(events
            .iter()
            .all(|event| event.kind == crate::TaskKind::Install));
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Completed)
        );
        assert_eq!(action_executor.take_requests(), vec![request.clone()]);

        let event = audit_log.take_event().expect("audit event recorded");
        assert_eq!(event.timestamp_unix_millis, 42);
        assert_eq!(event.category, "install");
        assert_eq!(event.operation, "rollback_install");
        assert_eq!(event.result, "success");
        assert_eq!(event.fields["task_id"], task.task_id);
        assert_eq!(event.fields["game_id"], "mhw");
        assert_eq!(event.fields["mod_id"], "mod-a");
        assert_eq!(event.fields["profile_id"], "default");
        assert_eq!(event.fields["remove_file_count"], "1");
        assert_eq!(event.fields["restore_file_count"], "1");
        assert_eq!(event.fields["backup_count"], "1");
        let serialized = serde_json::to_string(&event).expect("serialize audit event");
        assert!(!serialized.contains("nativePC/models/player.mod3"));
        assert!(!serialized.contains("backup-original"));
        assert!(!serialized.contains("C:/"));
        assert!(!serialized.contains('\\'));
    }

    #[test]
    fn run_recovery_action_task_reports_blocked_failure_without_paths() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let action_executor = Arc::new(FailingRecoveryActionExecutor {
            error: crate::InstallRecoveryActionError::Blocked {
                reasons: vec![crate::InstallRecoveryActionBlockReasonSummary {
                    reason: crate::InstallRecoveryActionBlockReason::TargetChanged,
                    count: 1,
                }],
            },
        });
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let runner = RecoveryActionTaskRunner::new(
            Arc::clone(&task_manager),
            action_executor,
            audit_log.clone(),
            Arc::new(FixedClock),
        );

        let error = runner
            .run_recovery_action_task(&task.task_id, sample_recovery_action_request())
            .expect_err("blocked recovery action should fail the task");

        assert_eq!(
            error
                .events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec!["install.recovery.planning", "install.recovery.failed"]
        );
        let failed = error.events.last().expect("failed event");
        assert_eq!(
            failed.error.as_deref(),
            Some("install_recovery_failed:planning")
        );
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Failed)
        );

        let event = audit_log.take_event().expect("failure audit recorded");
        assert_eq!(event.operation, "rollback_install");
        assert_eq!(event.result, "failure");
        assert_eq!(event.fields["remove_file_count"], "0");
        assert_eq!(event.fields["restore_file_count"], "0");
        assert_eq!(event.fields["backup_count"], "0");
        let serialized = serde_json::to_string(&event).expect("serialize audit event");
        assert!(!serialized.contains("nativePC/models/player.mod3"));
        assert!(!serialized.contains("backup-original"));
        assert!(!serialized.contains("C:/"));
        assert!(!serialized.contains('\\'));
    }

    #[test]
    fn install_and_uninstall_share_game_profile_write_lock_when_configured() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let install_task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("install task can be created");
        let uninstall_task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("uninstall task can be created");
        let write_locks = Arc::new(GameProfileWriteLockRegistry::default());
        let (commit_started_tx, commit_started_rx) = mpsc::channel();
        let (release_commit_tx, release_commit_rx) = mpsc::channel();
        let (uninstall_started_tx, uninstall_started_rx) = mpsc::channel();

        let install_runner = InstallTaskRunner::with_write_locks(
            Arc::clone(&task_manager),
            Arc::new(RecordingInstallPlanner::new(sample_plan())),
            Arc::new(BlockingInstallCommitter {
                manifest: sample_manifest(),
                started: Mutex::new(Some(commit_started_tx)),
                release: Mutex::new(release_commit_rx),
            }),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            Arc::clone(&write_locks),
        );
        let uninstall_runner = UninstallTaskRunner::with_write_locks(
            Arc::clone(&task_manager),
            Arc::new(NotifyingUninstaller {
                result: UninstallModResult {
                    manifest: sample_manifest(),
                    removed_file_count: 1,
                    restored_file_count: 0,
                },
                started: Mutex::new(Some(uninstall_started_tx)),
            }),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            write_locks,
        );

        let install_task_id = install_task.task_id.clone();
        let install_handle = thread::spawn(move || {
            install_runner.run_install_task(&install_task_id, sample_request())
        });
        commit_started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("install commit should enter write section");

        let uninstall_task_id = uninstall_task.task_id.clone();
        let uninstall_handle = thread::spawn(move || {
            uninstall_runner.run_uninstall_task(
                &uninstall_task_id,
                StartUninstallTaskRequest {
                    game_id: GameId::mhw(),
                    mod_id: ModId::new("mod-a"),
                    profile_id: ProfileId::new("default"),
                },
            )
        });

        wait_for_status(
            &task_manager,
            &uninstall_task.task_id,
            crate::TaskStatus::Running,
        );
        assert!(
            uninstall_started_rx
                .recv_timeout(Duration::from_millis(100))
                .is_err(),
            "uninstall must wait while install commit holds the same game/profile write lock"
        );

        release_commit_tx
            .send(())
            .expect("install commit can be released");
        uninstall_started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("uninstall should enter after install releases write lock");
        assert!(install_handle
            .join()
            .expect("install thread should not panic")
            .is_ok());
        assert!(uninstall_handle
            .join()
            .expect("uninstall thread should not panic")
            .is_ok());
    }

    #[test]
    fn install_and_recovery_action_share_game_profile_write_lock_when_configured() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let install_task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("install task can be created");
        let recovery_task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("recovery action task can be created");
        let write_locks = Arc::new(GameProfileWriteLockRegistry::default());
        let (commit_started_tx, commit_started_rx) = mpsc::channel();
        let (release_commit_tx, release_commit_rx) = mpsc::channel();
        let (recovery_started_tx, recovery_started_rx) = mpsc::channel();

        let install_runner = InstallTaskRunner::with_write_locks(
            Arc::clone(&task_manager),
            Arc::new(RecordingInstallPlanner::new(sample_plan())),
            Arc::new(BlockingInstallCommitter {
                manifest: sample_manifest(),
                started: Mutex::new(Some(commit_started_tx)),
                release: Mutex::new(release_commit_rx),
            }),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            Arc::clone(&write_locks),
        );
        let recovery_runner = RecoveryActionTaskRunner::with_write_locks(
            Arc::clone(&task_manager),
            Arc::new(NotifyingRecoveryActionExecutor {
                result: crate::InstallRecoveryActionResult {
                    profile_id: ProfileId::new("default"),
                    mod_id: ModId::new("mod-a"),
                    action_kind: crate::InstallRecoveryActionKind::RollbackInstall,
                    remove_file_count: 1,
                    restore_file_count: 1,
                    backup_count: 1,
                },
                started: Mutex::new(Some(recovery_started_tx)),
            }),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            write_locks,
        );

        let install_task_id = install_task.task_id.clone();
        let install_handle = thread::spawn(move || {
            install_runner.run_install_task(&install_task_id, sample_request())
        });
        commit_started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("install commit should enter write section");

        let recovery_task_id = recovery_task.task_id.clone();
        let recovery_handle = thread::spawn(move || {
            recovery_runner
                .run_recovery_action_task(&recovery_task_id, sample_recovery_action_request())
        });

        wait_for_status(
            &task_manager,
            &recovery_task.task_id,
            crate::TaskStatus::Running,
        );
        assert!(
            recovery_started_rx
                .recv_timeout(Duration::from_millis(100))
                .is_err(),
            "recovery action must wait while install commit holds the same game/profile write lock"
        );

        release_commit_tx
            .send(())
            .expect("install commit can be released");
        recovery_started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("recovery action should enter after install releases write lock");
        assert!(install_handle
            .join()
            .expect("install thread should not panic")
            .is_ok());
        assert!(recovery_handle
            .join()
            .expect("recovery action thread should not panic")
            .is_ok());
    }

    #[test]
    fn lifecycle_write_runners_reject_cancellation_after_commit_barrier() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let install_task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let install_cancellation = Arc::new(CommitCancellationProbe::new(
            Arc::clone(&task_manager),
            install_task.task_id.clone(),
        ));
        let install_runner = InstallTaskRunner::new(
            Arc::clone(&task_manager),
            Arc::new(RecordingInstallPlanner::new(sample_plan())),
            Arc::new(CancellationProbingInstallCommitter {
                cancellation: Arc::clone(&install_cancellation),
                manifest: sample_manifest(),
            }),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
        );

        let install_events = install_runner
            .run_install_task(&install_task.task_id, sample_request())
            .expect("install commit succeeds after rejecting cancellation");

        assert_eq!(
            install_events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec![
                "install.plan.building",
                "install.commit.processing",
                "install.completed"
            ]
        );
        install_cancellation.assert_rejected();
        assert_eq!(
            task_manager.task_status(&install_task.task_id),
            Some(crate::TaskStatus::Completed)
        );

        let uninstall_task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let uninstall_cancellation = Arc::new(CommitCancellationProbe::new(
            Arc::clone(&task_manager),
            uninstall_task.task_id.clone(),
        ));
        let uninstall_runner = UninstallTaskRunner::new(
            Arc::clone(&task_manager),
            Arc::new(CancellationProbingUninstaller {
                cancellation: Arc::clone(&uninstall_cancellation),
                result: UninstallModResult {
                    manifest: sample_manifest(),
                    removed_file_count: 1,
                    restored_file_count: 0,
                },
            }),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
        );

        let uninstall_events = uninstall_runner
            .run_uninstall_task(&uninstall_task.task_id, sample_uninstall_request())
            .expect("uninstall commit succeeds after rejecting cancellation");

        assert_eq!(
            uninstall_events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec![
                "install.uninstall.processing",
                "install.uninstall.completed"
            ]
        );
        uninstall_cancellation.assert_rejected();
        assert_eq!(
            task_manager.task_status(&uninstall_task.task_id),
            Some(crate::TaskStatus::Completed)
        );

        let recovery_task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let recovery_cancellation = Arc::new(CommitCancellationProbe::new(
            Arc::clone(&task_manager),
            recovery_task.task_id.clone(),
        ));
        let recovery_runner = RecoveryActionTaskRunner::new(
            Arc::clone(&task_manager),
            Arc::new(CancellationProbingRecoveryActionExecutor {
                cancellation: Arc::clone(&recovery_cancellation),
                result: crate::InstallRecoveryActionResult {
                    profile_id: ProfileId::new("default"),
                    mod_id: ModId::new("mod-a"),
                    action_kind: crate::InstallRecoveryActionKind::RollbackInstall,
                    remove_file_count: 1,
                    restore_file_count: 1,
                    backup_count: 1,
                },
            }),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
        );

        let recovery_events = recovery_runner
            .run_recovery_action_task(&recovery_task.task_id, sample_recovery_action_request())
            .expect("recovery commit succeeds after rejecting cancellation");

        assert_eq!(
            recovery_events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec![
                "install.recovery.planning",
                "install.recovery.processing",
                "install.recovery.completed"
            ]
        );
        recovery_cancellation.assert_rejected();
        assert_eq!(
            task_manager.task_status(&recovery_task.task_id),
            Some(crate::TaskStatus::Completed)
        );
    }

    #[test]
    fn run_install_task_stops_before_commit_when_cancelled_after_planning() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let planner = Arc::new(CancellingInstallPlanner {
            task_manager: Arc::clone(&task_manager),
            task_id: task.task_id.clone(),
            plan: sample_plan(),
        });
        let committer = Arc::new(RecordingInstallCommitter::new(sample_manifest()));
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let runner = InstallTaskRunner::new(
            Arc::clone(&task_manager),
            planner,
            committer.clone(),
            audit_log.clone(),
            Arc::new(FixedClock),
        );

        let events = runner
            .run_install_task(&task.task_id, sample_request())
            .expect("cancelled task should stop without failing");

        assert_eq!(
            events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec!["install.plan.building"]
        );
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Cancelled)
        );
        assert!(committer.take_profiles().is_empty());
        assert!(audit_log.take_event().is_none());
    }

    #[test]
    fn run_install_task_preserves_action_count_in_failure_audit_after_planning() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let planner = Arc::new(RecordingInstallPlanner::new(sample_plan()));
        let committer = Arc::new(FailingInstallCommitter);
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let runner = InstallTaskRunner::new(
            Arc::clone(&task_manager),
            planner,
            committer,
            audit_log.clone(),
            Arc::new(FixedClock),
        );

        let error = runner
            .run_install_task(&task.task_id, sample_request())
            .expect_err("commit failure should fail the task");

        assert_eq!(
            error
                .events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec![
                "install.plan.building",
                "install.commit.processing",
                "install.failed"
            ]
        );
        let event = audit_log.take_event().expect("failure audit recorded");
        assert_eq!(event.result, "failure");
        assert_eq!(event.fields["action_count"], "1");
        assert_eq!(event.fields["rollback_result"], "not_attempted");
    }

    #[test]
    fn run_install_task_audits_rollback_failure_without_internal_error_text() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let runner = InstallTaskRunner::new(
            Arc::clone(&task_manager),
            Arc::new(RecordingInstallPlanner::new(sample_plan())),
            Arc::new(RollbackFailedInstallCommitter),
            audit_log.clone(),
            Arc::new(FixedClock),
        );

        runner
            .run_install_task(&task.task_id, sample_request())
            .expect_err("rollback failure must fail the task");

        let event = audit_log.take_event().expect("failure audit recorded");
        assert_eq!(event.result, "failure");
        assert_eq!(event.fields["error_code"], "install_failed:commit");
        assert_eq!(event.fields["rollback_result"], "rollback_failed");
        let serialized = serde_json::to_string(&event).expect("serialize audit event");
        assert!(!serialized.contains("nativePC"));
        assert!(!serialized.contains("rollback fixture unavailable"));
    }

    #[test]
    fn run_install_task_blocks_missing_prerequisites_before_write_coordination() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let blocked = blocked_prerequisite_decision();
        let planner = Arc::new(RecordingInstallPlanner::with_decisions(
            sample_plan(),
            blocked.clone(),
            blocked,
        ));
        let committer = Arc::new(RecordingInstallCommitter::new(sample_manifest()));
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let runner = InstallTaskRunner::new(
            Arc::clone(&task_manager),
            planner,
            committer.clone(),
            audit_log.clone(),
            Arc::new(FixedClock),
        );

        let error = runner
            .run_install_task(&task.task_id, sample_request())
            .expect_err("missing prerequisites must block install");

        assert_eq!(
            error
                .events
                .iter()
                .map(|event| (event.phase.as_str(), event.error.as_deref()))
                .collect::<Vec<_>>(),
            vec![
                ("install.plan.building", None),
                ("install.failed", Some("install_failed:prerequisite")),
            ]
        );
        assert!(committer.take_profiles().is_empty());
        let audit = audit_log.take_event().expect("failure audit");
        assert_eq!(audit.result, "failure");
        assert_eq!(audit.fields["error_code"], "install_failed:prerequisite");
    }

    #[test]
    fn run_install_task_rejects_prerequisite_drift_before_write_lock() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let write_locks = Arc::new(GameProfileWriteLockRegistry::default());
        let write_lock = write_locks.lock_for(&GameId::mhw(), &ProfileId::new("default"));
        let planner = Arc::new(
            RecordingInstallPlanner::with_decisions(
                sample_plan(),
                warning_prerequisite_decision(),
                blocked_prerequisite_decision(),
            )
            .assert_prerequisite_revalidation_outside_write_lock(write_lock),
        );
        let committer = Arc::new(RecordingInstallCommitter::new(sample_manifest()));
        let runner = InstallTaskRunner::with_write_locks(
            Arc::clone(&task_manager),
            planner,
            committer.clone(),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            write_locks,
        );

        let error = runner
            .run_install_task(&task.task_id, sample_request())
            .expect_err("prerequisite drift must block install");

        assert_eq!(
            error.events.last().and_then(|event| event.error.as_deref()),
            Some("install_failed:prerequisite")
        );
        assert!(committer.take_profiles().is_empty());
    }

    fn sample_request() -> StartInstallTaskRequest {
        StartInstallTaskRequest {
            game_id: GameId::mhw(),
            mod_id: ModId::new("mod-a"),
            profile_id: ProfileId::new("default"),
            layer: FileLayer::new("base", 0),
        }
    }

    fn sample_recovery_action_request() -> StartRecoveryActionTaskRequest {
        StartRecoveryActionTaskRequest {
            game_id: GameId::mhw(),
            mod_id: ModId::new("mod-a"),
            profile_id: ProfileId::new("default"),
            action_kind: crate::InstallRecoveryActionKind::RollbackInstall,
        }
    }

    fn sample_uninstall_request() -> StartUninstallTaskRequest {
        StartUninstallTaskRequest {
            game_id: GameId::mhw(),
            mod_id: ModId::new("mod-a"),
            profile_id: ProfileId::new("default"),
        }
    }

    fn sample_plan() -> InstallPlan {
        InstallPlan::from_providers(vec![sample_provider()])
    }

    fn sample_manifest() -> InstallManifest {
        InstallManifest::completed(
            ProfileId::new("default"),
            vec![InstallManifestEntry {
                target_path: sample_target(),
                mod_id: ModId::new("mod-a"),
                revision_id: None,
                package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: None,
            }],
        )
    }

    fn sample_provider() -> InstallFileProvider {
        InstallFileProvider::new(
            ModId::new("mod-a"),
            PackageFileId::new("nativePC/models/player.mod3"),
            sample_target(),
            FileLayer::new("base", 0),
        )
    }

    fn sample_target() -> InstallTargetPath {
        InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("sample target is valid")
    }

    #[derive(Clone, Copy)]
    enum AdmissionRepositoryMode {
        Empty,
        Pending,
        Unavailable,
    }

    struct RejectingCrossProcessAdmission(CrossProcessWriteAdmissionError);

    impl CrossProcessWriteAdmission for RejectingCrossProcessAdmission {
        fn acquire(
            &self,
            _scope: &CrossProcessWriteScope,
            _timeout: Duration,
            _cancellation: &dyn CancellationToken,
        ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>> {
            Err(self.0)
        }
    }

    struct AdmissionRecoveryRepository {
        mode: AdmissionRepositoryMode,
    }

    impl ReinstallRecoveryTransactionRepository for AdmissionRecoveryRepository {
        fn load_transaction(
            &self,
            _profile_id: &ProfileId,
            _mod_id: &ModId,
        ) -> anyhow::Result<Option<ReinstallRecoveryTransaction>> {
            Ok(None)
        }

        fn list_transactions(
            &self,
            profile_id: &ProfileId,
        ) -> anyhow::Result<Vec<ReinstallRecoveryTransaction>> {
            match self.mode {
                AdmissionRepositoryMode::Empty => Ok(Vec::new()),
                AdmissionRepositoryMode::Pending => Ok(vec![ReinstallRecoveryTransaction {
                    profile_id: profile_id.clone(),
                    mod_id: ModId::new("mod-a"),
                    old_revision_id: ModRevisionId::new("installed-v1"),
                    candidate_revision_id: ModRevisionId::new("candidate-v2"),
                    plan_token: "opaque-token".to_owned(),
                    plan_hash: "opaque-hash".to_owned(),
                    status: ReinstallRecoveryTransactionStatus::RepairRequired,
                    pre_reinstall_manifest: sample_manifest(),
                    candidate_replacement_bindings: Vec::new(),
                    targets: Vec::new(),
                }]),
                AdmissionRepositoryMode::Unavailable => {
                    anyhow::bail!("simulated recovery repository failure")
                }
            }
        }

        fn save_transaction(
            &self,
            _transaction: &ReinstallRecoveryTransaction,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        fn remove_transaction(
            &self,
            _profile_id: &ProfileId,
            _mod_id: &ModId,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn wait_for_status(
        task_manager: &crate::TaskManager,
        task_id: &str,
        expected: crate::TaskStatus,
    ) {
        for _ in 0..100 {
            if task_manager.task_status(task_id) == Some(expected) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }

        panic!("task {task_id} did not reach expected status {expected:?}");
    }

    struct RecordingInstallPlanner {
        plan: InstallPlan,
        preview_decision: GamePrerequisiteDecision,
        revalidation_decision: GamePrerequisiteDecision,
        requests: Mutex<Vec<BuildImportedModInstallPlanRequest>>,
        prerequisite_write_lock: Option<Arc<Mutex<()>>>,
    }

    impl RecordingInstallPlanner {
        fn new(plan: InstallPlan) -> Self {
            let decision = ready_prerequisite_decision();
            Self::with_decisions(plan, decision.clone(), decision)
        }

        fn with_decisions(
            plan: InstallPlan,
            preview_decision: GamePrerequisiteDecision,
            revalidation_decision: GamePrerequisiteDecision,
        ) -> Self {
            Self {
                plan,
                preview_decision,
                revalidation_decision,
                requests: Mutex::new(Vec::new()),
                prerequisite_write_lock: None,
            }
        }

        fn assert_prerequisite_revalidation_outside_write_lock(
            mut self,
            write_lock: Arc<Mutex<()>>,
        ) -> Self {
            self.prerequisite_write_lock = Some(write_lock);
            self
        }

        fn take_requests(&self) -> Vec<BuildImportedModInstallPlanRequest> {
            std::mem::take(&mut *self.requests.lock().expect("requests"))
        }
    }

    impl ImportedModInstallPlanner for RecordingInstallPlanner {
        fn build_imported_mod_install_plan(
            &self,
            request: BuildImportedModInstallPlanRequest,
        ) -> Result<ImportedModInstallPreflight, InstallPlanningError> {
            self.requests.lock().expect("requests").push(request);
            Ok(ImportedModInstallPreflight {
                plan: self.plan.clone(),
                prerequisite_decision: self.preview_decision.clone(),
            })
        }

        fn build_imported_mod_revision_install_plan(
            &self,
            _game_id: &GameId,
            _mod_id: &ModId,
            _revision_id: &ModRevisionId,
            _layer: &FileLayer,
        ) -> Result<ImportedModInstallPreflight, InstallPlanningError> {
            Ok(ImportedModInstallPreflight {
                plan: self.plan.clone(),
                prerequisite_decision: self.preview_decision.clone(),
            })
        }

        fn prerequisite_decision(&self, _game_id: &GameId) -> GamePrerequisiteDecision {
            if let Some(write_lock) = &self.prerequisite_write_lock {
                let _guard = write_lock
                    .try_lock()
                    .expect("prerequisite revalidation must run outside the write lock");
            }
            self.revalidation_decision.clone()
        }
    }

    struct CancellingInstallPlanner {
        task_manager: Arc<crate::TaskManager>,
        task_id: String,
        plan: InstallPlan,
    }

    impl ImportedModInstallPlanner for CancellingInstallPlanner {
        fn build_imported_mod_install_plan(
            &self,
            _request: BuildImportedModInstallPlanRequest,
        ) -> Result<ImportedModInstallPreflight, InstallPlanningError> {
            self.task_manager
                .cancel_task(&self.task_id)
                .expect("task can be cancelled after planning");
            Ok(ImportedModInstallPreflight {
                plan: self.plan.clone(),
                prerequisite_decision: ready_prerequisite_decision(),
            })
        }

        fn build_imported_mod_revision_install_plan(
            &self,
            _game_id: &GameId,
            _mod_id: &ModId,
            _revision_id: &ModRevisionId,
            _layer: &FileLayer,
        ) -> Result<ImportedModInstallPreflight, InstallPlanningError> {
            self.task_manager
                .cancel_task(&self.task_id)
                .expect("task can be cancelled after revision planning");
            Ok(ImportedModInstallPreflight {
                plan: self.plan.clone(),
                prerequisite_decision: ready_prerequisite_decision(),
            })
        }

        fn prerequisite_decision(&self, _game_id: &GameId) -> GamePrerequisiteDecision {
            ready_prerequisite_decision()
        }
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

    struct RecordingInstallCommitter {
        manifest: InstallManifest,
        profiles: Mutex<Vec<String>>,
    }

    impl RecordingInstallCommitter {
        fn new(manifest: InstallManifest) -> Self {
            Self {
                manifest,
                profiles: Mutex::new(Vec::new()),
            }
        }

        fn take_profiles(&self) -> Vec<String> {
            std::mem::take(&mut *self.profiles.lock().expect("profiles"))
        }
    }

    impl InstallPlanCommitter for RecordingInstallCommitter {
        fn commit_install_plan(
            &self,
            request: ImportedModInstallCommitRequest,
        ) -> Result<InstallCommitResult, InstallCommitError> {
            self.profiles
                .lock()
                .expect("profiles")
                .push(request.profile_id.as_str().to_owned());
            Ok(InstallCommitResult {
                manifest: self.manifest.clone(),
            })
        }
    }

    struct BlockingInstallCommitter {
        manifest: InstallManifest,
        started: Mutex<Option<mpsc::Sender<()>>>,
        release: Mutex<mpsc::Receiver<()>>,
    }

    impl InstallPlanCommitter for BlockingInstallCommitter {
        fn commit_install_plan(
            &self,
            _request: ImportedModInstallCommitRequest,
        ) -> Result<InstallCommitResult, InstallCommitError> {
            if let Some(started) = self.started.lock().expect("started").take() {
                started.send(()).expect("commit start can be signalled");
            }
            self.release
                .lock()
                .expect("release")
                .recv_timeout(Duration::from_secs(2))
                .expect("commit release should arrive");

            Ok(InstallCommitResult {
                manifest: self.manifest.clone(),
            })
        }
    }

    struct RecordingUninstaller {
        result: UninstallModResult,
        requests: Mutex<Vec<StartUninstallTaskRequest>>,
    }

    impl RecordingUninstaller {
        fn new(result: UninstallModResult) -> Self {
            Self {
                result,
                requests: Mutex::new(Vec::new()),
            }
        }

        fn take_requests(&self) -> Vec<StartUninstallTaskRequest> {
            std::mem::take(&mut *self.requests.lock().expect("requests"))
        }
    }

    impl ModUninstaller for RecordingUninstaller {
        fn uninstall_mod(
            &self,
            request: StartUninstallTaskRequest,
        ) -> Result<UninstallModResult, UninstallModError> {
            self.requests.lock().expect("requests").push(request);
            Ok(self.result.clone())
        }

        fn uninstall_mod_for_revision(
            &self,
            request: StartUninstallTaskRequest,
            _expected_installed_revision_id: ModRevisionId,
        ) -> Result<UninstallModResult, UninstallModError> {
            self.uninstall_mod(request)
        }

        fn uninstall_mod_for_revision_and_manifest(
            &self,
            request: StartUninstallTaskRequest,
            expected_installed_revision_id: ModRevisionId,
            _expected_manifest_digest: &str,
        ) -> Result<UninstallModResult, UninstallModError> {
            self.uninstall_mod_for_revision(request, expected_installed_revision_id)
        }
    }

    struct NotifyingUninstaller {
        result: UninstallModResult,
        started: Mutex<Option<mpsc::Sender<()>>>,
    }

    impl ModUninstaller for NotifyingUninstaller {
        fn uninstall_mod(
            &self,
            _request: StartUninstallTaskRequest,
        ) -> Result<UninstallModResult, UninstallModError> {
            if let Some(started) = self.started.lock().expect("started").take() {
                started.send(()).expect("uninstall start can be signalled");
            }
            Ok(self.result.clone())
        }

        fn uninstall_mod_for_revision(
            &self,
            request: StartUninstallTaskRequest,
            _expected_installed_revision_id: ModRevisionId,
        ) -> Result<UninstallModResult, UninstallModError> {
            self.uninstall_mod(request)
        }

        fn uninstall_mod_for_revision_and_manifest(
            &self,
            request: StartUninstallTaskRequest,
            expected_installed_revision_id: ModRevisionId,
            _expected_manifest_digest: &str,
        ) -> Result<UninstallModResult, UninstallModError> {
            self.uninstall_mod_for_revision(request, expected_installed_revision_id)
        }
    }

    struct RecordingRecoveryActionExecutor {
        result: crate::InstallRecoveryActionResult,
        requests: Mutex<Vec<StartRecoveryActionTaskRequest>>,
    }

    impl RecordingRecoveryActionExecutor {
        fn new(result: crate::InstallRecoveryActionResult) -> Self {
            Self {
                result,
                requests: Mutex::new(Vec::new()),
            }
        }

        fn take_requests(&self) -> Vec<StartRecoveryActionTaskRequest> {
            std::mem::take(&mut *self.requests.lock().expect("requests"))
        }
    }

    impl InstallRecoveryActionExecutor for RecordingRecoveryActionExecutor {
        fn run_recovery_action(
            &self,
            request: StartRecoveryActionTaskRequest,
        ) -> Result<crate::InstallRecoveryActionResult, crate::InstallRecoveryActionError> {
            self.requests.lock().expect("requests").push(request);
            Ok(self.result.clone())
        }
    }

    struct FailingRecoveryActionExecutor {
        error: crate::InstallRecoveryActionError,
    }

    impl InstallRecoveryActionExecutor for FailingRecoveryActionExecutor {
        fn run_recovery_action(
            &self,
            _request: StartRecoveryActionTaskRequest,
        ) -> Result<crate::InstallRecoveryActionResult, crate::InstallRecoveryActionError> {
            Err(self.error.clone())
        }
    }

    struct NotifyingRecoveryActionExecutor {
        result: crate::InstallRecoveryActionResult,
        started: Mutex<Option<mpsc::Sender<()>>>,
    }

    impl InstallRecoveryActionExecutor for NotifyingRecoveryActionExecutor {
        fn run_recovery_action(
            &self,
            _request: StartRecoveryActionTaskRequest,
        ) -> Result<crate::InstallRecoveryActionResult, crate::InstallRecoveryActionError> {
            if let Some(started) = self.started.lock().expect("started").take() {
                started
                    .send(())
                    .expect("recovery action start can be signalled");
            }
            Ok(self.result.clone())
        }
    }

    struct FailingInstallCommitter;

    impl InstallPlanCommitter for FailingInstallCommitter {
        fn commit_install_plan(
            &self,
            _request: ImportedModInstallCommitRequest,
        ) -> Result<InstallCommitResult, InstallCommitError> {
            Err(InstallCommitError::Failed {
                phase: crate::InstallCommitPhase::Write,
            })
        }
    }

    struct RollbackFailedInstallCommitter;

    impl InstallPlanCommitter for RollbackFailedInstallCommitter {
        fn commit_install_plan(
            &self,
            _request: ImportedModInstallCommitRequest,
        ) -> Result<InstallCommitResult, InstallCommitError> {
            Err(InstallCommitError::RollbackFailed {
                failed_phase: crate::InstallCommitPhase::Write,
            })
        }
    }

    struct CommitCancellationProbe {
        task_manager: Arc<crate::TaskManager>,
        task_id: String,
        error: Mutex<Option<TaskManagerError>>,
    }

    impl CommitCancellationProbe {
        fn new(task_manager: Arc<crate::TaskManager>, task_id: String) -> Self {
            Self {
                task_manager,
                task_id,
                error: Mutex::new(None),
            }
        }

        fn attempt(&self) {
            let error = self
                .task_manager
                .cancel_task(&self.task_id)
                .expect_err("commit barrier must reject cancellation");
            *self.error.lock().expect("cancellation probe") = Some(error);
        }

        fn assert_rejected(&self) {
            assert!(matches!(
                self.error.lock().expect("cancellation probe").as_ref(),
                Some(TaskManagerError::TaskCannotBeCancelled {
                    status: TaskStatus::Running,
                    ..
                })
            ));
        }
    }

    struct CancellationProbingInstallCommitter {
        cancellation: Arc<CommitCancellationProbe>,
        manifest: InstallManifest,
    }

    impl InstallPlanCommitter for CancellationProbingInstallCommitter {
        fn commit_install_plan(
            &self,
            _request: ImportedModInstallCommitRequest,
        ) -> Result<InstallCommitResult, InstallCommitError> {
            self.cancellation.attempt();
            Ok(InstallCommitResult {
                manifest: self.manifest.clone(),
            })
        }
    }

    struct CancellationProbingUninstaller {
        cancellation: Arc<CommitCancellationProbe>,
        result: UninstallModResult,
    }

    impl ModUninstaller for CancellationProbingUninstaller {
        fn uninstall_mod(
            &self,
            _request: StartUninstallTaskRequest,
        ) -> Result<UninstallModResult, UninstallModError> {
            self.cancellation.attempt();
            Ok(self.result.clone())
        }

        fn uninstall_mod_for_revision(
            &self,
            request: StartUninstallTaskRequest,
            _expected_installed_revision_id: ModRevisionId,
        ) -> Result<UninstallModResult, UninstallModError> {
            self.uninstall_mod(request)
        }

        fn uninstall_mod_for_revision_and_manifest(
            &self,
            request: StartUninstallTaskRequest,
            expected_installed_revision_id: ModRevisionId,
            _expected_manifest_digest: &str,
        ) -> Result<UninstallModResult, UninstallModError> {
            self.uninstall_mod_for_revision(request, expected_installed_revision_id)
        }
    }

    struct CancellationProbingRecoveryActionExecutor {
        cancellation: Arc<CommitCancellationProbe>,
        result: InstallRecoveryActionResult,
    }

    impl InstallRecoveryActionExecutor for CancellationProbingRecoveryActionExecutor {
        fn run_recovery_action(
            &self,
            _request: StartRecoveryActionTaskRequest,
        ) -> Result<InstallRecoveryActionResult, InstallRecoveryActionError> {
            self.cancellation.attempt();
            Ok(self.result.clone())
        }
    }

    #[derive(Default)]
    struct RecordingAuditLogWriter {
        event: Mutex<Option<AuditLogEvent>>,
    }

    impl RecordingAuditLogWriter {
        fn take_event(&self) -> Option<AuditLogEvent> {
            self.event.lock().expect("event").take()
        }
    }

    impl AuditLogWriter for RecordingAuditLogWriter {
        fn record(&self, event: AuditLogEvent) -> anyhow::Result<()> {
            *self.event.lock().expect("event") = Some(event);
            Ok(())
        }
    }

    struct FixedClock;

    impl AppClock for FixedClock {
        fn now_unix_millis(&self) -> anyhow::Result<u128> {
            Ok(42)
        }
    }
}
