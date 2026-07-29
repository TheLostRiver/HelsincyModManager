use hmm_runtime::HmmRuntime;
use std::ops::Deref;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub(crate) use hmm_runtime::{ConfiguredReinstallExecutor, ConfiguredRetargetReinstallError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppStateStartup {
    #[cfg(test)]
    Headless,
    Gui,
}

pub struct AppState {
    runtime: HmmRuntime,
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
        let state = Self {
            runtime: HmmRuntime::from_app_data_dir(app_data_dir)?,
        };
        run_state_startup(startup, &state);
        Ok(state)
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
