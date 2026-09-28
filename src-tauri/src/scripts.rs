use std::{
    io::{Cursor, Read},
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

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use image::{DynamicImage, ImageFormat, Luma};
use qrcode::{bits::Bits, EcLevel, QrCode, QrResult, Version};
use tauri::{path::BaseDirectory, AppHandle, Manager};

use crate::config::{validate_script_result_image_data_url, ScriptCommandConfig, ScriptRuntime};

const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_RESULT_SHELL_COMMAND_BYTES: usize = 16 * 1024;
const SCRIPT_RESULT_PREFIX: &str = "SUO_RESULT:";
const SCRIPT_TEXT_RESULT_PREFIX: &str = "SUO_RESULT:text:";
const SCRIPT_IMAGE_RESULT_PREFIX: &str = "SUO_RESULT:image:";
const SCRIPT_QR_RESULT_PREFIX: &str = "SUO_RESULT:qrcode:";
// Keep the preview scannable inside the launcher. At error-correction level M,
// 500 UTF-8 bytes fit in version 17 or smaller (85 modules plus the quiet zone).
// Rendering every module as 4x4 pixels therefore stays within the 384 px preview.
const MAX_QR_CONTENT_BYTES: usize = 500;
const UTF8_ECI_DESIGNATOR: u32 = 26;
const QR_IMAGE_PIXELS: u32 = 384;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScriptOutput {
    Text(String),
    Image { data_url: String, copy_text: String },
}

#[tauri::command]
pub fn reveal_script_in_folder(app: AppHandle, configured_path: String) -> Result<(), String> {
    let script = find_script(&app, &configured_path)
        .ok_or_else(|| format!("找不到脚本：{}", configured_path.trim()))?;
    reveal_script(&script)
}

#[cfg(target_os = "macos")]
fn reveal_script(script: &Path) -> Result<(), String> {
    let mut command = Command::new("/usr/bin/open");
    command.arg("-R").arg(script);
    let mut child = command
        .spawn()
        .map_err(|error| format!("无法在文件夹中显示 {}：{error}", script.display()))?;
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(target_os = "windows")]
fn reveal_script(script: &Path) -> Result<(), String> {
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::RPC_E_CHANGED_MODE,
            System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED},
            UI::Shell::{ILCreateFromPathW, ILFree, SHOpenFolderAndSelectItems},
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

    let script_path = windows_shell_path(script);

    // SAFETY: the string remains alive and NUL-terminated for the complete call. The PIDL is
    // checked for null and released with ILFree. A zero-item selection tells Explorer to open
    // the parent of this fully qualified item and select the item itself.
    unsafe {
        let initialized_com = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if initialized_com.is_err() && initialized_com != RPC_E_CHANGED_MODE {
            return Err(format!(
                "无法初始化 Windows 文件定位环境：{}",
                initialized_com.message()
            ));
        }
        // RPC_E_CHANGED_MODE means this worker already has a usable COM apartment of another
        // type. Continue without uninitializing an apartment owned by the caller.
        let _apartment = ComApartment(initialized_com.is_ok());
        let script_pidl = ILCreateFromPathW(PCWSTR(script_path.as_ptr()));
        if script_pidl.is_null() {
            return Err(format!("无法解析脚本路径：{}", script.display()));
        }

        let result = SHOpenFolderAndSelectItems(script_pidl, None, 0)
            .map_err(|error| format!("无法在文件夹中选中 {}：{error}", script.display()));
        ILFree(Some(script_pidl));
        result
    }
}

