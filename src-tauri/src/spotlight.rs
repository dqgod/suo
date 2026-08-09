use std::{
    io::{self, BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use crate::catalog::CatalogEntry;

const PROCESS_TIMEOUT: Duration = Duration::from_millis(900);
const INDEX_STATUS_TIMEOUT: Duration = Duration::from_millis(150);
const METADATA_TIMEOUT: Duration = Duration::from_millis(240);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);

enum BoundedCommandOutcome {
    Completed {
        success: bool,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    Cancelled,
    TimedOut,
    Unavailable,
}

enum IndexStatusOutcome {
    Continue,
    Disabled(String),
    Cancelled,
}

enum MetadataOutcome {
    Value(bool),
    Unavailable,
    TimedOut,
    Cancelled,
}

enum PipeReadFailure {
    Cancelled,
    TimedOut,
    Unavailable,
}

pub enum SpotlightOutcome {
    Available(Vec<CatalogEntry>),
    Unavailable(String),
    Cancelled,
}

pub fn search<F>(query: &str, max_results: usize, is_cancelled: F) -> SpotlightOutcome
where
    F: Fn() -> bool,
{
    match unavailable_index_reason(&is_cancelled) {
        IndexStatusOutcome::Disabled(reason) => return SpotlightOutcome::Unavailable(reason),
        IndexStatusOutcome::Cancelled => return SpotlightOutcome::Cancelled,
        IndexStatusOutcome::Continue => {}
    }

    // -name keeps this launcher search filename-oriented instead of requesting
    // full-text document contents. User input remains a separate process arg.
    let mut command = Command::new("/usr/bin/mdfind");
    command
        .arg("-name")
        .arg(query.trim())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return SpotlightOutcome::Unavailable(format!("无法启动 Spotlight：{error}"));
        }
    };
    let stdout = child.stdout.take().map(|stdout| {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut paths = Vec::new();
            let mut sent = false;
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let line = line.trim();
                if !line.is_empty() && paths.len() < max_results {
                    paths.push(PathBuf::from(line));
                    if paths.len() == max_results {
                        let _ = sender.send(paths.clone());
                        sent = true;
                    }
                }
            }
            if !sent {
                let _ = sender.send(paths);
            }
        });
        receiver
    });
    let stderr = child.stderr.take().map(|mut stderr| {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut detail = String::new();
            let _ = stderr.read_to_string(&mut detail);
            let _ = sender.send(detail);
        });
        receiver
    });
    let started = Instant::now();
    let mut completed_entries = None;
    let status = loop {
        if is_cancelled() {
            terminate(&mut child);
            return SpotlightOutcome::Cancelled;
        }
        if let Some(receiver) = &stdout {
            if let Ok(entries) = receiver.try_recv() {
                if entries.len() >= max_results {
                    terminate(&mut child);
                    return match catalog_entries_from_spotlight_paths(
                        entries,
                        &is_cancelled,
                        Instant::now() + METADATA_TIMEOUT,
                    ) {
                        Ok(entries) => SpotlightOutcome::Available(entries),
                        Err(()) => SpotlightOutcome::Cancelled,
                    };
                }
                completed_entries = Some(entries);
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < PROCESS_TIMEOUT => thread::sleep(PROCESS_POLL_INTERVAL),
            Ok(None) => {
                terminate(&mut child);
                return SpotlightOutcome::Unavailable("Spotlight 响应超时".into());
            }
            Err(error) => {
                terminate(&mut child);
                return SpotlightOutcome::Unavailable(format!("无法检查 Spotlight 状态：{error}"));
            }
        }
    };
    let entries = completed_entries.unwrap_or_else(|| {
        stdout
            .and_then(|receiver| receiver.recv_timeout(Duration::from_millis(250)).ok())
            .unwrap_or_default()
    });
    let detail = stderr
        .and_then(|receiver| receiver.recv_timeout(Duration::from_millis(250)).ok())
        .unwrap_or_default();
    if !status.success() {
        let detail = detail.trim().to_string();
        return SpotlightOutcome::Unavailable(if detail.is_empty() {
            format!("Spotlight 退出码：{:?}", status.code())
        } else {
            detail
        });
    }
    match catalog_entries_from_spotlight_paths(
        entries,
        &is_cancelled,
        Instant::now() + METADATA_TIMEOUT,
    ) {
        Ok(entries) => SpotlightOutcome::Available(entries),
        Err(()) => SpotlightOutcome::Cancelled,
    }
}

fn catalog_entries_from_spotlight_paths<F>(
    paths: Vec<PathBuf>,
    is_cancelled: &F,
    metadata_deadline: Instant,
) -> Result<Vec<CatalogEntry>, ()>
where
    F: Fn() -> bool,
{
    let mut entries = Vec::with_capacity(paths.len());
    for path in paths {
        if is_cancelled() {
            return Err(());
        }
        entries.push(catalog_entry_from_spotlight_path(
            path,
            is_cancelled,
            metadata_deadline,
        )?);
    }
    Ok(entries)
}

