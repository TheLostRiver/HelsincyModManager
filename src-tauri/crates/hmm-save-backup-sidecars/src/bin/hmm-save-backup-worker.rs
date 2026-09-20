#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

// Task Scheduler launches this binary directly; debug builds must stay headless too.
fn main() {
    if let Err(error) = hmm_tauri::run_save_backup_worker_once_from_env() {
        eprintln!("{}", error.code());
        std::process::exit(1);
    }
}