#[cfg(target_os = "windows")]
fn windows_shell_path(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    // PathBuf preserves the slash used in a relative config value. Shell PIDL parsing is
    // stricter than ordinary Win32 file APIs, so normalize only separator code units while
    // preserving every non-UTF-8-capable Windows path unit losslessly.
    path.as_os_str()
        .encode_wide()
        .map(|unit| {
            if unit == u16::from(b'/') {
                u16::from(b'\\')
            } else {
                unit
            }
        })
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn reveal_script(_script: &Path) -> Result<(), String> {
    Err("当前平台不支持在文件夹中显示脚本".into())
}

pub fn run_configured<F>(
    app: &AppHandle,
    config: &ScriptCommandConfig,
    args: &[String],
    is_cancelled: F,
) -> Result<ScriptOutput, String>
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
        return collect_script_output(child, &is_cancelled, timeout);
    }

    for interpreter in interpreters {
        let mut command = Command::new(interpreter);
        if matches!(config.runtime, ScriptRuntime::PowerShell) {
            command.args(["-NoProfile", "-NonInteractive", "-File"]);
        }
        configure_plain_text_encoding(&mut command, config.runtime);
        command.arg(&script).args(args);
        if let Some(parent) = script.parent() {
            command.current_dir(parent);
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        configure_process_group(&mut command);
        hide_console(&mut command);
        match command.spawn() {
            Ok(child) => return collect_script_output(child, &is_cancelled, timeout),
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

pub fn run_result_shell<F>(
    app: &AppHandle,
    config: &ScriptCommandConfig,
    command_text: &str,
    is_cancelled: F,
) -> Result<String, String>
where
    F: Fn() -> bool,
{
    ensure_unprivileged()?;
    let command_text = validate_result_shell_command(command_text)?;
    let script = find_script(app, &config.script_path)
        .ok_or_else(|| format!("找不到脚本：{}", config.script_path))?;
    let timeout = Duration::from_millis(config.timeout_ms);
    let mut last_not_found = None;

    for shell in result_shell_candidates() {
        let mut command = result_shell_command(shell, &command_text);
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
            Err(error) => return Err(format!("无法执行返回的 Shell 命令：{error}")),
        }
    }

    Err(format!(
        "未找到用于执行返回值的 Shell：{}",
        last_not_found
            .map(|error| error.to_string())
            .unwrap_or_else(|| "未知错误".into())
    ))
}

pub(crate) fn validate_result_shell_command(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("脚本没有返回可执行的 Shell 命令".into());
    }
    if value.contains('\0') {
        return Err("脚本返回的 Shell 命令包含 NUL 字符".into());
    }
    if value.len() > MAX_RESULT_SHELL_COMMAND_BYTES {
        return Err(format!(
            "脚本返回的 Shell 命令超过 {} KB 上限",
            MAX_RESULT_SHELL_COMMAND_BYTES / 1024
        ));
    }
    Ok(value.to_string())
}

#[cfg(target_os = "windows")]
fn result_shell_candidates() -> &'static [&'static str] {
    &["powershell.exe", "pwsh"]
}

#[cfg(not(target_os = "windows"))]
fn result_shell_candidates() -> &'static [&'static str] {
    &["/bin/bash"]
}

fn result_shell_command(shell: &str, command_text: &str) -> Command {
    let mut command = Command::new(shell);
    #[cfg(target_os = "windows")]
    command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"]);
    #[cfg(not(target_os = "windows"))]
    command.arg("-lc");
    command.arg(command_text);
    command
}

fn collect_output<F>(child: Child, is_cancelled: &F, timeout: Duration) -> Result<String, String>
where
    F: Fn() -> bool,
{
    collect_process_output(child, is_cancelled, timeout)
}

fn collect_script_output<F>(
    child: Child,
    is_cancelled: &F,
    timeout: Duration,
) -> Result<ScriptOutput, String>
where
    F: Fn() -> bool,
{
    let output = collect_process_output(child, is_cancelled, timeout)?;
    parse_script_result_output(&output)
}

fn collect_process_output<F>(
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
        Ok(normalize_plain_text_output(&stdout))
    } else {
        let error = normalize_plain_text_output(&stderr);
        Err(if error.is_empty() {
            format!("脚本退出码：{:?}", status.code())
        } else {
            error
        })
    }
}

