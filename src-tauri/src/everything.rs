use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};

use tauri::{path::BaseDirectory, AppHandle, Manager};

use crate::catalog::CatalogEntry;

const PROCESS_TIMEOUT: Duration = Duration::from_millis(700);
const UNAVAILABLE_CACHE_TTL: Duration = Duration::from_secs(2);
static UNAVAILABLE_UNTIL: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
static EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub enum EverythingOutcome {
    Available(Vec<CatalogEntry>),
    Unavailable(String),
    Cancelled,
}

pub fn search<F>(
    app: &AppHandle,
    query: &str,
    max_results: usize,
    is_cancelled: F,
) -> EverythingOutcome
where
    F: Fn() -> bool,
{
    if is_recently_unavailable() {
        return EverythingOutcome::Unavailable("Everything IPC 暂不可用（短期缓存）".into());
    }

    let Some(es_path) = find_es(app) else {
        return EverythingOutcome::Unavailable("未找到 Everything ES 客户端".into());
    };

    // Always prefix the search term so user text cannot be interpreted as an ES option.
    let safe_query = format!("*{}*", query.trim());
    let export = TempExport::new();
    let mut command = Command::new(es_path);
    command
        .arg("-n")
        .arg(max_results.to_string())
        .arg("-match-path")
        .arg("-full-path-and-name")
        .arg("-timeout")
        .arg("300")
        .arg("-export-txt")
        .arg(export.path())
        .arg(&safe_query);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    hide_console(&mut command);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            remember_unavailable();
            return EverythingOutcome::Unavailable(format!("无法启动 Everything ES：{error}"));
        }
    };

    let started = Instant::now();
    let output = loop {
        if is_cancelled() {
            terminate(&mut child);
            return EverythingOutcome::Cancelled;
        }

        match child.try_wait() {
            Ok(Some(_)) => match child.wait_with_output() {
                Ok(output) => break output,
                Err(error) => {
                    remember_unavailable();
                    return EverythingOutcome::Unavailable(format!(
                        "无法读取 Everything ES 输出：{error}"
                    ));
                }
            },
            Ok(None) if started.elapsed() < PROCESS_TIMEOUT => {
                thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                terminate(&mut child);
                remember_unavailable();
                return EverythingOutcome::Unavailable("Everything ES 响应超时".into());
            }
            Err(error) => {
                terminate(&mut child);
                remember_unavailable();
                return EverythingOutcome::Unavailable(format!(
                    "无法检查 Everything ES 状态：{error}"
                ));
            }
        }
    };

    if !output.status.success() {
        remember_unavailable();
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return EverythingOutcome::Unavailable(if detail.is_empty() {
            format!("Everything IPC 不可用（退出码 {:?}）", output.status.code())
        } else {
            detail
        });
    }

    // ES documents TXT exports as UTF-8. Exporting avoids corrupting paths
    // when a redirected stdout stream uses the active Windows console code page.
    let exported = match fs::read(export.path()) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(value) => value,
            Err(error) => {
                return EverythingOutcome::Unavailable(format!(
                    "Everything 返回了无效 UTF-8：{error}"
                ));
            }
        },
        Err(error) => {
            return EverythingOutcome::Unavailable(format!(
                "无法读取 Everything 搜索结果：{error}"
            ));
        }
    };
    let entries = exported
        .trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(catalog_entry_from_export)
        .collect();

    EverythingOutcome::Available(entries)
}

/// Everything marks directory results by retaining a trailing path separator
/// in its TXT export. Preserve that fact before constructing `PathBuf`: an
/// index hit may be stale or temporarily inaccessible, so `Path::is_dir()`
/// alone would otherwise turn a real directory into a file result.
fn catalog_entry_from_export(line: &str) -> CatalogEntry {
    let is_directory = line.ends_with(['\\', '/']);
    CatalogEntry::from_path_with_type(PathBuf::from(line), is_directory)
}

struct TempExport {
    path: PathBuf,
}

impl TempExport {
    fn new() -> Self {
        let sequence = EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("suo-es-{}-{sequence}.txt", std::process::id()));
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempExport {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn unavailable_cache() -> &'static Mutex<Option<Instant>> {
    UNAVAILABLE_UNTIL.get_or_init(|| Mutex::new(None))
}

fn is_recently_unavailable() -> bool {
    unavailable_cache()
        .lock()
        .ok()
        .and_then(|until| *until)
        .is_some_and(|until| Instant::now() < until)
}

fn remember_unavailable() {
    if let Ok(mut until) = unavailable_cache().lock() {
        *until = Some(Instant::now() + UNAVAILABLE_CACHE_TTL);
    }
}

fn terminate(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn find_es(app: &AppHandle) -> Option<PathBuf> {
    let resource = app
        .path()
        .resolve("tools/es.exe", BaseDirectory::Resource)
        .ok();
    let source_tree =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../third_party/voidtools/es.exe");

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
    use super::catalog_entry_from_export;

    #[test]
    fn keeps_everything_trailing_separator_directory_hint() {
        let directory = catalog_entry_from_export(r"C:\indexed\folder\");
        let file = catalog_entry_from_export(r"C:\indexed\report.txt");

        assert!(directory.is_directory);
        assert!(!file.is_directory);
    }
}
