use std::collections::BTreeMap;
use std::sync::Arc;

use hmm_core::{FileLayer, GameId, ModId, ModRevisionId, ProfileId, ReplacementTargetId};
use hmm_ports::{AppClock, AuditLogEvent, AuditLogWriter, AuditWriteFailurePolicy};

use crate::reinstall::{PreparedReinstall, ReinstallPreparation};
use crate::replacement_audit::{
    append_adapter_audit_fields, unique_adapter_audit_facts, ReplacementAdapterAuditFacts,
};
use crate::task_manager::{noop_task_progress_observer, observe_task_progress};
use crate::{
    GameProfileWriteLockRegistry, InstallWriteAdmission, ReinstallCommitError,
    ReinstallCommitPhase, ReinstallCommitResult, ReinstallCommitService, ReinstallPreviewError,
    ReinstallPreviewRequest, ReinstallPreviewService, ReinstallTargetCounts,
    RetargetReinstallRequest, TaskKind, TaskManager, TaskManagerError, TaskProgressEvent,
    TaskProgressObserver, TaskStarted, TaskStatus,
};

const PLAN_BUILDING_PHASE: &str = "install.reinstall.plan.building";
const PREFLIGHT_PROCESSING_PHASE: &str = "install.reinstall.preflight.processing";
const COMMIT_PROCESSING_PHASE: &str = "install.reinstall.commit.processing";
const ROLLBACK_PROCESSING_PHASE: &str = "install.reinstall.rollback.processing";
const COMPLETED_PHASE: &str = "install.reinstall.completed";
const FAILED_PHASE: &str = "install.reinstall.failed";
const FAILED_ERROR_PREFIX: &str = "install_reinstall_failed";
const PREFLIGHT_FAILURE_PHASE: &str = "preflight";
pub(crate) const REINSTALL_PREFLIGHT_FAILURE_CODE: &str = "install_reinstall_failed:preflight";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartReinstallTaskRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub candidate_revision_id: ModRevisionId,
    pub layer: FileLayer,
    pub plan_token: String,
}

