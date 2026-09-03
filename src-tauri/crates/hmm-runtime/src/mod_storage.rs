//! Resolves the Mod storage root (the directory holding `sandboxes/`) once per process.
//!
//! #275: the root is either the historical default `<app-data>/mod-import` or a directory the
//! user configured in `settings.json`. Resolution never silently falls back to the default when
//! a configured directory is merely unavailable (unplugged drive, deleted marker): doing so
//! would make new imports land in the default location while the library lives elsewhere, and
//! "packages split across two roots" is exactly the state this feature must never create.
//! Instead the configured root stays in effect and every sandbox access fails closed with the
//! existing "sandbox unavailable" codes until the directory is back or the user re-selects one.

use hmm_infra::default_mod_storage_root;
use hmm_ports::{
    validate_mod_storage_directory_shape, AppSettings, AppSettingsRepositoryError,
    ModStorageDirectoryError, ModStorageDirectoryInspector,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModStorageRootSource {
    Default,
    Configured,
}

impl ModStorageRootSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Configured => "configured",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModStorageDegradedReason {
    /// `settings.json` could not be read; the configured value is unknown, so the default root is
    /// used. Imports continue there — the user-visible warning is the only mitigation.
    SettingsUnreadable,
    /// The persisted value fails the pure shape rules (relative path, `..`, file system root).
    /// Such a value cannot be used as a root at all, so the default is used.
    ConfiguredDirInvalid(ModStorageDirectoryError),
    /// The persisted value is well-formed but the directory is not usable right now. The
    /// configured root stays in effect; see the module documentation.
    ConfiguredDirUnavailable(ModStorageDirectoryError),
}

impl ModStorageDegradedReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::SettingsUnreadable => "settings_unreadable",
            Self::ConfiguredDirInvalid(_) => "configured_dir_invalid",
            Self::ConfiguredDirUnavailable(_) => "configured_dir_unavailable",
        }
    }

    pub const fn detail_code(self) -> Option<&'static str> {
        match self {
            Self::SettingsUnreadable => None,
            Self::ConfiguredDirInvalid(error) | Self::ConfiguredDirUnavailable(error) => {
                Some(error.code())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModStorageRootResolution {
    /// Effective storage root for this process.
    pub root: PathBuf,
    pub default_root: PathBuf,
    /// The persisted setting as written, even when it could not be honoured.
    pub configured: Option<PathBuf>,
    pub source: ModStorageRootSource,
    pub degraded: Option<ModStorageDegradedReason>,
}

impl ModStorageRootResolution {
    pub fn sandbox_root(&self) -> PathBuf {
        self.root.join(hmm_ports::MOD_STORAGE_SANDBOX_DIRECTORY)
    }
}

pub fn resolve_mod_storage_root(
    app_data_dir: &Path,
    settings: Result<&AppSettings, &AppSettingsRepositoryError>,
    inspector: &dyn ModStorageDirectoryInspector,
) -> ModStorageRootResolution {
    let default_root = default_mod_storage_root(app_data_dir);
    let configured = match settings {
        Ok(settings) => settings.mod_storage_dir.clone(),
        Err(_) => {
            return ModStorageRootResolution {
                root: default_root.clone(),
                default_root,
                configured: None,
                source: ModStorageRootSource::Default,
                degraded: Some(ModStorageDegradedReason::SettingsUnreadable),
            };
        }
    };
    let Some(configured_dir) = configured else {
        return ModStorageRootResolution {
            root: default_root.clone(),
            default_root,
            configured: None,
            source: ModStorageRootSource::Default,
            degraded: None,
        };
    };
    if let Err(error) = validate_mod_storage_directory_shape(&configured_dir) {
        return ModStorageRootResolution {
            root: default_root.clone(),
            default_root,
            configured: Some(configured_dir),
            source: ModStorageRootSource::Default,
            degraded: Some(ModStorageDegradedReason::ConfiguredDirInvalid(error)),
        };
    }
    let degraded = inspector
        .verify_claimed(&configured_dir)
        .err()
        .map(ModStorageDegradedReason::ConfiguredDirUnavailable);
    ModStorageRootResolution {
        root: configured_dir.clone(),
        default_root,
        configured: Some(configured_dir),
        source: ModStorageRootSource::Configured,
        degraded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_infra::FileSystemModStorageDirectoryInspector;
    use std::fs;

    fn settings_with(dir: Option<PathBuf>) -> AppSettings {
        AppSettings {
            mod_storage_dir: dir,
            ..AppSettings::default()
        }
    }

    #[test]
    fn unreadable_settings_fall_back_to_the_default_root_and_report_it() {
        let temp = tempfile::tempdir().expect("temp");
        let app_data = temp.path().join("app-data");

        let resolution = resolve_mod_storage_root(
            &app_data,
            Err(&AppSettingsRepositoryError::StorageCorrupted),
            &FileSystemModStorageDirectoryInspector,
        );

        assert_eq!(resolution.root, app_data.join("mod-import"));
        assert_eq!(resolution.source, ModStorageRootSource::Default);
        assert_eq!(
            resolution.degraded,
            Some(ModStorageDegradedReason::SettingsUnreadable)
        );
        assert_eq!(resolution.configured, None);
        assert_eq!(
            resolution.sandbox_root(),
            app_data.join("mod-import").join("sandboxes")
        );
    }

    #[test]
    fn absent_setting_uses_the_default_root_without_degradation() {
        let temp = tempfile::tempdir().expect("temp");
        let app_data = temp.path().join("app-data");

        let resolution = resolve_mod_storage_root(
            &app_data,
            Ok(&settings_with(None)),
            &FileSystemModStorageDirectoryInspector,
        );

        assert_eq!(resolution.root, app_data.join("mod-import"));
        assert_eq!(resolution.default_root, app_data.join("mod-import"));
        assert_eq!(resolution.source, ModStorageRootSource::Default);
        assert_eq!(resolution.degraded, None);
    }

    #[test]
    fn claimed_configured_directory_becomes_the_root() {
        let temp = tempfile::tempdir().expect("temp");
        let app_data = temp.path().join("app-data");
        let configured = temp.path().join("HMMMods");
        FileSystemModStorageDirectoryInspector
            .claim(&configured)
            .expect("claim");

        let resolution = resolve_mod_storage_root(
            &app_data,
            Ok(&settings_with(Some(configured.clone()))),
            &FileSystemModStorageDirectoryInspector,
        );

        assert_eq!(resolution.root, configured);
        assert_eq!(resolution.configured, Some(configured.clone()));
        assert_eq!(resolution.source, ModStorageRootSource::Configured);
        assert_eq!(resolution.degraded, None);
        assert_eq!(resolution.sandbox_root(), configured.join("sandboxes"));
    }

    #[test]
    fn malformed_configured_value_falls_back_to_default_but_is_still_reported() {
        let temp = tempfile::tempdir().expect("temp");
        let app_data = temp.path().join("app-data");
        let relative = PathBuf::from("relative/mods");

        let resolution = resolve_mod_storage_root(
            &app_data,
            Ok(&settings_with(Some(relative.clone()))),
            &FileSystemModStorageDirectoryInspector,
        );

        assert_eq!(resolution.root, app_data.join("mod-import"));
        assert_eq!(resolution.source, ModStorageRootSource::Default);
        assert_eq!(resolution.configured, Some(relative));
        assert_eq!(
            resolution.degraded,
            Some(ModStorageDegradedReason::ConfiguredDirInvalid(
                ModStorageDirectoryError::NotAbsolute
            ))
        );
    }

    #[test]
    fn unavailable_configured_directory_stays_the_root_and_is_reported() {
        let temp = tempfile::tempdir().expect("temp");
        let app_data = temp.path().join("app-data");
        let unplugged = temp.path().join("unplugged-drive").join("HMMMods");

        let resolution = resolve_mod_storage_root(
            &app_data,
            Ok(&settings_with(Some(unplugged.clone()))),
            &FileSystemModStorageDirectoryInspector,
        );

        assert_eq!(
            resolution.root, unplugged,
            "an unavailable root must not be swapped for the default"
        );
        assert_eq!(resolution.source, ModStorageRootSource::Configured);
        assert_eq!(
            resolution.degraded,
            Some(ModStorageDegradedReason::ConfiguredDirUnavailable(
                ModStorageDirectoryError::Unavailable
            ))
        );
    }

    #[test]
    fn foreign_configured_directory_is_reported_as_unavailable_with_marker_code() {
        let temp = tempfile::tempdir().expect("temp");
        let app_data = temp.path().join("app-data");
        let foreign = temp.path().join("Pictures");
        fs::create_dir(&foreign).expect("foreign");
        fs::write(foreign.join("photo.jpg"), b"not ours").expect("foreign file");

        let resolution = resolve_mod_storage_root(
            &app_data,
            Ok(&settings_with(Some(foreign.clone()))),
            &FileSystemModStorageDirectoryInspector,
        );

        assert_eq!(resolution.root, foreign);
        let degraded = resolution.degraded.expect("degraded");
        assert_eq!(degraded.code(), "configured_dir_unavailable");
        assert_eq!(
            degraded.detail_code(),
            Some("mod_storage_dir_marker_required")
        );
    }
}
