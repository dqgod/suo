//! Native focus policy for the launcher window.
//!
//! On macOS the launcher must accept keyboard input without activating Suo or
//! replacing the menu bar of the application the user is working in. Settings
//! deliberately remains a regular Tauri window and continues to activate Suo.

#[cfg(target_os = "macos")]
use objc2::runtime::{AnyClass, NSObject, NSObjectProtocol};
#[cfg(target_os = "macos")]
use objc2::{define_class, msg_send, ClassType};
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSPanel, NSWindowStyleMask};
use tauri::{Runtime, WebviewWindow};

#[cfg(target_os = "macos")]
struct SuoLauncherPanelIvars;

#[cfg(target_os = "macos")]
fn launcher_panel_style(style: NSWindowStyleMask) -> NSWindowStyleMask {
    style | NSWindowStyleMask::NonactivatingPanel
}

#[cfg(target_os = "macos")]
define_class!(
    #[unsafe(super = NSPanel)]
    #[name = "SuoLauncherPanel"]
    #[ivars = SuoLauncherPanelIvars]
    struct SuoLauncherPanel;

    unsafe impl NSObjectProtocol for SuoLauncherPanel {}

    impl SuoLauncherPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            false
        }
    }
);

/// Convert only the search launcher to a non-activating AppKit panel.
///
/// Tauri creates an `NSWindow`, so the conversion must happen once while the
/// window is still hidden. The underlying object and its Tauri delegate stay
/// intact; only its Objective-C class and panel style change.
#[cfg(target_os = "macos")]
pub fn prepare_launcher_window<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), String> {
    unsafe extern "C" {
        fn object_setClass(obj: *mut NSObject, cls: *const AnyClass) -> *const AnyClass;
    }

    let native_window = window.ns_window().map_err(|error| error.to_string())?;
    if native_window.is_null() {
        return Err("macOS 主窗口句柄为空".into());
    }

    unsafe {
        let object = native_window.cast::<NSObject>();
        let already_converted: bool = msg_send![object, isKindOfClass: SuoLauncherPanel::class()];
        if !already_converted {
            let previous_class = object_setClass(object, SuoLauncherPanel::class());
            if previous_class.is_null() {
                return Err("无法把 macOS 主窗口转换为非激活面板".into());
            }
        }

        let panel = &*native_window.cast::<SuoLauncherPanel>();
        panel.setStyleMask(launcher_panel_style(panel.styleMask()));
        panel.setHidesOnDeactivate(false);
        panel.setBecomesKeyOnlyIfNeeded(false);
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn prepare_launcher_window<R: Runtime>(_window: &WebviewWindow<R>) -> Result<(), String> {
    Ok(())
}

/// `show()` already calls `makeKeyAndOrderFront:` on macOS. Once the native
/// window is a non-activating panel that is sufficient to focus its WebView;
/// calling Tauri's `set_focus()` would additionally activate the whole app.
#[cfg(target_os = "macos")]
pub fn focus_shown_launcher<R: Runtime>(_window: &WebviewWindow<R>) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn focus_shown_launcher<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), String> {
    window.set_focus().map_err(|error| error.to_string())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::launcher_panel_style;
    use objc2_app_kit::NSWindowStyleMask;

    #[test]
    fn launcher_panel_style_preserves_existing_flags_and_becomes_nonactivating() {
        let style = launcher_panel_style(
            NSWindowStyleMask::Titled | NSWindowStyleMask::FullSizeContentView,
        );
        assert!(style.contains(NSWindowStyleMask::NonactivatingPanel));
        assert!(style.contains(NSWindowStyleMask::Titled));
        assert!(style.contains(NSWindowStyleMask::FullSizeContentView));
    }
}
