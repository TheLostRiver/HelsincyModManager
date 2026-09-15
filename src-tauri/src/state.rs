use crate::app_log;
use hmm_runtime::{HmmRuntime, RuntimeEnvironment, RuntimeEnvironmentKind};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

pub(crate) use hmm_runtime::{ConfiguredReinstallExecutor, ConfiguredRetargetReinstallError};

/// Optional disposable Sandbox environment. Without it, batch lifecycle uses the same
/// system app-data root as the desktop runtime.
pub(crate) const HMM_SANDBOX_DATA_DIR_ENV: &str = "HMM_SANDBOX_DATA_DIR";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppStateStartup {
    #[cfg(test)]
    Headless,
    Gui,
}

pub struct AppState {
    runtime: HmmRuntime,
    batch_environment: Result<RuntimeEnvironment, &'static str>,
}

impl AppState {
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|error| format!("failed to resolve app data dir: {error}"))?;
        Self::from_gui_app_data_dir(app_data_dir)
    }

    #[cfg(test)]
    pub fn from_app_data_dir(app_data_dir: PathBuf) -> Result<Self, String> {
        Self::from_app_data_dir_with_startup(app_data_dir, AppStateStartup::Headless)
    }

    fn from_gui_app_data_dir(app_data_dir: PathBuf) -> Result<Self, String> {
        Self::from_app_data_dir_with_startup(app_data_dir, AppStateStartup::Gui)
    }

    fn from_app_data_dir_with_startup(
        app_data_dir: PathBuf,
        startup: AppStateStartup,
    ) -> Result<Self, String> {
        let sandbox_environment = resolve_sandbox_environment()?;
        let mut runtime_builder = HmmRuntime::builder(app_data_dir.clone());
        if let Some(environment) = sandbox_environment.clone() {
            runtime_builder = runtime_builder.with_sandbox_environment(environment)?;
        }
        let runtime = runtime_builder.build()?;
        let batch_environment = batch_environment_for_app_data_dir(
            &app_data_dir,
            sandbox_environment,
            hmm_runtime::production_app_data_dir().as_deref(),
        );
        let state = Self {
            runtime,
            batch_environment,
        };
        run_state_startup(startup, &state);
        Ok(state)
    }

    /// Batch commands may share the GUI database only after both data roots match.
    pub fn batch_lifecycle_environment(&self) -> Result<&RuntimeEnvironment, &'static str> {
        self.batch_environment.as_ref().map_err(|code| *code)
    }
}

fn batch_environment_for_app_data_dir(
    app_data_dir: &Path,
    sandbox_environment: Option<RuntimeEnvironment>,
    production_root: Option<&Path>,
) -> Result<RuntimeEnvironment, &'static str> {
    let environment = match sandbox_environment {
        Some(environment) => environment,
        None => RuntimeEnvironment::from_options(RuntimeEnvironmentKind::Production, None)
            .map_err(|_| "batch_runtime_unavailable")?,
    };
    let batch_root = environment
        .sandbox_data_dir()
        .or(production_root)
        .ok_or("batch_runtime_unavailable")?;
    if !same_existing_directory(batch_root, app_data_dir) {
        return Err("batch_data_root_mismatch");
    }
    Ok(environment)
}

fn resolve_sandbox_environment() -> Result<Option<RuntimeEnvironment>, String> {
    let value = match std::env::var(HMM_SANDBOX_DATA_DIR_ENV) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err("sandbox_data_dir_invalid".to_owned());
        }
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    match RuntimeEnvironment::sandbox(PathBuf::from(trimmed)) {
        Ok(environment) => Ok(Some(environment)),
        Err(error) => {
            app_log::record_warning(
                error.code(),
                "batch_sandbox_environment",
                "batch_sandbox_environment_invalid",
            );
            // An explicitly invalid Sandbox must never fall back to Production writes.
            Err(error.code().to_owned())
        }
    }
}

fn same_existing_directory(left: &std::path::Path, right: &std::path::Path) -> bool {
    let (Ok(left), Ok(right)) = (left.canonicalize(), right.canonicalize()) else {
        return false;
    };

    if cfg!(any(target_os = "windows", target_os = "macos")) {
        let left = left.to_string_lossy().replace('\\', "/");
        let right = right.to_string_lossy().replace('\\', "/");
        left.eq_ignore_ascii_case(&right)
    } else {
        left == right
    }
}

