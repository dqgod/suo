//! Windows packaged-application discovery, activation and Shell parsing names.
//!
//! Start Menu shortcuts cover traditional desktop programs, while MSIX/UWP
//! applications such as Microsoft Store and Xbox are exposed by Windows through
//! application user model IDs (AUMIDs). Keep that platform-specific boundary in
//! one module so the shared launcher catalog never has to understand COM.

use std::{
    collections::HashSet,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const APPS_FOLDER_PREFIX: &str = "shell:AppsFolder\\";
const MAX_AUMID_BYTES: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackagedApplication {
    pub name: String,
    pub app_user_model_id: String,
}

#[derive(Debug, Deserialize)]
struct StartAppRecord {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "AppID")]
    app_user_model_id: String,
}

/// Discover Start-menu packaged applications without delaying hotkey setup.
/// The caller runs this function on a dedicated background thread.
pub fn discover() -> Result<Vec<PackagedApplication>, String> {
    let system_root = std::env::var_os("SystemRoot")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Windows 系统目录不可用".to_string())?;
    let powershell =
        PathBuf::from(system_root).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    if !powershell.is_file() {
        return Err("找不到 Windows PowerShell，无法读取打包应用目录".into());
    }

    // The command is fixed and receives no user-controlled values. Filtering
    // for `!` excludes ordinary desktop entries that are already represented
    // by trusted local Start Menu shortcuts.
    let output = Command::new(powershell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); @(Get-StartApps | Where-Object { $_.AppID -like '*!*' } | Select-Object Name,AppID) | ConvertTo-Json -Compress",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("无法读取 Windows 打包应用目录：{error}"))?;

    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            "Windows 打包应用目录命令执行失败".into()
        } else {
            format!("Windows 打包应用目录命令执行失败：{detail}")
        });
    }

    parse_start_apps_json(&String::from_utf8_lossy(&output.stdout))
}

fn parse_start_apps_json(json: &str) -> Result<Vec<PackagedApplication>, String> {
    let json = json.trim().trim_start_matches('\u{feff}');
    if json.is_empty() || json == "null" {
        return Ok(Vec::new());
    }

    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|error| format!("Windows 打包应用目录格式无效：{error}"))?;
    let records = match value {
        serde_json::Value::Array(values) => values,
        serde_json::Value::Object(_) => vec![value],
        _ => return Err("Windows 打包应用目录不是对象或数组".into()),
    };

    let mut seen = HashSet::new();
    let mut applications = Vec::new();
    for value in records {
        let Ok(record) = serde_json::from_value::<StartAppRecord>(value) else {
            continue;
        };
        let name = record.name.trim();
        let app_user_model_id = record.app_user_model_id.trim();
        if name.is_empty()
            || name.len() > 512
            || !is_valid_app_user_model_id(app_user_model_id)
            || !seen.insert(app_user_model_id.to_ascii_lowercase())
        {
            continue;
        }
        applications.push(PackagedApplication {
            name: name.to_string(),
            app_user_model_id: app_user_model_id.to_string(),
        });
    }
    applications.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then(
                left.app_user_model_id
                    .to_lowercase()
                    .cmp(&right.app_user_model_id.to_lowercase()),
            )
    });
    Ok(applications)
}

pub fn is_valid_app_user_model_id(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_AUMID_BYTES {
        return false;
    }
    let Some((package_family, application_id)) = value.split_once('!') else {
        return false;
    };
    if package_family.is_empty() || application_id.is_empty() || application_id.contains('!') {
        return false;
    }
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'!'))
}

pub fn shell_path(app_user_model_id: &str) -> Option<PathBuf> {
    is_valid_app_user_model_id(app_user_model_id)
        .then(|| PathBuf::from(format!("{APPS_FOLDER_PREFIX}{app_user_model_id}")))
}

pub fn app_user_model_id_from_shell_path(path: &Path) -> Option<&str> {
    let value = path.to_str()?;
    let app_user_model_id = value.strip_prefix(APPS_FOLDER_PREFIX)?;
    is_valid_app_user_model_id(app_user_model_id).then_some(app_user_model_id)
}

