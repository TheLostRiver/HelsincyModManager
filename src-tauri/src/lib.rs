mod dto;
mod game_setup_commands;
mod mod_import_commands;
mod state;
mod task_commands;
mod task_events;
mod thumbnail_protocol;

use game_setup_commands::{
    get_game_setup_status, save_game_directory, scan_game_candidates, validate_game_directory,
};
use mod_import_commands::{get_mod_detail, get_mod_library, start_import_mod_task};
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
            start_import_mod_task,
            get_mod_library,
            get_mod_detail
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
