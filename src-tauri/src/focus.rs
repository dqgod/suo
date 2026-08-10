//! Platform focus handoff for the global launcher shortcut.
//!
//! On macOS, hiding the launcher while Settings remains visible can make
//! Settings the next key window. Remembering the external application that
//! owned focus before the launcher appeared lets a second shortcut press
//! return to that application without affecting normal result activation.

#[cfg(target_os = "macos")]
use std::sync::Mutex;

#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};

#[cfg(target_os = "macos")]
static PREVIOUS_APPLICATION: Mutex<Option<Retained<NSRunningApplication>>> = Mutex::new(None);

#[cfg(target_os = "macos")]
pub fn capture_previous_application() {
    let workspace = NSWorkspace::sharedWorkspace();
    let current = NSRunningApplication::currentApplication();
    let previous = workspace
        .frontmostApplication()
        .filter(|application| application != &current);
    if let Ok(mut stored) = PREVIOUS_APPLICATION.lock() {
        *stored = previous;
    }
}

#[cfg(not(target_os = "macos"))]
pub fn capture_previous_application() {}

#[cfg(target_os = "macos")]
pub fn restore_previous_application() {
    let previous = PREVIOUS_APPLICATION
        .lock()
        .ok()
        .and_then(|mut stored| stored.take());
    let Some(previous) = previous else {
        return;
    };
    if previous.isTerminated() {
        return;
    }
    let _ = previous.unhide();
    if !previous.activateWithOptions(NSApplicationActivationOptions::empty()) {
        eprintln!("隐藏 Suo 搜索框后无法恢复先前应用焦点");
    }
}

#[cfg(not(target_os = "macos"))]
pub fn restore_previous_application() {}

#[cfg(target_os = "macos")]
pub fn forget_previous_application() {
    if let Ok(mut stored) = PREVIOUS_APPLICATION.lock() {
        *stored = None;
    }
}

#[cfg(not(target_os = "macos"))]
pub fn forget_previous_application() {}
