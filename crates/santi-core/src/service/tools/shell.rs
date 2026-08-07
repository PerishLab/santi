use std::{
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::service::interrupt::Control;

mod definition;
mod redact;

pub(super) fn definition() -> santi_provider::Tool {
    definition::tool()
}

#[derive(Debug, Deserialize)]
pub(super) struct Args {
    pub(super) command: String,
    pub(super) cwd: Option<String>,
}

pub(super) struct Prepared {
    pub(super) command: Command,
    pub(super) cwd: PathBuf,
}

pub(super) enum Outcome {
    Captured(Value),
    Failed(String),
    Unknown(String),
    Stopped(String),
}

pub(super) async fn ran(
    prepared: Prepared,
    output_limit: Option<usize>,
    control: &Control,
) -> Outcome {
    let Prepared { command, cwd } = prepared;
    let mut command = tokio::process::Command::from(command);
    command.kill_on_drop(true);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Outcome::Failed(format!("failed to spawn shell process: {error}"));
        }
    };
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        let _ = terminate(&mut child).await;
        return Outcome::Unknown("shell stdout or stderr pipe was unavailable".to_string());
    };
    let remaining = output_limit.map(|limit| Arc::new(AtomicUsize::new(limit)));
    let stdout = tokio::spawn(piped(stdout, remaining.clone()));
    let stderr = tokio::spawn(piped(stderr, remaining));
    let mut stopped = None;
    let status = tokio::select! {
        status = child.wait() => status,
        cause = control.wait() => {
            stopped = Some(cause);
            terminate(&mut child).await
        }
    };
    let stdout = joined(stdout, "stdout").await;
    let stderr = joined(stderr, "stderr").await;
    if let Some(cause) = stopped {
        return Outcome::Stopped(format!(
            "shell process tree interrupted by {}",
            cause.encode()
        ));
    }
    let (status, stdout, stderr) = match (status, stdout, stderr) {
        (Ok(status), Ok(stdout), Ok(stderr)) => (status, stdout, stderr),
        (status, stdout, stderr) => {
            return Outcome::Unknown(format!(
                "shell process was spawned but its result could not be captured: status={}; stdout={}; stderr={}",
                shown(status),
                shown(stdout),
                shown(stderr),
            ));
        }
    };
    captured(Capture {
        status,
        stdout,
        stderr,
        cwd,
        limit: output_limit,
    })
}

struct Pipe {
    bytes: Vec<u8>,
    truncated: bool,
}

struct Text {
    value: String,
    truncated: bool,
    redactions: usize,
}

async fn piped(
    mut reader: impl AsyncRead + Unpin,
    remaining: Option<Arc<AtomicUsize>>,
) -> Result<Pipe, String> {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut chunk = [0u8; 8192];
    loop {
        let read = reader
            .read(&mut chunk)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        let keep = remaining
            .as_ref()
            .map_or(read, |remaining| reserved(remaining, read));
        bytes.extend_from_slice(&chunk[..keep]);
        truncated |= keep < read;
    }
    Ok(Pipe { bytes, truncated })
}

struct Capture {
    status: std::process::ExitStatus,
    stdout: Pipe,
    stderr: Pipe,
    cwd: PathBuf,
    limit: Option<usize>,
}

fn captured(capture: Capture) -> Outcome {
    let Capture {
        status,
        stdout,
        stderr,
        cwd,
        limit,
    } = capture;
    let mut remaining = limit;
    let stdout = redacted(normalized(stdout, remaining.as_mut()));
    let stderr = redacted(normalized(stderr, remaining.as_mut()));
    let redactions = stdout.redactions + stderr.redactions;
    let mut output = json!({
        "exit_code": status.code().unwrap_or(-1),
        "stdout": stdout.value,
        "stderr": stderr.value,
        "shell": sheller(),
        "cwd": cwd.display().to_string(),
    });
    if let Some(limit) = limit {
        output["output_truncated"] = Value::Bool(stdout.truncated || stderr.truncated);
        output["output_limit_bytes"] = Value::from(limit);
    }
    if redactions > 0 {
        output["redacted"] = Value::Bool(true);
        output["redactions"] = Value::from(redactions);
    }
    Outcome::Captured(output)
}

fn normalized(pipe: Pipe, remaining: Option<&mut usize>) -> Text {
    let Pipe { bytes, truncated } = pipe;
    let value = String::from_utf8_lossy(&bytes);
    let Some(remaining) = remaining else {
        return Text {
            value: value.into_owned(),
            truncated,
            redactions: 0,
        };
    };
    let mut end = value.len().min(*remaining);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    *remaining -= end;
    Text {
        truncated: truncated || end < value.len(),
        value: value[..end].to_string(),
        redactions: 0,
    }
}

fn redacted(mut text: Text) -> Text {
    let ceiling = text.value.len();
    let (mut value, redactions) = redact::text(&text.value);
    if value.len() > ceiling {
        let mut end = ceiling;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value.truncate(end);
        text.truncated = true;
    }
    text.value = value;
    text.redactions = redactions;
    text
}

fn reserved(remaining: &AtomicUsize, requested: usize) -> usize {
    let mut available = remaining.load(Ordering::Acquire);
    loop {
        let reserved = available.min(requested);
        match remaining.compare_exchange_weak(
            available,
            available - reserved,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return reserved,
            Err(actual) => available = actual,
        }
    }
}

async fn joined(
    handle: tokio::task::JoinHandle<Result<Pipe, String>>,
    name: &str,
) -> Result<Pipe, String> {
    handle
        .await
        .map_err(|_| format!("{name} capture task panicked"))?
}

async fn terminate(child: &mut tokio::process::Child) -> std::io::Result<std::process::ExitStatus> {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        let result = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
    }
    #[cfg(not(unix))]
    child.kill().await?;
    child.wait().await
}

fn shown<T, E: std::fmt::Display>(result: Result<T, E>) -> String {
    match result {
        Ok(_) => "ok".to_string(),
        Err(error) => error.to_string(),
    }
}

pub(super) fn shell(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut shell = Command::new("pwsh");
        shell
            .arg("-NoLogo")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(command);
        shell
    }

    #[cfg(not(windows))]
    {
        let mut shell = Command::new("/bin/bash");
        shell.arg("-c").arg(command);
        shell
    }
}

fn sheller() -> &'static str {
    if cfg!(windows) { "pwsh" } else { "bash" }
}
