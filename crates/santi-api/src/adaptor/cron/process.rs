use std::{
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(super) fn read(command: &Path) -> Result<Vec<u8>, String> {
    let output = run(command, Some("-l"), None)?;
    if output.0 {
        return Ok(output.1);
    }
    if String::from_utf8_lossy(&output.2).contains("no crontab for") {
        return Ok(Vec::new());
    }
    Err("alarm cron could not read the current user table".into())
}

pub(super) fn install(command: &Path, bytes: &[u8]) -> Result<(), String> {
    if run(command, Some("-"), Some(bytes))?.0 {
        return Ok(());
    }
    Err("alarm cron rejected installation; adaptor will reconcile".into())
}

fn run(
    command: &Path,
    argument: Option<&str>,
    input: Option<&[u8]>,
) -> Result<(bool, Vec<u8>, Vec<u8>), String> {
    let mut command = Command::new(command);
    command
        .args(argument)
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("alarm cron invocation failed: {error}"))?;
    let stdout = child.stdout.take().ok_or("alarm cron stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("alarm cron stderr unavailable")?;
    let out = std::thread::spawn(move || capture(stdout));
    let err = std::thread::spawn(move || capture(stderr));
    if let Some(mut stdin) = child.stdin.take()
        && let Some(input) = input
    {
        stdin.write_all(input).map_err(|error| error.to_string())?;
    }
    let limit = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if Instant::now() >= limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err("alarm cron operation timed out; adaptor will reconcile".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout = out
        .join()
        .map_err(|_| "alarm cron stdout reader failed")??;
    let stderr = err
        .join()
        .map_err(|_| "alarm cron stderr reader failed")??;
    Ok((status.success(), stdout, stderr))
}

fn capture(reader: impl Read) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > 1_048_576 {
        return Err("alarm cron observation exceeded its byte limit".into());
    }
    Ok(bytes)
}
