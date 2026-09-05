mod app_log;
mod background_worker;
mod batch_mod_lifecycle_commands;
mod batch_mod_lifecycle_dto;
mod category_commands;
mod debug_log_commands;
mod diagnostics_dto;
mod dto;
mod external_import_commands;
mod external_import_dto;
mod external_mod_adopt_commands;
mod external_state_commands;
mod external_state_dto;
mod game_launch_commands;
mod game_launch_dto;
mod game_setup_commands;
mod install_commands;
mod installer_cleanup;
mod log_storage_commands;
mod mod_deletion_commands;
mod mod_import_commands;
mod mod_import_settings_commands;
mod mod_library_commands;
mod mod_library_dto;
#[cfg(test)]
mod mod_library_read_model_benchmark_tests;
mod mod_metadata_commands;
mod mod_storage_commands;
mod package_contents_commands;
mod profile_commands;
mod reinstall_commands;
mod reinstall_dto;
mod replacement_commands;
mod replacement_dto;
mod save_backup_center_commands;
mod save_backup_center_dto;
mod save_backup_commands;
mod save_backup_dto;
mod save_directory_discovery_commands;
mod save_directory_discovery_dto;
mod save_restore_commands;
mod save_restore_dto;
mod state;
mod task_commands;
mod task_events;
mod thumbnail_protocol;
mod update_commands;
mod update_dto;
mod window_lifecycle_commands;

use category_commands::{
    create_category, delete_category, get_mod_categories, list_categories, set_mod_categories,
    update_category,
};
use debug_log_commands::{get_debug_log_settings, set_debug_log_settings};
use external_import_commands::{
    create_external_import_selection, get_external_import_batch_result,
    get_external_import_preview, list_external_import_batches, retry_external_import_batch,
    select_all_external_import_candidates, select_external_import_source,
    start_external_import_batch, start_external_import_scan, update_external_import_selection,
};
use external_mod_adopt_commands::start_external_mod_adopt;
use external_state_commands::{get_external_mod_state, start_external_mod_state_scan};
use game_launch_commands::launch_game;
use game_setup_commands::{
    auto_detect_game_directory, get_game_prerequisite_status, get_game_setup_status,
    save_game_directory, scan_game_candidates, validate_game_directory,
};
use install_commands::{
    get_install_manifest_status, preview_imported_mod_install_plan, preview_install_plan,
    preview_recovery_action, scan_install_recovery, start_install_task, start_recovery_action_task,
    start_uninstall_task,
};
use log_storage_commands::{get_log_storage_settings, set_log_storage_settings};
use mod_deletion_commands::{delete_mod_from_library, preview_mod_deletion};
use mod_import_commands::{
    export_audit_log_diagnostics, export_preview_image_diagnostics, export_support_diagnostics,
    get_diagnostics_page_snapshot, get_mod_dependency_graph, get_mod_detail,
    get_mod_detail_preview_image, get_mod_library, get_preview_image_candidates,
    get_preview_image_diagnostics, get_thumbnail_cache_settings, maintain_thumbnail_cache,
    select_preview_image_candidate, set_thumbnail_cache_settings, start_import_mod_revision_task,
    start_import_mod_task,
};
use mod_import_settings_commands::{get_mod_import_settings, set_mod_import_settings};
use mod_library_commands::query_mod_library;
use mod_metadata_commands::{delete_mod_metadata, update_mod_metadata};
use mod_storage_commands::{
    get_mod_storage_settings, set_mod_storage_dir, start_mod_storage_migration_task,
    validate_mod_storage_dir,
};
use package_contents_commands::{
    clear_mod_package_content_root, get_mod_package_contents, set_mod_package_content_root,
};
use profile_commands::{
    create_profile, delete_profile, get_active_profile, get_profile_save_settings, list_profiles,
    open_profile_directory, set_active_profile, set_profile_save_settings, update_profile,
    validate_profile_backup_directory, validate_profile_save_directory,
};
use reinstall_commands::{get_mod_revisions, preview_reinstall_plan, start_reinstall_task};
use replacement_commands::{
    analyze_imported_mod_replacement, list_replacement_target_occupancy, list_replacement_targets,
    preview_initial_retarget_install, preview_retarget_reinstall, start_retarget_install_task,
    start_retarget_reinstall_task,
};
use save_backup_center_commands::{
    query_save_backup_center, run_save_backup_retention, update_save_backup_note,
};
use save_backup_commands::{
    check_auto_save_backup, disable_save_backup_background_protection,
    enable_save_backup_background_protection, get_save_backup_background_control_status,
    get_save_backup_background_status, list_save_backups, start_save_backup_task,
};
use save_directory_discovery_commands::{
    confirm_profile_save_directory_candidate, discover_profile_save_directories,
};
use save_restore_commands::{preview_save_restore, start_save_restore_task};
use state::AppState;
use task_commands::cancel_task;
use tauri::{Manager, RunEvent, State};
use thumbnail_protocol::register_thumbnail_protocol;
use update_commands::check_app_update;
use window_lifecycle_commands::{
    exit_app, get_app_exit_guard, hide_main_window_to_tray, register_window_lifecycle,
    ExitAuthorizationStore,
};

pub use background_worker::BackgroundWorkerEntryError;
use batch_mod_lifecycle_commands::{
    get_batch_mod_lifecycle_capability, get_batch_mod_lifecycle_result,
    preview_batch_mod_lifecycle, retry_batch_mod_lifecycle, seal_batch_mod_lifecycle,
    start_batch_mod_lifecycle,
};

