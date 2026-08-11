use tauri::AppHandle;

#[cfg(target_os = "macos")]
use tauri_plugin_autostart::ManagerExt;
#[cfg(target_os = "windows")]
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE},
    RegKey, RegValue,
};

#[cfg(target_os = "windows")]
const RUN_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
#[cfg(target_os = "windows")]
const STARTUP_APPROVED_KEY: &str =
    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";
pub const STARTUP_ARG: &str = "--autostart";
#[cfg(target_os = "windows")]
const STARTUP_APPROVED_ENABLED: [u8; 12] = [
    0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

pub struct RegisteredAutostartChange {
    previous: AutostartSnapshot,
}

pub fn is_background_launch(arguments: &[String]) -> bool {
    arguments.iter().any(|argument| argument == STARTUP_ARG)
}

impl RegisteredAutostartChange {
    pub fn apply(app: &AppHandle, desired_enabled: bool) -> Result<Option<Self>, String> {
        let previous = AutostartSnapshot::read(app)?;
        if previous.satisfies(app, desired_enabled)? {
            return Ok(None);
        }

        if let Err(change_error) = set_enabled(app, desired_enabled).and_then(|_| {
            let actual = AutostartSnapshot::read(app)?;
            if actual.satisfies(app, desired_enabled)? {
                Ok(())
            } else {
                Err(if desired_enabled {
                    "系统未确认开机自启已启用".into()
                } else {
                    "系统未确认开机自启已关闭".into()
                })
            }
        }) {
            return match previous.restore(app) {
                Ok(()) => Err(change_error),
                Err(rollback_error) => Err(format!("{change_error}；{rollback_error}")),
            };
        }

        Ok(Some(Self { previous }))
    }

    pub fn rollback(self, app: &AppHandle) -> Result<(), String> {
        self.previous
            .restore(app)
            .map_err(|error| format!("配置保存失败，且无法恢复原开机自启状态：{error}"))
    }
}

pub fn sync(app: &AppHandle, desired_enabled: bool) -> Result<(), String> {
    let _ = RegisteredAutostartChange::apply(app, desired_enabled)?;
    Ok(())
}

#[cfg(target_os = "macos")]
struct AutostartSnapshot {
    enabled: bool,
}

#[cfg(target_os = "macos")]
impl AutostartSnapshot {
    fn read(app: &AppHandle) -> Result<Self, String> {
        app.autolaunch()
            .is_enabled()
            .map(|enabled| Self { enabled })
            .map_err(|error| format!("无法读取开机自启状态：{error}"))
    }

    fn satisfies(&self, _app: &AppHandle, desired_enabled: bool) -> Result<bool, String> {
        Ok(self.enabled == desired_enabled)
    }

    fn restore(&self, app: &AppHandle) -> Result<(), String> {
        set_enabled(app, self.enabled)
    }
}

#[cfg(target_os = "macos")]
fn set_enabled(app: &AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        app.autolaunch()
            .enable()
            .map_err(|error| format!("无法启用开机自启：{error}"))
    } else {
        app.autolaunch()
            .disable()
            .map_err(|error| format!("无法关闭开机自启：{error}"))
    }
}

#[cfg(target_os = "windows")]
struct AutostartSnapshot {
    run_value: Option<String>,
    startup_approved_value: Option<RegValue>,
}

#[cfg(target_os = "windows")]
impl AutostartSnapshot {
    fn read(app: &AppHandle) -> Result<Self, String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let app_name = app.package_info().name.clone();
        let run_value = read_optional_string(&hkcu, RUN_KEY, &app_name)
            .map_err(|error| format!("无法读取 Windows 开机自启项：{error}"))?;
        let startup_approved_value = read_optional_raw(&hkcu, STARTUP_APPROVED_KEY, &app_name)
            .map_err(|error| format!("无法读取 Windows 启动批准状态：{error}"))?;
        Ok(Self {
            run_value,
            startup_approved_value,
        })
    }

    fn satisfies(&self, _app: &AppHandle, desired_enabled: bool) -> Result<bool, String> {
        if !desired_enabled {
            return Ok(self.run_value.is_none());
        }
        Ok(self.run_value.as_deref() == Some(&expected_run_command()?)
            && startup_approved_is_enabled(self.startup_approved_value.as_ref()))
    }

    fn restore(&self, app: &AppHandle) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let app_name = app.package_info().name.clone();
        let (run, _) = hkcu
            .create_subkey(RUN_KEY)
            .map_err(|error| format!("无法打开 Windows 开机自启项：{error}"))?;
        restore_string_value(&run, &app_name, self.run_value.as_deref())
            .map_err(|error| format!("无法恢复 Windows 开机自启项：{error}"))?;

        match hkcu.open_subkey_with_flags(STARTUP_APPROVED_KEY, KEY_SET_VALUE) {
            Ok(approved) => {
                restore_raw_value(&approved, &app_name, self.startup_approved_value.as_ref())
                    .map_err(|error| format!("无法恢复 Windows 启动批准状态：{error}"))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("无法打开 Windows 启动批准状态：{error}")),
        }
    }
}

