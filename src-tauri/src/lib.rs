mod dto;
mod game_setup_commands;
mod install_commands;
mod mod_import_commands;
mod state;
mod task_commands;
mod task_events;
mod thumbnail_protocol;

use game_setup_commands::{
    get_game_setup_status, save_game_directory, scan_game_candidates, validate_game_directory,
};
use install_commands::{
    get_install_manifest_status, preview_imported_mod_install_plan, preview_install_plan,
    start_install_task,
};
use mod_import_commands::{
    export_audit_log_diagnostics, export_preview_image_diagnostics, export_support_diagnostics,
    get_mod_dependency_graph, get_mod_detail, get_mod_detail_preview_image, get_mod_library,
    get_preview_image_candidates, get_preview_image_diagnostics, get_thumbnail_cache_settings,
    maintain_thumbnail_cache, select_preview_image_candidate, set_thumbnail_cache_settings,
    start_import_mod_task,
};
use state::AppState;
use task_commands::cancel_task;
use tauri::Manager;
use thumbnail_protocol::register_thumbnail_protocol;

#[tauri::command]
fn app_health() -> &'static str {
    "ok"
}

pub fn run() {
    register_thumbnail_protocol(tauri::Builder::default())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = AppState::new(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_health,
            get_game_setup_status,
            validate_game_directory,
            save_game_directory,
            scan_game_candidates,
            cancel_task,
            preview_install_plan,
            preview_imported_mod_install_plan,
            start_install_task,
            get_install_manifest_status,
            start_import_mod_task,
            get_mod_library,
            get_mod_detail,
            get_mod_dependency_graph,
            get_mod_detail_preview_image,
            get_preview_image_diagnostics,
            export_preview_image_diagnostics,
            export_audit_log_diagnostics,
            export_support_diagnostics,
            get_preview_image_candidates,
            select_preview_image_candidate,
            maintain_thumbnail_cache,
            get_thumbnail_cache_settings,
            set_thumbnail_cache_settings
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Helsincy Mod Manager");
}

#[cfg(test)]
mod tests {
    use super::app_health;

    #[test]
    fn app_health_returns_ok() {
        assert_eq!(app_health(), "ok");
    }
}