fn catalog_entry_from_spotlight_path<F>(
    path: PathBuf,
    is_cancelled: &F,
    metadata_deadline: Instant,
) -> Result<CatalogEntry, ()>
where
    F: Fn() -> bool,
{
    // `mdfind` emits only paths. Ask Spotlight for the directory bit rather
    // than guessing from an extension. All result metadata shares one budget
    // so a slow volume cannot make a 12-result search take 12 timeouts.
    let is_directory = match spotlight_directory_metadata(&path, metadata_deadline, is_cancelled) {
        MetadataOutcome::Value(is_directory) => is_directory,
        MetadataOutcome::Unavailable => {
            // A quick, ordinary `mdls` failure can be caused by stale Spotlight
            // data. Retain the previous filesystem fallback for that case.
            if is_cancelled() {
                return Err(());
            }
            path.is_dir()
        }
        // Never call `Path::is_dir()` after the metadata deadline: it could
        // block on the same unavailable volume that caused `mdls` to time out.
        // The result remains launchable and uses the conservative file icon.
        MetadataOutcome::TimedOut => false,
        MetadataOutcome::Cancelled => return Err(()),
    };
    Ok(CatalogEntry::from_path_with_type(path, is_directory))
}

fn spotlight_directory_metadata<F>(
    path: &Path,
    deadline: Instant,
    is_cancelled: &F,
) -> MetadataOutcome
where
    F: Fn() -> bool,
{
    let mut command = Command::new("/usr/bin/mdls");
    command
        .args(["-raw", "-name", "kMDItemFSIsDirectory"])
        .arg(path)
        .stdout(Stdio::piped())
        // `mdls` diagnostics are not needed for the fallback decision. Do not
        // keep an error pipe that a misbehaving helper could fill.
        .stderr(Stdio::null());
    match run_bounded_command(&mut command, deadline, is_cancelled) {
        BoundedCommandOutcome::Completed {
            success: true,
            stdout,
            ..
        } => parse_directory_metadata(&String::from_utf8_lossy(&stdout))
            .map_or(MetadataOutcome::Unavailable, MetadataOutcome::Value),
        BoundedCommandOutcome::Completed { .. } | BoundedCommandOutcome::Unavailable => {
            MetadataOutcome::Unavailable
        }
        BoundedCommandOutcome::TimedOut => MetadataOutcome::TimedOut,
        BoundedCommandOutcome::Cancelled => MetadataOutcome::Cancelled,
    }
}

fn parse_directory_metadata(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

fn unavailable_index_reason<F>(is_cancelled: &F) -> IndexStatusOutcome
where
    F: Fn() -> bool,
{
    let mut command = Command::new("/usr/bin/mdutil");
    command
        .args(["-s", "/"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match run_bounded_command(
        &mut command,
        Instant::now() + INDEX_STATUS_TIMEOUT,
        is_cancelled,
    ) {
        BoundedCommandOutcome::Completed { stdout, stderr, .. } => {
            let detail = format!(
                "{}\n{}",
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
            disabled_index_reason(&detail)
                .map_or(IndexStatusOutcome::Continue, IndexStatusOutcome::Disabled)
        }
        // The previous best-effort probe simply continued when mdutil could
        // not be started/read. Preserve that fallback, but keep the request
        // bounded rather than allowing an unavailable helper to hang search.
        BoundedCommandOutcome::TimedOut | BoundedCommandOutcome::Unavailable => {
            IndexStatusOutcome::Continue
        }
        BoundedCommandOutcome::Cancelled => IndexStatusOutcome::Cancelled,
    }
}

fn run_bounded_command<F>(
    command: &mut Command,
    deadline: Instant,
    is_cancelled: &F,
) -> BoundedCommandOutcome
where
    F: Fn() -> bool,
{
    if is_cancelled() {
        return BoundedCommandOutcome::Cancelled;
    }
    if Instant::now() >= deadline {
        return BoundedCommandOutcome::TimedOut;
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return BoundedCommandOutcome::Unavailable,
    };
    let stdout = child.stdout.take().map(spawn_output_reader);
    let stderr = child.stderr.take().map(spawn_output_reader);
    loop {
        if is_cancelled() {
            terminate(&mut child);
            return BoundedCommandOutcome::Cancelled;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = match collect_piped_output(stdout.as_ref(), deadline, is_cancelled) {
                    Ok(output) => output,
                    Err(PipeReadFailure::Cancelled) => return BoundedCommandOutcome::Cancelled,
                    Err(PipeReadFailure::TimedOut) => return BoundedCommandOutcome::TimedOut,
                    Err(PipeReadFailure::Unavailable) => {
                        return BoundedCommandOutcome::Unavailable;
                    }
                };
                let stderr = match collect_piped_output(stderr.as_ref(), deadline, is_cancelled) {
                    Ok(output) => output,
                    Err(PipeReadFailure::Cancelled) => return BoundedCommandOutcome::Cancelled,
                    Err(PipeReadFailure::TimedOut) => return BoundedCommandOutcome::TimedOut,
                    Err(PipeReadFailure::Unavailable) => {
                        return BoundedCommandOutcome::Unavailable;
                    }
                };
                return BoundedCommandOutcome::Completed {
                    success: status.success(),
                    stdout,
                    stderr,
                };
            }
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(
                    PROCESS_POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
                );
            }
            Ok(None) => {
                terminate(&mut child);
                return BoundedCommandOutcome::TimedOut;
            }
            Err(_) => {
                terminate(&mut child);
                return BoundedCommandOutcome::Unavailable;
            }
        }
    }
}

/// Drains a small, bounded system-helper pipe off the search task. The main
/// task still observes cancellation and the shared deadline while waiting for
/// the reader, so an inherited descriptor held by a helper descendant cannot
/// turn an exited child into an unbounded synchronous read.
fn spawn_output_reader<R>(mut pipe: R) -> mpsc::Receiver<io::Result<Vec<u8>>>
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut output = Vec::new();
        let _ = sender.send(pipe.read_to_end(&mut output).map(|_| output));
    });
    receiver
}

