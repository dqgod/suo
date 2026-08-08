use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, TryRecvError},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use tauri::{path::BaseDirectory, AppHandle, Manager};

use crate::config::{ScriptCommandConfig, ScriptRuntime};

const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

#[tauri::command]
pub fn reveal_script_in_folder(app: AppHandle, configured_path: String) -> Result<(), String> {
    let script = find_script(&app, &configured_path)
        .ok_or_else(|| format!("找不到脚本：{}", configured_path.trim()))?;
    let mut command = reveal_command(&script)?;
    let mut child = command
        .spawn()
        .map_err(|error| format!("无法在文件夹中显示 {}：{error}", script.display()))?;
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(target_os = "macos")]
fn reveal_command(script: &Path) -> Result<Command, String> {
    let mut command = Command::new("/usr/bin/open");
    command.arg("-R").arg(script);
    Ok(command)
}

#[cfg(target_os = "windows")]
fn reveal_command(script: &Path) -> Result<Command, String> {
    use std::ffi::OsString;
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut selection = OsString::from("/select,");
    selection.push(script.as_os_str());
    let mut command = Command::new("explorer.exe");
    command.arg(selection);
    command.creation_flags(CREATE_NO_WINDOW);
    Ok(command)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn reveal_command(_script: &Path) -> Result<Command, String> {
    Err("当前平台不支持在文件夹中显示脚本".into())
}

pub fn run_configured<F>(
    app: &AppHandle,
    config: &ScriptCommandConfig,
    args: &[String],
    is_cancelled: F,
) -> Result<String, String>
where
    F: Fn() -> bool,
{
    ensure_unprivileged()?;
    let script = find_script(app, &config.script_path)
        .ok_or_else(|| format!("找不到脚本：{}", config.script_path))?;
    let timeout = Duration::from_millis(config.timeout_ms);
    let interpreters: &[&str] = match config.runtime {
        ScriptRuntime::Python => &["python", "python3"],
        #[cfg(target_os = "windows")]
        ScriptRuntime::PowerShell => &["powershell.exe", "pwsh"],
        #[cfg(not(target_os = "windows"))]
        ScriptRuntime::PowerShell => &["pwsh"],
        ScriptRuntime::Bash => &["bash"],
        ScriptRuntime::Executable => &[],
    };
    let mut last_not_found = None;

    if matches!(config.runtime, ScriptRuntime::Executable) {
        let mut command = Command::new(&script);
        command.args(args);
        if let Some(parent) = script.parent() {
            command.current_dir(parent);
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        configure_process_group(&mut command);
        hide_console(&mut command);
        let child = command
            .spawn()
            .map_err(|error| format!("无法执行 {}：{error}", script.display()))?;
        return collect_output(child, &is_cancelled, timeout);
    }

    for interpreter in interpreters {
        let mut command = Command::new(interpreter);
        if matches!(config.runtime, ScriptRuntime::PowerShell) {
            command.args(["-NoProfile", "-NonInteractive", "-File"]);
        }
        command.arg(&script).args(args);
        if let Some(parent) = script.parent() {
            command.current_dir(parent);
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        configure_process_group(&mut command);
        hide_console(&mut command);
        match command.spawn() {
            Ok(child) => return collect_output(child, &is_cancelled, timeout),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                last_not_found = Some(error);
            }
            Err(error) => return Err(format!("无法执行 {interpreter}：{error}")),
        }
    }

    Err(format!(
        "未找到脚本解释器：{}",
        last_not_found
            .map(|error| error.to_string())
            .unwrap_or_else(|| "未知错误".into())
    ))
}

fn collect_output<F>(
    mut child: Child,
    is_cancelled: &F,
    timeout: Duration,
) -> Result<String, String>
where
    F: Fn() -> bool,
{
    let process_tree = match ProcessTree::attach(&child) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let total_bytes = Arc::new(AtomicUsize::new(0));
    let output_exceeded = Arc::new(AtomicBool::new(false));
    let stdout_reader = child
        .stdout
        .take()
        .map(|stdout| spawn_capped_reader(stdout, total_bytes.clone(), output_exceeded.clone()));
    let stderr_reader = child
        .stderr
        .take()
        .map(|stderr| spawn_capped_reader(stderr, total_bytes.clone(), output_exceeded.clone()));
    let started = Instant::now();
    let mut status = None;
    let mut stdout = stdout_reader.is_none().then(Vec::new);
    let mut stderr = stderr_reader.is_none().then(Vec::new);

    loop {
        poll_reader(&stdout_reader, &mut stdout);
        poll_reader(&stderr_reader, &mut stderr);

        if is_cancelled() {
            process_tree.terminate(&mut child);
            drain_reader(&stdout_reader);
            drain_reader(&stderr_reader);
            return Err("脚本执行已取消".into());
        }
        if output_exceeded.load(Ordering::SeqCst) {
            process_tree.terminate(&mut child);
            drain_reader(&stdout_reader);
            drain_reader(&stderr_reader);
            return Err("脚本输出超过 1 MB 技术验证上限".into());
        }
        if started.elapsed() >= timeout {
            process_tree.terminate(&mut child);
            drain_reader(&stdout_reader);
            drain_reader(&stderr_reader);
            return Err(format!(
                "脚本执行超过 {} ms，已终止进程树",
                timeout.as_millis()
            ));
        }

        if status.is_none() {
            match child.try_wait() {
                Ok(next_status) => status = next_status,
                Err(error) => {
                    process_tree.terminate(&mut child);
                    drain_reader(&stdout_reader);
                    drain_reader(&stderr_reader);
                    return Err(format!("无法检查脚本状态：{error}"));
                }
            }
        }
        if status.is_some() && stdout.is_some() && stderr.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    let status = status.expect("status is checked before leaving the collection loop");
    let stdout = stdout.unwrap_or_default();
    let stderr = stderr.unwrap_or_default();
    if status.success() {
        Ok(String::from_utf8_lossy(&stdout).trim().to_string())
    } else {
        let error = String::from_utf8_lossy(&stderr).trim().to_string();
        Err(if error.is_empty() {
            format!("脚本退出码：{:?}", status.code())
        } else {
            error
        })
    }
}

fn spawn_capped_reader<R>(
    mut reader: R,
    total_bytes: Arc<AtomicUsize>,
    output_exceeded: Arc<AtomicBool>,
) -> Receiver<Vec<u8>>
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut output = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let count = match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(count) => count,
            };
            let previous = total_bytes.fetch_add(count, Ordering::SeqCst);
            let remaining = MAX_OUTPUT_BYTES.saturating_sub(previous);
            output.extend_from_slice(&buffer[..count.min(remaining)]);
            if count > remaining {
                output_exceeded.store(true, Ordering::SeqCst);
                break;
            }
        }
        let _ = sender.send(output);
    });
    receiver
}

