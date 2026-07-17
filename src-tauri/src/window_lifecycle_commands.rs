use crate::dto::CommandErrorDto;
use crate::state::AppState;
use hmm_app::{SaveBackupExitDecision, SaveBackupExitGuard, SaveBackupExitReason};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Emitter, Manager, State, Window, WindowEvent};

pub const WINDOW_CLOSE_REQUESTED_EVENT: &str = "hmm://window-close-requested";
pub const TRAY_EXIT_REQUEST_EVENT: &str = WINDOW_CLOSE_REQUESTED_EVENT;
const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "hmm-main-tray";
const MENU_OPEN_ID: &str = "hmm-tray-open";
const MENU_EXIT_ID: &str = "hmm-tray-exit";

type ExitGuardEvaluation = Result<SaveBackupExitDecision, ()>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExitAppRequestDto {
    pub override_unprotected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppExitGuardDto {
    pub decision: AppExitDecisionDto,
    pub reason: Option<AppExitReasonDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppExitDecisionDto {
    Safe,
    ConfirmationRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppExitReasonDto {
    BackgroundStarting,
    BackgroundNotEnabled,
    RegistrationFailed,
    WorkerUnhealthy,
    PermissionRequired,
    UnsupportedPlatform,
    StatusUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExitAppAction {
    Exit,
    ConfirmationRequired,
    ExitWithOverrideAudit { reason: SaveBackupExitReason },
}

impl AppExitGuardDto {
    fn from_evaluation(evaluation: ExitGuardEvaluation) -> Self {
        match fail_closed_exit_decision(evaluation) {
            SaveBackupExitDecision::Safe => Self {
                decision: AppExitDecisionDto::Safe,
                reason: None,
            },
            SaveBackupExitDecision::ConfirmationRequired { reason } => {
                Self::confirmation_required(reason)
            }
        }
    }

    fn confirmation_required(reason: SaveBackupExitReason) -> Self {
        Self {
            decision: AppExitDecisionDto::ConfirmationRequired,
            reason: Some(reason.into()),
        }
    }
}

impl From<SaveBackupExitReason> for AppExitReasonDto {
    fn from(reason: SaveBackupExitReason) -> Self {
        match reason {
            SaveBackupExitReason::BackgroundStarting => Self::BackgroundStarting,
            SaveBackupExitReason::BackgroundNotEnabled => Self::BackgroundNotEnabled,
            SaveBackupExitReason::RegistrationFailed => Self::RegistrationFailed,
            SaveBackupExitReason::WorkerUnhealthy => Self::WorkerUnhealthy,
            SaveBackupExitReason::PermissionRequired => Self::PermissionRequired,
            SaveBackupExitReason::UnsupportedPlatform => Self::UnsupportedPlatform,
            SaveBackupExitReason::StatusUnavailable => Self::StatusUnavailable,
        }
    }
}

fn window_lifecycle_error(code: &'static str, message: impl Into<String>) -> CommandErrorDto {
    CommandErrorDto {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        if window.show().is_err() {
            crate::app_log::record_warning("window.operation_failed", "show", "window_show_failed");
        }
        if window.unminimize().is_err() {
            crate::app_log::record_warning(
                "window.operation_failed",
                "unminimize",
                "window_unminimize_failed",
            );
        }
        if window.set_focus().is_err() {
            crate::app_log::record_warning(
                "window.operation_failed",
                "focus",
                "window_focus_failed",
            );
        }
    }
}

fn request_exit_from_tray(app: &AppHandle) {
    show_main_window(app);
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.emit(TRAY_EXIT_REQUEST_EVENT, ());
    }
}

pub fn register_window_lifecycle(app: &mut App) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, MENU_OPEN_ID, "打开 Helsincy", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let exit_item = MenuItem::with_id(app, MENU_EXIT_ID, "退出程序", true, None::<&str>)?;
    let tray_menu = Menu::with_items(app, &[&open_item, &separator, &exit_item])?;

    let mut tray_builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Helsincy Mod Manager")
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_OPEN_ID => show_main_window(app),
            MENU_EXIT_ID => request_exit_from_tray(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray_builder = tray_builder.icon(icon);
    }

    let tray_icon = tray_builder.build(app)?;
    app.manage(tray_icon);

    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let close_event_window = window.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = close_event_window.emit(WINDOW_CLOSE_REQUESTED_EVENT, ());
            }
        });
    }

    Ok(())
}

