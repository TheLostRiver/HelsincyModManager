use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

use crate::{TaskKind, TaskManager, TaskManagerError, TaskStatus};
use hmm_core::ModId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartImportModTaskRequest {
    pub archive_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartImportModRevisionTaskRequest {
    pub archive_path: PathBuf,
    pub mod_id: ModId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStarted {
    pub task_id: String,
    pub kind: TaskKind,
    pub status: TaskStatus,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModImportTaskError {
    #[error("archive path cannot be empty")]
    ArchivePathEmpty,
    #[error("archive path must be absolute")]
    ArchivePathNotAbsolute,
    #[error("archive file not found")]
    ArchiveFileNotFound,
    #[error("archive path is not a file")]
    ArchivePathIsNotFile,
    #[error("logical Mod id cannot be empty")]
    ModIdEmpty,
    #[error("failed to generate task id: {0}")]
    TaskIdGenerationFailed(String),
    #[error("failed to register task: {0}")]
    TaskRegistrationFailed(String),
}

impl ModImportTaskError {
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::ArchivePathEmpty => "archive_path_empty",
            Self::ArchivePathNotAbsolute => "archive_path_not_absolute",
            Self::ArchiveFileNotFound => "archive_file_not_found",
            Self::ArchivePathIsNotFile => "archive_path_is_not_file",
            Self::ModIdEmpty => "mod_id_empty",
            Self::TaskIdGenerationFailed(_) => "task_id_generation_failed",
            Self::TaskRegistrationFailed(_) => "task_registration_failed",
        }
    }
}

pub struct ModImportTaskService {
    task_manager: Arc<TaskManager>,
}

impl ModImportTaskService {
    pub fn new(task_manager: Arc<TaskManager>) -> Self {
        Self { task_manager }
    }

    pub fn start_import_mod_task(
        &self,
        request: StartImportModTaskRequest,
    ) -> Result<TaskStarted, ModImportTaskError> {
        self.start_task(request.archive_path)
    }

    pub fn start_import_mod_revision_task(
        &self,
        request: StartImportModRevisionTaskRequest,
    ) -> Result<TaskStarted, ModImportTaskError> {
        if request.mod_id.as_str().trim().is_empty() {
            return Err(ModImportTaskError::ModIdEmpty);
        }
        self.start_task(request.archive_path)
    }

    fn start_task(&self, archive_path: PathBuf) -> Result<TaskStarted, ModImportTaskError> {
        if archive_path.as_os_str().is_empty() {
            return Err(ModImportTaskError::ArchivePathEmpty);
        }
        if !archive_path.is_absolute() {
            return Err(ModImportTaskError::ArchivePathNotAbsolute);
        }
        if !archive_path.exists() {
            return Err(ModImportTaskError::ArchiveFileNotFound);
        }
        if !archive_path.is_file() {
            return Err(ModImportTaskError::ArchivePathIsNotFile);
        }

        let task = self
            .task_manager
            .create_task(TaskKind::ModImport)
            .map_err(ModImportTaskError::from)?;

        Ok(TaskStarted {
            task_id: task.task_id,
            kind: task.kind,
            status: task.status,
        })
    }
}

impl From<TaskManagerError> for ModImportTaskError {
    fn from(error: TaskManagerError) -> Self {
        match error {
            TaskManagerError::TaskIdGenerationFailed(message) => {
                Self::TaskIdGenerationFailed(message)
            }
            TaskManagerError::TaskNotFound(_) => Self::TaskRegistrationFailed(error.to_string()),
            TaskManagerError::TaskCannotBeCancelled { .. } => {
                Self::TaskRegistrationFailed(error.to_string())
            }
            TaskManagerError::TaskCannotTransition { .. } => {
                Self::TaskRegistrationFailed(error.to_string())
            }
            TaskManagerError::TaskScopeBusy { .. } => {
                Self::TaskRegistrationFailed(error.to_string())
            }
            TaskManagerError::TaskCreationBlocked { .. } => {
                Self::TaskRegistrationFailed(error.to_string())
            }
            TaskManagerError::TaskStoreUnavailable => {
                Self::TaskRegistrationFailed(error.to_string())
            }
        }
    }
}

impl Default for ModImportTaskService {
    fn default() -> Self {
        Self::new(Arc::new(TaskManager::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TaskKind, TaskStatus};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn start_import_task_returns_stable_task_id_for_existing_file() {
        let root = temp_root("mod-import-task-valid");
        fs::create_dir_all(&root).expect("create temp root");
        let archive_path = root.join("sample.zip");
        fs::write(&archive_path, b"not a real archive yet").expect("write sample file");

        let task_manager = std::sync::Arc::new(crate::TaskManager::new());
        let service = ModImportTaskService::new(std::sync::Arc::clone(&task_manager));
        let task = service
            .start_import_mod_task(StartImportModTaskRequest { archive_path })
            .expect("task starts");

        assert!(task.task_id.starts_with("mod-import-"));
        assert!(!task.task_id.contains("sample.zip"));
        assert!(!task.task_id.contains('\\'));
        assert!(!task.task_id.contains('/'));
        assert_eq!(task.kind, TaskKind::ModImport);
        assert_eq!(task.status, TaskStatus::Queued);
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(TaskStatus::Queued)
        );

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    fn start_import_task_rejects_relative_paths() {
        let service = ModImportTaskService::default();
        let error = service
            .start_import_mod_task(StartImportModTaskRequest {
                archive_path: PathBuf::from("relative.zip"),
            })
            .expect_err("relative path rejected");

        assert_eq!(error, ModImportTaskError::ArchivePathNotAbsolute);
    }

    #[test]
    fn start_import_task_rejects_missing_paths() {
        let service = ModImportTaskService::default();
        let error = service
            .start_import_mod_task(StartImportModTaskRequest {
                archive_path: temp_root("mod-import-task-missing").join("missing.zip"),
            })
            .expect_err("missing path rejected");

        assert_eq!(error, ModImportTaskError::ArchiveFileNotFound);
    }

    #[test]
    fn start_import_task_rejects_directories() {
        let root = temp_root("mod-import-task-directory");
        fs::create_dir_all(&root).expect("create temp root");

        let service = ModImportTaskService::default();
        let error = service
            .start_import_mod_task(StartImportModTaskRequest {
                archive_path: root.clone(),
            })
            .expect_err("directory path rejected");

        assert_eq!(error, ModImportTaskError::ArchivePathIsNotFile);

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    fn start_revision_import_task_accepts_explicit_logical_mod_id() {
        let root = temp_root("mod-revision-import-task-valid");
        fs::create_dir_all(&root).expect("create temp root");
        let archive_path = root.join("candidate.zip");
        fs::write(&archive_path, b"not a real archive yet").expect("write sample file");
        let task_manager = Arc::new(crate::TaskManager::new());
        let service = ModImportTaskService::new(Arc::clone(&task_manager));

        let task = service
            .start_import_mod_revision_task(StartImportModRevisionTaskRequest {
                archive_path,
                mod_id: ModId::new("mod-a"),
            })
            .expect("revision import task starts");

        assert_eq!(task.kind, TaskKind::ModImport);
        assert_eq!(task.status, TaskStatus::Queued);
        assert!(!task.task_id.contains("mod-a"));
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(TaskStatus::Queued)
        );
        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    fn start_revision_import_task_rejects_empty_logical_mod_id() {
        let service = ModImportTaskService::default();
        let error = service
            .start_import_mod_revision_task(StartImportModRevisionTaskRequest {
                archive_path: PathBuf::from("unused.zip"),
                mod_id: ModId::new("  "),
            })
            .expect_err("empty logical Mod id rejected before path access");

        assert_eq!(error, ModImportTaskError::ModIdEmpty);
        assert_eq!(error.error_code(), "mod_id_empty");
    }

    fn temp_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{suffix}"))
    }
}
