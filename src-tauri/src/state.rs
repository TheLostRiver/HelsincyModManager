use crate::app_log;
use hmm_runtime::{HmmRuntime, RuntimeEnvironment};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

pub(crate) use hmm_runtime::{ConfiguredReinstallExecutor, ConfiguredRetargetReinstallError};

/// Environment variable that points the GUI at a disposable Sandbox data root. Batch mod
/// lifecycle commands are only available when this is set to a valid absolute directory;
/// Production writes remain rejected by the runtime's own sandbox gate.
pub(crate) const HMM_SANDBOX_DATA_DIR_ENV: &str = "HMM_SANDBOX_DATA_DIR";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppStateStartup {
    #[cfg(test)]
    Headless,
    Gui,
}

pub struct AppState {
    runtime: HmmRuntime,
    batch_sandbox: Option<BatchSandboxHandle>,
}

/// Shared handle for the batch mod lifecycle automation. It only carries the validated Sandbox
/// `RuntimeEnvironment`; the automation itself is stateless and builds short-lived write
/// contexts per command.
pub(crate) struct BatchSandboxHandle {
    environment: RuntimeEnvironment,
    in_process_database: Option<Arc<Mutex<rusqlite::Connection>>>,
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
        let sandbox_environment = resolve_sandbox_environment();
        let mut runtime_builder = HmmRuntime::builder(app_data_dir.clone());
        if let Some(environment) = sandbox_environment.clone() {
            runtime_builder = runtime_builder.with_sandbox_environment(environment)?;
        }
        let runtime = runtime_builder.build()?;
        let batch_sandbox = sandbox_environment.map(|environment| BatchSandboxHandle {
            in_process_database: environment
                .sandbox_data_dir()
                .is_some_and(|sandbox_root| same_existing_directory(sandbox_root, &app_data_dir))
                .then(|| runtime.database_handle()),
            environment,
        });
        let state = Self {
            runtime,
            batch_sandbox,
        };
        run_state_startup(startup, &state);
        Ok(state)
    }

    /// Returns the validated Sandbox environment used by batch mod lifecycle commands, or
    /// `None` when the GUI is not running against a Sandbox data root.
    pub fn batch_sandbox_environment(&self) -> Option<&RuntimeEnvironment> {
        self.batch_sandbox
            .as_ref()
            .map(|handle| &handle.environment)
    }

    /// Returns the GUI-owned database connection only when the batch root is the same app-data
    /// root. A differently configured batch root must keep the existing fail-closed snapshot
    /// behavior instead of accidentally journaling into the GUI database.
    pub fn batch_sandbox_database(&self) -> Option<Arc<Mutex<rusqlite::Connection>>> {
        self.batch_sandbox
            .as_ref()
            .and_then(|handle| handle.in_process_database.clone())
    }
}

fn resolve_sandbox_environment() -> Option<RuntimeEnvironment> {
    let Ok(value) = std::env::var(HMM_SANDBOX_DATA_DIR_ENV) else {
        return None;
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    match RuntimeEnvironment::sandbox(PathBuf::from(trimmed)) {
        Ok(environment) => Some(environment),
        Err(error) => {
            app_log::record_warning(
                error.code(),
                "batch_sandbox_environment",
                "batch_sandbox_environment_invalid",
            );
            None
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
