#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

// Installers consume the exit code; this helper must not create a console window.
fn main() {
    std::process::exit(hmm_tauri::run_installer_cleanup_from_env());
}