fn collect_piped_output<F>(
    receiver: Option<&mpsc::Receiver<io::Result<Vec<u8>>>>,
    deadline: Instant,
    is_cancelled: &F,
) -> Result<Vec<u8>, PipeReadFailure>
where
    F: Fn() -> bool,
{
    let Some(receiver) = receiver else {
        return Ok(Vec::new());
    };

    loop {
        if is_cancelled() {
            return Err(PipeReadFailure::Cancelled);
        }
        match receiver.try_recv() {
            Ok(output) => return output.map_err(|_| PipeReadFailure::Unavailable),
            Err(mpsc::TryRecvError::Disconnected) => return Err(PipeReadFailure::Unavailable),
            Err(mpsc::TryRecvError::Empty) => {}
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(PipeReadFailure::TimedOut);
        }
        match receiver.recv_timeout(PROCESS_POLL_INTERVAL.min(remaining)) {
            Ok(output) => return output.map_err(|_| PipeReadFailure::Unavailable),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(PipeReadFailure::Unavailable),
        }
    }
}

fn disabled_index_reason(detail: &str) -> Option<String> {
    let normalized = detail.to_lowercase();
    if normalized.contains("spotlight server is disabled")
        || normalized.contains("indexing disabled")
        || normalized.contains("spotlight 服务器已停用")
        || normalized.contains("索引已停用")
    {
        Some("本机 Spotlight 索引已停用".into())
    } else {
        None
    }
}

fn terminate(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use std::{
        process::{Command, Stdio},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread,
        time::{Duration, Instant},
    };

    use super::{
        disabled_index_reason, parse_directory_metadata, run_bounded_command, BoundedCommandOutcome,
    };

    #[test]
    fn detects_disabled_spotlight_status() {
        assert_eq!(
            disabled_index_reason("Spotlight server is disabled."),
            Some("本机 Spotlight 索引已停用".into())
        );
        assert_eq!(
            disabled_index_reason("/:\n\tIndexing disabled."),
            Some("本机 Spotlight 索引已停用".into())
        );
    }

    #[test]
    fn accepts_enabled_spotlight_status() {
        assert_eq!(disabled_index_reason("/:\n\tIndexing enabled."), None);
    }

    #[test]
    fn parses_spotlight_directory_metadata() {
        assert_eq!(parse_directory_metadata("1\n"), Some(true));
        assert_eq!(parse_directory_metadata("false"), Some(false));
        assert_eq!(parse_directory_metadata("(null)"), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn bounded_metadata_commands_observe_cancellation_promptly() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancellation = cancelled.clone();
        let signal = thread::spawn(move || {
            thread::sleep(Duration::from_millis(25));
            cancellation.store(true, Ordering::SeqCst);
        });
        let mut command = Command::new("/bin/sleep");
        command.arg("1").stdout(Stdio::null()).stderr(Stdio::null());

        let started = Instant::now();
        let outcome = run_bounded_command(
            &mut command,
            Instant::now() + Duration::from_secs(1),
            &|| cancelled.load(Ordering::SeqCst),
        );
        signal.join().expect("join cancellation signal");

        assert!(matches!(outcome, BoundedCommandOutcome::Cancelled));
        assert!(started.elapsed() < Duration::from_millis(250));
    }
}
