use std::{
    io::Read,
    path::PathBuf,
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

const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(3);

pub fn run_timestamp<F>(app: &AppHandle, args: &[String], is_cancelled: F) -> Result<String, String>
where
    F: Fn() -> bool,
{
    if args.is_empty() {
        return Err("用法：ts <毫秒时间戳> [更多时间戳]".into());
    }

    let script = find_script(app).ok_or_else(|| "找不到 examples/timestamp.py".to_string())?;
    let mut last_not_found = None;

    for python in ["python", "python3"] {
        let mut command = Command::new(python);
        command.arg(&script).args(args);
        if let Some(parent) = script.parent() {
            command.current_dir(parent);
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        configure_process_group(&mut command);
        hide_console(&mut command);

        match command.spawn() {
            Ok(child) => return collect_output(child, &is_cancelled, SCRIPT_TIMEOUT),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                last_not_found = Some(error);
            }
            Err(error) => return Err(format!("无法执行 {python}：{error}")),
        }
    }

    Err(format!(
        "未找到 Python 解释器：{}",
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
            return Err("脚本执行超过 3 秒，已终止进程树".into());
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
        terminate_unix_group(self.process_group, "-TERM");
        thread::sleep(Duration::from_millis(100));
        terminate_unix_group(self.process_group, "-KILL");
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(unix)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        terminate_unix_group(self.process_group, "-KILL");
    }
}

#[cfg(unix)]
fn terminate_unix_group(process_group: u32, signal: &str) {
    let process_group = format!("-{process_group}");
    let _ = Command::new("/bin/kill")
        .args([signal, &process_group])
        .status();
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

fn find_script(app: &AppHandle) -> Option<PathBuf> {
    let resource = app
        .path()
        .resolve("examples/timestamp.py", BaseDirectory::Resource)
        .ok();
    let source_tree = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/timestamp.py");

    resource
        .into_iter()
        .chain([source_tree])
        .find(|path| path.is_file())
}

#[cfg(target_os = "windows")]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn hide_console(_command: &mut Command) {}

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

    use super::{collect_output, hide_console, spawn_capped_reader, MAX_OUTPUT_BYTES};

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
