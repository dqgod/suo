use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

use serde::Serialize;
use tauri::State;

use crate::launcher::LauncherState;

const ICON_SIZE: u16 = 32;
const CACHE_CAPACITY: usize = 256;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppIcon {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Default)]
struct IconCache {
    values: HashMap<PathBuf, Option<AppIcon>>,
    insertion_order: VecDeque<PathBuf>,
}

static ICON_CACHE: OnceLock<Mutex<IconCache>> = OnceLock::new();

#[tauri::command]
pub async fn get_app_icon(
    state: State<'_, Arc<LauncherState>>,
    result_id: String,
) -> Result<Option<AppIcon>, String> {
    if result_id.len() > 8192 {
        return Ok(None);
    }
    let Some(path) = state.application_path_for_result_id(&result_id) else {
        return Ok(None);
    };

    tauri::async_runtime::spawn_blocking(move || load_cached(path))
        .await
        .map_err(|error| format!("应用图标任务异常结束：{error}"))
}

fn load_cached(path: PathBuf) -> Option<AppIcon> {
    if !is_supported_application_path(&path) {
        return None;
    }

    let cache = ICON_CACHE.get_or_init(|| Mutex::new(IconCache::default()));
    let Ok(mut cache) = cache.lock() else {
        return None;
    };
    if let Some(icon) = cache.values.get(&path) {
        return icon.clone();
    }

    let icon = load_platform_icon(&path);
    if cache.values.len() >= CACHE_CAPACITY {
        if let Some(oldest) = cache.insertion_order.pop_front() {
            cache.values.remove(&oldest);
        }
    }
    cache.insertion_order.push_back(path.clone());
    cache.values.insert(path, icon.clone());
    icon
}

#[cfg(target_os = "windows")]
fn load_platform_icon(path: &std::path::Path) -> Option<AppIcon> {
    convert_icon(file_icon_provider::get_file_icon(path, ICON_SIZE).ok()?)
}

#[cfg(target_os = "macos")]
fn load_platform_icon(path: &std::path::Path) -> Option<AppIcon> {
    objc2::rc::autoreleasepool(|_| {
        convert_icon(file_icon_provider::get_file_icon(path, ICON_SIZE).ok()?)
    })
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn convert_icon(icon: file_icon_provider::Icon) -> Option<AppIcon> {
    (icon.width > 0
        && icon.height > 0
        && icon.pixels.len() == (icon.width * icon.height * 4) as usize)
        .then_some(AppIcon {
            width: icon.width,
            height: icon.height,
            pixels: icon.pixels,
        })
}

#[cfg(target_os = "windows")]
fn is_supported_application_path(path: &std::path::Path) -> bool {
    use std::path::{Component, Prefix};

    let is_local_drive = matches!(
        path.components().next(),
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_))
    );
    is_local_drive
        && path.is_file()
        && matches!(
            path.extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("lnk" | "url" | "exe")
        )
}

#[cfg(target_os = "macos")]
fn is_supported_application_path(path: &std::path::Path) -> bool {
    path.is_absolute()
        && path.is_dir()
        && path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("app"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn is_supported_application_path(_path: &std::path::Path) -> bool {
    false
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn load_platform_icon(_path: &std::path::Path) -> Option<AppIcon> {
    None
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::load_cached;

    #[test]
    fn rejects_non_absolute_icon_paths() {
        assert!(load_cached(PathBuf::from("relative/app.exe")).is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn rejects_unc_and_non_application_paths() {
        assert!(load_cached(PathBuf::from(r"\\server\share\app.exe")).is_none());
        assert!(load_cached(PathBuf::from(r"C:\Windows\win.ini")).is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn extracts_rgba_pixels_from_a_windows_application() {
        let icon = load_cached(PathBuf::from(r"C:\Windows\explorer.exe"))
            .expect("Windows Explorer should expose an application icon");
        assert_eq!(icon.pixels.len(), (icon.width * icon.height * 4) as usize);
    }
}