fn configure_plain_text_encoding(command: &mut Command, runtime: ScriptRuntime) {
    if matches!(runtime, ScriptRuntime::Python) {
        // Python uses the active Windows code page when stdout/stderr are pipes.
        // Make the text protocol deterministic without changing argv handling.
        command.env("PYTHONIOENCODING", "utf-8");
    }
}

fn decode_plain_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_owned();
    }

    #[cfg(target_os = "windows")]
    if let Some(text) =
        decode_windows_code_page(bytes, unsafe { windows::Win32::Globalization::GetACP() })
    {
        return text;
    }

    String::from_utf8_lossy(bytes).into_owned()
}

fn normalize_plain_text_output(bytes: &[u8]) -> String {
    decode_plain_text(bytes).trim().to_string()
}

fn parse_script_result_output(output: &str) -> Result<ScriptOutput, String> {
    enum RichResult<'a> {
        Image(&'a str),
        QrCode(&'a str),
    }

    let mut found_prefix = false;
    let mut text_lines = Vec::new();
    let mut rich_result = None;

    for line in output.lines() {
        let (text, rich) = if let Some(value) = line.strip_prefix(SCRIPT_TEXT_RESULT_PREFIX) {
            (Some(value.strip_prefix(' ').unwrap_or(value)), None)
        } else if let Some(value) = line.strip_prefix(SCRIPT_IMAGE_RESULT_PREFIX) {
            (None, Some(RichResult::Image(value)))
        } else if let Some(value) = line.strip_prefix(SCRIPT_QR_RESULT_PREFIX) {
            (None, Some(RichResult::QrCode(value)))
        } else if let Some(value) = line.strip_prefix(SCRIPT_RESULT_PREFIX) {
            (Some(value.strip_prefix(' ').unwrap_or(value)), None)
        } else {
            continue;
        };
        found_prefix = true;

        if let Some(text) = text {
            if rich_result.is_some() {
                return Err("一次脚本执行不能同时返回文本和图片".into());
            }
            text_lines.push(text);
        }
        if let Some(rich) = rich {
            if !text_lines.is_empty() {
                return Err("一次脚本执行不能同时返回文本和图片".into());
            }
            if rich_result.is_some() {
                return Err("一次脚本执行只能返回一张图片".into());
            }
            rich_result = Some(rich);
        }
    }

    if !found_prefix {
        return Ok(ScriptOutput::Text(output.trim().to_string()));
    }
    if let Some(result) = rich_result {
        return match result {
            RichResult::Image(value) => parse_image_result(value),
            RichResult::QrCode(value) => generate_qr_result(value),
        };
    }
    Ok(ScriptOutput::Text(text_lines.join("\n").trim().to_string()))
}

fn parse_image_result(value: &str) -> Result<ScriptOutput, String> {
    let value = value.trim();
    let data_url = if value.starts_with("data:") {
        value.to_string()
    } else {
        // A raw payload has no MIME metadata. Treat it as PNG so the protocol stays concise
        // while JPEG/WebP callers use an explicit data URL.
        format!("data:image/png;base64,{value}")
    };
    validate_script_result_image_data_url(&data_url)?;
    Ok(ScriptOutput::Image {
        copy_text: data_url.clone(),
        data_url,
    })
}

fn generate_qr_result(value: &str) -> Result<ScriptOutput, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("二维码内容不能为空".into());
    }
    if value.len() > MAX_QR_CONTENT_BYTES {
        return Err(format!("二维码内容不能超过 {MAX_QR_CONTENT_BYTES} 字节"));
    }

    let code = generate_utf8_qr_code(value)?;
    let image = code
        .render::<Luma<u8>>()
        // Keep a high-quality source image. The launcher independently scales
        // all script images into a complete, height-aware thumbnail.
        .max_dimensions(QR_IMAGE_PIXELS, QR_IMAGE_PIXELS)
        .build();
    let mut png = Cursor::new(Vec::new());
    DynamicImage::ImageLuma8(image)
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|error| format!("无法编码二维码图片：{error}"))?;
    let data_url = format!(
        "data:image/png;base64,{}",
        BASE64_STANDARD.encode(png.into_inner())
    );
    validate_script_result_image_data_url(&data_url)?;
    Ok(ScriptOutput::Image {
        data_url,
        copy_text: value.to_string(),
    })
}

