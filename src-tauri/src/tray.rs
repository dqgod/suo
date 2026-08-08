use std::io::{Error, ErrorKind};

use tauri::{
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App,
};

use crate::{i18n, launcher};

const TRAY_ID: &str = "suo-tray";
const SHOW_ID: &str = "tray-show";
const SETTINGS_ID: &str = "tray-settings";
const QUIT_ID: &str = "tray-quit";

pub fn create(app: &App) -> tauri::Result<()> {
    let icon = app.default_window_icon().cloned().ok_or_else(|| {
        Error::new(
            ErrorKind::NotFound,
            "Suo 的默认应用图标不可用，无法创建系统托盘",
        )
    })?;
    let menu = MenuBuilder::new(app)
        .text(SHOW_ID, i18n::TRAY_SHOW)
        .text(SETTINGS_ID, i18n::TRAY_SETTINGS)
        .separator()
        .text(QUIT_ID, i18n::TRAY_QUIT)
        .build()?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip(i18n::TRAY_TOOLTIP)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW_ID => show_launcher(app),
            SETTINGS_ID => {
                if let Err(error) = launcher::open_settings(app.clone()) {
                    eprintln!("无法从系统托盘打开设置：{error}");
                }
            }
            QUIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_launcher(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn show_launcher(app: &tauri::AppHandle) {
    if let Err(error) = launcher::show_launcher(app) {
        eprintln!("无法从系统托盘显示 Suo：{error}");
    }
}
