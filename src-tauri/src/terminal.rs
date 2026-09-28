use std::process::Command;

use tauri::AppHandle;

use crate::config::TerminalCommandConfig;

pub(crate) const MAX_TERMINAL_COMMAND_BYTES: usize = 16 * 1024;

pub(crate) fn validate_command(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("请输入要执行的终端命令".into());
    }
    if value.contains('\0') {
        return Err("终端命令不能包含 NUL 字符".into());
    }
    if value.len() > MAX_TERMINAL_COMMAND_BYTES {
        return Err(format!(
            "终端命令超过 {} KB 上限",
            MAX_TERMINAL_COMMAND_BYTES / 1024
        ));
    }
    Ok(value.to_string())
}

pub(crate) fn target_label(config: &TerminalCommandConfig) -> String {
    #[cfg(target_os = "windows")]
    {
        return config.windows_shell.display_name().into();
    }
    #[cfg(target_os = "macos")]
    {
        return config.macos_terminal_application.clone();
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = config;
        "终端".into()
    }
}

pub(crate) fn launch(
    app: &AppHandle,
    config: &TerminalCommandConfig,
    command_text: &str,
) -> Result<(), String> {
    crate::scripts::ensure_unprivileged()?;
    let command_text = validate_command(command_text)?;
    launch_platform(app, config, &command_text)
}

#[cfg(target_os = "windows")]
fn launch_platform(
    _app: &AppHandle,
    config: &TerminalCommandConfig,
    command_text: &str,
) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    use windows::Win32::System::Threading::CREATE_NEW_CONSOLE;

    let system_directory = windows_system_directory()?;
    let mut command = windows_terminal_command(config, command_text, &system_directory);
    let program = command.get_program().to_owned();
    if !std::path::Path::new(&program).is_file() {
        return Err(format!("找不到终端程序：{}", program.to_string_lossy()));
    }
    if let Some(home) = dirs::home_dir().filter(|path| path.is_dir()) {
        command.current_dir(home);
    }
    command.creation_flags(CREATE_NEW_CONSOLE.0);
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法打开 {}：{error}", target_label(config)))
}

#[cfg(target_os = "windows")]
fn windows_system_directory() -> Result<std::path::PathBuf, String> {
    use std::os::windows::ffi::OsStringExt;

    use windows::Win32::System::SystemInformation::GetSystemDirectoryW;

    let mut buffer = vec![0_u16; 260];
    loop {
        // SAFETY: Windows receives a writable slice for the duration of the
        // call. The return value is checked before the initialized prefix is
        // converted into an OsString.
        let length = unsafe { GetSystemDirectoryW(Some(&mut buffer)) } as usize;
        if length == 0 {
            return Err(format!(
                "无法确定 Windows 系统目录：{}",
                std::io::Error::last_os_error()
            ));
        }
        if length < buffer.len() {
            buffer.truncate(length);
            return Ok(std::ffi::OsString::from_wide(&buffer).into());
        }
        let required = length
            .checked_add(1)
            .filter(|required| *required <= 32_768)
            .ok_or_else(|| "Windows 系统目录长度异常".to_string())?;
        buffer.resize(required, 0);
    }
}

#[cfg(target_os = "windows")]
fn windows_terminal_command(
    config: &TerminalCommandConfig,
    command_text: &str,
    system_directory: &std::path::Path,
) -> Command {
    use crate::config::WindowsTerminalShell;

    let (program, arguments): (std::path::PathBuf, &[&str]) = match config.windows_shell {
        WindowsTerminalShell::PowerShell => (
            system_directory
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe"),
            &["-NoLogo", "-NoProfile", "-NoExit", "-Command"],
        ),
        WindowsTerminalShell::CommandPrompt => (system_directory.join("cmd.exe"), &["/D", "/K"]),
    };
    let mut command = Command::new(program);
    command.args(arguments).arg(command_text);
    command
}