fn poll_reader(reader: &Option<Receiver<Vec<u8>>>, output: &mut Option<Vec<u8>>) {
    if output.is_some() {
        return;
    }
    let Some(reader) = reader else {
        *output = Some(Vec::new());
        return;
    };
    match reader.try_recv() {
        Ok(value) => *output = Some(value),
        Err(TryRecvError::Disconnected) => *output = Some(Vec::new()),
        Err(TryRecvError::Empty) => {}
    }
}

fn drain_reader(reader: &Option<Receiver<Vec<u8>>>) {
    if let Some(reader) = reader {
        let _ = reader.recv_timeout(Duration::from_millis(500));
    }
}

#[cfg(target_os = "windows")]
struct ProcessTree {
    job: windows::Win32::Foundation::HANDLE,
}

#[cfg(target_os = "windows")]
impl ProcessTree {
    fn attach(child: &Child) -> Result<Self, String> {
        use std::{ffi::c_void, mem::size_of, os::windows::io::AsRawHandle};

        use windows::{
            core::PCWSTR,
            Win32::{
                Foundation::{CloseHandle, HANDLE},
                System::JobObjects::{
                    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                },
            },
        };

        // SAFETY: all pointers passed to Win32 reference live values for the
        // duration of each call; the returned job handle is closed in Drop.
        unsafe {
            let job = CreateJobObjectW(None, PCWSTR::null())
                .map_err(|error| format!("无法创建脚本 Job Object：{error}"))?;
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if let Err(error) = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const c_void,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) {
                let _ = CloseHandle(job);
                return Err(format!("无法配置脚本 Job Object：{error}"));
            }

            let process = HANDLE(child.as_raw_handle());
            if let Err(error) = AssignProcessToJobObject(job, process) {
                let _ = CloseHandle(job);
                return Err(format!("无法隔离脚本进程树：{error}"));
            }
            if let Err(error) = resume_suspended_child(child.id()) {
                let _ = CloseHandle(job);
                return Err(error);
            }
            Ok(Self { job })
        }
    }

    fn terminate(&self, child: &mut Child) {
        use windows::Win32::System::JobObjects::TerminateJobObject;

        // SAFETY: self.job remains valid until Drop and belongs to this tree.
        let _ = unsafe { TerminateJobObject(self.job, 1) };
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(target_os = "windows")]
fn resume_suspended_child(process_id: u32) -> Result<(), String> {
    use std::mem::size_of;
    use windows::Win32::{
        Foundation::CloseHandle,
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD,
                THREADENTRY32,
            },
            Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
        },
    };

    // SAFETY: snapshot/thread handles are owned locally and closed once;
    // THREADENTRY32 has the required dwSize before enumeration.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)
            .map_err(|error| format!("无法枚举脚本线程：{error}"))?;
        let mut entry = THREADENTRY32 {
            dwSize: size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        let mut next = Thread32First(snapshot, &mut entry);
        while next.is_ok() {
            if entry.th32OwnerProcessID == process_id {
                let thread = match OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID) {
                    Ok(thread) => thread,
                    Err(error) => {
                        let _ = CloseHandle(snapshot);
                        return Err(format!("无法打开脚本主线程：{error}"));
                    }
                };
                let previous_count = ResumeThread(thread);
                let _ = CloseHandle(thread);
                let _ = CloseHandle(snapshot);
                if previous_count == u32::MAX {
                    return Err("无法恢复已隔离的脚本进程".into());
                }
                return Ok(());
            }
            next = Thread32Next(snapshot, &mut entry);
        }
        let _ = CloseHandle(snapshot);
    }
    Err("找不到已挂起的脚本主线程".into())
}