#[tauri::command]
pub fn hide_main_window_to_tray(window: Window) -> Result<(), CommandErrorDto> {
    window.hide().map_err(|_| {
        crate::app_log::record_warning("window.operation_failed", "hide", "window_hide_failed");
        window_lifecycle_error("window_hide_failed", "failed to hide main window")
    })
}

#[tauri::command]
pub async fn get_app_exit_guard(
    state: State<'_, AppState>,
) -> Result<AppExitGuardDto, CommandErrorDto> {
    let guard = Arc::clone(&state.save_backup_exit_guard);
    Ok(AppExitGuardDto::from_evaluation(
        evaluate_exit_guard(guard).await,
    ))
}

#[tauri::command]
pub async fn exit_app(
    request: ExitAppRequestDto,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), CommandErrorDto> {
    let guard = Arc::clone(&state.save_backup_exit_guard);
    let action = resolve_exit_action(
        evaluate_exit_guard(Arc::clone(&guard)).await,
        request.override_unprotected,
    );
    match action {
        ExitAppAction::ConfirmationRequired => {
            return Err(exit_confirmation_required_error());
        }
        ExitAppAction::ExitWithOverrideAudit { reason } => {
            let audit_result =
                tauri::async_runtime::spawn_blocking(move || guard.record_override(reason)).await;
            if !matches!(audit_result, Ok(Ok(()))) {
                crate::app_log::record_warning(
                    "audit.write_failed",
                    "exit_override",
                    "exit_override_audit_unavailable",
                );
            }
        }
        ExitAppAction::Exit => {}
    }
    app.exit(0);
    Ok(())
}

async fn evaluate_exit_guard(guard: Arc<SaveBackupExitGuard>) -> ExitGuardEvaluation {
    match tauri::async_runtime::spawn_blocking(move || guard.evaluate()).await {
        Ok(Ok(decision)) => Ok(decision),
        Ok(Err(_)) | Err(_) => Err(()),
    }
}

fn fail_closed_exit_decision(evaluation: ExitGuardEvaluation) -> SaveBackupExitDecision {
    evaluation.unwrap_or(SaveBackupExitDecision::ConfirmationRequired {
        reason: SaveBackupExitReason::StatusUnavailable,
    })
}

fn resolve_exit_action(
    evaluation: ExitGuardEvaluation,
    override_unprotected: bool,
) -> ExitAppAction {
    match fail_closed_exit_decision(evaluation) {
        SaveBackupExitDecision::Safe => ExitAppAction::Exit,
        SaveBackupExitDecision::ConfirmationRequired { .. } if !override_unprotected => {
            ExitAppAction::ConfirmationRequired
        }
        SaveBackupExitDecision::ConfirmationRequired { reason } => {
            ExitAppAction::ExitWithOverrideAudit { reason }
        }
    }
}

