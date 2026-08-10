use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Condvar, Mutex, OnceLock},
};

use tauri::State;

use crate::{launcher::LauncherState, models::NativeAppIcon};

const ICON_SIZE: u16 = 48;
const CACHE_CAPACITY: usize = 256;
const MAX_ICON_DIMENSION: u32 = 96;
const MAX_ICON_BYTES: usize = MAX_ICON_DIMENSION as usize * MAX_ICON_DIMENSION as usize * 4;
const MAX_CONCURRENT_ICON_LOADS: usize = 2;
const MAX_RESULT_ID_BYTES: usize = 8_192;

#[derive(Default)]
struct IconCache {
    values: HashMap<PathBuf, Option<NativeAppIcon>>,
    insertion_order: VecDeque<PathBuf>,
    loading: HashSet<PathBuf>,
}

#[derive(Default)]
struct IconCacheStore {
    values: Mutex<IconCache>,
    ready: Condvar,
}

/// Limits native Shell/AppKit work, not search work. The command is invoked
/// only after search results are returned, and runs its blocking portion on
/// Tauri's blocking runtime.
struct IconLoadGate {
    maximum: usize,
    active: Mutex<usize>,
    ready: Condvar,
}

impl IconLoadGate {
    fn new(maximum: usize) -> Self {
        Self {
            maximum: maximum.max(1),
            active: Mutex::new(0),
            ready: Condvar::new(),
        }
    }

    fn acquire(&self) -> Option<IconLoadPermit<'_>> {
        let mut active = self.active.lock().ok()?;
        while *active >= self.maximum {
            active = self.ready.wait(active).ok()?;
        }
        *active += 1;
        Some(IconLoadPermit { gate: self })
    }
}

struct IconLoadPermit<'a> {
    gate: &'a IconLoadGate,
}

impl Drop for IconLoadPermit<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.gate.active.lock() {
            *active = active.saturating_sub(1);
            self.gate.ready.notify_one();
        }
    }
}

/// Removes an in-flight marker even if a platform icon provider panics. This
/// prevents duplicate callers from waiting indefinitely for the same id.
struct CacheLoadingGuard<'a> {
    store: &'a IconCacheStore,
    path: PathBuf,
}

impl Drop for CacheLoadingGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut cache) = self.store.values.lock() {
            cache.loading.remove(&self.path);
        }
        self.store.ready.notify_all();
    }
}

static ICON_CACHE: OnceLock<IconCacheStore> = OnceLock::new();
static ICON_LOAD_GATE: OnceLock<IconLoadGate> = OnceLock::new();

#[tauri::command]
pub async fn get_app_icon(
    state: State<'_, std::sync::Arc<LauncherState>>,
    result_id: String,
) -> Result<Option<NativeAppIcon>, String> {
    // `result_id` is intentionally opaque. The frontend never supplies a
    // filesystem path, URL, or icon location; LauncherState resolves it only
    // against the local application catalog built at startup.
    if result_id.len() > MAX_RESULT_ID_BYTES {
        return Ok(None);
    }
    let Some(path) = state.application_path_for_result_id(&result_id) else {
        return Ok(None);
    };

    // An outdated icon request is data-only. It cannot launch or read an
    // arbitrary path, and the webview discards it when its result component is
    // no longer active. Keeping it off the search task avoids input latency.
    tauri::async_runtime::spawn_blocking(move || load_cached(path))
        .await
        .map_err(|error| format!("应用图标任务异常结束：{error}"))
}

fn icon_cache() -> &'static IconCacheStore {
    ICON_CACHE.get_or_init(IconCacheStore::default)
}

fn icon_load_gate() -> &'static IconLoadGate {
    ICON_LOAD_GATE.get_or_init(|| IconLoadGate::new(MAX_CONCURRENT_ICON_LOADS))
}

fn load_cached(path: PathBuf) -> Option<NativeAppIcon> {
    if !is_supported_application_path(&path) {
        return None;
    }

    load_cached_with(icon_cache(), path, |path| {
        let _permit = icon_load_gate().acquire()?;
        load_platform_icon(path)
    })
}

