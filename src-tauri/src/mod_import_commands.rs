use crate::dto::{CommandErrorDto, TaskStartedDto};
use crate::state::AppState;
use hmm_app::StartImportModTaskRequest;
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub fn start_import_mod_task(
    archive_path: String,
    state: State<'_, AppState>,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let archive_path = parse_archive_path(archive_path)?;

    state
        .mod_import_tasks
        .start_import_mod_task(StartImportModTaskRequest { archive_path })
        .map(Into::into)
        .map_err(CommandErrorDto::from_mod_import_task_error)
}

fn parse_archive_path(value: String) -> Result<PathBuf, CommandErrorDto> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(CommandErrorDto {
            code: "archive_path_empty".to_owned(),
            message: "archive path cannot be empty".to_owned(),
        });
    }

    let archive_path = PathBuf::from(trimmed);
    if !archive_path.is_absolute() {
        return Err(CommandErrorDto {
            code: "archive_path_not_absolute".to_owned(),
            message: "archive path must be an absolute path".to_owned(),
        });
    }

    Ok(archive_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_archive_path_rejects_empty_paths() {
        let error = parse_archive_path("  ".to_owned()).expect_err("empty path rejected");

        assert_eq!(error.code, "archive_path_empty");
    }

    #[test]
    fn parse_archive_path_rejects_relative_paths() {
        let error =
            parse_archive_path("mods/sample.zip".to_owned()).expect_err("relative path rejected");

        assert_eq!(error.code, "archive_path_not_absolute");
    }
}
