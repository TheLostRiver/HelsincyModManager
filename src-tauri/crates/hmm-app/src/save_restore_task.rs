use crate::save_profile_maintenance_scope::SaveProfileMaintenanceScopeRegistry;
use crate::task_manager::{noop_task_progress_observer, observe_task_progress};
use crate::{
    new_save_restore_transaction_id, CreateSaveBackupRequest, GameProfileWriteLockRegistry,
    SaveBackupExecutor, SaveRestoreCommitContext, SaveRestorePreviewError, SaveRestoreService,
    StartSaveRestoreRequest, TaskKind, TaskManager, TaskManagerError, TaskProgressEvent,
    TaskProgressObserver, TaskSnapshot, TaskStarted, TaskStatus,
};
use hmm_core::{
    SaveBackupStatus, SaveBackupTrigger, SaveRestoreTransaction, SaveRestoreTransactionStatus,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, AuditWriteFailurePolicy, SaveRestoreCommitError,
    CrossProcessWriteAdmissionResult, CrossProcessWriteGuard, SaveRestoreCommitRequest,
    SaveRestoreFileSystem, SaveRestoreFinalizeRequest, SaveRestorePrepareRequest,
    SaveRestoreTransactionRepository, ValidatedSaveRestoreSource,
};
use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

pub const SAVE_RESTORE_PREPARING_PHASE: &str = "save_restore.preparing";
pub const SAVE_RESTORE_REVALIDATING_PHASE: &str = "save_restore.revalidating";
pub const SAVE_RESTORE_PRE_RESTORE_BACKUP_PHASE: &str = "save_restore.pre_restore_backup";
pub const SAVE_RESTORE_COMMITTING_PHASE: &str = "save_restore.committing";
pub const SAVE_RESTORE_COMPLETED_PHASE: &str = "save_restore.completed";
pub const SAVE_RESTORE_FAILED_PHASE: &str = "save_restore.failed";
pub const SAVE_RESTORE_RECOVERY_REQUIRED_PHASE: &str = "save_restore.recovery_required";
pub const SAVE_RESTORE_CANCELLED_PHASE: &str = "save_restore.cancelled";

const SAVE_RESTORE_TRANSACTION_UNAVAILABLE: &str = "save_restore_transaction_unavailable";
const SAVE_RESTORE_LOCK_UNAVAILABLE: &str = "save_restore_lock_unavailable";
const SAVE_RESTORE_CANCELLED: &str = "save_restore_cancelled";
const SAVE_RESTORE_PRE_RESTORE_INVALID: &str = "save_restore_pre_restore_backup_invalid";
const SAVE_RESTORE_FACTS_CHANGED: &str = "save_restore_facts_changed";
const SAVE_RESTORE_EVIDENCE_DEGRADED: &str = "save_restore_evidence_degraded";

pub trait SaveRestoreCommitValidator: Send + Sync {
    fn validate_for_commit(
        &self,
        request: StartSaveRestoreRequest,
    ) -> Result<SaveRestoreCommitContext, SaveRestorePreviewError>;

    fn validate_for_commit_excluding_transaction(
        &self,
        request: StartSaveRestoreRequest,
        _transaction_id: &str,
    ) -> Result<SaveRestoreCommitContext, SaveRestorePreviewError> {
        self.validate_for_commit(request)
    }

    fn validate_prepared_for_commit_excluding_transaction(
        &self,
        request: StartSaveRestoreRequest,
        _validated_source: ValidatedSaveRestoreSource,
        transaction_id: &str,
    ) -> Result<SaveRestoreCommitContext, SaveRestorePreviewError> {
        self.validate_for_commit_excluding_transaction(request, transaction_id)
    }
}

impl SaveRestoreCommitValidator for SaveRestoreService {
    fn validate_for_commit(
        &self,
        request: StartSaveRestoreRequest,
    ) -> Result<SaveRestoreCommitContext, SaveRestorePreviewError> {
        SaveRestoreService::validate_for_commit(self, request)
    }

    fn validate_for_commit_excluding_transaction(
        &self,
        request: StartSaveRestoreRequest,
        transaction_id: &str,
    ) -> Result<SaveRestoreCommitContext, SaveRestorePreviewError> {
        SaveRestoreService::validate_for_commit_excluding_transaction(self, request, transaction_id)
    }