impl StartReinstallTaskRequest {
    fn preview_request(&self) -> ReinstallPreviewRequest {
        ReinstallPreviewRequest {
            game_id: self.game_id.clone(),
            profile_id: self.profile_id.clone(),
            mod_id: self.mod_id.clone(),
            candidate_revision_id: self.candidate_revision_id.clone(),
            layer: self.layer.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRetargetReinstallTaskRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub target_id: ReplacementTargetId,
    pub layer: FileLayer,
    pub plan_token: String,
}

impl StartRetargetReinstallTaskRequest {
    fn preview_request(&self) -> RetargetReinstallRequest {
        RetargetReinstallRequest {
            game_id: self.game_id.clone(),
            profile_id: self.profile_id.clone(),
            mod_id: self.mod_id.clone(),
            target_id: self.target_id.clone(),
            layer: self.layer.clone(),
        }
    }
}

trait ReinstallTaskRequestContext {
    fn game_id(&self) -> &GameId;
    fn profile_id(&self) -> &ProfileId;
    fn mod_id(&self) -> &ModId;
    fn target_id(&self) -> Option<&ReplacementTargetId>;
}

impl ReinstallTaskRequestContext for StartReinstallTaskRequest {
    fn game_id(&self) -> &GameId {
        &self.game_id
    }

    fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    fn mod_id(&self) -> &ModId {
        &self.mod_id
    }

    fn target_id(&self) -> Option<&ReplacementTargetId> {
        None
    }
}

impl ReinstallTaskRequestContext for StartRetargetReinstallTaskRequest {
    fn game_id(&self) -> &GameId {
        &self.game_id
    }

    fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    fn mod_id(&self) -> &ModId {
        &self.mod_id
    }

    fn target_id(&self) -> Option<&ReplacementTargetId> {
        Some(&self.target_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinstallTaskAuditContext {
    pub previous_revision_id: Option<ModRevisionId>,
    pub candidate_revision_id: ModRevisionId,
    pub counts: ReinstallTargetCounts,
    pub adapter_facts: Option<Box<ReplacementAdapterAuditFacts>>,
}

pub trait ReinstallTaskPrepared: Send {
    fn audit_context(&self) -> ReinstallTaskAuditContext;
    fn plan_token(&self) -> &str;
    fn batch_plan_digest(&self) -> String;
}

impl ReinstallTaskPrepared for PreparedReinstall {
    fn audit_context(&self) -> ReinstallTaskAuditContext {
        ReinstallTaskAuditContext {
            previous_revision_id: Some(self.installed_revision_id.clone()),
            candidate_revision_id: self.candidate.revision_id.clone(),
            counts: self.counts,
            adapter_facts: unique_adapter_audit_facts(&self.candidate_replacement_bindings),
        }
    }

    fn plan_token(&self) -> &str {
        PreparedReinstall::plan_token(self)
    }

    fn batch_plan_digest(&self) -> String {
        PreparedReinstall::batch_plan_digest(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReinstallTaskPrepareError {
    Planning(ReinstallTaskAuditContext),
    Preflight(ReinstallTaskAuditContext),
}

impl ReinstallTaskPrepareError {
    fn phase(&self) -> &'static str {
        match self {
            Self::Planning(_) => "planning",
            Self::Preflight(_) => "preflight",
        }
    }

    fn into_context(self) -> ReinstallTaskAuditContext {
        match self {
            Self::Planning(context) | Self::Preflight(context) => context,
        }
    }
}

pub trait ReinstallTaskExecutor: Send + Sync {
    type Prepared: ReinstallTaskPrepared;

    fn prepare(
        &self,
        request: ReinstallPreviewRequest,
    ) -> Result<Self::Prepared, ReinstallTaskPrepareError>;

    fn revalidate(&self, prepared: &Self::Prepared) -> Result<(), ReinstallCommitError>;

    fn commit(
        &self,
        prepared: Self::Prepared,
        expected_plan_token: &str,
    ) -> Result<ReinstallCommitResult, ReinstallCommitError>;
}

pub trait RetargetReinstallTaskExecutor: ReinstallTaskExecutor {
    fn prepare_retarget_reinstall(
        &self,
        request: RetargetReinstallRequest,
    ) -> Result<Self::Prepared, ReinstallTaskPrepareError>;
}

#[derive(Clone)]
pub struct ReinstallTaskExecutorService {
    preview: Arc<ReinstallPreviewService>,
    commit: Arc<ReinstallCommitService>,
}

impl ReinstallTaskExecutorService {
    pub fn new(preview: Arc<ReinstallPreviewService>, commit: Arc<ReinstallCommitService>) -> Self {
        Self { preview, commit }
    }
}

impl ReinstallTaskExecutor for ReinstallTaskExecutorService {
    type Prepared = PreparedReinstall;

    fn prepare(
        &self,
        request: ReinstallPreviewRequest,
    ) -> Result<Self::Prepared, ReinstallTaskPrepareError> {
        let fallback = ReinstallTaskAuditContext {
            previous_revision_id: None,
            candidate_revision_id: request.candidate_revision_id.clone(),
            counts: ReinstallTargetCounts::default(),
            adapter_facts: None,
        };
        match self.preview.prepare(request.clone()) {
            Ok(ReinstallPreparation::Ready(prepared)) => Ok(*prepared),
            Ok(ReinstallPreparation::Blocked(preview)) => Err(
                ReinstallTaskPrepareError::Preflight(ReinstallTaskAuditContext {
                    previous_revision_id: preview
                        .installed_revision
                        .map(|revision| revision.revision_id),
                    candidate_revision_id: preview
                        .candidate_revision
                        .map(|revision| revision.revision_id)
                        .unwrap_or(request.candidate_revision_id),
                    counts: preview.counts,
                    adapter_facts: None,
                }),
            ),
            Err(ReinstallPreviewError::CatalogUnavailable)
            | Err(ReinstallPreviewError::CandidatePlanUnavailable) => {
                Err(ReinstallTaskPrepareError::Planning(fallback))
            }
            Err(ReinstallPreviewError::ManifestUnavailable)
            | Err(ReinstallPreviewError::RecoveryUnavailable) => {
                Err(ReinstallTaskPrepareError::Preflight(fallback))
            }
        }
    }

    fn revalidate(&self, prepared: &Self::Prepared) -> Result<(), ReinstallCommitError> {
        let current_prerequisite_decision = self
            .preview
            .prerequisite_decision(&prepared.request.game_id);
        if current_prerequisite_decision.is_blocked()
            || current_prerequisite_decision != prepared.prerequisite_decision
        {
            return Err(ReinstallCommitError::PreviewStale);
        }
        Ok(())
    }

    fn commit(
        &self,
        prepared: Self::Prepared,
        expected_plan_token: &str,
    ) -> Result<ReinstallCommitResult, ReinstallCommitError> {
        self.commit.commit(prepared, expected_plan_token)
    }
}

pub struct ReinstallTaskService {
    task_manager: Arc<TaskManager>,
}

impl ReinstallTaskService {
    pub fn new(task_manager: Arc<TaskManager>) -> Self {
        Self { task_manager }
    }

    pub fn start_reinstall_task(
        &self,
        _request: StartReinstallTaskRequest,
    ) -> Result<TaskStarted, TaskManagerError> {
        let task = self.task_manager.create_task(TaskKind::Install)?;
        Ok(TaskStarted {
            task_id: task.task_id,
            kind: task.kind,
            status: task.status,
        })
    }

    pub fn start_retarget_reinstall_task(
        &self,
        _request: StartRetargetReinstallTaskRequest,
    ) -> Result<TaskStarted, TaskManagerError> {
        let task = self.task_manager.create_task(TaskKind::Install)?;
        Ok(TaskStarted {
            task_id: task.task_id,
            kind: task.kind,
            status: task.status,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinstallTaskRunError {
    pub events: Vec<TaskProgressEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReinstallTaskOrchestrationError {
    pub events: Vec<TaskProgressEvent>,
    pub commit_error: Option<ReinstallCommitError>,
    pub committed: bool,
}

pub struct ReinstallTaskRunner<E: ReinstallTaskExecutor> {
    task_manager: Arc<TaskManager>,
    executor: Arc<E>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
    write_locks: Arc<GameProfileWriteLockRegistry>,
    write_admission: Arc<dyn InstallWriteAdmission>,
}

impl<E: ReinstallTaskExecutor> ReinstallTaskRunner<E> {
    pub fn new(
        task_manager: Arc<TaskManager>,
        executor: Arc<E>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self::with_write_locks(
            task_manager,
            executor,
            audit_log,
            clock,
            Arc::new(GameProfileWriteLockRegistry::default()),
        )
    }

    pub fn with_write_locks(
        task_manager: Arc<TaskManager>,
        executor: Arc<E>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
    ) -> Self {
        Self::with_write_coordination(
            task_manager,
            executor,
            audit_log,
            clock,
            write_locks,
            Arc::new(crate::install_task::AllowInstallWriteAdmission),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_write_coordination(
        task_manager: Arc<TaskManager>,
        executor: Arc<E>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
        write_admission: Arc<dyn InstallWriteAdmission>,
    ) -> Self {
        Self {
            task_manager,
            executor,
            audit_log,
            clock,
            write_locks,
            write_admission,
        }
    }

    pub fn run_reinstall_task(
        &self,
        task_id: &str,
        request: StartReinstallTaskRequest,
    ) -> Result<Vec<TaskProgressEvent>, ReinstallTaskRunError> {
        let observer = noop_task_progress_observer();
        self.run_reinstall_task_with_observer(task_id, request, &observer)
    }

    pub fn run_reinstall_task_with_observer<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: StartReinstallTaskRequest,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, ReinstallTaskRunError> {
        let preview_request = request.preview_request();
        let expected_plan_token = request.plan_token.clone();
        self.run_task(
            task_id,
            &request,
            observer,
            || self.executor.prepare(preview_request),
            |_| Ok(expected_plan_token),
        )
        .map_err(public_run_error)
    }

    pub(crate) fn run_reinstall_task_for_orchestration_with_observer<
        O: TaskProgressObserver + ?Sized,
    >(
        &self,
        task_id: &str,
        request: StartReinstallTaskRequest,
        expected_batch_plan_digest: &str,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, ReinstallTaskOrchestrationError> {
        let preview_request = request.preview_request();
        let expected_batch_plan_digest = expected_batch_plan_digest.to_owned();
        self.run_task(
            task_id,
            &request,
            observer,
            || self.executor.prepare(preview_request),
            |prepared| orchestration_plan_token(prepared, &expected_batch_plan_digest),
        )
    }

    fn run_task<R, F, P, O>(
        &self,
        task_id: &str,
        request: &R,
        observer: &O,
        prepare: F,
        expected_plan_token: P,
    ) -> Result<Vec<TaskProgressEvent>, ReinstallTaskOrchestrationError>
    where
        R: ReinstallTaskRequestContext,
        F: FnOnce() -> Result<E::Prepared, ReinstallTaskPrepareError>,
        P: FnOnce(&E::Prepared) -> Result<String, ReinstallCommitError>,
        O: TaskProgressObserver + ?Sized,
    {
        if self.task_manager.start_task(task_id).is_err() {
            return if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                Ok(Vec::new())
            } else {
                Err(ReinstallTaskOrchestrationError {
                    events: Vec::new(),
                    commit_error: None,
                    committed: false,
                })
            };
        }

        let mut events = Vec::new();
        observe_task_progress(
            &mut events,
            observer,
            running_event(task_id, PLAN_BUILDING_PHASE),
        );
        let prepared = match prepare() {
            Ok(prepared) => prepared,
            Err(error) => {
                if self.is_cancelled(task_id) {
                    return Ok(events);
                }
                let phase = error.phase();
                let context = error.into_context();
                return self.fail_with_audit(
                    task_id,
                    request,
                    context,
                    events,
                    observer,
                    phase,
                    "not_attempted",
                    false,
                );
            }
        };
        let audit_context = prepared.audit_context();

        if self.is_cancelled(task_id) {
            return Ok(events);
        }
        let expected_plan_token = match expected_plan_token(&prepared) {
            Ok(token) => token,
            Err(error) => {
                return self.fail_with_commit_error(
                    task_id,
                    request,
                    audit_context,
                    events,
                    observer,
                    error,
                );
            }
        };
        observe_task_progress(
            &mut events,
            observer,
            running_event(task_id, PREFLIGHT_PROCESSING_PHASE),
        );
        if let Err(error) = self.executor.revalidate(&prepared) {
            if self.is_cancelled(task_id) {
                return Ok(events);
            }
            return self.fail_with_commit_error(
                task_id,
                request,
                audit_context,
                events,
                observer,
                error,
            );
        }

        let commit_result = {
            let _cross_process_guard = match self.write_locks.acquire_cross_process_for_task(
                request.game_id(),
                request.profile_id(),
                &self.task_manager,
                task_id,
            ) {
                Ok(guard) => guard,
                Err(error) => {
                    if self.is_cancelled(task_id) {
                        return Ok(events);
                    }
                    return self.fail_with_audit(
                        task_id,
                        request,
                        audit_context,
                        events,
                        observer,
                        error.code(),
                        "not_attempted",
                        false,
                    );
                }
            };
            let write_lock = self
                .write_locks
                .lock_for(request.game_id(), request.profile_id());
            let _guard = match write_lock.lock() {
                Ok(guard) => guard,
                Err(_) => {
                    return self.fail_with_audit(
                        task_id,
                        request,
                        audit_context,
                        events,
                        observer,
                        "lock",
                        "not_attempted",
                        false,
                    );
                }
            };
            if self.is_cancelled(task_id) {
                return Ok(events);
            }
            if let Err(error) = self
                .write_admission
                .ensure_write_allowed(request.game_id(), request.profile_id())
            {
                return self.fail_with_audit(
                    task_id,
                    request,
                    audit_context,
                    events,
                    observer,
                    error.failure_phase(),
                    "not_attempted",
                    false,
                );
            }
            observe_task_progress(
                &mut events,
                observer,
                running_event(task_id, COMMIT_PROCESSING_PHASE),
            );
            if self.task_manager.block_task_cancellation(task_id).is_err() {
                if self.is_cancelled(task_id) {
                    return Ok(events);
                }
                None
            } else {
                Some(self.executor.commit(prepared, &expected_plan_token))
            }
        };

        let Some(commit_result) = commit_result else {
            return self.fail_with_audit(
                task_id,
                request,
                audit_context,
                events,
                observer,
                "lock",
                "not_attempted",
                false,
            );
        };
        if let Err(error) = commit_result {
            return self.fail_with_commit_error(
                task_id,
                request,
                audit_context,
                events,
                observer,
                error,
            );
        }

        match self.task_manager.complete_task(task_id) {
            Ok(task) => {
                let audit_ok =
                    self.record_audit(task_id, request, &audit_context, "success", None, None);
                let mut event =
                    TaskProgressEvent::new(task.task_id, task.kind, task.status, COMPLETED_PHASE);
                if !audit_ok {
                    event.error = Some("install_audit_unavailable".to_owned());
                }
                observe_task_progress(&mut events, observer, event);
                Ok(events)
            }
            Err(_) => self.fail_after_commit(
                task_id,
                request,
                audit_context,
                events,
                observer,
                "complete",
                "not_attempted_post_commit",
            ),
        }
    }

    fn is_cancelled(&self, task_id: &str) -> bool {
        self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled)
    }

    #[allow(clippy::too_many_arguments)]
    fn fail_with_audit<R: ReinstallTaskRequestContext, O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &R,
        context: ReinstallTaskAuditContext,
        events: Vec<TaskProgressEvent>,
        observer: &O,
        phase: &str,
        rollback_result: &str,
        emit_rollback: bool,
    ) -> Result<Vec<TaskProgressEvent>, ReinstallTaskOrchestrationError> {
        self.fail_with_audit_details(
            task_id,
            request,
            context,
            events,
            observer,
            phase,
            rollback_result,
            emit_rollback,
            None,
            false,
        )
    }

    fn fail_with_commit_error<R: ReinstallTaskRequestContext, O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &R,
        context: ReinstallTaskAuditContext,
        events: Vec<TaskProgressEvent>,
        observer: &O,
        error: ReinstallCommitError,
    ) -> Result<Vec<TaskProgressEvent>, ReinstallTaskOrchestrationError> {
        let failure = commit_failure(&error);
        let committed = matches!(
            error,
            ReinstallCommitError::PostCommit | ReinstallCommitError::CleanupPending
        );
        self.fail_with_audit_details(
            task_id,
            request,
            context,
            events,
            observer,
            failure.phase,
            failure.rollback_result,
            failure.emit_rollback,
            Some(error),
            committed,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn fail_after_commit<R: ReinstallTaskRequestContext, O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &R,
        context: ReinstallTaskAuditContext,
        events: Vec<TaskProgressEvent>,
        observer: &O,
        phase: &str,
        rollback_result: &str,
    ) -> Result<Vec<TaskProgressEvent>, ReinstallTaskOrchestrationError> {
        self.fail_with_audit_details(
            task_id,
            request,
            context,
            events,
            observer,
            phase,
            rollback_result,
            false,
            None,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn fail_with_audit_details<R: ReinstallTaskRequestContext, O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &R,
        context: ReinstallTaskAuditContext,
        mut events: Vec<TaskProgressEvent>,
        observer: &O,
        phase: &str,
        rollback_result: &str,
        emit_rollback: bool,
        commit_error: Option<ReinstallCommitError>,
        committed: bool,
    ) -> Result<Vec<TaskProgressEvent>, ReinstallTaskOrchestrationError> {
        match self.task_manager.fail_task(task_id) {
            Ok(_) => {}
            Err(TaskManagerError::TaskCannotTransition {
                from: TaskStatus::Cancelled,
                to: TaskStatus::Failed,
                ..
            }) => {
                return Ok(events);
            }
            Err(_) => {}
        }
        if emit_rollback {
            observe_task_progress(
                &mut events,
                observer,
                running_event(task_id, ROLLBACK_PROCESSING_PHASE),
            );
        }
        let error_code = if phase == PREFLIGHT_FAILURE_PHASE {
            REINSTALL_PREFLIGHT_FAILURE_CODE.to_owned()
        } else {
            format!("{FAILED_ERROR_PREFIX}:{phase}")
        };
        observe_task_progress(&mut events, observer, failed_event(task_id, &error_code));
        let _ = self.record_audit(
            task_id,
            request,
            &context,
            "failure",
            Some(&error_code),
            Some(rollback_result),
        );
        Err(ReinstallTaskOrchestrationError {
            events,
            commit_error,
            committed,
        })
    }

    fn record_audit<R: ReinstallTaskRequestContext>(
        &self,
        task_id: &str,
        request: &R,
        context: &ReinstallTaskAuditContext,
        result: &str,
        error_code: Option<&str>,
        rollback_result: Option<&str>,
    ) -> bool {
        let mut fields = BTreeMap::new();
        fields.insert("task_id".to_owned(), task_id.to_owned());
        fields.insert("game_id".to_owned(), request.game_id().as_str().to_owned());
        fields.insert(
            "profile_id".to_owned(),
            request.profile_id().as_str().to_owned(),
        );
        fields.insert("mod_id".to_owned(), request.mod_id().as_str().to_owned());
        if let Some(target_id) = request.target_id() {
            fields.insert("target_id".to_owned(), target_id.as_str().to_owned());
        }
        if let Some(previous_revision_id) = &context.previous_revision_id {
            fields.insert(
                "previous_revision_id".to_owned(),
                previous_revision_id.as_str().to_owned(),
            );
        }
        fields.insert(
            "candidate_revision_id".to_owned(),
            context.candidate_revision_id.as_str().to_owned(),
        );
        fields.insert(
            "retained_count".to_owned(),
            context.counts.retained.to_string(),
        );
        fields.insert(
            "replaced_count".to_owned(),
            context.counts.replaced.to_string(),
        );
        fields.insert("added_count".to_owned(), context.counts.added.to_string());
        fields.insert("stale_count".to_owned(), context.counts.stale.to_string());
        append_adapter_audit_fields(&mut fields, context.adapter_facts.as_deref());
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
                    timestamp_unix_millis: self.clock.now_unix_millis().unwrap_or_default(),
                    category: "install".to_owned(),
                    operation: "reinstall_mod".to_owned(),
                    result: result.to_owned(),
                    fields,
                },
                policy,
            )
            .is_ok()
    }
}

impl<E: RetargetReinstallTaskExecutor> ReinstallTaskRunner<E> {
    pub fn run_retarget_reinstall_task(
        &self,
        task_id: &str,
        request: StartRetargetReinstallTaskRequest,
    ) -> Result<Vec<TaskProgressEvent>, ReinstallTaskRunError> {
        let observer = noop_task_progress_observer();
        self.run_retarget_reinstall_task_with_observer(task_id, request, &observer)
    }

    pub fn run_retarget_reinstall_task_with_observer<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: StartRetargetReinstallTaskRequest,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, ReinstallTaskRunError> {
        let preview_request = request.preview_request();
        let expected_plan_token = request.plan_token.clone();
        self.run_task(
            task_id,
            &request,
            observer,
            || self.executor.prepare_retarget_reinstall(preview_request),
            |_| Ok(expected_plan_token),
        )
        .map_err(public_run_error)
    }

    pub(crate) fn run_retarget_reinstall_task_for_orchestration_with_observer<
        O: TaskProgressObserver + ?Sized,
    >(
        &self,
        task_id: &str,
        request: StartRetargetReinstallTaskRequest,
        expected_batch_plan_digest: &str,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, ReinstallTaskOrchestrationError> {
        let preview_request = request.preview_request();
        let expected_batch_plan_digest = expected_batch_plan_digest.to_owned();
        self.run_task(
            task_id,
            &request,
            observer,
            || self.executor.prepare_retarget_reinstall(preview_request),
            |prepared| orchestration_plan_token(prepared, &expected_batch_plan_digest),
        )
    }
}

fn public_run_error(error: ReinstallTaskOrchestrationError) -> ReinstallTaskRunError {
    ReinstallTaskRunError {
        events: error.events,
    }
}

fn orchestration_plan_token<P: ReinstallTaskPrepared>(
    prepared: &P,
    expected_batch_plan_digest: &str,
) -> Result<String, ReinstallCommitError> {
    if prepared.batch_plan_digest() != expected_batch_plan_digest {
        return Err(ReinstallCommitError::PreviewStale);
    }
    Ok(prepared.plan_token().to_owned())
}

struct CommitFailure {
    phase: &'static str,
    rollback_result: &'static str,
    emit_rollback: bool,
}

fn commit_failure(error: &ReinstallCommitError) -> CommitFailure {
    match error {
        ReinstallCommitError::PreviewStale => CommitFailure {
            phase: PREFLIGHT_FAILURE_PHASE,
            rollback_result: "not_attempted",
            emit_rollback: false,
        },
        ReinstallCommitError::Failed { phase } => CommitFailure {
            phase: task_phase_for_commit_phase(*phase),
            rollback_result: if *phase == ReinstallCommitPhase::Rollback {
                "rollback_failed"
            } else {
                "not_attempted"
            },
            emit_rollback: *phase == ReinstallCommitPhase::Rollback,
        },
        ReinstallCommitError::RolledBack { failed_phase, .. } => CommitFailure {
            phase: task_phase_for_commit_phase(*failed_phase),
            rollback_result: "rolled_back",
            emit_rollback: true,
        },
        ReinstallCommitError::RollbackRequired { .. } => CommitFailure {
            phase: "rollback",
            rollback_result: "rollback_required",
            emit_rollback: true,
        },
        ReinstallCommitError::RepairRequired { .. } => CommitFailure {
            phase: "rollback",
            rollback_result: "repair_required",
            emit_rollback: true,
        },
        ReinstallCommitError::PostCommit | ReinstallCommitError::CleanupPending => CommitFailure {
            phase: "post_commit",
            rollback_result: "not_attempted_post_commit",
            emit_rollback: false,
        },
    }
}

fn task_phase_for_commit_phase(phase: ReinstallCommitPhase) -> &'static str {
    match phase {
        ReinstallCommitPhase::Revalidation => "preflight",
        ReinstallCommitPhase::Snapshot | ReinstallCommitPhase::Recovery => "backup",
        ReinstallCommitPhase::Mutation => "commit",
        ReinstallCommitPhase::Manifest => "manifest",
        ReinstallCommitPhase::Rollback => "rollback",
        ReinstallCommitPhase::PostCommit | ReinstallCommitPhase::Cleanup => "post_commit",
    }
}

fn running_event(task_id: &str, phase: &str) -> TaskProgressEvent {
    TaskProgressEvent::new(task_id, TaskKind::Install, TaskStatus::Running, phase)
}

fn failed_event(task_id: &str, error_code: &str) -> TaskProgressEvent {
    let mut event =
        TaskProgressEvent::new(task_id, TaskKind::Install, TaskStatus::Failed, FAILED_PHASE);
    event.error = Some(error_code.to_owned());
    event
}

#[cfg(test)]
#[path = "reinstall_task_tests.rs"]
mod tests;
