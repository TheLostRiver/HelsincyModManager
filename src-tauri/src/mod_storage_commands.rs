use crate::dto::{CommandErrorDto, TaskStartedDto};
use crate::state::AppState;
use crate::task_events::{emit_task_progress, TauriTaskProgressObserver};
use hmm_app::{
    queued_mod_storage_migration_event, ModStorageDirectoryValidation, ModStorageMigrationLaunch,
    ModStorageMigrationTaskError, ModStorageMigrationTaskService, ModStorageSettingsError,
    ModStorageSettingsSnapshot, ModStorageWriteFreeze, TaskManager, TaskProgressEvent, TaskStatus,
    MOD_STORAGE_MIGRATION_CANCELLED_PHASE, MOD_STORAGE_MIGRATION_FAILED_PHASE,
};
use hmm_runtime::{ModStorageDegradedReason, ModStorageRootSource};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};

/// #275: what the settings page shows. The directories are the user's own choices (or the
/// default below app-data), the same class of path `get_game_setup_status` already returns;
/// package sandboxes, caches and other internal paths are never part of this DTO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModStorageSettingsDto {
    pub effective_dir: String,
    pub default_dir: String,
    pub configured_dir: Option<String>,
    pub source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degraded_detail: Option<&'static str>,
    pub library_empty: bool,
    pub restart_required: bool,
    /// `none` | `migration` | `restart_required` — why import / delete are refused right now.
    pub writes_frozen: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModStorageDirValidationDto {
    pub ok: bool,
    pub code: Option<&'static str>,
    pub exists: bool,
    pub claimed: bool,
}

pub(crate) fn settings_to_dto(
    snapshot: ModStorageSettingsSnapshot,
    source: ModStorageRootSource,
    degraded: Option<ModStorageDegradedReason>,
) -> ModStorageSettingsDto {
    ModStorageSettingsDto {
        effective_dir: snapshot.effective_root.to_string_lossy().into_owned(),
        default_dir: snapshot.default_root.to_string_lossy().into_owned(),
        configured_dir: snapshot
            .configured
            .map(|path| path.to_string_lossy().into_owned()),
        source: source.as_str(),
        degraded_reason: degraded.map(ModStorageDegradedReason::code),
        degraded_detail: degraded.and_then(ModStorageDegradedReason::detail_code),
        library_empty: snapshot.library_empty,
        restart_required: snapshot.restart_required,
        writes_frozen: writes_frozen_code(snapshot.writes_frozen),
    }
}

fn writes_frozen_code(freeze: ModStorageWriteFreeze) -> &'static str {
    match freeze {
        ModStorageWriteFreeze::None => "none",
        ModStorageWriteFreeze::Migration => "migration",
        ModStorageWriteFreeze::RestartRequired => "restart_required",
    }
}

impl From<ModStorageDirectoryValidation> for ModStorageDirValidationDto {
    fn from(validation: ModStorageDirectoryValidation) -> Self {
        Self {
            ok: validation.ok,
            code: validation.code,
            exists: validation.exists,
            claimed: validation.claimed,
        }
    }
}

impl CommandErrorDto {
    pub fn from_mod_storage_settings_error(error: ModStorageSettingsError) -> Self {
        Self {
            code: error.code().to_owned(),
            message: error.to_string(),
        }
    }
}

#[tauri::command]
pub fn get_mod_storage_settings(
    state: State<'_, AppState>,
) -> Result<ModStorageSettingsDto, CommandErrorDto> {
    state
        .mod_storage_settings
        .get()
        .map(|snapshot| {
            settings_to_dto(
                snapshot,
                state.mod_storage.source,
                state.mod_storage.degraded,
            )
        })
        .map_err(CommandErrorDto::from_mod_storage_settings_error)
}

#[tauri::command]
pub fn validate_mod_storage_dir(
    directory: String,
    state: State<'_, AppState>,
) -> Result<ModStorageDirValidationDto, CommandErrorDto> {
    let directory = parse_directory(directory)?;
    state
        .mod_storage_settings
        .validate(&directory)
        .map(Into::into)
        .map_err(CommandErrorDto::from_mod_storage_settings_error)
}

/// `directory: null` restores the default root. Either way the change applies after a restart
/// and is only accepted while the library is empty (`mod_storage_migration_required` otherwise).
#[tauri::command]
pub fn set_mod_storage_dir(
    directory: Option<String>,
    state: State<'_, AppState>,
) -> Result<ModStorageSettingsDto, CommandErrorDto> {
    let directory = directory.map(parse_directory).transpose()?;
    state
        .mod_storage_settings
        .set(directory)
        .map(|snapshot| {
            settings_to_dto(
                snapshot,
                state.mod_storage.source,
                state.mod_storage.degraded,
            )
        })
        .map_err(CommandErrorDto::from_mod_storage_settings_error)
}

