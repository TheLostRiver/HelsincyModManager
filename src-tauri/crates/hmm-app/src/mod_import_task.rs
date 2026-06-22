use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartImportModTaskRequest {
    pub archive_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStarted {
    pub task_id: String,
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
    #[error("failed to generate task id: {0}")]
    TaskIdGenerationFailed(String),
}

impl ModImportTaskError {
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::ArchivePathEmpty => "archive_path_empty",
            Self::ArchivePathNotAbsolute => "archive_path_not_absolute",
            Self::ArchiveFileNotFound => "archive_file_not_found",
            Self::ArchivePathIsNotFile => "archive_path_is_not_file",
            Self::TaskIdGenerationFailed(_) => "task_id_generation_failed",
        }
    }
}

pub struct ModImportTaskService;

impl ModImportTaskService {
    pub fn new() -> Self {
        Self
    }

    pub fn start_import_mod_task(
        &self,
        request: StartImportModTaskRequest,
    ) -> Result<TaskStarted, ModImportTaskError> {
        let archive_path = request.archive_path;
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

        Ok(TaskStarted {
            task_id: generate_task_id()?,
        })
    }
}

impl Default for ModImportTaskService {
    fn default() -> Self {
        Self::new()
    }
}

fn generate_task_id() -> Result<String, ModImportTaskError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ModImportTaskError::TaskIdGenerationFailed(error.to_string()))?
        .as_millis();

    Ok(format!("mod-import-{millis}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn start_import_task_returns_stable_task_id_for_existing_file() {
        let root = temp_root("mod-import-task-valid");
        fs::create_dir_all(&root).expect("create temp root");
        let archive_path = root.join("sample.zip");
        fs::write(&archive_path, b"not a real archive yet").expect("write sample file");

        let service = ModImportTaskService::new();
        let task = service
            .start_import_mod_task(StartImportModTaskRequest { archive_path })
            .expect("task starts");

        assert!(task.task_id.starts_with("mod-import-"));
        assert!(!task.task_id.contains("sample.zip"));
        assert!(!task.task_id.contains('\\'));
        assert!(!task.task_id.contains('/'));

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    fn start_import_task_rejects_relative_paths() {
        let service = ModImportTaskService::new();
        let error = service
            .start_import_mod_task(StartImportModTaskRequest {
                archive_path: PathBuf::from("relative.zip"),
            })
            .expect_err("relative path rejected");

        assert_eq!(error, ModImportTaskError::ArchivePathNotAbsolute);
    }

    #[test]
    fn start_import_task_rejects_missing_paths() {
        let service = ModImportTaskService::new();
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

        let service = ModImportTaskService::new();
        let error = service
            .start_import_mod_task(StartImportModTaskRequest {
                archive_path: root.clone(),
            })
            .expect_err("directory path rejected");

        assert_eq!(error, ModImportTaskError::ArchivePathIsNotFile);

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    fn temp_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{suffix}"))
    }
}