fn load_cached_with<F>(store: &IconCacheStore, path: PathBuf, loader: F) -> Option<NativeAppIcon>
where
    F: FnOnce(&Path) -> Option<NativeAppIcon>,
{
    let mut cache = store.values.lock().ok()?;
    loop {
        if let Some(icon) = cache.values.get(&path) {
            return icon.clone();
        }
        if cache.loading.insert(path.clone()) {
            break;
        }
        cache = store.ready.wait(cache).ok()?;
    }
    drop(cache);

    let loading = CacheLoadingGuard {
        store,
        path: path.clone(),
    };
    let icon = loader(&path);

    if let Ok(mut cache) = store.values.lock() {
        insert_cached(&mut cache, path, icon.clone());
    }
    drop(loading);
    icon
}

fn insert_cached(cache: &mut IconCache, path: PathBuf, icon: Option<NativeAppIcon>) {
    if cache.values.len() >= CACHE_CAPACITY {
        if let Some(oldest) = cache.insertion_order.pop_front() {
            cache.values.remove(&oldest);
        }
    }
    cache.insertion_order.push_back(path.clone());
    // `None` is intentional negative caching. A broken shortcut must not
    // repeatedly dispatch expensive native extraction while it remains in the
    // bounded cache.
    cache.values.insert(path, icon);
}

#[cfg(target_os = "windows")]
fn load_platform_icon(path: &Path) -> Option<NativeAppIcon> {
    // A Shell API lookup of a .lnk can legally return the generic shortcut
    // document icon. Resolve only shortcuts discovered in the runtime app
    // catalog to verified local executables, without calling
    // IShellLink::Resolve (which may contact the network), so installed apps
    // retain their own artwork.
    if is_windows_shortcut(path) {
        if let Some(target) = resolve_windows_shortcut_target(path) {
            if let Some(icon) = load_windows_native_icon(&target) {
                return Some(icon);
            }
        }
    }

    load_windows_native_icon(path)
}

#[cfg(target_os = "windows")]
fn load_windows_native_icon(path: &Path) -> Option<NativeAppIcon> {
    // IShellItemImageFactory is the Windows Shell's native icon pipeline.
    // Some installed applications are rejected by that API, so fall back to
    // SHGetFileInfoW and then ExtractAssociatedIconW for compatibility.
    file_icon_provider::get_file_icon(path, ICON_SIZE)
        .ok()
        .and_then(convert_icon)
        .or_else(|| load_windows_shell_icon(path))
        .or_else(|| load_windows_associated_icon(path))
}

#[cfg(target_os = "windows")]
fn is_windows_shortcut(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))
}

#[cfg(target_os = "windows")]
fn resolve_windows_shortcut_target(path: &Path) -> Option<PathBuf> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};

    use windows::{
        core::{Interface, GUID, HSTRING},
        Win32::{
            Storage::FileSystem::WIN32_FIND_DATAW,
            System::Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile,
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ,
            },
            UI::Shell::{IShellLinkW, SLGP_RAWPATH},
        },
    };

    // `CLSID_ShellLink` is not exported by the windows crate's Shell module,
    // but its documented value is stable. Keep creation local to this helper
    // so only a local .lnk reached through the runtime-discovered result-id
    // catalog is read.
    const SHELL_LINK_CLSID: GUID = GUID::from_u128(0x00021401_0000_0000_c000_000000000046);
    const MAX_SHORTCUT_TARGET_UTF16: usize = 32_768;

    let shortcut_path = path.canonicalize().ok()?;
    let initialized_com = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok() };
    let target = (|| {
        let shell_link: IShellLinkW =
            unsafe { CoCreateInstance(&SHELL_LINK_CLSID, None, CLSCTX_INPROC_SERVER).ok()? };
        let persist_file: IPersistFile = shell_link.cast().ok()?;
        let shortcut = HSTRING::from(shortcut_path.as_path());
        unsafe { persist_file.Load(&shortcut, STGM_READ).ok()? };

        let mut target = [0u16; MAX_SHORTCUT_TARGET_UTF16];
        unsafe {
            shell_link
                .GetPath(
                    &mut target,
                    std::ptr::null_mut::<WIN32_FIND_DATAW>(),
                    SLGP_RAWPATH.0 as u32,
                )
                .ok()?;
        }
        let length = target.iter().position(|value| *value == 0)?;
        let target = PathBuf::from(OsString::from_wide(&target[..length]));
        // Reject UNC, relative, parent-traversal and reparse-point targets
        // before canonicalize or any Shell call can touch a remote location.
        if !is_local_executable_candidate(&target) || !has_no_reparse_components(&target) {
            return None;
        }
        let target = target.canonicalize().ok()?;
        is_local_executable_path(&target).then_some(target)
    })();
    if initialized_com {
        unsafe { CoUninitialize() };
    }
    target
}