    fn validate_prepared_for_commit_excluding_transaction(
        &self,
        request: StartSaveRestoreRequest,
        validated_source: ValidatedSaveRestoreSource,
        transaction_id: &str,
    ) -> Result<SaveRestoreCommitContext, SaveRestorePreviewError> {
        SaveRestoreService::validate_prepared_for_commit_excluding_transaction(
            self,
            request,
            validated_source,
            transaction_id,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRestoreTaskRunError {
    pub events: Vec<TaskProgressEvent>,
    pub error_code: String,
}

struct SaveRestoreTransactionFailure<'a> {
    status: SaveRestoreTransactionStatus,
    error_code: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SaveRestoreTaskScope {
    game_id: String,
    profile_id: String,
}

impl From<&StartSaveRestoreRequest> for SaveRestoreTaskScope {
    fn from(request: &StartSaveRestoreRequest) -> Self {
        Self {
            game_id: request.game_id.as_str().to_owned(),
            profile_id: request.profile_id.as_str().to_owned(),
        }
    }
}

#[derive(Debug)]
pub struct SaveRestoreTaskScopeRegistry {
    active_tasks: Mutex<BTreeMap<SaveRestoreTaskScope, String>>,
    exit_requested: AtomicBool,
    pending_sequence: AtomicU64,
    maintenance_registry: Arc<SaveProfileMaintenanceScopeRegistry>,
}

impl Default for SaveRestoreTaskScopeRegistry {
    fn default() -> Self {
        Self::with_maintenance_registry(Arc::new(SaveProfileMaintenanceScopeRegistry::default()))
    }
}

impl SaveRestoreTaskScopeRegistry {
    pub fn with_maintenance_registry(
        maintenance_registry: Arc<SaveProfileMaintenanceScopeRegistry>,
    ) -> Self {
        Self {
            active_tasks: Mutex::new(BTreeMap::new()),
            exit_requested: AtomicBool::new(false),
            pending_sequence: AtomicU64::new(0),
            maintenance_registry,
        }
    }

    pub fn reserve_task(
        &self,
        request: &StartSaveRestoreRequest,
        create_task: impl FnOnce() -> Result<TaskSnapshot, TaskManagerError>,
    ) -> Result<crate::TaskSnapshot, TaskManagerError> {
        let pending_id = format!(
            "save-restore-pending-{}",
            self.pending_sequence.fetch_add(1, Ordering::Relaxed)
        );
        let mut pending = self.reserve_pending(request, pending_id)?;
        let task = self.maintenance_registry.reserve_task(
            &request.game_id,
            &request.profile_id,
            TaskKind::SaveRestore,
            create_task,
        )?;
        if let Err(error) = pending.commit(&task.task_id) {
            self.maintenance_registry.release_task(
                &request.game_id,
                &request.profile_id,
                &task.task_id,
            );
            return Err(error);
        }
        Ok(task)
    }

    fn reserve_pending(
        &self,
        request: &StartSaveRestoreRequest,
        reservation_id: String,
    ) -> Result<SaveRestoreTaskScopePendingGuard<'_>, TaskManagerError> {
        let scope = SaveRestoreTaskScope::from(request);
        let mut active_tasks = self
            .active_tasks
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        if self.exit_requested.load(Ordering::Acquire) {
            return Err(TaskManagerError::TaskCreationBlocked {
                kind: TaskKind::SaveRestore,
            });
        }
        if let Some(task_id) = active_tasks.get(&scope) {
            return Err(TaskManagerError::TaskScopeBusy {
                kind: TaskKind::SaveRestore,
                task_id: task_id.clone(),
            });
        }
        active_tasks.insert(scope.clone(), reservation_id.clone());
        drop(active_tasks);
        Ok(SaveRestoreTaskScopePendingGuard {
            registry: self,
            scope,
            reservation_id,
            committed: false,
        })
    }

    pub fn release_task(&self, request: &StartSaveRestoreRequest, task_id: &str) {
        let scope = SaveRestoreTaskScope::from(request);
        let should_release_shared_scope = {
            let Ok(mut active_tasks) = self.active_tasks.lock() else {
                return;
            };
            if active_tasks
                .get(&scope)
                .is_some_and(|active_task_id| active_task_id == task_id)
            {
                active_tasks.remove(&scope);
                true
            } else {
                false
            }
        };
        if should_release_shared_scope {
            self.maintenance_registry
                .release_task(&request.game_id, &request.profile_id, task_id);
        }
    }

    fn acquire_cross_process_for_task(
        &self,
        request: &StartSaveRestoreRequest,
        task_manager: &TaskManager,
        task_id: &str,
    ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>> {
        self.maintenance_registry.acquire_cross_process_for_task(
            &request.game_id,
            &request.profile_id,
            task_manager,
            task_id,
        )
    }

    pub fn has_active_task(&self) -> Result<bool, TaskManagerError> {
        let active_tasks = self
            .active_tasks
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        Ok(!active_tasks.is_empty())
    }

    pub fn begin_exit_if_idle(&self) -> Result<bool, TaskManagerError> {
        let active_tasks = self
            .active_tasks
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        if !active_tasks.is_empty() {
            return Ok(false);
        }
        self.exit_requested.store(true, Ordering::Release);
        Ok(true)
    }
}

struct SaveRestoreTaskScopePendingGuard<'a> {
    registry: &'a SaveRestoreTaskScopeRegistry,
    scope: SaveRestoreTaskScope,
    reservation_id: String,
    committed: bool,
}

impl SaveRestoreTaskScopePendingGuard<'_> {
    fn commit(&mut self, task_id: &str) -> Result<(), TaskManagerError> {
        let mut active_tasks = self
            .registry
            .active_tasks
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        if active_tasks
            .get(&self.scope)
            .is_none_or(|active_id| active_id != &self.reservation_id)
        {
            return Err(TaskManagerError::TaskStoreUnavailable);
        }
        active_tasks.insert(self.scope.clone(), task_id.to_owned());
        self.committed = true;
        Ok(())
    }
}

impl Drop for SaveRestoreTaskScopePendingGuard<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let Ok(mut active_tasks) = self.registry.active_tasks.lock() else {
                return;
            };
            if active_tasks
                .get(&self.scope)
                .is_some_and(|active_id| active_id == &self.reservation_id)
            {
                active_tasks.remove(&self.scope);
            }
        }
    }
}

