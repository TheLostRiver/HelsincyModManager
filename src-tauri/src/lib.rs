mod dto;
mod game_setup_commands;
mod state;

use game_setup_commands::{
    get_game_setup_status, save_game_directory, scan_game_candidates, validate_game_directory,
};
use state::AppState;
use tauri::Manager;

#[tauri::command]
fn app_health() -> &'static str {
    "ok"
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = AppState::new(&app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_health,
            get_game_setup_status,
            validate_game_directory,
            save_game_directory,
            scan_game_candidates
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