#[cfg(target_os = "windows")]
fn load_windows_shell_icon(path: &Path) -> Option<NativeAppIcon> {
    use std::mem::size_of;

    use windows::{
        core::HSTRING,
        Win32::{
            Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
            UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON},
        },
    };

    let mut file_info = SHFILEINFOW::default();
    // Catalog roots are joined from portable slash-separated literals. The
    // Shell shortcut parser is less tolerant of a mixed-separator .lnk path,
    // so normalize the already-validated local catalog path first.
    let canonical_path = path.canonicalize().ok()?;
    let path = HSTRING::from(canonical_path.as_path());
    let received = unsafe {
        SHGetFileInfoW(
            &path,
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut file_info),
            u32::try_from(size_of::<SHFILEINFOW>()).ok()?,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if received == 0 || file_info.hIcon.is_invalid() {
        return None;
    }

    native_icon_from_windows_handle(file_info.hIcon)
}

#[cfg(target_os = "windows")]
fn load_windows_associated_icon(path: &Path) -> Option<NativeAppIcon> {
    use std::os::windows::ffi::OsStrExt;

    use windows::Win32::UI::Shell::ExtractAssociatedIconW;

    // ExtractAssociatedIconW has a fixed 128-code-unit input buffer. Falling
    // back only for paths that fit keeps this legacy Shell API memory-safe.
    let canonical_path = path.canonicalize().ok()?;
    let encoded = canonical_path.as_os_str().encode_wide().collect::<Vec<_>>();
    if encoded.len() >= 128 {
        return None;
    }
    let mut shell_path = [0u16; 128];
    shell_path[..encoded.len()].copy_from_slice(&encoded);
    let mut icon_index = 0u16;
    let icon = unsafe { ExtractAssociatedIconW(None, &mut shell_path, &mut icon_index) };
    if icon.is_invalid() {
        return None;
    }

    native_icon_from_windows_handle(icon)
}

#[cfg(target_os = "windows")]
fn native_icon_from_windows_handle(
    icon: windows::Win32::UI::WindowsAndMessaging::HICON,
) -> Option<NativeAppIcon> {
    use windows::Win32::{
        Graphics::Gdi::DeleteObject,
        UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO},
    };

    let mut icon_info = ICONINFO::default();
    let has_icon_info = unsafe { GetIconInfo(icon, &mut icon_info).is_ok() };
    // GetIconInfo copies the bitmap handles. The HICON is no longer needed,
    // and must be released even when the icon has no colour bitmap.
    unsafe {
        let _ = DestroyIcon(icon);
    }
    if !has_icon_info {
        return None;
    }
    if icon_info.hbmColor.is_invalid() {
        unsafe {
            let _ = DeleteObject(icon_info.hbmMask.into());
        }
        return None;
    }

    let icon = icon_from_windows_bitmap(icon_info.hbmColor);
    unsafe {
        let _ = DeleteObject(icon_info.hbmColor.into());
        let _ = DeleteObject(icon_info.hbmMask.into());
    }
    icon
}

