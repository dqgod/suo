#[cfg(target_os = "macos")]
use std::{
    sync::atomic::{AtomicU64, AtomicU8, Ordering},
    thread,
    time::Duration,
};

#[cfg(target_os = "macos")]
use tauri::Manager;

#[cfg(target_os = "macos")]
const DOCK_HIDDEN: u8 = 1;
#[cfg(target_os = "macos")]
const DOCK_VISIBLE: u8 = 2;
#[cfg(target_os = "macos")]
const DOCK_HIDE_RETRY_DELAY: Duration = Duration::from_millis(1_100);
#[cfg(target_os = "macos")]
const SETTINGS_RESTORE_DELAY: Duration = Duration::from_millis(80);
#[cfg(target_os = "macos")]
static REQUESTED_VISIBILITY: AtomicU8 = AtomicU8::new(0);
#[cfg(target_os = "macos")]
static VISIBILITY_REVISION: AtomicU64 = AtomicU64::new(0);

#[cfg(target_os = "macos")]
pub fn apply_initial_visibility(app: &mut tauri::App) {
    // The preference only exposes Suo in the Dock while Settings is open.
    // Startup therefore always begins as a menu-bar application, even when
    // the persisted preference is enabled.
    app.set_dock_visibility(false);
    REQUESTED_VISIBILITY.store(DOCK_HIDDEN, Ordering::Release);
}

#[cfg(not(target_os = "macos"))]
pub fn apply_initial_visibility(_app: &mut tauri::App) {}

pub fn visible_for_settings(preference_enabled: bool, settings_visible: bool) -> bool {
    preference_enabled && settings_visible
}

#[cfg(target_os = "macos")]
pub fn settings_opened(app: &tauri::AppHandle, preference_enabled: bool) -> Result<(), String> {
    apply_visibility(app, visible_for_settings(preference_enabled, true), true)
}

#[cfg(not(target_os = "macos"))]
pub fn settings_opened(_app: &tauri::AppHandle, _preference_enabled: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn settings_closed(app: &tauri::AppHandle) -> Result<(), String> {
    apply_visibility(app, false, false)
}

#[cfg(not(target_os = "macos"))]
pub fn settings_closed(_app: &tauri::AppHandle) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn preference_changed(app: &tauri::AppHandle, preference_enabled: bool) -> Result<(), String> {
    let settings_visible = app
        .get_webview_window("settings")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    apply_visibility(
        app,
        visible_for_settings(preference_enabled, settings_visible),
        settings_visible,
    )
}

#[cfg(not(target_os = "macos"))]
pub fn preference_changed(
    _app: &tauri::AppHandle,
    _preference_enabled: bool,
) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn apply_visibility(
    app: &tauri::AppHandle,
    visible: bool,
    preserve_settings_window: bool,
) -> Result<(), String> {
    let encoded = encoded_visibility(visible);
    REQUESTED_VISIBILITY.store(encoded, Ordering::Release);
    // Every lifecycle request gets a new revision, even if the requested Dock
    // state is unchanged. Closing Settings must cancel a pending window restore
    // or delayed hide that belonged to an earlier save.
    let revision = VISIBILITY_REVISION.fetch_add(1, Ordering::AcqRel) + 1;

    app.set_dock_visibility(visible)
        .map_err(|error| format!("设置已保存，但无法更新 Dock 图标：{error}"))?;

    if !visible && preserve_settings_window {
        restore_settings_window(app);
        schedule_settings_restore(app.clone(), revision);
    }

    if !visible {
        // tao intentionally ignores a hide request made within one second of
        // showing the Dock icon to avoid duplicate macOS icons. Reapply the
        // latest hidden state after that guard window. If Settings is still
        // open, restore it after both the immediate and delayed transitions so
        // disabling this preference cannot make the page disappear.
        let app = app.clone();
        thread::spawn(move || {
            thread::sleep(DOCK_HIDE_RETRY_DELAY);
            if is_current_hidden_request(revision) {
                if let Err(error) = app.set_dock_visibility(false) {
                    eprintln!("无法再次隐藏 Dock 图标：{error}");
                    return;
                }
                if preserve_settings_window {
                    restore_settings_window(&app);
                    schedule_settings_restore(app, revision);
                }
            }
        });
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn schedule_settings_restore(app: tauri::AppHandle, revision: u64) {
    thread::spawn(move || {
        thread::sleep(SETTINGS_RESTORE_DELAY);
        if is_current_hidden_request(revision) {
            restore_settings_window(&app);
        }
    });
}

#[cfg(target_os = "macos")]
fn restore_settings_window(app: &tauri::AppHandle) {
    // Switching the process from Foreground to UIElement can deactivate the
    // current window on macOS. Re-showing the application and Settings keeps
    // the page alive while the Dock icon disappears.
    if let Err(error) = app.show() {
        eprintln!("隐藏 Dock 图标后无法恢复应用：{error}");
    }
    let Some(window) = app.get_webview_window("settings") else {
        return;
    };
    if let Err(error) = window.unminimize() {
        eprintln!("隐藏 Dock 图标后无法恢复设置窗口大小：{error}");
    }
    if let Err(error) = window.show() {
        eprintln!("隐藏 Dock 图标后无法恢复设置窗口：{error}");
    }
    if let Err(error) = window.set_focus() {
        eprintln!("隐藏 Dock 图标后无法聚焦设置窗口：{error}");
    }
}

#[cfg(target_os = "macos")]
fn is_current_hidden_request(revision: u64) -> bool {
    VISIBILITY_REVISION.load(Ordering::Acquire) == revision
        && REQUESTED_VISIBILITY.load(Ordering::Acquire) == DOCK_HIDDEN
}

#[cfg(target_os = "macos")]
const fn encoded_visibility(visible: bool) -> u8 {
    if visible {
        DOCK_VISIBLE
    } else {
        DOCK_HIDDEN
    }
}

#[cfg(test)]
mod tests {
    use super::visible_for_settings;

    #[test]
    fn dock_icon_only_follows_an_open_settings_window() {
        assert!(!visible_for_settings(false, false));
        assert!(!visible_for_settings(false, true));
        assert!(!visible_for_settings(true, false));
        assert!(visible_for_settings(true, true));
    }
}