fn generate_utf8_qr_code(value: &str) -> Result<QrCode, String> {
    // `QrCode::new` treats input as opaque bytes and emits no character-set
    // declaration. Prefix byte mode with ECI 26 so standards-compliant readers
    // decode Chinese, emoji, and other non-ASCII content as UTF-8.
    for version in 1..=40 {
        let Ok(bits) = utf8_qr_bits(value, Version::Normal(version)) else {
            continue;
        };
        if let Ok(code) = QrCode::with_bits(bits, EcLevel::M) {
            return Ok(code);
        }
    }
    Err("无法生成二维码：内容超过二维码容量".into())
}

fn utf8_qr_bits(value: &str, version: Version) -> QrResult<Bits> {
    let mut bits = Bits::new(version);
    bits.push_eci_designator(UTF8_ECI_DESIGNATOR)?;
    bits.push_byte_data(value.as_bytes())?;
    bits.push_terminator(EcLevel::M)?;
    Ok(bits)
}

#[cfg(target_os = "windows")]
fn decode_windows_code_page(bytes: &[u8], code_page: u32) -> Option<String> {
    use windows::Win32::Globalization::{MultiByteToWideChar, MULTI_BYTE_TO_WIDE_CHAR_FLAGS};

    if bytes.is_empty() {
        return Some(String::new());
    }
    let required =
        unsafe { MultiByteToWideChar(code_page, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, None) };
    if required <= 0 {
        return None;
    }
    let mut wide = vec![0_u16; required as usize];
    let written = unsafe {
        MultiByteToWideChar(
            code_page,
            MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
            bytes,
            Some(&mut wide),
        )
    };
    if written <= 0 {
        return None;
    }
    wide.truncate(written as usize);
    String::from_utf16(&wide).ok()
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

    let config_relative = app
        .path()
        .app_config_dir()
        .ok()
        .map(|directory| directory.join(&expanded));
    if expanded.starts_with(Path::new("scripts")) {
        let bundled_template = bundled_template_path(&expanded);
        let bundled_resource = bundled_template
            .as_ref()
            .and_then(|path| app.path().resolve(path, BaseDirectory::Resource).ok());
        let bundled_source_tree = bundled_template.map(|path| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(path)
        });

        return config_relative
            .into_iter()
            .chain(bundled_resource)
            .chain(bundled_source_tree)
            .find(|path| path.is_file());
    }

    let resource = app.path().resolve(&expanded, BaseDirectory::Resource).ok();
    let source_tree = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(&expanded);

    resource
        .into_iter()
        .chain(config_relative)
        .chain([source_tree])
        .find(|path| path.is_file())
}

