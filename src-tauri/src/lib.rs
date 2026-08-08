mod app_icon;
mod calculator;
mod catalog;
mod config;
#[cfg(target_os = "windows")]
mod everything;
mod file_search;
mod i18n;
mod launcher;
mod models;
mod scripts;
#[cfg(target_os = "macos")]
mod spotlight;
mod translator;
mod tray;

use std::sync::Arc;

use launcher::{hide_launcher, position_launcher, LauncherState};
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    // 单实例插件必须最先注册，避免第二个进程初始化索引或抢占全局快捷键。
    #[cfg(any(target_os = "macos", windows))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        launcher::request_show_launcher(app);
    }));

    builder
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
            tray::create(app)?;

            let config_state = Arc::new(config::ConfigState::load(app.handle()));
            let initial_config = config_state.snapshot();
            app.manage(config_state);

            let state = Arc::new(LauncherState::new());
            state.update_preferences(
                initial_config.launcher.close_on_blur,
                initial_config.launcher.keep_last_input,
            );
            app.manage(state.clone());
            // macOS protected folders can trigger privacy prompts. Defer the
            // fallback scan until the user explicitly requests a file search.
            #[cfg(not(target_os = "macos"))]
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

            launcher::show_pending_launcher(app.handle());
            let _ = position_launcher(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            launcher::app_version,
            app_icon::get_app_icon,
            launcher::search_launcher,
            launcher::cancel_search,
            launcher::activate_result,
            launcher::open_settings,
            launcher::rebuild_file_index,
            launcher::get_index_status,
            hide_launcher,
            config::get_app_config,
            config::save_app_config,
            config::set_translation_api_key,
            config::clear_translation_api_key
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