#[cfg(target_os = "macos")]
fn launch_platform(
    app: &AppHandle,
    config: &TerminalCommandConfig,
    command_text: &str,
) -> Result<(), String> {
    use std::{
        fs,
        io::Write,
        os::unix::fs::{OpenOptionsExt, PermissionsExt},
        time::{Duration, SystemTime},
    };

    use tauri::Manager;

    let directory = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("无法确定终端命令缓存目录：{error}"))?
        .join("terminal-commands");
    fs::create_dir_all(&directory).map_err(|error| format!("无法创建终端命令缓存目录：{error}"))?;
    let mut permissions = fs::metadata(&directory)
        .map_err(|error| format!("无法检查终端命令缓存目录：{error}"))?
        .permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&directory, permissions)
        .map_err(|error| format!("无法保护终端命令缓存目录：{error}"))?;
    cleanup_stale_command_files(&directory, SystemTime::now(), Duration::from_secs(86_400));

    let stem = format!("suo-terminal-{}", uuid::Uuid::new_v4());
    let payload_path = directory.join(format!("{stem}.sh"));
    let wrapper_path = directory.join(format!("{stem}.command"));

    let mut payload = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&payload_path)
        .map_err(|error| format!("无法创建临时终端命令：{error}"))?;
    if let Err(error) = write!(payload, "#!/bin/bash\ncd -- \"$HOME\"\n{command_text}\n")
        .and_then(|_| payload.sync_all())
    {
        let _ = fs::remove_file(&payload_path);
        return Err(format!("无法写入临时终端命令：{error}"));
    }

    let mut wrapper = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(&wrapper_path)
    {
        Ok(file) => file,
        Err(error) => {
            let _ = fs::remove_file(&payload_path);
            return Err(format!("无法创建终端启动文件：{error}"));
        }
    };
    let wrapper_text = terminal_wrapper_script();
    if let Err(error) = wrapper
        .write_all(wrapper_text.as_bytes())
        .and_then(|_| wrapper.sync_all())
    {
        let _ = fs::remove_file(&wrapper_path);
        let _ = fs::remove_file(&payload_path);
        return Err(format!("无法写入终端启动文件：{error}"));
    }

    let output = match Command::new("/usr/bin/open")
        .arg("-a")
        .arg(&config.macos_terminal_application)
        .arg(&wrapper_path)
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            let _ = fs::remove_file(&wrapper_path);
            let _ = fs::remove_file(&payload_path);
            return Err(format!("无法调用 macOS open：{error}"));
        }
    };
    if output.status.success() {
        return Ok(());
    }

    let _ = fs::remove_file(&wrapper_path);
    let _ = fs::remove_file(&payload_path);
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if detail.is_empty() {
        format!("无法用 {} 打开终端命令", config.macos_terminal_application)
    } else {
        format!(
            "无法用 {} 打开终端命令：{detail}",
            config.macos_terminal_application
        )
    })
}

#[cfg(target_os = "macos")]
fn terminal_wrapper_script() -> &'static str {
    "#!/bin/bash\n\
wrapper_path=\"$0\"\n\
payload_path=\"${wrapper_path%.command}.sh\"\n\
rm -f -- \"$wrapper_path\"\n\
/bin/bash \"$payload_path\"\n\
status=$?\n\
rm -f -- \"$payload_path\"\n\
printf '\\n[Suo] command exited with status %s. Press Control-D to close.\\n' \"$status\"\n\
exec /bin/bash -l\n"
}

