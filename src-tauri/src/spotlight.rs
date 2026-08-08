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

fn terminate(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}
