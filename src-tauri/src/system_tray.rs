use tauri::{
    AppHandle, Manager, Runtime, Emitter,
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
};
use crate::events::navigation::{NAVIGATE_TO, SHOW_ABOUT};

/// Initialize the system tray with menu items
pub fn init_system_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    // Create the tray menu
    let quit_item = MenuItemBuilder::new("Quit")
        .id("quit")
        .build(app)?;

    let show_item = MenuItemBuilder::new("Show")
        .id("show")
        .build(app)?;

    let hide_item = MenuItemBuilder::new("Hide")
        .id("hide")
        .build(app)?;

    let _separator = PredefinedMenuItem::separator(app)?;

    let settings_item = MenuItemBuilder::new("Settings")
        .id("settings")
        .build(app)?;

    let about_item = MenuItemBuilder::new("About StratoSort")
        .id("about")
        .build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .item(&hide_item)
        .separator()
        .item(&settings_item)
        .separator()
        .item(&about_item)
        .separator()
        .item(&quit_item)
        .build()?;

    // Create the system tray
    let icon = app.default_window_icon()
        .ok_or_else(|| {
            tauri::Error::from(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Failed to get default window icon for system tray"
            ))
        })?;
        
    let _tray = TrayIconBuilder::new()
        .icon(icon.clone())
        .menu(&menu)
        .tooltip("StratoSort - AI File Organization")
        .on_menu_event(move |app, event| {
            match event.id.as_ref() {
                "quit" => {
                    app.exit(0);
                }
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }
                "hide" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }
                "settings" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                        // Emit event to navigate to settings
                        let _ = window.emit(NAVIGATE_TO, "settings");
                    }
                }
                "about" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                        // Emit event to show about dialog
                        let _ = window.emit(SHOW_ABOUT, ());
                    }
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    // Show/hide window on left click
                    if let Some(app) = tray.app_handle().get_webview_window("main") {
                        if app.is_visible().unwrap_or(false) {
                            let _ = app.hide();
                        } else {
                            let _ = app.show();
                            let _ = app.unminimize();
                            let _ = app.set_focus();
                        }
                    }
                }
                TrayIconEvent::Click {
                    button: MouseButton::Right,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    // Right click handled by menu
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

/// Update tray tooltip with status information
pub fn update_tray_tooltip<R: Runtime>(_app: &AppHandle<R>, _status: &str) {
    // This function would update the tray tooltip with current status
    // For now, we'll keep it simple
}