pub fn launch(app_user_model_id: &str) -> Result<(), String> {
    if !is_valid_app_user_model_id(app_user_model_id) {
        return Err("Windows 应用标识无效".into());
    }

    use windows::{
        core::{HSTRING, PCWSTR},
        Win32::{
            Foundation::RPC_E_CHANGED_MODE,
            System::Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_LOCAL_SERVER,
                COINIT_APARTMENTTHREADED,
            },
            UI::Shell::{ApplicationActivationManager, IApplicationActivationManager, AO_NONE},
        },
    };

    struct ComApartment(bool);
    impl Drop for ComApartment {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if initialized.is_err() && initialized != RPC_E_CHANGED_MODE {
            return Err(format!(
                "无法初始化 Windows 应用启动环境：{}",
                initialized.message()
            ));
        }
        // RPC_E_CHANGED_MODE means this worker already has a usable COM
        // apartment of another type. Continue, but do not uninitialize it.
        let _apartment = ComApartment(initialized.is_ok());
        let manager: IApplicationActivationManager =
            CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER)
                .map_err(|error| format!("无法创建 Windows 应用启动器：{error}"))?;
        manager
            .ActivateApplication(&HSTRING::from(app_user_model_id), PCWSTR::null(), AO_NONE)
            .map_err(|error| format!("无法启动 Windows 应用：{error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_one_or_many_packaged_applications_and_deduplicates_ids() {
        let one = parse_start_apps_json(
            r#"{"Name":"Microsoft Store","AppID":"Microsoft.WindowsStore_8wekyb3d8bbwe!App"}"#,
        )
        .expect("parse one app");
        assert_eq!(one.len(), 1);

        let many = parse_start_apps_json(
            r#"[
              {"Name":"Xbox","AppID":"Microsoft.GamingApp_8wekyb3d8bbwe!Microsoft.Xbox.App"},
              {"Name":"Xbox duplicate","AppID":"Microsoft.GamingApp_8wekyb3d8bbwe!Microsoft.Xbox.App"},
              {"Name":"Unsafe","AppID":"shell:AppsFolder\\unsafe"}
            ]"#,
        )
        .expect("parse many apps");
        assert_eq!(many.len(), 1);
        assert_eq!(many[0].name, "Xbox");
    }

    #[test]
    fn validates_only_bounded_packaged_application_ids() {
        assert!(is_valid_app_user_model_id("OpenAI.Codex_2p2nqsd0c76g0!App"));
        assert!(is_valid_app_user_model_id(
            "Microsoft.GamingApp_8wekyb3d8bbwe!Microsoft.Xbox.App"
        ));
        for invalid in [
            "",
            "NoApplicationSeparator",
            "!App",
            "Package!",
            "Package!App!Other",
            "Package!../../cmd.exe",
            "Package Family!App",
        ] {
            assert!(!is_valid_app_user_model_id(invalid), "accepted {invalid}");
        }
    }

    #[test]
    fn shell_parsing_path_round_trips_validated_id() {
        let app_user_model_id = "Microsoft.WindowsStore_8wekyb3d8bbwe!App";
        let path = shell_path(app_user_model_id).expect("valid shell path");
        assert_eq!(
            app_user_model_id_from_shell_path(&path),
            Some(app_user_model_id)
        );
        assert!(
            app_user_model_id_from_shell_path(Path::new("shell:AppsFolder\\..\\cmd.exe")).is_none()
        );
    }

    #[test]
    #[ignore = "requires the local Windows Start application catalog"]
    fn discovers_installed_chatgpt_xbox_and_microsoft_store() {
        let applications = discover().expect("discover Start applications");
        for app_user_model_id in [
            "OpenAI.Codex_2p2nqsd0c76g0!App",
            "Microsoft.GamingApp_8wekyb3d8bbwe!Microsoft.Xbox.App",
            "Microsoft.WindowsStore_8wekyb3d8bbwe!App",
        ] {
            assert!(
                applications.iter().any(|application| application
                    .app_user_model_id
                    .eq_ignore_ascii_case(app_user_model_id)),
                "missing {app_user_model_id}"
            );
        }
    }
}
