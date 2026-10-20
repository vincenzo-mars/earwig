//! Comandi per la gestione della finestra e della modalità ridotta (edge drawer / mini bar).

use std::sync::{Arc, Mutex};
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, Position, Size, State, WebviewWindow};

pub struct SavedWindowState {
    pub is_compact: bool,
    pub is_hovered: bool,
    pub is_recording: bool,
    pub prev_x: f64,
    pub prev_y: f64,
    pub prev_width: f64,
    pub prev_height: f64,
    pub compact_y: Option<f64>,
}

impl Default for SavedWindowState {
    fn default() -> Self {
        Self {
            is_compact: false,
            is_hovered: false,
            is_recording: false,
            prev_x: 100.0,
            prev_y: 100.0,
            prev_width: 640.0,
            prev_height: 720.0,
            compact_y: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct WindowModeState(pub Arc<Mutex<SavedWindowState>>);

pub const COMPACT_HEIGHT: f64 = 48.0;
pub const COMPACT_WIDTH_IDLE: f64 = 18.0;
pub const COMPACT_WIDTH_EXPANDED: f64 = 86.0;

fn get_screen_bounds(window: &WebviewWindow) -> Result<(f64, f64, f64, f64), String> {
    let monitor = window
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Nessun monitor trovato".to_string())?;

    let scale = monitor.scale_factor();
    let mon_pos = monitor.position();
    let mon_size = monitor.size();

    let mon_x = mon_pos.x as f64 / scale;
    let mon_y = mon_pos.y as f64 / scale;
    let mon_w = mon_size.width as f64 / scale;
    let mon_h = mon_size.height as f64 / scale;

    let right_edge = mon_x + mon_w;
    let min_y = mon_y + 35.0; // evita menu bar macOS in alto
    let max_y = mon_y + mon_h - COMPACT_HEIGHT - 35.0; // evita dock inferiore
    let default_center_y = mon_y + (mon_h / 2.0) - (COMPACT_HEIGHT / 2.0);

    Ok((right_edge, min_y, max_y, default_center_y))
}

#[tauri::command]
pub async fn set_compact_mode(
    window: WebviewWindow,
    state: State<'_, WindowModeState>,
    enabled: bool,
) -> Result<(), String> {
    let mut lock = state.0.lock().map_err(|e| e.to_string())?;

    if enabled {
        if lock.is_compact {
            return Ok(());
        }

        // Salva posizione e dimensione attuali
        if let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) {
            if let Ok(scale) = window.scale_factor() {
                lock.prev_x = pos.x as f64 / scale;
                lock.prev_y = pos.y as f64 / scale;
                lock.prev_width = size.width as f64 / scale;
                lock.prev_height = size.height as f64 / scale;
            }
        }

        let (right_edge, min_y, max_y, default_center_y) = get_screen_bounds(&window)?;
        let width = COMPACT_WIDTH_IDLE;
        let height = COMPACT_HEIGHT;
        let x = right_edge - width;
        let y = lock.compact_y.unwrap_or(default_center_y).clamp(min_y, max_y);
        lock.compact_y = Some(y);

        lock.is_compact = true;
        lock.is_hovered = false;
        drop(lock);

        // Configura la finestra per la modalità ridotta
        let _ = window.set_min_size(None::<Size>);
        let _ = window.set_always_on_top(true);
        let _ = window.set_decorations(false);
        let _ = window.set_shadow(false);
        let _ = window.set_resizable(false);

        let _ = window.set_size(Size::Logical(LogicalSize::new(width, height)));
        let _ = window.set_position(Position::Logical(LogicalPosition::new(x, y)));
        let _ = window.emit("compact-stage", "idle");
    } else {
        if !lock.is_compact {
            return Ok(());
        }

        let prev_x = lock.prev_x;
        let prev_y = lock.prev_y;
        let prev_w = lock.prev_width.max(620.0);
        let prev_h = lock.prev_height.max(700.0);

        lock.is_compact = false;
        lock.is_hovered = false;

        let _ = window.set_always_on_top(false);
        let _ = window.set_decorations(true);
        let _ = window.set_shadow(true);
        let _ = window.set_resizable(true);
        let _ = window.set_min_size(Some(Size::Logical(LogicalSize::new(600.0, 680.0))));

        let _ = window.set_size(Size::Logical(LogicalSize::new(prev_w, prev_h)));
        let _ = window.set_position(Position::Logical(LogicalPosition::new(prev_x, prev_y)));
    }

    Ok(())
}

#[tauri::command]
pub async fn set_compact_hover(
    window: WebviewWindow,
    state: State<'_, WindowModeState>,
    hovered: bool,
    is_recording: bool,
) -> Result<(), String> {
    let mut lock = state.0.lock().map_err(|e| e.to_string())?;
    if !lock.is_compact {
        return Ok(());
    }

    lock.is_hovered = hovered;
    lock.is_recording = is_recording;

    let (right_edge, min_y, max_y, default_center_y) = get_screen_bounds(&window)?;
    let target_width = if hovered {
        COMPACT_WIDTH_EXPANDED
    } else {
        COMPACT_WIDTH_IDLE
    };

    let height = COMPACT_HEIGHT;
    let target_x = right_edge - target_width;
    let target_y = lock.compact_y.unwrap_or(default_center_y).clamp(min_y, max_y);

    if hovered {
        let _ = window.set_position(Position::Logical(LogicalPosition::new(target_x, target_y)));
        let _ = window.set_size(Size::Logical(LogicalSize::new(target_width, height)));
        let _ = window.emit("compact-stage", "expanded");
    } else {
        let _ = window.set_size(Size::Logical(LogicalSize::new(target_width, height)));
        let _ = window.set_position(Position::Logical(LogicalPosition::new(target_x, target_y)));
        let _ = window.emit("compact-stage", "idle");
    }

    Ok(())
}

#[tauri::command]
pub async fn set_compact_y(
    window: WebviewWindow,
    state: State<'_, WindowModeState>,
    y: f64,
) -> Result<(), String> {
    let mut lock = state.0.lock().map_err(|e| e.to_string())?;
    if !lock.is_compact {
        return Ok(());
    }

    let (right_edge, min_y, max_y, _) = get_screen_bounds(&window)?;
    let clamped_y = y.clamp(min_y, max_y);
    lock.compact_y = Some(clamped_y);

    let scale = window.scale_factor().unwrap_or(1.0);
    let cur_w = window
        .outer_size()
        .map(|s| s.width as f64 / scale)
        .unwrap_or(COMPACT_WIDTH_IDLE);
    let x = right_edge - cur_w;

    let _ = window.set_position(Position::Logical(LogicalPosition::new(x, clamped_y)));
    Ok(())
}

#[tauri::command]
pub async fn get_compact_position(window: WebviewWindow) -> Result<(f64, f64), String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    Ok((pos.x as f64 / scale, pos.y as f64 / scale))
}

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