#[cfg(target_os = "macos")]
fn cleanup_stale_command_files(
    directory: &std::path::Path,
    now: std::time::SystemTime,
    maximum_age: std::time::Duration,
) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with("suo-terminal-")
            || !matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("sh" | "command")
            )
        {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= maximum_age);
        if stale {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn launch_platform(
    _app: &AppHandle,
    _config: &TerminalCommandConfig,
    _command_text: &str,
) -> Result<(), String> {
    Err("内置 > 命令当前仅支持 Windows 和 macOS".into())
}

#[cfg(test)]
mod tests {
    use super::{validate_command, MAX_TERMINAL_COMMAND_BYTES};

    #[test]
    fn terminal_command_validation_is_explicit_and_bounded() {
        assert_eq!(validate_command("  echo hello  ").unwrap(), "echo hello");
        assert!(validate_command("   ").is_err());
        assert!(validate_command("echo\0hello").is_err());
        assert!(validate_command(&"x".repeat(MAX_TERMINAL_COMMAND_BYTES)).is_ok());
        assert!(validate_command(&"x".repeat(MAX_TERMINAL_COMMAND_BYTES + 1)).is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn stale_cleanup_preserves_recent_and_unrelated_files() {
        use std::{
            fs,
            time::{Duration, SystemTime},
        };

        let directory = std::env::temp_dir().join(format!(
            "suo-terminal-cleanup-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&directory).unwrap();
        let stale = SystemTime::now() - Duration::from_secs(2 * 86_400);
        let old_owned = directory.join("suo-terminal-old.sh");
        let recent_owned = directory.join("suo-terminal-recent.command");
        let old_unrelated = directory.join("other-terminal-old.sh");
        let old_wrong_extension = directory.join("suo-terminal-old.txt");
        for path in [
            &old_owned,
            &recent_owned,
            &old_unrelated,
            &old_wrong_extension,
        ] {
            fs::write(path, b"test").unwrap();
        }
        for path in [&old_owned, &old_unrelated, &old_wrong_extension] {
            fs::File::open(path).unwrap().set_modified(stale).unwrap();
        }

        super::cleanup_stale_command_files(
            &directory,
            SystemTime::now(),
            Duration::from_secs(86_400),
        );
        assert!(!old_owned.exists());
        assert!(recent_owned.exists());
        assert!(old_unrelated.exists());
        assert!(old_wrong_extension.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_terminal_command_uses_the_selected_visible_shell() {
        use crate::config::{AppConfig, WindowsTerminalShell};

        let system_directory = super::windows_system_directory().expect("trusted system directory");
        let mut config = AppConfig::default().launcher.terminal;
        let powershell = super::windows_terminal_command(&config, "Get-Date", &system_directory);
        let expected_powershell = system_directory
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        assert!(powershell
            .get_program()
            .to_string_lossy()
            .eq_ignore_ascii_case(&expected_powershell.to_string_lossy()));
        assert_eq!(
            powershell
                .get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            ["-NoLogo", "-NoProfile", "-NoExit", "-Command", "Get-Date"]
        );

        config.windows_shell = WindowsTerminalShell::CommandPrompt;
        let command_prompt = super::windows_terminal_command(&config, "dir", &system_directory);
        let expected_command_prompt = system_directory.join("cmd.exe");
        assert!(command_prompt
            .get_program()
            .to_string_lossy()
            .eq_ignore_ascii_case(&expected_command_prompt.to_string_lossy()));
        assert_eq!(
            command_prompt
                .get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            ["/D", "/K", "dir"]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_system_directory_does_not_trust_inherited_system_root() {
        const CHILD_MARKER: &str = "SUO_SYSTEM_DIRECTORY_ENV_TEST";
        const FAKE_ROOT: &str = "C:\\suo-fake-windows";
        if std::env::var_os(CHILD_MARKER).is_some() {
            let directory = super::windows_system_directory().expect("trusted system directory");
            assert_ne!(
                directory,
                std::path::PathBuf::from(FAKE_ROOT).join("System32")
            );
            assert!(directory.is_absolute());
            return;
        }

        let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "terminal::tests::windows_system_directory_does_not_trust_inherited_system_root",
                "--nocapture",
            ])
            .env(CHILD_MARKER, "1")
            .env("SystemRoot", FAKE_ROOT)
            .output()
            .expect("run isolated environment test");
        assert!(
            output.status.success(),
            "child test failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
