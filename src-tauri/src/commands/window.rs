//! Comandi per la gestione della finestra e della visibilità nel Dock.

use tauri::{Manager, WebviewWindow};

/// Imposta la visibilità nel Dock di macOS cambiando la NSApplicationActivationPolicy.
/// In modalità Regular l'app è visibile nel Dock e nello switcher Cmd+Tab.
/// In modalità Accessory l'app vive solo nella menu bar / tray e non compare nel Dock.
#[cfg(target_os = "macos")]
pub fn set_dock_visible(app: &tauri::AppHandle, visible: bool) {
    let _ = app.run_on_main_thread(move || {
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        use objc2_foundation::MainThreadMarker;

        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            let policy = if visible {
                NSApplicationActivationPolicy::Regular
            } else {
                NSApplicationActivationPolicy::Accessory
            };
            let _ = app.setActivationPolicy(policy);
            if visible {
                #[allow(deprecated)]
                app.activateIgnoringOtherApps(true);
            }
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub fn set_dock_visible(_app: &tauri::AppHandle, _visible: bool) {}

#[tauri::command]
pub async fn show_dashboard(window: WebviewWindow) -> Result<(), String> {
    set_dock_visible(window.app_handle(), true);
    window.show().map_err(|e| e.to_string())?;
    window.unminimize().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn hide_dashboard(window: WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())?;
    set_dock_visible(window.app_handle(), false);
    Ok(())
}