#[cfg(target_os = "windows")]
fn icon_from_windows_bitmap(
    bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
) -> Option<NativeAppIcon> {
    use std::mem::size_of;

    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
        BI_RGB, DIB_RGB_COLORS,
    };

    let mut bitmap_info = BITMAP::default();
    let object_size = i32::try_from(size_of::<BITMAP>()).ok()?;
    if unsafe {
        GetObjectW(
            bitmap.into(),
            object_size,
            Some((&mut bitmap_info as *mut BITMAP).cast()),
        )
    } == 0
    {
        return None;
    }
    let width = u32::try_from(bitmap_info.bmWidth).ok()?;
    let height = u32::try_from(bitmap_info.bmHeight).ok()?;
    if width == 0 || height == 0 || width > MAX_ICON_DIMENSION || height > MAX_ICON_DIMENSION {
        return None;
    }
    let pixel_bytes = usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(4)?;
    if pixel_bytes > MAX_ICON_BYTES {
        return None;
    }

    let mut dib_info = BITMAPINFO::default();
    dib_info.bmiHeader = BITMAPINFOHEADER {
        biSize: u32::try_from(size_of::<BITMAPINFOHEADER>()).ok()?,
        biWidth: i32::try_from(width).ok()?,
        biHeight: -i32::try_from(height).ok()?,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let hdc = unsafe { CreateCompatibleDC(None) };
    if hdc.is_invalid() {
        return None;
    }
    let mut pixels = vec![0; pixel_bytes];
    let copied = unsafe {
        GetDIBits(
            hdc,
            bitmap,
            0,
            height,
            Some(pixels.as_mut_ptr().cast()),
            &mut dib_info,
            DIB_RGB_COLORS,
        )
    };
    unsafe {
        let _ = DeleteDC(hdc);
    }
    if copied != i32::try_from(height).ok()? {
        return None;
    }
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Some(NativeAppIcon {
        width,
        height,
        pixels,
    })
}

#[cfg(target_os = "macos")]
fn load_platform_icon(path: &Path) -> Option<NativeAppIcon> {
    // NSWorkspace/AppKit objects are autoreleased; scope the pool to one icon
    // conversion so repeated results do not accumulate temporary objects.
    objc2::rc::autoreleasepool(|_| {
        convert_icon(file_icon_provider::get_file_icon(path, ICON_SIZE).ok()?)
    })
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn convert_icon(icon: file_icon_provider::Icon) -> Option<NativeAppIcon> {
    if icon.width == 0
        || icon.height == 0
        || icon.width > MAX_ICON_DIMENSION
        || icon.height > MAX_ICON_DIMENSION
    {
        return None;
    }
    let pixel_bytes = usize::try_from(icon.width)
        .ok()?
        .checked_mul(usize::try_from(icon.height).ok()?)?
        .checked_mul(4)?;
    (pixel_bytes <= MAX_ICON_BYTES && icon.pixels.len() == pixel_bytes).then_some(NativeAppIcon {
        width: icon.width,
        height: icon.height,
        pixels: icon.pixels,
    })
}

#[cfg(target_os = "windows")]
fn is_supported_application_path(path: &Path) -> bool {
    is_local_regular_file(path)
        && matches!(
            path.extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            // Application discovery may keep .url entries launchable, but
            // only executables and local Start Menu shortcuts are trusted as
            // native-icon inputs.
            Some("lnk" | "exe")
        )
}

#[cfg(target_os = "windows")]
fn is_local_executable_path(path: &Path) -> bool {
    is_local_executable_candidate(path) && is_local_regular_file(path)
}

#[cfg(target_os = "windows")]
fn is_local_executable_candidate(path: &Path) -> bool {
    is_fixed_local_disk_path(path)
        && !path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        && path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
}

#[cfg(target_os = "windows")]
fn is_local_regular_file(path: &Path) -> bool {
    is_fixed_local_disk_path(path)
        && has_no_reparse_components(path)
        && path
            .symlink_metadata()
            .map(|metadata| metadata.file_type().is_file())
            .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn is_fixed_local_disk_path(path: &Path) -> bool {
    use windows::{core::PCWSTR, Win32::Storage::FileSystem::GetDriveTypeW};

    is_fixed_local_disk_path_with(path, |drive_letter| {
        let root = [
            u16::from(drive_letter),
            u16::from(b':'),
            u16::from(b'\\'),
            0,
        ];
        unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) }
    })
}

#[cfg(target_os = "windows")]
fn is_fixed_local_disk_path_with<F>(path: &Path, drive_type: F) -> bool
where
    F: FnOnce(u8) -> u32,
{
    use std::path::{Component, Prefix};

    if !path.is_absolute() {
        return false;
    }
    let drive_letter = match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter,
            _ => return false,
        },
        _ => return false,
    };
    // Win32 DRIVE_FIXED. Reject remote, removable, optical, RAM, unknown and
    // unavailable drives before any metadata, canonicalize or Shell call.
    drive_type(drive_letter) == 3
}

#[cfg(target_os = "windows")]
fn has_no_reparse_components(path: &Path) -> bool {
    use std::{os::windows::fs::MetadataExt, path::Component};

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        let Ok(metadata) = current.symlink_metadata() else {
            return false;
        };
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return false;
        }
    }
    true
}