#[cfg(target_os = "windows")]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;

        // SAFETY: the handle is owned by this ProcessTree and closed once.
        let _ = unsafe { CloseHandle(self.job) };
    }
}

#[cfg(unix)]
struct ProcessTree {
    process_group: u32,
}

#[cfg(unix)]
impl ProcessTree {
    fn attach(child: &Child) -> Result<Self, String> {
        Ok(Self {
            process_group: child.id(),
        })
    }

    fn terminate(&self, child: &mut Child) {
        terminate_unix_group(self.process_group, libc::SIGTERM);
        thread::sleep(Duration::from_millis(100));
        terminate_unix_group(self.process_group, libc::SIGKILL);
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(unix)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        terminate_unix_group(self.process_group, libc::SIGKILL);
    }
}

#[cfg(unix)]
fn terminate_unix_group(process_group: u32, signal: libc::c_int) {
    let Ok(process_group) = i32::try_from(process_group) else {
        return;
    };
    // SAFETY: a negative PID targets the child-owned process group created in
    // configure_process_group. Signals are best effort because the group may
    // already have exited between polling and termination.
    let _ = unsafe { libc::kill(-process_group, signal) };
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

fn find_script(app: &AppHandle, configured_path: &str) -> Option<PathBuf> {
    let configured_path = configured_path.trim();
    let expanded = if configured_path == "~" {
        dirs::home_dir()
    } else if let Some(relative) = configured_path
        .strip_prefix("~/")
        .or_else(|| configured_path.strip_prefix("~\\"))
    {
        dirs::home_dir().map(|home| home.join(relative))
    } else {
        Some(PathBuf::from(configured_path))
    }?;
    if expanded.is_absolute() {
        return expanded.is_file().then_some(expanded);
    }

    let resource = app.path().resolve(&expanded, BaseDirectory::Resource).ok();
    let config_relative = app
        .path()
        .app_config_dir()
        .ok()
        .map(|directory| directory.join(&expanded));
    let source_tree = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(&expanded);

    resource
        .into_iter()
        .chain(config_relative)
        .chain([source_tree])
        .find(|path| path.is_file())
}

#[cfg(target_os = "windows")]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_SUSPENDED: u32 = 0x0000_0004;
    // Suspending before spawn returns closes the gap in which a script could
    // create descendants before it is assigned to the Job Object.
    command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
}

#[cfg(not(target_os = "windows"))]
fn hide_console(_command: &mut Command) {}