fn bundled_template_path(path: &Path) -> Option<PathBuf> {
    if path.parent() != Some(Path::new("scripts")) {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    matches!(
        name,
        "timestamp.py" | "qr.py" | "open_path.py" | "script_template.py"
    )
    .then(|| Path::new("examples").join(name))
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
pub(crate) fn ensure_unprivileged() -> Result<(), String> {
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
            return Err("Suo 正以管理员权限运行，已拒绝执行终端命令或自定义脚本".into());
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn ensure_unprivileged() -> Result<(), String> {
    // SAFETY: geteuid has no preconditions and does not dereference pointers.
    if unsafe { libc::geteuid() } == 0 {
        Err("Suo 正以 root 权限运行，已拒绝执行终端命令或自定义脚本".into())
    } else {
        Ok(())
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(crate) fn ensure_unprivileged() -> Result<(), String> {
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

    #[cfg(unix)]
    use super::configure_process_group;
    #[cfg(target_os = "windows")]
    use super::hide_console;
    use super::{
        bundled_template_path, collect_output, configure_plain_text_encoding,
        generate_utf8_qr_code, normalize_plain_text_output, parse_script_result_output,
        result_shell_command, spawn_capped_reader, utf8_qr_bits, validate_result_shell_command,
        ScriptOutput, MAX_OUTPUT_BYTES, MAX_QR_CONTENT_BYTES, MAX_RESULT_SHELL_COMMAND_BYTES,
    };

    #[test]
    fn only_known_user_script_templates_have_bundled_fallbacks() {
        assert_eq!(
            bundled_template_path(std::path::Path::new("scripts/timestamp.py")),
            Some(std::path::PathBuf::from("examples/timestamp.py"))
        );
        assert_eq!(
            bundled_template_path(std::path::Path::new("scripts/qr.py")),
            Some(std::path::PathBuf::from("examples/qr.py"))
        );
        assert!(bundled_template_path(std::path::Path::new("scripts/custom.py")).is_none());
        assert!(bundled_template_path(std::path::Path::new("other/timestamp.py")).is_none());
    }

    #[test]
    fn plain_text_preserves_utf8_chinese_and_internal_newlines() {
        assert_eq!(
            normalize_plain_text_output(
                "\nUTC+8 当前时间：2026-09-09\nUnix时间戳：1788958239819\r\n".as_bytes()
            ),
            "UTC+8 当前时间：2026-09-09\nUnix时间戳：1788958239819"
        );
    }

    #[test]
    fn prefixed_script_results_exclude_unmarked_stdout_logs() {
        assert_eq!(
            parse_script_result_output(
                "starting conversion\nSUO_RESULT:text: 第一行\nloaded cache\nSUO_RESULT:第二行\ndone"
            )
            .unwrap(),
            ScriptOutput::Text("第一行\n第二行".into())
        );
    }

    #[test]
    fn scripts_without_a_result_prefix_keep_legacy_stdout_behavior() {
        let output = "legacy first line\nlegacy second line";
        assert_eq!(
            parse_script_result_output(output).unwrap(),
            ScriptOutput::Text("legacy first line\nlegacy second line".into())
        );
        assert_eq!(
            normalize_plain_text_output(b"error\nSUO_RESULT: remains an error detail"),
            "error\nSUO_RESULT: remains an error detail"
        );
    }

    #[test]
    fn typed_qr_results_become_valid_local_png_images() {
        let output = parse_script_result_output(
            "debug log\nSUO_RESULT:qrcode: https://www.google.com\nfinished",
        )
        .expect("render QR result");
        let ScriptOutput::Image {
            data_url,
            copy_text,
        } = output
        else {
            panic!("expected image output");
        };
        assert!(data_url.starts_with("data:image/png;base64,"));
        assert_eq!(copy_text, "https://www.google.com");

        let explicit_image = parse_script_result_output(&format!("SUO_RESULT:image: {data_url}"))
            .expect("accept validated image data URL");
        assert!(matches!(
            explicit_image,
            ScriptOutput::Image { data_url: ref parsed, .. } if parsed == &data_url
        ));

        let raw_png = data_url
            .strip_prefix("data:image/png;base64,")
            .expect("generated PNG prefix");
        assert!(matches!(
            parse_script_result_output(&format!("SUO_RESULT:image: {raw_png}"))
                .expect("accept raw PNG base64"),
            ScriptOutput::Image { data_url: ref parsed, .. } if parsed == &data_url
        ));
    }

    #[test]
    fn qr_results_declare_utf8_and_bound_preview_density() {
        let unicode = "你好，Suo 🚀";
        let code = generate_utf8_qr_code(unicode).expect("encode Unicode QR result");
        assert!(code.version().width() <= 85);

        // Normal QR mode starts with ECI mode 0111, designator 26 (00011010),
        // then byte mode 0100. These first 16 bits prove the UTF-8 declaration
        // is present before the Unicode payload.
        let bits = utf8_qr_bits(unicode, qrcode::Version::Normal(2)).unwrap();
        assert_eq!(&bits.into_bytes()[..2], &[0x71, 0xA4]);

        let maximum = "x".repeat(MAX_QR_CONTENT_BYTES);
        let maximum_code = generate_utf8_qr_code(&maximum).unwrap();
        assert!(maximum_code.version().width() <= 85);
        let preview = maximum_code
            .render::<image::Luma<u8>>()
            .max_dimensions(384, 384)
            .build();
        let modules_with_quiet_zone = maximum_code.width() as u32 + 8;
        assert!(preview.width() / modules_with_quiet_zone >= 4);
        assert!(parse_script_result_output(&format!(
            "SUO_RESULT:qrcode: {}",
            "x".repeat(MAX_QR_CONTENT_BYTES + 1)
        ))
        .unwrap_err()
        .contains("500"));
    }

    #[test]
    fn typed_results_reject_mixed_or_invalid_image_content() {
        assert!(parse_script_result_output(
            "SUO_RESULT:text: caption\nSUO_RESULT:image: not-base64"
        )
        .unwrap_err()
        .contains("不能同时"));
        assert!(parse_script_result_output("SUO_RESULT:image: not-base64")
            .unwrap_err()
            .contains("无效"));
        assert!(
            parse_script_result_output("SUO_RESULT:qrcode: one\nSUO_RESULT:qrcode: two")
                .unwrap_err()
                .contains("只能返回一张")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_shell_paths_normalize_mixed_separators_without_losing_unicode() {
        use std::os::windows::ffi::OsStringExt;

        let encoded =
            super::windows_shell_path(std::path::Path::new(r"C:\Users\测试/scripts/timestamp.py"));
        assert_eq!(encoded.last(), Some(&0));
        assert!(!encoded[..encoded.len() - 1].contains(&u16::from(b'/')));
        let decoded = std::ffi::OsString::from_wide(&encoded[..encoded.len() - 1]);
        assert_eq!(decoded, r"C:\Users\测试\scripts\timestamp.py");
    }

    #[test]
    fn python_plain_text_requests_utf8_for_piped_output() {
        let mut command = Command::new("python");
        configure_plain_text_encoding(&mut command, crate::config::ScriptRuntime::Python);
        assert!(command.get_envs().any(|(key, value)| {
            key == "PYTHONIOENCODING" && value == Some(std::ffi::OsStr::new("utf-8"))
        }));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_plain_text_can_decode_gbk_fallback() {
        let gbk = [
            0x55, 0x54, 0x43, 0x2B, 0x38, 0x20, 0xB5, 0xB1, 0xC7, 0xB0, 0xCA, 0xB1, 0xBC, 0xE4,
            0xA3, 0xBA, 0x32, 0x30, 0x32, 0x36,
        ];
        assert_eq!(
            super::decode_windows_code_page(&gbk, 936).as_deref(),
            Some("UTC+8 当前时间：2026")
        );
    }

    #[test]
    fn result_shell_command_validation_is_bounded() {
        assert_eq!(
            validate_result_shell_command("  open ~/  ").unwrap(),
            "open ~/"
        );
        assert!(validate_result_shell_command(" \n\t ").is_err());
        assert!(validate_result_shell_command("printf 'x\0y'").is_err());
        assert!(
            validate_result_shell_command(&"x".repeat(MAX_RESULT_SHELL_COMMAND_BYTES + 1)).is_err()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_result_shell_uses_bash_login_command_mode() {
        let command = result_shell_command("/bin/bash", "open ~/");
        assert_eq!(command.get_program(), "/bin/bash");
        assert_eq!(command.get_args().collect::<Vec<_>>(), ["-lc", "open ~/"]);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_result_shell_uses_powershell_command_mode() {
        let command = result_shell_command("powershell.exe", "Start-Process C:\\\\");
        assert_eq!(command.get_program(), "powershell.exe");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                r"Start-Process C:\\",
            ]
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