#[cfg(target_os = "macos")]
fn is_supported_application_path(path: &Path) -> bool {
    // Do not follow a symlink before handing the path to NSWorkspace. The
    // catalog normally contains real .app bundles, and accepting an alias here
    // would let a discovered entry resolve outside the startup application
    // catalog.
    let Ok(metadata) = path.symlink_metadata() else {
        return false;
    };

    path.is_absolute()
        && metadata.file_type().is_dir()
        && !metadata.file_type().is_symlink()
        && path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("app"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn is_supported_application_path(_path: &Path) -> bool {
    false
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn load_platform_icon(_path: &Path) -> Option<NativeAppIcon> {
    None
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Barrier,
        },
        thread,
        time::Duration,
    };

    use super::{
        load_cached, load_cached_with, IconCacheStore, IconLoadGate, NativeAppIcon,
        MAX_ICON_DIMENSION,
    };

    fn test_icon() -> NativeAppIcon {
        NativeAppIcon {
            width: 1,
            height: 1,
            pixels: vec![0, 1, 2, 3],
        }
    }

    #[test]
    fn cache_includes_negative_results() {
        let store = IconCacheStore::default();
        let path = PathBuf::from("test-app.exe");
        let attempts = AtomicUsize::new(0);

        assert!(load_cached_with(&store, path.clone(), |_| {
            attempts.fetch_add(1, Ordering::SeqCst);
            None
        })
        .is_none());
        assert!(load_cached_with(&store, path, |_| {
            attempts.fetch_add(1, Ordering::SeqCst);
            Some(test_icon())
        })
        .is_none());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn duplicate_icon_requests_share_one_native_load() {
        let store = Arc::new(IconCacheStore::default());
        let path = PathBuf::from("test-app.exe");
        let attempts = Arc::new(AtomicUsize::new(0));
        let start = Arc::new(Barrier::new(3));

        thread::scope(|scope| {
            for _ in 0..3 {
                let store = store.clone();
                let path = path.clone();
                let attempts = attempts.clone();
                let start = start.clone();
                scope.spawn(move || {
                    start.wait();
                    assert!(load_cached_with(&store, path, |_| {
                        attempts.fetch_add(1, Ordering::SeqCst);
                        thread::sleep(Duration::from_millis(20));
                        Some(test_icon())
                    })
                    .is_some());
                });
            }
        });

        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn native_load_gate_has_a_fixed_concurrency_limit() {
        let gate = Arc::new(IconLoadGate::new(2));
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let start = Arc::new(Barrier::new(4));

        thread::scope(|scope| {
            for _ in 0..4 {
                let gate = gate.clone();
                let active = active.clone();
                let peak = peak.clone();
                let start = start.clone();
                scope.spawn(move || {
                    start.wait();
                    let _permit = gate.acquire().expect("gate lock available");
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(20));
                    active.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });

        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn rejects_non_absolute_icon_paths() {
        assert!(load_cached(PathBuf::from("relative/app.exe")).is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn rejects_unc_reparse_and_non_application_paths() {
        assert!(load_cached(PathBuf::from(r"\\server\share\app.exe")).is_none());
        assert!(load_cached(PathBuf::from(r"C:\Windows\win.ini")).is_none());
        assert!(load_cached(PathBuf::from(r"C:\Windows\explorer.url")).is_none());
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    #[test]
    fn rejects_icon_payloads_outside_the_dimension_bound() {
        let oversized = file_icon_provider::Icon {
            width: MAX_ICON_DIMENSION + 1,
            height: 1,
            pixels: vec![0; (MAX_ICON_DIMENSION as usize + 1) * 4],
        };
        assert!(super::convert_icon(oversized).is_none());

        let malformed = file_icon_provider::Icon {
            width: 48,
            height: 48,
            pixels: vec![0; 1],
        };
        assert!(super::convert_icon(malformed).is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn accepts_only_absolute_fixed_drive_targets_before_filesystem_access() {
        let fixed_drive = |_: u8| 3;
        let remote_drive = |_: u8| 4;

        assert!(super::is_fixed_local_disk_path_with(
            PathBuf::from(r"D:\Software\Steam\steam.exe").as_path(),
            fixed_drive
        ));
        assert!(!super::is_fixed_local_disk_path_with(
            PathBuf::from(r"Z:\apps\steam.exe").as_path(),
            remote_drive
        ));
        assert!(!super::is_fixed_local_disk_path_with(
            PathBuf::from(r"C:relative.exe").as_path(),
            fixed_drive
        ));
        assert!(!super::is_fixed_local_disk_path_with(
            PathBuf::from(r"\\unreachable-host\share\app.exe").as_path(),
            fixed_drive
        ));
        assert!(!super::is_local_executable_candidate(
            PathBuf::from(r"\\unreachable-host\share\app.exe").as_path()
        ));
        assert!(!super::is_local_executable_candidate(
            PathBuf::from(r"C:\apps\..\remote\app.exe").as_path()
        ));
        assert!(!super::is_local_executable_candidate(
            PathBuf::from(r"C:relative.exe").as_path()
        ));
        assert!(super::is_local_executable_candidate(
            PathBuf::from(r"D:\Software\Weixin\Weixin.exe").as_path()
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn resolves_and_uses_the_target_icon_for_an_installed_unicode_shortcut() {
        // This regression fixture is present on the Windows acceptance machine.
        // Keep it optional so clean developer/CI installations still run the
        // portable tests, but never fall back to Explorer when the actual
        // Unicode Start Menu shortcut is available.
        let Some(program_data) = std::env::var_os("PROGRAMDATA") else {
            return;
        };
        let shortcut = PathBuf::from(program_data)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("微信")
            .join("微信.lnk");
        if !shortcut.is_file() {
            return;
        }

        let target = super::resolve_windows_shortcut_target(&shortcut)
            .expect("Unicode Start Menu shortcut should resolve to a local executable");
        assert!(super::is_local_executable_path(&target));

        let shortcut_icon = load_cached(shortcut)
            .expect("Unicode Start Menu shortcut should expose its target application icon");
        let target_icon = super::load_windows_native_icon(&target)
            .expect("resolved application executable should expose an icon");
        assert_eq!(shortcut_icon.width, target_icon.width);
        assert_eq!(shortcut_icon.height, target_icon.height);
        assert_eq!(shortcut_icon.pixels, target_icon.pixels);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn extracts_rgba_pixels_from_a_discovered_windows_application_or_shortcut() {
        // Exercise a real Start Menu shortcut when WeChat is installed, while
        // keeping the test portable to a clean Windows installation.
        let path = crate::catalog::discover_applications()
            .into_iter()
            .find(|entry| {
                matches!(entry.name.as_str(), "微信" | "WeChat")
                    && matches!(
                        entry
                            .path
                            .extension()
                            .and_then(|extension| extension.to_str()),
                        Some(extension)
                            if extension.eq_ignore_ascii_case("lnk")
                                || extension.eq_ignore_ascii_case("exe")
                    )
            })
            .map(|entry| entry.path)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows\explorer.exe"));
        let is_supported = super::is_supported_application_path(&path);
        let icon = load_cached(path.clone()).unwrap_or_else(|| {
            panic!(
                "Windows Shell should expose an application icon: {} (supported={is_supported})",
                path.display()
            )
        });
        assert_eq!(icon.pixels.len(), (icon.width * icon.height * 4) as usize);
        assert!(
            icon.pixels.chunks_exact(4).any(|rgba| rgba[3] != 0),
            "native icon should contain visible pixels"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn rejects_symlinked_macos_application_bundle() {
        use std::{fs, os::unix::fs::symlink};

        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "suo-app-icon-symlink-{}-{nonce}",
            std::process::id()
        ));
        let target = root.join("Actual.app");
        let alias = root.join("Alias.app");
        fs::create_dir_all(&target).expect("create application bundle directory");
        symlink(&target, &alias).expect("create application bundle symlink");

        let accepted = super::is_supported_application_path(&alias);
        fs::remove_dir_all(&root).expect("remove temporary application bundle directory");

        assert!(!accepted);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn extracts_rgba_pixels_from_a_macos_application_bundle() {
        let icon = load_cached(PathBuf::from("/System/Library/CoreServices/Finder.app"))
            .expect("Finder should expose a native application icon");
        assert_eq!(icon.pixels.len(), (icon.width * icon.height * 4) as usize);
        assert!(icon.pixels.chunks_exact(4).any(|rgba| rgba[3] != 0));
    }
}
