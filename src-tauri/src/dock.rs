#[cfg(target_os = "macos")]
use std::{
    sync::atomic::{AtomicU64, AtomicU8, Ordering},
    thread,
    time::Duration,
};

#[cfg(target_os = "macos")]
const DOCK_HIDDEN: u8 = 1;
#[cfg(target_os = "macos")]
const DOCK_VISIBLE: u8 = 2;
#[cfg(target_os = "macos")]
const DOCK_HIDE_RETRY_DELAY: Duration = Duration::from_millis(1_100);
#[cfg(target_os = "macos")]
static REQUESTED_VISIBILITY: AtomicU8 = AtomicU8::new(0);
#[cfg(target_os = "macos")]
static VISIBILITY_REVISION: AtomicU64 = AtomicU64::new(0);

#[cfg(target_os = "macos")]
pub fn apply_initial_visibility(app: &mut tauri::App, visible: bool) {
    // Apply before the event loop starts so a persisted hidden state does not
    // briefly leave a Dock icon behind during normal startup.
    app.set_dock_visibility(visible);
    REQUESTED_VISIBILITY.store(encoded_visibility(visible), Ordering::Release);
}

#[cfg(not(target_os = "macos"))]
pub fn apply_initial_visibility(_app: &mut tauri::App, _visible: bool) {}

#[cfg(target_os = "macos")]
pub fn apply_visibility(app: &tauri::AppHandle, visible: bool) -> Result<(), String> {
    let encoded = encoded_visibility(visible);
    if REQUESTED_VISIBILITY.load(Ordering::Acquire) == encoded {
        return Ok(());
    }
    app.set_dock_visibility(visible)
        .map_err(|error| format!("设置已保存，但无法更新 Dock 图标：{error}"))?;
    REQUESTED_VISIBILITY.store(encoded, Ordering::Release);
    let revision = VISIBILITY_REVISION.fetch_add(1, Ordering::AcqRel) + 1;

    if !visible {
        // tao intentionally ignores a hide request made within one second of
        // showing the Dock icon to avoid duplicate macOS icons. Reapply the
        // latest hidden state after that guard window; a newer user choice
        // invalidates this retry through the revision check.
        let app = app.clone();
        thread::spawn(move || {
            thread::sleep(DOCK_HIDE_RETRY_DELAY);
            if VISIBILITY_REVISION.load(Ordering::Acquire) == revision
                && REQUESTED_VISIBILITY.load(Ordering::Acquire) == DOCK_HIDDEN
            {
                if let Err(error) = app.set_dock_visibility(false) {
                    eprintln!("无法再次隐藏 Dock 图标：{error}");
                }
            }
        });
    }
    Ok(())
}

#[cfg(target_os = "macos")]
const fn encoded_visibility(visible: bool) -> u8 {
    if visible {
        DOCK_VISIBLE
    } else {
        DOCK_HIDDEN
    }
}

#[cfg(not(target_os = "macos"))]
pub fn apply_visibility(_app: &tauri::AppHandle, _visible: bool) -> Result<(), String> {
    Ok(())
}
