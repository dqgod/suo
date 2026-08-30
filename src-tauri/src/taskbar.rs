#[cfg(target_os = "windows")]
use tauri::Manager;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;

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

/// Capture the taskbar item that Windows currently presents as active before
/// the launcher receives keyboard focus. This is a visual-only handle: focus
/// remains on Suo after the launcher is shown.
#[cfg(target_os = "windows")]
pub fn capture_foreground_taskbar_item() -> Option<HWND> {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsWindowVisible};

    let window = unsafe { GetForegroundWindow() };
    (!window.0.is_null() && unsafe { IsWindowVisible(window).as_bool() }).then_some(window)
}

/// Re-mark the previous application's taskbar item as active without
/// reactivating its window. `ITaskbarList::ActivateTab` intentionally changes
/// taskbar presentation only, so the launcher still owns keyboard input.
#[cfg(target_os = "windows")]
pub fn preserve_taskbar_selection(window: HWND) -> Result<(), String> {
    use windows::Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_SERVER,
            COINIT_APARTMENTTHREADED,
        },
        UI::Shell::{ITaskbarList, TaskbarList},
    };

    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if initialized.is_err() && initialized != RPC_E_CHANGED_MODE {
        return Err(format!(
            "无法初始化 Windows 任务栏环境：{}",
            initialized.message()
        ));
    }
    let result = (|| unsafe {
        let taskbar: ITaskbarList = CoCreateInstance(&TaskbarList, None, CLSCTX_SERVER)
            .map_err(|error| format!("无法连接 Windows 任务栏：{error}"))?;
        taskbar
            .HrInit()
            .map_err(|error| format!("无法初始化 Windows 任务栏接口：{error}"))?;
        taskbar
            .ActivateTab(window)
            .map_err(|error| format!("无法保持原应用的任务栏状态：{error}"))?;
        Ok(())
    })();
    if initialized.is_ok() {
        unsafe { CoUninitialize() };
    }
    result
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
