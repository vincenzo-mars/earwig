mod commands;
mod permissions;
mod recorder;

use commands::llm::LlamaServerState;
use commands::stt::SttEngineState;
use recorder::RecorderState;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .manage(RecorderState::new())
        .manage(SttEngineState::new())
        .manage(LlamaServerState::new())
        .manage(commands::window::WindowModeState::default())
        .setup(|app| {
            // Menu contestuale per la Tray icon
            let settings_item =
                MenuItem::with_id(app, "settings", "Impostazioni...", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Esci da Earwig", true, None::<&str>)?;

            let tray_menu = MenuBuilder::new(app)
                .item(&settings_item)
                .separator()
                .item(&quit_item)
                .build()?;

            let tray_icon = Image::from_bytes(include_bytes!("../icons/tray-icon.png"))
                .map_err(|e| e.to_string())?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .icon_as_template(true)
                .tooltip("Earwig")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "settings" => {
                        if let Some(window) = app.get_webview_window("main") {
                            commands::window::set_dock_visible(app, true);
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                            let _ = window.emit("open-settings", ());
                        }
                    }
                    "quit" => {
                        commands::server::kill_tracked(
                            &app.state::<LlamaServerState>().0,
                            &commands::server_lock_path(app, commands::llm::LLM_LOCK),
                        );
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let is_visible = window.is_visible().unwrap_or(false);
                            let is_minimized = window.is_minimized().unwrap_or(false);
                            if is_visible && !is_minimized {
                                let _ = window.hide();
                                commands::window::set_dock_visible(app, false);
                            } else {
                                commands::window::set_dock_visible(app, true);
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Gestione eventi finestra principale (chiusura e minimizzazione a tray)
            if let Some(window) = app.get_webview_window("main") {
                let w_clone = window.clone();
                window.on_window_event(move |event| match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = w_clone.hide();
                        commands::window::set_dock_visible(w_clone.app_handle(), false);
                    }
                    tauri::WindowEvent::Focused(false) => {
                        if w_clone.is_minimized().unwrap_or(false) {
                            let _ = w_clone.unminimize();
                            let _ = w_clone.hide();
                            commands::window::set_dock_visible(w_clone.app_handle(), false);
                        }
                    }
                    _ => {}
                });

                // Avvio: silenzioso in background se flag presente, altrimenti visibile
                let is_background =
                    std::env::args().any(|arg| arg == "--background" || arg == "--hidden");
                if is_background {
                    commands::window::set_dock_visible(app.handle(), false);
                    let _ = window.hide();
                } else {
                    commands::window::set_dock_visible(app.handle(), true);
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::recording::start_recording,
            commands::recording::stop_recording,
            commands::recording::import_audio_file,
            commands::recording::list_recordings,
            commands::stt::transcribe_recording,
            commands::stt::check_stt_server,
            commands::stt::start_stt_server,
            commands::stt::stop_stt_server,
            commands::stt::restart_stt_server,
            commands::stt::download_local_model,
            commands::stt::delete_local_model,
            commands::llm::get_system_memory_gb,
            commands::llm::search_hf_models,
            commands::llm::get_hf_model_files,
            commands::llm::set_llm_model,
            commands::llm::download_llm_model,
            commands::llm::check_llm_server,
            commands::llm::start_llm_server,
            commands::llm::stop_llm_server,
            commands::llm::restart_llm_server,
            commands::llm::clear_llm_cache,
            commands::llm::read_recording_notes,
            commands::llm::write_recording_notes,
            commands::llm::delete_recording_notes,
            commands::get_stt_settings,
            commands::save_stt_settings,
            commands::get_local_model_path,
            commands::get_recordings_dir,
            commands::get_transcripts_dir,
            commands::set_transcripts_dir,
            commands::check_microphone_permission,
            commands::check_screen_recording_permission,
            commands::open_permission_settings,
            commands::window::set_compact_mode,
            commands::window::set_compact_hover,
            commands::window::set_compact_y,
            commands::window::save_compact_y,
            commands::window::get_compact_position,
            commands::window::show_dashboard,
            commands::window::hide_dashboard,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::ExitRequested { .. } => {
            commands::server::kill_tracked(
                &app_handle.state::<LlamaServerState>().0,
                &commands::server_lock_path(app_handle, commands::llm::LLM_LOCK),
            );
        }
        tauri::RunEvent::Reopen {
            has_visible_windows,
            ..
        } => {
            if !has_visible_windows {
                if let Some(window) = app_handle.get_webview_window("main") {
                    commands::window::set_dock_visible(app_handle, true);
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
        }
        _ => {}
    });
}
