#[cfg(target_os = "windows")]
use tauri::Manager;

#[cfg(any(target_os = "windows", test))]
const WINDOWS_TASKBAR_POLICY: [(&str, bool); 2] = [("main", true), ("settings", false)];

/// The launcher is summoned by the global shortcut and stays out of the
/// Windows taskbar. Settings remains a normal taskbar window so users can
/// switch back to a longer configuration session. macOS Dock visibility is
/// managed independently by `dock.rs`.
#[cfg(target_os = "windows")]
pub fn apply_window_policy(app: &tauri::App) -> tauri::Result<()> {
    for (label, skip_taskbar) in WINDOWS_TASKBAR_POLICY {
        if let Some(window) = app.get_webview_window(label) {
            window.set_skip_taskbar(skip_taskbar)?;
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn apply_window_policy(_app: &tauri::App) -> tauri::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::WINDOWS_TASKBAR_POLICY;

    #[test]
    fn launcher_and_settings_have_separate_windows_taskbar_roles() {
        assert_eq!(
            WINDOWS_TASKBAR_POLICY,
            [("main", true), ("settings", false)]
        );
    }
}