#[tauri::command]
fn app_health(health: State<'_, hmm_infra::AppLogHealth>) -> &'static str {
    app_log::status_code(&health)
}

pub fn run_save_backup_worker_once_from_env() -> Result<(), BackgroundWorkerEntryError> {
    background_worker::run_save_backup_worker_once_from_env()
}

pub fn run_installer_cleanup_from_env() -> i32 {
    installer_cleanup::run_installer_cleanup_from_env()
}

pub fn run() {
    let app = register_thumbnail_protocol(tauri::Builder::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_log_health = app_log::initialize(app.handle());
            app.manage(app_log_health);
            let state = AppState::new(app.handle()).inspect_err(|_| {
                app_log::record_state_initialization_failed();
            })?;
            app_log::record_state_initialized();
            app.manage(state);
            app.manage(ExitAuthorizationStore::default());
            register_window_lifecycle(app).inspect_err(|_| {
                app_log::record_warning(
                    "application.window_lifecycle_initialization_failed",
                    "window_lifecycle_initialization",
                    "window_lifecycle_initialization_failed",
                );
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_health,
            get_batch_mod_lifecycle_capability,
            preview_batch_mod_lifecycle,
            seal_batch_mod_lifecycle,
            start_batch_mod_lifecycle,
            get_batch_mod_lifecycle_result,
            retry_batch_mod_lifecycle,
            launch_game,
            get_game_prerequisite_status,
            get_game_setup_status,
            auto_detect_game_directory,
            validate_game_directory,
            save_game_directory,
            scan_game_candidates,
            cancel_task,
            select_external_import_source,
            start_external_import_scan,
            start_external_mod_state_scan,
            get_external_mod_state,
            start_external_mod_adopt,
            get_external_import_preview,
            create_external_import_selection,
            update_external_import_selection,
            select_all_external_import_candidates,
            start_external_import_batch,
            retry_external_import_batch,
            get_external_import_batch_result,
            list_external_import_batches,
            preview_install_plan,
            preview_imported_mod_install_plan,
            get_mod_package_contents,
            set_mod_package_content_root,
            clear_mod_package_content_root,
            start_install_task,
            start_uninstall_task,
            get_install_manifest_status,
            scan_install_recovery,
            preview_recovery_action,
            start_recovery_action_task,
            start_import_mod_task,
            start_import_mod_revision_task,
            get_mod_revisions,
            preview_reinstall_plan,
            start_reinstall_task,
            list_replacement_targets,
            analyze_imported_mod_replacement,
            list_replacement_target_occupancy,
            preview_mod_deletion,
            delete_mod_from_library,
            preview_initial_retarget_install,
            start_retarget_install_task,
            preview_retarget_reinstall,
            start_retarget_reinstall_task,
            get_mod_library,
            query_mod_library,
            get_mod_detail,
            get_mod_dependency_graph,
            get_mod_detail_preview_image,
            get_preview_image_diagnostics,
            export_preview_image_diagnostics,
            export_audit_log_diagnostics,
            export_support_diagnostics,
            get_diagnostics_page_snapshot,
            get_preview_image_candidates,
            select_preview_image_candidate,
            maintain_thumbnail_cache,
            get_thumbnail_cache_settings,
            set_thumbnail_cache_settings,
            get_log_storage_settings,
            set_log_storage_settings,
            get_debug_log_settings,
            set_debug_log_settings,
            get_mod_import_settings,
            set_mod_import_settings,
            get_mod_storage_settings,
            validate_mod_storage_dir,
            set_mod_storage_dir,
            start_mod_storage_migration_task,
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
            open_profile_directory,
            validate_profile_save_directory,
            validate_profile_backup_directory,
            set_profile_save_settings,
            discover_profile_save_directories,
            confirm_profile_save_directory_candidate,
            start_save_backup_task,
            check_auto_save_backup,
            get_save_backup_background_status,
            get_save_backup_background_control_status,
            enable_save_backup_background_protection,
            disable_save_backup_background_protection,
            list_save_backups,
            query_save_backup_center,
            update_save_backup_note,
            run_save_backup_retention,
            preview_save_restore,
            start_save_restore_task,
            hide_main_window_to_tray,
            get_app_exit_guard,
            exit_app,
            check_app_update
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Helsincy Mod Manager");

    let exit_code = app.run_return(|_, event| match event {
        RunEvent::ExitRequested { .. } => {
            app_log::record_application_lifecycle(app_log::ApplicationLifecycleStage::ExitRequested)
        }
        RunEvent::Exit => {
            app_log::record_application_lifecycle(app_log::ApplicationLifecycleStage::Exit)
        }
        _ => {}
    });
    app_log::record_application_lifecycle(app_log::ApplicationLifecycleStage::EventLoopReturned);
    std::process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::app_log;
    use hmm_infra::AppLogHealth;

    #[test]
    fn app_health_returns_logging_health_status() {
        assert_eq!(app_log::status_code(&AppLogHealth::ready()), "ok");
    }

    #[test]
    fn application_run_releases_tauri_state_before_final_process_exit() {
        let source = include_str!("lib.rs");
        let build = source.find(".build(tauri::generate_context!())").unwrap();
        let run_return = source.find("app.run_return(").unwrap();
        let event_loop_stopped = source
            .find("ApplicationLifecycleStage::EventLoopReturned")
            .unwrap();
        let final_exit = source.find("std::process::exit(exit_code)").unwrap();

        assert!(build < run_return);
        assert!(run_return < event_loop_stopped);
        assert!(event_loop_stopped < final_exit);
        let legacy_run = [".run(", "tauri::generate_context!())"].concat();
        assert!(!source.contains(&legacy_run));
    }
}