#[cfg(target_os = "windows")]
fn ensure_unprivileged() -> Result<(), String> {
    use std::{ffi::c_void, mem::size_of};
    use windows::Win32::{
        Foundation::{CloseHandle, HANDLE},
        Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    // SAFETY: token is initialized by OpenProcessToken; the elevation buffer
    // is valid for GetTokenInformation and the owned token is closed once.
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|error| format!("无法检查当前进程权限：{error}"))?;
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0;
        let result = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut c_void),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        );
        let _ = CloseHandle(token);
        result.map_err(|error| format!("无法检查当前进程权限：{error}"))?;
        if elevation.TokenIsElevated != 0 {
            return Err("Suo 正以管理员权限运行，已拒绝执行自定义脚本".into());
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn ensure_unprivileged() -> Result<(), String> {
    // SAFETY: geteuid has no preconditions and does not dereference pointers.
    if unsafe { libc::geteuid() } == 0 {
        Err("Suo 正以 root 权限运行，已拒绝执行自定义脚本".into())
    } else {
        Ok(())
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn ensure_unprivileged() -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        io::Cursor,
        process::{Command, Stdio},
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };

    #[cfg(target_os = "windows")]
    use super::hide_console;
    use super::{collect_output, configure_process_group, spawn_capped_reader, MAX_OUTPUT_BYTES};

    #[cfg(target_os = "macos")]
    #[test]
    fn finder_reveal_command_selects_the_script() {
        let command = super::reveal_command(std::path::Path::new("/tmp/example script.py"))
            .expect("macOS reveal command should be available");
        assert_eq!(command.get_program(), "/usr/bin/open");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["-R", "/tmp/example script.py"]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn explorer_reveal_command_selects_the_script() {
        let command = super::reveal_command(std::path::Path::new(r"C:\scripts\example script.py"))
            .expect("Windows reveal command should be available");
        assert_eq!(command.get_program(), "explorer.exe");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [r"/select,C:\scripts\example script.py"]
        );
    }

    #[test]
    fn reader_caps_output_while_streaming() {
        let total = Arc::new(AtomicUsize::new(0));
        let exceeded = Arc::new(AtomicBool::new(false));
        let input = Cursor::new(vec![b'x'; MAX_OUTPUT_BYTES + 1]);
        let reader = spawn_capped_reader(input, total, exceeded.clone());
        let output = reader
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("reader should finish");

        assert_eq!(output.len(), MAX_OUTPUT_BYTES);
        assert!(exceeded.load(Ordering::SeqCst));
    }

    #[cfg(unix)]
    #[test]
    fn inherited_pipe_descendant_cannot_escape_deadline() {
        let pid_file = std::env::temp_dir().join(format!(
            "suo-process-tree-test-{}-{}.pid",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "sleep 5 & echo $! > \"$1\"; wait",
            "suo-process-tree-test",
            pid_file.to_str().expect("temporary path should be UTF-8"),
        ]);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        configure_process_group(&mut command);
        let child = command.spawn().expect("test command should start");
        // SAFETY: getpgid only reads kernel process metadata for the live child.
        assert_eq!(
            unsafe { libc::getpgid(child.id() as i32) },
            child.id() as i32
        );
        let started = Instant::now();

        let result = collect_output(child, &|| false, Duration::from_millis(250));

        assert_eq!(result.unwrap_err(), "脚本执行超过 250 ms，已终止进程树");
        assert!(started.elapsed() < Duration::from_secs(2));
        let descendant = std::fs::read_to_string(&pid_file)
            .expect("descendant PID should be recorded")
            .trim()
            .parse::<i32>()
            .expect("descendant PID should be numeric");
        let deadline = Instant::now() + Duration::from_secs(1);
        while process_exists(descendant) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let descendant_survived = process_exists(descendant);
        if descendant_survived {
            // SAFETY: this only cleans up the exact descendant created above.
            let _ = unsafe { libc::kill(descendant, libc::SIGKILL) };
        }
        let _ = std::fs::remove_file(pid_file);
        assert!(!descendant_survived, "descendant process survived timeout");
    }

    #[cfg(unix)]
    fn process_exists(process_id: i32) -> bool {
        // SAFETY: signal 0 performs an existence/permission check only.
        if unsafe { libc::kill(process_id, 0) } == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn inherited_pipe_descendant_cannot_escape_deadline() {
        let mut command = Command::new("cmd.exe");
        command.args(["/D", "/S", "/C", r#"start "" /b ping.exe -n 6 127.0.0.1"#]);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        hide_console(&mut command);
        let child = command.spawn().expect("test command should start");
        let started = Instant::now();

        let result = collect_output(child, &|| false, Duration::from_millis(250));

        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
