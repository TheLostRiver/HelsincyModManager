fn main() {
    if let Err(error) = hmm_tauri::run_save_backup_worker_once_from_env() {
        eprintln!("{}", error.code());
        std::process::exit(1);
    }
}
