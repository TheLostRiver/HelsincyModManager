use hmm_infra::{emit_safe_app_log, initialize_app_logging, AppLogEvent, AppLogHealth};
use tauri::{AppHandle, Manager};

pub fn initialize(app_handle: &AppHandle) -> AppLogHealth {
    let health = match app_handle.path().app_data_dir() {
        Ok(app_data_dir) => initialize_app_logging(&app_data_dir),
        Err(_) => AppLogHealth::initialization_failed(),
    };
    if health.status_code() == "ok" {
        emit_safe_app_log(AppLogEvent::info("application.started").with_result("success"));
    }
    health
}

pub fn record_state_initialized() {
    emit_safe_app_log(
        AppLogEvent::info("application.state_initialized")
            .with_operation("configuration_database")
            .with_result("success"),
    );
}

pub fn record_state_initialization_failed() {
    record_warning(
        "application.state_initialization_failed",
        "configuration_database",
        "app_state_initialization_failed",
    );
}

pub fn record_warning(event_name: &'static str, operation: &'static str, error_code: &'static str) {
    emit_safe_app_log(
        AppLogEvent::warning(event_name)
            .with_operation(operation)
            .with_result("failed")
            .with_error_code(error_code),
    );
}

pub fn status_code(health: &AppLogHealth) -> &'static str {
    health.status_code()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_health_status_uses_stable_logging_codes() {
        assert_eq!(status_code(&AppLogHealth::ready()), "ok");
        assert_eq!(
            status_code(&AppLogHealth::initialization_failed()),
            "app_log_initialization_failed"
        );
    }
}