struct SaveRestoreTaskScopeReleaseGuard<'a> {
    registry: &'a SaveRestoreTaskScopeRegistry,
    request: &'a StartSaveRestoreRequest,
    task_id: &'a str,
}

impl Drop for SaveRestoreTaskScopeReleaseGuard<'_> {
    fn drop(&mut self) {
        self.registry.release_task(self.request, self.task_id);
    }
}

pub struct SaveRestoreTaskService {
    task_manager: Arc<TaskManager>,
    scope_registry: Arc<SaveRestoreTaskScopeRegistry>,
}

impl SaveRestoreTaskService {
    pub fn new(task_manager: Arc<TaskManager>) -> Self {
        Self::with_scope_registry(
            task_manager,
            Arc::new(SaveRestoreTaskScopeRegistry::default()),
        )
    }

    pub fn with_scope_registry(
        task_manager: Arc<TaskManager>,
        scope_registry: Arc<SaveRestoreTaskScopeRegistry>,
    ) -> Self {
        Self {
            task_manager,
            scope_registry,
        }
    }

    pub fn start_save_restore_task(
        &self,
        request: &StartSaveRestoreRequest,
    ) -> Result<TaskStarted, TaskManagerError> {
        let task = self.scope_registry.reserve_task(request, || {
            self.task_manager.create_task(TaskKind::SaveRestore)
        })?;
        Ok(TaskStarted {
            task_id: task.task_id,
            kind: task.kind,
            status: task.status,
        })
    }

    pub fn abort_queued_save_restore_task(
        &self,
        request: &StartSaveRestoreRequest,
        task_id: &str,
    ) -> Result<(), TaskManagerError> {
        let result = match self.task_manager.task_status(task_id) {
            Some(TaskStatus::Queued | TaskStatus::Running) => {
                self.task_manager.fail_task(task_id).map(|_| ())
            }
            Some(TaskStatus::Failed | TaskStatus::Cancelled) => Ok(()),
            Some(status) => Err(TaskManagerError::TaskCannotTransition {
                task_id: task_id.to_owned(),
                from: status,
                to: TaskStatus::Failed,
            }),
            None => Err(TaskManagerError::TaskNotFound(task_id.to_owned())),
        };
        self.scope_registry.release_task(request, task_id);
        result
    }
}

pub struct SaveRestoreTaskRunner {
    task_manager: Arc<TaskManager>,
    validator: Arc<dyn SaveRestoreCommitValidator>,
    file_system: Arc<dyn SaveRestoreFileSystem>,
    transactions: Arc<dyn SaveRestoreTransactionRepository>,
    backup_executor: Arc<dyn SaveBackupExecutor>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
    write_locks: Arc<GameProfileWriteLockRegistry>,
    scope_registry: Arc<SaveRestoreTaskScopeRegistry>,
}

