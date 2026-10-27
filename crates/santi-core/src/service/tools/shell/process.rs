use super::{Bounds, Capture, Outcome, Prepared, bound, captured};
use crate::service::interrupt::Control;
use std::{io, process::ExitStatus, time::Duration};
use tokio::{process::Child, task::JoinHandle};

type Reader = JoinHandle<Result<bound::Pipe, String>>;

struct Process {
    child: Child,
    group: Option<u32>,
    stdout: Reader,
    stderr: Reader,
}

#[derive(Default)]
struct Pending {
    status: Option<io::Result<ExitStatus>>,
    stdout: Option<Result<bound::Pipe, String>>,
    stderr: Option<Result<bound::Pipe, String>>,
}

struct Collected {
    status: ExitStatus,
    stdout: bound::Pipe,
    stderr: bound::Pipe,
}

enum End {
    Complete,
    Expired,
    Stopped(crate::turn::Cause),
}

pub(in crate::service::tools) async fn ran(
    prepared: Prepared,
    bounds: Bounds,
    control: &Control,
) -> Outcome {
    let Prepared { command, cwd } = prepared;
    let mut process = match Process::spawn(command, bounds.bytes) {
        Ok(process) => process,
        Err(error) => return error,
    };
    let (end, mut pending) = process.wait(bounds.timeout, control).await;
    if !matches!(end, End::Complete)
        && let Err(error) = process.kill()
    {
        process.stdout.abort();
        process.stderr.abort();
        return Outcome::Unknown(format!(
            "shell deadline/stop could not reclaim its process group: {error}"
        ));
    }
    let collected =
        tokio::time::timeout(Duration::from_secs(1), pending.finish(&mut process)).await;
    let parts = match collected {
        Ok(Ok(parts)) => parts,
        Ok(Err(error)) => return Outcome::Unknown(error),
        Err(_) => {
            process.stdout.abort();
            process.stderr.abort();
            return Outcome::Unknown("shell process was interrupted but its status or output pipes did not close; partial external effects remain possible".into());
        }
    };
    if let End::Stopped(cause) = end {
        return Outcome::Stopped(format!(
            "shell process tree interrupted by {}",
            cause.encode()
        ));
    }
    let seconds = bounds.timeout.as_secs_f64();
    let output = captured(Capture {
        status: parts.status,
        stdout: parts.stdout,
        stderr: parts.stderr,
        cwd,
        bounds,
    });
    match (end, output) {
        (End::Expired, Outcome::Captured(mut output)) => {
            output["timed_out"] = true.into();
            output["timeout_seconds"] = seconds.into();
            Outcome::Expired(output)
        }
        (_, output) => output,
    }
}

impl Process {
    fn spawn(command: std::process::Command, bytes: Option<usize>) -> Result<Self, Outcome> {
        let mut command = tokio::process::Command::from(command);
        command.kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|error| Outcome::Failed(format!("failed to spawn shell process: {error}")))?;
        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            let _ = child.start_kill();
            return Err(Outcome::Unknown(
                "shell was spawned but stdout or stderr pipe was unavailable".into(),
            ));
        };
        Ok(Self {
            group: child.id(),
            child,
            stdout: tokio::spawn(bound::piped(stdout, bytes)),
            stderr: tokio::spawn(bound::piped(stderr, bytes)),
        })
    }

    async fn wait(&mut self, timeout: Duration, control: &Control) -> (End, Pending) {
        let mut pending = Pending::default();
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline);
        let end = loop {
            if pending.status.is_some() && pending.stdout.is_some() && pending.stderr.is_some() {
                break End::Complete;
            }
            tokio::select! {
                status = self.child.wait(), if pending.status.is_none() => pending.status = Some(status),
                stdout = &mut self.stdout, if pending.stdout.is_none() => pending.stdout = Some(joined(stdout, "stdout")),
                stderr = &mut self.stderr, if pending.stderr.is_none() => pending.stderr = Some(joined(stderr, "stderr")),
                cause = control.wait() => break End::Stopped(cause),
                _ = &mut deadline => break End::Expired,
            }
        };
        (end, pending)
    }

    fn kill(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        if let Some(group) = self.group {
            let result = unsafe { libc::kill(-(group as i32), libc::SIGKILL) };
            if result != 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
                }
            }
        }
        #[cfg(not(unix))]
        self.child.start_kill()?;
        Ok(())
    }
}

impl Pending {
    async fn finish(&mut self, process: &mut Process) -> Result<Collected, String> {
        if self.status.is_none() {
            self.status = Some(process.child.wait().await);
        }
        if self.stdout.is_none() {
            self.stdout = Some(joined((&mut process.stdout).await, "stdout"));
        }
        if self.stderr.is_none() {
            self.stderr = Some(joined((&mut process.stderr).await, "stderr"));
        }
        Ok(Collected {
            status: self
                .status
                .take()
                .expect("collected status")
                .map_err(|error| error.to_string())?,
            stdout: self.stdout.take().expect("collected stdout")?,
            stderr: self.stderr.take().expect("collected stderr")?,
        })
    }
}

fn joined(
    result: Result<Result<bound::Pipe, String>, tokio::task::JoinError>,
    stream: &str,
) -> Result<bound::Pipe, String> {
    result.map_err(|_| format!("{stream} capture task panicked"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;

    async fn invoke(command: &str, timeout: Duration, control: &Control) -> Outcome {
        let temp = tempfile::tempdir().unwrap();
        let mut command = super::super::shell(command);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        ran(
            Prepared {
                command,
                cwd: temp.path().to_path_buf(),
            },
            Bounds {
                timeout,
                bytes: Some(1024),
                lines: Some(16),
                saved: None,
            },
            control,
        )
        .await
    }

    #[tokio::test]
    async fn foreground() {
        for command in ["sleep 30", "printf before; sleep 30 &"] {
            let started = std::time::Instant::now();
            let result = invoke(command, Duration::from_millis(150), &Control::new()).await;
            let Outcome::Expired(output) = result else {
                panic!("expected a bounded deadline result")
            };
            assert_eq!(output["timed_out"], true);
            assert!(started.elapsed() < Duration::from_secs(3));
            if command.starts_with("printf") {
                assert_eq!(output["stdout"], "before");
                assert_eq!(output["exit_code"], 0);
            }
        }
    }

    #[tokio::test]
    async fn completed() {
        let result = invoke("printf ready", Duration::from_secs(5), &Control::new()).await;
        let Outcome::Captured(output) = result else {
            panic!("short command must complete")
        };
        assert_eq!(output["stdout"], "ready");
        assert!(output.get("timed_out").is_none());
    }

    #[tokio::test]
    async fn stopped() {
        let control = Control::new();
        control.stop(crate::turn::Cause::Operator);
        let result = invoke("sleep 30", Duration::from_secs(5), &control).await;
        assert!(matches!(result, Outcome::Stopped(_)));
    }
}
