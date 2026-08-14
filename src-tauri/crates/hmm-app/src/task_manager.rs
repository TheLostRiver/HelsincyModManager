use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    ModImport,
    Install,
    SaveBackup,
    SaveRestore,
}

impl TaskKind {
    fn id_prefix(self) -> &'static str {
        match self {
            Self::ModImport => "mod-import",
            Self::Install => "install",
            Self::SaveBackup => "save-backup",
            Self::SaveRestore => "save-restore",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSnapshot {
    pub task_id: String,
    pub kind: TaskKind,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskProgressEvent {
    pub task_id: String,
    pub kind: TaskKind,
    pub status: TaskStatus,
    pub phase: String,
    pub current: Option<u64>,
    pub total: Option<u64>,
    pub message: Option<String>,
    pub error: Option<String>,
    pub result_ref: Option<String>,
}

impl TaskProgressEvent {
    pub fn new(
        task_id: impl Into<String>,
        kind: TaskKind,
        status: TaskStatus,
        phase: impl Into<String>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            kind,
            status,
            phase: phase.into(),
            current: None,
            total: None,
            message: None,
            error: None,
            result_ref: None,
        }
    }
}

/// Transport-neutral hook invoked while a task runner advances.
///
/// Observer failures are intentionally isolated from task state and player-file facts. Callers
/// can retain the error for reporting, but runners must not turn an observer failure into a
/// commit, rollback, or task transition.
pub trait TaskProgressObserver: Send + Sync {
    type Error;

    fn observe(&self, event: &TaskProgressEvent) -> Result<(), Self::Error>;
}

pub(crate) struct NoopTaskProgressObserver;

impl TaskProgressObserver for NoopTaskProgressObserver {
    type Error = std::convert::Infallible;

    fn observe(&self, _event: &TaskProgressEvent) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(crate) fn observe_task_progress<O: TaskProgressObserver + ?Sized>(
    events: &mut Vec<TaskProgressEvent>,
    observer: &O,
    event: TaskProgressEvent,
) {
    let _ = observer.observe(&event);
    events.push(event);
}

pub(crate) fn noop_task_progress_observer() -> NoopTaskProgressObserver {
    NoopTaskProgressObserver
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TaskManagerError {
    #[error("failed to generate task id: {0}")]
    TaskIdGenerationFailed(String),
    #[error("task not found: {0}")]
    TaskNotFound(String),
    #[error("task cannot be cancelled from status {status:?}: {task_id}")]
    TaskCannotBeCancelled { task_id: String, status: TaskStatus },
    #[error("task cannot transition from {from:?} to {to:?}: {task_id}")]
    TaskCannotTransition {
        task_id: String,
        from: TaskStatus,
        to: TaskStatus,
    },
    #[error("task scope is already active for {kind:?}: {task_id}")]
    TaskScopeBusy { kind: TaskKind, task_id: String },
    #[error("task creation is blocked during application shutdown for {kind:?}")]
    TaskCreationBlocked { kind: TaskKind },
    #[error("task store is unavailable")]
    TaskStoreUnavailable,
}

#[derive(Debug, Default)]
pub struct TaskManager {
    sequence: AtomicU64,
    lifecycle: Mutex<()>,
    tasks: Mutex<HashMap<String, TaskSnapshot>>,
    cancellation_barriers: Mutex<HashSet<String>>,
    deferred_cancellations: Mutex<HashSet<String>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_task(&self, kind: TaskKind) -> Result<TaskSnapshot, TaskManagerError> {
        let task = TaskSnapshot {
            task_id: self.generate_task_id(kind)?,
            kind,
            status: TaskStatus::Queued,
        };

        self.tasks
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?
            .insert(task.task_id.clone(), task.clone());

        Ok(task)
    }

    pub fn task_status(&self, task_id: &str) -> Option<TaskStatus> {
        self.tasks
            .lock()
            .ok()
            .and_then(|tasks| tasks.get(task_id).map(|task| task.status))
    }

    pub fn start_task(&self, task_id: &str) -> Result<TaskSnapshot, TaskManagerError> {
        self.transition_task(task_id, TaskStatus::Running, &[TaskStatus::Queued])
    }

    pub fn complete_task(&self, task_id: &str) -> Result<TaskSnapshot, TaskManagerError> {
        self.transition_task(task_id, TaskStatus::Completed, &[TaskStatus::Running])
    }

    pub fn fail_task(&self, task_id: &str) -> Result<TaskSnapshot, TaskManagerError> {
        self.transition_task(
            task_id,
            TaskStatus::Failed,
            &[TaskStatus::Queued, TaskStatus::Running],
        )
    }

    pub fn block_task_cancellation(&self, task_id: &str) -> Result<TaskSnapshot, TaskManagerError> {
        self.block_tasks_cancellation(&[task_id])
            .map(|mut tasks| tasks.remove(0))
    }

    /// Enters the cancellation barrier for all supplied running tasks while holding one
    /// lifecycle lock. Batch item commit uses this to protect the outer task and its hidden child
    /// task from a cancellation race between the two state transitions.
    pub fn block_tasks_cancellation(
        &self,
        task_ids: &[&str],
    ) -> Result<Vec<TaskSnapshot>, TaskManagerError> {
        if task_ids.is_empty() {
            return Ok(Vec::new());
        }
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let tasks = self
            .tasks
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let mut cancellation_barriers = self
            .cancellation_barriers
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let mut snapshots = Vec::with_capacity(task_ids.len());
        for task_id in task_ids {
            let task = tasks
                .get(*task_id)
                .ok_or_else(|| TaskManagerError::TaskNotFound((*task_id).to_owned()))?;
            if task.status != TaskStatus::Running {
                return Err(TaskManagerError::TaskCannotTransition {
                    task_id: task.task_id.clone(),
                    from: task.status,
                    to: TaskStatus::Running,
                });
            }
            snapshots.push(task.clone());
        }

        for task in &snapshots {
            cancellation_barriers.insert(task.task_id.clone());
        }
        Ok(snapshots)
    }

    /// Re-opens cancellation between batch items. The operation is deliberately idempotent so a
    /// terminal child task cannot turn a successfully persisted item into a runner failure.
    pub fn unblock_task_cancellation(
        &self,
        task_id: &str,
    ) -> Result<TaskSnapshot, TaskManagerError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let tasks = self
            .tasks
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let mut cancellation_barriers = self
            .cancellation_barriers
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let task = tasks
            .get(task_id)
            .ok_or_else(|| TaskManagerError::TaskNotFound(task_id.to_owned()))?;
        cancellation_barriers.remove(task_id);
        Ok(task.clone())
    }

    pub fn cancel_task(&self, task_id: &str) -> Result<TaskSnapshot, TaskManagerError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let cancellation_barriers = self
            .cancellation_barriers
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let mut deferred_cancellations = self
            .deferred_cancellations
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| TaskManagerError::TaskNotFound(task_id.to_owned()))?;

        if !matches!(task.status, TaskStatus::Queued | TaskStatus::Running) {
            return Err(TaskManagerError::TaskCannotBeCancelled {
                task_id: task.task_id.clone(),
                status: task.status,
            });
        }
        if cancellation_barriers.contains(task_id) {
            deferred_cancellations.insert(task_id.to_owned());
            return Err(TaskManagerError::TaskCannotBeCancelled {
                task_id: task.task_id.clone(),
                status: task.status,
            });
        }

        task.status = TaskStatus::Cancelled;
        deferred_cancellations.remove(task_id);

        Ok(task.clone())
    }

    /// Applies a cancellation request that arrived while a commit barrier was active. Batch
    /// runners call this only after the current item result is durable and the outer barrier has
    /// reopened, so a committed item remains successful and only later items are stopped.
    pub fn apply_deferred_cancellation(
        &self,
        task_id: &str,
    ) -> Result<Option<TaskSnapshot>, TaskManagerError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let cancellation_barriers = self
            .cancellation_barriers
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let mut deferred_cancellations = self
            .deferred_cancellations
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| TaskManagerError::TaskNotFound(task_id.to_owned()))?;
        if !deferred_cancellations.contains(task_id) || cancellation_barriers.contains(task_id) {
            return Ok(None);
        }
        deferred_cancellations.remove(task_id);
        if !matches!(task.status, TaskStatus::Queued | TaskStatus::Running) {
            return Ok(None);
        }
        task.status = TaskStatus::Cancelled;
        Ok(Some(task.clone()))
    }

    fn transition_task(
        &self,
        task_id: &str,
        to: TaskStatus,
        allowed_from: &[TaskStatus],
    ) -> Result<TaskSnapshot, TaskManagerError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let mut cancellation_barriers = self
            .cancellation_barriers
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let mut deferred_cancellations = self
            .deferred_cancellations
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| TaskManagerError::TaskNotFound(task_id.to_owned()))?;

        if !allowed_from.contains(&task.status) {
            return Err(TaskManagerError::TaskCannotTransition {
                task_id: task.task_id.clone(),
                from: task.status,
                to,
            });
        }

        task.status = to;
        if matches!(to, TaskStatus::Completed | TaskStatus::Failed) {
            cancellation_barriers.remove(task_id);
            deferred_cancellations.remove(task_id);
        }

        Ok(task.clone())
    }

    fn generate_task_id(&self, kind: TaskKind) -> Result<String, TaskManagerError> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| TaskManagerError::TaskIdGenerationFailed(error.to_string()))?
            .as_millis();
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);

        Ok(format!("{}-{millis}-{sequence}", kind.id_prefix()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_queued_mod_import_task_with_safe_task_id() {
        let manager = TaskManager::new();

        let task = manager
            .create_task(TaskKind::ModImport)
            .expect("task can be created");

        assert!(task.task_id.starts_with("mod-import-"));
        assert!(!task.task_id.contains('\\'));
        assert!(!task.task_id.contains('/'));
        assert_eq!(task.kind, TaskKind::ModImport);
        assert_eq!(task.status, TaskStatus::Queued);
        assert_eq!(manager.task_status(&task.task_id), Some(TaskStatus::Queued));
    }

    #[test]
    fn creates_queued_install_task_with_safe_task_id() {
        let manager = TaskManager::new();

        let task = manager
            .create_task(TaskKind::Install)
            .expect("install task can be created");

        assert!(task.task_id.starts_with("install-"));
        assert!(!task.task_id.contains('\\'));
        assert!(!task.task_id.contains('/'));
        assert_eq!(task.kind, TaskKind::Install);
        assert_eq!(task.status, TaskStatus::Queued);
        assert_eq!(manager.task_status(&task.task_id), Some(TaskStatus::Queued));
    }

    #[test]
    fn creates_queued_save_backup_task_with_safe_task_id() {
        let manager = TaskManager::new();

        let task = manager
            .create_task(TaskKind::SaveBackup)
            .expect("save backup task can be created");

        assert!(task.task_id.starts_with("save-backup-"));
        assert!(!task.task_id.contains('\\'));
        assert!(!task.task_id.contains('/'));
        assert_eq!(task.kind, TaskKind::SaveBackup);
        assert_eq!(task.status, TaskStatus::Queued);
        assert_eq!(manager.task_status(&task.task_id), Some(TaskStatus::Queued));
    }

    #[test]
    fn cancel_queued_task_marks_it_cancelled() {
        let manager = TaskManager::new();
        let task = manager
            .create_task(TaskKind::ModImport)
            .expect("task can be created");

        let cancelled = manager
            .cancel_task(&task.task_id)
            .expect("queued task can be cancelled");

        assert_eq!(cancelled.task_id, task.task_id);
        assert_eq!(cancelled.kind, TaskKind::ModImport);
        assert_eq!(cancelled.status, TaskStatus::Cancelled);
        assert_eq!(
            manager.task_status(&cancelled.task_id),
            Some(TaskStatus::Cancelled)
        );
    }

    #[test]
    fn cancel_unknown_task_returns_not_found() {
        let manager = TaskManager::new();

        let error = manager
            .cancel_task("missing-task")
            .expect_err("unknown task rejected");

        assert_eq!(
            error,
            TaskManagerError::TaskNotFound("missing-task".to_owned())
        );
    }

    #[test]
    fn cancel_running_task_marks_it_cancelled() {
        let manager = TaskManager::new();
        let task = manager
            .create_task(TaskKind::ModImport)
            .expect("task can be created");
        manager.start_task(&task.task_id).expect("task can start");

        let cancelled = manager
            .cancel_task(&task.task_id)
            .expect("running task can be cancelled");

        assert_eq!(cancelled.task_id, task.task_id);
        assert_eq!(cancelled.status, TaskStatus::Cancelled);
        assert_eq!(
            manager.task_status(&cancelled.task_id),
            Some(TaskStatus::Cancelled)
        );
    }

    #[test]
    fn cancellation_barrier_rejects_cancel_and_preserves_terminal_transition() {
        let manager = TaskManager::new();
        let completed_task = manager
            .create_task(TaskKind::Install)
            .expect("task can be created");
        manager
            .start_task(&completed_task.task_id)
            .expect("task can start");
        manager
            .block_task_cancellation(&completed_task.task_id)
            .expect("running task can enter commit barrier");

        let error = manager
            .cancel_task(&completed_task.task_id)
            .expect_err("commit barrier rejects cancellation");
        assert_eq!(
            error,
            TaskManagerError::TaskCannotBeCancelled {
                task_id: completed_task.task_id.clone(),
                status: TaskStatus::Running,
            }
        );
        assert_eq!(
            manager.task_status(&completed_task.task_id),
            Some(TaskStatus::Running)
        );
        assert_eq!(
            manager
                .complete_task(&completed_task.task_id)
                .expect("barrier does not block completion")
                .status,
            TaskStatus::Completed
        );

        let failed_task = manager
            .create_task(TaskKind::Install)
            .expect("task can be created");
        manager
            .start_task(&failed_task.task_id)
            .expect("task can start");
        manager
            .block_task_cancellation(&failed_task.task_id)
            .expect("running task can enter commit barrier");
        assert_eq!(
            manager
                .fail_task(&failed_task.task_id)
                .expect("barrier does not block failure")
                .status,
            TaskStatus::Failed
        );
    }

    #[test]
    fn cancellation_barrier_rejects_cancelled_or_non_running_task() {
        let manager = TaskManager::new();
        let queued = manager
            .create_task(TaskKind::Install)
            .expect("task can be created");
        assert!(matches!(
            manager.block_task_cancellation(&queued.task_id),
            Err(TaskManagerError::TaskCannotTransition {
                from: TaskStatus::Queued,
                to: TaskStatus::Running,
                ..
            })
        ));

        let cancelled = manager
            .create_task(TaskKind::Install)
            .expect("task can be created");
        manager
            .cancel_task(&cancelled.task_id)
            .expect("queued task can be cancelled");
        assert!(matches!(
            manager.block_task_cancellation(&cancelled.task_id),
            Err(TaskManagerError::TaskCannotTransition {
                from: TaskStatus::Cancelled,
                to: TaskStatus::Running,
                ..
            })
        ));
    }

    #[test]
    fn cancellation_barrier_can_be_reopened_between_items() {
        let manager = TaskManager::new();
        let task = manager
            .create_task(TaskKind::Install)
            .expect("task can be created");
        manager.start_task(&task.task_id).expect("task can start");
        manager
            .block_task_cancellation(&task.task_id)
            .expect("task can enter barrier");
        manager
            .unblock_task_cancellation(&task.task_id)
            .expect("task can leave barrier");
        let cancelled = manager
            .cancel_task(&task.task_id)
            .expect("cancel is accepted after item convergence");
        assert_eq!(cancelled.status, TaskStatus::Cancelled);
    }

    #[test]
    fn deferred_cancellation_applies_after_commit_barrier_reopens() {
        let manager = TaskManager::new();
        let task = manager
            .create_task(TaskKind::Install)
            .expect("task can be created");
        manager.start_task(&task.task_id).expect("task can start");
        manager
            .block_task_cancellation(&task.task_id)
            .expect("task can enter barrier");

        assert!(manager.cancel_task(&task.task_id).is_err());
        assert_eq!(
            manager
                .apply_deferred_cancellation(&task.task_id)
                .expect("deferred check"),
            None
        );
        manager
            .unblock_task_cancellation(&task.task_id)
            .expect("task can leave barrier");
        let cancelled = manager
            .apply_deferred_cancellation(&task.task_id)
            .expect("deferred cancellation can apply")
            .expect("cancellation was pending");

        assert_eq!(cancelled.status, TaskStatus::Cancelled);
        assert_eq!(
            manager.task_status(&task.task_id),
            Some(TaskStatus::Cancelled)
        );
    }

    #[test]
    fn paired_cancellation_barrier_is_atomic_for_batch_commit() {
        let manager = TaskManager::new();
        let outer = manager.create_task(TaskKind::Install).expect("outer task");
        let child = manager.create_task(TaskKind::Install).expect("child task");
        manager.start_task(&outer.task_id).expect("outer starts");
        manager.start_task(&child.task_id).expect("child starts");
        manager
            .block_tasks_cancellation(&[&outer.task_id, &child.task_id])
            .expect("both tasks enter barrier");
        assert!(manager.cancel_task(&outer.task_id).is_err());
        assert!(manager.cancel_task(&child.task_id).is_err());
        manager
            .unblock_task_cancellation(&outer.task_id)
            .expect("outer reopens");
        manager
            .unblock_task_cancellation(&child.task_id)
            .expect("child reopens");
        assert!(manager.cancel_task(&outer.task_id).is_ok());
    }

    #[test]
    fn cancel_finished_or_already_cancelled_task_is_rejected_without_status_change() {
        let manager = TaskManager::new();

        for status in [
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
        ] {
            let task = manager
                .create_task(TaskKind::ModImport)
                .expect("task can be created");
            set_task_status(&manager, &task.task_id, status);

            let error = manager
                .cancel_task(&task.task_id)
                .expect_err("non-queued task cannot be cancelled");

            assert_eq!(
                error,
                TaskManagerError::TaskCannotBeCancelled {
                    task_id: task.task_id.clone(),
                    status
                }
            );
            assert_eq!(manager.task_status(&task.task_id), Some(status));
        }
    }

    #[test]
    fn task_lifecycle_moves_from_queued_to_running_to_completed() {
        let manager = TaskManager::new();
        let task = manager
            .create_task(TaskKind::ModImport)
            .expect("task can be created");

        let running = manager.start_task(&task.task_id).expect("task can start");
        assert_eq!(running.status, TaskStatus::Running);
        assert_eq!(
            manager.task_status(&task.task_id),
            Some(TaskStatus::Running)
        );

        let completed = manager
            .complete_task(&task.task_id)
            .expect("task can complete");
        assert_eq!(completed.status, TaskStatus::Completed);
        assert_eq!(
            manager.task_status(&task.task_id),
            Some(TaskStatus::Completed)
        );
    }

    #[test]
    fn task_lifecycle_can_mark_running_task_failed() {
        let manager = TaskManager::new();
        let task = manager
            .create_task(TaskKind::ModImport)
            .expect("task can be created");

        manager.start_task(&task.task_id).expect("task can start");
        let failed = manager.fail_task(&task.task_id).expect("task can fail");

        assert_eq!(failed.status, TaskStatus::Failed);
        assert_eq!(manager.task_status(&task.task_id), Some(TaskStatus::Failed));
    }

    #[test]
    fn progress_event_carries_task_identity_kind_and_status() {
        let manager = TaskManager::new();
        let task = manager
            .create_task(TaskKind::ModImport)
            .expect("task can be created");

        let event = TaskProgressEvent::new(
            task.task_id.clone(),
            task.kind,
            TaskStatus::Running,
            "mod_import.preview_image.processing",
        );

        assert_eq!(event.task_id, task.task_id);
        assert_eq!(event.kind, TaskKind::ModImport);
        assert_eq!(event.status, TaskStatus::Running);
        assert_eq!(event.phase, "mod_import.preview_image.processing");
        assert_eq!(event.current, None);
        assert_eq!(event.total, None);
        assert_eq!(event.message, None);
        assert_eq!(event.error, None);
        assert_eq!(event.result_ref, None);
    }

    fn set_task_status(manager: &TaskManager, task_id: &str, status: TaskStatus) {
        let mut tasks = manager.tasks.lock().expect("task store can be locked");
        let task = tasks.get_mut(task_id).expect("task exists");
        task.status = status;
    }
}
