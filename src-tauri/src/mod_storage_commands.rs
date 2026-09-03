use crate::dto::CommandErrorDto;
use crate::state::AppState;
use hmm_app::{ModStorageDirectoryValidation, ModStorageSettingsError, ModStorageSettingsSnapshot};
use hmm_runtime::{ModStorageDegradedReason, ModStorageRootSource};
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;

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
    }
}
