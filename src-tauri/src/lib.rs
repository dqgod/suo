mod app_icon;
mod arguments;
#[cfg(any(target_os = "macos", windows))]
mod autostart;
mod calculator;
mod catalog;
mod config;
mod dock;
#[cfg(target_os = "windows")]
mod everything;
mod file_search;
mod focus;
mod hotkey;
mod i18n;
mod launcher;
mod models;
mod scripts;
#[cfg(target_os = "macos")]
mod spotlight;
mod taskbar;
mod translator;
mod tray;
mod web_search;

use std::sync::Arc;

use launcher::{hide_launcher, LauncherState};
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    // 单实例插件必须最先注册，避免第二个进程初始化索引或抢占全局快捷键。
    #[cfg(any(target_os = "macos", windows))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
        if !autostart::is_background_launch(&args) {
            launcher::request_show_launcher(app);
        }
    }));

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        Some(vec![autostart::STARTUP_ARG]),
    ));

    let app = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        match hotkey::dispatch_shortcut(shortcut) {
                            hotkey::ShortcutDispatch::Capture(value) => {
                                if let Some(settings) = app.get_webview_window("settings") {
                                    let _ = settings.emit("hotkey-recording-captured", value);
                                }
                            }
                            hotkey::ShortcutDispatch::Ignore => {}
                            hotkey::ShortcutDispatch::ToggleLauncher => {
                                launcher::toggle_launcher(app);
                            }
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            taskbar::apply_window_policy(app)?;
            let config_state = Arc::new(config::ConfigState::load(app.handle()));
            let initial_config = config_state.snapshot();
            if !config_state.is_read_only() {
                if let Err(error) =
                    autostart::sync(app.handle(), initial_config.launcher.start_at_login)
                {
                    eprintln!("无法同步开机自启设置：{error}");
                }
            }
            dock::apply_initial_visibility(app);
            tray::create(app)?;
            app.manage(config_state);

            if let Err(error) = launcher::prepare_launcher_window(app.handle(), &initial_config) {
                eprintln!("无法在首次显示前应用启动器尺寸与位置：{error}");
            }

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

            let hotkey_status =
                hotkey::register_initial(app.handle(), &initial_config.launcher.global_hotkey);
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
                window.on_window_event(move |event| match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        if let Err(error) = hotkey::stop_recording(event_window.app_handle()) {
                            eprintln!("关闭设置时无法结束快捷键录制：{error}");
                            let _ = event_window.emit("hotkey-recording-error", error);
                            return;
                        }
                        let _ = event_window.emit("hotkey-recording-stopped", ());
                        let _ = event_window.hide();
                        if let Err(error) = dock::settings_closed(event_window.app_handle()) {
                            eprintln!("关闭设置后无法隐藏 Dock 图标：{error}");
                        }
                    }
                    WindowEvent::Focused(false) => {
                        if let Err(error) = hotkey::stop_recording(event_window.app_handle()) {
                            eprintln!("设置失焦时无法结束快捷键录制：{error}");
                            let _ = event_window.emit("hotkey-recording-error", error);
                        } else {
                            let _ = event_window.emit("hotkey-recording-stopped", ());
                        }
                    }
                    WindowEvent::Focused(true) => {
                        // If Windows could not unregister the temporary
                        // Alt+Space guard while focus was leaving, retry as
                        // soon as the user returns to Settings.
                        if let Err(error) = hotkey::stop_recording(event_window.app_handle()) {
                            eprintln!("设置重新聚焦时仍无法结束快捷键录制：{error}");
                            let _ = event_window.emit("hotkey-recording-error", error);
                        } else {
                            let _ = event_window.emit("hotkey-recording-stopped", ());
                        }
                    }
                    _ => {}
                });
            }

            launcher::show_pending_launcher(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            launcher::app_version,
            app_icon::get_app_icon,
            launcher::search_launcher,
            launcher::cancel_search,
            launcher::activate_result,
            launcher::open_settings,
            launcher::hide_settings,
            launcher::rebuild_file_index,
            launcher::get_index_status,
            hide_launcher,
            launcher::set_launcher_compact,
            config::get_app_config,
            config::open_config_directory,
            config::change_config_directory,
            config::save_app_config,
            hotkey::set_hotkey_recording,
            config::set_translation_credentials,
            config::clear_translation_credentials,
            scripts::reveal_script_in_folder
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Suo");

    app.run(|_app, _event| {
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen {
            has_visible_windows: false,
            ..
        } = _event
        {
            // Finder and Dock reactivate an existing macOS application through
            // applicationShouldHandleReopen instead of starting a second process.
            launcher::request_show_launcher(_app);
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn transparent_windows_keep_macos_private_api_enabled() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
        assert_eq!(
            config.pointer("/app/macOSPrivateApi"),
            Some(&serde_json::Value::Bool(true)),
            "macOS ignores transparent windows without its private API feature"
        );

        let windows = config["app"]["windows"]
            .as_array()
            .expect("configured windows");
        for label in ["main", "settings"] {
            let window = windows
                .iter()
                .find(|window| window["label"] == label)
                .unwrap_or_else(|| panic!("missing {label} window"));
            assert_eq!(window["transparent"], true, "{label} must stay transparent");
            assert_eq!(
                window["decorations"], false,
                "{label} must stay undecorated"
            );
        }
    }
}