fn exit_confirmation_required_error() -> CommandErrorDto {
    window_lifecycle_error("exit_confirmation_required", "exit requires confirmation")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_request_deserializes_explicit_override_flag_and_rejects_extra_fields() {
        let request: ExitAppRequestDto = serde_json::from_value(serde_json::json!({
            "overrideUnprotected": false
        }))
        .expect("deserialize explicit exit request");

        assert!(!request.override_unprotected);
        assert!(serde_json::from_value::<ExitAppRequestDto>(serde_json::json!({})).is_err());
        assert!(
            serde_json::from_value::<ExitAppRequestDto>(serde_json::json!({
                "overrideUnprotected": true,
                "path": "C:/Users/Alice/save"
            }))
            .is_err()
        );
    }

    #[test]
    fn exit_guard_dto_serializes_safe_and_stable_reasons_without_raw_details() {
        let safe = serde_json::to_value(AppExitGuardDto::from_evaluation(Ok(
            SaveBackupExitDecision::Safe,
        )))
        .expect("serialize safe exit guard");
        assert_eq!(
            safe,
            serde_json::json!({
                "decision": "safe",
                "reason": null
            })
        );

        let unavailable = serde_json::to_value(AppExitGuardDto::from_evaluation(Err(())))
            .expect("serialize unavailable exit guard");
        assert_eq!(unavailable["decision"], "confirmation_required");
        assert_eq!(unavailable["reason"], "status_unavailable");

        let cases = [
            (
                hmm_app::SaveBackupExitReason::BackgroundStarting,
                "background_starting",
            ),
            (
                hmm_app::SaveBackupExitReason::BackgroundNotEnabled,
                "background_not_enabled",
            ),
            (
                hmm_app::SaveBackupExitReason::RegistrationFailed,
                "registration_failed",
            ),
            (
                hmm_app::SaveBackupExitReason::WorkerUnhealthy,
                "worker_unhealthy",
            ),
            (
                hmm_app::SaveBackupExitReason::PermissionRequired,
                "permission_required",
            ),
            (
                hmm_app::SaveBackupExitReason::UnsupportedPlatform,
                "unsupported_platform",
            ),
            (
                hmm_app::SaveBackupExitReason::StatusUnavailable,
                "status_unavailable",
            ),
        ];

        for (reason, expected) in cases {
            let dto = AppExitGuardDto::confirmation_required(reason);
            let value = serde_json::to_value(dto).expect("serialize exit guard");

            assert_eq!(value["decision"], "confirmation_required");
            assert_eq!(value["reason"], expected);
            for forbidden in [
                "path",
                "profileIds",
                "taskName",
                "sid",
                "workerId",
                "leaseOwner",
                "errorDetails",
            ] {
                assert!(value.get(forbidden).is_none());
            }
        }
    }

    #[test]
    fn exit_action_covers_safe_confirmation_override_and_unavailable() {
        assert_eq!(
            resolve_exit_action(Ok(hmm_app::SaveBackupExitDecision::Safe), false),
            ExitAppAction::Exit
        );
        assert_eq!(
            resolve_exit_action(Ok(hmm_app::SaveBackupExitDecision::Safe), true),
            ExitAppAction::Exit
        );

        let unsafe_decision = hmm_app::SaveBackupExitDecision::ConfirmationRequired {
            reason: hmm_app::SaveBackupExitReason::BackgroundStarting,
        };
        assert_eq!(
            resolve_exit_action(Ok(unsafe_decision.clone()), false),
            ExitAppAction::ConfirmationRequired
        );
        assert_eq!(
            resolve_exit_action(Ok(unsafe_decision), true),
            ExitAppAction::ExitWithOverrideAudit {
                reason: hmm_app::SaveBackupExitReason::BackgroundStarting,
            }
        );
        assert_eq!(
            resolve_exit_action(Err(()), false),
            ExitAppAction::ConfirmationRequired
        );
        assert_eq!(
            resolve_exit_action(Err(()), true),
            ExitAppAction::ExitWithOverrideAudit {
                reason: hmm_app::SaveBackupExitReason::StatusUnavailable,
            }
        );
    }

    #[test]
    fn exit_confirmation_error_is_stable_and_sanitized() {
        let error = exit_confirmation_required_error();

        assert_eq!(error.code, "exit_confirmation_required");
        assert_eq!(error.message, "exit requires confirmation");
        assert!(!error.message.contains("C:/Users"));
        assert!(!error.message.contains("S-1-5-21"));
    }

    #[test]
    fn tray_exit_uses_the_same_window_close_event() {
        assert_eq!(TRAY_EXIT_REQUEST_EVENT, WINDOW_CLOSE_REQUESTED_EVENT);
    }

    #[test]
    fn exit_guard_command_is_exposed() {
        let _ = get_app_exit_guard;
    }

    #[test]
    fn window_lifecycle_event_and_menu_ids_are_stable() {
        assert_eq!(WINDOW_CLOSE_REQUESTED_EVENT, "hmm://window-close-requested");
        assert_eq!(MAIN_WINDOW_LABEL, "main");
        assert_eq!(TRAY_ID, "hmm-main-tray");
        assert_eq!(MENU_OPEN_ID, "hmm-tray-open");
        assert_eq!(MENU_EXIT_ID, "hmm-tray-exit");
    }

    #[test]
    fn window_lifecycle_error_uses_stable_code_without_paths() {
        let dto = window_lifecycle_error("window_hide_failed", "hide failed");

        assert_eq!(dto.code, "window_hide_failed");
        assert_eq!(dto.message, "hide failed");
        assert!(!dto.message.contains("C:/"));
    }
}
