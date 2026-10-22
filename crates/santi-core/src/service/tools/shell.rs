use std::{path::PathBuf, process::Command};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::service::interrupt::Control;

mod bound;
mod definition;
mod redact;

pub(super) use bound::{Bounds, Saved};

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

pub(super) async fn ran(prepared: Prepared, bounds: Bounds, control: &Control) -> Outcome {
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
    let stdout = tokio::spawn(bound::piped(stdout, bounds.bytes));
    let stderr = tokio::spawn(bound::piped(stderr, bounds.bytes));
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
        bounds,
    })
}

struct Capture {
    status: std::process::ExitStatus,
    stdout: bound::Pipe,
    stderr: bound::Pipe,
    cwd: PathBuf,
    bounds: Bounds,
}

fn captured(capture: Capture) -> Outcome {
    let Capture {
        status,
        stdout,
        stderr,
        cwd,
        bounds,
    } = capture;
    let bytes = bounds.bytes.unwrap_or(usize::MAX);
    let lines = bounds.lines.unwrap_or(usize::MAX);
    let [(out, above), (err, below)] = bound::shared(&stdout, &stderr, bytes, lines);
    let saved = bounds.saved.as_ref();
    let stdout = bound::rendered(stdout, out, above, saved.map(|saved| (saved, "stdout")));
    let stderr = bound::rendered(stderr, err, below, saved.map(|saved| (saved, "stderr")));
    let redactions = stdout.redactions + stderr.redactions;
    let mut output = json!({
        "exit_code": status.code().unwrap_or(-1),
        "stdout": stdout.value,
        "stderr": stderr.value,
        "shell": sheller(),
        "cwd": cwd.display().to_string(),
    });
    if let Some(limit) = bounds.bytes {
        output["output_truncated"] = Value::Bool(stdout.truncated || stderr.truncated);
        output["output_limit_bytes"] = Value::from(limit);
    }
    if let Some(limit) = bounds.lines {
        output["output_limit_lines"] = Value::from(limit);
    }
    let saved = [("stdout", stdout.saved), ("stderr", stderr.saved)]
        .into_iter()
        .filter_map(|(stream, uri)| uri.map(|uri| (stream.to_string(), Value::from(uri))))
        .collect::<serde_json::Map<_, _>>();
    if !saved.is_empty() {
        output["saved"] = Value::Object(saved);
    }
    if redactions > 0 {
        output["redacted"] = Value::Bool(true);
        output["redactions"] = Value::from(redactions);
    }
    Outcome::Captured(output)
}

async fn joined(
    handle: tokio::task::JoinHandle<Result<bound::Pipe, String>>,
    name: &str,
) -> Result<bound::Pipe, String> {
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
