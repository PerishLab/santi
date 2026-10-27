use std::{path::PathBuf, process::Command};

use serde::Deserialize;
use serde_json::{Value, json};

mod bound;
mod definition;
mod process;
mod redact;
pub(super) use process::ran;

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
    Expired(Value),
    Failed(String),
    Unknown(String),
    Stopped(String),
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
