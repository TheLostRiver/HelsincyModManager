use crate::dto::CommandErrorDto;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Emitter, Manager, Window, WindowEvent};

pub const WINDOW_CLOSE_REQUESTED_EVENT: &str = "hmm://window-close-requested";
const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "hmm-main-tray";
const MENU_OPEN_ID: &str = "hmm-tray-open";
const MENU_EXIT_ID: &str = "hmm-tray-exit";

fn window_lifecycle_error(code: &'static str, message: impl Into<String>) -> CommandErrorDto {
    CommandErrorDto {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
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
            MENU_EXIT_ID => app.exit(0),
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
    window
        .hide()
        .map_err(|error| window_lifecycle_error("window_hide_failed", error.to_string()))
}

#[tauri::command]
pub fn exit_app(app: AppHandle) -> Result<(), CommandErrorDto> {
    app.exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
