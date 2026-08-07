mod calculator;
mod catalog;
mod everything;
mod i18n;
mod launcher;
mod models;
mod scripts;

use std::sync::Arc;

use launcher::{hide_launcher, position_launcher, LauncherState};
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        launcher::toggle_launcher(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let state = Arc::new(LauncherState::new());
            app.manage(state.clone());
            LauncherState::start_file_index(state.clone());

            let (shortcut, shortcut_label) = default_shortcut();
            let hotkey_status = match app.global_shortcut().register(shortcut) {
                Ok(()) => format!("{shortcut_label} 已就绪"),
                Err(error) => format!("{shortcut_label} 注册失败：{error}"),
            };
            state.set_hotkey_status(hotkey_status);

            if let Some(window) = app.get_webview_window("main") {
                let event_window = window.clone();
                let event_state = state.clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::Focused(false) => {
                        if event_state.consume_keep_visible_on_blur()
                            || !event_state.close_on_blur()
                        {
                            return;
                        }
                        let _ = event_window.hide();
                        let _ = event_window.emit("launcher-hidden", ());
                    }
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = event_window.hide();
                        let _ = event_window.emit("launcher-hidden", ());
                    }
                    _ => {}
                });
            }

            if let Some(window) = app.get_webview_window("settings") {
                let event_window = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = event_window.hide();
                    }
                });
            }

            let _ = position_launcher(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            launcher::app_version,
            launcher::search_launcher,
            launcher::cancel_search,
            launcher::activate_result,
            launcher::open_settings,
            launcher::get_launcher_preferences,
            launcher::update_launcher_preferences,
            launcher::rebuild_file_index,
            hide_launcher
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Suo");
}

fn default_shortcut() -> (Shortcut, &'static str) {
    #[cfg(target_os = "macos")]
    {
        (
            Shortcut::new(Some(Modifiers::SUPER), Code::Space),
            "Command+Space",
        )
    }

    #[cfg(not(target_os = "macos"))]
    {
        (
            Shortcut::new(Some(Modifiers::ALT), Code::Space),
            "Alt+Space",
        )
    }
}
