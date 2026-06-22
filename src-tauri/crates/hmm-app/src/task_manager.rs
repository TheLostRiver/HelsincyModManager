use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    ModImport,
}

impl TaskKind {
    fn id_prefix(self) -> &'static str {
        match self {
            Self::ModImport => "mod-import",
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
}

impl TaskProgressEvent {
    pub fn new(task: &TaskSnapshot, status: TaskStatus, phase: impl Into<String>) -> Self {
        Self {
            task_id: task.task_id.clone(),
            kind: task.kind,
            status,
            phase: phase.into(),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TaskManagerError {
    #[error("failed to generate task id: {0}")]
    TaskIdGenerationFailed(String),
    #[error("task store is unavailable")]
    TaskStoreUnavailable,
}

#[derive(Debug, Default)]
pub struct TaskManager {
    sequence: AtomicU64,
    tasks: Mutex<HashMap<String, TaskSnapshot>>,
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
    fn progress_event_carries_task_identity_kind_and_status() {
        let manager = TaskManager::new();
        let task = manager
            .create_task(TaskKind::ModImport)
            .expect("task can be created");

        let event = TaskProgressEvent::new(
            &task,
            TaskStatus::Running,
            "mod_import.preview_image.processing",
        );

        assert_eq!(event.task_id, task.task_id);
        assert_eq!(event.kind, TaskKind::ModImport);
        assert_eq!(event.status, TaskStatus::Running);
        assert_eq!(event.phase, "mod_import.preview_image.processing");
    }
}
