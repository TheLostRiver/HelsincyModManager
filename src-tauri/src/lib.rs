mod category_commands;
mod dto;
mod game_launch_commands;
mod game_launch_dto;
mod game_setup_commands;
mod install_commands;
mod mod_import_commands;
mod mod_metadata_commands;
mod profile_commands;
mod state;
mod task_commands;
mod task_events;
mod thumbnail_protocol;

use category_commands::{
    create_category, delete_category, get_mod_categories, list_categories, set_mod_categories,
    update_category,
};
use game_launch_commands::launch_game;
use game_setup_commands::{
    get_game_setup_status, save_game_directory, scan_game_candidates, validate_game_directory,
};
use install_commands::{
    get_install_manifest_status, preview_imported_mod_install_plan, preview_install_plan,
    preview_recovery_action, scan_install_recovery, start_install_task, start_recovery_action_task,
    start_uninstall_task,
};
use mod_import_commands::{
    export_audit_log_diagnostics, export_preview_image_diagnostics, export_support_diagnostics,
    get_mod_dependency_graph, get_mod_detail, get_mod_detail_preview_image, get_mod_library,
    get_preview_image_candidates, get_preview_image_diagnostics, get_thumbnail_cache_settings,
    maintain_thumbnail_cache, select_preview_image_candidate, set_thumbnail_cache_settings,
    start_import_mod_task,
};
use mod_metadata_commands::{delete_mod_metadata, update_mod_metadata};
use profile_commands::{
    create_profile, delete_profile, get_active_profile, get_profile_save_settings, list_profiles,
    set_active_profile, set_profile_save_settings, update_profile,
    validate_profile_backup_directory, validate_profile_save_directory,
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
            launch_game,
            get_game_setup_status,
            validate_game_directory,
            save_game_directory,
            scan_game_candidates,
            cancel_task,
            preview_install_plan,
            preview_imported_mod_install_plan,
            start_install_task,
            start_uninstall_task,
            get_install_manifest_status,
            scan_install_recovery,
            preview_recovery_action,
            start_recovery_action_task,
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
            set_thumbnail_cache_settings,
            update_mod_metadata,
            delete_mod_metadata,
            create_category,
            update_category,
            delete_category,
            list_categories,
            set_mod_categories,
            get_mod_categories,
            list_profiles,
            get_active_profile,
            create_profile,
            update_profile,
            delete_profile,
            set_active_profile,
            get_profile_save_settings,
            validate_profile_save_directory,
            validate_profile_backup_directory,
            set_profile_save_settings
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