#[cfg(target_os = "windows")]
fn set_enabled(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let app_name = app.package_info().name.clone();
    let (run, _) = hkcu
        .create_subkey(RUN_KEY)
        .map_err(|error| format!("无法打开 Windows 开机自启项：{error}"))?;
    if enabled {
        run.set_value(&app_name, &expected_run_command()?)
            .map_err(|error| format!("无法启用开机自启：{error}"))?;
        match hkcu.open_subkey_with_flags(STARTUP_APPROVED_KEY, KEY_SET_VALUE) {
            Ok(approved) => approved
                .set_raw_value(
                    &app_name,
                    &RegValue {
                        vtype: winreg::enums::RegType::REG_BINARY,
                        bytes: STARTUP_APPROVED_ENABLED.to_vec(),
                    },
                )
                .map_err(|error| format!("无法启用 Windows 启动批准状态：{error}"))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("无法打开 Windows 启动批准状态：{error}")),
        }
    } else {
        delete_value_if_present(&run, &app_name)
            .map_err(|error| format!("无法关闭开机自启：{error}"))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn expected_run_command() -> Result<String, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("无法确定 Suo 可执行文件位置：{error}"))?;
    Ok(format!("\"{}\" {STARTUP_ARG}", executable.display()))
}

#[cfg(target_os = "windows")]
fn read_optional_string(root: &RegKey, path: &str, name: &str) -> std::io::Result<Option<String>> {
    let key = match root.open_subkey_with_flags(path, KEY_READ) {
        Ok(key) => key,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    match key.get_value(name) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "windows")]
fn read_optional_raw(root: &RegKey, path: &str, name: &str) -> std::io::Result<Option<RegValue>> {
    let key = match root.open_subkey_with_flags(path, KEY_READ) {
        Ok(key) => key,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    match key.get_raw_value(name) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "windows")]
fn restore_string_value(key: &RegKey, name: &str, value: Option<&str>) -> std::io::Result<()> {
    if let Some(value) = value {
        key.set_value(name, &value)
    } else {
        delete_value_if_present(key, name)
    }
}

#[cfg(target_os = "windows")]
fn restore_raw_value(key: &RegKey, name: &str, value: Option<&RegValue>) -> std::io::Result<()> {
    if let Some(value) = value {
        key.set_raw_value(name, value)
    } else {
        delete_value_if_present(key, name)
    }
}

#[cfg(target_os = "windows")]
fn delete_value_if_present(key: &RegKey, name: &str) -> std::io::Result<()> {
    match key.delete_value(name) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "windows")]
fn startup_approved_is_enabled(value: Option<&RegValue>) -> bool {
    let Some(value) = value else {
        // A Run entry without an Explorer override is enabled by default.
        return true;
    };
    value.vtype == winreg::enums::RegType::REG_BINARY
        && value.bytes.len() == STARTUP_APPROVED_ENABLED.len()
        && value.bytes.first() == Some(&0x02)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn startup_approved_matches_windows_enabled_shape() {
        let enabled = RegValue {
            vtype: winreg::enums::RegType::REG_BINARY,
            bytes: STARTUP_APPROVED_ENABLED.to_vec(),
        };
        let mut enabled_with_timestamp = STARTUP_APPROVED_ENABLED.to_vec();
        enabled_with_timestamp[4] = 1;
        let enabled_with_timestamp = RegValue {
            vtype: winreg::enums::RegType::REG_BINARY,
            bytes: enabled_with_timestamp,
        };
        let mut disabled_bytes = STARTUP_APPROVED_ENABLED.to_vec();
        disabled_bytes[0] = 0x03;
        let disabled = RegValue {
            vtype: winreg::enums::RegType::REG_BINARY,
            bytes: disabled_bytes,
        };
        let malformed = RegValue {
            vtype: winreg::enums::RegType::REG_SZ,
            bytes: STARTUP_APPROVED_ENABLED.to_vec(),
        };
        assert!(startup_approved_is_enabled(None));
        assert!(startup_approved_is_enabled(Some(&enabled)));
        assert!(startup_approved_is_enabled(Some(&enabled_with_timestamp)));
        assert!(!startup_approved_is_enabled(Some(&disabled)));
        assert!(!startup_approved_is_enabled(Some(&malformed)));
    }

    #[test]
    fn recognizes_background_startup_in_forwarded_arguments() {
        assert!(is_background_launch(&[
            r#"C:\Program Files\Suo\suo.exe"#.into(),
            STARTUP_ARG.into(),
        ]));
        assert!(!is_background_launch(&["suo.exe".into()]));
    }
}