/// #275 slice 2: moves the library to `directory` (`null` = back to the default root). Returns
/// the queued task; progress and the terminal outcome arrive as `hmm://task-progress` events
/// with the `mod_storage.migration.*` phases. Sandbox writes stay refused until restart once
/// the migration switched the setting.
#[tauri::command]
pub fn start_mod_storage_migration_task(
    directory: Option<String>,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<TaskStartedDto, CommandErrorDto> {
    let directory = directory.map(parse_directory).transpose()?;
    let launch = state
        .mod_storage_migration_tasks
        .start(directory)
        .map_err(mod_storage_migration_task_error)?;
    let response = TaskStartedDto::from(launch.task.clone());

    if let Err(error) = emit_task_progress(&app_handle, queued_mod_storage_migration_event(&launch))
    {
        // No queued event means the frontend never learns the task id; closing the launch also
        // reopens the write gate the start froze.
        let _ = state.mod_storage_migration_tasks.abort_queued(&launch);
        return Err(error);
    }
    spawn_mod_storage_migration_runner(
        Arc::clone(&state.mod_storage_migration_tasks),
        Arc::clone(&state.task_manager),
        app_handle,
        launch,
    );
    Ok(response)
}

fn spawn_mod_storage_migration_runner(
    service: Arc<ModStorageMigrationTaskService>,
    task_manager: Arc<TaskManager>,
    app_handle: AppHandle,
    launch: ModStorageMigrationLaunch,
) {
    std::thread::spawn(move || {
        let task = launch.task.clone();
        let observer = TauriTaskProgressObserver::new(&app_handle);
        // Live events go out through the observer; only a broken runner needs the fallback.
        if let Err(error) = service.run_with_observer(launch, &observer) {
            let _ = emit_task_progress(
                &app_handle,
                fallback_migration_terminal_event(&task_manager, task, error),
            );
        }
    });
}

/// The runner itself failed (task registry refused a transition): the task must not stay
/// queued/running, and the frontend needs a terminal event with a stable code.
fn fallback_migration_terminal_event(
    task_manager: &TaskManager,
    task: hmm_app::TaskStarted,
    error: ModStorageMigrationTaskError,
) -> TaskProgressEvent {
    if matches!(
        task_manager.task_status(&task.task_id),
        Some(TaskStatus::Queued | TaskStatus::Running)
    ) {
        let _ = task_manager.fail_task(&task.task_id);
    }
    let status = task_manager
        .task_status(&task.task_id)
        .unwrap_or(TaskStatus::Failed);
    let mut event = TaskProgressEvent::new(
        task.task_id,
        task.kind,
        status,
        if status == TaskStatus::Cancelled {
            MOD_STORAGE_MIGRATION_CANCELLED_PHASE
        } else {
            MOD_STORAGE_MIGRATION_FAILED_PHASE
        },
    );
    if status != TaskStatus::Cancelled {
        event.error = Some(error.code().to_owned());
    }
    event
}

fn mod_storage_migration_task_error(error: ModStorageMigrationTaskError) -> CommandErrorDto {
    CommandErrorDto {
        code: error.code().to_owned(),
        message: error.to_string(),
    }
}

/// Only the trivially cheap shape rule lives here; everything else (links, markers, overlap,
/// write probe) is the inspector's job so the CLI and GUI cannot drift apart.
fn parse_directory(value: String) -> Result<PathBuf, CommandErrorDto> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CommandErrorDto {
            code: hmm_ports::ModStorageDirectoryError::NotAbsolute
                .code()
                .to_owned(),
            message: "directory cannot be empty".to_owned(),
        });
    }
    Ok(PathBuf::from(trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_ports::ModStorageDirectoryError;

    fn snapshot(configured: Option<&str>, restart_required: bool) -> ModStorageSettingsSnapshot {
        ModStorageSettingsSnapshot {
            effective_root: PathBuf::from("C:/app-data/mod-import"),
            default_root: PathBuf::from("C:/app-data/mod-import"),
            configured: configured.map(PathBuf::from),
            library_empty: true,
            restart_required,
            writes_frozen: if restart_required {
                ModStorageWriteFreeze::RestartRequired
            } else {
                ModStorageWriteFreeze::None
            },
        }
    }

    #[test]
    fn settings_dto_omits_degradation_keys_when_healthy() {
        let dto = settings_to_dto(snapshot(None, false), ModStorageRootSource::Default, None);
        let value = serde_json::to_value(&dto).expect("serialize");

        assert_eq!(value["effectiveDir"], "C:/app-data/mod-import");
        assert_eq!(value["defaultDir"], "C:/app-data/mod-import");
        assert_eq!(value["configuredDir"], serde_json::Value::Null);
        assert_eq!(value["source"], "default");
        assert_eq!(value["libraryEmpty"], true);
        assert_eq!(value["restartRequired"], false);
        assert_eq!(value["writesFrozen"], "none");
        assert!(value.get("degradedReason").is_none());
        assert!(value.get("degradedDetail").is_none());
    }

    #[test]
    fn settings_dto_carries_degradation_codes_without_paths() {
        let dto = settings_to_dto(
            snapshot(Some("E:/HMMMods"), true),
            ModStorageRootSource::Configured,
            Some(ModStorageDegradedReason::ConfiguredDirUnavailable(
                ModStorageDirectoryError::MarkerRequired,
            )),
        );
        let value = serde_json::to_value(&dto).expect("serialize");

        assert_eq!(value["configuredDir"], "E:/HMMMods");
        assert_eq!(value["source"], "configured");
        assert_eq!(value["degradedReason"], "configured_dir_unavailable");
        assert_eq!(value["degradedDetail"], "mod_storage_dir_marker_required");
        assert_eq!(value["restartRequired"], true);
        assert_eq!(value["writesFrozen"], "restart_required");
        assert_eq!(
            writes_frozen_code(ModStorageWriteFreeze::Migration),
            "migration"
        );
    }

    #[test]
    fn validation_dto_keeps_stable_codes() {
        let dto: ModStorageDirValidationDto = ModStorageDirectoryValidation {
            ok: false,
            code: Some(ModStorageDirectoryError::OverlapsGameRoot.code()),
            exists: false,
            claimed: false,
        }
        .into();
        let value = serde_json::to_value(&dto).expect("serialize");

        assert_eq!(value["ok"], false);
        assert_eq!(value["code"], "mod_storage_dir_overlaps_game_root");
        assert_eq!(value["exists"], false);
        assert_eq!(value["claimed"], false);
    }

    #[test]
    fn empty_directory_input_is_rejected_with_a_stable_code() {
        let error = parse_directory("   ".to_owned()).expect_err("empty input");
        assert_eq!(error.code, "mod_storage_dir_not_absolute");
        assert_eq!(
            parse_directory(" D:/HMMMods ".to_owned()).expect("trimmed"),
            PathBuf::from("D:/HMMMods")
        );
    }

    #[test]
    fn service_errors_map_to_their_stable_codes() {
        let migration = CommandErrorDto::from_mod_storage_settings_error(
            ModStorageSettingsError::MigrationRequired,
        );
        assert_eq!(migration.code, "mod_storage_migration_required");
        let directory = CommandErrorDto::from_mod_storage_settings_error(
            ModStorageSettingsError::Directory(ModStorageDirectoryError::NotWritable),
        );
        assert_eq!(directory.code, "mod_storage_dir_not_writable");
        let frozen =
            CommandErrorDto::from_mod_storage_settings_error(ModStorageSettingsError::WriteFrozen(
                hmm_app::ModStorageWriteGateError::MigrationInProgress,
            ));
        assert_eq!(frozen.code, "mod_storage_migration_in_progress");
        let imports_active =
            mod_storage_migration_task_error(ModStorageMigrationTaskError::ImportsActive);
        assert_eq!(imports_active.code, "mod_storage_migration_imports_active");
        assert!(!imports_active.message.is_empty());
    }

    #[test]
    fn fallback_terminal_event_fails_a_running_migration_task_with_a_stable_code() {
        let task_manager = TaskManager::new();
        let task = task_manager
            .create_task(hmm_app::TaskKind::ModStorageMigration)
            .expect("task");
        task_manager.start_task(&task.task_id).expect("start");
        let started = hmm_app::TaskStarted {
            task_id: task.task_id.clone(),
            kind: task.kind,
            status: task.status,
        };

        let event = fallback_migration_terminal_event(
            &task_manager,
            started.clone(),
            ModStorageMigrationTaskError::TaskUnavailable,
        );

        assert_eq!(event.status, TaskStatus::Failed);
        assert_eq!(event.phase, MOD_STORAGE_MIGRATION_FAILED_PHASE);
        assert_eq!(
            event.error.as_deref(),
            Some("mod_storage_migration_task_unavailable")
        );
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(TaskStatus::Failed)
        );

        let cancelled = task_manager
            .create_task(hmm_app::TaskKind::ModStorageMigration)
            .expect("task");
        task_manager
            .cancel_task(&cancelled.task_id)
            .expect("cancel queued");
        let event = fallback_migration_terminal_event(
            &task_manager,
            hmm_app::TaskStarted {
                task_id: cancelled.task_id,
                kind: cancelled.kind,
                status: cancelled.status,
            },
            ModStorageMigrationTaskError::TaskUnavailable,
        );
        assert_eq!(event.status, TaskStatus::Cancelled);
        assert_eq!(event.phase, MOD_STORAGE_MIGRATION_CANCELLED_PHASE);
        assert_eq!(event.error, None);
        let value =
            serde_json::to_value(crate::dto::TaskProgressEventDto::from(event)).expect("serialize");
        assert_eq!(value["kind"], "mod_storage_migration");
    }
}