impl SaveRestoreTaskRunner {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_manager: Arc<TaskManager>,
        validator: Arc<dyn SaveRestoreCommitValidator>,
        file_system: Arc<dyn SaveRestoreFileSystem>,
        transactions: Arc<dyn SaveRestoreTransactionRepository>,
        backup_executor: Arc<dyn SaveBackupExecutor>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
    ) -> Self {
        Self::with_scope_registry(
            task_manager,
            validator,
            file_system,
            transactions,
            backup_executor,
            audit_log,
            clock,
            write_locks,
            Arc::new(SaveRestoreTaskScopeRegistry::default()),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_scope_registry(
        task_manager: Arc<TaskManager>,
        validator: Arc<dyn SaveRestoreCommitValidator>,
        file_system: Arc<dyn SaveRestoreFileSystem>,
        transactions: Arc<dyn SaveRestoreTransactionRepository>,
        backup_executor: Arc<dyn SaveBackupExecutor>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
        scope_registry: Arc<SaveRestoreTaskScopeRegistry>,
    ) -> Self {
        Self {
            task_manager,
            validator,
            file_system,
            transactions,
            backup_executor,
            audit_log,
            clock,
            write_locks,
            scope_registry,
        }
    }

    pub fn run_save_restore_task(
        &self,
        task_id: &str,
        request: StartSaveRestoreRequest,
    ) -> Result<Vec<TaskProgressEvent>, SaveRestoreTaskRunError> {
        let observer = noop_task_progress_observer();
        self.run_save_restore_task_with_observer(task_id, request, &observer)
    }

    pub fn run_save_restore_task_with_observer<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: StartSaveRestoreRequest,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, SaveRestoreTaskRunError> {
        let _scope_release = SaveRestoreTaskScopeReleaseGuard {
            registry: self.scope_registry.as_ref(),
            request: &request,
            task_id,
        };
        if self.task_manager.start_task(task_id).is_err() {
            return Err(SaveRestoreTaskRunError {
                events: Vec::new(),
                error_code: SAVE_RESTORE_TRANSACTION_UNAVAILABLE.to_owned(),
            });
        }

        let _save_cross_process_guard = match self.scope_registry.acquire_cross_process_for_task(
            &request,
            &self.task_manager,
            task_id,
        ) {
            Ok(guard) => guard,
            Err(error) => {
                return Err(self.fail_without_transaction(
                    task_id,
                    &request,
                    Vec::new(),
                    observer,
                    error.code(),
                ));
            }
        };

        let mut events = Vec::new();
        observe_task_progress(
            &mut events,
            observer,
            running_event(task_id, SAVE_RESTORE_PREPARING_PHASE),
        );
        let context = self
            .validator
            .validate_for_commit(request.clone())
            .map_err(|error| {
                self.fail_without_transaction(
                    task_id,
                    &request,
                    events.clone(),
                    observer,
                    error.code(),
                )
            })?;
        let now = self.clock.now_unix_millis().map_err(|_| {
            self.fail_without_transaction(
                task_id,
                &request,
                events.clone(),
                observer,
                SAVE_RESTORE_TRANSACTION_UNAVAILABLE,
            )
        })?;
        let transaction_id = new_save_restore_transaction_id();
        let mut transaction = SaveRestoreTransaction {
            transaction_id: transaction_id.clone(),
            game_id: request.game_id.clone(),
            profile_id: request.profile_id.clone(),
            backup_id: request.backup_id.clone(),
            pre_restore_backup_id: None,
            status: SaveRestoreTransactionStatus::Planned,
            error_code: None,
            created_at: now,
            updated_at: now,
        };
        if self.transactions.save_transaction(&transaction).is_err() {
            return Err(self.fail_without_transaction(
                task_id,
                &request,
                events,
                observer,
                SAVE_RESTORE_TRANSACTION_UNAVAILABLE,
            ));
        }

        let prepared = match self.file_system.prepare_restore(SaveRestorePrepareRequest {
            transaction_id: transaction_id.clone(),
            summary: context.summary.clone(),
            target_directory: context.settings.save_directory.clone(),
        }) {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(self.fail_transaction(
                    task_id,
                    &request,
                    &mut transaction,
                    events,
                    observer,
                    SaveRestoreTransactionFailure {
                        status: SaveRestoreTransactionStatus::Failed,
                        error_code: error.code(),
                    },
                ));
            }
        };
        if prepared.evidence_digest != context.validated_source.evidence_digest
            || prepared.file_count != context.validated_source.file_count
            || prepared.total_uncompressed_bytes
                != context.validated_source.total_uncompressed_bytes
        {
            self.file_system.discard_prepared(&prepared.prepared_id);
            return Err(self.fail_transaction(
                task_id,
                &request,
                &mut transaction,
                events,
                observer,
                SaveRestoreTransactionFailure {
                    status: SaveRestoreTransactionStatus::Failed,
                    error_code: SAVE_RESTORE_FACTS_CHANGED,
                },
            ));
        }
        if self
            .persist_transaction_status(
                &mut transaction,
                SaveRestoreTransactionStatus::Prepared,
                None,
            )
            .is_err()
        {
            self.file_system.discard_prepared(&prepared.prepared_id);
            return Err(self.fail_transaction_persistence(
                task_id,
                &request,
                &transaction,
                events,
                observer,
            ));
        }
        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
            return Err(self.cancel_transaction(
                task_id,
                &request,
                &mut transaction,
                &prepared.prepared_id,
                events,
                observer,
            ));
        }

        let pre_restore_summary = if context.settings.pre_restore_backup_enabled {
            observe_task_progress(
                &mut events,
                observer,
                running_event(task_id, SAVE_RESTORE_PRE_RESTORE_BACKUP_PHASE),
            );
            let result = self.backup_executor.create_backup(
                CreateSaveBackupRequest {
                    game_id: request.game_id.clone(),
                    profile_id: request.profile_id.clone(),
                    note: None,
                },
                SaveBackupTrigger::PreRestore,
            );
            let summary = match result {
                Ok(result) => result.summary,
                Err(error) => {
                    self.file_system.discard_prepared(&prepared.prepared_id);
                    return Err(self.fail_transaction(
                        task_id,
                        &request,
                        &mut transaction,
                        events,
                        observer,
                        SaveRestoreTransactionFailure {
                            status: SaveRestoreTransactionStatus::Failed,
                            error_code: error.code(),
                        },
                    ));
                }
            };
            if summary.game_id != request.game_id
                || summary.profile_id != request.profile_id
                || summary.trigger != SaveBackupTrigger::PreRestore
                || summary.status != SaveBackupStatus::Completed
            {
                self.file_system.discard_prepared(&prepared.prepared_id);
                return Err(self.fail_transaction(
                    task_id,
                    &request,
                    &mut transaction,
                    events,
                    observer,
                    SaveRestoreTransactionFailure {
                        status: SaveRestoreTransactionStatus::Failed,
                        error_code: SAVE_RESTORE_PRE_RESTORE_INVALID,
                    },
                ));
            }
            transaction.pre_restore_backup_id = Some(summary.backup_id.clone());
            if self
                .persist_transaction_status(
                    &mut transaction,
                    SaveRestoreTransactionStatus::PreRestoreCompleted,
                    None,
                )
                .is_err()
            {
                self.file_system.discard_prepared(&prepared.prepared_id);
                return Err(self.fail_transaction_persistence(
                    task_id,
                    &request,
                    &transaction,
                    events,
                    observer,
                ));
            }
            Some(summary)
        } else {
            None
        };

        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
            return Err(self.cancel_transaction(
                task_id,
                &request,
                &mut transaction,
                &prepared.prepared_id,
                events,
                observer,
            ));
        }

        let _game_cross_process_guard =
            match self.write_locks.acquire_cross_process_for_task(
                &request.game_id,
                &request.profile_id,
                &self.task_manager,
                task_id,
            ) {
                Ok(guard) => guard,
                Err(error) => {
                    if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                        return Err(self.cancel_transaction(
                            task_id,
                            &request,
                            &mut transaction,
                            &prepared.prepared_id,
                            events,
                            observer,
                        ));
                    }
                    self.file_system.discard_prepared(&prepared.prepared_id);
                    return Err(self.fail_transaction(
                        task_id,
                        &request,
                        &mut transaction,
                        events,
                        observer,
                        SaveRestoreTransactionFailure {
                            status: SaveRestoreTransactionStatus::Failed,
                            error_code: error.code(),
                        },
                    ));
                }
            };
        let write_lock = self
            .write_locks
            .lock_for(&request.game_id, &request.profile_id);
        let _guard = write_lock.lock().map_err(|_| {
            self.file_system.discard_prepared(&prepared.prepared_id);
            self.fail_transaction(
                task_id,
                &request,
                &mut transaction,
                events.clone(),
                observer,
                SaveRestoreTransactionFailure {
                    status: SaveRestoreTransactionStatus::Failed,
                    error_code: SAVE_RESTORE_LOCK_UNAVAILABLE,
                },
            )
        })?;
        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
            return Err(self.cancel_transaction(
                task_id,
                &request,
                &mut transaction,
                &prepared.prepared_id,
                events,
                observer,
            ));
        }
        observe_task_progress(
            &mut events,
            observer,
            running_event(task_id, SAVE_RESTORE_REVALIDATING_PHASE),
        );
        let revalidated = self
            .validator
            .validate_prepared_for_commit_excluding_transaction(
                request.clone(),
                context.validated_source.clone(),
                &transaction_id,
            )
            .map_err(|error| {
                self.file_system.discard_prepared(&prepared.prepared_id);
                self.fail_transaction(
                    task_id,
                    &request,
                    &mut transaction,
                    events.clone(),
                    observer,
                    SaveRestoreTransactionFailure {
                        status: SaveRestoreTransactionStatus::Failed,
                        error_code: error.code(),
                    },
                )
            })?;
        if revalidated.facts_digest != context.facts_digest {
            self.file_system.discard_prepared(&prepared.prepared_id);
            return Err(self.fail_transaction(
                task_id,
                &request,
                &mut transaction,
                events,
                observer,
                SaveRestoreTransactionFailure {
                    status: SaveRestoreTransactionStatus::Failed,
                    error_code: SAVE_RESTORE_FACTS_CHANGED,
                },
            ));
        }
        if self.task_manager.block_task_cancellation(task_id).is_err() {
            if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                return Err(self.cancel_transaction(
                    task_id,
                    &request,
                    &mut transaction,
                    &prepared.prepared_id,
                    events,
                    observer,
                ));
            }
            self.file_system.discard_prepared(&prepared.prepared_id);
            return Err(self.fail_transaction(
                task_id,
                &request,
                &mut transaction,
                events,
                observer,
                SaveRestoreTransactionFailure {
                    status: SaveRestoreTransactionStatus::Failed,
                    error_code: SAVE_RESTORE_CANCELLED,
                },
            ));
        }
        if self
            .persist_transaction_status(
                &mut transaction,
                SaveRestoreTransactionStatus::Committing,
                None,
            )
            .is_err()
        {
            self.file_system.discard_prepared(&prepared.prepared_id);
            return Err(self.fail_transaction_persistence(
                task_id,
                &request,
                &transaction,
                events,
                observer,
            ));
        }
        observe_task_progress(
            &mut events,
            observer,
            running_event(task_id, SAVE_RESTORE_COMMITTING_PHASE),
        );
        let commit = self.file_system.commit_restore(SaveRestoreCommitRequest {
            transaction_id: transaction_id.clone(),
            prepared_id: prepared.prepared_id,
            summary: revalidated.summary,
            target_directory: revalidated.settings.save_directory.clone(),
            pre_restore_summary,
        });
        let commit = match commit {
            Ok(commit) => commit,
            Err(error) => {
                if error == SaveRestoreCommitError::RolledBack {
                    return Err(self.fail_rolled_back_transaction(
                        task_id,
                        &request,
                        &mut transaction,
                        events,
                        observer,
                        SaveRestoreFinalizeRequest {
                            transaction_id: transaction_id.clone(),
                            target_directory: revalidated.settings.save_directory,
                        },
                        error.code(),
                    ));
                }
                let terminal = match error {
                    SaveRestoreCommitError::RecoveryRequired => {
                        SaveRestoreTransactionStatus::RecoveryRequired
                    }
                    _ => SaveRestoreTransactionStatus::Failed,
                };
                let run_error = self.fail_transaction(
                    task_id,
                    &request,
                    &mut transaction,
                    events,
                    observer,
                    SaveRestoreTransactionFailure {
                        status: terminal,
                        error_code: error.code(),
                    },
                );
                return Err(run_error);
            }
        };

        if self
            .persist_transaction_status(
                &mut transaction,
                SaveRestoreTransactionStatus::Committed,
                None,
            )
            .is_err()
        {
            return Err(self.fail_transaction_persistence(
                task_id,
                &request,
                &transaction,
                events,
                observer,
            ));
        }
        if let Err(error) = self
            .file_system
            .finalize_restore(SaveRestoreFinalizeRequest {
                transaction_id: transaction_id.clone(),
                target_directory: revalidated.settings.save_directory,
            })
        {
            return Err(self.fail_transaction(
                task_id,
                &request,
                &mut transaction,
                events,
                observer,
                SaveRestoreTransactionFailure {
                    status: SaveRestoreTransactionStatus::RecoveryRequired,
                    error_code: error.code(),
                },
            ));
        }
        if self
            .persist_transaction_status(
                &mut transaction,
                SaveRestoreTransactionStatus::Completed,
                None,
            )
            .is_err()
        {
            return Err(self.fail_transaction_persistence(
                task_id,
                &request,
                &transaction,
                events,
                observer,
            ));
        }
        let audit_ok = self.record_success_audit(
            task_id,
            &request,
            &transaction_id,
            commit.restored_file_count,
        );
        let task_projection_degraded = self.task_manager.complete_task(task_id).is_err();
        if task_projection_degraded {
            self.record_warning_audit(
                task_id,
                &request,
                &transaction_id,
                SAVE_RESTORE_EVIDENCE_DEGRADED,
            );
        }
        let mut completed = TaskProgressEvent::new(
            task_id.to_owned(),
            TaskKind::SaveRestore,
            TaskStatus::Completed,
            SAVE_RESTORE_COMPLETED_PHASE,
        );
        completed.current = Some(commit.restored_file_count as u64);
        completed.total = Some(commit.restored_file_count as u64);
        if !audit_ok || task_projection_degraded {
            completed.error = Some(SAVE_RESTORE_EVIDENCE_DEGRADED.to_owned());
        }
        observe_task_progress(&mut events, observer, completed);
        Ok(events)
    }

    fn persist_transaction_status(
        &self,
        transaction: &mut SaveRestoreTransaction,
        status: SaveRestoreTransactionStatus,
        error_code: Option<&str>,
    ) -> anyhow::Result<()> {
        let mut candidate = transaction.clone();
        candidate.status = status;
        candidate.error_code = error_code.map(str::to_owned);
        candidate.updated_at = self.clock.now_unix_millis()?;
        self.transactions.save_transaction(&candidate)?;
        *transaction = candidate;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn cancel_transaction<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &StartSaveRestoreRequest,
        transaction: &mut SaveRestoreTransaction,
        prepared_id: &str,
        mut events: Vec<TaskProgressEvent>,
        observer: &O,
    ) -> SaveRestoreTaskRunError {
        if self
            .persist_transaction_status(
                transaction,
                SaveRestoreTransactionStatus::Failed,
                Some(SAVE_RESTORE_CANCELLED),
            )
            .is_err()
        {
            return self.fail_transaction_unavailable(
                task_id,
                request,
                &transaction.transaction_id,
                events,
                observer,
            );
        }

        self.file_system.discard_prepared(prepared_id);
        self.record_failure_audit(
            task_id,
            request,
            &transaction.transaction_id,
            SAVE_RESTORE_CANCELLED,
        );
        observe_task_progress(&mut events, observer, cancelled_event(task_id));
        SaveRestoreTaskRunError {
            events,
            error_code: SAVE_RESTORE_CANCELLED.to_owned(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn fail_rolled_back_transaction<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &StartSaveRestoreRequest,
        transaction: &mut SaveRestoreTransaction,
        events: Vec<TaskProgressEvent>,
        observer: &O,
        finalize_request: SaveRestoreFinalizeRequest,
        error_code: &str,
    ) -> SaveRestoreTaskRunError {
        let transaction_id = transaction.transaction_id.clone();
        if self
            .persist_transaction_status(
                transaction,
                SaveRestoreTransactionStatus::RolledBack,
                Some(error_code),
            )
            .is_err()
        {
            return self.fail_transaction_unavailable(
                task_id,
                request,
                &transaction_id,
                events,
                observer,
            );
        }

        let cleanup_warning = self
            .file_system
            .finalize_restore(finalize_request)
            .err()
            .map(|error| error.code());
        let result = self.fail_task_with_message(
            task_id,
            request,
            events,
            observer,
            Some(&transaction_id),
            SAVE_RESTORE_FAILED_PHASE,
            error_code,
            cleanup_warning,
        );
        if let Some(warning) = cleanup_warning {
            self.record_warning_audit(task_id, request, &transaction_id, warning);
        }
        result
    }

    fn fail_transaction<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &StartSaveRestoreRequest,
        transaction: &mut SaveRestoreTransaction,
        events: Vec<TaskProgressEvent>,
        observer: &O,
        failure: SaveRestoreTransactionFailure<'_>,
    ) -> SaveRestoreTaskRunError {
        let transaction_id = transaction.transaction_id.clone();
        if self
            .persist_transaction_status(transaction, failure.status, Some(failure.error_code))
            .is_err()
        {
            return self.fail_transaction_unavailable(
                task_id,
                request,
                &transaction_id,
                events,
                observer,
            );
        }
        let phase = if failure.status == SaveRestoreTransactionStatus::RecoveryRequired {
            SAVE_RESTORE_RECOVERY_REQUIRED_PHASE
        } else {
            SAVE_RESTORE_FAILED_PHASE
        };
        self.fail_task(
            task_id,
            request,
            events,
            observer,
            Some(&transaction_id),
            phase,
            failure.error_code,
        )
    }

    fn fail_without_transaction<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &StartSaveRestoreRequest,
        events: Vec<TaskProgressEvent>,
        observer: &O,
        error_code: &str,
    ) -> SaveRestoreTaskRunError {
        self.fail_task(
            task_id,
            request,
            events,
            observer,
            None,
            SAVE_RESTORE_FAILED_PHASE,
            error_code,
        )
    }

    fn fail_transaction_persistence<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &StartSaveRestoreRequest,
        transaction: &SaveRestoreTransaction,
        events: Vec<TaskProgressEvent>,
        observer: &O,
    ) -> SaveRestoreTaskRunError {
        self.fail_transaction_unavailable(
            task_id,
            request,
            &transaction.transaction_id,
            events,
            observer,
        )
    }

    fn fail_transaction_unavailable<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &StartSaveRestoreRequest,
        transaction_id: &str,
        mut events: Vec<TaskProgressEvent>,
        observer: &O,
    ) -> SaveRestoreTaskRunError {
        // Durable transaction failure takes precedence over a volatile cancellation projection.
        let _ = self.task_manager.fail_task(task_id);
        let mut failed = TaskProgressEvent::new(
            task_id.to_owned(),
            TaskKind::SaveRestore,
            TaskStatus::Failed,
            SAVE_RESTORE_RECOVERY_REQUIRED_PHASE,
        );
        failed.error = Some(SAVE_RESTORE_TRANSACTION_UNAVAILABLE.to_owned());
        observe_task_progress(&mut events, observer, failed);
        self.record_failure_audit(
            task_id,
            request,
            transaction_id,
            SAVE_RESTORE_TRANSACTION_UNAVAILABLE,
        );
        SaveRestoreTaskRunError {
            events,
            error_code: SAVE_RESTORE_TRANSACTION_UNAVAILABLE.to_owned(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn fail_task<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &StartSaveRestoreRequest,
        events: Vec<TaskProgressEvent>,
        observer: &O,
        transaction_id: Option<&str>,
        phase: &str,
        error_code: &str,
    ) -> SaveRestoreTaskRunError {
        self.fail_task_with_message(
            task_id,
            request,
            events,
            observer,
            transaction_id,
            phase,
            error_code,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn fail_task_with_message<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: &StartSaveRestoreRequest,
        mut events: Vec<TaskProgressEvent>,
        observer: &O,
        transaction_id: Option<&str>,
        phase: &str,
        error_code: &str,
        message: Option<&str>,
    ) -> SaveRestoreTaskRunError {
        if self.task_manager.fail_task(task_id).is_err()
            && self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled)
        {
            observe_task_progress(&mut events, observer, cancelled_event(task_id));
            self.record_failure_audit(
                task_id,
                request,
                transaction_id.unwrap_or("unavailable"),
                SAVE_RESTORE_CANCELLED,
            );
            return SaveRestoreTaskRunError {
                events,
                error_code: SAVE_RESTORE_CANCELLED.to_owned(),
            };
        }
        let mut failed = TaskProgressEvent::new(
            task_id.to_owned(),
            TaskKind::SaveRestore,
            TaskStatus::Failed,
            phase,
        );
        failed.error = Some(error_code.to_owned());
        failed.message = message.map(str::to_owned);
        observe_task_progress(&mut events, observer, failed);
        self.record_failure_audit(
            task_id,
            request,
            transaction_id.unwrap_or("unavailable"),
            error_code,
        );
        SaveRestoreTaskRunError {
            events,
            error_code: error_code.to_owned(),
        }
    }

    fn record_success_audit(
        &self,
        task_id: &str,
        request: &StartSaveRestoreRequest,
        transaction_id: &str,
        file_count: u32,
    ) -> bool {
        let mut fields = audit_fields(task_id, request, transaction_id);
        fields.insert("file_count".to_owned(), file_count.to_string());
        self.record_audit("success", fields)
    }

    fn record_failure_audit(
        &self,
        task_id: &str,
        request: &StartSaveRestoreRequest,
        transaction_id: &str,
        error_code: &str,
    ) {
        let mut fields = audit_fields(task_id, request, transaction_id);
        fields.insert("error_code".to_owned(), error_code.to_owned());
        self.record_audit("failure", fields);
    }

    fn record_warning_audit(
        &self,
        task_id: &str,
        request: &StartSaveRestoreRequest,
        transaction_id: &str,
        warning_code: &str,
    ) {
        let mut fields = audit_fields(task_id, request, transaction_id);
        fields.insert("error_code".to_owned(), warning_code.to_owned());
        self.record_audit("warning", fields);
    }

    fn record_audit(&self, result: &str, fields: BTreeMap<String, String>) -> bool {
        self.audit_log
            .record_with_policy(
                AuditLogEvent {
                    timestamp_unix_millis: self.clock.now_unix_millis().unwrap_or_default(),
                    category: "save_restore".to_owned(),
                    operation: "restore".to_owned(),
                    result: result.to_owned(),
                    fields,
                },
                AuditWriteFailurePolicy::for_commit_result(result),
            )
            .is_ok()
    }
}

fn running_event(task_id: &str, phase: &str) -> TaskProgressEvent {
    TaskProgressEvent::new(
        task_id.to_owned(),
        TaskKind::SaveRestore,
        TaskStatus::Running,
        phase,
    )
}

fn cancelled_event(task_id: &str) -> TaskProgressEvent {
    TaskProgressEvent::new(
        task_id.to_owned(),
        TaskKind::SaveRestore,
        TaskStatus::Cancelled,
        SAVE_RESTORE_CANCELLED_PHASE,
    )
}

fn audit_fields(
    task_id: &str,
    request: &StartSaveRestoreRequest,
    transaction_id: &str,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("task_id".to_owned(), task_id.to_owned()),
        ("transaction_id".to_owned(), transaction_id.to_owned()),
        ("game_id".to_owned(), request.game_id.as_str().to_owned()),
        (
            "profile_id".to_owned(),
            request.profile_id.as_str().to_owned(),
        ),
        ("backup_id".to_owned(), request.backup_id.clone()),
    ])
}
