use std::{
    io::{BufRead, BufReader, Read},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use crate::catalog::CatalogEntry;

const PROCESS_TIMEOUT: Duration = Duration::from_millis(900);

pub enum SpotlightOutcome {
    Available(Vec<CatalogEntry>),
    Unavailable(String),
    Cancelled,
}

pub fn search<F>(query: &str, max_results: usize, is_cancelled: F) -> SpotlightOutcome
where
    F: Fn() -> bool,
{
    if let Some(reason) = unavailable_index_reason() {
        return SpotlightOutcome::Unavailable(reason);
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
            let mut entries = Vec::new();
            let mut sent = false;
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let line = line.trim();
                if !line.is_empty() && entries.len() < max_results {
                    entries.push(CatalogEntry::from_path(std::path::PathBuf::from(line)));
                    if entries.len() == max_results {
                        let _ = sender.send(entries.clone());
                        sent = true;
                    }
                }
            }
            if !sent {
                let _ = sender.send(entries);
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
                    return SpotlightOutcome::Available(entries);
                }
                completed_entries = Some(entries);
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < PROCESS_TIMEOUT => {
                thread::sleep(Duration::from_millis(10))
            }
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
    SpotlightOutcome::Available(entries)
}

fn unavailable_index_reason() -> Option<String> {
    let output = Command::new("/usr/bin/mdutil")
        .args(["-s", "/"])
        .output()
        .ok()?;
    let detail = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    disabled_index_reason(&detail)
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
    use super::disabled_index_reason;

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
}