impl Deref for AppState {
    type Target = HmmRuntime;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

#[cfg(test)]
type StateStartupObserver = Box<dyn Fn(AppStateStartup)>;

#[cfg(test)]
thread_local! {
    static STATE_STARTUP_OBSERVER: std::cell::RefCell<Option<StateStartupObserver>> =
        const { std::cell::RefCell::new(None) };
}

fn run_state_startup(startup: AppStateStartup, state: &AppState) {
    #[cfg(test)]
    if STATE_STARTUP_OBSERVER.with(|observer| {
        let observer = observer.borrow();
        observer
            .as_ref()
            .map(|observer| observer(startup))
            .is_some()
    }) {
        return;
    }

    if matches!(startup, AppStateStartup::Gui) {
        state.start_thumbnail_cache_maintenance();
    }
}

#[cfg(test)]
fn with_state_startup_observer<R>(
    observer: impl Fn(AppStateStartup) + 'static,
    action: impl FnOnce() -> R,
) -> R {
    STATE_STARTUP_OBSERVER.with(|active_observer| {
        let previous = active_observer.replace(Some(Box::new(observer)));
        let result = action();
        active_observer.replace(previous);
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn batch_environment_uses_production_only_for_the_gui_data_root() {
        let temp = tempfile::tempdir().expect("temporary desktop roots");
        let gui_root = temp.path().join("gui");
        let other_root = temp.path().join("other");
        std::fs::create_dir_all(&gui_root).unwrap();
        std::fs::create_dir_all(&other_root).unwrap();
        let environment =
            batch_environment_for_app_data_dir(&gui_root, None, Some(&gui_root.join(".")))
                .expect("matching desktop and system root enables production batch");
        assert_eq!(environment.kind(), RuntimeEnvironmentKind::Production);
        assert!(environment.sandbox_data_dir().is_none());
        assert_eq!(
            batch_environment_for_app_data_dir(&gui_root, None, Some(&other_root)),
            Err("batch_data_root_mismatch"),
        );
        assert_eq!(
            batch_environment_for_app_data_dir(&gui_root, None, None),
            Err("batch_runtime_unavailable"),
        );
    }

    #[test]
    fn explicit_sandbox_must_match_gui_root_and_never_falls_back_to_production() {
        let gui = tempfile::tempdir().expect("GUI root");
        let other = tempfile::tempdir().expect("other sandbox root");
        let sandbox = RuntimeEnvironment::sandbox(gui.path().to_path_buf()).unwrap();
        assert_eq!(
            batch_environment_for_app_data_dir(gui.path(), Some(sandbox.clone()), None),
            Ok(sandbox),
        );
        let mismatch = RuntimeEnvironment::sandbox(other.path().to_path_buf()).unwrap();
        assert_eq!(
            batch_environment_for_app_data_dir(gui.path(), Some(mismatch), Some(gui.path())),
            Err("batch_data_root_mismatch"),
        );
    }

    #[test]
    fn same_existing_directory_accepts_aliases_and_rejects_other_roots() {
        let parent =
            std::env::temp_dir().join(format!("hmm-state-root-identity-{}", uuid::Uuid::new_v4()));
        let app_data_dir = parent.join("app-data");
        let other_dir = parent.join("other");
        std::fs::create_dir_all(&app_data_dir).expect("create app data directory");
        std::fs::create_dir_all(&other_dir).expect("create other directory");

        assert!(same_existing_directory(
            &app_data_dir.join("."),
            &app_data_dir
        ));
        assert!(!same_existing_directory(&other_dir, &app_data_dir));
        assert!(!same_existing_directory(
            &parent.join("missing"),
            &app_data_dir
        ));

        std::fs::remove_dir_all(parent).expect("remove temporary root identity directory");
    }

    #[test]
    fn public_headless_entry_selects_headless_startup() {
        let app_data_dir = std::env::temp_dir().join(format!(
            "hmm-headless-state-composition-{}",
            uuid::Uuid::new_v4()
        ));
        let selected_startup = Arc::new(Mutex::new(Vec::new()));
        let selected_startup_for_observer = Arc::clone(&selected_startup);

        with_state_startup_observer(
            move |startup| {
                selected_startup_for_observer
                    .lock()
                    .expect("startup observer lock")
                    .push(startup);
            },
            || {
                AppState::from_app_data_dir(app_data_dir.clone())
                    .expect("headless state composition succeeds");
            },
        );

        assert_eq!(
            selected_startup
                .lock()
                .expect("startup observer lock")
                .as_slice(),
            [AppStateStartup::Headless]
        );
        std::fs::remove_dir_all(app_data_dir).expect("remove temporary app data directory");
    }

    #[test]
    fn gui_app_data_entry_selects_gui_startup_once() {
        let app_data_dir = std::env::temp_dir().join(format!(
            "hmm-gui-state-composition-{}",
            uuid::Uuid::new_v4()
        ));
        let selected_startup = Arc::new(Mutex::new(Vec::new()));
        let selected_startup_for_observer = Arc::clone(&selected_startup);

        with_state_startup_observer(
            move |startup| {
                selected_startup_for_observer
                    .lock()
                    .expect("startup observer lock")
                    .push(startup);
            },
            || {
                AppState::from_gui_app_data_dir(app_data_dir.clone())
                    .expect("GUI state composition succeeds");
            },
        );

        assert_eq!(
            selected_startup
                .lock()
                .expect("startup observer lock")
                .as_slice(),
            [AppStateStartup::Gui]
        );
        std::fs::remove_dir_all(app_data_dir).expect("remove temporary app data directory");
    }
}
