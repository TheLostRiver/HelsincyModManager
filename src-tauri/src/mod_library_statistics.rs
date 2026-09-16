use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

pub(crate) fn start_refresh(app: &AppHandle) {
    let service = Arc::clone(&app.state::<crate::state::AppState>().mod_library_statistics);
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || match service.refresh_missing_sizes() {
        Ok(0) => {}
        Ok(_) => {
            let _ = app.emit("mod-library-statistics-updated", ());
        }
        Err(_) => crate::app_log::record_warning(
            "mod_library.statistics_refresh_failed",
            "mod_library_statistics",
            "statistics_unavailable",
        ),
    });
}
